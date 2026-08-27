//! Canonical untrusted compiler target for Product-native payoff artifacts.
//!
//! This module allocates because it is an offline compiler. It emits exact
//! bytes and typed IDs owned by `clutch-product-series`, but never authenticates
//! a registry, Source release, account, signer, or deployment.

use std::vec::Vec;

use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_product_series::{
    ContentId, Error as ProductError, FixedCodec, NativeClaimBasisId, NativeClaimBasisV1,
    BASIS_BYTES, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::artifact::{
    decode_rational_v1, domain_digest, encode_rational_v1, ArtifactError, NativeShapeCertificateV1,
    Reader, BASIS_EVALUATOR_VERSION_V1, MAX_CERTIFICATE_BYTES_V1, SHAPE_COMPILER_VERSION_V1,
    WEIGHT_ROUND_VERSION_V1,
};
use crate::{Shape, SpanStatus};

const EXACT_SMOOTH_PAYOFF_MAGIC_V1: [u8; 8] = *b"DCPAYV1\0";
const EXACT_SMOOTH_PAYOFF_SCHEMA_V1: u16 = 1;
const EXACT_SMOOTH_PAYOFF_CONSTRUCTION_V1: u8 = 1;
const EXACT_IN_SPAN_TAG_V1: u8 = 1;
const EXACT_SMOOTH_PAYOFF_FIXED_BYTES_V1: usize = 128;
const EXACT_SMOOTH_PAYOFF_DOMAIN_V1: &[u8] = b"dragons-clutch/exact-smooth-payoff/v1";

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_PAYOUTS == MAX_OUTCOMES);
const _: () = assert!(MAX_KNOTS == MAX_OUTCOMES);

/// Exact rational categorical payoff table over ordered coordinate cells.
///
/// `cell_payouts` has one row per native outcome/cell and one component per
/// native claim. Every row must be a nonnegative rational simplex. The
/// compiler derives the least common integer denominator, deduplicates equal
/// rows, and emits Product's canonical first-use payout map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCategoricalPayoffDefinitionV1 {
    /// Inclusive complete Terms-domain minimum.
    pub coordinate_domain_min: u128,
    /// Inclusive complete Terms-domain maximum.
    pub coordinate_domain_max: u128,
    /// Exactly `cell_payouts.len() - 1` increasing interior boundaries.
    pub knots: Vec<u128>,
    /// Exact rational native payout vectors in coordinate-cell order.
    pub cell_payouts: Vec<Vec<BigRational>>,
    /// Nonzero central-registry ambiguity selector.
    pub ambiguity_policy_registry_value: u8,
    /// Nonzero central-registry edge selector; categorical evaluation is clamp.
    pub edge_policy_registry_value: u8,
}

/// Product-native smooth basis parameters shared by exact and analytic payoffs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmoothNativeBasisDefinitionV1 {
    /// B-spline degree in `1..=3`.
    pub degree: u8,
    /// Inclusive complete Terms-domain minimum.
    pub coordinate_domain_min: u128,
    /// Inclusive complete Terms-domain maximum.
    pub coordinate_domain_max: u128,
    /// Positive production payout denominator.
    pub payout_denominator: u64,
    /// Distinct active knots with no padding.
    pub knots: Vec<u128>,
    /// Registry-resolved edge behavior used by the host evaluator.
    pub resolved_edge_policy: EdgePolicy,
    /// Nonzero central-registry ambiguity selector.
    pub ambiguity_policy_registry_value: u8,
    /// Nonzero central-registry edge selector naming the resolved behavior.
    pub edge_policy_registry_value: u8,
}

/// Exact rational spline payoff represented by canonical control values.
///
/// The control vector itself defines the requested payoff, so this case is
/// exact in the selected spline span. `maximum_liability` must be the exact
/// maximum active control value, which removes a nonsemantic free cap.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSmoothPayoffDefinitionV1 {
    /// Product-native smooth basis.
    pub basis: SmoothNativeBasisDefinitionV1,
    /// Exact rational control values in native basis order.
    pub control_values: Vec<BigRational>,
    /// Exact maximum active control value and payoff liability bound.
    pub maximum_liability: BigRational,
}

