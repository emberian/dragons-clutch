//! Registry-finalized runtime graded-basis admission.
//!
//! The linked basis, the 256-byte projection/error certificate, and the
//! 304-byte admission binding are three distinct finalized records.  This
//! module hostile-decodes all three bodies and recomputes their exact raw
//! digests and semantic joins.  Registry ownership, raw/staging PDAs, rent,
//! and finalization remain adapter obligations.

use core::convert::TryInto;

use dclutch_product_runtime_v2::ResultDomainV2;
use sha2::{Digest, Sha256};

use crate::runtime_v3::{
    BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3,
};

#[allow(missing_docs)]
mod generated {
    include!("generated_admission_v3.rs");
}

pub use generated::*;

/// Canonical projection-certificate schema version.
pub const APPROXIMATION_CERTIFICATE_SCHEMA_V3: u16 = GRADED_BASIS_REGISTRY_SCHEMA_VERSION_V3;

/// Refusal from a Registry-finalized graded-basis record join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed record had the wrong exact width.
    InvalidLength,
    /// Magic, version, or projection boundary selected another schema.
    UnsupportedSchema,
    /// Reserved bytes or a required count were noncanonical.
    NonCanonical,
    /// A required content identity was all zero.
    ZeroIdentifier,
    /// Runtime Product, result-domain, or basis decoding refused.
    InvalidRecord,
    /// Exact Product/domain/basis/certificate identities diverged.
    IdentityMismatch,
    /// Checked digest input or byte offset arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for Registry-finalized graded-basis admission.
pub type Result<T> = core::result::Result<T, Error>;

/// Sole categorical projection boundary admitted by V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategoricalProjectionBoundaryV3 {
    /// Sample exact left cuts and clamp both outer tails.
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
            _ => Err(Error::UnsupportedSchema),
        }
    }
}

/// Fixed-width independently recheckable categorical projection certificate.
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
    /// Exact Product-owned ResultDomainV2 raw-record digest.
    pub result_domain_id: [u8; 32],
    /// Acyclic semantic basis identity selected by Product.
    pub semantic_basis_id: [u8; 32],
    /// Full linked basis raw-record digest.
    pub linked_basis_record_id: [u8; 32],
    /// Exact native evaluator semantic release.
    pub evaluator_release_id: [u8; 32],
    /// Hash of ordered payouts followed by ordered component error bounds.
    pub projection_digest: [u8; 32],
}

