#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Deterministic construction of bounded, exact dClutch Product artifacts.
//!
//! The compiler is deliberately a host-side constructor, not a payout VM. It
//! accepts only a small family of named univariate payoff shapes and emits the
//! fixed records defined by `dclutch-product-contract`. All coordinates and
//! payouts are rational scaled integers; no floating-point conversion occurs.
//!
//! The emitted partition is a finite, ordered partition of one closed bounded
//! coordinate domain. Cells are `[lower, upper)` except the final cell, which
//! is `[lower, upper]`. Coefficients are cell-major and degree-ascending and
//! are evaluated against the *scaled coordinate numerator*. Thus a coefficient
//! row `(a0, a1)` denotes `(a0 + a1 * coordinate_numerator) / denominator`.

use core::fmt;

use dclutch_product_contract::capacity::{CapacityProfileId, CapacityProfileV1, ExactWordWidth};
use dclutch_product_contract::claim::{
    ClaimBasisProfileV1, CoefficientDegree, FiniteExactV1, FiniteExactV1Input, RedemptionRounding,
};
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
    /// Signed numerator. The compiler rejects negative payout numerators.
    pub numerator: i128,
    /// Positive explicit denominator.
    pub denominator: u64,
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

/// The only product shapes accepted by this exact V1 compiler.
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
    ///
    /// `cut_points` must be strictly increasing interior domain points and
    /// `payouts.len()` must be exactly `cut_points.len() + 1`.
    OrderedRangeBuckets {
        /// Interior bucket boundaries.
        cut_points: Vec<i128>,
        /// Payout for every bucket in canonical order.
        payouts: Vec<ExactAmount>,
    },
    /// Pays `payout` in the lower crash tail through `trigger`, inclusively.
    CrashTail {
        /// Last coordinate numerator covered by the tail.
        trigger: i128,
        /// Fixed tail payout.
        payout: ExactAmount,
    },
    /// Zero below `start`, linear to `cap` at `end`, then capped at `cap`.
    CappedRamp {
        /// First coordinate numerator of the ramp.
        start: i128,
        /// First coordinate numerator at the cap.
        end: i128,
        /// Capped payout.
        cap: ExactAmount,
    },
    /// Zero outside the interval, rising to `cap` at `peak`, then falling.
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

/// Explicit policy at the one redemption rounding boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingPolicy {
    /// Refuse a non-integral redemption.
    ExactOnly,
    /// Round toward zero at redemption.
    FloorAtRedemption,
    /// Persist the remainder under this nonzero policy identity.
    CreditRemainder(ContentId),
}

/// Release identities and opaque occurrence bytes supplied by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationContext {
    /// Authenticated immutable capacity profile and its content identity.
    pub capacity_profile: CapacityProfileV1,
    /// Content identity of `capacity_profile` established at the caller's hash boundary.
    pub capacity_profile_id: CapacityProfileId,
    /// Release defining the named Terms semantics.
    pub terms_semantic_release_id: ContentId,
    /// Release defining finite exact payout evaluation.
    pub evaluator_release_id: ContentId,
    /// Release/profile defining signed coefficient word semantics.
    pub coefficient_profile_id: ContentId,
    /// Canonical occurrence-specific semantic bytes; must be nonempty.
    pub occurrence_artifact: Vec<u8>,
    /// The sole named redemption rounding boundary.
    pub rounding: RoundingPolicy,
}

/// A complete compiler request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequest {
    /// One finite scaled coordinate domain.
    pub domain: ScaledDomain,
    /// One supported named payoff shape.
    pub shape: ProductShape,
    /// Contract identities, capacity profile, occurrence bytes, and rounding.
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

    /// Return the underlying scaled coordinate domain.
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

/// Cell-major degree-ascending signed exact coefficient artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoefficientArtifact {
    /// Polynomial degree selected by the compiler.
    pub degree: CoefficientDegree,
    /// Positive common denominator of all entries.
    pub denominator: u64,
    /// Signed exact entries, cell-major then ascending degree.
    pub entries: Vec<i128>,
    /// Exact little-endian coefficient words emitted to the contract artifact.
    pub bytes: Vec<u8>,
}