/// Named analytic target compiled against one Product-native smooth basis.
///
/// The existing exact-rational research compiler decides whether the target is
/// algebraically exact in the span or emits its explicit approximation bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyticSmoothPayoffDefinitionV1 {
    /// Product-native smooth basis.
    pub basis: SmoothNativeBasisDefinitionV1,
    /// Bounded exact-integer analytic target interpreted rationally.
    pub shape: Shape,
}

/// Exact or explicitly approximate Product-facing payoff request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionPayoffDefinitionV1 {
    /// Exact finite categorical payout table; Product Basis owns all payoff rows.
    ExactCategorical(ExactCategoricalPayoffDefinitionV1),
    /// Exact rational control payoff in one smooth native span.
    ExactSmooth(ExactSmoothPayoffDefinitionV1),
    /// Named analytic target that may require a certified approximation.
    AnalyticSmooth(AnalyticSmoothPayoffDefinitionV1),
}

/// Canonical variable-length exact rational smooth-payoff certificate.
///
/// It is compiler provenance and a deterministic portfolio recipe, not a
/// persisted Product semantic owner or an authorization to create claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSmoothPayoffCertificateV1 {
    /// Complete Product terms/Genesis identity owning the coordinate domain.
    pub product_terms_id: ContentId,
    /// Canonical Product native basis identity.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Inclusive lower coordinate bound.
    pub coordinate_domain_min: u128,
    /// Inclusive upper coordinate bound.
    pub coordinate_domain_max: u128,
    /// Exact smooth degree.
    pub basis_degree: u8,
    /// Active native outcome/control width.
    pub outcome_count: u8,
    /// Exact minimal maximum of active controls.
    pub maximum_liability: BigRational,
    /// Exact active controls with no padding.
    pub control_values: Vec<BigRational>,
}

impl ExactSmoothPayoffCertificateV1 {
    /// Verify this exact recipe against the canonical Product basis and Terms ID.
    pub fn verify(
        &self,
        expected_terms_id: ContentId,
        basis: &NativeClaimBasisV1,
        coordinate_domain_min: u128,
        coordinate_domain_max: u128,
    ) -> Result<(), ProductionCompilerError> {
        if self.product_terms_id != expected_terms_id
            || self.native_claim_basis_id != basis.id()?
            || self.coordinate_domain_min != coordinate_domain_min
            || self.coordinate_domain_max != coordinate_domain_max
            || self.basis_degree != basis.basis_degree
            || self.outcome_count != basis.outcome_count
            || self.control_values.len() != usize::from(basis.outcome_count)
            || basis.basis_degree == 0
        {
            return Err(ProductionCompilerError::CertificateMismatch);
        }
        validate_exact_controls(&self.control_values, &self.maximum_liability)
    }

