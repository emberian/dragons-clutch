#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Pure consumer-side admission for exact-rational Product payoff evidence.
//!
//! The authenticated Market and its capability manifest remain authority. A
//! finalized binding record is selected only through the manifest and is the
//! sole owner of the correspondence between canonical 32-byte Product/result
//! identities and the payoff interpreter's compact scalar identities. The
//! evaluator emits evidence; it cannot open, settle, mint, transfer, or mutate
//! a Market. Admission emits one immutable replay receipt and likewise owns no
//! economic state.
//!
//! Hashing, finalized-record PDAs, Loader V3 authentication, account ownership,
//! rent, and receipt creation remain adapter obligations.

use core::convert::TryInto;

use dclutch_capability_contract::{ActivationPolicy, CapabilityEntryV1, CapabilityManifestV1};
use dclutch_core_contract::{MarketRoot, Phase};
use dclutch_product_contract::{product::InstanceV1, result_domain::FiniteResultDomainV1};
use dclutch_product_payoff_v2_codec::ProductPayoffV2;
use dclutch_product_payoff_v2_svm::{
    CertificateKindV2, PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2, PayoffCertificateV2,
};
use dclutch_resolution_codec::{ResolutionCertificateKindV1, ResolutionCertificateV1};

/// Exact finalized binding-record width.
pub const PAYOFF_BINDING_BYTES_V1: usize = 384;
/// Exact admission request width.
pub const PAYOFF_ADMISSION_REQUEST_BYTES_V1: usize = 128;
/// Exact immutable admission receipt width.
pub const PAYOFF_ADMISSION_RECEIPT_BYTES_V1: usize = 448;
/// Binding-record magic.
pub const PAYOFF_BINDING_MAGIC_V1: [u8; 8] = *b"DCLTPAB1";
/// Admission request magic.
pub const PAYOFF_ADMISSION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTPAR1";
/// Admission receipt magic.
pub const PAYOFF_ADMISSION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTPAC1";
/// Shared wire version.
pub const PAYOFF_ADMISSION_VERSION_V1: u16 = 1;
/// Receipt PDA seed domain.
pub const PAYOFF_ADMISSION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch:payoff-admission:v1";

/// SHA-256 of `dclutch/capability/product-payoff-admission-v1`.
pub const PRODUCT_PAYOFF_ADMISSION_KIND_ID_V1: [u8; 32] = [
    0x8e, 0x8a, 0x06, 0x39, 0x32, 0x33, 0x9a, 0x7e, 0xb9, 0x10, 0x60, 0x8e, 0x76, 0xb1, 0xe7, 0x0a,
    0xd0, 0xf4, 0x1b, 0x99, 0x9b, 0x62, 0x52, 0xee, 0xab, 0x89, 0x0f, 0xfb, 0x73, 0x3b, 0x54, 0x74,
];
/// SHA-256 of `dclutch/semantic/product-payoff-admission-v1`.
pub const PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1: [u8; 32] = [
    0xe0, 0x4a, 0x35, 0x1d, 0x24, 0x3b, 0x5e, 0x7f, 0x96, 0xe1, 0x1b, 0x67, 0x1d, 0x5f, 0xc7, 0xbb,
    0xfb, 0xd9, 0x8f, 0x28, 0xd9, 0x8c, 0x99, 0x4d, 0xd9, 0x00, 0x3c, 0xb2, 0x42, 0x41, 0xaf, 0x33,
];
/// SHA-256 of `dclutch/schema/product-payoff-binding-v1`.
pub const PRODUCT_PAYOFF_BINDING_SCHEMA_ID_V1: [u8; 32] = [
    0x7e, 0xcf, 0xd4, 0xfc, 0x07, 0xa4, 0xc6, 0x9a, 0x29, 0x52, 0x37, 0xb6, 0xd0, 0xad, 0x81, 0x44,
    0x8f, 0x5a, 0x68, 0x14, 0xf1, 0x67, 0xa1, 0x7a, 0xa7, 0x9a, 0x4e, 0x43, 0x95, 0x08, 0x77, 0x91,
];
/// SHA-256 of `dclutch/schema/product-payoff-admission-receipt-v1`.
pub const PRODUCT_PAYOFF_ADMISSION_RECEIPT_SCHEMA_ID_V1: [u8; 32] = [
    0x27, 0x56, 0x1f, 0x5a, 0xd6, 0xbf, 0x18, 0x13, 0x02, 0xbf, 0x5d, 0x39, 0x22, 0xdc, 0x9e, 0xeb,
    0x4e, 0xe0, 0x21, 0x2a, 0xf3, 0x43, 0x0c, 0x63, 0x60, 0x33, 0x27, 0x24, 0x55, 0x5a, 0x8e, 0xca,
];
/// SHA-256 of `dclutch/derivation/product-payoff-admission-receipt-v1`.
pub const PRODUCT_PAYOFF_ADMISSION_RECEIPT_DERIVATION_ID_V1: [u8; 32] = [
    0xd7, 0xca, 0xab, 0x45, 0x7c, 0x4c, 0x40, 0xba, 0x86, 0x9f, 0xe6, 0xd2, 0x6c, 0xd7, 0xd0, 0x6d,
    0x13, 0x09, 0xaf, 0x19, 0x6f, 0x5c, 0x6b, 0x86, 0xee, 0xc2, 0x61, 0x7d, 0x49, 0xcf, 0x1b, 0x2e,
];

const ROLE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const BINDING_PRODUCT_INSTANCE_OFFSET: usize = 16;
const BINDING_RESULT_DOMAIN_OFFSET: usize = 48;
const BINDING_PAYOFF_RECORD_OFFSET: usize = 80;
const BINDING_PAYOFF_PROGRAM_OFFSET: usize = 112;
const BINDING_PAYOFF_ARTIFACT_OFFSET: usize = 144;
const BINDING_RESOLUTION_PROGRAM_OFFSET: usize = 176;
const BINDING_RESOLUTION_ARTIFACT_OFFSET: usize = 208;
const BINDING_ADMISSION_PROGRAM_OFFSET: usize = 240;
const BINDING_ADMISSION_ARTIFACT_OFFSET: usize = 272;
const BINDING_ROUNDING_RELEASE_OFFSET: usize = 304;
const BINDING_PRODUCT_SCALAR_OFFSET: usize = 336;
const BINDING_DOMAIN_SCALAR_OFFSET: usize = 344;
const BINDING_UNIT_SCALAR_OFFSET: usize = 352;
const BINDING_PAYOUT_SCALE_OFFSET: usize = 360;
const BINDING_FAILURE_PAYOUT_OFFSET: usize = 368;
const BINDING_TAIL_RESERVED_OFFSET: usize = 376;
const REQUEST_GENERATION_OFFSET: usize = 16;
const REQUEST_BINDING_DIGEST_OFFSET: usize = 24;
const REQUEST_PAYOFF_CERTIFICATE_DIGEST_OFFSET: usize = 56;
const REQUEST_RESOLUTION_CERTIFICATE_DIGEST_OFFSET: usize = 88;
const REQUEST_TAIL_RESERVED_OFFSET: usize = 120;
const RECEIPT_MARKET_OFFSET: usize = 16;
const RECEIPT_MARKET_IDENTITY_DIGEST_OFFSET: usize = 48;
const RECEIPT_MANIFEST_DIGEST_OFFSET: usize = 80;
const RECEIPT_BINDING_DIGEST_OFFSET: usize = 112;
const RECEIPT_PRODUCT_INSTANCE_OFFSET: usize = 144;
const RECEIPT_RESULT_DOMAIN_OFFSET: usize = 176;
const RECEIPT_PAYOFF_CERTIFICATE_ACCOUNT_OFFSET: usize = 208;
const RECEIPT_PAYOFF_CERTIFICATE_DIGEST_OFFSET: usize = 240;
const RECEIPT_RESOLUTION_CERTIFICATE_ACCOUNT_OFFSET: usize = 272;
const RECEIPT_RESOLUTION_CERTIFICATE_DIGEST_OFFSET: usize = 304;
const RECEIPT_EVALUATOR_ARTIFACT_OFFSET: usize = 336;
const RECEIPT_ROUNDING_RELEASE_OFFSET: usize = 368;
const RECEIPT_GENERATION_OFFSET: usize = 400;
const RECEIPT_RESULT_NUMERATOR_OFFSET: usize = 408;
const RECEIPT_RESULT_DENOMINATOR_OFFSET: usize = 424;
const RECEIPT_PAYOUT_OFFSET: usize = 432;
const RECEIPT_LIABILITY_OFFSET: usize = 440;