/// A deterministic certificate binding source request and emitted artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCertificate {
    /// Certificate schema version.
    pub version: u16,
    /// Digest of the canonical source shape and coordinate domain.
    pub shape_commitment: ContentId,
    /// Digest of the canonical partition artifact.
    pub partition_id: ContentId,
    /// Digest of the exact coefficient artifact.
    pub coefficient_artifact_id: ContentId,
    /// Digest of the Terms preimage.
    pub terms_id: ContentId,
    /// Digest of the Occurrence preimage.
    pub occurrence_id: ContentId,
    /// Digest of the finite claim-basis preimage.
    pub claim_basis_id: ContentId,
    /// Digest of the Instance preimage.
    pub instance_id: ContentId,
}

/// All deterministic output artifacts and contract preimages from one request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledProduct {
    /// Canonical partition object.
    pub partition: CanonicalPartition,
    /// Canonical partition artifact bytes.
    pub partition_bytes: Vec<u8>,
    /// Exact finite coefficient artifact.
    pub coefficients: CoefficientArtifact,
    /// Terms preimage referring to the partition artifact.
    pub terms: TermsV1,
    /// Occurrence preimage referring to caller-provided occurrence bytes.
    pub occurrence: OccurrenceV1,
    /// Finite exact claim-basis preimage.
    pub claim_basis: FiniteExactV1,
    /// Product Instance preimage binding all three records.
    pub instance: InstanceV1,
    /// Independently recheckable compiler certificate.
    pub certificate: CompilerCertificate,
}

/// A precise refusal from exact construction or independent rechecking.
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
    /// A payout numerator was negative.
    NegativePayout,
    /// Bucket count and range-boundary count disagree.
    BucketCountMismatch,
    /// A named trigger or knot was not an appropriate interior coordinate.
    InvalidKnot,
    /// An exact integer intermediate or capacity count overflowed.
    ArithmeticOverflow,
    /// A collection length cannot be represented in the target contract.
    CountOverflow,
    /// A coefficient does not fit the selected contract word width.
    UnrepresentableCoefficient,
    /// The selected rounding policy is not valid for the exact denominator.
    InvalidRoundingPolicy,
    /// The caller omitted the required canonical occurrence bytes.
    EmptyOccurrenceArtifact,
    /// A partition artifact was malformed or noncanonical.
    InvalidPartitionArtifact,
    /// A coefficient artifact was malformed or did not match its declaration.
    InvalidCoefficientArtifact,
    /// A certificate field or artifact content identity did not match.
    CertificateMismatch,
    /// A certificate release is unsupported.
    UnsupportedCertificate,
    /// A contract preimage rejected an emitted declaration.
    Contract(ContractError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "exact Product compilation refused: {self:?}")
    }
}

impl std::error::Error for CompileError {}

impl From<ContractError> for CompileError {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}

