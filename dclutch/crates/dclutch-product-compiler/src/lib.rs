#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Deterministic construction of categorical dClutch Product recipes.
//!
//! This host compiler emits an exhaustive ordered partition, the elementary
//! [`CategoricalUnitV1`] liability basis over its cells, and one exact-width
//! [`PortfolioTemplateV1`] user recipe. The portfolio is not another native
//! liability basis: its `N` rational coefficients are quantities of the `N`
//! categorical claims in canonical partition order.
//!
//! V1 native liabilities remain categorical and therefore constant within
//! every partition cell. Direct graded ramps and tents are refused explicitly.
//! [`graded`] exposes a separately named midpoint-projection boundary that can
//! compile those shapes into honest categorical portfolios without changing
//! the native basis or Market accounting.

use core::fmt;

use dclutch_product_contract::capacity::{CapacityProfileId, CapacityProfileV1};
use dclutch_product_contract::claim::{CategoricalUnitV1, CategoricalUnitV1Input};
use dclutch_product_contract::portfolio::{
    PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, PortfolioTemplateV1,
};
use dclutch_product_contract::product::{
    InstanceV1, InstanceV1Input, OccurrenceV1, OccurrenceV1Input, TermsV1, TermsV1Input,
};
use dclutch_product_contract::result_domain::{
    FINITE_RESULT_DOMAIN_BYTES, FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1,
};
use dclutch_product_contract::{ContentId, Error as ContractError};
use sha2::{Digest, Sha256};

/// Explicit categorical projection of graded user payoffs.
pub mod graded;

const CERTIFICATE_VERSION: u16 = 1;

/// An exact nonnegative payout amount represented as `numerator / denominator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactAmount {
    /// Nonnegative numerator.
    pub numerator: u64,
    /// Positive explicit denominator.
    pub denominator: u64,
}

impl ExactAmount {
    const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };
}

/// A finite closed coordinate domain, in units of `numerator / denominator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScaledDomain {
    /// Inclusive lower coordinate numerator.
    pub lower: i128,
    /// Inclusive upper coordinate numerator.
    pub upper: i128,
    /// Positive shared coordinate denominator.
    pub denominator: u64,
}

/// Named payoff recipes understood by this categorical V1 compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductShape {
    /// Pays `payout` when the coordinate is at least `threshold`.
    BinaryThreshold {
        /// First winning coordinate numerator.
        threshold: i128,
        /// Fixed winning payout.
        payout: ExactAmount,
        /// Payout of the explicit resolution-failure claim.
        failure_payout: ExactAmount,
    },
    /// Pays one fixed amount in each ordered range bucket.
    OrderedRangeBuckets {
        /// Strictly increasing interior bucket boundaries.
        cut_points: Vec<i128>,
        /// Payout for every bucket in canonical order.
        payouts: Vec<ExactAmount>,
        /// Payout of the explicit resolution-failure claim.
        failure_payout: ExactAmount,
    },
    /// Pays `payout` in the lower tail and zero from `trigger` onward.
    CrashTail {
        /// First coordinate numerator outside the crash tail.
        trigger: i128,
        /// Fixed tail payout.
        payout: ExactAmount,
        /// Payout of the explicit resolution-failure claim.
        failure_payout: ExactAmount,
    },
    /// A within-cell graded shape, unsupported by categorical V1.
    CappedRamp {
        /// First coordinate numerator of the ramp.
        start: i128,
        /// First coordinate numerator at the cap.
        end: i128,
        /// Capped payout.
        cap: ExactAmount,
    },
    /// A within-cell graded shape, unsupported by categorical V1.
    Tent {
        /// First coordinate numerator of the tent.
        start: i128,
        /// Coordinate numerator of the unique peak.
        peak: i128,
        /// First coordinate numerator after the tent.
        end: i128,
        /// Peak payout.
        cap: ExactAmount,
    },
}

/// Authenticated Product context supplied by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationContext {
    /// Authenticated immutable capacity profile.
    pub capacity_profile: CapacityProfileV1,
    /// Content identity of `capacity_profile` established at the hash boundary.
    pub capacity_profile_id: CapacityProfileId,
    /// Release defining the named Terms semantics.
    pub terms_semantic_release_id: ContentId,
    /// Product-owned semantics of the exact source coordinate being partitioned.
    pub coordinate_domain_id: ContentId,
    /// Exact unit identity of the Source statistic consumed by the partition.
    pub result_unit_id: ContentId,
    /// Canonical occurrence-specific semantic bytes; must be nonempty.
    pub occurrence_artifact: Vec<u8>,
}

/// A complete compiler request for exactly `N` categorical outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequest<const N: usize> {
    /// One finite scaled coordinate domain.
    pub domain: ScaledDomain,
    /// One named payoff recipe.
    pub shape: ProductShape,
    /// Contract identities, capacity profile, and occurrence bytes.
    pub context: CompilationContext,
}

/// Canonical finite partition of a closed scaled-coordinate domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPartition {
    domain: ScaledDomain,
    cuts: Vec<i128>,
}

