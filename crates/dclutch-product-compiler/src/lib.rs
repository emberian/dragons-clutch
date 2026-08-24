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
//! V1 supports only payoffs constant within every partition cell. Graded ramps
//! and tents are refused explicitly. There is no evaluator identity,
//! coefficient artifact, polynomial approximation, or rounding authority.

use core::fmt;

use dclutch_product_contract::capacity::{CapacityProfileId, CapacityProfileV1};
use dclutch_product_contract::claim::{CategoricalUnitV1, CategoricalUnitV1Input};
use dclutch_product_contract::portfolio::PortfolioTemplateV1;
use dclutch_product_contract::product::{
    InstanceV1, InstanceV1Input, OccurrenceV1, OccurrenceV1Input, TermsV1, TermsV1Input,
};
use dclutch_product_contract::{ContentId, Error as ContractError};
use sha2::{Digest, Sha256};

const PARTITION_MAGIC: [u8; 8] = *b"DCLTPAR1";
const PARTITION_VERSION: u16 = 1;
const PARTITION_HEADER_BYTES: usize = 64;
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
    },
    /// Pays one fixed amount in each ordered range bucket.
    OrderedRangeBuckets {
        /// Strictly increasing interior bucket boundaries.
        cut_points: Vec<i128>,
        /// Payout for every bucket in canonical order.
        payouts: Vec<ExactAmount>,
    },
    /// Pays `payout` in the lower tail and zero from `trigger` onward.
    CrashTail {
        /// First coordinate numerator outside the crash tail.
        trigger: i128,
        /// Fixed tail payout.
        payout: ExactAmount,
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
        if cuts.is_empty() {
            return Err(CompileError::PartitionTooSmall);
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

    /// Encode the canonical partition artifact.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CompileError> {
        let count = self.cell_count()?;
        let cut_bytes = self
            .cuts
            .len()
            .checked_mul(16)
            .ok_or(CompileError::CountOverflow)?;
        let total = PARTITION_HEADER_BYTES
            .checked_add(cut_bytes)
            .ok_or(CompileError::CountOverflow)?;
        let mut output = Vec::with_capacity(total);
        output.extend_from_slice(&PARTITION_MAGIC);
        output.extend_from_slice(&PARTITION_VERSION.to_le_bytes());
        output.extend_from_slice(&[0; 6]);
        output.extend_from_slice(&self.domain.denominator.to_le_bytes());
        output.extend_from_slice(&count.to_le_bytes());
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&self.domain.lower.to_le_bytes());
        output.extend_from_slice(&self.domain.upper.to_le_bytes());
        for cut in &self.cuts {
            output.extend_from_slice(&cut.to_le_bytes());
        }
        Ok(output)
    }

    /// Decode and independently validate a partition artifact.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompileError> {
        if bytes.len() < PARTITION_HEADER_BYTES {
            return Err(CompileError::InvalidPartitionArtifact);
        }
        if bytes.get(0..8) != Some(PARTITION_MAGIC.as_slice())
            || bytes.get(8..10) != Some(PARTITION_VERSION.to_le_bytes().as_slice())
            || bytes
                .get(10..16)
                .is_none_or(|reserved| reserved.iter().any(|value| *value != 0))
            || bytes
                .get(28..32)
                .is_none_or(|reserved| reserved.iter().any(|value| *value != 0))
        {
            return Err(CompileError::InvalidPartitionArtifact);
        }
        let denominator = read_u64(bytes, 16)?;
        let cells = read_u32(bytes, 24)?;
        if cells < 2 {
            return Err(CompileError::PartitionTooSmall);
        }
        let cuts = usize::try_from(cells - 1).map_err(|_| CompileError::CountOverflow)?;
        let expected = PARTITION_HEADER_BYTES
            .checked_add(cuts.checked_mul(16).ok_or(CompileError::CountOverflow)?)
            .ok_or(CompileError::CountOverflow)?;
        if bytes.len() != expected {
            return Err(CompileError::InvalidPartitionArtifact);
        }
        let mut decoded_cuts = Vec::with_capacity(cuts);
        for index in 0..cuts {
            let offset = PARTITION_HEADER_BYTES
                .checked_add(index.checked_mul(16).ok_or(CompileError::CountOverflow)?)
                .ok_or(CompileError::CountOverflow)?;
            decoded_cuts.push(read_i128(bytes, offset)?);
        }
        Self::new(
            ScaledDomain {
                lower: read_i128(bytes, 32)?,
                upper: read_i128(bytes, 48)?,
                denominator,
            },
            decoded_cuts,
        )
    }
}

