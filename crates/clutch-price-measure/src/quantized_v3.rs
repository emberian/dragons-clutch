//! Additive quantized price-measure certificates for finite degree-zero and
//! smooth degree-one-through-three payout geometries.

use super::{
    accumulate_quantized_weights, finish_quantized_reconstruction, validate_atom_components,
    validate_price_components, BasisSpec, QuantizedCoreError, ValidatedBasisSpec, MAX_OUTCOMES,
    MAX_QUANTIZED_ATOMS, PRICE_MEASURE_WITNESS_VERSION_V3, QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3,
    QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};

/// Canonical marker in every unused degree-zero payout-map slot.
pub const PAYOUT_MAP_UNUSED_V3: u8 = u8::MAX;

const _: () = assert!(PAYOUT_MAP_UNUSED_V3 == u8::MAX);

/// Adapter-authenticated bindings for one V3 witness body.
///
/// The adapter owns the canonical byte codecs and digest construction. This
/// arithmetic crate only compares already-authenticated byte arrays. Before
/// calling a V3 checker, the adapter must prove the evaluator's Product fields
/// equal the basis artifact, its coordinate bounds equal the relation/domain,
/// and that relation/domain binds the exact basis identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterBindingsV3 {
    /// Candidate-feed account identity.
    pub candidate_feed: [u8; 32],
    /// Digest of coordinate bounds and their join to the exact basis identity.
    pub relation_domain_digest: [u8; 32],
    /// Canonical `NativeClaimBasisV1Id` bytes supplied by the adapter.
    ///
    /// For degree zero this identity covers
    /// rows, map, knots, payout denominator, and immutable edge/ambiguity
    /// registry selectors. Coordinate bounds and the join to this exact basis
    /// identity remain owned by `relation_domain_digest`.
    pub basis_digest: [u8; 32],
    /// Digest of the exact candidate price vector.
    pub candidate_price_digest: [u8; 32],
    /// Digest recomputed over canonical V3 witness bytes excluding its digest field.
    pub observed_body_digest: [u8; 32],
}

/// Exact finite payout geometry for a degree-zero native-claim basis.
///
/// There are `native_outcome_count` ordered coordinate cells. The
/// `native_outcome_count - 1` active knots divide the authenticated closed
/// interval `domain_min..=domain_max`; equality at a knot selects the cell to
/// its right. Each cell maps to one of `payout_count` distinct exact simplex
/// rows. Row identifiers are canonical by first use, so the first map entry is
/// zero and each later entry is an existing row or exactly the next new row.
///
/// This is an ephemeral checked projection that combines an authenticated
/// Product basis artifact with separately authenticated concrete
/// relation-domain bounds. It is not a persisted artifact and does not have
/// one combined canonical digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DegreeZeroPayoutTableV3 {
    /// Native claim width and coordinate-cell count in `2..=16`.
    pub native_outcome_count: u8,
    /// Number of distinct active payout rows in `1..=native_outcome_count`.
    pub payout_count: u8,
    /// Exact `native_outcome_count - 1`.
    pub knot_count: u8,
    /// Positive common denominator of every active payout row.
    pub payout_denominator: u64,
    /// Inclusive authenticated lower coordinate bound.
    pub domain_min: u128,
    /// Inclusive authenticated upper coordinate bound.
    pub domain_max: u128,
    /// Active exact simplex rows, then all-zero row and column padding.
    pub payout_weights: [[u64; MAX_OUTCOMES]; MAX_OUTCOMES],
    /// Active canonical cell-to-row map, then [`PAYOUT_MAP_UNUSED_V3`].
    pub payout_map: [u8; MAX_OUTCOMES],
    /// Strictly increasing interior boundaries, then zero padding.
    pub knots: [u128; MAX_OUTCOMES],
}

impl DegreeZeroPayoutTableV3 {
    /// Validate the complete finite table and all canonical padding.
    pub fn validate(&self) -> ResultV3<()> {
        validate_degree_zero_table(self).map(|_| ())
    }

