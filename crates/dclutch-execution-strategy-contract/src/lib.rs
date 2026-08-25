#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Strategy-independent AOT certificate and shadow-comparison wire.
//!
//! This crate performs no hashing, finalized-record authentication, Registry
//! CPI, Loader inspection, account projection, or state/effect mutation. An
//! adapter supplies already authenticated identities and hashes. Trading runs
//! the canonical interpreter and a stateless AOT accelerator over one exact
//! runtime-width input register bank, requires identical acceptance/refusal and
//! accepted output-bank bytes, projects the one canonical effect itself, and
//! remains the only state/effect writer.
//!
//! A finalized certificate is content authenticity, not authorization. Profile
//! 1 therefore refuses AOT-only execution until Registry owns an immutable
//! descriptor-to-certificate-to-artifact admission and reauthentication route,
//! or a checked proof verifier establishes the same relation.

use dclutch_capability_program_contract::CapabilityProgramV1;
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ArtifactReleaseIdV1;

/// Schema label for [`ExecutionStrategyCertificateV1`].
pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/execution-strategy-certificate-v1";
/// SHA-256 of [`EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V1`].
pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V1: [u8; 32] = [
    0xa8, 0x4c, 0x7e, 0xef, 0x8f, 0xbb, 0x90, 0xa0, 0x3c, 0xe8, 0x99, 0x37, 0xf5, 0x21, 0xb2, 0xe3,
    0x3b, 0x8f, 0xb6, 0x15, 0xbf, 0x0e, 0x6c, 0x68, 0x58, 0xb9, 0x2d, 0x52, 0xec, 0xbc, 0xfa, 0xf5,
];
/// Schema label for [`AcceleratorRequestV1`].
pub const ACCELERATOR_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/accelerator-request-v1";
/// SHA-256 of [`ACCELERATOR_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const ACCELERATOR_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0xb1, 0x9c, 0x43, 0x39, 0xe7, 0x5d, 0xeb, 0x08, 0x16, 0xe1, 0xd4, 0x71, 0xb1, 0xf6, 0xdf, 0xac,
    0x4d, 0xaf, 0x10, 0x10, 0x2b, 0x0f, 0x85, 0xe5, 0x32, 0x38, 0xf0, 0x60, 0xa0, 0xb5, 0xc5, 0x9a,
];
/// Schema label for [`AcceleratorAckV1`].
pub const ACCELERATOR_ACK_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/accelerator-ack-v1";
/// SHA-256 of [`ACCELERATOR_ACK_SCHEMA_PREIMAGE_V1`].
pub const ACCELERATOR_ACK_SCHEMA_ID_V1: [u8; 32] = [
    0x88, 0xeb, 0x8a, 0x64, 0x7f, 0xbf, 0xe4, 0xe5, 0x7e, 0xcd, 0xee, 0xf7, 0x19, 0x80, 0x39, 0x1a,
    0x95, 0x5e, 0x00, 0x97, 0x12, 0x78, 0xea, 0xe8, 0x9f, 0x03, 0x47, 0xb3, 0xca, 0x8c, 0x06, 0xf8,
];

/// Exact certificate wire magic.
pub const EXECUTION_STRATEGY_CERTIFICATE_MAGIC_V1: [u8; 8] = *b"DCLTESC1";
/// Exact accelerator-request wire magic.
pub const ACCELERATOR_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTAIR1";
/// Exact accelerator-ack wire magic.
pub const ACCELERATOR_ACK_MAGIC_V1: [u8; 8] = *b"DCLTAAK1";
/// Implemented strategy schema version.
pub const EXECUTION_STRATEGY_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented strategy physical profile.
pub const EXECUTION_STRATEGY_PROFILE_V1: u16 = 1;
/// Exact bytes in one stateless-AOT certificate.
pub const EXECUTION_STRATEGY_CERTIFICATE_BYTES_V1: usize = 240;
/// Fixed bytes before one runtime-width input register bank.
pub const ACCELERATOR_REQUEST_HEADER_BYTES_V1: usize = 128;
/// Fixed bytes before one accepted output register bank.
pub const ACCELERATOR_ACK_HEADER_BYTES_V1: usize = 160;
/// Pinned SVM return-data limit exposed by the local Solana SDK.
pub const SVM_RETURN_DATA_BYTES_V1: usize = 1_024;
/// Largest output register bank transportable by profile-1 return data.
///
/// This is a chain-derived transport bound, not a Product-width bound. The
/// lifting path is an authenticated transaction-local scratch-page transport.
pub const ACCELERATOR_ACK_MAX_BANK_BYTES_V1: usize =
    SVM_RETURN_DATA_BYTES_V1 - ACCELERATOR_ACK_HEADER_BYTES_V1;