impl CanonicalPartition {
    /// Construct and validate an ordered exhaustive partition.
    pub fn new(domain: ScaledDomain, cuts: Vec<i128>) -> Result<Self, CompileError> {
        if domain.denominator == 0 {
            return Err(CompileError::ZeroCoordinateDenominator);
        }
        if domain.lower >= domain.upper {
            return Err(CompileError::InvalidDomain);
        }
        let mut previous = domain.lower;
        for cut in &cuts {
            if *cut <= previous || *cut >= domain.upper {
                return Err(CompileError::NonCanonicalPartition);
            }
            previous = *cut;
        }
        Ok(Self { domain, cuts })
    }

    /// Return the scaled coordinate domain.
    pub const fn domain(&self) -> ScaledDomain {
        self.domain
    }

    /// Return the strictly ordered interior boundaries.
    pub fn cuts(&self) -> &[i128] {
        &self.cuts
    }

    /// Return the number of exhaustive cells.
    pub fn cell_count(&self) -> Result<u32, CompileError> {
        u32::try_from(self.cuts.len())
            .map_err(|_| CompileError::CountOverflow)?
            .checked_add(1)
            .ok_or(CompileError::CountOverflow)
    }

}

/// Certificate binding one source request and every emitted preimage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCertificate {
    /// Certificate schema version.
    pub version: u16,
    /// Digest of the canonical source shape and coordinate domain.
    pub shape_commitment: ContentId,
    /// Digest of the canonical Product-owned finite result domain.
    pub result_domain_id: ContentId,
    /// Digest of the normalized exact-width portfolio template.
    pub portfolio_template_id: ContentId,
    /// Digest of the Terms preimage.
    pub terms_id: ContentId,
    /// Digest of the Occurrence preimage.
    pub occurrence_id: ContentId,
    /// Digest of the categorical-unit basis preimage.
    pub claim_basis_id: ContentId,
    /// Digest of the Instance preimage.
    pub instance_id: ContentId,
}

/// Deterministic outputs for an exact `N`-outcome Product recipe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledProduct<const N: usize> {
    /// Compiler-side finite projection partition over ordinary regions.
    pub partition: CanonicalPartition,
    /// Canonical Product-owned result domain consumed by Source.
    pub result_domain: FiniteResultDomainV1,
    /// Exact result-domain content preimage.
    pub result_domain_bytes: [u8; FINITE_RESULT_DOMAIN_BYTES],
    /// Elementary one-hot native claim basis.
    pub claim_basis: CategoricalUnitV1,
    /// Normalized rational portfolio recipe over the native claims.
    pub portfolio_template: PortfolioTemplateV1<N>,
    /// Exact encoded portfolio-template content preimage.
    pub portfolio_template_bytes: Vec<u8>,
    /// Terms preimage referring to the partition artifact.
    pub terms: TermsV1,
    /// Occurrence preimage referring to caller-provided occurrence bytes.
    pub occurrence: OccurrenceV1,
    /// Product Instance preimage binding the categorical basis.
    pub instance: InstanceV1,
    /// Independently recheckable compiler certificate.
    pub certificate: CompilerCertificate,
}

/// A precise refusal from categorical construction or independent rechecking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    /// A scaled-coordinate denominator was zero.
    ZeroCoordinateDenominator,
    /// The closed coordinate domain was empty or reversed.
    InvalidDomain,
    /// A partition did not contain the required two cells.
    PartitionTooSmall,
    /// Partition boundaries were not interior, strictly ordered, and unique.
    NonCanonicalPartition,
    /// A payoff denominator was zero.
    ZeroPayoutDenominator,
    /// Bucket count and range-boundary count disagree.
    BucketCountMismatch,
    /// A named trigger was not an interior coordinate.
    InvalidKnot,
    /// A ramp or tent needs within-cell graded claims unavailable in V1.
    UnsupportedWithinCellGradedShape,
    /// A graded categorical projection omitted one of its semantic knots.
    ProjectionKnotMissing,
    /// A projected graded payoff could not fit the exact `u64` rational form.
    UnrepresentableProjectedPayout,
    /// Runtime partition width did not equal the exact compile-time width `N`.
    OutcomeCountMismatch,
    /// An exact integer intermediate overflowed.
    ArithmeticOverflow,
    /// A collection length cannot be represented in the target contract.
    CountOverflow,
    /// A normalized portfolio coefficient cannot be represented as `u64`.
    UnrepresentablePortfolioCoefficient,
    /// The caller omitted required canonical occurrence bytes.
    EmptyOccurrenceArtifact,
    /// A Product result-domain artifact was malformed or noncanonical.
    InvalidResultDomainArtifact,
    /// A portfolio-template artifact was malformed or noncanonical.
    InvalidPortfolioTemplate,
    /// A certificate field or artifact content identity did not match.
    CertificateMismatch,
    /// A certificate release is unsupported.
    UnsupportedCertificate,
    /// A contract preimage rejected an emitted declaration.
    Contract(ContractError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "categorical Product compilation refused: {self:?}"
        )
    }
}

impl std::error::Error for CompileError {}

impl From<ContractError> for CompileError {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}