    /// Evaluate one in-domain coordinate to its exact native payout row.
    ///
    /// The complete table is validated before evaluation. Callers checking a
    /// witness should use the accumulator/checker, which validates only once.
    pub fn evaluate(&self, coordinate: u128) -> ResultV3<DegreeZeroPayoutVectorV3> {
        let validated = validate_degree_zero_table(self)?;
        let weights = validated.evaluate(coordinate)?;
        Ok(DegreeZeroPayoutVectorV3 {
            native_outcome_count: self.native_outcome_count,
            payout_denominator: self.payout_denominator,
            weights,
        })
    }
}

/// One exact degree-zero native payout vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DegreeZeroPayoutVectorV3 {
    /// Active native claim width.
    pub native_outcome_count: u8,
    /// Common positive payout denominator.
    pub payout_denominator: u64,
    /// Active exact simplex weights, then zero padding.
    pub weights: [u64; MAX_OUTCOMES],
}

/// Exact already-quantized candidate prices for V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceVectorV3 {
    /// Degree zero for a finite table, or degree one through three for a smooth basis.
    pub basis_degree: u8,
    /// Active native claim-price prefix in `2..=16`.
    pub native_outcome_count: u8,
    /// Positive integer simplex scale.
    pub price_scale: u64,
    /// Active native claim prices summing exactly to `price_scale`, then zero padding.
    pub prices: [u64; MAX_OUTCOMES],
}

/// Canonical finite-mixture certificate for the V3 quantized payout family.
///
/// Active atoms are strictly coordinate-sorted and positive-mass. Degree zero
/// admits only the finite table's authenticated closed domain. Degrees one
/// through three admit only the closed stored-knot interval. Every inactive
/// field is canonical zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomWitnessV3 {
    /// Exact [`PRICE_MEASURE_WITNESS_VERSION_V3`].
    pub schema_version: u8,
    /// Exact [`QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1`].
    ///
    /// This scalar marker freezes exact mapped rows at degree zero; frozen
    /// largest-remainder/lowest-index B-spline evaluation at degrees one
    /// through three; upstream-exact candidate prices; strict atom ordering;
    /// primitive mass scale; canonical padding; and exact reconstruction.
    pub quantized_semantics_version: u8,
    /// Must repeat the authenticated candidate-feed identity.
    pub candidate_feed: [u8; 32],
    /// Must repeat the authenticated relation-domain digest.
    pub relation_domain_digest: [u8; 32],
    /// Must repeat the authenticated canonical Product basis-artifact digest.
    pub basis_digest: [u8; 32],
    /// Must repeat the authenticated exact-price digest.
    pub candidate_price_digest: [u8; 32],
    /// Digest of adapter-owned canonical V3 body bytes excluding this field.
    pub body_digest: [u8; 32],
    /// Must repeat [`PriceVectorV3::basis_degree`].
    pub basis_degree: u8,
    /// Must repeat [`PriceVectorV3::native_outcome_count`].
    pub native_outcome_count: u8,
    /// Number of active sorted atoms in `1..=native_outcome_count`.
    pub atom_count: u8,
    /// Primitive positive common denominator of the atom masses.
    pub common_denominator: u64,
    /// Integer resolved coordinates in strictly ascending active order.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Positive active masses summing exactly to `common_denominator`.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

/// Successful V3 certificate summary for a versioned adapter checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedPriceMeasureV3 {
    /// Checked basis degree.
    pub basis_degree: u8,
    /// Checked active native claim width.
    pub native_outcome_count: u8,
    /// Degree-zero cells or `native_outcome_count - basis_degree` smooth regions.
    pub basis_region_count: u8,
    /// Primitive checked witness denominator.
    pub common_denominator: u64,
    /// Adapter-authenticated canonical V3 body digest.
    pub body_digest: [u8; 32],
}

/// Adapter binding coordinate that did not match authenticated truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingFieldV3 {
    /// Candidate-feed identity.
    CandidateFeed,
    /// Relation-domain digest.
    RelationDomainDigest,
    /// Canonical immutable basis-artifact digest.
    BasisDigest,
    /// Exact-price digest.
    CandidatePriceDigest,
    /// Canonical witness-body digest.
    BodyDigest,
}