/// Stable refusal from exact Product evidence admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed-layout input had the wrong exact width.
    InvalidLength,
    /// Magic selected another record family.
    InvalidMagic,
    /// The schema version was unsupported.
    UnsupportedVersion,
    /// A role discriminant was unknown.
    UnknownRole,
    /// Reserved or role-inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A required digest or program identity was all zero.
    ZeroIdentity,
    /// Market identity, generation, lifecycle, or owner binding differed.
    MarketMismatch,
    /// The Market-selected capability entry was missing or inconsistent.
    CapabilityMismatch,
    /// Product instance, finite result domain, or scalar binding differed.
    ProductMismatch,
    /// Payoff certificate identity, role, or recomputed result differed.
    PayoffCertificateMismatch,
    /// Resolution certificate identity, role, result, or selector differed.
    ResolutionCertificateMismatch,
    /// Available collateral did not cover evaluator and failure liabilities.
    UnderCollateralized,
}

/// Admission role and immutable replay dimension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdmissionRoleV1 {
    /// Founding-time conservative liability admission.
    Liability = 0,
    /// Terminal success evaluation against exact Source rational evidence.
    SuccessEvaluation = 1,
    /// Terminal failure evaluation against the explicit failure payout.
    FailureEvaluation = 2,
}

impl AdmissionRoleV1 {
    /// Return the stable PDA/wire byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Liability => 0,
            Self::SuccessEvaluation => 1,
            Self::FailureEvaluation => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::Liability),
            1 => Ok(Self::SuccessEvaluation),
            2 => Ok(Self::FailureEvaluation),
            _ => Err(Error::UnknownRole),
        }
    }
}

/// Immutable manifest-selected association among canonical Product truth and
/// the exact evaluator, resolver, and admission deployments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffBindingV1 {
    product_instance_id: [u8; 32],
    result_domain_id: [u8; 32],
    payoff_record_digest: [u8; 32],
    payoff_program: [u8; 32],
    payoff_artifact_digest: [u8; 32],
    resolution_program: [u8; 32],
    resolution_artifact_digest: [u8; 32],
    admission_program: [u8; 32],
    admission_artifact_digest: [u8; 32],
    rounding_release_id: [u8; 32],
    product_id: u64,
    domain_id: u64,
    coordinate_unit_id: u64,
    payout_scale: u64,
    failure_payout: u64,
}