/// Compile one exact-width categorical Product and portfolio recipe.
pub fn compile<const N: usize>(
    request: &CompileRequest<N>,
) -> Result<CompiledProduct<N>, CompileError> {
    if request.context.occurrence_artifact.is_empty() {
        return Err(CompileError::EmptyOccurrenceArtifact);
    }
    let derived = derive_shape::<N>(&request.domain, &request.shape)?;
    let price_region_count = derived.partition.cell_count()?;
    require_outcome_width::<N>(price_region_count)?;
    let result_domain = FiniteResultDomainV1::new(
        request.context.coordinate_domain_id,
        request.context.result_unit_id,
        request.domain.denominator,
        derived.partition.cuts(),
    )?;
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_id = content_id(FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &result_domain_bytes)?;
    let partition_evidence_id = content_id(
        b"dclutch.result-domain-evidence.v1",
        &result_domain_bytes,
    )?;
    let partition_size = u32::try_from(result_domain_bytes.len())
        .map_err(|_| CompileError::CountOverflow)?;
    let outcome_count = u32::from(result_domain.outcome_count());
    let terms = TermsV1::new(
        TermsV1Input {
            capacity_profile_id: request.context.capacity_profile_id,
            semantic_release_id: request.context.terms_semantic_release_id,
            artifact_id: result_domain_id,
            partition_evidence_id,
            artifact_bytes: partition_size,
            page_count: pages(
                partition_size,
                request.context.capacity_profile.page_payload_bytes(),
            )?,
            partition_cell_count: outcome_count,
        },
        request.context.capacity_profile,
    )?;
    let terms_id = content_id(b"dclutch.terms.v1", &terms.to_bytes())?;

    let occurrence_size = u32::try_from(request.context.occurrence_artifact.len())
        .map_err(|_| CompileError::CountOverflow)?;
    let occurrence = OccurrenceV1::new(
        OccurrenceV1Input {
            terms_id,
            capacity_profile_id: request.context.capacity_profile_id,
            occurrence_artifact_id: content_id(
                b"dclutch.occurrence-artifact.v1",
                &request.context.occurrence_artifact,
            )?,
            artifact_bytes: occurrence_size,
            page_count: pages(
                occurrence_size,
                request.context.capacity_profile.page_payload_bytes(),
            )?,
        },
        request.context.capacity_profile,
    )?;
    let occurrence_id = content_id(b"dclutch.occurrence.v1", &occurrence.to_bytes())?;

    let claim_basis = CategoricalUnitV1::new(
        CategoricalUnitV1Input {
            capacity_profile_id: request.context.capacity_profile_id,
            outcome_count,
        },
        request.context.capacity_profile,
    )?;
    let claim_basis_id = content_id(b"dclutch.claim-basis.v1", &claim_basis.to_bytes())?;
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id,
        occurrence_id,
        claim_basis_id,
        result_domain_id,
        capacity_profile_id: request.context.capacity_profile_id,
        partition_cell_count: outcome_count,
    })?;
    let instance_id = content_id(b"dclutch.instance.v1", &instance.to_bytes())?;
    let portfolio_template = PortfolioTemplateV1::new(
        claim_basis_id,
        result_domain_id,
        derived.coefficients,
        derived.denominator,
    )?;
    let mut portfolio_template_bytes = vec![0; PortfolioTemplateV1::<N>::encoded_len()?];
    portfolio_template.encode(&mut portfolio_template_bytes)?;
    let portfolio_template_id =
        content_id(PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, &portfolio_template_bytes)?;
    let certificate = CompilerCertificate {
        version: CERTIFICATE_VERSION,
        shape_commitment: shape_commitment(&request.domain, &request.shape)?,
        result_domain_id,
        portfolio_template_id,
        terms_id,
        occurrence_id,
        claim_basis_id,
        instance_id,
    };
    let output = CompiledProduct {
        partition: derived.partition,
        result_domain,
        result_domain_bytes,
        claim_basis,
        portfolio_template,
        portfolio_template_bytes,
        terms,
        occurrence,
        instance,
        certificate,
    };
    recheck(request, &output)?;
    Ok(output)
}