const CERTIFICATE_STRATEGY_TAG: u8 = 1;
const CERTIFICATE_CAPABILITY_PROGRAM_OFFSET: usize = 16;
const CERTIFICATE_ACCOUNT_PROFILE_OFFSET: usize = 48;
const CERTIFICATE_EFFECT_SCHEMA_OFFSET: usize = 80;
const CERTIFICATE_ARTIFACT_RELEASE_OFFSET: usize = 112;
const CERTIFICATE_COMPILER_RELEASE_OFFSET: usize = 144;
const CERTIFICATE_TOOLCHAIN_OFFSET: usize = 176;
const CERTIFICATE_VALIDATION_OFFSET: usize = 208;

const REQUEST_CERTIFICATE_OFFSET: usize = 16;
const REQUEST_CAPABILITY_PROGRAM_OFFSET: usize = 48;
const REQUEST_CONTEXT_DIGEST_OFFSET: usize = 80;
const REQUEST_SCALAR_COUNT_OFFSET: usize = 112;
const REQUEST_IDENTITY_COUNT_OFFSET: usize = 114;
const REQUEST_RESERVED_OFFSET: usize = 116;
const REQUEST_RESERVED_BYTES: usize = 12;

const ACK_CERTIFICATE_OFFSET: usize = 16;
const ACK_CAPABILITY_PROGRAM_OFFSET: usize = 48;
const ACK_REQUEST_DIGEST_OFFSET: usize = 80;
const ACK_BANK_DIGEST_OFFSET: usize = 112;
const ACK_SCALAR_COUNT_OFFSET: usize = 144;
const ACK_IDENTITY_COUNT_OFFSET: usize = 146;
const ACK_RESERVED_OFFSET: usize = 148;
const ACK_RESERVED_BYTES: usize = 12;

/// Stable refusal from strategy certificate or wire validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input bytes did not have their exact count-derived width.
    InvalidLength,
    /// Wire magic selected another schema.
    InvalidMagic,
    /// Schema version is unsupported.
    UnsupportedSchema,
    /// Physical profile is unsupported.
    UnsupportedProfile,
    /// The strategy or disposition tag was unknown.
    UnknownTag,
    /// Reserved bytes or action-inactive fields were nonzero.
    NonCanonicalReservedBytes,
    /// A required content identity was zero.
    ZeroIdentity,
    /// Checked count-to-width arithmetic overflowed.
    ArithmeticOverflow,
    /// Descriptor identity or descriptor-owned schema differed.
    DescriptorMismatch,
    /// Authenticated accelerator artifact differed from the certificate.
    ArtifactMismatch,
    /// Request or acknowledgement coordinates differed from the exact call.
    BindingMismatch,
    /// Interpreter and accelerator acceptance/refusal/output bank diverged.
    StrategyDivergence,
    /// An accepted output exceeded the profile-1 return-data transport.
    ResultCapacityExceeded,
    /// AOT-only execution was requested before Registry admission exists.
    AotOnlyUnavailable,
}

/// Result alias for execution-strategy operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Canonical execution disposition shared by interpreter and accelerator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDispositionV1 {
    /// The semantic input was accepted and produced a successor register bank.
    Accepted,
    /// The semantic input was refused and produced no successor bank.
    Refused,
}

/// Immutable certificate for one stateless AOT implementation.
///
/// Program, ProgramData, ELF, deployment slot, semantic release, and upgrade
/// policy remain solely owned by the referenced `ArtifactReleaseV1` record and
/// are intentionally not duplicated here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionStrategyCertificateV1 {
    capability_program: ContentId,
    account_profile: ContentId,
    effect_schema: ContentId,
    artifact_release: ArtifactReleaseIdV1,
    compiler_release: ContentId,
    toolchain_digest: ContentId,
    translation_validation: ContentId,
}