impl PayoffBindingV1 {
    /// Construct one exact nonzero association.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        product_instance_id: [u8; 32],
        result_domain_id: [u8; 32],
        payoff_record_digest: [u8; 32],
        payoff_program: [u8; 32],
        payoff_artifact_digest: [u8; 32],
        resolution_program: [u8; 32],
        resolution_artifact_digest: [u8; 32],
        admission_program: [u8; 32],
        admission_artifact_digest: [u8; 32],
        product_id: u64,
        domain_id: u64,
        coordinate_unit_id: u64,
        payout_scale: u64,
        failure_payout: u64,
    ) -> Result<Self, Error> {
        for identity in [
            product_instance_id,
            result_domain_id,
            payoff_record_digest,
            payoff_program,
            payoff_artifact_digest,
            resolution_program,
            resolution_artifact_digest,
            admission_program,
            admission_artifact_digest,
        ] {
            require_nonzero(identity)?;
        }
        if product_id == 0 || domain_id == 0 || coordinate_unit_id == 0 || payout_scale == 0 {
            return Err(Error::ProductMismatch);
        }
        Ok(Self {
            product_instance_id,
            result_domain_id,
            payoff_record_digest,
            payoff_program,
            payoff_artifact_digest,
            resolution_program,
            resolution_artifact_digest,
            admission_program,
            admission_artifact_digest,
            rounding_release_id: PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2,
            product_id,
            domain_id,
            coordinate_unit_id,
            payout_scale,
            failure_payout,
        })
    }

    /// Hostile-decode one exact binding record.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(bytes, PAYOFF_BINDING_BYTES_V1, &PAYOFF_BINDING_MAGIC_V1)?;
        if read_byte(bytes, ROLE_OFFSET)? != 0
            || !zero_span(bytes, HEADER_RESERVED_OFFSET, 5)?
            || !zero_span(bytes, BINDING_TAIL_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self::new(
            read_array(bytes, BINDING_PRODUCT_INSTANCE_OFFSET)?,
            read_array(bytes, BINDING_RESULT_DOMAIN_OFFSET)?,
            read_array(bytes, BINDING_PAYOFF_RECORD_OFFSET)?,
            read_array(bytes, BINDING_PAYOFF_PROGRAM_OFFSET)?,
            read_array(bytes, BINDING_PAYOFF_ARTIFACT_OFFSET)?,
            read_array(bytes, BINDING_RESOLUTION_PROGRAM_OFFSET)?,
            read_array(bytes, BINDING_RESOLUTION_ARTIFACT_OFFSET)?,
            read_array(bytes, BINDING_ADMISSION_PROGRAM_OFFSET)?,
            read_array(bytes, BINDING_ADMISSION_ARTIFACT_OFFSET)?,
            read_u64(bytes, BINDING_PRODUCT_SCALAR_OFFSET)?,
            read_u64(bytes, BINDING_DOMAIN_SCALAR_OFFSET)?,
            read_u64(bytes, BINDING_UNIT_SCALAR_OFFSET)?,
            read_u64(bytes, BINDING_PAYOUT_SCALE_OFFSET)?,
            read_u64(bytes, BINDING_FAILURE_PAYOUT_OFFSET)?,
        )?;
        if read_array::<32>(bytes, BINDING_ROUNDING_RELEASE_OFFSET)?
            != PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2
        {
            return Err(Error::ProductMismatch);
        }
        Ok(value)
    }

    /// Encode the sole canonical binding record.
    pub fn to_bytes(self) -> [u8; PAYOFF_BINDING_BYTES_V1] {
        let mut output = [0_u8; PAYOFF_BINDING_BYTES_V1];
        put_header(&mut output, &PAYOFF_BINDING_MAGIC_V1, 0);
        for (offset, value) in [
            (BINDING_PRODUCT_INSTANCE_OFFSET, self.product_instance_id),
            (BINDING_RESULT_DOMAIN_OFFSET, self.result_domain_id),
            (BINDING_PAYOFF_RECORD_OFFSET, self.payoff_record_digest),
            (BINDING_PAYOFF_PROGRAM_OFFSET, self.payoff_program),
            (BINDING_PAYOFF_ARTIFACT_OFFSET, self.payoff_artifact_digest),
            (BINDING_RESOLUTION_PROGRAM_OFFSET, self.resolution_program),
            (
                BINDING_RESOLUTION_ARTIFACT_OFFSET,
                self.resolution_artifact_digest,
            ),
            (BINDING_ADMISSION_PROGRAM_OFFSET, self.admission_program),
            (
                BINDING_ADMISSION_ARTIFACT_OFFSET,
                self.admission_artifact_digest,
            ),
            (BINDING_ROUNDING_RELEASE_OFFSET, self.rounding_release_id),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (BINDING_PRODUCT_SCALAR_OFFSET, self.product_id),
            (BINDING_DOMAIN_SCALAR_OFFSET, self.domain_id),
            (BINDING_UNIT_SCALAR_OFFSET, self.coordinate_unit_id),
            (BINDING_PAYOUT_SCALE_OFFSET, self.payout_scale),
            (BINDING_FAILURE_PAYOUT_OFFSET, self.failure_payout),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Return the canonical Product-instance identity.
    pub const fn product_instance_id(self) -> [u8; 32] {
        self.product_instance_id
    }
    /// Return the Product-owned result-domain content identity.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }
    /// Return the exact payoff record digest.
    pub const fn payoff_record_digest(self) -> [u8; 32] {
        self.payoff_record_digest
    }
    /// Return the exact evaluator program.
    pub const fn payoff_program(self) -> [u8; 32] {
        self.payoff_program
    }
    /// Return the exact evaluator artifact-release digest.
    pub const fn payoff_artifact_digest(self) -> [u8; 32] {
        self.payoff_artifact_digest
    }
    /// Return the exact resolution program.
    pub const fn resolution_program(self) -> [u8; 32] {
        self.resolution_program
    }
    /// Return the exact resolution artifact-release digest.
    pub const fn resolution_artifact_digest(self) -> [u8; 32] {
        self.resolution_artifact_digest
    }
    /// Return this admission program identity.
    pub const fn admission_program(self) -> [u8; 32] {
        self.admission_program
    }
    /// Return this admission artifact-release digest.
    pub const fn admission_artifact_digest(self) -> [u8; 32] {
        self.admission_artifact_digest
    }
    /// Return the sole exact rounding semantic release.
    pub const fn rounding_release_id(self) -> [u8; 32] {
        self.rounding_release_id
    }
    /// Return the payoff Product scalar identity.
    pub const fn product_id(self) -> u64 {
        self.product_id
    }
    /// Return the payoff domain scalar identity.
    pub const fn domain_id(self) -> u64 {
        self.domain_id
    }
    /// Return the payoff coordinate-unit scalar identity.
    pub const fn coordinate_unit_id(self) -> u64 {
        self.coordinate_unit_id
    }
    /// Return the exact payout unit scale.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }
    /// Return the explicit failure payout in scaled units.
    pub const fn failure_payout(self) -> u64 {
        self.failure_payout
    }
}

/// Exact optimistic and digest-bound admission request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffAdmissionRequestV1 {
    role: AdmissionRoleV1,
    expected_generation: u64,
    binding_digest: [u8; 32],
    payoff_certificate_digest: [u8; 32],
    resolution_certificate_digest: [u8; 32],
}

impl PayoffAdmissionRequestV1 {
    /// Construct and validate one role-specific request.
    pub fn new(
        role: AdmissionRoleV1,
        expected_generation: u64,
        binding_digest: [u8; 32],
        payoff_certificate_digest: [u8; 32],
        resolution_certificate_digest: [u8; 32],
    ) -> Result<Self, Error> {
        if expected_generation == 0 {
            return Err(Error::MarketMismatch);
        }
        require_nonzero(binding_digest)?;
        match role {
            AdmissionRoleV1::Liability
                if !is_zero(&payoff_certificate_digest)
                    && is_zero(&resolution_certificate_digest) => {}
            AdmissionRoleV1::SuccessEvaluation
                if !is_zero(&payoff_certificate_digest)
                    && !is_zero(&resolution_certificate_digest) => {}
            AdmissionRoleV1::FailureEvaluation
                if is_zero(&payoff_certificate_digest)
                    && !is_zero(&resolution_certificate_digest) => {}
            _ => return Err(Error::NonCanonicalReserved),
        }
        Ok(Self {
            role,
            expected_generation,
            binding_digest,
            payoff_certificate_digest,
            resolution_certificate_digest,
        })
    }

    /// Hostile-decode one exact request.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(
            bytes,
            PAYOFF_ADMISSION_REQUEST_BYTES_V1,
            &PAYOFF_ADMISSION_REQUEST_MAGIC_V1,
        )?;
        if !zero_span(bytes, HEADER_RESERVED_OFFSET, 5)?
            || !zero_span(bytes, REQUEST_TAIL_RESERVED_OFFSET, 8)?
        {
            return Err(Error::NonCanonicalReserved);
        }
        Self::new(
            AdmissionRoleV1::decode(read_byte(bytes, ROLE_OFFSET)?)?,
            read_u64(bytes, REQUEST_GENERATION_OFFSET)?,
            read_array(bytes, REQUEST_BINDING_DIGEST_OFFSET)?,
            read_array(bytes, REQUEST_PAYOFF_CERTIFICATE_DIGEST_OFFSET)?,
            read_array(bytes, REQUEST_RESOLUTION_CERTIFICATE_DIGEST_OFFSET)?,
        )
    }

    /// Encode the sole canonical request.
    pub fn to_bytes(self) -> [u8; PAYOFF_ADMISSION_REQUEST_BYTES_V1] {
        let mut output = [0_u8; PAYOFF_ADMISSION_REQUEST_BYTES_V1];
        put_header(
            &mut output,
            &PAYOFF_ADMISSION_REQUEST_MAGIC_V1,
            self.role.byte(),
        );
        put(
            &mut output,
            REQUEST_GENERATION_OFFSET,
            &self.expected_generation.to_le_bytes(),
        );
        put(
            &mut output,
            REQUEST_BINDING_DIGEST_OFFSET,
            &self.binding_digest,
        );
        put(
            &mut output,
            REQUEST_PAYOFF_CERTIFICATE_DIGEST_OFFSET,
            &self.payoff_certificate_digest,
        );
        put(
            &mut output,
            REQUEST_RESOLUTION_CERTIFICATE_DIGEST_OFFSET,
            &self.resolution_certificate_digest,
        );
        output
    }

    /// Return the admission role.
    pub const fn role(self) -> AdmissionRoleV1 {
        self.role
    }
    /// Return the expected immutable Market generation.
    pub const fn expected_generation(self) -> u64 {
        self.expected_generation
    }
    /// Return the exact binding-record digest.
    pub const fn binding_digest(self) -> [u8; 32] {
        self.binding_digest
    }
    /// Return the payoff-certificate digest or zero for failure evaluation.
    pub const fn payoff_certificate_digest(self) -> [u8; 32] {
        self.payoff_certificate_digest
    }
    /// Return the resolution-certificate digest or zero for liability.
    pub const fn resolution_certificate_digest(self) -> [u8; 32] {
        self.resolution_certificate_digest
    }
}