/// Independently parse and recheck every emitted artifact and record binding.
///
/// This does not call [`compile`]. It regenerates the requested categorical
/// recipe, parses every persisted preimage, validates capacity and record
/// links, rechecks normalization and materialization, and compares every ID.
pub fn recheck<const N: usize>(
    request: &CompileRequest<N>,
    output: &CompiledProduct<N>,
) -> Result<(), CompileError> {
    if request.context.occurrence_artifact.is_empty() {
        return Err(CompileError::EmptyOccurrenceArtifact);
    }
    if output.certificate.version != CERTIFICATE_VERSION {
        return Err(CompileError::UnsupportedCertificate);
    }
    if output.certificate.shape_commitment != shape_commitment(&request.domain, &request.shape)? {
        return Err(CompileError::CertificateMismatch);
    }

    let expected = derive_shape::<N>(&request.domain, &request.shape)?;
    if output.partition != expected.partition {
        return Err(CompileError::CertificateMismatch);
    }
    let price_region_count = output.partition.cell_count()?;
    require_outcome_width::<N>(price_region_count)?;
    let result_domain = FiniteResultDomainV1::decode(&output.result_domain_bytes)
        .map_err(|_| CompileError::InvalidResultDomainArtifact)?;
    let expected_result_domain = FiniteResultDomainV1::new(
        request.context.coordinate_domain_id,
        request.context.result_unit_id,
        request.domain.denominator,
        output.partition.cuts(),
    )?;
    if result_domain != output.result_domain || result_domain != expected_result_domain {
        return Err(CompileError::CertificateMismatch);
    }
    let result_domain_id = content_id(
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &output.result_domain_bytes,
    )?;
    let partition_evidence_id = content_id(
        b"dclutch.result-domain-evidence.v1",
        &output.result_domain_bytes,
    )?;
    let partition_size = u32::try_from(output.result_domain_bytes.len())
        .map_err(|_| CompileError::CountOverflow)?;
    let partition_pages = pages(
        partition_size,
        request.context.capacity_profile.page_payload_bytes(),
    )?;
    let occurrence_artifact_id = content_id(
        b"dclutch.occurrence-artifact.v1",
        &request.context.occurrence_artifact,
    )?;
    let occurrence_size = u32::try_from(request.context.occurrence_artifact.len())
        .map_err(|_| CompileError::CountOverflow)?;
    let occurrence_pages = pages(
        occurrence_size,
        request.context.capacity_profile.page_payload_bytes(),
    )?;
    let terms = TermsV1::decode(&output.terms.to_bytes())?;
    let occurrence = OccurrenceV1::decode(&output.occurrence.to_bytes())?;
    let claim_basis = CategoricalUnitV1::decode(&output.claim_basis.to_bytes())?;
    let instance = InstanceV1::decode(&output.instance.to_bytes())?;
    let portfolio_template = PortfolioTemplateV1::<N>::decode(&output.portfolio_template_bytes)
        .map_err(portfolio_decode_error)?;

    terms.validate_capacity(request.context.capacity_profile)?;
    occurrence.validate_capacity(request.context.capacity_profile)?;
    claim_basis.validate_capacity(request.context.capacity_profile)?;
    let terms_id = content_id(b"dclutch.terms.v1", &terms.to_bytes())?;
    let occurrence_id = content_id(b"dclutch.occurrence.v1", &occurrence.to_bytes())?;
    let claim_basis_id = content_id(b"dclutch.claim-basis.v1", &claim_basis.to_bytes())?;
    let portfolio_template_id = content_id(
        PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1,
        &output.portfolio_template_bytes,
    )?;
    let instance_id = content_id(b"dclutch.instance.v1", &instance.to_bytes())?;

    instance.validate_occurrence(terms_id, terms, occurrence_id, occurrence)?;
    instance.validate_claim_basis(claim_basis_id, claim_basis)?;
    portfolio_template.validate_claim_basis(claim_basis_id, result_domain_id, claim_basis)?;
    if portfolio_template != output.portfolio_template
        || portfolio_template.claim_basis_id() != claim_basis_id
        || portfolio_template.result_domain_id() != result_domain_id
        || portfolio_template.denominator() != expected.denominator
        || portfolio_template.coefficients() != &expected.coefficients
        || claim_basis.outcome_count() != u32::from(result_domain.outcome_count())
        || normalization_divisor(&expected.coefficients, expected.denominator) != 1
    {
        return Err(CompileError::CertificateMismatch);
    }
    let mut materialized = [0; N];
    portfolio_template.materialize(expected.denominator, &mut materialized)?;
    if materialized != expected.coefficients {
        return Err(CompileError::InvalidPortfolioTemplate);
    }

    if output.certificate.result_domain_id != result_domain_id
        || output.certificate.portfolio_template_id != portfolio_template_id
        || output.certificate.terms_id != terms_id
        || output.certificate.occurrence_id != occurrence_id
        || output.certificate.claim_basis_id != claim_basis_id
        || output.certificate.instance_id != instance_id
        || terms.artifact_id() != result_domain_id
        || terms.partition_evidence_id() != partition_evidence_id
        || terms.partition_cell_count() != u32::from(result_domain.outcome_count())
        || terms.capacity_profile_id() != request.context.capacity_profile_id
        || terms.semantic_release_id() != request.context.terms_semantic_release_id
        || terms.artifact_bytes() != partition_size
        || terms.page_count() != partition_pages
        || occurrence.terms_id() != terms_id
        || occurrence.capacity_profile_id() != request.context.capacity_profile_id
        || occurrence.occurrence_artifact_id() != occurrence_artifact_id
        || occurrence.artifact_bytes() != occurrence_size
        || occurrence.page_count() != occurrence_pages
        || claim_basis.capacity_profile_id() != request.context.capacity_profile_id
        || instance.terms_id() != terms_id
        || instance.occurrence_id() != occurrence_id
        || instance.claim_basis_id() != claim_basis_id
        || instance.result_domain_id() != result_domain_id
        || instance.capacity_profile_id() != request.context.capacity_profile_id
    {
        return Err(CompileError::CertificateMismatch);
    }
    Ok(())
}

struct DerivedShape<const N: usize> {
    partition: CanonicalPartition,
    coefficients: [u64; N],
    denominator: u64,
}