impl ExecutionStrategyCertificateV1 {
    /// Construct one already typed stateless-AOT certificate.
    pub const fn new(
        capability_program: ContentId,
        account_profile: ContentId,
        effect_schema: ContentId,
        artifact_release: ArtifactReleaseIdV1,
        compiler_release: ContentId,
        toolchain_digest: ContentId,
        translation_validation: ContentId,
    ) -> Self {
        Self {
            capability_program,
            account_profile,
            effect_schema,
            artifact_release,
            compiler_release,
            toolchain_digest,
            translation_validation,
        }
    }

    /// Hostile-decode one exact certificate.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_exact_header(
            bytes,
            EXECUTION_STRATEGY_CERTIFICATE_BYTES_V1,
            &EXECUTION_STRATEGY_CERTIFICATE_MAGIC_V1,
        )?;
        if byte(bytes, 12)? != CERTIFICATE_STRATEGY_TAG {
            return Err(Error::UnknownTag);
        }
        require_zero(bytes, 13, 3)?;
        Ok(Self::new(
            content(bytes, CERTIFICATE_CAPABILITY_PROGRAM_OFFSET)?,
            content(bytes, CERTIFICATE_ACCOUNT_PROFILE_OFFSET)?,
            content(bytes, CERTIFICATE_EFFECT_SCHEMA_OFFSET)?,
            ArtifactReleaseIdV1::decode(slice(bytes, CERTIFICATE_ARTIFACT_RELEASE_OFFSET, 32)?)
                .map_err(|_| Error::ZeroIdentity)?,
            content(bytes, CERTIFICATE_COMPILER_RELEASE_OFFSET)?,
            content(bytes, CERTIFICATE_TOOLCHAIN_OFFSET)?,
            content(bytes, CERTIFICATE_VALIDATION_OFFSET)?,
        ))
    }

    /// Encode the one canonical certificate preimage.
    pub fn to_bytes(self) -> [u8; EXECUTION_STRATEGY_CERTIFICATE_BYTES_V1] {
        let mut output = [0_u8; EXECUTION_STRATEGY_CERTIFICATE_BYTES_V1];
        put(&mut output, 0, &EXECUTION_STRATEGY_CERTIFICATE_MAGIC_V1);
        put(
            &mut output,
            8,
            &EXECUTION_STRATEGY_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &EXECUTION_STRATEGY_PROFILE_V1.to_le_bytes(),
        );
        if let Some(tag) = output.get_mut(12) {
            *tag = CERTIFICATE_STRATEGY_TAG;
        }
        put(
            &mut output,
            CERTIFICATE_CAPABILITY_PROGRAM_OFFSET,
            self.capability_program.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_ACCOUNT_PROFILE_OFFSET,
            self.account_profile.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_EFFECT_SCHEMA_OFFSET,
            self.effect_schema.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_ARTIFACT_RELEASE_OFFSET,
            self.artifact_release.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_COMPILER_RELEASE_OFFSET,
            self.compiler_release.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_TOOLCHAIN_OFFSET,
            self.toolchain_digest.as_bytes(),
        );
        put(
            &mut output,
            CERTIFICATE_VALIDATION_OFFSET,
            self.translation_validation.as_bytes(),
        );
        output
    }

    /// Require the certificate to bind the exact descriptor and its schemas.
    ///
    /// `program_id` is the adapter-authenticated digest of the complete
    /// descriptor. This SDK-free contract deliberately performs no hashing.
    pub fn validate_descriptor(
        self,
        program_id: ContentId,
        program: CapabilityProgramV1<'_>,
    ) -> Result<()> {
        if self.capability_program != program_id
            || self.account_profile != program.account_profile()
            || self.effect_schema != program.effect_schema()
        {
            Err(Error::DescriptorMismatch)
        } else {
            Ok(())
        }
    }

    /// Require the exact Registry-authenticated artifact-release identity.
    pub fn validate_artifact(self, authenticated_artifact: ArtifactReleaseIdV1) -> Result<()> {
        if self.artifact_release == authenticated_artifact {
            Ok(())
        } else {
            Err(Error::ArtifactMismatch)
        }
    }

    /// Refuse AOT-only execution in profile 1.
    pub const fn require_aot_only_admitted(self) -> Result<()> {
        let _ = self;
        Err(Error::AotOnlyUnavailable)
    }

    /// Return the exact capability-program content identity.
    pub const fn capability_program(self) -> ContentId {
        self.capability_program
    }
    /// Return the descriptor's account projection schema.
    pub const fn account_profile(self) -> ContentId {
        self.account_profile
    }
    /// Return the descriptor's one canonical effect schema.
    pub const fn effect_schema(self) -> ContentId {
        self.effect_schema
    }
    /// Return the referenced accelerator artifact release.
    pub const fn artifact_release(self) -> ArtifactReleaseIdV1 {
        self.artifact_release
    }
    /// Return the compiler semantic release.
    pub const fn compiler_release(self) -> ContentId {
        self.compiler_release
    }
    /// Return the exact toolchain/build-manifest digest.
    pub const fn toolchain_digest(self) -> ContentId {
        self.toolchain_digest
    }
    /// Return the translation-validation or theorem/corpus identity.
    pub const fn translation_validation(self) -> ContentId {
        self.translation_validation
    }
}