/// Total hostile-input refusal set for V3 quantized certificate checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV3 {
    /// Certificate schema version is not V3.
    UnsupportedSchemaVersion,
    /// Quantized evaluator/rounding/order/reconstruction semantics are not V1.
    UnsupportedQuantizedSemanticsVersion,
    /// An adapter-authenticated identity or digest did not match.
    BindingMismatch {
        /// First mismatching authenticated coordinate.
        field: BindingFieldV3,
    },
    /// Basis degree was outside the checker-specific V3 range.
    InvalidDegree,
    /// Native claim width was outside the degree-specific bound and `2..=16`.
    InvalidNativeOutcomeCount,
    /// Witness degree or native claim width differed from the price vector.
    WitnessShapeMismatch,
    /// Immutable smooth basis validation or price/basis shape equality failed.
    InvalidBasis,
    /// Degree-zero payout count or knot count was not canonical for its width.
    InvalidDegreeZeroShape,
    /// Degree-zero domain bounds did not contain nonempty ordered cells.
    InvalidDegreeZeroDomain,
    /// A direct degree-zero evaluation coordinate was outside the closed domain.
    DegreeZeroCoordinateOutOfRange,
    /// Degree-zero payout denominator was zero.
    InvalidPayoutDenominator,
    /// An active degree-zero row component exceeded its denominator.
    PayoutWeightExceedsDenominator {
        /// Active payout row.
        row: u8,
        /// Native outcome component.
        outcome: u8,
    },
    /// An active degree-zero payout row did not sum to its denominator.
    PayoutRowSimplexMismatch {
        /// First invalid active payout row.
        row: u8,
    },
    /// A degree-zero payout row or column padding cell was nonzero.
    NonCanonicalPayoutPadding {
        /// Padded payout row.
        row: u8,
        /// Padded native outcome component.
        outcome: u8,
    },
    /// Two active degree-zero payout rows were identical.
    DuplicatePayoutRow {
        /// Earlier identical row.
        first: u8,
        /// Later identical row.
        second: u8,
    },
    /// An active degree-zero cell mapped outside the active payout rows.
    PayoutMapOutOfRange {
        /// First invalid cell.
        cell: u8,
    },
    /// Degree-zero row identifiers did not follow canonical first-use order.
    NonCanonicalPayoutMapOrder {
        /// First noncanonical cell.
        cell: u8,
    },
    /// An inactive degree-zero map slot was not the unused marker.
    NonCanonicalPayoutMapPadding {
        /// First invalid inactive cell.
        cell: u8,
    },
    /// An active degree-zero knot was not strictly ordered inside the domain.
    InvalidDegreeZeroKnot {
        /// First invalid active knot.
        knot: u8,
    },
    /// An inactive degree-zero knot was nonzero.
    NonCanonicalKnotPadding {
        /// First nonzero inactive knot.
        knot: u8,
    },
    /// Price scale was zero.
    InvalidPriceScale,
    /// An active price exceeded the simplex scale.
    PriceExceedsScale {
        /// First active native outcome exceeding the scale.
        outcome: u8,
    },
    /// Active prices did not sum exactly to the simplex scale.
    PriceSimplexMismatch,
    /// An inactive price was nonzero.
    NonCanonicalPricePadding {
        /// First nonzero inactive native outcome.
        outcome: u8,
    },
    /// Common witness denominator was zero.
    InvalidCommonDenominator,
    /// Atom count was zero or exceeded the native affine support bound.
    InvalidAtomCount,
    /// An active atom coordinate lay outside its canonical interval.
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
    /// A staged append was attempted after every atom was consumed.
    AtomCursorExhausted,
    /// Reconstruction was requested before every active atom was consumed.
    IncompleteAtomAccumulation {
        /// Exact next atom that remained.
        cursor: u8,
        /// Total active atom count.
        atom_count: u8,
    },
    /// Exact reconstruction disagreed with a candidate price.
    PriceReconstructionMismatch {
        /// First mismatching native outcome.
        outcome: u8,
    },
    /// A checked operation overflowed despite the validated envelope.
    ArithmeticOverflow,
}

/// Result alias for V3 quantized certificate operations.
pub type ResultV3<T> = core::result::Result<T, ErrorV3>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvaluatorV3<'a> {
    degree_zero: Option<ValidatedDegreeZeroPayoutTableV3<'a>>,
    smooth: Option<ValidatedBasisSpec>,
}

