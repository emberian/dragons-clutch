//! Exact positive certificates for the production quantized spline image.

use clutch_bspline::{BasisSpec, EdgePolicy, ValidatedBasisSpec};

use crate::{MAX_OUTCOMES, MAX_QUANTIZED_ATOMS};

/// Magic prefix of the canonical certificate body.
pub const QUANTIZED_ATOM_MIXTURE_MAGIC_V1: [u8; 8] = *b"DCQAMV1\0";
/// Exact schema version of [`QuantizedAtomMixtureCertificateV1`].
pub const QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1: u8 = 1;
/// Exact evaluator, price-scale, ordering, and reconstruction semantics.
pub const QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1: u8 = 1;
/// Exact affine-Caratheodory support profile selected by this checker.
pub const QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1: u8 = 1;
/// Exact canonical byte width of [`QuantizedAtomMixtureCertificateV1`].
pub const QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1: usize = 544;

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_QUANTIZED_ATOMS == MAX_OUTCOMES);

/// Identities repeated by one certificate and compared to adapter-derived truth.
///
/// These bytes do not authenticate themselves. The live adapter must derive
/// them from owner-checked canonical Market, complete Terms, Basis, and exact
/// candidate-price bodies before calling the verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct QuantizedAtomMixtureBindingsV1 {
    /// Canonical Market identity that selects the immutable Terms.
    pub market_id: [u8; 32],
    /// Complete immutable Terms identity, including the coordinate domain.
    pub terms_id: [u8; 32],
    /// Canonical Basis identity, including knots, denominator, and selectors.
    pub basis_id: [u8; 32],
    /// Canonical identity of the exact payout-denominator-scale price vector.
    pub price_id: [u8; 32],
}

impl QuantizedAtomMixtureBindingsV1 {
    fn validate(self) -> ResultV1<()> {
        for (value, field) in [
            (self.market_id, IdentityFieldV1::Market),
            (self.terms_id, IdentityFieldV1::Terms),
            (self.basis_id, IdentityFieldV1::Basis),
            (self.price_id, IdentityFieldV1::Price),
        ] {
            if value == [0; 32] {
                return Err(ErrorV1::InvalidIdentity { field });
            }
        }
        Ok(())
    }
}

/// Complete ephemeral projection of one authenticated quantized spline.
///
/// This is not a second persisted Terms or Basis body. An adapter constructs
/// it only after checking the canonical IDs in [`Self::bindings`] against the
/// decoded semantic owners and resolving the Basis-owned edge selector through
/// the authenticated registry. The verifier then checks the complete spline,
/// exact Terms domain, and every observation against this projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundQuantizedSplineV1 {
    /// Exact expected identities, including the candidate-price identity.
    pub bindings: QuantizedAtomMixtureBindingsV1,
    /// Inclusive Terms-owned lower observation coordinate.
    pub coordinate_domain_min: u128,
    /// Inclusive Terms-owned upper observation coordinate.
    pub coordinate_domain_max: u128,
    /// Complete Terms-derived production evaluator projection.
    pub basis: BasisSpec,
}

impl BoundQuantizedSplineV1 {
    pub(crate) fn validated(self) -> ResultV1<ValidatedBasisSpec> {
        self.bindings.validate()?;
        if self.basis.degree != 2 && self.basis.degree != 3 {
            return Err(ErrorV1::InvalidDegree);
        }
        if self.basis.outcome_count <= self.basis.degree
            || usize::from(self.basis.outcome_count) > MAX_OUTCOMES
        {
            return Err(ErrorV1::InvalidOutcomeCount);
        }
        if self.coordinate_domain_min >= self.coordinate_domain_max
            || self.basis.domain_max != self.coordinate_domain_max
        {
            return Err(ErrorV1::InvalidTermsDomain);
        }
        let knot_count = usize::from(self.basis.knot_count);
        if knot_count == 0 || knot_count > self.basis.knots.len() {
            return Err(ErrorV1::InvalidBasis);
        }
        let first = self.basis.knots[0];
        let last = self.basis.knots[knot_count - 1];
        if first < self.coordinate_domain_min || last > self.coordinate_domain_max {
            return Err(ErrorV1::InvalidTermsDomain);
        }
        if self.basis.edge_policy == EdgePolicy::Refuse
            && (first != self.coordinate_domain_min || last != self.coordinate_domain_max)
        {
            return Err(ErrorV1::IncompleteRefusingDomain);
        }
        self.basis.validated().map_err(|_| ErrorV1::InvalidBasis)
    }
}