/// Borrowed runtime-width accelerator request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceleratorRequestV1<'a> {
    certificate: ContentId,
    capability_program: ContentId,
    invocation_context_digest: ContentId,
    scalar_count: u16,
    identity_count: u16,
    bank: &'a [u8],
}

impl<'a> AcceleratorRequestV1<'a> {
    /// Construct a request over one canonical input register bank.
    pub fn new(
        certificate: ContentId,
        capability_program: ContentId,
        invocation_context_digest: ContentId,
        scalar_count: u16,
        identity_count: u16,
        bank: &'a [u8],
    ) -> Result<Self> {
        if bank.len() != register_bank_bytes(scalar_count, identity_count)? {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            certificate,
            capability_program,
            invocation_context_digest,
            scalar_count,
            identity_count,
            bank,
        })
    }

    /// Hostile-decode one exact header plus count-derived input bank.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < ACCELERATOR_REQUEST_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        require_common_header(bytes, &ACCELERATOR_REQUEST_MAGIC_V1)?;
        require_zero(bytes, 12, 4)?;
        require_zero(bytes, REQUEST_RESERVED_OFFSET, REQUEST_RESERVED_BYTES)?;
        let scalar_count = read_u16(bytes, REQUEST_SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, REQUEST_IDENTITY_COUNT_OFFSET)?;
        let bank_bytes = register_bank_bytes(scalar_count, identity_count)?;
        let expected = ACCELERATOR_REQUEST_HEADER_BYTES_V1
            .checked_add(bank_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        Self::new(
            content(bytes, REQUEST_CERTIFICATE_OFFSET)?,
            content(bytes, REQUEST_CAPABILITY_PROGRAM_OFFSET)?,
            content(bytes, REQUEST_CONTEXT_DIGEST_OFFSET)?,
            scalar_count,
            identity_count,
            slice(bytes, ACCELERATOR_REQUEST_HEADER_BYTES_V1, bank_bytes)?,
        )
    }

    /// Encode the exact header and borrowed input bank into caller-owned bytes.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = ACCELERATOR_REQUEST_HEADER_BYTES_V1
            .checked_add(self.bank.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != expected
            || self.bank.len() != register_bank_bytes(self.scalar_count, self.identity_count)?
        {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &ACCELERATOR_REQUEST_MAGIC_V1);
        put(
            output,
            8,
            &EXECUTION_STRATEGY_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(output, 10, &EXECUTION_STRATEGY_PROFILE_V1.to_le_bytes());
        put(
            output,
            REQUEST_CERTIFICATE_OFFSET,
            self.certificate.as_bytes(),
        );
        put(
            output,
            REQUEST_CAPABILITY_PROGRAM_OFFSET,
            self.capability_program.as_bytes(),
        );
        put(
            output,
            REQUEST_CONTEXT_DIGEST_OFFSET,
            self.invocation_context_digest.as_bytes(),
        );
        put(
            output,
            REQUEST_SCALAR_COUNT_OFFSET,
            &self.scalar_count.to_le_bytes(),
        );
        put(
            output,
            REQUEST_IDENTITY_COUNT_OFFSET,
            &self.identity_count.to_le_bytes(),
        );
        put(output, ACCELERATOR_REQUEST_HEADER_BYTES_V1, self.bank);
        Ok(())
    }

    /// Require the request to bind the exact certificate and descriptor.
    pub fn validate_certificate(
        self,
        certificate_id: ContentId,
        certificate: ExecutionStrategyCertificateV1,
    ) -> Result<()> {
        if self.certificate != certificate_id
            || self.capability_program != certificate.capability_program
        {
            Err(Error::BindingMismatch)
        } else {
            Ok(())
        }
    }

    /// Return the strategy-certificate identity.
    pub const fn certificate(self) -> ContentId {
        self.certificate
    }
    /// Return the capability-program identity.
    pub const fn capability_program(self) -> ContentId {
        self.capability_program
    }
    /// Return the digest of exact release/Market/root/request/account inputs.
    pub const fn invocation_context_digest(self) -> ContentId {
        self.invocation_context_digest
    }
    /// Return the runtime scalar count.
    pub const fn scalar_count(self) -> u16 {
        self.scalar_count
    }
    /// Return the runtime identity count.
    pub const fn identity_count(self) -> u16 {
        self.identity_count
    }
    /// Borrow the canonical scalar bytes followed by identity bytes.
    pub const fn bank(self) -> &'a [u8] {
        self.bank
    }
}

