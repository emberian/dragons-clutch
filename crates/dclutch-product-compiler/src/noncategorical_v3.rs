//! Certified categorical projection of runtime-width noncategorical bases.
//!
//! The native authority is [`ProductBasisV3`]. This compiler authenticates its
//! acyclic semantic identity, full linked-record identity, and Product-owned
//! [`ResultDomainV2`] identity before projecting it. The categorical matrix is
//! sampled at a named exact boundary and carries a conservative componentwise
//! integer error bound. It is an approximation certificate, never an assertion
//! that the categorical matrix is the native continuous payoff.

use core::convert::{TryFrom, TryInto};

use dclutch_product_payoff_v2_codec::runtime_v3::{
    BasisKindV3, LINKED_BASIS_CONTENT_DOMAIN_V3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
    semantic_basis_preimage_v3,
};
use dclutch_product_runtime_v2::{ContentId, RESULT_DOMAIN_CONTENT_DOMAIN_V2, ResultDomainV2};
use sha2::{Digest, Sha256};

/// Exact compiler-certificate width.
pub const APPROXIMATION_CERTIFICATE_BYTES_V3: usize = 256;
/// Canonical compiler-certificate magic.
pub const APPROXIMATION_CERTIFICATE_MAGIC_V3: [u8; 8] = *b"DCLTAPX3";
/// Canonical compiler-certificate schema.
pub const APPROXIMATION_CERTIFICATE_SCHEMA_V3: u16 = 3;
/// Projection matrix content-hash domain.
pub const PROJECTION_MATRIX_CONTENT_DOMAIN_V3: &[u8] =
    b"dclutch/product/categorical-approximation-matrix/v3";

const BOUNDARY_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 11;
const BASIS_WIDTH_OFFSET: usize = 16;
const OUTCOME_COUNT_OFFSET: usize = 20;
const PAYOUT_SCALE_OFFSET: usize = 24;
const MAX_ERROR_OFFSET: usize = 32;
const PRODUCT_ID_OFFSET: usize = 40;
const RESULT_DOMAIN_ID_OFFSET: usize = 72;
const SEMANTIC_BASIS_ID_OFFSET: usize = 104;
const LINKED_BASIS_ID_OFFSET: usize = 136;
const EVALUATOR_RELEASE_ID_OFFSET: usize = 168;
const PROJECTION_DIGEST_OFFSET: usize = 200;
const RESERVED_TAIL_OFFSET: usize = 232;

/// Refusal from exact Product joins or categorical certification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Runtime Product or basis bytes were malformed.
    InvalidRecord,
    /// A supplied or derived identity was all zero.
    ZeroIdentifier,
    /// Product, result-domain, semantic-basis, or raw-record identities diverged.
    IdentityMismatch,
    /// Only a graded exact-complement basis can use this projection.
    UnsupportedBasis,
    /// A Product-owned basis knot was omitted from the categorical partition.
    KnotMissing,
    /// A runtime count, matrix, or caller buffer had the wrong exact width.
    WidthMismatch,
    /// A certificate selected another schema or projection boundary.
    UnsupportedCertificate,
    /// Reserved certificate bytes were nonzero.
    NonCanonicalCertificate,
    /// Checked sizing, payout, error, or hash input arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for the runtime noncategorical compiler.
pub type Result<T> = core::result::Result<T, Error>;

/// Sole categorical projection boundary for this successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategoricalProjectionBoundaryV3 {
    /// Sample finite regions at their exact left cut and clamp both outer tails.
    ///
    /// All Product-owned curve knots must be explicit cuts. Every term is then
    /// monotone or constant inside a cell. Endpoint variation gives an exact,
    /// conservative integer error certificate without midpoint overflow or
    /// coordinate quantization.
    LeftCutClampedTails,
}

impl CategoricalProjectionBoundaryV3 {
    const fn tag(self) -> u8 {
        match self {
            Self::LeftCutClampedTails => 1,
        }
    }

    fn decode(tag: u8) -> Result<Self> {
        match tag {
            1 => Ok(Self::LeftCutClampedTails),
            _ => Err(Error::UnsupportedCertificate),
        }
    }
}