impl CategoricalApproximationCertificateV3 {
    /// Hostile-decode the sole exact certificate representation.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != APPROXIMATION_CERTIFICATE_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, APPROXIMATION_CERTIFICATE_MAGIC_OFFSET_V3)?
            != APPROXIMATION_CERTIFICATE_MAGIC_V3
            || read_u16(bytes, APPROXIMATION_CERTIFICATE_VERSION_OFFSET_V3)?
                != GRADED_BASIS_REGISTRY_SCHEMA_VERSION_V3
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            APPROXIMATION_CERTIFICATE_HEADER_RESERVED_OFFSET_V3,
            5,
        )?;
        require_zero(bytes, APPROXIMATION_CERTIFICATE_TAIL_RESERVED_OFFSET_V3, 24)?;
        let value = Self {
            boundary: CategoricalProjectionBoundaryV3::decode(byte(
                bytes,
                APPROXIMATION_CERTIFICATE_BOUNDARY_OFFSET_V3,
            )?)?,
            basis_width: read_u32(bytes, APPROXIMATION_CERTIFICATE_BASIS_WIDTH_OFFSET_V3)?,
            outcome_count: read_u32(bytes, APPROXIMATION_CERTIFICATE_OUTCOME_COUNT_OFFSET_V3)?,
            payout_scale: read_u64(bytes, APPROXIMATION_CERTIFICATE_PAYOUT_SCALE_OFFSET_V3)?,
            max_component_error_atoms: read_u64(
                bytes,
                APPROXIMATION_CERTIFICATE_MAX_ERROR_OFFSET_V3,
            )?,
            product_id: nonzero_id(bytes, APPROXIMATION_CERTIFICATE_PRODUCT_OFFSET_V3)?,
            result_domain_id: nonzero_id(bytes, APPROXIMATION_CERTIFICATE_RESULT_DOMAIN_OFFSET_V3)?,
            semantic_basis_id: nonzero_id(
                bytes,
                APPROXIMATION_CERTIFICATE_SEMANTIC_BASIS_OFFSET_V3,
            )?,
            linked_basis_record_id: nonzero_id(
                bytes,
                APPROXIMATION_CERTIFICATE_LINKED_BASIS_OFFSET_V3,
            )?,
            evaluator_release_id: nonzero_id(
                bytes,
                APPROXIMATION_CERTIFICATE_EVALUATOR_RELEASE_OFFSET_V3,
            )?,
            projection_digest: nonzero_id(
                bytes,
                APPROXIMATION_CERTIFICATE_PROJECTION_DIGEST_OFFSET_V3,
            )?,
        };
        if value.basis_width < 2 || value.outcome_count < 2 || value.payout_scale == 0 {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }

    /// Encode the sole exact certificate representation.
    pub fn to_bytes(self) -> [u8; APPROXIMATION_CERTIFICATE_BYTES_V3] {
        let mut output = [0_u8; APPROXIMATION_CERTIFICATE_BYTES_V3];
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_MAGIC_OFFSET_V3,
            &APPROXIMATION_CERTIFICATE_MAGIC_V3,
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_VERSION_OFFSET_V3,
            &GRADED_BASIS_REGISTRY_SCHEMA_VERSION_V3.to_le_bytes(),
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_BOUNDARY_OFFSET_V3,
            &[self.boundary.tag()],
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_BASIS_WIDTH_OFFSET_V3,
            &self.basis_width.to_le_bytes(),
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_OUTCOME_COUNT_OFFSET_V3,
            &self.outcome_count.to_le_bytes(),
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_PAYOUT_SCALE_OFFSET_V3,
            &self.payout_scale.to_le_bytes(),
        );
        put(
            &mut output,
            APPROXIMATION_CERTIFICATE_MAX_ERROR_OFFSET_V3,
            &self.max_component_error_atoms.to_le_bytes(),
        );
        for (offset, value) in [
            (APPROXIMATION_CERTIFICATE_PRODUCT_OFFSET_V3, self.product_id),
            (
                APPROXIMATION_CERTIFICATE_RESULT_DOMAIN_OFFSET_V3,
                self.result_domain_id,
            ),
            (
                APPROXIMATION_CERTIFICATE_SEMANTIC_BASIS_OFFSET_V3,
                self.semantic_basis_id,
            ),
            (
                APPROXIMATION_CERTIFICATE_LINKED_BASIS_OFFSET_V3,
                self.linked_basis_record_id,
            ),
            (
                APPROXIMATION_CERTIFICATE_EVALUATOR_RELEASE_OFFSET_V3,
                self.evaluator_release_id,
            ),
            (
                APPROXIMATION_CERTIFICATE_PROJECTION_DIGEST_OFFSET_V3,
                self.projection_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        output
    }
}

/// Registry-finalized binding of one Product V3 graded basis and certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GradedBasisAdmissionV3 {
    result_domain_id: [u8; 32],
    product_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_id: [u8; 32],
    compiler_release_id: [u8; 32],
    toolchain_id: [u8; 32],
    certificate_digest: [u8; 32],
}