/// Borrowed stateless accelerator acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceleratorAckV1<'a> {
    disposition: ExecutionDispositionV1,
    certificate: ContentId,
    capability_program: ContentId,
    request_digest: ContentId,
    bank_digest: Option<ContentId>,
    scalar_count: u16,
    identity_count: u16,
    bank: &'a [u8],
}

impl<'a> AcceleratorAckV1<'a> {
    /// Construct one accepted output bank after the adapter hashes both wires.
    pub fn accepted(
        request: AcceleratorRequestV1<'_>,
        request_digest: ContentId,
        bank_digest: ContentId,
        bank: &'a [u8],
    ) -> Result<Self> {
        let expected = register_bank_bytes(request.scalar_count, request.identity_count)?;
        if bank.len() != expected || expected > ACCELERATOR_ACK_MAX_BANK_BYTES_V1 {
            return Err(Error::ResultCapacityExceeded);
        }
        Ok(Self {
            disposition: ExecutionDispositionV1::Accepted,
            certificate: request.certificate,
            capability_program: request.capability_program,
            request_digest,
            bank_digest: Some(bank_digest),
            scalar_count: request.scalar_count,
            identity_count: request.identity_count,
            bank,
        })
    }

    /// Construct one semantic refusal with no candidate output.
    pub const fn refused(request: AcceleratorRequestV1<'_>, request_digest: ContentId) -> Self {
        Self {
            disposition: ExecutionDispositionV1::Refused,
            certificate: request.certificate,
            capability_program: request.capability_program,
            request_digest,
            bank_digest: None,
            scalar_count: 0,
            identity_count: 0,
            bank: &[],
        }
    }

    /// Hostile-decode one exact acknowledgement.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < ACCELERATOR_ACK_HEADER_BYTES_V1 || bytes.len() > SVM_RETURN_DATA_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        require_common_header(bytes, &ACCELERATOR_ACK_MAGIC_V1)?;
        require_zero(bytes, 13, 3)?;
        require_zero(bytes, ACK_RESERVED_OFFSET, ACK_RESERVED_BYTES)?;
        let disposition = match byte(bytes, 12)? {
            0 => ExecutionDispositionV1::Refused,
            1 => ExecutionDispositionV1::Accepted,
            _ => return Err(Error::UnknownTag),
        };
        let scalar_count = read_u16(bytes, ACK_SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, ACK_IDENTITY_COUNT_OFFSET)?;
        let bank_digest_bytes = read_array(bytes, ACK_BANK_DIGEST_OFFSET)?;
        let bank_bytes = register_bank_bytes(scalar_count, identity_count)?;
        let expected = ACCELERATOR_ACK_HEADER_BYTES_V1
            .checked_add(bank_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let (bank_digest, bank) = match disposition {
            ExecutionDispositionV1::Accepted => {
                if bank_bytes == 0 || bank_bytes > ACCELERATOR_ACK_MAX_BANK_BYTES_V1 {
                    return Err(Error::ResultCapacityExceeded);
                }
                (
                    Some(ContentId::new(bank_digest_bytes).map_err(|_| Error::ZeroIdentity)?),
                    slice(bytes, ACCELERATOR_ACK_HEADER_BYTES_V1, bank_bytes)?,
                )
            }
            ExecutionDispositionV1::Refused => {
                if scalar_count != 0 || identity_count != 0 || bank_digest_bytes != [0; 32] {
                    return Err(Error::NonCanonicalReservedBytes);
                }
                (None, slice(bytes, ACCELERATOR_ACK_HEADER_BYTES_V1, 0)?)
            }
        };
        Ok(Self {
            disposition,
            certificate: content(bytes, ACK_CERTIFICATE_OFFSET)?,
            capability_program: content(bytes, ACK_CAPABILITY_PROGRAM_OFFSET)?,
            request_digest: content(bytes, ACK_REQUEST_DIGEST_OFFSET)?,
            bank_digest,
            scalar_count,
            identity_count,
            bank,
        })
    }