fn derive_shape<const N: usize>(
    domain: &ScaledDomain,
    shape: &ProductShape,
) -> Result<DerivedShape<N>, CompileError> {
    let (partition, mut payouts, failure_payout) = match shape {
        ProductShape::BinaryThreshold {
            threshold,
            payout,
            failure_payout,
        } => {
            interior(domain, *threshold)?;
            (
                CanonicalPartition::new(*domain, vec![*threshold])?,
                vec![ExactAmount::ZERO, *payout],
                *failure_payout,
            )
        }
        ProductShape::OrderedRangeBuckets {
            cut_points,
            payouts,
            failure_payout,
        } => {
            let expected = cut_points
                .len()
                .checked_add(1)
                .ok_or(CompileError::CountOverflow)?;
            if payouts.len() != expected {
                return Err(CompileError::BucketCountMismatch);
            }
            (
                CanonicalPartition::new(*domain, cut_points.clone())?,
                payouts.clone(),
                *failure_payout,
            )
        }
        ProductShape::CrashTail {
            trigger,
            payout,
            failure_payout,
        } => {
            interior(domain, *trigger)?;
            (
                CanonicalPartition::new(*domain, vec![*trigger])?,
                vec![*payout, ExactAmount::ZERO],
                *failure_payout,
            )
        }
        ProductShape::CappedRamp { .. } | ProductShape::Tent { .. } => {
            return Err(CompileError::UnsupportedWithinCellGradedShape);
        }
    };
    let price_region_count = partition.cell_count()?;
    let payout_count = payouts
        .len()
        .checked_add(1)
        .ok_or(CompileError::CountOverflow)?;
    if payout_count != N {
        return Err(CompileError::OutcomeCountMismatch);
    }
    require_outcome_width::<N>(price_region_count)?;
    payouts.push(failure_payout);
    let (coefficients, denominator) = normalize_payouts::<N>(&payouts)?;
    Ok(DerivedShape {
        partition,
        coefficients,
        denominator,
    })
}

fn normalize_payouts<const N: usize>(
    payouts: &[ExactAmount],
) -> Result<([u64; N], u64), CompileError> {
    if payouts.len() != N {
        return Err(CompileError::OutcomeCountMismatch);
    }
    let mut denominator = 1;
    for payout in payouts {
        if payout.denominator == 0 {
            return Err(CompileError::ZeroPayoutDenominator);
        }
        let reduced_denominator = payout.denominator / gcd(payout.numerator, payout.denominator);
        denominator = lcm(denominator, reduced_denominator)?;
    }
    let mut coefficients = [0; N];
    for (index, payout) in payouts.iter().enumerate() {
        let divisor = gcd(payout.numerator, payout.denominator);
        let numerator = payout.numerator / divisor;
        let reduced_denominator = payout.denominator / divisor;
        let multiplier = denominator
            .checked_div(reduced_denominator)
            .ok_or(CompileError::ArithmeticOverflow)?;
        *coefficients
            .get_mut(index)
            .ok_or(CompileError::OutcomeCountMismatch)? = numerator
            .checked_mul(multiplier)
            .ok_or(CompileError::UnrepresentablePortfolioCoefficient)?;
    }
    let common = normalization_divisor(&coefficients, denominator);
    if common > 1 {
        denominator /= common;
        for coefficient in &mut coefficients {
            *coefficient /= common;
        }
    }
    Ok((coefficients, denominator))
}

fn require_outcome_width<const N: usize>(price_region_count: u32) -> Result<(), CompileError> {
    let outcomes = usize::try_from(price_region_count)
        .map_err(|_| CompileError::CountOverflow)?
        .checked_add(1)
        .ok_or(CompileError::CountOverflow)?;
    if outcomes != N {
        return Err(CompileError::OutcomeCountMismatch);
    }
    Ok(())
}

fn normalization_divisor<const N: usize>(coefficients: &[u64; N], denominator: u64) -> u64 {
    let mut divisor = denominator;
    for coefficient in coefficients {
        divisor = gcd(divisor, *coefficient);
    }
    divisor
}