/// Compile one supported exact Product shape into bounded contract artifacts.
pub fn compile(request: &CompileRequest) -> Result<CompiledProduct, CompileError> {
    if request.context.occurrence_artifact.is_empty() {
        return Err(CompileError::EmptyOccurrenceArtifact);
    }
    let (partition, degree, denominator, entries) = derive_shape(&request.domain, &request.shape)?;
    let partition_bytes = partition.to_bytes()?;
    let partition_id = content_id(b"dclutch.partition.v1", &partition_bytes)?;
    let partition_evidence_id = content_id(b"dclutch.partition-evidence.v1", &partition_bytes)?;
    let coefficient_bytes =
        encode_coefficients(&entries, request.context.capacity_profile.word_width())?;
    let coefficient_artifact_id = content_id(b"dclutch.coefficients.v1", &coefficient_bytes)?;
    let cell_count = partition.cell_count()?;
    let entry_count = u32::try_from(entries.len()).map_err(|_| CompileError::CountOverflow)?;
    let partition_size =
        u32::try_from(partition_bytes.len()).map_err(|_| CompileError::CountOverflow)?;
    let coefficient_size =
        u32::try_from(coefficient_bytes.len()).map_err(|_| CompileError::CountOverflow)?;
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
    let (rounding, fractional_credit_policy_id) =
        contract_rounding(request.context.rounding, denominator)?;
    let claim_basis = FiniteExactV1::new(
        FiniteExactV1Input {
            capacity_profile_id: request.context.capacity_profile_id,
            payout_artifact_id: coefficient_artifact_id,
            evaluator_release_id: request.context.evaluator_release_id,
            coefficient_profile_id: request.context.coefficient_profile_id,
            fractional_credit_policy_id,
            payout_denominator: denominator,
            coefficient_degree: degree,
            rounding,
            partition_cell_count: cell_count,
            coefficient_entry_count: entry_count,
            artifact_bytes: coefficient_size,
            page_count: pages(
                coefficient_size,
                request.context.capacity_profile.page_payload_bytes(),
            )?,
        },
        request.context.capacity_profile,
    )?;
    let claim_basis_id = content_id(b"dclutch.claim-basis.v1", &claim_basis.to_bytes())?;
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
        coefficient_artifact_id,
        terms_id,
        occurrence_id,
        claim_basis_id,
        instance_id,
    };
    let output = CompiledProduct {
        partition,
        partition_bytes,
        coefficients: CoefficientArtifact {
            degree,
            denominator,
            entries,
            bytes: coefficient_bytes,
        },
        terms,
        occurrence,
        claim_basis,
        instance,
        certificate,
    };
    recheck(request, &output)?;
    Ok(output)
}

/// Independently parse and recheck every emitted artifact and record binding.
///
/// This routine does not call [`compile`]. It reconstructs the expected named
/// shape, parses bytes, validates contract capacity/link rules, and compares
/// all content identities in the certificate.
pub fn recheck(request: &CompileRequest, output: &CompiledProduct) -> Result<(), CompileError> {
    if request.context.occurrence_artifact.is_empty() {
        return Err(CompileError::EmptyOccurrenceArtifact);
    }
    if output.certificate.version != CERTIFICATE_VERSION
        || output.certificate.shape_commitment != shape_commitment(&request.domain, &request.shape)?
    {
        return Err(CompileError::UnsupportedCertificate);
    }
    let parsed_partition = CanonicalPartition::decode(&output.partition_bytes)?;
    if parsed_partition != output.partition {
        return Err(CompileError::CertificateMismatch);
    }
    let (expected_partition, expected_degree, expected_denominator, expected_entries) =
        derive_shape(&request.domain, &request.shape)?;
    if parsed_partition != expected_partition
        || output.coefficients.degree != expected_degree
        || output.coefficients.denominator != expected_denominator
        || output.coefficients.entries != expected_entries
    {
        return Err(CompileError::CertificateMismatch);
    }
    let parsed_entries = decode_coefficients(
        &output.coefficients.bytes,
        request.context.capacity_profile.word_width(),
    )?;
    if parsed_entries != expected_entries
        || encode_coefficients(
            &parsed_entries,
            request.context.capacity_profile.word_width(),
        )? != output.coefficients.bytes
    {
        return Err(CompileError::InvalidCoefficientArtifact);
    }
    let partition_id = content_id(b"dclutch.partition.v1", &output.partition_bytes)?;
    let partition_evidence_id =
        content_id(b"dclutch.partition-evidence.v1", &output.partition_bytes)?;
    let coefficient_artifact_id =
        content_id(b"dclutch.coefficients.v1", &output.coefficients.bytes)?;
    let occurrence_artifact_id = content_id(
        b"dclutch.occurrence-artifact.v1",
        &request.context.occurrence_artifact,
    )?;
    let terms = TermsV1::decode(&output.terms.to_bytes())?;
    let occurrence = OccurrenceV1::decode(&output.occurrence.to_bytes())?;
    let claim_basis = FiniteExactV1::decode(&output.claim_basis.to_bytes())?;
    let instance = InstanceV1::decode(&output.instance.to_bytes())?;
    terms.validate_capacity(request.context.capacity_profile)?;
    occurrence.validate_capacity(request.context.capacity_profile)?;
    claim_basis.validate_capacity(request.context.capacity_profile)?;
    let terms_id = content_id(b"dclutch.terms.v1", &terms.to_bytes())?;
    let occurrence_id = content_id(b"dclutch.occurrence.v1", &occurrence.to_bytes())?;
    let claim_basis_id = content_id(b"dclutch.claim-basis.v1", &claim_basis.to_bytes())?;
    let instance_id = content_id(b"dclutch.instance.v1", &instance.to_bytes())?;
    instance.validate_occurrence(terms_id, terms, occurrence_id, occurrence)?;
    instance.validate_claim_basis(
        claim_basis_id,
        ClaimBasisProfileV1::FiniteExact(claim_basis),
    )?;
    if output.certificate.partition_id != partition_id
        || output.certificate.coefficient_artifact_id != coefficient_artifact_id
        || output.certificate.terms_id != terms_id
        || output.certificate.occurrence_id != occurrence_id
        || output.certificate.claim_basis_id != claim_basis_id
        || output.certificate.instance_id != instance_id
        || terms.artifact_id() != partition_id
        || terms.partition_evidence_id() != partition_evidence_id
        || claim_basis.payout_denominator() != expected_denominator
        || claim_basis.partition_cell_count() != parsed_partition.cell_count()?
        || occurrence.terms_id() != terms_id
        || occurrence.to_bytes().get(80..112) != Some(occurrence_artifact_id.as_bytes().as_slice())
    {
        return Err(CompileError::CertificateMismatch);
    }
    Ok(())
}