/// Certificate binding one source request and every emitted preimage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCertificate {
    /// Certificate schema version.
    pub version: u16,
    /// Digest of the canonical source shape and coordinate domain.
    pub shape_commitment: ContentId,
    /// Digest of the canonical partition artifact.
    pub partition_id: ContentId,
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
    /// Canonical partition object.
    pub partition: CanonicalPartition,
    /// Canonical partition artifact bytes.
    pub partition_bytes: Vec<u8>,
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
    /// A partition artifact was malformed or noncanonical.
    InvalidPartitionArtifact,
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
    let cell_count = derived.partition.cell_count()?;
    require_width::<N>(cell_count)?;

    let partition_bytes = derived.partition.to_bytes()?;
    let partition_id = content_id(b"dclutch.partition.v1", &partition_bytes)?;
    let partition_evidence_id = content_id(b"dclutch.partition-evidence.v1", &partition_bytes)?;
    let partition_size =
        u32::try_from(partition_bytes.len()).map_err(|_| CompileError::CountOverflow)?;
    let terms = TermsV1::new(
        TermsV1Input {
            capacity_profile_id: request.context.capacity_profile_id,
            semantic_release_id: request.context.terms_semantic_release_id,
            artifact_id: partition_id,
            partition_evidence_id,
            artifact_bytes: partition_size,
            page_count: pages(
                partition_size,
                request.context.capacity_profile.page_payload_bytes(),
            )?,
            partition_cell_count: cell_count,
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
            outcome_count: cell_count,
        },
        request.context.capacity_profile,
    )?;
    let claim_basis_id = content_id(b"dclutch.claim-basis.v1", &claim_basis.to_bytes())?;
    let portfolio_template =
        PortfolioTemplateV1::new(claim_basis_id, derived.coefficients, derived.denominator)?;
    let mut portfolio_template_bytes = vec![0; PortfolioTemplateV1::<N>::encoded_len()?];
    portfolio_template.encode(&mut portfolio_template_bytes)?;
    let portfolio_template_id =
        content_id(b"dclutch.portfolio-template.v1", &portfolio_template_bytes)?;

    let instance = InstanceV1::new(InstanceV1Input {
        terms_id,
        occurrence_id,
        claim_basis_id,
        capacity_profile_id: request.context.capacity_profile_id,
        partition_cell_count: cell_count,
    })?;
    let instance_id = content_id(b"dclutch.instance.v1", &instance.to_bytes())?;
    let certificate = CompilerCertificate {
        version: CERTIFICATE_VERSION,
        shape_commitment: shape_commitment(&request.domain, &request.shape)?,
        partition_id,
        portfolio_template_id,
        terms_id,
        occurrence_id,
        claim_basis_id,
        instance_id,
    };
    let output = CompiledProduct {
        partition: derived.partition,
        partition_bytes,
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

    let parsed_partition = CanonicalPartition::decode(&output.partition_bytes)?;
    let expected = derive_shape::<N>(&request.domain, &request.shape)?;
    if parsed_partition != output.partition || parsed_partition != expected.partition {
        return Err(CompileError::CertificateMismatch);
    }
    let cell_count = parsed_partition.cell_count()?;
    require_width::<N>(cell_count)?;

    let partition_id = content_id(b"dclutch.partition.v1", &output.partition_bytes)?;
    let partition_evidence_id =
        content_id(b"dclutch.partition-evidence.v1", &output.partition_bytes)?;
    let occurrence_artifact_id = content_id(
        b"dclutch.occurrence-artifact.v1",
        &request.context.occurrence_artifact,
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
        b"dclutch.portfolio-template.v1",
        &output.portfolio_template_bytes,
    )?;
    let instance_id = content_id(b"dclutch.instance.v1", &instance.to_bytes())?;

    instance.validate_occurrence(terms_id, terms, occurrence_id, occurrence)?;
    instance.validate_claim_basis(claim_basis_id, claim_basis)?;
    portfolio_template.validate_claim_basis(claim_basis_id, claim_basis)?;
    if portfolio_template != output.portfolio_template
        || portfolio_template.claim_basis_id() != claim_basis_id
        || portfolio_template.denominator() != expected.denominator
        || portfolio_template.coefficients() != &expected.coefficients
        || claim_basis.outcome_count() != cell_count
        || normalization_divisor(&expected.coefficients, expected.denominator) != 1
    {
        return Err(CompileError::CertificateMismatch);
    }
    let mut materialized = [0; N];
    portfolio_template.materialize(expected.denominator, &mut materialized)?;
    if materialized != expected.coefficients {
        return Err(CompileError::InvalidPortfolioTemplate);
    }

    if output.certificate.partition_id != partition_id
        || output.certificate.portfolio_template_id != portfolio_template_id
        || output.certificate.terms_id != terms_id
        || output.certificate.occurrence_id != occurrence_id
        || output.certificate.claim_basis_id != claim_basis_id
        || output.certificate.instance_id != instance_id
        || terms.artifact_id() != partition_id
        || terms.partition_evidence_id() != partition_evidence_id
        || terms.partition_cell_count() != cell_count
        || occurrence.terms_id() != terms_id
        || occurrence.to_bytes().get(80..112) != Some(occurrence_artifact_id.as_bytes().as_slice())
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
    let (partition, payouts) = match shape {
        ProductShape::BinaryThreshold { threshold, payout } => {
            interior(domain, *threshold)?;
            (
                CanonicalPartition::new(*domain, vec![*threshold])?,
                vec![ExactAmount::ZERO, *payout],
            )
        }
        ProductShape::OrderedRangeBuckets {
            cut_points,
            payouts,
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
            )
        }
        ProductShape::CrashTail { trigger, payout } => {
            interior(domain, *trigger)?;
            (
                CanonicalPartition::new(*domain, vec![*trigger])?,
                vec![*payout, ExactAmount::ZERO],
            )
        }
        ProductShape::CappedRamp { .. } | ProductShape::Tent { .. } => {
            return Err(CompileError::UnsupportedWithinCellGradedShape);
        }
    };
    let cell_count = partition.cell_count()?;
    if payouts.len() != N {
        return Err(CompileError::OutcomeCountMismatch);
    }
    require_width::<N>(cell_count)?;
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