/// Fixed-width independently recheckable approximation certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalApproximationCertificateV3 {
    /// Sole named sampling boundary.
    pub boundary: CategoricalProjectionBoundaryV3,
    /// Runtime noncategorical basis width.
    pub basis_width: u32,
    /// Runtime categorical terminal width, including explicit failure.
    pub outcome_count: u32,
    /// Exact native basis payout scale `Q`.
    pub payout_scale: u64,
    /// Conservative L-infinity component error in collateral atoms.
    pub max_component_error_atoms: u64,
    /// Stable Product semantic identity.
    pub product_id: [u8; 32],
    /// Exact Product-owned result-domain raw-record identity.
    pub result_domain_id: [u8; 32],
    /// Acyclic semantic basis identity selected by Product.
    pub semantic_basis_id: [u8; 32],
    /// Full linked basis raw-record identity.
    pub linked_basis_record_id: [u8; 32],
    /// Exact native evaluator semantic release.
    pub evaluator_release_id: [u8; 32],
    /// Hash of ordered payouts followed by ordered component error bounds.
    pub projection_digest: [u8; 32],
}

impl CategoricalApproximationCertificateV3 {
    /// Hostile-decode the sole exact canonical certificate representation.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != APPROXIMATION_CERTIFICATE_BYTES_V3 {
            return Err(Error::WidthMismatch);
        }
        if array::<8>(bytes, 0)? != APPROXIMATION_CERTIFICATE_MAGIC_V3 {
            return Err(Error::UnsupportedCertificate);
        }
        if read_u16(bytes, 8)? != APPROXIMATION_CERTIFICATE_SCHEMA_V3 {
            return Err(Error::UnsupportedCertificate);
        }
        require_zero(bytes, RESERVED_OFFSET, 5)?;
        require_zero(bytes, RESERVED_TAIL_OFFSET, 24)?;
        let value = Self {
            boundary: CategoricalProjectionBoundaryV3::decode(byte(bytes, BOUNDARY_OFFSET)?)?,
            basis_width: read_u32(bytes, BASIS_WIDTH_OFFSET)?,
            outcome_count: read_u32(bytes, OUTCOME_COUNT_OFFSET)?,
            payout_scale: read_u64(bytes, PAYOUT_SCALE_OFFSET)?,
            max_component_error_atoms: read_u64(bytes, MAX_ERROR_OFFSET)?,
            product_id: nonzero_id(bytes, PRODUCT_ID_OFFSET)?,
            result_domain_id: nonzero_id(bytes, RESULT_DOMAIN_ID_OFFSET)?,
            semantic_basis_id: nonzero_id(bytes, SEMANTIC_BASIS_ID_OFFSET)?,
            linked_basis_record_id: nonzero_id(bytes, LINKED_BASIS_ID_OFFSET)?,
            evaluator_release_id: nonzero_id(bytes, EVALUATOR_RELEASE_ID_OFFSET)?,
            projection_digest: nonzero_id(bytes, PROJECTION_DIGEST_OFFSET)?,
        };
        if value.basis_width < 2 || value.outcome_count < 2 || value.payout_scale == 0 {
            return Err(Error::NonCanonicalCertificate);
        }
        Ok(value)
    }

    /// Encode the sole exact canonical certificate representation.
    pub fn to_bytes(self) -> [u8; APPROXIMATION_CERTIFICATE_BYTES_V3] {
        let mut output = [0_u8; APPROXIMATION_CERTIFICATE_BYTES_V3];
        put(&mut output, 0, &APPROXIMATION_CERTIFICATE_MAGIC_V3);
        put(
            &mut output,
            8,
            &APPROXIMATION_CERTIFICATE_SCHEMA_V3.to_le_bytes(),
        );
        put(&mut output, BOUNDARY_OFFSET, &[self.boundary.tag()]);
        put(
            &mut output,
            BASIS_WIDTH_OFFSET,
            &self.basis_width.to_le_bytes(),
        );
        put(
            &mut output,
            OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        );
        put(
            &mut output,
            PAYOUT_SCALE_OFFSET,
            &self.payout_scale.to_le_bytes(),
        );
        put(
            &mut output,
            MAX_ERROR_OFFSET,
            &self.max_component_error_atoms.to_le_bytes(),
        );
        put(&mut output, PRODUCT_ID_OFFSET, &self.product_id);
        put(&mut output, RESULT_DOMAIN_ID_OFFSET, &self.result_domain_id);
        put(
            &mut output,
            SEMANTIC_BASIS_ID_OFFSET,
            &self.semantic_basis_id,
        );
        put(
            &mut output,
            LINKED_BASIS_ID_OFFSET,
            &self.linked_basis_record_id,
        );
        put(
            &mut output,
            EVALUATOR_RELEASE_ID_OFFSET,
            &self.evaluator_release_id,
        );
        put(
            &mut output,
            PROJECTION_DIGEST_OFFSET,
            &self.projection_digest,
        );
        output
    }
}