/// Exact candidate prices on the immutable payout denominator.
///
/// Active components must sum to the Basis payout denominator itself. This is
/// deliberately narrower and more direct than an arbitrary rational price
/// scale: the verifier checks `price_i * W == sum_k(weight_k * atom_k_i)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct QuantizedPayoutPriceVectorV1 {
    /// Canonical identity of these exact active prices.
    pub price_id: [u8; 32],
    /// Active native payout width.
    pub outcome_count: u8,
    /// Active exact prices followed by zero padding.
    pub prices: [u64; MAX_OUTCOMES],
}

/// Sparse positive mixture over exact integer observation coordinates.
///
/// Profile V1 uses at most `outcome_count` active coordinates. Every production
/// payout atom lies in the affine hyperplane `sum(atom_i) = D`, whose dimension
/// is at most `outcome_count - 1`; affine Caratheodory therefore gives a support
/// bound of at most `outcome_count`. Active weights are positive because a zero
/// coefficient is omitted from a sparse canonical support. They sum exactly to
/// `weight_denominator`, are primitive with it, and use strictly increasing
/// coordinates. Inactive slots are zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct QuantizedAtomMixtureCertificateV1 {
    magic: [u8; 8],
    /// Exact [`QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1`].
    pub schema_version: u8,
    /// Exact [`QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1`].
    pub semantics_version: u8,
    /// Exact [`QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1`].
    pub caratheodory_profile: u8,
    /// Exact immutable spline degree; two or three.
    pub basis_degree: u8,
    /// Active native payout width.
    pub outcome_count: u8,
    /// Active coordinate/weight prefix length.
    pub witness_count: u8,
    reserved: [u8; 2],
    /// Exact identities repeated from the bound Market/Terms/Basis/price join.
    pub bindings: QuantizedAtomMixtureBindingsV1,
    /// Exact payout denominator committed by the Basis.
    pub payout_denominator: u64,
    /// Positive common denominator of the sparse weights.
    pub weight_denominator: u64,
    /// Strictly increasing active integer observations, then zero padding.
    pub observation_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Positive active weights, then zero padding.
    pub weights: [u64; MAX_QUANTIZED_ATOMS],
}

const _: () = assert!(
    core::mem::size_of::<QuantizedAtomMixtureCertificateV1>()
        == QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1
);