fn require_width<const N: usize>(cell_count: u32) -> Result<(), CompileError> {
    if usize::try_from(cell_count).map_err(|_| CompileError::CountOverflow)? != N {
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
        ProductShape::BinaryThreshold { threshold, payout } => {
            bytes.push(1);
            bytes.extend_from_slice(&threshold.to_le_bytes());
            amount_bytes(&mut bytes, *payout);
        }
        ProductShape::OrderedRangeBuckets {
            cut_points,
            payouts,
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
        }
        ProductShape::CrashTail { trigger, payout } => {
            bytes.push(3);
            bytes.extend_from_slice(&trigger.to_le_bytes());
            amount_bytes(&mut bytes, *payout);
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

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CompileError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, CompileError> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128, CompileError> {
    Ok(i128::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], CompileError> {
    let end = offset
        .checked_add(N)
        .ok_or(CompileError::InvalidPartitionArtifact)?;
    bytes
        .get(offset..end)
        .ok_or(CompileError::InvalidPartitionArtifact)?
        .try_into()
        .map_err(|_| CompileError::InvalidPartitionArtifact)
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
    fn binary_emits_basis_and_normalized_exact_two_template() {
        let request = request::<2>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 14,
                denominator: 6,
            },
        });
        let compiled = compile::<2>(&request).expect("compile binary");
        assert_eq!(compiled.partition.cuts(), &[60]);
        assert_eq!(compiled.partition_bytes.len(), 80);
        assert_eq!(compiled.claim_basis.outcome_count(), 2);
        assert_eq!(compiled.portfolio_template.denominator(), 3);
        assert_eq!(compiled.portfolio_template.coefficients(), &[0, 7]);
        let mut materialized = [0; 2];
        compiled
            .portfolio_template
            .materialize(3, &mut materialized)
            .expect("exact materialization");
        assert_eq!(materialized, [0, 7]);
        recheck(&request, &compiled).expect("independent recheck");
    }

    #[test]
    fn buckets_and_crash_tail_preserve_exact_rational_recipes() {
        let bucket_request = request::<3>(ProductShape::OrderedRangeBuckets {
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
        });
        let buckets = compile::<3>(&bucket_request).expect("buckets");
        assert_eq!(buckets.portfolio_template.denominator(), 4);
        assert_eq!(buckets.portfolio_template.coefficients(), &[0, 2, 3]);

        let tail_request = request::<2>(ProductShape::CrashTail {
            trigger: 30,
            payout: ExactAmount {
                numerator: 10,
                denominator: 2,
            },
        });
        let tail = compile::<2>(&tail_request).expect("tail");
        assert_eq!(tail.partition.cuts(), &[30]);
        assert_eq!(tail.portfolio_template.denominator(), 1);
        assert_eq!(tail.portfolio_template.coefficients(), &[5, 0]);
    }

    #[test]
    fn empty_and_graded_recipes_refuse_without_approximation() {
        let empty = request::<2>(ProductShape::OrderedRangeBuckets {
            cut_points: vec![50],
            payouts: vec![ExactAmount::ZERO, ExactAmount::ZERO],
        });
        assert_eq!(
            compile::<2>(&empty),
            Err(CompileError::Contract(
                ContractError::EmptyPortfolioTemplate
            ))
        );
        let cap = ExactAmount {
            numerator: 9,
            denominator: 2,
        };
        assert_eq!(
            compile::<3>(&request(ProductShape::CappedRamp {
                start: 20,
                end: 60,
                cap,
            })),
            Err(CompileError::UnsupportedWithinCellGradedShape)
        );
        assert_eq!(
            compile::<4>(&request(ProductShape::Tent {
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
            compile::<3>(&request(ProductShape::BinaryThreshold {
                threshold: 60,
                payout: ExactAmount {
                    numerator: 1,
                    denominator: 1,
                },
            })),
            Err(CompileError::OutcomeCountMismatch)
        );
        assert_eq!(
            compile::<2>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60],
                payouts: vec![ExactAmount {
                    numerator: 1,
                    denominator: 1,
                }],
            })),
            Err(CompileError::BucketCountMismatch)
        );
        assert_eq!(
            compile::<2>(&request(ProductShape::CrashTail {
                trigger: 30,
                payout: ExactAmount {
                    numerator: 1,
                    denominator: 0,
                },
            })),
            Err(CompileError::ZeroPayoutDenominator)
        );
    }

    #[test]
    fn noncanonical_partitions_and_exact_arithmetic_overflow_refuse() {
        assert_eq!(
            compile::<3>(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60, 60],
                payouts: vec![
                    ExactAmount {
                        numerator: 1,
                        denominator: 1,
                    };
                    3
                ],
            })),
            Err(CompileError::NonCanonicalPartition)
        );
        assert_eq!(
            compile::<2>(&request(ProductShape::OrderedRangeBuckets {
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
            })),
            Err(CompileError::ArithmeticOverflow)
        );
        assert_eq!(
            compile::<2>(&request(ProductShape::OrderedRangeBuckets {
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
            })),
            Err(CompileError::UnrepresentablePortfolioCoefficient)
        );
    }

    #[test]
    fn recheck_detects_partition_template_basis_and_certificate_substitution() {
        let request = request::<2>(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 7,
                denominator: 3,
            },
        });
        let mut changed = compile::<2>(&request).expect("compile");
        *changed.partition_bytes.last_mut().expect("partition cut") ^= 1;
        assert!(recheck(&request, &changed).is_err());

        let mut changed = compile::<2>(&request).expect("compile");
        *changed
            .portfolio_template_bytes
            .last_mut()
            .expect("template coefficient") ^= 1;
        assert!(recheck(&request, &changed).is_err());

        let mut changed = compile::<2>(&request).expect("compile");
        changed
            .portfolio_template_bytes
            .get_mut(48..56)
            .expect("denominator word")
            .copy_from_slice(&6u64.to_le_bytes());
        changed
            .portfolio_template_bytes
            .get_mut(64..72)
            .expect("winning coefficient word")
            .copy_from_slice(&14u64.to_le_bytes());
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::InvalidPortfolioTemplate)
        );

        let mut changed = compile::<2>(&request).expect("compile");
        changed.portfolio_template =
            PortfolioTemplateV1::new(id(88), [0, 7], 3).expect("foreign template");
        changed
            .portfolio_template
            .encode(&mut changed.portfolio_template_bytes)
            .expect("foreign encoding");
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::Contract(ContractError::IdentityMismatch))
        );

        let mut changed = compile::<2>(&request).expect("compile");
        changed.claim_basis = CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: CapacityProfileId::new(id(99)),
                outcome_count: 2,
            },
            request.context.capacity_profile,
        )
        .expect("alternate basis");
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::Contract(ContractError::IdentityMismatch))
        );

        let mut changed = compile::<2>(&request).expect("compile");
        changed.certificate.portfolio_template_id = id(98);
        assert_eq!(
            recheck(&request, &changed),
            Err(CompileError::CertificateMismatch)
        );
    }

    #[test]
    fn partition_decoder_refuses_noncanonical_bytes_and_trailing_data() {
        let partition = CanonicalPartition::new(
            ScaledDomain {
                lower: 0,
                upper: 10,
                denominator: 1,
            },
            vec![5],
        )
        .expect("partition");
        let bytes = partition.to_bytes().expect("encode");
        for length in 0..bytes.len() {
            assert!(
                CanonicalPartition::decode(bytes.get(..length).expect("in-bounds prefix")).is_err()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            CanonicalPartition::decode(&trailing),
            Err(CompileError::InvalidPartitionArtifact)
        );
        let mut reserved = bytes;
        *reserved.get_mut(10).expect("reserved byte") = 1;
        assert_eq!(
            CanonicalPartition::decode(&reserved),
            Err(CompileError::InvalidPartitionArtifact)
        );
    }
}