impl<'a> EvaluatorV3<'a> {
    fn degree_zero(table: ValidatedDegreeZeroPayoutTableV3<'a>) -> Self {
        Self {
            degree_zero: Some(table),
            smooth: None,
        }
    }

    fn smooth(basis: ValidatedBasisSpec) -> Self {
        Self {
            degree_zero: None,
            smooth: Some(basis),
        }
    }

    fn payout_denominator(&self) -> ResultV3<u64> {
        if let Some(table) = &self.degree_zero {
            Ok(table.table.payout_denominator)
        } else if let Some(basis) = &self.smooth {
            Ok(basis.spec().denominator)
        } else {
            Err(ErrorV3::InvalidBasis)
        }
    }

    fn evaluate(&self, coordinate: u128) -> ResultV3<[u64; MAX_OUTCOMES]> {
        if let Some(table) = &self.degree_zero {
            table.evaluate(coordinate)
        } else if let Some(basis) = &self.smooth {
            basis
                .evaluate_point(coordinate)
                .map(|weights| weights.weights)
                .map_err(|_| ErrorV3::InvalidBasis)
        } else {
            Err(ErrorV3::InvalidBasis)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ValidatedDegreeZeroPayoutTableV3<'a> {
    table: &'a DegreeZeroPayoutTableV3,
}

impl ValidatedDegreeZeroPayoutTableV3<'_> {
    fn evaluate(&self, coordinate: u128) -> ResultV3<[u64; MAX_OUTCOMES]> {
        if coordinate < self.table.domain_min || coordinate > self.table.domain_max {
            return Err(ErrorV3::DegreeZeroCoordinateOutOfRange);
        }
        let mut cell = 0_usize;
        let knots = usize::from(self.table.knot_count);
        while cell < knots && coordinate >= self.table.knots[cell] {
            cell += 1;
        }
        Ok(self.table.payout_weights[usize::from(self.table.payout_map[cell])])
    }
}

/// Validated append-only V3 quantized certificate verifier.
///
/// Fields are private so cursor, validated evaluator, and partial sums cannot
/// be forged. Every refusing append leaves the accumulator unchanged. Cloning
/// or forking this pure arithmetic value carries no persisted authority: only
/// a separately versioned adapter checkpoint may authorize later protocol work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuantizedPriceMeasureAccumulatorV3<'a> {
    evaluator: EvaluatorV3<'a>,
    prices: &'a PriceVectorV3,
    witness: &'a QuantizedAtomWitnessV3,
    atom_cursor: u8,
    accumulators: [u128; MAX_OUTCOMES],
}