impl QuantizedAtomMixtureCertificateV1 {
    /// Construct one canonical structural certificate body.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bindings: QuantizedAtomMixtureBindingsV1,
        basis_degree: u8,
        outcome_count: u8,
        payout_denominator: u64,
        weight_denominator: u64,
        witness_count: u8,
        observation_coordinates: [u128; MAX_QUANTIZED_ATOMS],
        weights: [u64; MAX_QUANTIZED_ATOMS],
    ) -> ResultV1<Self> {
        let value = Self {
            magic: QUANTIZED_ATOM_MIXTURE_MAGIC_V1,
            schema_version: QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1,
            semantics_version: QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1,
            caratheodory_profile: QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1,
            basis_degree,
            outcome_count,
            witness_count,
            reserved: [0; 2],
            bindings,
            payout_denominator,
            weight_denominator,
            observation_coordinates,
            weights,
        };
        value.validate_structure()?;
        Ok(value)
    }

    /// Encode the exact fixed-width canonical certificate body.
    pub fn encode_into(
        self,
        output: &mut [u8; QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1],
    ) -> ResultV1<()> {
        self.validate_structure()?;
        let mut cursor = 0_usize;
        put(output, &mut cursor, &self.magic)?;
        put(
            output,
            &mut cursor,
            &[
                self.schema_version,
                self.semantics_version,
                self.caratheodory_profile,
                self.basis_degree,
                self.outcome_count,
                self.witness_count,
            ],
        )?;
        put(output, &mut cursor, &self.reserved)?;
        for id in [
            self.bindings.market_id,
            self.bindings.terms_id,
            self.bindings.basis_id,
            self.bindings.price_id,
        ] {
            put(output, &mut cursor, &id)?;
        }
        put(output, &mut cursor, &self.payout_denominator.to_le_bytes())?;
        put(output, &mut cursor, &self.weight_denominator.to_le_bytes())?;
        let mut atom = 0_usize;
        while atom < MAX_QUANTIZED_ATOMS {
            put(
                output,
                &mut cursor,
                &self.observation_coordinates[atom].to_le_bytes(),
            )?;
            atom += 1;
        }
        atom = 0;
        while atom < MAX_QUANTIZED_ATOMS {
            put(output, &mut cursor, &self.weights[atom].to_le_bytes())?;
            atom += 1;
        }
        if cursor != output.len() {
            return Err(ErrorV1::ArithmeticOverflow);
        }
        Ok(())
    }

    /// Decode and structurally validate one hostile canonical certificate body.
    pub fn decode(input: &[u8]) -> ResultV1<Self> {
        if input.len() != QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1 {
            return Err(ErrorV1::InvalidEncodedLength);
        }
        let mut cursor = 0_usize;
        let magic = take::<8>(input, &mut cursor)?;
        if magic != QUANTIZED_ATOM_MIXTURE_MAGIC_V1 {
            return Err(ErrorV1::InvalidMagic);
        }
        let schema_version = take::<1>(input, &mut cursor)?[0];
        let semantics_version = take::<1>(input, &mut cursor)?[0];
        let caratheodory_profile = take::<1>(input, &mut cursor)?[0];
        let basis_degree = take::<1>(input, &mut cursor)?[0];
        let outcome_count = take::<1>(input, &mut cursor)?[0];
        let witness_count = take::<1>(input, &mut cursor)?[0];
        let reserved = take::<2>(input, &mut cursor)?;
        if reserved != [0; 2] {
            return Err(ErrorV1::NonCanonicalReserved);
        }
        let bindings = QuantizedAtomMixtureBindingsV1 {
            market_id: take::<32>(input, &mut cursor)?,
            terms_id: take::<32>(input, &mut cursor)?,
            basis_id: take::<32>(input, &mut cursor)?,
            price_id: take::<32>(input, &mut cursor)?,
        };
        let payout_denominator = u64::from_le_bytes(take::<8>(input, &mut cursor)?);
        let weight_denominator = u64::from_le_bytes(take::<8>(input, &mut cursor)?);
        let mut observation_coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
        let mut atom = 0_usize;
        while atom < MAX_QUANTIZED_ATOMS {
            observation_coordinates[atom] = u128::from_le_bytes(take::<16>(input, &mut cursor)?);
            atom += 1;
        }
        let mut weights = [0_u64; MAX_QUANTIZED_ATOMS];
        atom = 0;
        while atom < MAX_QUANTIZED_ATOMS {
            weights[atom] = u64::from_le_bytes(take::<8>(input, &mut cursor)?);
            atom += 1;
        }
        if cursor != input.len() {
            return Err(ErrorV1::InvalidEncodedLength);
        }
        let value = Self {
            magic,
            schema_version,
            semantics_version,
            caratheodory_profile,
            basis_degree,
            outcome_count,
            witness_count,
            reserved,
            bindings,
            payout_denominator,
            weight_denominator,
            observation_coordinates,
            weights,
        };
        value.validate_structure()?;
        Ok(value)
    }

    fn validate_structure(self) -> ResultV1<()> {
        if self.magic != QUANTIZED_ATOM_MIXTURE_MAGIC_V1 {
            return Err(ErrorV1::InvalidMagic);
        }
        if self.schema_version != QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1 {
            return Err(ErrorV1::UnsupportedSchemaVersion);
        }
        if self.semantics_version != QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1 {
            return Err(ErrorV1::UnsupportedSemanticsVersion);
        }
        if self.caratheodory_profile != QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1 {
            return Err(ErrorV1::UnsupportedCaratheodoryProfile);
        }
        if self.reserved != [0; 2] {
            return Err(ErrorV1::NonCanonicalReserved);
        }
        self.bindings.validate()?;
        if self.basis_degree != 2 && self.basis_degree != 3 {
            return Err(ErrorV1::InvalidDegree);
        }
        if self.outcome_count <= self.basis_degree || usize::from(self.outcome_count) > MAX_OUTCOMES
        {
            return Err(ErrorV1::InvalidOutcomeCount);
        }
        let active = usize::from(self.witness_count);
        if active == 0 || active > usize::from(self.outcome_count) || active > MAX_QUANTIZED_ATOMS {
            return Err(ErrorV1::InvalidWitnessCount);
        }
        if self.payout_denominator == 0 {
            return Err(ErrorV1::InvalidPayoutDenominator);
        }
        if self.weight_denominator == 0 {
            return Err(ErrorV1::InvalidWeightDenominator);
        }

        let mut weight_sum = 0_u128;
        let mut divisor = self.weight_denominator;
        let mut atom = 0_usize;
        while atom < MAX_QUANTIZED_ATOMS {
            let coordinate = self.observation_coordinates[atom];
            let weight = self.weights[atom];
            if atom < active {
                if atom != 0 && coordinate <= self.observation_coordinates[atom - 1] {
                    return Err(ErrorV1::NonCanonicalObservationOrder {
                        witness: u8_index(atom)?,
                    });
                }
                if weight == 0 {
                    return Err(ErrorV1::ZeroWitnessWeight {
                        witness: u8_index(atom)?,
                    });
                }
                weight_sum = weight_sum
                    .checked_add(u128::from(weight))
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                divisor = gcd(divisor, weight);
            } else if coordinate != 0 || weight != 0 {
                return Err(ErrorV1::NonCanonicalWitnessPadding {
                    witness: u8_index(atom)?,
                });
            }
            atom += 1;
        }
        if weight_sum != u128::from(self.weight_denominator) {
            return Err(ErrorV1::WitnessWeightSumMismatch);
        }
        if divisor != 1 {
            return Err(ErrorV1::NonPrimitiveWeightScale);
        }
        Ok(())
    }
}