    /// Encode the unique exact rational certificate body.
    pub fn encode(&self) -> Result<Vec<u8>, ProductionCompilerError> {
        if self.control_values.len() != usize::from(self.outcome_count) {
            return Err(ProductionCompilerError::CertificateMismatch);
        }
        validate_exact_controls(&self.control_values, &self.maximum_liability)?;
        if self.product_terms_id.is_zero()
            || self.native_claim_basis_id.content_id().is_zero()
            || self.basis_degree == 0
            || self.basis_degree > 3
            || self.outcome_count <= self.basis_degree
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.coordinate_domain_min >= self.coordinate_domain_max
        {
            return Err(ProductionCompilerError::InvalidDefinition);
        }
        let mut output = Vec::with_capacity(EXACT_SMOOTH_PAYOFF_FIXED_BYTES_V1 + 64);
        output.extend_from_slice(&EXACT_SMOOTH_PAYOFF_MAGIC_V1);
        output.extend_from_slice(&EXACT_SMOOTH_PAYOFF_SCHEMA_V1.to_le_bytes());
        output.extend_from_slice(&SHAPE_COMPILER_VERSION_V1.to_le_bytes());
        output.extend_from_slice(&BASIS_EVALUATOR_VERSION_V1.to_le_bytes());
        output.extend_from_slice(&WEIGHT_ROUND_VERSION_V1.to_le_bytes());
        output.push(EXACT_IN_SPAN_TAG_V1);
        output.push(EXACT_SMOOTH_PAYOFF_CONSTRUCTION_V1);
        output.push(self.basis_degree);
        output.push(self.outcome_count);
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&self.product_terms_id.bytes());
        output.extend_from_slice(&self.native_claim_basis_id.bytes());
        output.extend_from_slice(&self.coordinate_domain_min.to_le_bytes());
        output.extend_from_slice(&self.coordinate_domain_max.to_le_bytes());
        output.extend_from_slice(
            &u16::try_from(self.control_values.len())
                .map_err(|_| ProductionCompilerError::InvalidLength)?
                .to_le_bytes(),
        );
        output.extend_from_slice(&[0; 6]);
        if output.len() != EXACT_SMOOTH_PAYOFF_FIXED_BYTES_V1 {
            return Err(ProductionCompilerError::InternalInvariant);
        }
        encode_rational_v1(&self.maximum_liability, &mut output)?;
        for value in &self.control_values {
            encode_rational_v1(value, &mut output)?;
        }
        if output.len() > MAX_CERTIFICATE_BYTES_V1 {
            return Err(ProductionCompilerError::InvalidLength);
        }
        Ok(output)
    }

    /// Decode one hostile body, re-encode it, and reject noncanonical forms.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionCompilerError> {
        if bytes.len() > MAX_CERTIFICATE_BYTES_V1 {
            return Err(ProductionCompilerError::InvalidLength);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != EXACT_SMOOTH_PAYOFF_MAGIC_V1
            || reader.u16()? != EXACT_SMOOTH_PAYOFF_SCHEMA_V1
            || reader.u16()? != SHAPE_COMPILER_VERSION_V1
            || reader.u16()? != BASIS_EVALUATOR_VERSION_V1
            || reader.u16()? != WEIGHT_ROUND_VERSION_V1
            || reader.u8()? != EXACT_IN_SPAN_TAG_V1
            || reader.u8()? != EXACT_SMOOTH_PAYOFF_CONSTRUCTION_V1
        {
            return Err(ProductionCompilerError::InvalidDiscriminant);
        }
        let basis_degree = reader.u8()?;
        let outcome_count = reader.u8()?;
        if reader.take(4)?.iter().any(|value| *value != 0) {
            return Err(ProductionCompilerError::NonCanonicalPadding);
        }
        let product_terms_id = ContentId::from_bytes(reader.array32()?);
        let native_claim_basis_id = NativeClaimBasisId::from_bytes(reader.array32()?);
        let coordinate_domain_min = reader.u128()?;
        let coordinate_domain_max = reader.u128()?;
        let control_count = usize::from(reader.u16()?);
        if reader.take(6)?.iter().any(|value| *value != 0)
            || control_count != usize::from(outcome_count)
        {
            return Err(ProductionCompilerError::NonCanonicalPadding);
        }
        let maximum_liability = decode_rational_v1(&mut reader)?;
        let mut control_values = Vec::with_capacity(control_count);
        let mut index = 0_usize;
        while index < control_count {
            control_values.push(decode_rational_v1(&mut reader)?);
            index += 1;
        }
        if !reader.done() {
            return Err(ProductionCompilerError::TrailingBytes);
        }
        let value = Self {
            product_terms_id,
            native_claim_basis_id,
            coordinate_domain_min,
            coordinate_domain_max,
            basis_degree,
            outcome_count,
            maximum_liability,
            control_values,
        };
        if value.encode()?.as_slice() != bytes {
            return Err(ProductionCompilerError::CertificateMismatch);
        }
        Ok(value)
    }

    /// Domain-separated content ID of the unique certificate bytes.
    pub fn content_id(&self) -> Result<ContentId, ProductionCompilerError> {
        Ok(ContentId::from_bytes(domain_digest(
            EXACT_SMOOTH_PAYOFF_DOMAIN_V1,
            &self.encode()?,
        )))
    }
}