impl GradedBasisAdmissionV3 {
    /// Construct one nonzero exact admission binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        result_domain_id: [u8; 32],
        product_id: [u8; 32],
        coordinate_domain_id: [u8; 32],
        result_unit_id: [u8; 32],
        semantic_basis_id: [u8; 32],
        linked_basis_record_id: [u8; 32],
        compiler_release_id: [u8; 32],
        toolchain_id: [u8; 32],
        certificate_digest: [u8; 32],
    ) -> Result<Self> {
        for identity in [
            result_domain_id,
            product_id,
            coordinate_domain_id,
            result_unit_id,
            semantic_basis_id,
            linked_basis_record_id,
            compiler_release_id,
            toolchain_id,
            certificate_digest,
        ] {
            require_nonzero(identity)?;
        }
        Ok(Self {
            result_domain_id,
            product_id,
            coordinate_domain_id,
            result_unit_id,
            semantic_basis_id,
            linked_basis_record_id,
            compiler_release_id,
            toolchain_id,
            certificate_digest,
        })
    }

    /// Hostile-decode one exact admission record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != GRADED_BASIS_ADMISSION_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, GRADED_BASIS_ADMISSION_MAGIC_OFFSET_V3)?
            != GRADED_BASIS_ADMISSION_MAGIC_V3
            || read_u16(bytes, GRADED_BASIS_ADMISSION_VERSION_OFFSET_V3)?
                != GRADED_BASIS_REGISTRY_SCHEMA_VERSION_V3
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, GRADED_BASIS_ADMISSION_RESERVED_OFFSET_V3, 6)?;
        Self::new(
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_RESULT_DOMAIN_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_PRODUCT_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_COORDINATE_DOMAIN_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_RESULT_UNIT_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_SEMANTIC_BASIS_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_LINKED_BASIS_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_COMPILER_RELEASE_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_TOOLCHAIN_OFFSET_V3)?,
            nonzero_id(bytes, GRADED_BASIS_ADMISSION_CERTIFICATE_DIGEST_OFFSET_V3)?,
        )
    }

    /// Encode the sole exact admission record.
    pub fn to_bytes(self) -> [u8; GRADED_BASIS_ADMISSION_BYTES_V3] {
        let mut output = [0_u8; GRADED_BASIS_ADMISSION_BYTES_V3];
        put(
            &mut output,
            GRADED_BASIS_ADMISSION_MAGIC_OFFSET_V3,
            &GRADED_BASIS_ADMISSION_MAGIC_V3,
        );
        put(
            &mut output,
            GRADED_BASIS_ADMISSION_VERSION_OFFSET_V3,
            &GRADED_BASIS_REGISTRY_SCHEMA_VERSION_V3.to_le_bytes(),
        );
        for (offset, value) in [
            (
                GRADED_BASIS_ADMISSION_RESULT_DOMAIN_OFFSET_V3,
                self.result_domain_id,
            ),
            (GRADED_BASIS_ADMISSION_PRODUCT_OFFSET_V3, self.product_id),
            (
                GRADED_BASIS_ADMISSION_COORDINATE_DOMAIN_OFFSET_V3,
                self.coordinate_domain_id,
            ),
            (
                GRADED_BASIS_ADMISSION_RESULT_UNIT_OFFSET_V3,
                self.result_unit_id,
            ),
            (
                GRADED_BASIS_ADMISSION_SEMANTIC_BASIS_OFFSET_V3,
                self.semantic_basis_id,
            ),
            (
                GRADED_BASIS_ADMISSION_LINKED_BASIS_OFFSET_V3,
                self.linked_basis_record_id,
            ),
            (
                GRADED_BASIS_ADMISSION_COMPILER_RELEASE_OFFSET_V3,
                self.compiler_release_id,
            ),
            (
                GRADED_BASIS_ADMISSION_TOOLCHAIN_OFFSET_V3,
                self.toolchain_id,
            ),
            (
                GRADED_BASIS_ADMISSION_CERTIFICATE_DIGEST_OFFSET_V3,
                self.certificate_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        output
    }

    /// Exact ResultDomainV2 raw-record digest.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }
    /// Stable Product identity.
    pub const fn product_id(self) -> [u8; 32] {
        self.product_id
    }
    /// Product-owned coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> [u8; 32] {
        self.coordinate_domain_id
    }
    /// Product-owned result-unit identity.
    pub const fn result_unit_id(self) -> [u8; 32] {
        self.result_unit_id
    }
    /// Acyclic semantic Product V3 basis identity.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.semantic_basis_id
    }
    /// Exact linked basis raw-record digest.
    pub const fn linked_basis_record_id(self) -> [u8; 32] {
        self.linked_basis_record_id
    }
    /// Exact compiler release identity.
    pub const fn compiler_release_id(self) -> [u8; 32] {
        self.compiler_release_id
    }
    /// Exact compiler toolchain identity.
    pub const fn toolchain_id(self) -> [u8; 32] {
        self.toolchain_id
    }
    /// SHA-256 digest of the exact 256-byte certificate body.
    pub const fn certificate_digest(self) -> [u8; 32] {
        self.certificate_digest
    }
}