    /// Encode the exact acknowledgement into caller-owned return-data bytes.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = ACCELERATOR_ACK_HEADER_BYTES_V1
            .checked_add(self.bank.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != expected || expected > SVM_RETURN_DATA_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        match self.disposition {
            ExecutionDispositionV1::Accepted => {
                if self.bank.is_empty()
                    || self.bank_digest.is_none()
                    || self.bank.len()
                        != register_bank_bytes(self.scalar_count, self.identity_count)?
                {
                    return Err(Error::NonCanonicalReservedBytes);
                }
            }
            ExecutionDispositionV1::Refused => {
                if !self.bank.is_empty()
                    || self.bank_digest.is_some()
                    || self.scalar_count != 0
                    || self.identity_count != 0
                {
                    return Err(Error::NonCanonicalReservedBytes);
                }
            }
        }
        output.fill(0);
        put(output, 0, &ACCELERATOR_ACK_MAGIC_V1);
        put(
            output,
            8,
            &EXECUTION_STRATEGY_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(output, 10, &EXECUTION_STRATEGY_PROFILE_V1.to_le_bytes());
        if let Some(disposition) = output.get_mut(12) {
            *disposition = match self.disposition {
                ExecutionDispositionV1::Refused => 0,
                ExecutionDispositionV1::Accepted => 1,
            };
        }
        put(output, ACK_CERTIFICATE_OFFSET, self.certificate.as_bytes());
        put(
            output,
            ACK_CAPABILITY_PROGRAM_OFFSET,
            self.capability_program.as_bytes(),
        );
        put(
            output,
            ACK_REQUEST_DIGEST_OFFSET,
            self.request_digest.as_bytes(),
        );
        if let Some(digest) = self.bank_digest {
            put(output, ACK_BANK_DIGEST_OFFSET, digest.as_bytes());
        }
        put(
            output,
            ACK_SCALAR_COUNT_OFFSET,
            &self.scalar_count.to_le_bytes(),
        );
        put(
            output,
            ACK_IDENTITY_COUNT_OFFSET,
            &self.identity_count.to_le_bytes(),
        );
        put(output, ACCELERATOR_ACK_HEADER_BYTES_V1, self.bank);
        Ok(())
    }

    /// Return the accelerator disposition.
    pub const fn disposition(self) -> ExecutionDispositionV1 {
        self.disposition
    }
    /// Return the request digest echoed by the accelerator.
    pub const fn request_digest(self) -> ContentId {
        self.request_digest
    }
    /// Return the adapter-authenticated candidate-bank digest on acceptance.
    pub const fn bank_digest(self) -> Option<ContentId> {
        self.bank_digest
    }
    /// Borrow the accepted candidate bank, empty on refusal.
    pub const fn bank(self) -> &'a [u8] {
        self.bank
    }
}

/// Require exact interpreter/AOT acceptance, refusal, and output-bank equality.
///
/// The outer adapter must first authenticate the certificate and accelerator
/// artifact, require the accelerator Program as the immediate return-data
/// producer, and hash the exact request and candidate bank. On success Trading
/// alone projects and applies the descriptor's canonical effect, commit-last.
pub fn compare_execution_v1(
    request: AcceleratorRequestV1<'_>,
    request_digest: ContentId,
    interpreter_disposition: ExecutionDispositionV1,
    interpreter_bank_digest: Option<ContentId>,
    interpreter_bank: &[u8],
    aot: AcceleratorAckV1<'_>,
) -> Result<()> {
    if aot.certificate != request.certificate
        || aot.capability_program != request.capability_program
        || aot.request_digest != request_digest
    {
        return Err(Error::BindingMismatch);
    }
    match (interpreter_disposition, aot.disposition) {
        (ExecutionDispositionV1::Refused, ExecutionDispositionV1::Refused) => {
            if interpreter_bank.is_empty()
                && interpreter_bank_digest.is_none()
                && aot.bank.is_empty()
                && aot.bank_digest.is_none()
            {
                Ok(())
            } else {
                Err(Error::StrategyDivergence)
            }
        }
        (ExecutionDispositionV1::Accepted, ExecutionDispositionV1::Accepted) => {
            if interpreter_bank.len()
                != register_bank_bytes(request.scalar_count, request.identity_count)?
                || aot.scalar_count != request.scalar_count
                || aot.identity_count != request.identity_count
                || interpreter_bank_digest != aot.bank_digest
                || interpreter_bank != aot.bank
            {
                Err(Error::StrategyDivergence)
            } else {
                Ok(())
            }
        }
        _ => Err(Error::StrategyDivergence),
    }
}