/// Adapter-authenticated facts supplied to pure admission.
#[derive(Clone, Copy)]
pub struct AdmissionFactsV1<'a> {
    /// Current admission program identity.
    pub admission_program: [u8; 32],
    /// Finalized-record Registry/core program.
    pub registry_program: [u8; 32],
    /// Canonical Market account identity.
    pub market_account: [u8; 32],
    /// Digest of the immutable Market identity preimage.
    pub market_identity_digest: [u8; 32],
    /// Decoded Market root.
    pub market: &'a MarketRoot,
    /// Exact capability-manifest digest.
    pub manifest_digest: [u8; 32],
    /// Decoded Market-selected capability manifest.
    pub manifest: CapabilityManifestV1<'a>,
    /// Exact finalized binding-record digest.
    pub binding_digest: [u8; 32],
    /// Decoded binding record.
    pub binding: &'a PayoffBindingV1,
    /// Exact finalized Product-instance digest.
    pub product_instance_id: [u8; 32],
    /// Decoded Product instance.
    pub product_instance: &'a InstanceV1,
    /// Exact Product-owned result-domain semantic identity.
    pub result_domain_id: [u8; 32],
    /// Decoded finite result domain.
    pub result_domain: &'a FiniteResultDomainV1,
    /// Decoded exact-rational payoff program.
    pub payoff: &'a ProductPayoffV2,
    /// Payoff-certificate account or zero when absent.
    pub payoff_certificate_account: [u8; 32],
    /// Exact payoff-certificate digest or zero when absent.
    pub payoff_certificate_digest: [u8; 32],
    /// Decoded payoff certificate when required by the role.
    pub payoff_certificate: Option<&'a PayoffCertificateV2>,
    /// Resolution-certificate account or zero when absent.
    pub resolution_certificate_account: [u8; 32],
    /// Exact resolution-certificate digest or zero when absent.
    pub resolution_certificate_digest: [u8; 32],
    /// Decoded resolution certificate when required by the role.
    pub resolution_certificate: Option<&'a ResolutionCertificateV1>,
}

/// Immutable consumer receipt. It is evidence only, not Market state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoffAdmissionReceiptV1 {
    role: AdmissionRoleV1,
    market: [u8; 32],
    market_identity_digest: [u8; 32],
    manifest_digest: [u8; 32],
    binding_digest: [u8; 32],
    product_instance_id: [u8; 32],
    result_domain_id: [u8; 32],
    payoff_certificate_account: [u8; 32],
    payoff_certificate_digest: [u8; 32],
    resolution_certificate_account: [u8; 32],
    resolution_certificate_digest: [u8; 32],
    evaluator_artifact_digest: [u8; 32],
    rounding_release_id: [u8; 32],
    generation: u64,
    result_numerator: i128,
    result_denominator: u64,
    payout: u64,
    liability_bound: u64,
}