/// Complete claim-major categorical matrix and its componentwise error evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedCategoricalApproximationV3 {
    /// Claim-major `[claim][outcome]` integer categorical payouts.
    pub payouts: Vec<u64>,
    /// Claim-major conservative absolute error bound in payout atoms.
    pub component_error_bounds: Vec<u64>,
    /// Fixed-width identity and maximum-error certificate.
    pub certificate: CategoricalApproximationCertificateV3,
}

impl CertifiedCategoricalApproximationV3 {
    /// Read one projected payout without accepting an unchecked index.
    pub fn payout(&self, claim: u32, outcome: u32) -> Option<u64> {
        matrix_index(
            self.certificate.basis_width,
            self.certificate.outcome_count,
            claim,
            outcome,
        )
        .ok()
        .and_then(|index| self.payouts.get(index).copied())
    }

    /// Read one certified component error bound.
    pub fn component_error_bound(&self, claim: u32, outcome: u32) -> Option<u64> {
        matrix_index(
            self.certificate.basis_width,
            self.certificate.outcome_count,
            claim,
            outcome,
        )
        .ok()
        .and_then(|index| self.component_error_bounds.get(index).copied())
    }
}

/// Authenticate and project one graded runtime basis over one Product domain.
pub fn certify_categorical_approximation_v3(
    result_domain_bytes: &[u8],
    linked_basis_record_bytes: &[u8],
    boundary: CategoricalProjectionBoundaryV3,
) -> Result<CertifiedCategoricalApproximationV3> {
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(|_| Error::InvalidRecord)?;
    let basis =
        ProductBasisV3::decode(linked_basis_record_bytes).map_err(|_| Error::InvalidRecord)?;
    if basis.kind() != BasisKindV3::GradedExactComplement {
        return Err(Error::UnsupportedBasis);
    }
    let result_domain_id = raw_digest(result_domain_bytes)?;
    let linked_basis_record_id = raw_digest(linked_basis_record_bytes)?;
    let semantic =
        semantic_basis_preimage_v3(linked_basis_record_bytes).map_err(|_| Error::InvalidRecord)?;
    let semantic_basis_id = digest_fragments(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])?;
    if domain.product_id().to_bytes() != basis.product_id()
        || domain.coordinate_domain_id().to_bytes() != basis.coordinate_domain_id()
        || domain.result_unit_id().to_bytes() != basis.result_unit_id()
        || domain.liability_basis_id().to_bytes() != semantic_basis_id
        || basis.result_domain_id() != result_domain_id
    {
        return Err(Error::IdentityMismatch);
    }
    // Domain-separated aliases are published for adapters; finalized Registry
    // identity remains SHA-256 over exact raw bytes.
    let _ = (
        RESULT_DOMAIN_CONTENT_DOMAIN_V2,
        LINKED_BASIS_CONTENT_DOMAIN_V3,
    );
    let cuts: Vec<i128> = domain.cuts().collect();
    for knot in basis.knots() {
        if cuts.binary_search(&knot).is_err() {
            return Err(Error::KnotMissing);
        }
    }
    let basis_width = basis.basis_width();
    let outcome_count = domain
        .outcome_count()
        .map_err(|_| Error::ArithmeticOverflow)?;
    let matrix_len = usize::try_from(basis_width)
        .map_err(|_| Error::WidthMismatch)?
        .checked_mul(usize::try_from(outcome_count).map_err(|_| Error::WidthMismatch)?)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut payouts = vec![0_u64; matrix_len];
    let mut component_error_bounds = vec![0_u64; matrix_len];
    let mut scratch = vec![0_u64; usize::try_from(basis_width).map_err(|_| Error::WidthMismatch)?];
    let region_count = domain.region_count();
    let denominator = domain.cut_denominator();
    for region in 0..region_count {
        let sample = sample_for_region(&cuts, region)?;
        basis
            .evaluate_rational(sample, denominator, &mut scratch)
            .map_err(|_| Error::ArithmeticOverflow)?;
        write_column(&mut payouts, basis_width, outcome_count, region, &scratch)?;
    }
    basis
        .evaluate_failure(&mut scratch)
        .map_err(|_| Error::ArithmeticOverflow)?;
    write_column(
        &mut payouts,
        basis_width,
        outcome_count,
        domain.failure_selector(),
        &scratch,
    )?;

    // Only regions with two finite cuts can vary. Every curve knot is a cut,
    // so every term is monotone or constant throughout each such cell.
    let finite_cell_count = cuts.len().saturating_sub(1);
    for finite_index in 0..finite_cell_count {
        let left = *cuts.get(finite_index).ok_or(Error::WidthMismatch)?;
        let right = *cuts
            .get(
                finite_index
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::WidthMismatch)?;
        let region = u32::try_from(finite_index)
            .map_err(|_| Error::WidthMismatch)?
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut complement_error = 0_u64;
        for term_index in 0..basis.term_count() {
            let (left_claim, left_payout) = basis
                .evaluate_term_rational(term_index, left, denominator)
                .map_err(|_| Error::ArithmeticOverflow)?;
            let (right_claim, right_payout) = basis
                .evaluate_term_rational(term_index, right, denominator)
                .map_err(|_| Error::ArithmeticOverflow)?;
            if left_claim != right_claim {
                return Err(Error::InvalidRecord);
            }
            let variation = left_payout.abs_diff(right_payout);
            let index = matrix_index(basis_width, outcome_count, left_claim, region)?;
            let bound = component_error_bounds
                .get_mut(index)
                .ok_or(Error::WidthMismatch)?;
            *bound = bound
                .checked_add(variation)
                .ok_or(Error::ArithmeticOverflow)?;
            complement_error = complement_error
                .checked_add(variation)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let complement = basis_width.checked_sub(1).ok_or(Error::WidthMismatch)?;
        let complement_index = matrix_index(basis_width, outcome_count, complement, region)?;
        *component_error_bounds
            .get_mut(complement_index)
            .ok_or(Error::WidthMismatch)? = complement_error;
    }
    validate_projection(&payouts, basis_width, outcome_count, basis.payout_scale())?;
    let max_component_error_atoms = component_error_bounds.iter().copied().max().unwrap_or(0);
    let projection_digest = projection_digest_v3(
        basis_width,
        outcome_count,
        &payouts,
        &component_error_bounds,
    )?;
    let certificate = CategoricalApproximationCertificateV3 {
        boundary,
        basis_width,
        outcome_count,
        payout_scale: basis.payout_scale(),
        max_component_error_atoms,
        product_id: basis.product_id(),
        result_domain_id,
        semantic_basis_id,
        linked_basis_record_id,
        evaluator_release_id: basis.evaluator_release_id(),
        projection_digest,
    };
    let _ = CategoricalApproximationCertificateV3::decode(&certificate.to_bytes())?;
    Ok(CertifiedCategoricalApproximationV3 {
        payouts,
        component_error_bounds,
        certificate,
    })
}