/// Successful exact positive-certificate result.
///
/// Fields are private so this value can only be minted by the verifier. It is
/// an in-memory arithmetic fact, not account authority, settlement authority,
/// a uniqueness claim, or an optimality certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedQuantizedAtomMixtureV1 {
    bindings: QuantizedAtomMixtureBindingsV1,
    basis_degree: u8,
    outcome_count: u8,
    witness_count: u8,
    payout_denominator: u64,
    weight_denominator: u64,
}

impl VerifiedQuantizedAtomMixtureV1 {
    /// Exact checked Market/Terms/Basis/price identities.
    pub const fn bindings(self) -> QuantizedAtomMixtureBindingsV1 {
        self.bindings
    }

    /// Checked spline degree.
    pub const fn basis_degree(self) -> u8 {
        self.basis_degree
    }

    /// Checked native payout width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Checked active sparse-support width.
    pub const fn witness_count(self) -> u8 {
        self.witness_count
    }

    /// Checked exact payout denominator.
    pub const fn payout_denominator(self) -> u64 {
        self.payout_denominator
    }

    /// Checked primitive weight denominator.
    pub const fn weight_denominator(self) -> u64 {
        self.weight_denominator
    }
}

/// Identity field refused by structural or adapter-binding checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityFieldV1 {
    /// Market identity.
    Market,
    /// Complete Terms identity.
    Terms,
    /// Basis identity.
    Basis,
    /// Exact price identity.
    Price,
}