impl PayoffAdmissionReceiptV1 {
    /// Hostile-decode one exact immutable receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        require_header(
            bytes,
            PAYOFF_ADMISSION_RECEIPT_BYTES_V1,
            &PAYOFF_ADMISSION_RECEIPT_MAGIC_V1,
        )?;
        if !zero_span(bytes, HEADER_RESERVED_OFFSET, 5)? {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            role: AdmissionRoleV1::decode(read_byte(bytes, ROLE_OFFSET)?)?,
            market: read_array(bytes, RECEIPT_MARKET_OFFSET)?,
            market_identity_digest: read_array(bytes, RECEIPT_MARKET_IDENTITY_DIGEST_OFFSET)?,
            manifest_digest: read_array(bytes, RECEIPT_MANIFEST_DIGEST_OFFSET)?,
            binding_digest: read_array(bytes, RECEIPT_BINDING_DIGEST_OFFSET)?,
            product_instance_id: read_array(bytes, RECEIPT_PRODUCT_INSTANCE_OFFSET)?,
            result_domain_id: read_array(bytes, RECEIPT_RESULT_DOMAIN_OFFSET)?,
            payoff_certificate_account: read_array(
                bytes,
                RECEIPT_PAYOFF_CERTIFICATE_ACCOUNT_OFFSET,
            )?,
            payoff_certificate_digest: read_array(bytes, RECEIPT_PAYOFF_CERTIFICATE_DIGEST_OFFSET)?,
            resolution_certificate_account: read_array(
                bytes,
                RECEIPT_RESOLUTION_CERTIFICATE_ACCOUNT_OFFSET,
            )?,
            resolution_certificate_digest: read_array(
                bytes,
                RECEIPT_RESOLUTION_CERTIFICATE_DIGEST_OFFSET,
            )?,
            evaluator_artifact_digest: read_array(bytes, RECEIPT_EVALUATOR_ARTIFACT_OFFSET)?,
            rounding_release_id: read_array(bytes, RECEIPT_ROUNDING_RELEASE_OFFSET)?,
            generation: read_u64(bytes, RECEIPT_GENERATION_OFFSET)?,
            result_numerator: read_i128(bytes, RECEIPT_RESULT_NUMERATOR_OFFSET)?,
            result_denominator: read_u64(bytes, RECEIPT_RESULT_DENOMINATOR_OFFSET)?,
            payout: read_u64(bytes, RECEIPT_PAYOUT_OFFSET)?,
            liability_bound: read_u64(bytes, RECEIPT_LIABILITY_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), Error> {
        for identity in [
            self.market,
            self.market_identity_digest,
            self.manifest_digest,
            self.binding_digest,
            self.product_instance_id,
            self.result_domain_id,
            self.evaluator_artifact_digest,
            self.rounding_release_id,
        ] {
            require_nonzero(identity)?;
        }
        if self.generation == 0
            || self.liability_bound == 0
            || self.payout > self.liability_bound
            || self.rounding_release_id != PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2
        {
            return Err(Error::ProductMismatch);
        }
        match self.role {
            AdmissionRoleV1::Liability
                if is_zero(&self.resolution_certificate_account)
                    && is_zero(&self.resolution_certificate_digest)
                    && self.result_numerator == 0
                    && self.result_denominator == 0
                    && self.payout == 0
                    && !is_zero(&self.payoff_certificate_account)
                    && !is_zero(&self.payoff_certificate_digest) =>
            {
                Ok(())
            }
            AdmissionRoleV1::SuccessEvaluation
                if !is_zero(&self.payoff_certificate_account)
                    && !is_zero(&self.payoff_certificate_digest)
                    && !is_zero(&self.resolution_certificate_account)
                    && !is_zero(&self.resolution_certificate_digest)
                    && self.result_denominator != 0 =>
            {
                Ok(())
            }
            AdmissionRoleV1::FailureEvaluation
                if is_zero(&self.payoff_certificate_account)
                    && is_zero(&self.payoff_certificate_digest)
                    && !is_zero(&self.resolution_certificate_account)
                    && !is_zero(&self.resolution_certificate_digest)
                    && self.result_numerator == 0
                    && self.result_denominator == 0 =>
            {
                Ok(())
            }
            _ => Err(Error::NonCanonicalReserved),
        }
    }

    /// Encode the sole canonical immutable receipt.
    pub fn to_bytes(self) -> [u8; PAYOFF_ADMISSION_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; PAYOFF_ADMISSION_RECEIPT_BYTES_V1];
        put_header(
            &mut output,
            &PAYOFF_ADMISSION_RECEIPT_MAGIC_V1,
            self.role.byte(),
        );
        for (offset, value) in [
            (RECEIPT_MARKET_OFFSET, self.market),
            (
                RECEIPT_MARKET_IDENTITY_DIGEST_OFFSET,
                self.market_identity_digest,
            ),
            (RECEIPT_MANIFEST_DIGEST_OFFSET, self.manifest_digest),
            (RECEIPT_BINDING_DIGEST_OFFSET, self.binding_digest),
            (RECEIPT_PRODUCT_INSTANCE_OFFSET, self.product_instance_id),
            (RECEIPT_RESULT_DOMAIN_OFFSET, self.result_domain_id),
            (
                RECEIPT_PAYOFF_CERTIFICATE_ACCOUNT_OFFSET,
                self.payoff_certificate_account,
            ),
            (
                RECEIPT_PAYOFF_CERTIFICATE_DIGEST_OFFSET,
                self.payoff_certificate_digest,
            ),
            (
                RECEIPT_RESOLUTION_CERTIFICATE_ACCOUNT_OFFSET,
                self.resolution_certificate_account,
            ),
            (
                RECEIPT_RESOLUTION_CERTIFICATE_DIGEST_OFFSET,
                self.resolution_certificate_digest,
            ),
            (
                RECEIPT_EVALUATOR_ARTIFACT_OFFSET,
                self.evaluator_artifact_digest,
            ),
            (RECEIPT_ROUNDING_RELEASE_OFFSET, self.rounding_release_id),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            RECEIPT_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            RECEIPT_RESULT_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        );
        put(
            &mut output,
            RECEIPT_RESULT_DENOMINATOR_OFFSET,
            &self.result_denominator.to_le_bytes(),
        );
        put(
            &mut output,
            RECEIPT_PAYOUT_OFFSET,
            &self.payout.to_le_bytes(),
        );
        put(
            &mut output,
            RECEIPT_LIABILITY_OFFSET,
            &self.liability_bound.to_le_bytes(),
        );
        output
    }

    /// Return the role.
    pub const fn role(self) -> AdmissionRoleV1 {
        self.role
    }
    /// Return the exact Market account.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return the exact signed result numerator, zero outside success evaluation.
    pub const fn result_numerator(self) -> i128 {
        self.result_numerator
    }
    /// Return the exact result denominator, zero outside success evaluation.
    pub const fn result_denominator(self) -> u64 {
        self.result_denominator
    }
    /// Return the admitted payout.
    pub const fn payout(self) -> u64 {
        self.payout
    }
    /// Return the total conservative bound including explicit failure payout.
    pub const fn liability_bound(self) -> u64 {
        self.liability_bound
    }
}

/// Validate all semantic joins and construct the sole immutable receipt bytes.
pub fn admit(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
) -> Result<PayoffAdmissionReceiptV1, Error> {
    validate_common(request, facts)?;
    let total_bound = facts
        .payoff
        .liability_bound()
        .max(facts.binding.failure_payout());
    let (numerator, denominator, payout) = match request.role() {
        AdmissionRoleV1::Liability => admit_liability(request, facts, total_bound)?,
        AdmissionRoleV1::SuccessEvaluation => admit_success(request, facts, total_bound)?,
        AdmissionRoleV1::FailureEvaluation => admit_failure(request, facts, total_bound)?,
    };
    let receipt = PayoffAdmissionReceiptV1 {
        role: request.role(),
        market: facts.market_account,
        market_identity_digest: facts.market_identity_digest,
        manifest_digest: facts.manifest_digest,
        binding_digest: facts.binding_digest,
        product_instance_id: facts.product_instance_id,
        result_domain_id: facts.result_domain_id,
        payoff_certificate_account: facts.payoff_certificate_account,
        payoff_certificate_digest: facts.payoff_certificate_digest,
        resolution_certificate_account: facts.resolution_certificate_account,
        resolution_certificate_digest: facts.resolution_certificate_digest,
        evaluator_artifact_digest: facts.binding.payoff_artifact_digest(),
        rounding_release_id: facts.binding.rounding_release_id(),
        generation: request.expected_generation(),
        result_numerator: numerator,
        result_denominator: denominator,
        payout,
        liability_bound: total_bound,
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_common(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
) -> Result<(), Error> {
    for identity in [
        facts.admission_program,
        facts.registry_program,
        facts.market_account,
        facts.market_identity_digest,
    ] {
        require_nonzero(identity)?;
    }
    let identity = facts.market.identity();
    if identity.generation() != request.expected_generation()
        || identity.product_instance_id().to_bytes() != facts.product_instance_id
        || identity.capability_manifest_id().to_bytes() != facts.manifest_digest
        || request.binding_digest() != facts.binding_digest
    {
        return Err(Error::MarketMismatch);
    }
    if facts.binding.product_instance_id() != facts.product_instance_id
        || facts.binding.result_domain_id() != facts.result_domain_id
        || facts.binding.admission_program() != facts.admission_program
        || facts.product_instance.result_domain_id().to_bytes() != facts.result_domain_id
        || facts.product_instance.claim_basis_id().to_bytes()
            != identity.claim_basis_id().to_bytes()
        || facts.product_instance.partition_cell_count()
            != u32::from(facts.result_domain.outcome_count())
        || facts.payoff.product_id() != facts.binding.product_id()
        || facts.payoff.domain_id() != facts.binding.domain_id()
        || facts.payoff.coordinate_unit_id() != facts.binding.coordinate_unit_id()
        || facts.payoff.payout_scale() != facts.binding.payout_scale()
        || facts.binding.rounding_release_id() != PRODUCT_PAYOFF_ROUNDING_RELEASE_ID_V2
    {
        return Err(Error::ProductMismatch);
    }
    let entry = select_capability(facts.manifest)?;
    if entry.release_id().to_bytes() != PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1
        || entry.config_id().to_bytes() != facts.binding_digest
        || entry.capacity_profile_id().to_bytes()
            != facts
                .product_instance
                .capacity_profile_id()
                .content_id()
                .to_bytes()
        || entry.child_schema_id().to_bytes() != PRODUCT_PAYOFF_ADMISSION_RECEIPT_SCHEMA_ID_V1
        || entry.child_derivation_id().to_bytes()
            != PRODUCT_PAYOFF_ADMISSION_RECEIPT_DERIVATION_ID_V1
        || entry.activation_policy() != ActivationPolicy::RequiredAtFounding
    {
        return Err(Error::CapabilityMismatch);
    }
    Ok(())
}

fn select_capability(manifest: CapabilityManifestV1<'_>) -> Result<CapabilityEntryV1, Error> {
    let mut found = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|_| Error::CapabilityMismatch)?;
        if entry.kind_id().to_bytes() == PRODUCT_PAYOFF_ADMISSION_KIND_ID_V1 {
            if found.is_some() {
                return Err(Error::CapabilityMismatch);
            }
            found = Some(entry);
        }
        index = index.checked_add(1).ok_or(Error::CapabilityMismatch)?;
    }
    found.ok_or(Error::CapabilityMismatch)
}