/// Exact compiler evidence emitted alongside the Product-native basis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionPayoffEvidenceV1 {
    /// The canonical degree-zero `NativeClaimBasisV1` itself owns every row.
    ExactCategoricalBasis,
    /// Exact rational smooth control recipe in the selected native span.
    ExactSmooth {
        /// Canonical recompilable certificate value.
        certificate: ExactSmoothPayoffCertificateV1,
        /// Canonical certificate bytes.
        certificate_bytes: Vec<u8>,
        /// Domain-separated certificate content ID.
        certificate_id: ContentId,
    },
    /// Existing analytic compiler result, explicitly exact or approximate.
    AnalyticSmooth {
        /// Exact-in-span versus certified-approximation classification.
        status: SpanStatus,
        /// Recompile-verifiable certificate bytes.
        certificate_bytes: Vec<u8>,
        /// Domain-separated certificate content ID.
        certificate_id: ContentId,
    },
}

/// Canonical Product-native compiler output without any authority claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledProductionPayoffV1 {
    /// Sole Product owner of native partition/basis/payout bytes.
    pub native_claim_basis: NativeClaimBasisV1,
    /// Exact Product codec bytes for `native_claim_basis`.
    pub native_claim_basis_bytes: [u8; BASIS_BYTES],
    /// Typed Product content ID of the exact basis bytes.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Inclusive complete Product terms-domain minimum.
    pub coordinate_domain_min: u128,
    /// Inclusive complete Product terms-domain maximum.
    pub coordinate_domain_max: u128,
    /// Smooth evaluator projection, absent for Product-owned categorical rows.
    pub smooth_basis: Option<BasisSpec>,
    /// Exact or explicitly approximate payoff evidence.
    pub evidence: ProductionPayoffEvidenceV1,
}

impl CompiledProductionPayoffV1 {
    /// Recompute Product bytes/ID and all certificate bindings.
    pub fn verify(&self, product_terms_id: ContentId) -> Result<(), ProductionCompilerError> {
        if product_terms_id.is_zero() {
            return Err(ProductionCompilerError::OutputMismatch);
        }
        self.native_claim_basis.validate()?;
        let mut expected_bytes = [0_u8; BASIS_BYTES];
        self.native_claim_basis.encode_into(&mut expected_bytes)?;
        if self.native_claim_basis_bytes != expected_bytes
            || self.native_claim_basis_id != self.native_claim_basis.id()?
            || self.coordinate_domain_min >= self.coordinate_domain_max
        {
            return Err(ProductionCompilerError::OutputMismatch);
        }
        match (&self.smooth_basis, &self.evidence) {
            (None, ProductionPayoffEvidenceV1::ExactCategoricalBasis)
                if self.native_claim_basis.basis_degree == 0 =>
            {
                validate_domain(
                    self.coordinate_domain_min,
                    self.coordinate_domain_max,
                    &self.native_claim_basis.knots
                        [..usize::from(self.native_claim_basis.knot_count)],
                    DomainPolicyV1::CategoricalInterior,
                )?;
            }
            (
                Some(spec),
                ProductionPayoffEvidenceV1::ExactSmooth {
                    certificate,
                    certificate_bytes,
                    certificate_id,
                },
            ) => {
                verify_smooth_projection(self, spec)?;
                certificate.verify(
                    product_terms_id,
                    &self.native_claim_basis,
                    self.coordinate_domain_min,
                    self.coordinate_domain_max,
                )?;
                if certificate.encode()? != *certificate_bytes
                    || certificate.content_id()? != *certificate_id
                {
                    return Err(ProductionCompilerError::OutputMismatch);
                }
            }
            (
                Some(spec),
                ProductionPayoffEvidenceV1::AnalyticSmooth {
                    status,
                    certificate_bytes,
                    certificate_id,
                },
            ) => {
                verify_smooth_projection(self, spec)?;
                let certificate = NativeShapeCertificateV1::decode(certificate_bytes)?;
                if certificate.terms_digest != product_terms_id.bytes()
                    || certificate.basis != *spec
                    || certificate.compilation.status != *status
                    || ContentId::from_bytes(certificate.digest()?) != *certificate_id
                {
                    return Err(ProductionCompilerError::OutputMismatch);
                }
            }
            _ => return Err(ProductionCompilerError::OutputMismatch),
        }
        Ok(())
    }
}