fn derive_shape(
    domain: &ScaledDomain,
    shape: &ProductShape,
) -> Result<(CanonicalPartition, CoefficientDegree, u64, Vec<i128>), CompileError> {
    match shape {
        ProductShape::BinaryThreshold { threshold, payout } => {
            interior(domain, *threshold)?;
            let partition = CanonicalPartition::new(*domain, vec![*threshold])?;
            let denominator = payout_denominator(&[*payout])?;
            Ok((
                partition,
                CoefficientDegree::Zero,
                denominator,
                vec![0, scaled(*payout, denominator)?],
            ))
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
            let partition = CanonicalPartition::new(*domain, cut_points.clone())?;
            let denominator = payout_denominator(payouts)?;
            let mut entries = Vec::with_capacity(payouts.len());
            for payout in payouts {
                entries.push(scaled(*payout, denominator)?);
            }
            Ok((partition, CoefficientDegree::Zero, denominator, entries))
        }
        ProductShape::CrashTail { trigger, payout } => {
            interior(domain, *trigger)?;
            let partition = CanonicalPartition::new(*domain, vec![*trigger])?;
            let denominator = payout_denominator(&[*payout])?;
            Ok((
                partition,
                CoefficientDegree::Zero,
                denominator,
                vec![scaled(*payout, denominator)?, 0],
            ))
        }
        ProductShape::CappedRamp { start, end, cap } => {
            ordered_knots(domain, &[*start, *end])?;
            validate_amount(*cap)?;
            let span = end
                .checked_sub(*start)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let denominator = multiply_u64(
                cap.denominator,
                u64::try_from(span).map_err(|_| CompileError::ArithmeticOverflow)?,
            )?;
            let partition = CanonicalPartition::new(*domain, vec![*start, *end])?;
            let cap_constant = cap
                .numerator
                .checked_mul(span)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let ramp_constant = cap
                .numerator
                .checked_mul(
                    start
                        .checked_neg()
                        .ok_or(CompileError::ArithmeticOverflow)?,
                )
                .ok_or(CompileError::ArithmeticOverflow)?;
            Ok((
                partition,
                CoefficientDegree::One,
                denominator,
                vec![0, 0, ramp_constant, cap.numerator, cap_constant, 0],
            ))
        }
        ProductShape::Tent {
            start,
            peak,
            end,
            cap,
        } => {
            ordered_knots(domain, &[*start, *peak, *end])?;
            validate_amount(*cap)?;
            let left = peak
                .checked_sub(*start)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let right = end
                .checked_sub(*peak)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let left_denominator = multiply_u64(
                cap.denominator,
                u64::try_from(left).map_err(|_| CompileError::ArithmeticOverflow)?,
            )?;
            let right_denominator = multiply_u64(
                cap.denominator,
                u64::try_from(right).map_err(|_| CompileError::ArithmeticOverflow)?,
            )?;
            let denominator = lcm(left_denominator, right_denominator)?;
            let left_scale = i128::from(
                denominator
                    .checked_div(left_denominator)
                    .ok_or(CompileError::ArithmeticOverflow)?,
            );
            let right_scale = i128::from(
                denominator
                    .checked_div(right_denominator)
                    .ok_or(CompileError::ArithmeticOverflow)?,
            );
            let left_slope = cap
                .numerator
                .checked_mul(left_scale)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let right_slope = cap
                .numerator
                .checked_mul(right_scale)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let left_constant = left_slope
                .checked_mul(
                    start
                        .checked_neg()
                        .ok_or(CompileError::ArithmeticOverflow)?,
                )
                .ok_or(CompileError::ArithmeticOverflow)?;
            let right_constant = right_slope
                .checked_mul(*end)
                .ok_or(CompileError::ArithmeticOverflow)?;
            let partition = CanonicalPartition::new(*domain, vec![*start, *peak, *end])?;
            Ok((
                partition,
                CoefficientDegree::One,
                denominator,
                vec![
                    0,
                    0,
                    left_constant,
                    left_slope,
                    right_constant,
                    right_slope
                        .checked_neg()
                        .ok_or(CompileError::ArithmeticOverflow)?,
                    0,
                    0,
                ],
            ))
        }
    }
}