fn validate_payoff_certificate(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
    expected_kind: CertificateKindV2,
) -> Result<PayoffCertificateV2, Error> {
    let certificate = facts
        .payoff_certificate
        .ok_or(Error::PayoffCertificateMismatch)?;
    if request.payoff_certificate_digest() != facts.payoff_certificate_digest
        || is_zero(&facts.payoff_certificate_account)
        || certificate.kind() != expected_kind
        || certificate.registry_program() != facts.registry_program
        || certificate.product_record_digest() != facts.binding.payoff_record_digest()
        || certificate.artifact_release_digest() != facts.binding.payoff_artifact_digest()
        || certificate.rounding_release_id() != facts.binding.rounding_release_id()
        || certificate.product_id() != facts.payoff.product_id()
        || certificate.domain_id() != facts.payoff.domain_id()
        || certificate.coordinate_unit_id() != facts.payoff.coordinate_unit_id()
        || certificate.payout_scale() != facts.payoff.payout_scale()
        || certificate.liability_bound() != facts.payoff.liability_bound()
    {
        return Err(Error::PayoffCertificateMismatch);
    }
    Ok(*certificate)
}

fn admit_liability(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
    total_bound: u64,
) -> Result<(i128, u64, u64), Error> {
    if facts.market.phase() != Phase::Founding
        || facts.resolution_certificate.is_some()
        || !is_zero(&facts.resolution_certificate_account)
        || !is_zero(&facts.resolution_certificate_digest)
    {
        return Err(Error::MarketMismatch);
    }
    let certificate = validate_payoff_certificate(request, facts, CertificateKindV2::Liability)?;
    if !certificate.collateralized() || certificate.available() < total_bound {
        return Err(Error::UnderCollateralized);
    }
    Ok((0, 0, 0))
}

fn validate_resolution(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
    expected: ResolutionCertificateKindV1,
) -> Result<ResolutionCertificateV1, Error> {
    let certificate = facts
        .resolution_certificate
        .ok_or(Error::ResolutionCertificateMismatch)?;
    if request.resolution_certificate_digest() != facts.resolution_certificate_digest
        || is_zero(&facts.resolution_certificate_account)
        || certificate.receipt_account != facts.resolution_certificate_account
        || certificate.kind != expected
        || certificate.market != facts.market_account
        || certificate.product != facts.product_instance_id
        || certificate.source_material != facts.market.identity().resolution_policy_id().to_bytes()
        || certificate.generation != request.expected_generation()
    {
        return Err(Error::ResolutionCertificateMismatch);
    }
    Ok(*certificate)
}

fn terminal_phase(phase: Phase) -> bool {
    matches!(phase, Phase::Resolved | Phase::Retiring | Phase::Retired)
}

fn admit_success(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
    total_bound: u64,
) -> Result<(i128, u64, u64), Error> {
    if !terminal_phase(facts.market.phase()) {
        return Err(Error::MarketMismatch);
    }
    let resolution = validate_resolution(
        request,
        facts,
        ResolutionCertificateKindV1::ResolutionSuccess,
    )?;
    let selector = facts
        .result_domain
        .map(resolution.result_numerator, resolution.result_denominator)
        .map_err(|_| Error::ResolutionCertificateMismatch)?;
    if u32::from(selector) != resolution.selector {
        return Err(Error::ResolutionCertificateMismatch);
    }
    let certificate = validate_payoff_certificate(request, facts, CertificateKindV2::Evaluation)?;
    let payout = facts
        .payoff
        .evaluate_rational(resolution.result_numerator, resolution.result_denominator)
        .map_err(|_| Error::PayoffCertificateMismatch)?;
    if certificate.result_numerator() != resolution.result_numerator
        || certificate.result_denominator() != resolution.result_denominator
        || certificate.payout() != payout
        || payout > total_bound
    {
        return Err(Error::PayoffCertificateMismatch);
    }
    Ok((
        resolution.result_numerator,
        resolution.result_denominator,
        payout,
    ))
}

fn admit_failure(
    request: PayoffAdmissionRequestV1,
    facts: AdmissionFactsV1<'_>,
    total_bound: u64,
) -> Result<(i128, u64, u64), Error> {
    if !terminal_phase(facts.market.phase())
        || facts.payoff_certificate.is_some()
        || !is_zero(&facts.payoff_certificate_account)
        || !is_zero(&facts.payoff_certificate_digest)
    {
        return Err(Error::MarketMismatch);
    }
    let resolution = validate_resolution(
        request,
        facts,
        ResolutionCertificateKindV1::ResolutionFailure,
    )?;
    if resolution.selector != u32::from(facts.result_domain.failure_selector())
        || facts.binding.failure_payout() > total_bound
    {
        return Err(Error::ResolutionCertificateMismatch);
    }
    Ok((0, 0, facts.binding.failure_payout()))
}