fn interior(domain: &ScaledDomain, knot: i128) -> Result<(), CompileError> {
    if domain.denominator == 0 {
        return Err(CompileError::ZeroCoordinateDenominator);
    }
    if domain.lower >= domain.upper {
        return Err(CompileError::InvalidDomain);
    }
    if knot <= domain.lower || knot >= domain.upper {
        return Err(CompileError::InvalidKnot);
    }
    Ok(())
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn lcm(left: u64, right: u64) -> Result<u64, CompileError> {
    left.checked_div(gcd(left, right))
        .ok_or(CompileError::ArithmeticOverflow)?
        .checked_mul(right)
        .ok_or(CompileError::ArithmeticOverflow)
}

fn pages(bytes: u32, payload: u32) -> Result<u32, CompileError> {
    if bytes == 0 || payload == 0 {
        return Err(CompileError::ArithmeticOverflow);
    }
    bytes
        .checked_sub(1)
        .ok_or(CompileError::ArithmeticOverflow)?
        .checked_div(payload)
        .ok_or(CompileError::ArithmeticOverflow)?
        .checked_add(1)
        .ok_or(CompileError::ArithmeticOverflow)
}

fn shape_commitment(
    domain: &ScaledDomain,
    shape: &ProductShape,
) -> Result<ContentId, CompileError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&domain.lower.to_le_bytes());
    bytes.extend_from_slice(&domain.upper.to_le_bytes());
    bytes.extend_from_slice(&domain.denominator.to_le_bytes());
    match shape {
        ProductShape::BinaryThreshold {
            threshold,
            payout,
            failure_payout,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&threshold.to_le_bytes());
            amount_bytes(&mut bytes, *payout);
            amount_bytes(&mut bytes, *failure_payout);
        }
        ProductShape::OrderedRangeBuckets {
            cut_points,
            payouts,
            failure_payout,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(
                &u32::try_from(cut_points.len())
                    .map_err(|_| CompileError::CountOverflow)?
                    .to_le_bytes(),
            );
            for cut in cut_points {
                bytes.extend_from_slice(&cut.to_le_bytes());
            }
            bytes.extend_from_slice(
                &u32::try_from(payouts.len())
                    .map_err(|_| CompileError::CountOverflow)?
                    .to_le_bytes(),
            );
            for payout in payouts {
                amount_bytes(&mut bytes, *payout);
            }
            amount_bytes(&mut bytes, *failure_payout);
        }
        ProductShape::CrashTail {
            trigger,
            payout,
            failure_payout,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&trigger.to_le_bytes());
            amount_bytes(&mut bytes, *payout);
            amount_bytes(&mut bytes, *failure_payout);
        }
        ProductShape::CappedRamp { start, end, cap } => {
            bytes.push(4);
            bytes.extend_from_slice(&start.to_le_bytes());
            bytes.extend_from_slice(&end.to_le_bytes());
            amount_bytes(&mut bytes, *cap);
        }
        ProductShape::Tent {
            start,
            peak,
            end,
            cap,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&start.to_le_bytes());
            bytes.extend_from_slice(&peak.to_le_bytes());
            bytes.extend_from_slice(&end.to_le_bytes());
            amount_bytes(&mut bytes, *cap);
        }
    }
    content_id(b"dclutch.product-shape.v1", &bytes)
}

fn amount_bytes(output: &mut Vec<u8>, amount: ExactAmount) {
    output.extend_from_slice(&amount.numerator.to_le_bytes());
    output.extend_from_slice(&amount.denominator.to_le_bytes());
}

fn content_id(domain: &[u8], bytes: &[u8]) -> Result<ContentId, CompileError> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(bytes);
    ContentId::new(hasher.finalize().into()).map_err(CompileError::from)
}