/// Recompute every join, payout, error bound, and digest in a certificate.
pub fn recheck_categorical_approximation_v3(
    result_domain_bytes: &[u8],
    linked_basis_record_bytes: &[u8],
    certificate_bytes: &[u8],
    payouts: &[u64],
    component_error_bounds: &[u64],
) -> Result<()> {
    let expected = CategoricalApproximationCertificateV3::decode(certificate_bytes)?;
    let recomputed = certify_categorical_approximation_v3(
        result_domain_bytes,
        linked_basis_record_bytes,
        expected.boundary,
    )?;
    if recomputed.certificate != expected
        || recomputed.payouts != payouts
        || recomputed.component_error_bounds != component_error_bounds
    {
        return Err(Error::IdentityMismatch);
    }
    Ok(())
}

fn sample_for_region(cuts: &[i128], region: u32) -> Result<i128> {
    if cuts.is_empty() {
        return Ok(0);
    }
    if region == 0 {
        return cuts.first().copied().ok_or(Error::WidthMismatch);
    }
    let index = usize::try_from(region.checked_sub(1).ok_or(Error::WidthMismatch)?)
        .map_err(|_| Error::WidthMismatch)?;
    cuts.get(index).copied().ok_or(Error::WidthMismatch)
}

fn write_column(
    matrix: &mut [u64],
    basis_width: u32,
    outcome_count: u32,
    outcome: u32,
    column: &[u64],
) -> Result<()> {
    if column.len() != usize::try_from(basis_width).map_err(|_| Error::WidthMismatch)? {
        return Err(Error::WidthMismatch);
    }
    for (claim, payout) in column.iter().copied().enumerate() {
        let index = matrix_index(
            basis_width,
            outcome_count,
            u32::try_from(claim).map_err(|_| Error::WidthMismatch)?,
            outcome,
        )?;
        *matrix.get_mut(index).ok_or(Error::WidthMismatch)? = payout;
    }
    Ok(())
}