fn validate_amount(amount: ExactAmount) -> Result<(), CompileError> {
    if amount.denominator == 0 {
        return Err(CompileError::ZeroPayoutDenominator);
    }
    if amount.numerator < 0 {
        return Err(CompileError::NegativePayout);
    }
    Ok(())
}

fn payout_denominator(amounts: &[ExactAmount]) -> Result<u64, CompileError> {
    if amounts.is_empty() {
        return Err(CompileError::PartitionTooSmall);
    }
    let mut denominator = 1;
    for amount in amounts {
        validate_amount(*amount)?;
        denominator = lcm(denominator, amount.denominator)?;
    }
    Ok(denominator)
}

fn scaled(amount: ExactAmount, denominator: u64) -> Result<i128, CompileError> {
    validate_amount(amount)?;
    let multiplier = denominator
        .checked_div(amount.denominator)
        .ok_or(CompileError::ArithmeticOverflow)?;
    amount
        .numerator
        .checked_mul(i128::from(multiplier))
        .ok_or(CompileError::ArithmeticOverflow)
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

fn ordered_knots(domain: &ScaledDomain, knots: &[i128]) -> Result<(), CompileError> {
    let mut prior = domain.lower;
    for knot in knots {
        interior(domain, *knot)?;
        if *knot <= prior {
            return Err(CompileError::InvalidKnot);
        }
        prior = *knot;
    }
    Ok(())
}

fn multiply_u64(left: u64, right: u64) -> Result<u64, CompileError> {
    left.checked_mul(right)
        .ok_or(CompileError::ArithmeticOverflow)
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

fn encode_coefficients(entries: &[i128], width: ExactWordWidth) -> Result<Vec<u8>, CompileError> {
    let word_bytes = usize::try_from(width.bytes()).map_err(|_| CompileError::CountOverflow)?;
    let capacity = entries
        .len()
        .checked_mul(word_bytes)
        .ok_or(CompileError::CountOverflow)?;
    let mut bytes = Vec::with_capacity(capacity);
    for entry in entries {
        match width {
            ExactWordWidth::Eight => bytes.extend_from_slice(
                &i64::try_from(*entry)
                    .map_err(|_| CompileError::UnrepresentableCoefficient)?
                    .to_le_bytes(),
            ),
            ExactWordWidth::Sixteen => bytes.extend_from_slice(&entry.to_le_bytes()),
        }
    }
    Ok(bytes)
}

fn decode_coefficients(bytes: &[u8], width: ExactWordWidth) -> Result<Vec<i128>, CompileError> {
    let word_bytes = usize::try_from(width.bytes()).map_err(|_| CompileError::CountOverflow)?;
    if bytes.is_empty() || !bytes.len().is_multiple_of(word_bytes) {
        return Err(CompileError::InvalidCoefficientArtifact);
    }
    let mut entries = Vec::with_capacity(bytes.len() / word_bytes);
    for word in bytes.chunks_exact(word_bytes) {
        let value = match width {
            ExactWordWidth::Eight => i128::from(i64::from_le_bytes(
                word.try_into()
                    .map_err(|_| CompileError::InvalidCoefficientArtifact)?,
            )),
            ExactWordWidth::Sixteen => i128::from_le_bytes(
                word.try_into()
                    .map_err(|_| CompileError::InvalidCoefficientArtifact)?,
            ),
        };
        entries.push(value);
    }
    Ok(entries)
}

fn contract_rounding(
    policy: RoundingPolicy,
    denominator: u64,
) -> Result<(RedemptionRounding, Option<ContentId>), CompileError> {
    let pair = match policy {
        RoundingPolicy::ExactOnly => (RedemptionRounding::ExactOnly, None),
        RoundingPolicy::FloorAtRedemption => (RedemptionRounding::FloorAtRedemption, None),
        RoundingPolicy::CreditRemainder(id) => (RedemptionRounding::CreditRemainder, Some(id)),
    };
    if denominator == 0
        || (denominator == 1 && pair.0 != RedemptionRounding::ExactOnly)
        || (pair.0 == RedemptionRounding::CreditRemainder && pair.1.is_none())
    {
        return Err(CompileError::InvalidRoundingPolicy);
    }
    Ok(pair)
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

    fn context(width: ExactWordWidth) -> CompilationContext {
        let profile = CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            word_width: width,
            verifier_release_id: id(1),
            envelope_basis_id: id(2),
            max_artifact_bytes: 4_096,
            page_payload_bytes: 128,
            max_pages: 32,
            max_partition_cells: 16,
            max_coefficient_entries: 64,
        })
        .expect("valid fixture profile");
        CompilationContext {
            capacity_profile: profile,
            capacity_profile_id: CapacityProfileId::new(id(3)),
            terms_semantic_release_id: id(4),
            evaluator_release_id: id(5),
            coefficient_profile_id: id(6),
            occurrence_artifact: b"occurrence-v1: fixture".to_vec(),
            rounding: RoundingPolicy::ExactOnly,
        }
    }

    fn request(shape: ProductShape) -> CompileRequest {
        CompileRequest {
            domain: ScaledDomain {
                lower: 0,
                upper: 100,
                denominator: 100,
            },
            shape,
            context: context(ExactWordWidth::Sixteen),
        }
    }

    #[test]
    fn golden_binary_threshold_is_canonical_and_recheckable() {
        let compiled = compile(&request(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 7,
                denominator: 3,
            },
        }))
        .expect("compile binary");
        assert_eq!(compiled.partition.cuts(), &[60]);
        assert_eq!(compiled.coefficients.entries, vec![0, 7]);
        assert_eq!(compiled.coefficients.denominator, 3);
        assert_eq!(compiled.partition_bytes.len(), 80);
        assert_eq!(compiled.coefficients.bytes.len(), 32);
        assert_eq!(compiled.terms.partition_cell_count(), 2);
        recheck(
            &request(ProductShape::BinaryThreshold {
                threshold: 60,
                payout: ExactAmount {
                    numerator: 7,
                    denominator: 3,
                },
            }),
            &compiled,
        )
        .expect("independent recheck");
    }

    #[test]
    fn golden_ramp_and_tent_have_exact_degree_one_coefficients() {
        let ramp = compile(&request(ProductShape::CappedRamp {
            start: 20,
            end: 60,
            cap: ExactAmount {
                numerator: 9,
                denominator: 2,
            },
        }))
        .expect("ramp");
        assert_eq!(ramp.coefficients.denominator, 80);
        assert_eq!(ramp.coefficients.entries, vec![0, 0, -180, 9, 360, 0]);
        let tent = compile(&request(ProductShape::Tent {
            start: 20,
            peak: 50,
            end: 80,
            cap: ExactAmount {
                numerator: 3,
                denominator: 2,
            },
        }))
        .expect("tent");
        assert_eq!(tent.coefficients.denominator, 60);
        assert_eq!(tent.coefficients.entries, vec![0, 0, -60, 3, 240, -3, 0, 0]);
    }

    #[test]
    fn ordered_buckets_and_crash_tail_compile_without_approximation() {
        let buckets = compile(&request(ProductShape::OrderedRangeBuckets {
            cut_points: vec![25, 75],
            payouts: vec![
                ExactAmount {
                    numerator: 0,
                    denominator: 1,
                },
                ExactAmount {
                    numerator: 1,
                    denominator: 2,
                },
                ExactAmount {
                    numerator: 3,
                    denominator: 4,
                },
            ],
        }))
        .expect("buckets");
        assert_eq!(buckets.coefficients.denominator, 4);
        assert_eq!(buckets.coefficients.entries, vec![0, 2, 3]);
        let tail = compile(&request(ProductShape::CrashTail {
            trigger: 30,
            payout: ExactAmount {
                numerator: 5,
                denominator: 1,
            },
        }))
        .expect("tail");
        assert_eq!(tail.coefficients.entries, vec![5, 0]);
    }

    #[test]
    fn rejects_overlap_missing_buckets_unrepresentable_and_invalid_rounding() {
        assert_eq!(
            compile(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60, 60],
                payouts: vec![
                    ExactAmount {
                        numerator: 1,
                        denominator: 1
                    };
                    3
                ],
            }))
            .expect_err("duplicate cuts"),
            CompileError::NonCanonicalPartition
        );
        assert_eq!(
            compile(&request(ProductShape::OrderedRangeBuckets {
                cut_points: vec![60],
                payouts: vec![ExactAmount {
                    numerator: 1,
                    denominator: 1
                }],
            }))
            .expect_err("missing bucket"),
            CompileError::BucketCountMismatch
        );
        let oversized = CompileRequest {
            context: context(ExactWordWidth::Eight),
            ..request(ProductShape::CappedRamp {
                start: 1,
                end: 2,
                cap: ExactAmount {
                    numerator: i128::from(i64::MAX) + 1,
                    denominator: 1,
                },
            })
        };
        assert_eq!(
            compile(&oversized).expect_err("i64 coefficient overflow"),
            CompileError::UnrepresentableCoefficient
        );
        let mut invalid_rounding = request(ProductShape::CrashTail {
            trigger: 30,
            payout: ExactAmount {
                numerator: 5,
                denominator: 1,
            },
        });
        invalid_rounding.context.rounding = RoundingPolicy::FloorAtRedemption;
        assert_eq!(
            compile(&invalid_rounding).expect_err("denominator one floor"),
            CompileError::InvalidRoundingPolicy
        );
    }

    #[test]
    fn recheck_detects_artifact_and_certificate_substitution() {
        let request = request(ProductShape::BinaryThreshold {
            threshold: 60,
            payout: ExactAmount {
                numerator: 7,
                denominator: 3,
            },
        });
        let mut compiled = compile(&request).expect("compile");
        *compiled
            .coefficients
            .bytes
            .get_mut(0)
            .expect("nonempty coefficient fixture") = 1;
        assert_eq!(
            recheck(&request, &compiled),
            Err(CompileError::InvalidCoefficientArtifact)
        );
        let mut compiled = compile(&request).expect("compile");
        compiled.certificate.terms_id = id(99);
        assert_eq!(
            recheck(&request, &compiled),
            Err(CompileError::CertificateMismatch)
        );
    }
}