/// Decode and join already Registry-authenticated raw record bodies.
///
/// This recomputes the exact ResultDomain, linked-basis, semantic-basis, and
/// certificate digests.  It does not treat a schema ID or an admission record
/// as evidence that Registry owner/PDA/rent/staging checks occurred.
pub fn admit_authenticated_graded_basis_v3(
    result_domain_bytes: &[u8],
    linked_basis_bytes: &[u8],
    certificate_bytes: &[u8],
    admission_bytes: &[u8],
) -> Result<GradedBasisAdmissionV3> {
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(|_| Error::InvalidRecord)?;
    let basis = ProductBasisV3::decode(linked_basis_bytes).map_err(|_| Error::InvalidRecord)?;
    if basis.kind() != BasisKindV3::GradedExactComplement {
        return Err(Error::InvalidRecord);
    }
    let certificate = CategoricalApproximationCertificateV3::decode(certificate_bytes)?;
    let admission = GradedBasisAdmissionV3::decode(admission_bytes)?;
    let result_domain_id = digest(result_domain_bytes);
    let linked_basis_record_id = digest(linked_basis_bytes);
    let certificate_digest = digest(certificate_bytes);
    let semantic =
        semantic_basis_preimage_v3(linked_basis_bytes).map_err(|_| Error::InvalidRecord)?;
    let semantic_basis_id = digest_fragments(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ]);
    let product_id = domain.product_id().to_bytes();
    let coordinate_domain_id = domain.coordinate_domain_id().to_bytes();
    let result_unit_id = domain.result_unit_id().to_bytes();
    let outcome_count = domain.outcome_count().map_err(|_| Error::InvalidRecord)?;
    if admission.result_domain_id != result_domain_id
        || admission.product_id != product_id
        || admission.coordinate_domain_id != coordinate_domain_id
        || admission.result_unit_id != result_unit_id
        || admission.semantic_basis_id != semantic_basis_id
        || admission.linked_basis_record_id != linked_basis_record_id
        || admission.certificate_digest != certificate_digest
        || domain.liability_basis_id().to_bytes() != semantic_basis_id
        || basis.result_domain_id() != result_domain_id
        || basis.product_id() != product_id
        || basis.coordinate_domain_id() != coordinate_domain_id
        || basis.result_unit_id() != result_unit_id
        || certificate.product_id != product_id
        || certificate.result_domain_id != result_domain_id
        || certificate.semantic_basis_id != semantic_basis_id
        || certificate.linked_basis_record_id != linked_basis_record_id
        || certificate.evaluator_release_id != basis.evaluator_release_id()
        || certificate.basis_width != basis.basis_width()
        || certificate.outcome_count != outcome_count
        || certificate.payout_scale != basis.payout_scale()
    {
        return Err(Error::IdentityMismatch);
    }
    Ok(admission)
}

/// Derive the sole canonical admission record from exact authenticated bodies
/// and explicit compiler/toolchain identities.
///
/// The returned record is not finalization evidence.  An operator must still
/// upload and finalize its exact bytes under
/// [`GRADED_BASIS_ADMISSION_SCHEMA_ID_V3`].
pub fn derive_graded_basis_admission_v3(
    result_domain_bytes: &[u8],
    linked_basis_bytes: &[u8],
    certificate_bytes: &[u8],
    compiler_release_id: [u8; 32],
    toolchain_id: [u8; 32],
) -> Result<GradedBasisAdmissionV3> {
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(|_| Error::InvalidRecord)?;
    let basis = ProductBasisV3::decode(linked_basis_bytes).map_err(|_| Error::InvalidRecord)?;
    let certificate = CategoricalApproximationCertificateV3::decode(certificate_bytes)?;
    let semantic =
        semantic_basis_preimage_v3(linked_basis_bytes).map_err(|_| Error::InvalidRecord)?;
    let record = GradedBasisAdmissionV3::new(
        digest(result_domain_bytes),
        domain.product_id().to_bytes(),
        domain.coordinate_domain_id().to_bytes(),
        domain.result_unit_id().to_bytes(),
        digest_fragments(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ]),
        digest(linked_basis_bytes),
        compiler_release_id,
        toolchain_id,
        digest(certificate_bytes),
    )?;
    if certificate.evaluator_release_id != basis.evaluator_release_id() {
        return Err(Error::IdentityMismatch);
    }
    let bytes = record.to_bytes();
    admit_authenticated_graded_basis_v3(
        result_domain_bytes,
        linked_basis_bytes,
        certificate_bytes,
        &bytes,
    )
}

/// SHA-256 digest of one exact Registry raw body.
pub fn raw_record_digest_v3(bytes: &[u8]) -> [u8; 32] {
    digest(bytes)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn digest_fragments(fragments: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for fragment in fragments {
        hasher.update(fragment);
    }
    hasher.finalize().into()
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

fn nonzero_id(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array(bytes, offset)?;
    require_nonzero(value)?;
    Ok(value)
}

fn require_zero(bytes: &[u8], offset: usize, count: usize) -> Result<()> {
    let end = offset.checked_add(count).ok_or(Error::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
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

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}