/// Deterministic refusal from the untrusted production compiler target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionCompilerError {
    /// Definition counts, selectors, or rational bounds were malformed.
    InvalidDefinition,
    /// Coordinate domain or active knots were inconsistent.
    InvalidDomain,
    /// A categorical row was negative or did not sum exactly to one.
    InvalidCategoricalSimplex,
    /// Exact rational integerization exceeded the Product `u64` envelope.
    RationalIntegerizationOverflow,
    /// The requested basis/payoff is not representable by the canonical owner.
    UnrepresentableShape,
    /// A certificate magic, version, status, construction, or rounding tag differed.
    InvalidDiscriminant,
    /// A variable body exceeded the frozen host bound.
    InvalidLength,
    /// Reserved bytes or inactive fields were noncanonical.
    NonCanonicalPadding,
    /// Hostile bytes ended before one complete value.
    Truncated,
    /// Hostile bytes contained a trailing suffix.
    TrailingBytes,
    /// Decoded certificate did not match the named exact basis/payoff.
    CertificateMismatch,
    /// Stored output bytes or IDs did not recompute.
    OutputMismatch,
    /// A supposedly unreachable compiler condition occurred.
    InternalInvariant,
    /// Canonical Product artifact logic refused the proposed output.
    Product(ProductError),
    /// Existing exact-rational compiler/certificate logic refused the request.
    Artifact(ArtifactError),
}

impl From<ProductError> for ProductionCompilerError {
    fn from(value: ProductError) -> Self {
        Self::Product(value)
    }
}

impl From<ArtifactError> for ProductionCompilerError {
    fn from(value: ArtifactError) -> Self {
        match value {
            ArtifactError::Truncated => Self::Truncated,
            ArtifactError::TrailingBytes => Self::TrailingBytes,
            other => Self::Artifact(other),
        }
    }
}