/// Total hostile-input refusal set for the V1 exact mixture checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV1 {
    /// Canonical body length was not exactly 544 bytes.
    InvalidEncodedLength,
    /// Canonical magic prefix differed from V1.
    InvalidMagic,
    /// Certificate schema version differed from V1.
    UnsupportedSchemaVersion,
    /// Quantized evaluator/reconstruction semantics differed from V1.
    UnsupportedSemanticsVersion,
    /// Affine-Caratheodory support profile differed from V1.
    UnsupportedCaratheodoryProfile,
    /// Reserved bytes were not canonical zero.
    NonCanonicalReserved,
    /// An identity was zero.
    InvalidIdentity {
        /// First invalid identity.
        field: IdentityFieldV1,
    },
    /// A certificate identity differed from adapter-derived truth.
    BindingMismatch {
        /// First mismatching identity.
        field: IdentityFieldV1,
    },
    /// Spline degree was not two or three.
    InvalidDegree,
    /// Active width was outside `degree + 1 ..= 16`.
    InvalidOutcomeCount,
    /// The complete Terms domain was malformed or disagreed with the evaluator.
    InvalidTermsDomain,
    /// A refusing evaluator did not cover the complete Terms domain.
    IncompleteRefusingDomain,
    /// The complete immutable spline projection failed validation.
    InvalidBasis,
    /// Certificate degree, width, or payout denominator differed from the Basis.
    CertificateBasisMismatch,
    /// Sparse support was empty or exceeded the affine Caratheodory bound.
    InvalidWitnessCount,
    /// Payout denominator was zero.
    InvalidPayoutDenominator,
    /// Sparse-weight denominator was zero.
    InvalidWeightDenominator,
    /// An active coordinate was not strictly above its predecessor.
    NonCanonicalObservationOrder {
        /// First noncanonical witness index.
        witness: u8,
    },
    /// An active sparse coefficient was zero instead of being omitted.
    ZeroWitnessWeight {
        /// First zero-weight witness index.
        witness: u8,
    },
    /// An inactive coordinate or weight was nonzero.
    NonCanonicalWitnessPadding {
        /// First nonzero inactive witness index.
        witness: u8,
    },
    /// Sparse weights did not sum exactly to their denominator.
    WitnessWeightSumMismatch,
    /// Weights and denominator shared a nontrivial divisor.
    NonPrimitiveWeightScale,
    /// Price identity or active width differed from the certificate.
    PriceBindingMismatch,
    /// An active price exceeded the payout denominator.
    PriceExceedsPayoutDenominator {
        /// First excessive outcome.
        outcome: u8,
    },
    /// Active prices did not sum exactly to the payout denominator.
    PriceSimplexMismatch,
    /// An inactive price component was nonzero.
    NonCanonicalPricePadding {
        /// First nonzero inactive outcome.
        outcome: u8,
    },
    /// An observation lay outside the exact Terms domain.
    ObservationOutOfDomain {
        /// First out-of-domain witness.
        witness: u8,
    },
    /// The production evaluator refused an admitted observation.
    AtomEvaluationFailed {
        /// First unevaluable witness.
        witness: u8,
    },
    /// A recomputed atom did not sum to the payout denominator.
    AtomSimplexMismatch {
        /// First malformed atom.
        witness: u8,
    },
    /// A recomputed atom had nonzero inactive padding.
    NonCanonicalAtomPadding {
        /// First malformed atom.
        witness: u8,
        /// First nonzero inactive outcome.
        outcome: u8,
    },
    /// Direct componentwise integer reconstruction failed.
    PriceReconstructionMismatch {
        /// First mismatching active outcome.
        outcome: u8,
    },
    /// The independently accumulated left or right simplex sum disagreed.
    MixtureSumMismatch,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
}

/// Result alias for exact V1 atom-mixture operations.
pub type ResultV1<T> = core::result::Result<T, ErrorV1>;