fn portfolio_decode_error(error: ContractError) -> CompileError {
    match error {
        ContractError::InvalidLength
        | ContractError::InvalidMagic
        | ContractError::UnsupportedSchema
        | ContractError::NonCanonicalReservedBytes
        | ContractError::UnsupportedPortfolioWidth
        | ContractError::ZeroPortfolioDenominator
        | ContractError::EmptyPortfolioTemplate
        | ContractError::NonCanonicalPortfolioTemplate => CompileError::InvalidPortfolioTemplate,
        other => CompileError::Contract(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product_contract::capacity::{CapacityEnvelope, CapacityProfileV1Input};

    fn id(fill: u8) -> ContentId {
        ContentId::new([fill; 32]).expect("nonzero fixture")
    }

    fn context() -> CompilationContext {
        let profile = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 4_096,
            page_payload_bytes: 128,
            max_pages: 32,
            max_partition_cells: 16,
        })
        .expect("valid fixture profile");
        CompilationContext {
            capacity_profile: profile,
            capacity_profile_id: CapacityProfileId::new(id(3)),
            terms_semantic_release_id: id(4),
            coordinate_domain_id: id(5),
            result_unit_id: id(6),
            occurrence_artifact: b"occurrence-v1: fixture".to_vec(),
        }
    }

    fn request<const N: usize>(shape: ProductShape) -> CompileRequest<N> {
        CompileRequest {
            domain: ScaledDomain {
                lower: 0,
                upper: 100,
                denominator: 100,
            },
            shape,
            context: context(),
        }
    }

    #[test]
    fn binary_emits_price_regions_and_distinct_failure() {
        let request = request::<3>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 14,
                denominator: 6,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let compiled = compile::<3>(&request).expect("compile binary");
        assert_eq!(compiled.partition.cuts(), &[60]);
        assert_eq!(compiled.result_domain_bytes.len(), FINITE_RESULT_DOMAIN_BYTES);
        assert_eq!(compiled.result_domain.region_count(), 2);
        assert_eq!(compiled.result_domain.failure_selector(), 2);
        assert_eq!(compiled.claim_basis.outcome_count(), 3);
        assert_eq!(compiled.portfolio_template.denominator(), 3);
        assert_eq!(compiled.portfolio_template.coefficients(), &[0, 7, 0]);
        let mut materialized = [0; 3];
        compiled
            .portfolio_template
            .materialize(3, &mut materialized)
            .expect("exact materialization");
        assert_eq!(materialized, [0, 7, 0]);
        recheck(&request, &compiled).expect("independent recheck");
    }

    #[test]
    fn buckets_and_crash_tail_preserve_exact_rational_recipes() {
        let bucket_request = request::<4>(ProductShape::OrderedRangeBuckets {
            cut_points: vec![25, 75],
            payouts: vec![
                ExactAmount {
                    numerator: 0,
                    denominator: 9,
                },
                ExactAmount {
                    numerator: 2,
                    denominator: 4,
                },
                ExactAmount {
                    numerator: 3,
                    denominator: 4,
                },
            ],
            failure_payout: ExactAmount::ZERO,
        });
        let buckets = compile::<4>(&bucket_request).expect("buckets");
        assert_eq!(buckets.portfolio_template.denominator(), 4);
        assert_eq!(buckets.portfolio_template.coefficients(), &[0, 2, 3, 0]);

        let tail_request = request::<3>(ProductShape::CrashTail {
            trigger: 30,
            payout: ExactAmount {
                numerator: 10,
                denominator: 2,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let tail = compile::<3>(&tail_request).expect("tail");
        assert_eq!(tail.partition.cuts(), &[30]);
        assert_eq!(tail.portfolio_template.denominator(), 1);
        assert_eq!(tail.portfolio_template.coefficients(), &[5, 0, 0]);
    }

    #[test]
    fn empty_and_graded_recipes_refuse_without_approximation() {
        let empty = request::<3>(ProductShape::OrderedRangeBuckets {
            cut_points: vec![50],
            payouts: vec![ExactAmount::ZERO, ExactAmount::ZERO],
            failure_payout: ExactAmount::ZERO,
        });
        assert_eq!(
            compile::<3>(&empty),
            Err(CompileError::Contract(
                ContractError::EmptyPortfolioTemplate
            ))
        );
        let cap = ExactAmount {
            numerator: 9,
            denominator: 2,
        };
        assert_eq!(
            compile::<4>(&request(ProductShape::CappedRamp {
                start: 20,
                end: 60,
                cap,
            })),
            Err(CompileError::UnsupportedWithinCellGradedShape)
        );
        assert_eq!(
            compile::<5>(&request(ProductShape::Tent {
                start: 20,
                peak: 50,
                end: 80,
                cap,
            })),
            Err(CompileError::UnsupportedWithinCellGradedShape)
        );
    }

    #[test]
    fn width_bucket_and_denominator_refusals_are_explicit() {
        assert_eq!(
            compile::<2>(&request(ProductShape::BinaryThreshold {
                threshold: 60,
                payout: ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::OutcomeCountMismatch)
        );
        assert_eq!(
            compile::<3>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60],
                payouts: vec![ExactAmount {
                    numerator: 1,
                    denominator: 1,
                }],
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::BucketCountMismatch)
        );
        assert_eq!(
            compile::<3>(&request(ProductShape::CrashTail {
                trigger: 30,
                payout: ExactAmount {
                    numerator: 1,
                    denominator: 0,
                },
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::ZeroPayoutDenominator)
        );
    }

    #[test]
    fn noncanonical_partitions_and_exact_arithmetic_overflow_refuse() {
        assert_eq!(
            compile::<4>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60, 60],
                payouts: vec![
                    ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    };
                    3
                ],
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::NonCanonicalPartition)
        );
        assert_eq!(
            compile::<3>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![50],
                payouts: vec![
                    ExactAmount {
                        numerator: 1,
                        denominator: u64::MAX,
                    },
                    ExactAmount {
                        numerator: 1,
                        denominator: u64::MAX - 1,
                    },
                ],
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::ArithmeticOverflow)
        );
        assert_eq!(
            compile::<3>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![50],
                payouts: vec![
                    ExactAmount {
                        numerator: u64::MAX,
                        denominator: 1,
                    },
                    ExactAmount {
                        numerator: 1,
                        denominator: 2,
                    },
                ],
                failure_payout: ExactAmount::ZERO,
            })),
            Err(CompileError::UnrepresentablePortfolioCoefficient)
        );
    }

    #[test]
    fn recheck_detects_partition_template_basis_and_certificate_substitution() {
        let request = request::<3>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 7,
                denominator: 3,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let mut changed = compile::<3>(&request).expect("compile");
        *changed
            .result_domain_bytes
            .last_mut()
            .expect("canonical tail") ^= 1;
        assert!(recheck(&request, &changed).is_err());

        let mut changed = compile::<3>(&request).expect("compile");
        *changed
            .portfolio_template_bytes
            .last_mut()
            .expect("template coefficient") ^= 1;
        assert!(recheck(&request, &changed).is_err());

        let mut changed = compile::<3>(&request).expect("compile");
        changed
            .portfolio_template_bytes
            .get_mut(80..88)
            .expect("denominator word")
            .copy_from_slice(&6u64.to_le_bytes());
        changed
            .portfolio_template_bytes
            .get_mut(96..104)
            .expect("winning coefficient word")
            .copy_from_slice(&14u64.to_le_bytes());
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::InvalidPortfolioTemplate)
        );

        let mut changed = compile::<3>(&request).expect("compile");
        changed.portfolio_template = PortfolioTemplateV1::new(
            id(88),
            changed.instance.result_domain_id(),
            [0, 7, 0],
            3,
        )
        .expect("foreign template");
        changed
            .portfolio_template
            .encode(&mut changed.portfolio_template_bytes)
            .expect("foreign encoding");
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::Contract(ContractError::IdentityMismatch))
        );

        let mut changed = compile::<3>(&request).expect("compile");
        changed.claim_basis = CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: CapacityProfileId::new(id(99)),
                outcome_count: 3,
            },
            request.context.capacity_profile,
        )
        .expect("alternate basis");
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::Contract(ContractError::IdentityMismatch))
        );

        let mut changed = compile::<3>(&request).expect("compile");
        let foreign_terms = TermsV1::new(
            TermsV1Input {
                capacity_profile_id: request.context.capacity_profile_id,
                semantic_release_id: id(77),
                artifact_id: changed.terms.artifact_id(),
                partition_evidence_id: changed.terms.partition_evidence_id(),
                artifact_bytes: changed.terms.artifact_bytes(),
                page_count: changed.terms.page_count(),
                partition_cell_count: changed.terms.partition_cell_count(),
            },
            request.context.capacity_profile,
        )
        .expect("foreign terms");
        let foreign_terms_id =
            content_id(b"dclutch.terms.v1", &foreign_terms.to_bytes()).expect("terms id");
        let foreign_occurrence = OccurrenceV1::new(
            OccurrenceV1Input {
                terms_id: foreign_terms_id,
                capacity_profile_id: changed.occurrence.capacity_profile_id(),
                occurrence_artifact_id: changed.occurrence.occurrence_artifact_id(),
                artifact_bytes: changed.occurrence.artifact_bytes(),
                page_count: changed.occurrence.page_count(),
            },
            request.context.capacity_profile,
        )
        .expect("foreign occurrence");
        let foreign_occurrence_id =
            content_id(b"dclutch.occurrence.v1", &foreign_occurrence.to_bytes())
                .expect("occurrence id");
        let foreign_instance = InstanceV1::new(InstanceV1Input {
            terms_id: foreign_terms_id,
            occurrence_id: foreign_occurrence_id,
            claim_basis_id: changed.instance.claim_basis_id(),
            result_domain_id: changed.instance.result_domain_id(),
            capacity_profile_id: changed.instance.capacity_profile_id(),
            partition_cell_count: changed.instance.partition_cell_count(),
        })
        .expect("foreign instance");
        let foreign_instance_id =
            content_id(b"dclutch.instance.v1", &foreign_instance.to_bytes()).expect("instance id");
        changed.terms = foreign_terms;
        changed.occurrence = foreign_occurrence;
        changed.instance = foreign_instance;
        changed.certificate.terms_id = foreign_terms_id;
        changed.certificate.occurrence_id = foreign_occurrence_id;
        changed.certificate.instance_id = foreign_instance_id;
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::CertificateMismatch)
        );

        let mut changed = compile::<3>(&request).expect("compile");
        changed.certificate.portfolio_template_id = id(98);
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::CertificateMismatch)
        );
    }

    #[test]
    fn same_width_result_domain_substitution_refuses() {
        let request = request::<3>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 1,
                denominator: 1,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let mut compiled = compile(&request).expect("compiled");
        let substitute = FiniteResultDomainV1::new(
            request.context.coordinate_domain_id,
            request.context.result_unit_id,
            request.domain.denominator,
            &[61],
        )
        .expect("same-width substitute");
        compiled.result_domain = substitute;
        compiled.result_domain_bytes = substitute.to_bytes();
        assert_eq!(
            recheck(&request, &compiled),
            Err(CompileError::CertificateMismatch)
        );
    }

    #[test]
    fn result_domain_identity_uses_product_namespace_and_exact_bytes() {
        assert_eq!(
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            b"dclutch.result-domain.v1"
        );
        let request = request::<3>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 1,
                denominator: 1,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let compiled = compile(&request).expect("compiled");
        let expected_id = content_id(
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            &compiled.result_domain_bytes,
        )
        .expect("result-domain identity");
        assert_eq!(compiled.certificate.result_domain_id, expected_id);
        assert_eq!(compiled.terms.artifact_id(), expected_id);
        assert_eq!(compiled.instance.result_domain_id(), expected_id);

        let substitute = FiniteResultDomainV1::new(
            request.context.coordinate_domain_id,
            request.context.result_unit_id,
            request.domain.denominator,
            &[61],
        )
        .expect("same-width substitute");
        let substitute_id = content_id(
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            &substitute.to_bytes(),
        )
        .expect("substitute identity");
        assert_ne!(substitute_id, expected_id);
    }

    #[test]
    fn portfolio_template_identity_uses_product_namespace_and_exact_bytes() {
        assert_eq!(
            PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1,
            b"dclutch.portfolio-template.v1"
        );
        let request = request::<3>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 1,
                denominator: 1,
            },
            failure_payout: ExactAmount::ZERO,
        });
        let compiled = compile(&request).expect("compiled");
        let expected_id = content_id(
            PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1,
            &compiled.portfolio_template_bytes,
        )
        .expect("portfolio-template identity");
        assert_eq!(compiled.certificate.portfolio_template_id, expected_id);

        let substitute = PortfolioTemplateV1::new(
            compiled.instance.claim_basis_id(),
            compiled.instance.result_domain_id(),
            [1, 0, 0],
            1,
        )
        .expect("same-width substitute");
        let mut substitute_bytes = vec![0; PortfolioTemplateV1::<3>::encoded_len().expect("width")];
        substitute
            .encode(&mut substitute_bytes)
            .expect("substitute encoding");
        assert_ne!(substitute_bytes, compiled.portfolio_template_bytes);
        let substitute_id = content_id(PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, &substitute_bytes)
            .expect("substitute identity");
        assert_ne!(substitute_id, expected_id);
    }
}