fn require_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != PAYOFF_ADMISSION_VERSION_V1 {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn put_header(output: &mut [u8], magic: &[u8; 8], role: u8) {
    put(output, 0, magic);
    put(output, 8, &PAYOFF_ADMISSION_VERSION_V1.to_le_bytes());
    put_byte(output, ROLE_OFFSET, role);
}

fn require_nonzero(value: [u8; 32]) -> Result<(), Error> {
    if is_zero(&value) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8, Error> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_small_array(bytes, offset)?))
}
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_small_array(bytes, offset)?))
}
fn read_i128(bytes: &[u8], offset: usize) -> Result<i128, Error> {
    Ok(i128::from_le_bytes(read_small_array(bytes, offset)?))
}
fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    read_small_array(bytes, offset)
}
fn read_small_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}
fn zero_span(bytes: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}
fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    if let Some(end) = offset.checked_add(source.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(source);
    }
}
fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = value;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1,
        MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId as CoreId, MarketIdentity};
    use dclutch_product_contract::{
        ContentId as ProductId, capacity::CapacityProfileId, product::InstanceV1Input,
    };
    use dclutch_product_payoff_v2_codec::{
        ABI_BYTES_V2, KNOT_BYTES_V2, KNOTS_OFFSET_V2, MAGIC_V2, TERMS_OFFSET_V2, VERSION_V2,
    };
    use std::vec::Vec;

    fn binding() -> PayoffBindingV1 {
        PayoffBindingV1::new(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32], 81,
            70, 11, 100, 13,
        )
        .expect("binding")
    }

    fn core_id(bytes: [u8; 32]) -> CoreId {
        CoreId::new(bytes).expect("core id")
    }

    fn product_id(bytes: [u8; 32]) -> ProductId {
        ProductId::new(bytes).expect("Product id")
    }

    fn zero_quote() -> FundingQuoteV1 {
        let none = CompartmentFundingV1::not_applicable();
        FundingQuoteV1::new(
            FundingAmountsV1::new(none, none, none, none, none, none, none).expect("amounts"),
            None,
        )
        .expect("quote")
    }

    fn payoff_bytes() -> [u8; ABI_BYTES_V2] {
        let mut bytes = [0_u8; ABI_BYTES_V2];
        put(&mut bytes, 0, &MAGIC_V2);
        put(&mut bytes, 8, &VERSION_V2.to_le_bytes());
        put_byte(&mut bytes, 10, 5);
        put_byte(&mut bytes, 11, 4);
        for (offset, value) in [(16, 81_u64), (24, 70), (32, 11), (40, 100), (48, 2)] {
            put(&mut bytes, offset, &value.to_le_bytes());
        }
        for (index, knot) in [-100_i128, -50, 0, 50, 100].into_iter().enumerate() {
            let offset = KNOTS_OFFSET_V2 + index * KNOT_BYTES_V2;
            put(&mut bytes, offset, &knot.to_le_bytes());
        }
        for (index, (tag, left, peak, right, amplitude)) in [
            (0_u8, 0_u8, 0_u8, 0_u8, 2_u64),
            (1, 0, 0, 4, 10),
            (2, 0, 0, 4, 5),
            (3, 1, 2, 3, 20),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = TERMS_OFFSET_V2 + index * 16;
            put_byte(&mut bytes, offset, tag);
            put_byte(&mut bytes, offset + 1, left);
            put_byte(&mut bytes, offset + 2, peak);
            put_byte(&mut bytes, offset + 3, right);
            put(&mut bytes, offset + 8, &amplitude.to_le_bytes());
        }
        bytes
    }

    struct SemanticFixture {
        market: MarketRoot,
        instance: InstanceV1,
        domain: FiniteResultDomainV1,
        payoff: ProductPayoffV2,
        binding: PayoffBindingV1,
        manifest_bytes: [u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES],
    }

    fn fixture(phase: Phase) -> SemanticFixture {
        let binding = binding();
        let capacity = [41_u8; 32];
        let instance = InstanceV1::new(InstanceV1Input {
            terms_id: product_id([42; 32]),
            occurrence_id: product_id([43; 32]),
            claim_basis_id: product_id([44; 32]),
            result_domain_id: product_id([2; 32]),
            capacity_profile_id: CapacityProfileId::new(product_id(capacity)),
            partition_cell_count: 4,
        })
        .expect("instance");
        let domain =
            FiniteResultDomainV1::new(product_id([45; 32]), product_id([46; 32]), 2, &[-50, 0])
                .expect("domain");
        let entry = CapabilityEntryV1::new(
            core_id(PRODUCT_PAYOFF_ADMISSION_KIND_ID_V1),
            core_id(PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1),
            core_id([21; 32]),
            core_id(capacity),
            core_id(PRODUCT_PAYOFF_ADMISSION_RECEIPT_SCHEMA_ID_V1),
            core_id(PRODUCT_PAYOFF_ADMISSION_RECEIPT_DERIVATION_ID_V1),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            zero_quote(),
        )
        .expect("entry");
        let mut manifest_bytes = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest");
        let identity = MarketIdentity::new(
            core_id([47; 32]),
            core_id([1; 32]),
            core_id([44; 32]),
            core_id([14; 32]),
            core_id([20; 32]),
            7,
        );
        let mut market = MarketRoot::founding(identity, [48; 32]).expect("market");
        if phase != Phase::Founding {
            market.transition_phase(7, Phase::Open).expect("open");
        }
        if matches!(phase, Phase::Resolved | Phase::Retiring | Phase::Retired) {
            market
                .transition_phase(7, Phase::Resolved)
                .expect("resolved");
        }
        if matches!(phase, Phase::Retiring | Phase::Retired) {
            market
                .transition_phase(7, Phase::Retiring)
                .expect("retiring");
        }
        if phase == Phase::Retired {
            market.transition_phase(7, Phase::Retired).expect("retired");
        }
        SemanticFixture {
            market,
            instance,
            domain,
            payoff: ProductPayoffV2::decode(&payoff_bytes()).expect("payoff"),
            binding,
            manifest_bytes,
        }
    }

    fn success_resolution() -> ResolutionCertificateV1 {
        ResolutionCertificateV1 {
            kind: ResolutionCertificateKindV1::ResolutionSuccess,
            market: [15; 32],
            route: [16; 32],
            source_material: [14; 32],
            product: [1; 32],
            provider_evidence: [17; 32],
            funding_allocation: [0; 32],
            receipt_account: [18; 32],
            generation: 7,
            attempt_index: 0,
            schedule_index: 0,
            selector: 2,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: 75,
            result_denominator: 2,
            observed_at: 50,
        }
    }

    fn facts<'a>(
        fixture: &'a SemanticFixture,
        manifest: CapabilityManifestV1<'a>,
        payoff_certificate: Option<&'a PayoffCertificateV2>,
        resolution_certificate: Option<&'a ResolutionCertificateV1>,
    ) -> AdmissionFactsV1<'a> {
        AdmissionFactsV1 {
            admission_program: [8; 32],
            registry_program: [99; 32],
            market_account: [15; 32],
            market_identity_digest: [19; 32],
            market: &fixture.market,
            manifest_digest: [20; 32],
            manifest,
            binding_digest: [21; 32],
            binding: &fixture.binding,
            product_instance_id: [1; 32],
            product_instance: &fixture.instance,
            result_domain_id: [2; 32],
            result_domain: &fixture.domain,
            payoff: &fixture.payoff,
            payoff_certificate_account: if payoff_certificate.is_some() {
                [22; 32]
            } else {
                [0; 32]
            },
            payoff_certificate_digest: if payoff_certificate.is_some() {
                [23; 32]
            } else {
                [0; 32]
            },
            payoff_certificate,
            resolution_certificate_account: resolution_certificate
                .map_or([0; 32], |value| value.receipt_account),
            resolution_certificate_digest: if resolution_certificate.is_some() {
                [24; 32]
            } else {
                [0; 32]
            },
            resolution_certificate,
        }
    }

    #[test]
    fn binding_is_exact_and_rounding_owned() {
        let bytes = binding().to_bytes();
        assert_eq!(PayoffBindingV1::decode(&bytes), Ok(binding()));
        for width in 0..PAYOFF_BINDING_BYTES_V1 {
            assert_eq!(
                PayoffBindingV1::decode(bytes.get(..width).expect("width")),
                Err(Error::InvalidLength)
            );
        }
        let mut rounded = bytes;
        *rounded
            .get_mut(BINDING_ROUNDING_RELEASE_OFFSET)
            .expect("offset") ^= 1;
        assert_eq!(
            PayoffBindingV1::decode(&rounded),
            Err(Error::ProductMismatch)
        );
    }

    #[test]
    fn role_wires_refuse_truncation_padding_and_inactive_digests() {
        let values = [
            PayoffAdmissionRequestV1::new(AdmissionRoleV1::Liability, 7, [1; 32], [2; 32], [0; 32])
                .expect("liability"),
            PayoffAdmissionRequestV1::new(
                AdmissionRoleV1::SuccessEvaluation,
                7,
                [1; 32],
                [2; 32],
                [3; 32],
            )
            .expect("success"),
            PayoffAdmissionRequestV1::new(
                AdmissionRoleV1::FailureEvaluation,
                7,
                [1; 32],
                [0; 32],
                [3; 32],
            )
            .expect("failure"),
        ];
        for value in values {
            let bytes = value.to_bytes();
            assert_eq!(PayoffAdmissionRequestV1::decode(&bytes), Ok(value));
            let mut padded = Vec::from(bytes);
            padded.push(0);
            assert_eq!(
                PayoffAdmissionRequestV1::decode(&padded),
                Err(Error::InvalidLength)
            );
        }
        assert_eq!(
            PayoffAdmissionRequestV1::new(
                AdmissionRoleV1::FailureEvaluation,
                7,
                [1; 32],
                [2; 32],
                [3; 32]
            ),
            Err(Error::NonCanonicalReserved)
        );
    }

    #[test]
    fn liability_success_failure_and_exact_replay_are_joined() {
        let founding = fixture(Phase::Founding);
        let manifest = CapabilityManifestV1::decode(&founding.manifest_bytes).expect("manifest");
        let liability =
            PayoffCertificateV2::liability([99; 32], [3; 32], [5; 32], 81, 70, 11, 100, 37, 37)
                .expect("liability cert");
        let request = PayoffAdmissionRequestV1::new(
            AdmissionRoleV1::Liability,
            7,
            [21; 32],
            [23; 32],
            [0; 32],
        )
        .expect("request");
        let admitted = admit(request, facts(&founding, manifest, Some(&liability), None))
            .expect("liability admission");
        assert_eq!(admitted.liability_bound(), 37);
        assert_eq!(admitted.payout(), 0);
        assert_eq!(
            admit(request, facts(&founding, manifest, Some(&liability), None)),
            Ok(admitted)
        );

        let resolved = fixture(Phase::Resolved);
        let manifest = CapabilityManifestV1::decode(&resolved.manifest_bytes).expect("manifest");
        let evaluation = PayoffCertificateV2::evaluation(
            [99; 32], [3; 32], [5; 32], 81, 70, 11, 100, 75, 2, 10, 37,
        )
        .expect("evaluation cert");
        let resolution = success_resolution();
        let request = PayoffAdmissionRequestV1::new(
            AdmissionRoleV1::SuccessEvaluation,
            7,
            [21; 32],
            [23; 32],
            [24; 32],
        )
        .expect("request");
        let admitted = admit(
            request,
            facts(&resolved, manifest, Some(&evaluation), Some(&resolution)),
        )
        .expect("success admission");
        assert_eq!(admitted.result_numerator(), 75);
        assert_eq!(admitted.result_denominator(), 2);
        assert_eq!(admitted.payout(), 10);

        let failure = ResolutionCertificateV1 {
            kind: ResolutionCertificateKindV1::ResolutionFailure,
            market: [15; 32],
            route: [0; 32],
            source_material: [14; 32],
            product: [1; 32],
            provider_evidence: [0; 32],
            funding_allocation: [25; 32],
            receipt_account: [26; 32],
            generation: 7,
            attempt_index: 2,
            schedule_index: 0,
            selector: 3,
            work_paid: 1,
            funding_remaining: 0,
            result_numerator: 0,
            result_denominator: 0,
            observed_at: 0,
        };
        let request = PayoffAdmissionRequestV1::new(
            AdmissionRoleV1::FailureEvaluation,
            7,
            [21; 32],
            [0; 32],
            [24; 32],
        )
        .expect("failure request");
        let admitted = admit(request, facts(&resolved, manifest, None, Some(&failure)))
            .expect("failure admission");
        assert_eq!(admitted.payout(), 13);
    }

    #[test]
    fn substitution_wrong_phase_and_rounding_attack_refuse() {
        let resolved = fixture(Phase::Resolved);
        let manifest = CapabilityManifestV1::decode(&resolved.manifest_bytes).expect("manifest");
        let evaluation = PayoffCertificateV2::evaluation(
            [99; 32], [3; 32], [5; 32], 81, 70, 11, 100, 75, 2, 10, 37,
        )
        .expect("evaluation");
        let request = PayoffAdmissionRequestV1::new(
            AdmissionRoleV1::SuccessEvaluation,
            7,
            [21; 32],
            [23; 32],
            [24; 32],
        )
        .expect("request");
        let mut rounded = evaluation.to_bytes();
        put(&mut rounded, 208, &11_u64.to_le_bytes());
        let rounded = PayoffCertificateV2::decode(&rounded).expect("structurally valid lie");
        let success_resolution = success_resolution();
        assert_eq!(
            admit(
                request,
                facts(
                    &resolved,
                    manifest,
                    Some(&rounded),
                    Some(&success_resolution),
                )
            ),
            Err(Error::PayoffCertificateMismatch)
        );
        let mut substituted = facts(
            &resolved,
            manifest,
            Some(&evaluation),
            Some(&success_resolution),
        );
        substituted.result_domain_id = [55; 32];
        assert_eq!(admit(request, substituted), Err(Error::ProductMismatch));

        let open = fixture(Phase::Open);
        let open_manifest = CapabilityManifestV1::decode(&open.manifest_bytes).expect("manifest");
        assert_eq!(
            admit(
                request,
                facts(
                    &open,
                    open_manifest,
                    Some(&evaluation),
                    Some(&success_resolution),
                )
            ),
            Err(Error::MarketMismatch)
        );
    }
}