/// Return the exact scalar-then-identity register-bank width.
pub fn register_bank_bytes(scalar_count: u16, identity_count: u16) -> Result<usize> {
    usize::from(scalar_count)
        .checked_mul(8)
        .and_then(|scalars| {
            usize::from(identity_count)
                .checked_mul(32)
                .and_then(|identities| scalars.checked_add(identities))
        })
        .ok_or(Error::ArithmeticOverflow)
}

/// Encode one scalar-then-identity register bank into exact caller storage.
///
/// Scalars use canonical little-endian u64 bytes. Identities remain exact
/// 32-byte values. This codec is strategy-neutral and runtime-width.
pub fn encode_register_bank_into(
    scalars: &[u64],
    identities: &[[u8; 32]],
    output: &mut [u8],
) -> Result<()> {
    let scalar_count = u16::try_from(scalars.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let identity_count = u16::try_from(identities.len()).map_err(|_| Error::ArithmeticOverflow)?;
    if output.len() != register_bank_bytes(scalar_count, identity_count)? {
        return Err(Error::InvalidLength);
    }
    let scalar_bytes = scalars
        .len()
        .checked_mul(8)
        .ok_or(Error::ArithmeticOverflow)?;
    let (scalar_output, identity_output) = output.split_at_mut(scalar_bytes);
    for (value, encoded) in scalars.iter().zip(scalar_output.chunks_exact_mut(8)) {
        encoded.copy_from_slice(&value.to_le_bytes());
    }
    for (identity, encoded) in identities.iter().zip(identity_output.chunks_exact_mut(32)) {
        encoded.copy_from_slice(identity);
    }
    Ok(())
}

/// Decode one exact scalar-then-identity bank into caller storage.
///
/// Width is validated before either output slice is modified.
pub fn decode_register_bank_into(
    bank: &[u8],
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    let scalar_count = u16::try_from(scalars.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let identity_count = u16::try_from(identities.len()).map_err(|_| Error::ArithmeticOverflow)?;
    if bank.len() != register_bank_bytes(scalar_count, identity_count)? {
        return Err(Error::InvalidLength);
    }
    let scalar_bytes = scalars
        .len()
        .checked_mul(8)
        .ok_or(Error::ArithmeticOverflow)?;
    let (scalar_input, identity_input) = bank.split_at(scalar_bytes);
    for (encoded, value) in scalar_input.chunks_exact(8).zip(scalars.iter_mut()) {
        *value = u64::from_le_bytes(encoded.try_into().map_err(|_| Error::InvalidLength)?);
    }
    for (encoded, identity) in identity_input.chunks_exact(32).zip(identities.iter_mut()) {
        identity.copy_from_slice(encoded);
    }
    Ok(())
}

fn require_exact_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    require_common_header(bytes, magic)
}

fn require_common_header(bytes: &[u8], magic: &[u8; 8]) -> Result<()> {
    if bytes.len() < 12 {
        return Err(Error::InvalidLength);
    }
    if slice(bytes, 0, 8)? != magic {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != EXECUTION_STRATEGY_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if read_u16(bytes, 10)? != EXECUTION_STRATEGY_PROFILE_V1 {
        return Err(Error::UnsupportedProfile);
    }
    Ok(())
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array(bytes, offset)?).map_err(|_| Error::ZeroIdentity)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
    if let Some(end) = offset.checked_add(bytes.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(bytes);
    }
}

#[cfg(test)]
mod tests;