fn validate_projection(
    payouts: &[u64],
    basis_width: u32,
    outcome_count: u32,
    scale: u64,
) -> Result<()> {
    for outcome in 0..outcome_count {
        let mut total = 0_u64;
        for claim in 0..basis_width {
            total = total
                .checked_add(
                    *payouts
                        .get(matrix_index(basis_width, outcome_count, claim, outcome)?)
                        .ok_or(Error::WidthMismatch)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if total != scale {
            return Err(Error::InvalidRecord);
        }
    }
    Ok(())
}

fn matrix_index(basis_width: u32, outcome_count: u32, claim: u32, outcome: u32) -> Result<usize> {
    if claim >= basis_width || outcome >= outcome_count {
        return Err(Error::WidthMismatch);
    }
    usize::try_from(claim)
        .map_err(|_| Error::WidthMismatch)?
        .checked_mul(usize::try_from(outcome_count).map_err(|_| Error::WidthMismatch)?)
        .and_then(|offset| offset.checked_add(usize::try_from(outcome).ok()?))
        .ok_or(Error::ArithmeticOverflow)
}

fn projection_digest_v3(
    basis_width: u32,
    outcome_count: u32,
    payouts: &[u64],
    component_error_bounds: &[u64],
) -> Result<[u8; 32]> {
    if payouts.len() != component_error_bounds.len() {
        return Err(Error::WidthMismatch);
    }
    let mut hasher = Sha256::new();
    hasher.update(PROJECTION_MATRIX_CONTENT_DOMAIN_V3);
    hasher.update(basis_width.to_le_bytes());
    hasher.update(outcome_count.to_le_bytes());
    for payout in payouts {
        hasher.update(payout.to_le_bytes());
    }
    for bound in component_error_bounds {
        hasher.update(bound.to_le_bytes());
    }
    nonzero_digest(hasher.finalize().into())
}

fn raw_digest(bytes: &[u8]) -> Result<[u8; 32]> {
    nonzero_digest(Sha256::digest(bytes).into())
}

fn digest_fragments(fragments: &[&[u8]]) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    for fragment in fragments {
        hasher.update(fragment);
    }
    nonzero_digest(hasher.finalize().into())
}

fn nonzero_digest(value: [u8; 32]) -> Result<[u8; 32]> {
    ContentId::new(value)
        .map(|identity| identity.to_bytes())
        .map_err(|_| Error::ZeroIdentifier)
}

fn nonzero_id(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array(bytes, offset)?;
    if value.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(value)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::WidthMismatch)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::WidthMismatch)?;
    bytes
        .get(offset..end)
        .ok_or(Error::WidthMismatch)?
        .try_into()
        .map_err(|_| Error::WidthMismatch)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::WidthMismatch)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::WidthMismatch)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalCertificate);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisShapeV3, BasisTermV3, basis_record_bytes_v3, compile_basis_v3,
    };
    use dclutch_product_runtime_v2::{
        ResultDomainInputV2, compile_result_domain_v2, result_domain_record_bytes,
    };

    fn id(fill: u8) -> [u8; 32] {
        [fill; 32]
    }

    fn content(fill: u8) -> ContentId {
        ContentId::new(id(fill)).expect("identity")
    }

    struct Fixture {
        domain: Vec<u8>,
        basis: Vec<u8>,
    }

    fn fixture(cuts: &[i128]) -> Fixture {
        let product_id = id(1);
        let coordinate_domain_id = id(2);
        let result_unit_id = id(3);
        let evaluator_release_id = id(4);
        let placeholder_domain = id(9);
        let knots = [0, 10, 20];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 2 },
                amplitude: 60,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::Tent {
                    left: 0,
                    peak: 1,
                    right: 2,
                },
                amplitude: 30,
            },
        ];
        let input = BasisInputV3 {
            kind: BasisKindV3::GradedExactComplement,
            product_id,
            result_domain_id: placeholder_domain,
            coordinate_domain_id,
            result_unit_id,
            evaluator_release_id,
            basis_width: 3,
            payout_scale: 100,
            knot_denominator: 1,
            knots: &knots,
            terms: &terms,
            failure_payouts: &[1, 2, 97],
        };
        let basis_width =
            basis_record_bytes_v3(input.kind, 3, knots.len(), terms.len()).expect("basis width");
        let mut provisional_basis = vec![0; basis_width];
        compile_basis_v3(input, &mut provisional_basis).expect("provisional basis");
        let semantic = semantic_basis_preimage_v3(&provisional_basis).expect("semantic");
        let semantic_basis_id = digest_fragments(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .expect("semantic digest");
        let mut domain = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id: ContentId::new(product_id).expect("product"),
                coordinate_domain_id: ContentId::new(coordinate_domain_id).expect("coordinate"),
                result_unit_id: ContentId::new(result_unit_id).expect("unit"),
                liability_basis_id: ContentId::new(semantic_basis_id).expect("basis"),
                representation_release_id: content(6),
                mapping_release_id: content(7),
                cut_denominator: 1,
                cuts,
            },
            &mut domain,
        )
        .expect("domain");
        let result_domain_id = raw_digest(&domain).expect("domain digest");
        let mut basis = vec![0; basis_width];
        compile_basis_v3(
            BasisInputV3 {
                result_domain_id,
                ..input
            },
            &mut basis,
        )
        .expect("linked basis");
        Fixture { domain, basis }
    }

    #[test]
    fn exact_product_join_emits_partitioned_matrix_and_error_certificate() {
        let fixture = fixture(&[-10, 0, 5, 10, 15, 20, 30]);
        let approximation = certify_categorical_approximation_v3(
            &fixture.domain,
            &fixture.basis,
            CategoricalProjectionBoundaryV3::LeftCutClampedTails,
        )
        .expect("projection");
        assert_eq!(approximation.certificate.basis_width, 3);
        assert_eq!(approximation.certificate.outcome_count, 9);
        assert!(approximation.certificate.max_component_error_atoms > 0);
        for outcome in 0..approximation.certificate.outcome_count {
            let total = (0..approximation.certificate.basis_width)
                .map(|claim| approximation.payout(claim, outcome).expect("payout"))
                .sum::<u64>();
            assert_eq!(total, 100);
        }
        let certificate = approximation.certificate.to_bytes();
        assert_eq!(
            CategoricalApproximationCertificateV3::decode(&certificate),
            Ok(approximation.certificate)
        );
        recheck_categorical_approximation_v3(
            &fixture.domain,
            &fixture.basis,
            &certificate,
            &approximation.payouts,
            &approximation.component_error_bounds,
        )
        .expect("independent recheck");
    }

    #[test]
    fn runtime_domain_width_258_has_no_compiler_profile_ceiling() {
        let cuts: Vec<i128> = (-128..128).map(i128::from).collect();
        let fixture = fixture(&cuts);
        let approximation = certify_categorical_approximation_v3(
            &fixture.domain,
            &fixture.basis,
            CategoricalProjectionBoundaryV3::LeftCutClampedTails,
        )
        .expect("258 outcome projection");
        assert_eq!(approximation.certificate.outcome_count, 258);
        assert_eq!(approximation.payouts.len(), 774);
    }

    #[test]
    fn missing_knot_and_all_identity_substitutions_refuse() {
        let missing = fixture(&[-10, 0, 5, 15, 20, 30]);
        assert_eq!(
            certify_categorical_approximation_v3(
                &missing.domain,
                &missing.basis,
                CategoricalProjectionBoundaryV3::LeftCutClampedTails,
            ),
            Err(Error::KnotMissing)
        );

        let fixture = fixture(&[-10, 0, 10, 20, 30]);
        for offset in [32_usize, 64, 96, 128, 176, fixture.basis.len() - 1] {
            let mut hostile = fixture.basis.clone();
            *hostile.get_mut(offset).expect("offset") ^= 1;
            assert!(
                certify_categorical_approximation_v3(
                    &fixture.domain,
                    &hostile,
                    CategoricalProjectionBoundaryV3::LeftCutClampedTails,
                )
                .is_err(),
                "basis offset {offset}"
            );
        }
        let mut substituted_domain = fixture.domain.clone();
        *substituted_domain.get_mut(32).expect("product id") ^= 1;
        assert_eq!(
            certify_categorical_approximation_v3(
                &substituted_domain,
                &fixture.basis,
                CategoricalProjectionBoundaryV3::LeftCutClampedTails,
            ),
            Err(Error::IdentityMismatch)
        );
    }

    #[test]
    fn certificate_and_projection_tampering_refuse() {
        let fixture = fixture(&[-10, 0, 10, 20, 30]);
        let approximation = certify_categorical_approximation_v3(
            &fixture.domain,
            &fixture.basis,
            CategoricalProjectionBoundaryV3::LeftCutClampedTails,
        )
        .expect("projection");
        let certificate = approximation.certificate.to_bytes();
        for offset in [0, 8, 11, 255] {
            let mut hostile = certificate;
            *hostile.get_mut(offset).expect("offset") ^= 1;
            assert!(CategoricalApproximationCertificateV3::decode(&hostile).is_err());
        }
        for offset in [40, 72, 104, 136, 168, 200] {
            let mut substituted = certificate;
            *substituted.get_mut(offset).expect("offset") ^= 1;
            assert!(CategoricalApproximationCertificateV3::decode(&substituted).is_ok());
            assert_eq!(
                recheck_categorical_approximation_v3(
                    &fixture.domain,
                    &fixture.basis,
                    &substituted,
                    &approximation.payouts,
                    &approximation.component_error_bounds,
                ),
                Err(Error::IdentityMismatch)
            );
        }
        let mut hostile_payouts = approximation.payouts.clone();
        *hostile_payouts.get_mut(0).expect("payout") ^= 1;
        assert_eq!(
            recheck_categorical_approximation_v3(
                &fixture.domain,
                &fixture.basis,
                &certificate,
                &hostile_payouts,
                &approximation.component_error_bounds,
            ),
            Err(Error::IdentityMismatch)
        );
        let mut hostile_bounds = approximation.component_error_bounds.clone();
        *hostile_bounds.get_mut(4).expect("bound") ^= 1;
        assert_eq!(
            recheck_categorical_approximation_v3(
                &fixture.domain,
                &fixture.basis,
                &certificate,
                &approximation.payouts,
                &hostile_bounds,
            ),
            Err(Error::IdentityMismatch)
        );
    }
}