/// Compile exact rational payoff semantics into canonical Product artifacts.
///
/// `product_terms_id` is the complete canonical Genesis/Terms identity that
/// owns the coordinate domain. It is copied into certificates but is not
/// authenticated by this host compiler.
pub fn compile_production_payoff_v1(
    product_terms_id: ContentId,
    definition: ProductionPayoffDefinitionV1,
) -> Result<CompiledProductionPayoffV1, ProductionCompilerError> {
    if product_terms_id.is_zero() {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let value = match definition {
        ProductionPayoffDefinitionV1::ExactCategorical(definition) => {
            compile_exact_categorical(definition)?
        }
        ProductionPayoffDefinitionV1::ExactSmooth(definition) => {
            compile_exact_smooth(product_terms_id, definition)?
        }
        ProductionPayoffDefinitionV1::AnalyticSmooth(definition) => {
            compile_analytic_smooth(product_terms_id, definition)?
        }
    };
    value.verify(product_terms_id)?;
    Ok(value)
}

fn compile_exact_categorical(
    definition: ExactCategoricalPayoffDefinitionV1,
) -> Result<CompiledProductionPayoffV1, ProductionCompilerError> {
    let outcomes = definition.cell_payouts.len();
    if !(2..=MAX_OUTCOMES).contains(&outcomes)
        || definition.knots.len() != outcomes - 1
        || definition.ambiguity_policy_registry_value == 0
        || definition.edge_policy_registry_value == 0
    {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    validate_domain(
        definition.coordinate_domain_min,
        definition.coordinate_domain_max,
        &definition.knots,
        DomainPolicyV1::CategoricalInterior,
    )?;

    let mut denominator = BigInt::one();
    for row in &definition.cell_payouts {
        if row.len() != outcomes {
            return Err(ProductionCompilerError::InvalidDefinition);
        }
        let mut sum = BigRational::zero();
        for value in row {
            if value.is_negative() || value > &BigRational::one() {
                return Err(ProductionCompilerError::InvalidCategoricalSimplex);
            }
            denominator = checked_lcm(denominator, value.denom().clone())?;
            sum += value;
        }
        if sum != BigRational::one() {
            return Err(ProductionCompilerError::InvalidCategoricalSimplex);
        }
    }
    let denominator_u64 = denominator
        .to_u64()
        .ok_or(ProductionCompilerError::RationalIntegerizationOverflow)?;
    if denominator_u64 == 0 {
        return Err(ProductionCompilerError::RationalIntegerizationOverflow);
    }

    let mut distinct_rows = [[0_u64; MAX_OUTCOMES]; MAX_PAYOUTS];
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    let mut payout_count = 0_usize;
    for (cell, rational_row) in definition.cell_payouts.iter().enumerate() {
        let mut row = [0_u64; MAX_OUTCOMES];
        let mut outcome = 0_usize;
        while outcome < outcomes {
            let value = &rational_row[outcome];
            let multiplier = &denominator / value.denom();
            let scaled = value.numer() * multiplier;
            row[outcome] = scaled
                .to_u64()
                .ok_or(ProductionCompilerError::RationalIntegerizationOverflow)?;
            outcome += 1;
        }
        let existing = distinct_rows[..payout_count]
            .iter()
            .position(|candidate| *candidate == row);
        let row_index = match existing {
            Some(index) => index,
            None => {
                if payout_count >= MAX_PAYOUTS {
                    return Err(ProductionCompilerError::UnrepresentableShape);
                }
                distinct_rows[payout_count] = row;
                let index = payout_count;
                payout_count += 1;
                index
            }
        };
        payout_map[cell] =
            u8::try_from(row_index).map_err(|_| ProductionCompilerError::InternalInvariant)?;
    }

    let mut knots = [0_u128; MAX_OUTCOMES];
    knots[..definition.knots.len()].copy_from_slice(&definition.knots);
    let basis = NativeClaimBasisV1 {
        basis_degree: 0,
        outcome_count: u8::try_from(outcomes)
            .map_err(|_| ProductionCompilerError::InvalidDefinition)?,
        payout_count: u8::try_from(payout_count)
            .map_err(|_| ProductionCompilerError::InternalInvariant)?,
        knot_count: u8::try_from(definition.knots.len())
            .map_err(|_| ProductionCompilerError::InvalidDefinition)?,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        ambiguity_policy_registry_value: definition.ambiguity_policy_registry_value,
        edge_policy_registry_value: definition.edge_policy_registry_value,
        denominator: denominator_u64,
        payout_weights: distinct_rows,
        payout_map,
        knots,
    };
    finish_output(
        basis,
        definition.coordinate_domain_min,
        definition.coordinate_domain_max,
        None,
        ProductionPayoffEvidenceV1::ExactCategoricalBasis,
    )
}

fn compile_exact_smooth(
    product_terms_id: ContentId,
    definition: ExactSmoothPayoffDefinitionV1,
) -> Result<CompiledProductionPayoffV1, ProductionCompilerError> {
    let (basis, spec) = compile_smooth_basis(&definition.basis)?;
    validate_exact_controls(&definition.control_values, &definition.maximum_liability)?;
    if definition.control_values.len() != usize::from(basis.outcome_count) {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let certificate = ExactSmoothPayoffCertificateV1 {
        product_terms_id,
        native_claim_basis_id: basis.id()?,
        coordinate_domain_min: definition.basis.coordinate_domain_min,
        coordinate_domain_max: definition.basis.coordinate_domain_max,
        basis_degree: basis.basis_degree,
        outcome_count: basis.outcome_count,
        maximum_liability: definition.maximum_liability,
        control_values: definition.control_values,
    };
    let certificate_bytes = certificate.encode()?;
    let certificate_id = certificate.content_id()?;
    finish_output(
        basis,
        definition.basis.coordinate_domain_min,
        definition.basis.coordinate_domain_max,
        Some(spec),
        ProductionPayoffEvidenceV1::ExactSmooth {
            certificate,
            certificate_bytes,
            certificate_id,
        },
    )
}

fn compile_analytic_smooth(
    product_terms_id: ContentId,
    definition: AnalyticSmoothPayoffDefinitionV1,
) -> Result<CompiledProductionPayoffV1, ProductionCompilerError> {
    let (basis, spec) = compile_smooth_basis(&definition.basis)?;
    let certificate =
        NativeShapeCertificateV1::compile(product_terms_id.bytes(), spec, definition.shape)?;
    let status = certificate.compilation.status;
    let certificate_bytes = certificate.encode()?;
    let certificate_id = ContentId::from_bytes(certificate.digest()?);
    finish_output(
        basis,
        definition.basis.coordinate_domain_min,
        definition.basis.coordinate_domain_max,
        Some(spec),
        ProductionPayoffEvidenceV1::AnalyticSmooth {
            status,
            certificate_bytes,
            certificate_id,
        },
    )
}

fn compile_smooth_basis(
    definition: &SmoothNativeBasisDefinitionV1,
) -> Result<(NativeClaimBasisV1, BasisSpec), ProductionCompilerError> {
    if !(1..=3).contains(&definition.degree)
        || definition.payout_denominator == 0
        || definition.ambiguity_policy_registry_value == 0
        || definition.edge_policy_registry_value == 0
        || definition.knots.len() < 2
        || definition.knots.len() > MAX_OUTCOMES
    {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    validate_domain(
        definition.coordinate_domain_min,
        definition.coordinate_domain_max,
        &definition.knots,
        if definition.resolved_edge_policy == EdgePolicy::Refuse {
            DomainPolicyV1::SmoothRefuse
        } else {
            DomainPolicyV1::SmoothClamp
        },
    )?;
    let outcomes = definition
        .knots
        .len()
        .checked_sub(1)
        .and_then(|value| value.checked_add(usize::from(definition.degree)))
        .ok_or(ProductionCompilerError::InvalidDefinition)?;
    if !(usize::from(definition.degree) + 1..=MAX_OUTCOMES).contains(&outcomes) {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let uniform_log2_spacing = canonical_uniform_spacing(&definition.knots)?;
    if definition.degree >= 2 && uniform_log2_spacing == UNIFORM_SPACING_NONE {
        return Err(ProductionCompilerError::UnrepresentableShape);
    }
    let mut knots = [0_u128; MAX_OUTCOMES];
    knots[..definition.knots.len()].copy_from_slice(&definition.knots);
    let basis = NativeClaimBasisV1 {
        basis_degree: definition.degree,
        outcome_count: u8::try_from(outcomes)
            .map_err(|_| ProductionCompilerError::InvalidDefinition)?,
        payout_count: 0,
        knot_count: u8::try_from(definition.knots.len())
            .map_err(|_| ProductionCompilerError::InvalidDefinition)?,
        uniform_log2_spacing,
        ambiguity_policy_registry_value: definition.ambiguity_policy_registry_value,
        edge_policy_registry_value: definition.edge_policy_registry_value,
        denominator: definition.payout_denominator,
        payout_weights: [[0; MAX_OUTCOMES]; MAX_PAYOUTS],
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
    };
    basis.validate()?;
    let spec = BasisSpec {
        outcome_count: basis.outcome_count,
        degree: basis.basis_degree,
        knot_count: basis.knot_count,
        uniform_log2_spacing: basis.uniform_log2_spacing,
        denominator: basis.denominator,
        domain_max: definition.coordinate_domain_max,
        edge_policy: definition.resolved_edge_policy,
        knots: basis.knots,
    };
    spec.validate()
        .map_err(|_| ProductionCompilerError::UnrepresentableShape)?;
    Ok((basis, spec))
}

fn finish_output(
    basis: NativeClaimBasisV1,
    coordinate_domain_min: u128,
    coordinate_domain_max: u128,
    smooth_basis: Option<BasisSpec>,
    evidence: ProductionPayoffEvidenceV1,
) -> Result<CompiledProductionPayoffV1, ProductionCompilerError> {
    basis.validate()?;
    let mut native_claim_basis_bytes = [0_u8; BASIS_BYTES];
    basis.encode_into(&mut native_claim_basis_bytes)?;
    Ok(CompiledProductionPayoffV1 {
        native_claim_basis_id: basis.id()?,
        native_claim_basis: basis,
        native_claim_basis_bytes,
        coordinate_domain_min,
        coordinate_domain_max,
        smooth_basis,
        evidence,
    })
}

fn verify_smooth_projection(
    output: &CompiledProductionPayoffV1,
    spec: &BasisSpec,
) -> Result<(), ProductionCompilerError> {
    spec.validate()
        .map_err(|_| ProductionCompilerError::OutputMismatch)?;
    let basis = &output.native_claim_basis;
    if basis.basis_degree == 0
        || spec.degree != basis.basis_degree
        || spec.outcome_count != basis.outcome_count
        || spec.knot_count != basis.knot_count
        || spec.uniform_log2_spacing != basis.uniform_log2_spacing
        || spec.denominator != basis.denominator
        || spec.domain_max != output.coordinate_domain_max
        || spec.knots != basis.knots
    {
        return Err(ProductionCompilerError::OutputMismatch);
    }
    validate_domain(
        output.coordinate_domain_min,
        output.coordinate_domain_max,
        &basis.knots[..usize::from(basis.knot_count)],
        if spec.edge_policy == EdgePolicy::Refuse {
            DomainPolicyV1::SmoothRefuse
        } else {
            DomainPolicyV1::SmoothClamp
        },
    )
}

fn validate_exact_controls(
    controls: &[BigRational],
    maximum_liability: &BigRational,
) -> Result<(), ProductionCompilerError> {
    if controls.len() < 2
        || controls.len() > MAX_OUTCOMES
        || maximum_liability <= &BigRational::zero()
        || controls.iter().any(|value| value.is_negative())
    {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let maximum = controls
        .iter()
        .max()
        .ok_or(ProductionCompilerError::InvalidDefinition)?;
    if maximum != maximum_liability {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DomainPolicyV1 {
    CategoricalInterior,
    SmoothClamp,
    SmoothRefuse,
}

fn validate_domain(
    domain_min: u128,
    domain_max: u128,
    knots: &[u128],
    policy: DomainPolicyV1,
) -> Result<(), ProductionCompilerError> {
    if domain_min >= domain_max || knots.is_empty() {
        return Err(ProductionCompilerError::InvalidDomain);
    }
    let mut index = 0_usize;
    while index < knots.len() {
        let knot = knots[index];
        if knot < domain_min || knot > domain_max || (index != 0 && knot <= knots[index - 1]) {
            return Err(ProductionCompilerError::InvalidDomain);
        }
        index += 1;
    }
    match policy {
        DomainPolicyV1::CategoricalInterior if knots[0] <= domain_min => {
            return Err(ProductionCompilerError::InvalidDomain);
        }
        DomainPolicyV1::SmoothRefuse
            if knots[0] != domain_min || knots[knots.len() - 1] != domain_max =>
        {
            return Err(ProductionCompilerError::InvalidDomain);
        }
        DomainPolicyV1::CategoricalInterior
        | DomainPolicyV1::SmoothClamp
        | DomainPolicyV1::SmoothRefuse => {}
    }
    Ok(())
}

fn canonical_uniform_spacing(knots: &[u128]) -> Result<u8, ProductionCompilerError> {
    if knots.len() < 2 {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let gap = knots[1]
        .checked_sub(knots[0])
        .ok_or(ProductionCompilerError::InvalidDomain)?;
    let uniform = knots
        .windows(2)
        .all(|pair| pair[1].checked_sub(pair[0]) == Some(gap));
    if !uniform || !gap.is_power_of_two() {
        return Ok(UNIFORM_SPACING_NONE);
    }
    u8::try_from(gap.trailing_zeros()).map_err(|_| ProductionCompilerError::UnrepresentableShape)
}

fn checked_lcm(left: BigInt, right: BigInt) -> Result<BigInt, ProductionCompilerError> {
    if left <= BigInt::zero() || right <= BigInt::zero() {
        return Err(ProductionCompilerError::InvalidDefinition);
    }
    let divisor = gcd_bigint(left.clone(), right.clone());
    let quotient = left / divisor;
    Ok(quotient * right)
}

fn gcd_bigint(mut left: BigInt, mut right: BigInt) -> BigInt {
    while !right.is_zero() {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left
}