/// Verify an exact positive mixture over production-quantized spline atoms.
///
/// Every atom is recomputed through the validated degree-two/three
/// `clutch-bspline` evaluator, whose semantics include the exact knots,
/// denominator, resolved edge behavior, largest-remainder allocation, and
/// lowest-outcome-index tie break. No continuous moment cone or verifier-side
/// rounding participates.
pub fn verify_quantized_atom_mixture_v1(
    bound: &BoundQuantizedSplineV1,
    prices: &QuantizedPayoutPriceVectorV1,
    certificate: &QuantizedAtomMixtureCertificateV1,
) -> ResultV1<VerifiedQuantizedAtomMixtureV1> {
    let basis = bound.validated()?;
    certificate.validate_structure()?;
    for (actual, expected, field) in [
        (
            certificate.bindings.market_id,
            bound.bindings.market_id,
            IdentityFieldV1::Market,
        ),
        (
            certificate.bindings.terms_id,
            bound.bindings.terms_id,
            IdentityFieldV1::Terms,
        ),
        (
            certificate.bindings.basis_id,
            bound.bindings.basis_id,
            IdentityFieldV1::Basis,
        ),
        (
            certificate.bindings.price_id,
            bound.bindings.price_id,
            IdentityFieldV1::Price,
        ),
    ] {
        if actual != expected {
            return Err(ErrorV1::BindingMismatch { field });
        }
    }
    let spec = basis.spec();
    if certificate.basis_degree != spec.degree
        || certificate.outcome_count != spec.outcome_count
        || certificate.payout_denominator != spec.denominator
    {
        return Err(ErrorV1::CertificateBasisMismatch);
    }
    if prices.price_id == [0; 32]
        || prices.price_id != bound.bindings.price_id
        || prices.outcome_count != spec.outcome_count
    {
        return Err(ErrorV1::PriceBindingMismatch);
    }

    let outcomes = usize::from(spec.outcome_count);
    let mut price_sum = 0_u128;
    let mut outcome = 0_usize;
    while outcome < MAX_OUTCOMES {
        let price = prices.prices[outcome];
        if outcome < outcomes {
            if price > spec.denominator {
                return Err(ErrorV1::PriceExceedsPayoutDenominator {
                    outcome: u8_index(outcome)?,
                });
            }
            price_sum = price_sum
                .checked_add(u128::from(price))
                .ok_or(ErrorV1::ArithmeticOverflow)?;
        } else if price != 0 {
            return Err(ErrorV1::NonCanonicalPricePadding {
                outcome: u8_index(outcome)?,
            });
        }
        outcome += 1;
    }
    if price_sum != u128::from(spec.denominator) {
        return Err(ErrorV1::PriceSimplexMismatch);
    }

    let mut mixture = [0_u128; MAX_OUTCOMES];
    let mut witness = 0_usize;
    while witness < usize::from(certificate.witness_count) {
        let witness_index = u8_index(witness)?;
        let coordinate = certificate.observation_coordinates[witness];
        if coordinate < bound.coordinate_domain_min || coordinate > bound.coordinate_domain_max {
            return Err(ErrorV1::ObservationOutOfDomain {
                witness: witness_index,
            });
        }
        let atom = basis
            .evaluate_point(coordinate)
            .map_err(|_| ErrorV1::AtomEvaluationFailed {
                witness: witness_index,
            })?;
        let mut atom_sum = 0_u128;
        outcome = 0;
        while outcome < MAX_OUTCOMES {
            let component = atom.weights[outcome];
            if outcome < outcomes {
                atom_sum = atom_sum
                    .checked_add(u128::from(component))
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                let weighted = u128::from(certificate.weights[witness])
                    .checked_mul(u128::from(component))
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                mixture[outcome] = mixture[outcome]
                    .checked_add(weighted)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
            } else if component != 0 {
                return Err(ErrorV1::NonCanonicalAtomPadding {
                    witness: witness_index,
                    outcome: u8_index(outcome)?,
                });
            }
            outcome += 1;
        }
        if atom_sum != u128::from(spec.denominator) {
            return Err(ErrorV1::AtomSimplexMismatch {
                witness: witness_index,
            });
        }
        witness += 1;
    }

    let expected_sum = u128::from(spec.denominator)
        .checked_mul(u128::from(certificate.weight_denominator))
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let mut left_sum = 0_u128;
    let mut right_sum = 0_u128;
    outcome = 0;
    while outcome < outcomes {
        let left = u128::from(prices.prices[outcome])
            .checked_mul(u128::from(certificate.weight_denominator))
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        if left != mixture[outcome] {
            return Err(ErrorV1::PriceReconstructionMismatch {
                outcome: u8_index(outcome)?,
            });
        }
        left_sum = left_sum
            .checked_add(left)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        right_sum = right_sum
            .checked_add(mixture[outcome])
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    if left_sum != expected_sum || right_sum != expected_sum {
        return Err(ErrorV1::MixtureSumMismatch);
    }

    Ok(VerifiedQuantizedAtomMixtureV1 {
        bindings: bound.bindings,
        basis_degree: spec.degree,
        outcome_count: spec.outcome_count,
        witness_count: certificate.witness_count,
        payout_denominator: spec.denominator,
        weight_denominator: certificate.weight_denominator,
    })
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn u8_index(index: usize) -> ResultV1<u8> {
    u8::try_from(index).map_err(|_| ErrorV1::ArithmeticOverflow)
}

fn put(output: &mut [u8], cursor: &mut usize, bytes: &[u8]) -> ResultV1<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let destination = output
        .get_mut(*cursor..end)
        .ok_or(ErrorV1::InvalidEncodedLength)?;
    destination.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<const N: usize>(input: &[u8], cursor: &mut usize) -> ResultV1<[u8; N]> {
    let end = cursor.checked_add(N).ok_or(ErrorV1::ArithmeticOverflow)?;
    let source = input
        .get(*cursor..end)
        .ok_or(ErrorV1::InvalidEncodedLength)?;
    let mut output = [0_u8; N];
    output.copy_from_slice(source);
    *cursor = end;
    Ok(output)
}