const _: () = assert!(core::mem::size_of::<QuantizedPriceMeasureAccumulatorV3<'static>>() <= 1_536);

impl<'a> QuantizedPriceMeasureAccumulatorV3<'a> {
    /// Validate a degree-zero finite payout certificate and open its staged sum.
    pub fn begin_degree_zero(
        expected: &AdapterBindingsV3,
        table: &'a DegreeZeroPayoutTableV3,
        prices: &'a PriceVectorV3,
        witness: &'a QuantizedAtomWitnessV3,
    ) -> ResultV3<Self> {
        validate_header(expected, prices, witness)?;
        if prices.basis_degree != 0 {
            return Err(ErrorV3::InvalidDegree);
        }
        validate_shape(prices.basis_degree, prices.native_outcome_count)?;
        let table = validate_degree_zero_table(table)?;
        if table.table.native_outcome_count != prices.native_outcome_count {
            return Err(ErrorV3::InvalidDegreeZeroShape);
        }
        validate_common_body(
            table.table.domain_min,
            table.table.domain_max,
            prices,
            witness,
        )?;
        Ok(Self::new(EvaluatorV3::degree_zero(table), prices, witness))
    }

    /// Validate a smooth degree-one-through-three certificate and open its staged sum.
    ///
    /// `basis` is an ephemeral joined projection, not the object hashed by the
    /// V3 `basis_digest`; the adapter must prove both of its projections first.
    pub fn begin_smooth(
        expected: &AdapterBindingsV3,
        basis: &BasisSpec,
        prices: &'a PriceVectorV3,
        witness: &'a QuantizedAtomWitnessV3,
    ) -> ResultV3<Self> {
        validate_header(expected, prices, witness)?;
        if prices.basis_degree == 0 {
            return Err(ErrorV3::InvalidDegree);
        }
        validate_shape(prices.basis_degree, prices.native_outcome_count)?;
        let validated_basis = basis.validated().map_err(|_| ErrorV3::InvalidBasis)?;
        if basis.degree != prices.basis_degree || basis.outcome_count != prices.native_outcome_count
        {
            return Err(ErrorV3::InvalidBasis);
        }
        let first = basis.knots[0];
        let last = basis.knots[usize::from(basis.knot_count) - 1];
        validate_common_body(first, last, prices, witness)?;
        Ok(Self::new(
            EvaluatorV3::smooth(validated_basis),
            prices,
            witness,
        ))
    }

    fn new(
        evaluator: EvaluatorV3<'a>,
        prices: &'a PriceVectorV3,
        witness: &'a QuantizedAtomWitnessV3,
    ) -> Self {
        Self {
            evaluator,
            prices,
            witness,
            atom_cursor: 0,
            accumulators: [0; MAX_OUTCOMES],
        }
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
    pub fn accumulate_atom(&mut self, atom: u8) -> ResultV3<()> {
        if atom != self.atom_cursor {
            return Err(ErrorV3::AtomCursorMismatch {
                expected: self.atom_cursor,
                provided: atom,
            });
        }
        if atom >= self.witness.atom_count {
            return Err(ErrorV3::AtomCursorExhausted);
        }
        let index = usize::from(atom);
        let weights = self
            .evaluator
            .evaluate(self.witness.atom_coordinates[index])?;
        let accumulators = accumulate_quantized_weights(
            &weights,
            self.prices.native_outcome_count,
            self.witness.atom_masses[index],
            self.accumulators,
        )
        .map_err(core_error_v3)?;
        let next = atom.checked_add(1).ok_or(ErrorV3::ArithmeticOverflow)?;
        self.accumulators = accumulators;
        self.atom_cursor = next;
        Ok(())
    }

    /// Finish exact reconstruction after consuming every active atom.
    pub fn finish(self) -> ResultV3<VerifiedPriceMeasureV3> {
        if self.atom_cursor != self.witness.atom_count {
            return Err(ErrorV3::IncompleteAtomAccumulation {
                cursor: self.atom_cursor,
                atom_count: self.witness.atom_count,
            });
        }
        finish_quantized_reconstruction(
            self.prices.native_outcome_count,
            self.prices.price_scale,
            &self.prices.prices,
            self.evaluator.payout_denominator()?,
            self.witness.common_denominator,
            &self.accumulators,
        )
        .map_err(core_error_v3)?;
        Ok(VerifiedPriceMeasureV3 {
            basis_degree: self.prices.basis_degree,
            native_outcome_count: self.prices.native_outcome_count,
            basis_region_count: if self.prices.basis_degree == 0 {
                self.prices.native_outcome_count
            } else {
                self.prices.native_outcome_count - self.prices.basis_degree
            },
            common_denominator: self.witness.common_denominator,
            body_digest: self.witness.body_digest,
        })
    }
}

/// Verify a V3 support-bounded measure over a degree-zero finite payout table.
pub fn verify_quantized_price_measure_v3_degree_zero(
    expected: &AdapterBindingsV3,
    table: &DegreeZeroPayoutTableV3,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> ResultV3<VerifiedPriceMeasureV3> {
    finish_all(QuantizedPriceMeasureAccumulatorV3::begin_degree_zero(
        expected, table, prices, witness,
    )?)
}

/// Verify a V3 support-bounded measure over a smooth degree-one-through-three basis.
pub fn verify_quantized_price_measure_v3_smooth(
    expected: &AdapterBindingsV3,
    basis: &BasisSpec,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> ResultV3<VerifiedPriceMeasureV3> {
    finish_all(QuantizedPriceMeasureAccumulatorV3::begin_smooth(
        expected, basis, prices, witness,
    )?)
}

fn finish_all(
    mut accumulator: QuantizedPriceMeasureAccumulatorV3<'_>,
) -> ResultV3<VerifiedPriceMeasureV3> {
    while accumulator.atom_cursor() < accumulator.atom_count() {
        let atom = accumulator.atom_cursor();
        accumulator.accumulate_atom(atom)?;
    }
    accumulator.finish()
}

fn validate_header(
    expected: &AdapterBindingsV3,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> ResultV3<()> {
    if witness.schema_version != PRICE_MEASURE_WITNESS_VERSION_V3 {
        return Err(ErrorV3::UnsupportedSchemaVersion);
    }
    if witness.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1 {
        return Err(ErrorV3::UnsupportedQuantizedSemanticsVersion);
    }
    for (matches, field) in [
        (
            witness.candidate_feed == expected.candidate_feed,
            BindingFieldV3::CandidateFeed,
        ),
        (
            witness.relation_domain_digest == expected.relation_domain_digest,
            BindingFieldV3::RelationDomainDigest,
        ),
        (
            witness.basis_digest == expected.basis_digest,
            BindingFieldV3::BasisDigest,
        ),
        (
            witness.candidate_price_digest == expected.candidate_price_digest,
            BindingFieldV3::CandidatePriceDigest,
        ),
        (
            witness.body_digest == expected.observed_body_digest,
            BindingFieldV3::BodyDigest,
        ),
    ] {
        if !matches {
            return Err(ErrorV3::BindingMismatch { field });
        }
    }
    if witness.basis_degree != prices.basis_degree
        || witness.native_outcome_count != prices.native_outcome_count
    {
        return Err(ErrorV3::WitnessShapeMismatch);
    }
    Ok(())
}

fn validate_shape(degree: u8, native_outcome_count: u8) -> ResultV3<()> {
    if degree > QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3 {
        return Err(ErrorV3::InvalidDegree);
    }
    if native_outcome_count < 2
        || native_outcome_count < degree + 1
        || usize::from(native_outcome_count) > MAX_OUTCOMES
    {
        return Err(ErrorV3::InvalidNativeOutcomeCount);
    }
    Ok(())
}

fn validate_common_body(
    first: u128,
    last: u128,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> ResultV3<()> {
    validate_price_components(
        prices.price_scale,
        &prices.prices,
        usize::from(prices.native_outcome_count),
    )
    .map_err(core_error_v3)?;
    validate_atom_components(
        first,
        last,
        prices.native_outcome_count,
        witness.atom_count,
        witness.common_denominator,
        &witness.atom_coordinates,
        &witness.atom_masses,
    )
    .map_err(core_error_v3)
}

fn validate_degree_zero_table(
    table: &DegreeZeroPayoutTableV3,
) -> ResultV3<ValidatedDegreeZeroPayoutTableV3<'_>> {
    let outcomes = usize::from(table.native_outcome_count);
    let payouts = usize::from(table.payout_count);
    if !(2..=MAX_OUTCOMES).contains(&outcomes)
        || payouts == 0
        || payouts > outcomes
        || usize::from(table.knot_count) != outcomes - 1
    {
        return Err(ErrorV3::InvalidDegreeZeroShape);
    }
    if table.domain_min >= table.domain_max {
        return Err(ErrorV3::InvalidDegreeZeroDomain);
    }
    if table.payout_denominator == 0 {
        return Err(ErrorV3::InvalidPayoutDenominator);
    }

    let mut row = 0_u8;
    while usize::from(row) < MAX_OUTCOMES {
        let mut sum = 0_u128;
        let mut outcome = 0_u8;
        while usize::from(outcome) < MAX_OUTCOMES {
            let weight = table.payout_weights[usize::from(row)][usize::from(outcome)];
            if usize::from(row) < payouts && usize::from(outcome) < outcomes {
                if weight > table.payout_denominator {
                    return Err(ErrorV3::PayoutWeightExceedsDenominator { row, outcome });
                }
                sum = sum
                    .checked_add(u128::from(weight))
                    .ok_or(ErrorV3::ArithmeticOverflow)?;
            } else if weight != 0 {
                return Err(ErrorV3::NonCanonicalPayoutPadding { row, outcome });
            }
            outcome += 1;
        }
        if usize::from(row) < payouts && sum != u128::from(table.payout_denominator) {
            return Err(ErrorV3::PayoutRowSimplexMismatch { row });
        }
        row += 1;
    }

    let mut first = 0_u8;
    while usize::from(first) < payouts {
        let mut second = first + 1;
        while usize::from(second) < payouts {
            if table.payout_weights[usize::from(first)] == table.payout_weights[usize::from(second)]
            {
                return Err(ErrorV3::DuplicatePayoutRow { first, second });
            }
            second += 1;
        }
        first += 1;
    }

    let mut next_new_row = 0_u8;
    let mut cell = 0_u8;
    while usize::from(cell) < MAX_OUTCOMES {
        let value = table.payout_map[usize::from(cell)];
        if usize::from(cell) < outcomes {
            if value >= table.payout_count {
                return Err(ErrorV3::PayoutMapOutOfRange { cell });
            }
            if value > next_new_row {
                return Err(ErrorV3::NonCanonicalPayoutMapOrder { cell });
            }
            if value == next_new_row {
                next_new_row = next_new_row
                    .checked_add(1)
                    .ok_or(ErrorV3::ArithmeticOverflow)?;
            }
        } else if value != PAYOUT_MAP_UNUSED_V3 {
            return Err(ErrorV3::NonCanonicalPayoutMapPadding { cell });
        }
        cell += 1;
    }
    if next_new_row != table.payout_count {
        return Err(ErrorV3::InvalidDegreeZeroShape);
    }

    let knots = usize::from(table.knot_count);
    let mut knot = 0_u8;
    while usize::from(knot) < MAX_OUTCOMES {
        let value = table.knots[usize::from(knot)];
        if usize::from(knot) < knots {
            let below_domain = knot == 0 && value <= table.domain_min;
            let not_increasing = knot != 0 && value <= table.knots[usize::from(knot - 1)];
            if below_domain || not_increasing || value > table.domain_max {
                return Err(ErrorV3::InvalidDegreeZeroKnot { knot });
            }
        } else if value != 0 {
            return Err(ErrorV3::NonCanonicalKnotPadding { knot });
        }
        knot += 1;
    }

    Ok(ValidatedDegreeZeroPayoutTableV3 { table })
}

const fn core_error_v3(error: QuantizedCoreError) -> ErrorV3 {
    match error {
        QuantizedCoreError::InvalidBasis => ErrorV3::InvalidBasis,
        QuantizedCoreError::InvalidPriceScale => ErrorV3::InvalidPriceScale,
        QuantizedCoreError::PriceExceedsScale { outcome } => ErrorV3::PriceExceedsScale { outcome },
        QuantizedCoreError::PriceSimplexMismatch => ErrorV3::PriceSimplexMismatch,
        QuantizedCoreError::NonCanonicalPricePadding { outcome } => {
            ErrorV3::NonCanonicalPricePadding { outcome }
        }
        QuantizedCoreError::InvalidCommonDenominator => ErrorV3::InvalidCommonDenominator,
        QuantizedCoreError::InvalidAtomCount => ErrorV3::InvalidAtomCount,
        QuantizedCoreError::AtomCoordinateOutOfRange { atom } => {
            ErrorV3::AtomCoordinateOutOfRange { atom }
        }
        QuantizedCoreError::NonCanonicalAtomOrder { atom } => {
            ErrorV3::NonCanonicalAtomOrder { atom }
        }
        QuantizedCoreError::ZeroAtomMass { atom } => ErrorV3::ZeroAtomMass { atom },
        QuantizedCoreError::NonCanonicalAtomPadding { atom } => {
            ErrorV3::NonCanonicalAtomPadding { atom }
        }
        QuantizedCoreError::AtomMassMismatch => ErrorV3::AtomMassMismatch,
        QuantizedCoreError::NonPrimitiveAtomScale => ErrorV3::NonPrimitiveAtomScale,
        QuantizedCoreError::PriceReconstructionMismatch { outcome } => {
            ErrorV3::PriceReconstructionMismatch { outcome }
        }
        QuantizedCoreError::ArithmeticOverflow => ErrorV3::ArithmeticOverflow,
    }
}
