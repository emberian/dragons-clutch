//! Acyclic execution-strategy selection and authenticated bank transport.

use core::convert::{TryFrom, TryInto};

use dclutch_capability_program_contract::{
    v3 as capability_v3, v3::CapabilityProgramV3, v4::CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ArtifactReleaseIdV1;

use crate::shadow_v3::{SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3};

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_v2.rs"]
mod generated;

pub use generated::*;

/// Schema label for finalized [`ExecutionStrategyProgramV2`] records.
pub const EXECUTION_STRATEGY_PROGRAM_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/execution-strategy-program-v2";
/// SHA-256 of [`EXECUTION_STRATEGY_PROGRAM_SCHEMA_PREIMAGE_V2`].
pub const EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2: [u8; 32] = [
    0x87, 0x34, 0x45, 0x0e, 0x4f, 0x09, 0xc9, 0xb2, 0xa0, 0x74, 0xd4, 0xcc, 0x30, 0x58, 0x92, 0xd9,
    0xd1, 0x1f, 0xc1, 0x1a, 0x69, 0xad, 0x6b, 0x92, 0x4c, 0x6c, 0xe9, 0x2e, 0xbc, 0x17, 0xe2, 0xc2,
];
/// Schema label for finalized [`ExecutionStrategyCertificateV2`] records.
pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/execution-strategy-certificate-v2";
/// SHA-256 of [`EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V2`].
pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2: [u8; 32] = [
    0x86, 0x4e, 0x61, 0x8d, 0xdc, 0xe0, 0xde, 0x13, 0x74, 0x9a, 0x07, 0x59, 0xf8, 0x1d, 0x67, 0xe0,
    0x27, 0x21, 0x0d, 0xb6, 0x6c, 0x3b, 0xa8, 0xb0, 0x0d, 0x41, 0x46, 0xa4, 0xcb, 0xc3, 0x17, 0x3b,
];
/// Schema label for finalized [`ExecutionStrategyAdmissionV2`] records.
pub const EXECUTION_STRATEGY_ADMISSION_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/execution-strategy-admission-v2";
/// SHA-256 of [`EXECUTION_STRATEGY_ADMISSION_SCHEMA_PREIMAGE_V2`].
pub const EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2: [u8; 32] = [
    0x30, 0x55, 0x4b, 0xae, 0x27, 0x4a, 0x3f, 0x79, 0x58, 0x58, 0x07, 0xdf, 0xb3, 0xab, 0x19, 0x72,
    0x97, 0xc9, 0xf7, 0xc0, 0x28, 0xc6, 0xb4, 0x85, 0xb2, 0x66, 0x73, 0xeb, 0xf8, 0xe2, 0x82, 0x12,
];
/// Schema label for [`AcceleratorRequestV2`].
pub const ACCELERATOR_REQUEST_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/accelerator-request-v2";
/// SHA-256 of [`ACCELERATOR_REQUEST_SCHEMA_PREIMAGE_V2`].
pub const ACCELERATOR_REQUEST_SCHEMA_ID_V2: [u8; 32] = [
    0x11, 0xe4, 0x4d, 0x67, 0x14, 0xe7, 0x7f, 0xb8, 0xba, 0xd3, 0xd5, 0x1e, 0x7c, 0x94, 0x2a, 0xe8,
    0x88, 0xfd, 0xed, 0xe9, 0x35, 0x61, 0x2f, 0xfa, 0x71, 0xd2, 0xb0, 0xe0, 0x9f, 0x1b, 0x4b, 0x76,
];
/// Schema label for [`AcceleratorAckV2`].
pub const ACCELERATOR_ACK_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/accelerator-ack-v2";
/// SHA-256 of [`ACCELERATOR_ACK_SCHEMA_PREIMAGE_V2`].
pub const ACCELERATOR_ACK_SCHEMA_ID_V2: [u8; 32] = [
    0x82, 0x84, 0x9e, 0x0f, 0x4f, 0xfc, 0xbd, 0x22, 0xe4, 0x12, 0xba, 0xbb, 0x70, 0x02, 0xd7, 0x0d,
    0x8f, 0x63, 0xae, 0x24, 0x42, 0x45, 0xd0, 0xbe, 0x88, 0x42, 0x73, 0xf9, 0x4b, 0xf7, 0xee, 0x4f,
];
/// Schema label for Trading-owned [`AuthenticatedScratchPageV2`] accounts.
pub const SCRATCH_PAGE_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/execution-strategy-scratch-page-v2";
/// SHA-256 of [`SCRATCH_PAGE_SCHEMA_PREIMAGE_V2`].
pub const SCRATCH_PAGE_SCHEMA_ID_V2: [u8; 32] = [
    0xf0, 0x17, 0x56, 0x8c, 0xe6, 0x75, 0x6f, 0xd0, 0x52, 0xbf, 0xf9, 0x3b, 0x44, 0xa3, 0x31, 0x96,
    0x91, 0x51, 0x97, 0x6c, 0x1f, 0xa4, 0xe1, 0x8c, 0x4c, 0x97, 0xd6, 0xf4, 0x0d, 0x94, 0xc1, 0xb5,
];

const DISPOSITION_INTERPRETED: u8 = 0;
const DISPOSITION_SHADOW_AOT: u8 = 1;
const DISPOSITION_ADMITTED_AOT: u8 = 2;
const PRESENT: u8 = 1;
const ABSENT: u8 = 0;
const TRANSPORT_INLINE: u8 = 0;
const TRANSPORT_SCRATCH: u8 = 1;
const ACK_REFUSED: u8 = 0;
const ACK_ACCEPTED: u8 = 1;
const SCRATCH_INPUT: u8 = 0;
const SCRATCH_CANDIDATE: u8 = 1;

/// Stable refusal from V2 strategy, authorization, or transport validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input bytes did not have their exact fixed or count-derived width.
    InvalidLength,
    /// Magic selected another record or transport schema.
    InvalidMagic,
    /// Schema version, physical profile, or fixed schema identity differed.
    UnsupportedSchema,
    /// Disposition, presence, transport, acknowledgement, or page tag was unknown.
    UnknownTag,
    /// Reserved or inactive optional bytes were nonzero.
    NonCanonicalReservedBytes,
    /// A required content identity was zero.
    ZeroIdentity,
    /// Disposition and optional Certificate/Admission presence were not canonical.
    InvalidDisposition,
    /// Capability descriptor did not select this exact Strategy record.
    DescriptorMismatch,
    /// Certificate semantic tuple differed from authenticated V3 artifacts.
    CertificateMismatch,
    /// Certificate referenced another authenticated ArtifactRelease.
    ArtifactMismatch,
    /// Admitted-AOT lacked its separately authenticated Registry admission.
    MissingAdmission,
    /// Admission did not authorize this exact Certificate and disposition.
    AdmissionMismatch,
    /// Request, acknowledgement, or scratch commitment coordinates differed.
    BindingMismatch,
    /// Checked bank-width, chunk-count, or offset arithmetic overflowed.
    ArithmeticOverflow,
    /// Inline return data was requested for a bank requiring scratch pages.
    ScratchRequired,
    /// Accelerator and interpreter acceptance or complete candidate bank diverged.
    StrategyDivergence,
}

/// Result alias for Execution Strategy V2.
pub type Result<T> = core::result::Result<T, Error>;

/// Descriptor-selected execution disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrategyDispositionV2 {
    /// Trading executes only the authenticated interpreter.
    Interpreted,
    /// Trading executes the interpreter and a stateless AOT comparator.
    ShadowAot,
    /// Trading may use the AOT result only with immutable Registry admission.
    AdmittedAot,
}

/// Exact paired accelerator transport selected by a Strategy record.
///
/// The pair is semantic authority: request and acknowledgement schemas may
/// never be mixed across profiles. Interpreted and admitted-AOT retain the
/// chunked candidate-bank V2 transport, while Shadow-AOT uses the read-only
/// transcript/digest V3 transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceleratorTransportProfileV2 {
    /// Chunked candidate-bank request and acknowledgement used by admitted AOT.
    ChunkedBankV2,
    /// Complete read-only runtime/candidate/effect comparison used by Shadow AOT.
    ShadowTranscriptV3,
}

impl StrategyDispositionV2 {
    const fn tag(self) -> u8 {
        match self {
            Self::Interpreted => DISPOSITION_INTERPRETED,
            Self::ShadowAot => DISPOSITION_SHADOW_AOT,
            Self::AdmittedAot => DISPOSITION_ADMITTED_AOT,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            DISPOSITION_INTERPRETED => Ok(Self::Interpreted),
            DISPOSITION_SHADOW_AOT => Ok(Self::ShadowAot),
            DISPOSITION_ADMITTED_AOT => Ok(Self::AdmittedAot),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Finalized acyclic strategy selected by CapabilityProgramV3 Transition fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionStrategyProgramV2 {
    disposition: StrategyDispositionV2,
    transition_schema: ContentId,
    transition_program: ContentId,
    certificate_schema: ContentId,
    certificate_program: Option<ContentId>,
    admission_schema: ContentId,
    admission_program: Option<ContentId>,
    request_schema: ContentId,
    ack_schema: ContentId,
}

impl ExecutionStrategyProgramV2 {
    /// Construct one typed Strategy after enforcing its acyclic presence grammar.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        disposition: StrategyDispositionV2,
        transition_schema: ContentId,
        transition_program: ContentId,
        certificate_schema: ContentId,
        certificate_program: Option<ContentId>,
        admission_schema: ContentId,
        admission_program: Option<ContentId>,
        request_schema: ContentId,
        ack_schema: ContentId,
    ) -> Result<Self> {
        if certificate_schema != schema_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?
            || admission_schema != schema_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?
        {
            return Err(Error::UnsupportedSchema);
        }
        let transport = transport_profile(request_schema, ack_schema)?;
        let transport_matches_disposition = match disposition {
            StrategyDispositionV2::ShadowAot => {
                transport == AcceleratorTransportProfileV2::ShadowTranscriptV3
            }
            StrategyDispositionV2::Interpreted | StrategyDispositionV2::AdmittedAot => {
                transport == AcceleratorTransportProfileV2::ChunkedBankV2
            }
        };
        if !transport_matches_disposition {
            return Err(Error::UnsupportedSchema);
        }
        let presence_is_canonical = match disposition {
            StrategyDispositionV2::Interpreted => {
                certificate_program.is_none() && admission_program.is_none()
            }
            StrategyDispositionV2::ShadowAot => {
                certificate_program.is_some() && admission_program.is_none()
            }
            StrategyDispositionV2::AdmittedAot => {
                certificate_program.is_some() && admission_program.is_some()
            }
        };
        if !presence_is_canonical {
            return Err(Error::InvalidDisposition);
        }
        Ok(Self {
            disposition,
            transition_schema,
            transition_program,
            certificate_schema,
            certificate_program,
            admission_schema,
            admission_program,
            request_schema,
            ack_schema,
        })
    }

    /// Hostile-decode one exact fixed Strategy record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
            &EXECUTION_STRATEGY_PROGRAM_MAGIC_V2,
        )?;
        require_zero(bytes, STRATEGY_HEADER_RESERVED_OFFSET_V2, 1)?;
        let disposition =
            StrategyDispositionV2::decode(byte(bytes, STRATEGY_DISPOSITION_OFFSET_V2)?)?;
        Self::new(
            disposition,
            content(bytes, STRATEGY_TRANSITION_SCHEMA_OFFSET_V2)?,
            content(bytes, STRATEGY_TRANSITION_PROGRAM_OFFSET_V2)?,
            content(bytes, STRATEGY_CERTIFICATE_SCHEMA_OFFSET_V2)?,
            optional_content(
                bytes,
                STRATEGY_CERTIFICATE_PRESENT_OFFSET_V2,
                STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2,
            )?,
            content(bytes, STRATEGY_ADMISSION_SCHEMA_OFFSET_V2)?,
            optional_content(
                bytes,
                STRATEGY_ADMISSION_PRESENT_OFFSET_V2,
                STRATEGY_ADMISSION_PROGRAM_OFFSET_V2,
            )?,
            content(bytes, STRATEGY_REQUEST_SCHEMA_OFFSET_V2)?,
            content(bytes, STRATEGY_ACK_SCHEMA_OFFSET_V2)?,
        )
    }

    /// Encode exact canonical Strategy bytes.
    pub fn to_bytes(self) -> [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2] {
        let mut output = [0_u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2];
        write_header(&mut output, &EXECUTION_STRATEGY_PROGRAM_MAGIC_V2);
        put_byte(
            &mut output,
            STRATEGY_DISPOSITION_OFFSET_V2,
            self.disposition.tag(),
        );
        put_optional(
            &mut output,
            STRATEGY_CERTIFICATE_PRESENT_OFFSET_V2,
            STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2,
            self.certificate_program,
        );
        put_optional(
            &mut output,
            STRATEGY_ADMISSION_PRESENT_OFFSET_V2,
            STRATEGY_ADMISSION_PROGRAM_OFFSET_V2,
            self.admission_program,
        );
        for (offset, value) in [
            (STRATEGY_TRANSITION_SCHEMA_OFFSET_V2, self.transition_schema),
            (
                STRATEGY_TRANSITION_PROGRAM_OFFSET_V2,
                self.transition_program,
            ),
            (
                STRATEGY_CERTIFICATE_SCHEMA_OFFSET_V2,
                self.certificate_schema,
            ),
            (STRATEGY_ADMISSION_SCHEMA_OFFSET_V2, self.admission_schema),
            (STRATEGY_REQUEST_SCHEMA_OFFSET_V2, self.request_schema),
            (STRATEGY_ACK_SCHEMA_OFFSET_V2, self.ack_schema),
        ] {
            put(&mut output, offset, value.as_bytes());
        }
        output
    }

    /// Require CapabilityProgramV3 to select this Strategy schema and content.
    pub fn validate_descriptor_selection(
        self,
        strategy_program: ContentId,
        descriptor: CapabilityProgramV3,
    ) -> Result<()> {
        let _ = self;
        if descriptor.transition_schema() == schema_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?
            && descriptor.transition_program() == strategy_program
        {
            Ok(())
        } else {
            Err(Error::DescriptorMismatch)
        }
    }

    /// Require CapabilityProgramV4 to select this Strategy and its exact
    /// underlying Transition schema/content pair.
    pub fn validate_descriptor_selection_v4(
        self,
        strategy_program: ContentId,
        descriptor: CapabilityProgramV4,
    ) -> Result<()> {
        if descriptor.strategy().schema() == schema_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?
            && descriptor.strategy().program() == strategy_program
            && descriptor.transition().schema() == self.transition_schema
            && descriptor.transition().program() == self.transition_program
        {
            Ok(())
        } else {
            Err(Error::DescriptorMismatch)
        }
    }

    /// Selected interpreter, shadow comparator, or admitted-AOT disposition.
    pub const fn disposition(self) -> StrategyDispositionV2 {
        self.disposition
    }
    /// Underlying TransitionVM finalized-record schema identity.
    pub const fn transition_schema(self) -> ContentId {
        self.transition_schema
    }
    /// SHA-256 of exact underlying TransitionVM bytes.
    pub const fn transition_program(self) -> ContentId {
        self.transition_program
    }
    /// Static finalized Certificate schema identity.
    pub const fn certificate_schema(self) -> ContentId {
        self.certificate_schema
    }
    /// Optional exact Certificate content identity.
    pub const fn certificate_program(self) -> Option<ContentId> {
        self.certificate_program
    }
    /// Static finalized Admission schema identity.
    pub const fn admission_schema(self) -> ContentId {
        self.admission_schema
    }
    /// Optional exact Registry-owned Admission content identity.
    pub const fn admission_program(self) -> Option<ContentId> {
        self.admission_program
    }
    /// Exact generic accelerator-request schema identity.
    pub const fn request_schema(self) -> ContentId {
        self.request_schema
    }
    /// Exact generic accelerator-acknowledgement schema identity.
    pub const fn ack_schema(self) -> ContentId {
        self.ack_schema
    }

    /// Exact paired accelerator transport selected by this accepted record.
    pub fn transport_profile(self) -> Result<AcceleratorTransportProfileV2> {
        transport_profile(self.request_schema, self.ack_schema)
    }
}

fn transport_profile(
    request_schema: ContentId,
    ack_schema: ContentId,
) -> Result<AcceleratorTransportProfileV2> {
    let request_v2 = schema_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?;
    let ack_v2 = schema_id(ACCELERATOR_ACK_SCHEMA_ID_V2)?;
    let request_v3 = schema_id(SHADOW_REQUEST_SCHEMA_ID_V3)?;
    let ack_v3 = schema_id(SHADOW_ACK_SCHEMA_ID_V3)?;
    match (request_schema, ack_schema) {
        (request, ack) if request == request_v2 && ack == ack_v2 => {
            Ok(AcceleratorTransportProfileV2::ChunkedBankV2)
        }
        (request, ack) if request == request_v3 && ack == ack_v3 => {
            Ok(AcceleratorTransportProfileV2::ShadowTranscriptV3)
        }
        _ => Err(Error::UnsupportedSchema),
    }
}

/// Immutable semantic-equivalence tuple for one stateless AOT ArtifactRelease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionStrategyCertificateV2 {
    account_profile_program: ContentId,
    request_profile_schema: ContentId,
    request_profile_program: ContentId,
    transition_schema: ContentId,
    transition_program: ContentId,
    effect_program: ContentId,
    artifact_release: ArtifactReleaseIdV1,
    compiler_release: ContentId,
    toolchain: ContentId,
    translation_validation: ContentId,
}

impl ExecutionStrategyCertificateV2 {
    /// Construct one typed Certificate without adding parent back-edges.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        account_profile_program: ContentId,
        request_profile_schema: ContentId,
        request_profile_program: ContentId,
        transition_schema: ContentId,
        transition_program: ContentId,
        effect_program: ContentId,
        artifact_release: ArtifactReleaseIdV1,
        compiler_release: ContentId,
        toolchain: ContentId,
        translation_validation: ContentId,
    ) -> Self {
        Self {
            account_profile_program,
            request_profile_schema,
            request_profile_program,
            transition_schema,
            transition_program,
            effect_program,
            artifact_release,
            compiler_release,
            toolchain,
            translation_validation,
        }
    }

    /// Hostile-decode one exact Certificate.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2,
            &EXECUTION_STRATEGY_CERTIFICATE_MAGIC_V2,
        )?;
        require_zero(bytes, CERTIFICATE_RESERVED_OFFSET_V2, 4)?;
        Ok(Self::new(
            content(bytes, CERTIFICATE_ACCOUNT_PROFILE_PROGRAM_OFFSET_V2)?,
            content(bytes, CERTIFICATE_REQUEST_PROFILE_SCHEMA_OFFSET_V2)?,
            content(bytes, CERTIFICATE_REQUEST_PROFILE_PROGRAM_OFFSET_V2)?,
            content(bytes, CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2)?,
            content(bytes, CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2)?,
            content(bytes, CERTIFICATE_EFFECT_PROGRAM_OFFSET_V2)?,
            ArtifactReleaseIdV1::decode(slice(bytes, CERTIFICATE_ARTIFACT_RELEASE_OFFSET_V2, 32)?)
                .map_err(|_| Error::ZeroIdentity)?,
            content(bytes, CERTIFICATE_COMPILER_RELEASE_OFFSET_V2)?,
            content(bytes, CERTIFICATE_TOOLCHAIN_OFFSET_V2)?,
            content(bytes, CERTIFICATE_TRANSLATION_VALIDATION_OFFSET_V2)?,
        ))
    }

    /// Encode exact canonical Certificate bytes.
    pub fn to_bytes(self) -> [u8; EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2] {
        let mut output = [0_u8; EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2];
        write_header(&mut output, &EXECUTION_STRATEGY_CERTIFICATE_MAGIC_V2);
        for (offset, value) in [
            (
                CERTIFICATE_ACCOUNT_PROFILE_PROGRAM_OFFSET_V2,
                self.account_profile_program,
            ),
            (
                CERTIFICATE_REQUEST_PROFILE_SCHEMA_OFFSET_V2,
                self.request_profile_schema,
            ),
            (
                CERTIFICATE_REQUEST_PROFILE_PROGRAM_OFFSET_V2,
                self.request_profile_program,
            ),
            (
                CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2,
                self.transition_schema,
            ),
            (
                CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2,
                self.transition_program,
            ),
            (CERTIFICATE_EFFECT_PROGRAM_OFFSET_V2, self.effect_program),
            (
                CERTIFICATE_COMPILER_RELEASE_OFFSET_V2,
                self.compiler_release,
            ),
            (CERTIFICATE_TOOLCHAIN_OFFSET_V2, self.toolchain),
            (
                CERTIFICATE_TRANSLATION_VALIDATION_OFFSET_V2,
                self.translation_validation,
            ),
        ] {
            put(&mut output, offset, value.as_bytes());
        }
        put(
            &mut output,
            CERTIFICATE_ARTIFACT_RELEASE_OFFSET_V2,
            self.artifact_release.as_bytes(),
        );
        output
    }

    /// Join the Certificate tuple to authenticated V3 descriptor and artifacts.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_v3(
        self,
        certificate_program: ContentId,
        strategy_program: ContentId,
        strategy: ExecutionStrategyProgramV2,
        descriptor: CapabilityProgramV3,
        artifacts: AuthenticatedInterpreterArtifactsV2,
    ) -> Result<()> {
        strategy.validate_descriptor_selection(strategy_program, descriptor)?;
        if strategy.certificate_program != Some(certificate_program)
            || self.account_profile_program != descriptor.account_profile()
            || self.request_profile_schema != descriptor.request_profile_schema()
            || self.request_profile_program != descriptor.request_profile_program()
            || self.transition_schema != strategy.transition_schema
            || self.transition_program != strategy.transition_program
            || self.effect_program != descriptor.effect_program()
            || self.account_profile_program != artifacts.account_profile_program
            || self.request_profile_schema != artifacts.request_profile_schema
            || self.request_profile_program != artifacts.request_profile_program
            || self.transition_schema != artifacts.transition_schema
            || self.transition_program != artifacts.transition_program
            || self.effect_program != artifacts.effect_program
        {
            Err(Error::CertificateMismatch)
        } else {
            Ok(())
        }
    }

    /// Join the Certificate tuple to one schema-bound V4 descriptor and the
    /// independently authenticated interpreter artifact tuple.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_v4(
        self,
        certificate_program: ContentId,
        strategy_program: ContentId,
        strategy: ExecutionStrategyProgramV2,
        descriptor: CapabilityProgramV4,
        artifacts: AuthenticatedInterpreterArtifactsV2,
    ) -> Result<()> {
        strategy.validate_descriptor_selection_v4(strategy_program, descriptor)?;
        if strategy.certificate_program != Some(certificate_program)
            || self.account_profile_program != descriptor.account_profile().program()
            || self.request_profile_schema != descriptor.request_profile().schema()
            || self.request_profile_program != descriptor.request_profile().program()
            || self.transition_schema != descriptor.transition().schema()
            || self.transition_program != descriptor.transition().program()
            || self.effect_program != descriptor.effect().program()
            || self.account_profile_program != artifacts.account_profile_program
            || self.request_profile_schema != artifacts.request_profile_schema
            || self.request_profile_program != artifacts.request_profile_program
            || self.transition_schema != artifacts.transition_schema
            || self.transition_program != artifacts.transition_program
            || self.effect_program != artifacts.effect_program
        {
            Err(Error::CertificateMismatch)
        } else {
            Ok(())
        }
    }

    /// Require the separately Registry-authenticated ArtifactRelease identity.
    pub fn validate_artifact(self, authenticated: ArtifactReleaseIdV1) -> Result<()> {
        if self.artifact_release == authenticated {
            Ok(())
        } else {
            Err(Error::ArtifactMismatch)
        }
    }

    /// Referenced stateless accelerator ArtifactRelease.
    pub const fn artifact_release(self) -> ArtifactReleaseIdV1 {
        self.artifact_release
    }
    /// Compiler semantic release identity.
    pub const fn compiler_release(self) -> ContentId {
        self.compiler_release
    }
    /// Exact compiler/toolchain build-manifest digest.
    pub const fn toolchain(self) -> ContentId {
        self.toolchain
    }
    /// Exact translation-validation theorem/corpus identity.
    pub const fn translation_validation(self) -> ContentId {
        self.translation_validation
    }
}

/// Adapter-authenticated finalized interpreter artifact tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedInterpreterArtifactsV2 {
    /// Exact AccountProfile content digest.
    pub account_profile_program: ContentId,
    /// Exact RequestProfile static schema identity.
    pub request_profile_schema: ContentId,
    /// Exact RequestProfile content digest.
    pub request_profile_program: ContentId,
    /// Exact underlying TransitionVM static schema identity.
    pub transition_schema: ContentId,
    /// Exact underlying TransitionVM content digest.
    pub transition_program: ContentId,
    /// Exact EffectProgram content digest.
    pub effect_program: ContentId,
}

/// Minimal immutable Registry admission of one exact Certificate for AOT-only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionStrategyAdmissionV2 {
    certificate_program: ContentId,
}

/// Private-witness result of the complete Registry-authenticated admitted chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedAotAuthorizationV2 {
    private: (),
}

impl ExecutionStrategyAdmissionV2 {
    /// Construct the sole admitted-AOT authorization fact.
    pub const fn new(certificate_program: ContentId) -> Self {
        Self {
            certificate_program,
        }
    }

    /// Hostile-decode one exact minimal Admission.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            EXECUTION_STRATEGY_ADMISSION_BYTES_V2,
            &EXECUTION_STRATEGY_ADMISSION_MAGIC_V2,
        )?;
        if byte(bytes, ADMISSION_DISPOSITION_OFFSET_V2)? != DISPOSITION_ADMITTED_AOT {
            return Err(Error::InvalidDisposition);
        }
        require_zero(bytes, ADMISSION_RESERVED_OFFSET_V2, 3)?;
        Ok(Self::new(content(
            bytes,
            ADMISSION_CERTIFICATE_PROGRAM_OFFSET_V2,
        )?))
    }

    /// Encode exact canonical Admission bytes.
    pub fn to_bytes(self) -> [u8; EXECUTION_STRATEGY_ADMISSION_BYTES_V2] {
        let mut output = [0_u8; EXECUTION_STRATEGY_ADMISSION_BYTES_V2];
        write_header(&mut output, &EXECUTION_STRATEGY_ADMISSION_MAGIC_V2);
        put_byte(
            &mut output,
            ADMISSION_DISPOSITION_OFFSET_V2,
            DISPOSITION_ADMITTED_AOT,
        );
        put(
            &mut output,
            ADMISSION_CERTIFICATE_PROGRAM_OFFSET_V2,
            self.certificate_program.as_bytes(),
        );
        output
    }

    /// Exact Certificate content identity authorized by Registry.
    pub const fn certificate_program(self) -> ContentId {
        self.certificate_program
    }
}

/// Require the complete admitted-AOT chain after Registry finalized-record auth.
///
/// The caller must authenticate Admission and Certificate bytes as immutable
/// Registry raw records under their static schemas before calling this join.
/// A Certificate alone can never satisfy this function.
#[allow(clippy::too_many_arguments)]
pub fn validate_admitted_aot_v2(
    strategy_program: ContentId,
    strategy: ExecutionStrategyProgramV2,
    descriptor: CapabilityProgramV3,
    certificate_program: ContentId,
    certificate: ExecutionStrategyCertificateV2,
    artifacts: AuthenticatedInterpreterArtifactsV2,
    authenticated_artifact: ArtifactReleaseIdV1,
    authenticated_admission: Option<(ContentId, ExecutionStrategyAdmissionV2)>,
) -> Result<AdmittedAotAuthorizationV2> {
    if strategy.disposition != StrategyDispositionV2::AdmittedAot {
        return Err(Error::InvalidDisposition);
    }
    let (admission_program, admission) = authenticated_admission.ok_or(Error::MissingAdmission)?;
    if strategy.certificate_program != Some(certificate_program)
        || strategy.admission_program != Some(admission_program)
        || admission.certificate_program != certificate_program
    {
        return Err(Error::AdmissionMismatch);
    }
    certificate.validate_v3(
        certificate_program,
        strategy_program,
        strategy,
        descriptor,
        artifacts,
    )?;
    certificate.validate_artifact(authenticated_artifact)?;
    Ok(AdmittedAotAuthorizationV2 { private: () })
}

/// Require the complete admitted-AOT chain for a schema-bound V4 descriptor.
///
/// The V4 descriptor independently binds every artifact schema/content pair;
/// this join preserves the existing certificate wire while refusing any
/// Strategy or underlying Transition pair that differs from those descriptor
/// edges.
#[allow(clippy::too_many_arguments)]
pub fn validate_admitted_aot_v4(
    strategy_program: ContentId,
    strategy: ExecutionStrategyProgramV2,
    descriptor: CapabilityProgramV4,
    certificate_program: ContentId,
    certificate: ExecutionStrategyCertificateV2,
    artifacts: AuthenticatedInterpreterArtifactsV2,
    authenticated_artifact: ArtifactReleaseIdV1,
    authenticated_admission: Option<(ContentId, ExecutionStrategyAdmissionV2)>,
) -> Result<AdmittedAotAuthorizationV2> {
    if strategy.disposition != StrategyDispositionV2::AdmittedAot {
        return Err(Error::InvalidDisposition);
    }
    let (admission_program, admission) = authenticated_admission.ok_or(Error::MissingAdmission)?;
    if strategy.certificate_program != Some(certificate_program)
        || strategy.admission_program != Some(admission_program)
        || admission.certificate_program != certificate_program
    {
        return Err(Error::AdmissionMismatch);
    }
    certificate.validate_v4(
        certificate_program,
        strategy_program,
        strategy,
        descriptor,
        artifacts,
    )?;
    certificate.validate_artifact(authenticated_artifact)?;
    Ok(AdmittedAotAuthorizationV2 { private: () })
}

/// Complete candidate bank or semantic refusal produced by one execution path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCandidateV2<'a> {
    /// The authenticated program refused the semantic transition.
    Refused,
    /// Exact complete candidate register-bank bytes.
    Accepted(&'a [u8]),
}

/// Resolve one complete candidate without giving an accelerator write authority.
///
/// Interpreted execution consumes only the interpreter result. Shadow AOT
/// requires exact acceptance and bank-byte equality, then returns the
/// interpreter result. Admitted AOT consumes only the accelerator result and a
/// private witness produced by [`validate_admitted_aot_v2`]. Trading remains
/// responsible for the single common effect projection and commit.
pub fn resolve_execution_candidate_v2<'a>(
    disposition: StrategyDispositionV2,
    interpreted: Option<ExecutionCandidateV2<'a>>,
    accelerated: Option<ExecutionCandidateV2<'a>>,
    admitted: Option<AdmittedAotAuthorizationV2>,
) -> Result<ExecutionCandidateV2<'a>> {
    match (disposition, interpreted, accelerated, admitted) {
        (StrategyDispositionV2::Interpreted, Some(candidate), None, None) => Ok(candidate),
        (StrategyDispositionV2::ShadowAot, Some(interpreted), Some(accelerated), None)
            if interpreted == accelerated =>
        {
            Ok(interpreted)
        }
        (StrategyDispositionV2::ShadowAot, Some(_), Some(_), None) => {
            Err(Error::StrategyDivergence)
        }
        (StrategyDispositionV2::AdmittedAot, None, Some(candidate), Some(_)) => Ok(candidate),
        _ => Err(Error::InvalidDisposition),
    }
}

/// Input bank transport selected only by physical byte width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BankTransportV2 {
    /// The whole bank fits one authenticated return-data chunk.
    InlineReturnData {
        /// Exact whole-bank byte width.
        bank_bytes: u64,
    },
    /// Trading assembles multiple authenticated pages without changing semantics.
    AuthenticatedScratchPages {
        /// Exact whole-bank byte width.
        bank_bytes: u64,
        /// Exact canonical page count.
        page_count: u32,
    },
}

/// Return exact scalar-then-identity bank bytes without narrowing semantic counts.
pub fn register_bank_bytes_v2(scalar_count: u32, identity_count: u32) -> Result<u64> {
    let scalars = match u64::from(scalar_count).checked_mul(8) {
        Some(value) => value,
        None => return Err(Error::ArithmeticOverflow),
    };
    let identities = match u64::from(identity_count).checked_mul(32) {
        Some(value) => value,
        None => return Err(Error::ArithmeticOverflow),
    };
    match scalars.checked_add(identities) {
        Some(value) => Ok(value),
        None => Err(Error::ArithmeticOverflow),
    }
}

/// Classify the chain-derived return-data bound without imposing a semantic N cap.
pub fn classify_bank_transport_v2(
    scalar_count: u32,
    identity_count: u32,
) -> Result<BankTransportV2> {
    let bank_bytes = register_bank_bytes_v2(scalar_count, identity_count)?;
    let page_count = chunk_count(bank_bytes)?;
    if page_count <= 1 {
        Ok(BankTransportV2::InlineReturnData { bank_bytes })
    } else {
        Ok(BankTransportV2::AuthenticatedScratchPages {
            bank_bytes,
            page_count,
        })
    }
}

/// Inline instruction bank or authenticated Trading scratch-page input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestTransportV2 {
    /// Exact full input bank follows the request header.
    Inline,
    /// Input bank is supplied through authenticated readonly scratch pages.
    ScratchPages,
}

impl RequestTransportV2 {
    const fn tag(self) -> u8 {
        match self {
            Self::Inline => TRANSPORT_INLINE,
            Self::ScratchPages => TRANSPORT_SCRATCH,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            TRANSPORT_INLINE => Ok(Self::Inline),
            TRANSPORT_SCRATCH => Ok(Self::ScratchPages),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Borrowed runtime-width accelerator request for one canonical output chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceleratorRequestV2<'a> {
    transport: RequestTransportV2,
    strategy_program: ContentId,
    certificate_program: ContentId,
    capability_program: ContentId,
    invocation_context: ContentId,
    input_bank_digest: ContentId,
    tail_count: u32,
    scalar_count: u32,
    identity_count: u32,
    chunk_index: u32,
    chunk_count: u32,
    chunk_offset: u64,
    total_bank_bytes: u64,
    inline_bank: &'a [u8],
}

impl<'a> AcceleratorRequestV2<'a> {
    /// Construct one exact inline or scratch-backed request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        transport: RequestTransportV2,
        strategy_program: ContentId,
        certificate_program: ContentId,
        capability_program: ContentId,
        invocation_context: ContentId,
        input_bank_digest: ContentId,
        tail_count: u32,
        scalar_count: u32,
        identity_count: u32,
        chunk_index: u32,
        inline_bank: &'a [u8],
    ) -> Result<Self> {
        let total_bank_bytes = register_bank_bytes_v2(scalar_count, identity_count)?;
        let (chunk_count, chunk_offset, _) = chunk_geometry(total_bank_bytes, chunk_index)?;
        match transport {
            RequestTransportV2::Inline => {
                if u64::try_from(inline_bank.len()).map_err(|_| Error::InvalidLength)?
                    != total_bank_bytes
                {
                    return Err(Error::InvalidLength);
                }
            }
            RequestTransportV2::ScratchPages => {
                if !inline_bank.is_empty() {
                    return Err(Error::NonCanonicalReservedBytes);
                }
            }
        }
        Ok(Self {
            transport,
            strategy_program,
            certificate_program,
            capability_program,
            invocation_context,
            input_bank_digest,
            tail_count,
            scalar_count,
            identity_count,
            chunk_index,
            chunk_count,
            chunk_offset,
            total_bank_bytes,
            inline_bank,
        })
    }

    /// Hostile-decode one exact request and optional inline bank.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        require_prefix_header(
            bytes,
            ACCELERATOR_REQUEST_HEADER_BYTES_V2,
            &ACCELERATOR_REQUEST_MAGIC_V2,
        )?;
        require_zero(bytes, REQUEST_HEADER_RESERVED_OFFSET_V2, 3)?;
        require_zero(bytes, REQUEST_TAIL_RESERVED_OFFSET_V2, 12)?;
        let transport = RequestTransportV2::decode(byte(bytes, REQUEST_TRANSPORT_OFFSET_V2)?)?;
        let scalar_count = read_u32(bytes, REQUEST_SCALAR_COUNT_OFFSET_V2)?;
        let identity_count = read_u32(bytes, REQUEST_IDENTITY_COUNT_OFFSET_V2)?;
        let chunk_index = read_u32(bytes, REQUEST_CHUNK_INDEX_OFFSET_V2)?;
        let value = Self::new(
            transport,
            content(bytes, REQUEST_STRATEGY_PROGRAM_OFFSET_V2)?,
            content(bytes, REQUEST_CERTIFICATE_PROGRAM_OFFSET_V2)?,
            content(bytes, REQUEST_CAPABILITY_PROGRAM_OFFSET_V2)?,
            content(bytes, REQUEST_INVOCATION_CONTEXT_OFFSET_V2)?,
            content(bytes, REQUEST_INPUT_BANK_DIGEST_OFFSET_V2)?,
            read_u32(bytes, REQUEST_TAIL_COUNT_OFFSET_V2)?,
            scalar_count,
            identity_count,
            chunk_index,
            match transport {
                RequestTransportV2::Inline => bytes
                    .get(ACCELERATOR_REQUEST_HEADER_BYTES_V2..)
                    .ok_or(Error::InvalidLength)?,
                RequestTransportV2::ScratchPages => &[],
            },
        )?;
        if read_u32(bytes, REQUEST_CHUNK_COUNT_OFFSET_V2)? != value.chunk_count
            || read_u64(bytes, REQUEST_CHUNK_OFFSET_OFFSET_V2)? != value.chunk_offset
            || read_u64(bytes, REQUEST_TOTAL_BANK_BYTES_OFFSET_V2)? != value.total_bank_bytes
            || (transport == RequestTransportV2::ScratchPages
                && bytes.len() != ACCELERATOR_REQUEST_HEADER_BYTES_V2)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(value)
    }

    /// Encode exact request header and optional inline input bank.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = ACCELERATOR_REQUEST_HEADER_BYTES_V2
            .checked_add(self.inline_bank.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != expected {
            return Err(Error::InvalidLength);
        }
        let canonical = Self::new(
            self.transport,
            self.strategy_program,
            self.certificate_program,
            self.capability_program,
            self.invocation_context,
            self.input_bank_digest,
            self.tail_count,
            self.scalar_count,
            self.identity_count,
            self.chunk_index,
            self.inline_bank,
        )?;
        if canonical != self {
            return Err(Error::BindingMismatch);
        }
        output.fill(0);
        write_header(output, &ACCELERATOR_REQUEST_MAGIC_V2);
        put_byte(output, REQUEST_TRANSPORT_OFFSET_V2, self.transport.tag());
        for (offset, value) in [
            (REQUEST_STRATEGY_PROGRAM_OFFSET_V2, self.strategy_program),
            (
                REQUEST_CERTIFICATE_PROGRAM_OFFSET_V2,
                self.certificate_program,
            ),
            (
                REQUEST_CAPABILITY_PROGRAM_OFFSET_V2,
                self.capability_program,
            ),
            (
                REQUEST_INVOCATION_CONTEXT_OFFSET_V2,
                self.invocation_context,
            ),
            (REQUEST_INPUT_BANK_DIGEST_OFFSET_V2, self.input_bank_digest),
        ] {
            put(output, offset, value.as_bytes());
        }
        put_u32(output, REQUEST_TAIL_COUNT_OFFSET_V2, self.tail_count);
        put_u32(output, REQUEST_SCALAR_COUNT_OFFSET_V2, self.scalar_count);
        put_u32(
            output,
            REQUEST_IDENTITY_COUNT_OFFSET_V2,
            self.identity_count,
        );
        put_u32(output, REQUEST_CHUNK_INDEX_OFFSET_V2, self.chunk_index);
        put_u32(output, REQUEST_CHUNK_COUNT_OFFSET_V2, self.chunk_count);
        put_u64(output, REQUEST_CHUNK_OFFSET_OFFSET_V2, self.chunk_offset);
        put_u64(
            output,
            REQUEST_TOTAL_BANK_BYTES_OFFSET_V2,
            self.total_bank_bytes,
        );
        put(
            output,
            ACCELERATOR_REQUEST_HEADER_BYTES_V2,
            self.inline_bank,
        );
        Ok(())
    }

    /// Input transport chosen by the caller after authenticated bank sizing.
    pub const fn transport(self) -> RequestTransportV2 {
        self.transport
    }
    /// Exact finalized Strategy content identity.
    pub const fn strategy_program(self) -> ContentId {
        self.strategy_program
    }
    /// Exact finalized Certificate content identity.
    pub const fn certificate_program(self) -> ContentId {
        self.certificate_program
    }
    /// Exact CapabilityProgramV3 descriptor content identity.
    pub const fn capability_program(self) -> ContentId {
        self.capability_program
    }
    /// Exact invocation-context digest.
    pub const fn invocation_context(self) -> ContentId {
        self.invocation_context
    }
    /// Exact input-bank digest.
    pub const fn input_bank_digest(self) -> ContentId {
        self.input_bank_digest
    }
    /// Product-authoritative semantic tail count.
    pub const fn tail_count(self) -> u32 {
        self.tail_count
    }
    /// Runtime scalar count.
    pub const fn scalar_count(self) -> u32 {
        self.scalar_count
    }
    /// Runtime identity count.
    pub const fn identity_count(self) -> u32 {
        self.identity_count
    }
    /// Canonical output chunk index.
    pub const fn chunk_index(self) -> u32 {
        self.chunk_index
    }
    /// Exact output chunk count.
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }
    /// Exact output chunk byte offset.
    pub const fn chunk_offset(self) -> u64 {
        self.chunk_offset
    }
    /// Exact whole-bank byte width.
    pub const fn total_bank_bytes(self) -> u64 {
        self.total_bank_bytes
    }
    /// Borrow exact inline input bank, empty for scratch transport.
    pub const fn inline_bank(self) -> &'a [u8] {
        self.inline_bank
    }
}

/// Accelerator acknowledgement disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceleratorDispositionV2 {
    /// Semantic refusal with no candidate chunk.
    Refused,
    /// Accepted candidate chunk.
    Accepted,
}

/// Borrowed exact return-data acknowledgement for one output chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceleratorAckV2<'a> {
    disposition: AcceleratorDispositionV2,
    request_digest: ContentId,
    invocation_context: ContentId,
    total_bank_digest: Option<ContentId>,
    total_bank_bytes: u64,
    chunk_index: u32,
    chunk_count: u32,
    chunk_offset: u64,
    payload: &'a [u8],
}

impl<'a> AcceleratorAckV2<'a> {
    /// Construct one accepted canonical candidate chunk.
    pub fn accepted(
        request: AcceleratorRequestV2<'_>,
        request_digest: ContentId,
        total_bank_digest: ContentId,
        payload: &'a [u8],
    ) -> Result<Self> {
        let (_, _, expected_payload) =
            chunk_geometry(request.total_bank_bytes, request.chunk_index)?;
        if payload.len() != expected_payload {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            disposition: AcceleratorDispositionV2::Accepted,
            request_digest,
            invocation_context: request.invocation_context,
            total_bank_digest: Some(total_bank_digest),
            total_bank_bytes: request.total_bank_bytes,
            chunk_index: request.chunk_index,
            chunk_count: request.chunk_count,
            chunk_offset: request.chunk_offset,
            payload,
        })
    }

    /// Construct one canonical semantic refusal with no candidate bank.
    pub const fn refused(request: AcceleratorRequestV2<'_>, request_digest: ContentId) -> Self {
        Self {
            disposition: AcceleratorDispositionV2::Refused,
            request_digest,
            invocation_context: request.invocation_context,
            total_bank_digest: None,
            total_bank_bytes: 0,
            chunk_index: 0,
            chunk_count: 0,
            chunk_offset: 0,
            payload: &[],
        }
    }

    /// Hostile-decode one exact acknowledgement within the SVM return-data bound.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < ACCELERATOR_ACK_HEADER_BYTES_V2 || bytes.len() > SVM_RETURN_DATA_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        require_prefix_header(
            bytes,
            ACCELERATOR_ACK_HEADER_BYTES_V2,
            &ACCELERATOR_ACK_MAGIC_V2,
        )?;
        require_zero(bytes, ACK_HEADER_RESERVED_OFFSET_V2, 3)?;
        require_zero(bytes, ACK_TAIL_RESERVED_OFFSET_V2, 6)?;
        let payload = bytes
            .get(ACCELERATOR_ACK_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?;
        if usize::from(read_u16(bytes, ACK_PAYLOAD_BYTES_OFFSET_V2)?) != payload.len() {
            return Err(Error::InvalidLength);
        }
        let disposition = match byte(bytes, ACK_DISPOSITION_OFFSET_V2)? {
            ACK_REFUSED => AcceleratorDispositionV2::Refused,
            ACK_ACCEPTED => AcceleratorDispositionV2::Accepted,
            _ => return Err(Error::UnknownTag),
        };
        let digest_bytes = read_array(bytes, ACK_TOTAL_BANK_DIGEST_OFFSET_V2)?;
        let total_bank_digest = match disposition {
            AcceleratorDispositionV2::Accepted => {
                Some(ContentId::new(digest_bytes).map_err(|_| Error::ZeroIdentity)?)
            }
            AcceleratorDispositionV2::Refused => {
                if digest_bytes != [0; 32] {
                    return Err(Error::NonCanonicalReservedBytes);
                }
                None
            }
        };
        let value = Self {
            disposition,
            request_digest: content(bytes, ACK_REQUEST_DIGEST_OFFSET_V2)?,
            invocation_context: content(bytes, ACK_INVOCATION_CONTEXT_OFFSET_V2)?,
            total_bank_digest,
            total_bank_bytes: read_u64(bytes, ACK_TOTAL_BANK_BYTES_OFFSET_V2)?,
            chunk_index: read_u32(bytes, ACK_CHUNK_INDEX_OFFSET_V2)?,
            chunk_count: read_u32(bytes, ACK_CHUNK_COUNT_OFFSET_V2)?,
            chunk_offset: read_u64(bytes, ACK_CHUNK_OFFSET_OFFSET_V2)?,
            payload,
        };
        value.validate_canonical()?;
        Ok(value)
    }

    /// Encode one exact acknowledgement into caller-owned return-data bytes.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        self.validate_canonical()?;
        let expected = ACCELERATOR_ACK_HEADER_BYTES_V2
            .checked_add(self.payload.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != expected || output.len() > SVM_RETURN_DATA_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        write_header(output, &ACCELERATOR_ACK_MAGIC_V2);
        put_byte(
            output,
            ACK_DISPOSITION_OFFSET_V2,
            match self.disposition {
                AcceleratorDispositionV2::Refused => ACK_REFUSED,
                AcceleratorDispositionV2::Accepted => ACK_ACCEPTED,
            },
        );
        put(
            output,
            ACK_REQUEST_DIGEST_OFFSET_V2,
            self.request_digest.as_bytes(),
        );
        put(
            output,
            ACK_INVOCATION_CONTEXT_OFFSET_V2,
            self.invocation_context.as_bytes(),
        );
        if let Some(digest) = self.total_bank_digest {
            put(output, ACK_TOTAL_BANK_DIGEST_OFFSET_V2, digest.as_bytes());
        }
        put_u64(
            output,
            ACK_TOTAL_BANK_BYTES_OFFSET_V2,
            self.total_bank_bytes,
        );
        put_u32(output, ACK_CHUNK_INDEX_OFFSET_V2, self.chunk_index);
        put_u32(output, ACK_CHUNK_COUNT_OFFSET_V2, self.chunk_count);
        put_u64(output, ACK_CHUNK_OFFSET_OFFSET_V2, self.chunk_offset);
        put_u16(
            output,
            ACK_PAYLOAD_BYTES_OFFSET_V2,
            u16::try_from(self.payload.len()).map_err(|_| Error::InvalidLength)?,
        );
        put(output, ACCELERATOR_ACK_HEADER_BYTES_V2, self.payload);
        Ok(())
    }

    /// Require the exact request digest, context, and chunk coordinates.
    pub fn validate_request(
        self,
        request: AcceleratorRequestV2<'_>,
        request_digest: ContentId,
    ) -> Result<()> {
        if self.request_digest != request_digest
            || self.invocation_context != request.invocation_context
            || (self.disposition == AcceleratorDispositionV2::Accepted
                && (self.total_bank_bytes != request.total_bank_bytes
                    || self.chunk_index != request.chunk_index
                    || self.chunk_count != request.chunk_count
                    || self.chunk_offset != request.chunk_offset))
        {
            Err(Error::BindingMismatch)
        } else {
            Ok(())
        }
    }

    fn validate_canonical(self) -> Result<()> {
        match self.disposition {
            AcceleratorDispositionV2::Refused => {
                if self.total_bank_digest.is_some()
                    || self.total_bank_bytes != 0
                    || self.chunk_index != 0
                    || self.chunk_count != 0
                    || self.chunk_offset != 0
                    || !self.payload.is_empty()
                {
                    Err(Error::NonCanonicalReservedBytes)
                } else {
                    Ok(())
                }
            }
            AcceleratorDispositionV2::Accepted => {
                let (count, offset, payload) =
                    chunk_geometry(self.total_bank_bytes, self.chunk_index)?;
                if self.total_bank_digest.is_none()
                    || self.chunk_count != count
                    || self.chunk_offset != offset
                    || self.payload.len() != payload
                {
                    Err(Error::BindingMismatch)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Accepted or refused disposition.
    pub const fn disposition(self) -> AcceleratorDispositionV2 {
        self.disposition
    }
    /// SHA-256 of the exact accelerator request bytes.
    pub const fn request_digest(self) -> ContentId {
        self.request_digest
    }
    /// Exact invocation-context digest.
    pub const fn invocation_context(self) -> ContentId {
        self.invocation_context
    }
    /// Whole accepted-bank digest, absent on refusal.
    pub const fn total_bank_digest(self) -> Option<ContentId> {
        self.total_bank_digest
    }
    /// Exact complete candidate-bank width, zero for refusal.
    pub const fn total_bank_bytes(self) -> u64 {
        self.total_bank_bytes
    }
    /// Canonical candidate chunk index, zero for refusal.
    pub const fn chunk_index(self) -> u32 {
        self.chunk_index
    }
    /// Canonical candidate chunk count, zero for refusal.
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }
    /// Canonical candidate chunk byte offset, zero for refusal.
    pub const fn chunk_offset(self) -> u64 {
        self.chunk_offset
    }
    /// Borrow the exact accepted chunk payload.
    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }
}

/// Trading-owned authenticated scratch-page kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScratchPageKindV2 {
    /// Canonical projected interpreter/AOT input bank.
    Input,
    /// Candidate AOT output assembled by Trading.
    Candidate,
}

impl ScratchPageKindV2 {
    const fn tag(self) -> u8 {
        match self {
            Self::Input => SCRATCH_INPUT,
            Self::Candidate => SCRATCH_CANDIDATE,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            SCRATCH_INPUT => Ok(Self::Input),
            SCRATCH_CANDIDATE => Ok(Self::Candidate),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Borrowed exact Trading-owned scratch page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedScratchPageV2<'a> {
    kind: ScratchPageKindV2,
    trading_program: ContentId,
    strategy_program: ContentId,
    invocation_context: ContentId,
    total_bank_digest: ContentId,
    tail_count: u32,
    scalar_count: u32,
    identity_count: u32,
    chunk_index: u32,
    chunk_count: u32,
    chunk_offset: u64,
    total_bank_bytes: u64,
    payload: &'a [u8],
}

impl<'a> AuthenticatedScratchPageV2<'a> {
    /// Construct one exact canonical page commitment.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: ScratchPageKindV2,
        trading_program: ContentId,
        strategy_program: ContentId,
        invocation_context: ContentId,
        total_bank_digest: ContentId,
        tail_count: u32,
        scalar_count: u32,
        identity_count: u32,
        chunk_index: u32,
        payload: &'a [u8],
    ) -> Result<Self> {
        let total_bank_bytes = register_bank_bytes_v2(scalar_count, identity_count)?;
        let (chunk_count, chunk_offset, expected_payload) =
            chunk_geometry(total_bank_bytes, chunk_index)?;
        if payload.len() != expected_payload {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            kind,
            trading_program,
            strategy_program,
            invocation_context,
            total_bank_digest,
            tail_count,
            scalar_count,
            identity_count,
            chunk_index,
            chunk_count,
            chunk_offset,
            total_bank_bytes,
            payload,
        })
    }

    /// Hostile-decode one exact scratch page.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        require_prefix_header(bytes, SCRATCH_PAGE_HEADER_BYTES_V2, &SCRATCH_PAGE_MAGIC_V2)?;
        require_zero(bytes, SCRATCH_HEADER_RESERVED_OFFSET_V2, 3)?;
        require_zero(bytes, SCRATCH_TAIL_RESERVED_OFFSET_V2, 10)?;
        let payload = bytes
            .get(SCRATCH_PAGE_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?;
        if usize::from(read_u16(bytes, SCRATCH_PAYLOAD_BYTES_OFFSET_V2)?) != payload.len() {
            return Err(Error::InvalidLength);
        }
        let value = Self::new(
            ScratchPageKindV2::decode(byte(bytes, SCRATCH_KIND_OFFSET_V2)?)?,
            content(bytes, SCRATCH_TRADING_PROGRAM_OFFSET_V2)?,
            content(bytes, SCRATCH_STRATEGY_PROGRAM_OFFSET_V2)?,
            content(bytes, SCRATCH_INVOCATION_CONTEXT_OFFSET_V2)?,
            content(bytes, SCRATCH_TOTAL_BANK_DIGEST_OFFSET_V2)?,
            read_u32(bytes, SCRATCH_TAIL_COUNT_OFFSET_V2)?,
            read_u32(bytes, SCRATCH_SCALAR_COUNT_OFFSET_V2)?,
            read_u32(bytes, SCRATCH_IDENTITY_COUNT_OFFSET_V2)?,
            read_u32(bytes, SCRATCH_CHUNK_INDEX_OFFSET_V2)?,
            payload,
        )?;
        if read_u32(bytes, SCRATCH_CHUNK_COUNT_OFFSET_V2)? != value.chunk_count
            || read_u64(bytes, SCRATCH_CHUNK_OFFSET_OFFSET_V2)? != value.chunk_offset
            || read_u64(bytes, SCRATCH_TOTAL_BANK_BYTES_OFFSET_V2)? != value.total_bank_bytes
        {
            return Err(Error::BindingMismatch);
        }
        Ok(value)
    }

    /// Encode one exact page into caller-owned Trading account bytes.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = SCRATCH_PAGE_HEADER_BYTES_V2
            .checked_add(self.payload.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != expected {
            return Err(Error::InvalidLength);
        }
        let canonical = Self::new(
            self.kind,
            self.trading_program,
            self.strategy_program,
            self.invocation_context,
            self.total_bank_digest,
            self.tail_count,
            self.scalar_count,
            self.identity_count,
            self.chunk_index,
            self.payload,
        )?;
        if canonical != self {
            return Err(Error::BindingMismatch);
        }
        output.fill(0);
        write_header(output, &SCRATCH_PAGE_MAGIC_V2);
        put_byte(output, SCRATCH_KIND_OFFSET_V2, self.kind.tag());
        for (offset, value) in [
            (SCRATCH_TRADING_PROGRAM_OFFSET_V2, self.trading_program),
            (SCRATCH_STRATEGY_PROGRAM_OFFSET_V2, self.strategy_program),
            (
                SCRATCH_INVOCATION_CONTEXT_OFFSET_V2,
                self.invocation_context,
            ),
            (SCRATCH_TOTAL_BANK_DIGEST_OFFSET_V2, self.total_bank_digest),
        ] {
            put(output, offset, value.as_bytes());
        }
        put_u32(output, SCRATCH_TAIL_COUNT_OFFSET_V2, self.tail_count);
        put_u32(output, SCRATCH_SCALAR_COUNT_OFFSET_V2, self.scalar_count);
        put_u32(
            output,
            SCRATCH_IDENTITY_COUNT_OFFSET_V2,
            self.identity_count,
        );
        put_u32(output, SCRATCH_CHUNK_INDEX_OFFSET_V2, self.chunk_index);
        put_u32(output, SCRATCH_CHUNK_COUNT_OFFSET_V2, self.chunk_count);
        put_u64(output, SCRATCH_CHUNK_OFFSET_OFFSET_V2, self.chunk_offset);
        put_u64(
            output,
            SCRATCH_TOTAL_BANK_BYTES_OFFSET_V2,
            self.total_bank_bytes,
        );
        put_u16(
            output,
            SCRATCH_PAYLOAD_BYTES_OFFSET_V2,
            u16::try_from(self.payload.len()).map_err(|_| Error::InvalidLength)?,
        );
        put(output, SCRATCH_PAGE_HEADER_BYTES_V2, self.payload);
        Ok(())
    }

    /// Require this page to be one exact input page for an accelerator request.
    pub fn validate_request_input(
        self,
        trading_program: ContentId,
        request: AcceleratorRequestV2<'_>,
    ) -> Result<()> {
        if request.transport != RequestTransportV2::ScratchPages
            || self.kind != ScratchPageKindV2::Input
            || self.trading_program != trading_program
            || self.strategy_program != request.strategy_program
            || self.invocation_context != request.invocation_context
            || self.total_bank_digest != request.input_bank_digest
            || self.tail_count != request.tail_count
            || self.scalar_count != request.scalar_count
            || self.identity_count != request.identity_count
        {
            Err(Error::BindingMismatch)
        } else {
            Ok(())
        }
    }

    /// Canonical page kind.
    pub const fn kind(self) -> ScratchPageKindV2 {
        self.kind
    }
    /// Trading program that must own the physical page account.
    pub const fn trading_program(self) -> ContentId {
        self.trading_program
    }
    /// Exact finalized Strategy content identity.
    pub const fn strategy_program(self) -> ContentId {
        self.strategy_program
    }
    /// Exact invocation-context digest.
    pub const fn invocation_context(self) -> ContentId {
        self.invocation_context
    }
    /// Canonical page index.
    pub const fn chunk_index(self) -> u32 {
        self.chunk_index
    }
    /// Whole-bank digest shared by every page.
    pub const fn total_bank_digest(self) -> ContentId {
        self.total_bank_digest
    }
    /// Product-authoritative semantic tail count.
    pub const fn tail_count(self) -> u32 {
        self.tail_count
    }
    /// Runtime scalar count.
    pub const fn scalar_count(self) -> u32 {
        self.scalar_count
    }
    /// Runtime identity count.
    pub const fn identity_count(self) -> u32 {
        self.identity_count
    }
    /// Exact canonical page count.
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }
    /// Exact canonical page offset.
    pub const fn chunk_offset(self) -> u64 {
        self.chunk_offset
    }
    /// Exact whole-bank byte width.
    pub const fn total_bank_bytes(self) -> u64 {
        self.total_bank_bytes
    }
    /// Borrow exact page payload.
    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }
}

/// Validate a complete ordered, unmixed scratch-page sequence without allocation.
#[allow(clippy::too_many_arguments)]
pub fn validate_scratch_page_sequence_v2(
    pages: &[AuthenticatedScratchPageV2<'_>],
    kind: ScratchPageKindV2,
    trading_program: ContentId,
    strategy_program: ContentId,
    invocation_context: ContentId,
    total_bank_digest: ContentId,
    tail_count: u32,
    scalar_count: u32,
    identity_count: u32,
) -> Result<()> {
    let total_bank_bytes = register_bank_bytes_v2(scalar_count, identity_count)?;
    let expected_count = chunk_count(total_bank_bytes)?;
    if pages.len() != usize::try_from(expected_count).map_err(|_| Error::InvalidLength)? {
        return Err(Error::InvalidLength);
    }
    let mut index = 0_u32;
    while index < expected_count {
        let page = *pages
            .get(usize::try_from(index).map_err(|_| Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)?;
        if page.kind != kind
            || page.trading_program != trading_program
            || page.strategy_program != strategy_program
            || page.invocation_context != invocation_context
            || page.total_bank_digest != total_bank_digest
            || page.tail_count != tail_count
            || page.scalar_count != scalar_count
            || page.identity_count != identity_count
            || page.chunk_index != index
            || page.chunk_count != expected_count
            || page.total_bank_bytes != total_bank_bytes
        {
            return Err(Error::BindingMismatch);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn chunk_count(total_bank_bytes: u64) -> Result<u32> {
    if total_bank_bytes == 0 {
        return Ok(0);
    }
    let payload =
        u64::try_from(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2).map_err(|_| Error::ArithmeticOverflow)?;
    let count = total_bank_bytes
        .checked_add(payload.checked_sub(1).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?
        / payload;
    u32::try_from(count).map_err(|_| Error::ArithmeticOverflow)
}

fn chunk_geometry(total_bank_bytes: u64, chunk_index: u32) -> Result<(u32, u64, usize)> {
    let count = chunk_count(total_bank_bytes)?;
    if chunk_index >= count {
        return Err(Error::BindingMismatch);
    }
    let payload =
        u64::try_from(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2).map_err(|_| Error::ArithmeticOverflow)?;
    let offset = u64::from(chunk_index)
        .checked_mul(payload)
        .ok_or(Error::ArithmeticOverflow)?;
    let remaining = total_bank_bytes
        .checked_sub(offset)
        .ok_or(Error::ArithmeticOverflow)?;
    let width = if remaining < payload {
        remaining
    } else {
        payload
    };
    Ok((
        count,
        offset,
        usize::try_from(width).map_err(|_| Error::ArithmeticOverflow)?,
    ))
}

fn schema_id(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::ZeroIdentity)
}

fn optional_content(
    bytes: &[u8],
    presence_offset: usize,
    value_offset: usize,
) -> Result<Option<ContentId>> {
    match byte(bytes, presence_offset)? {
        ABSENT => {
            require_zero(bytes, value_offset, 32)?;
            Ok(None)
        }
        PRESENT => Ok(Some(content(bytes, value_offset)?)),
        _ => Err(Error::UnknownTag),
    }
}

fn require_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    require_prefix_header(bytes, width, magic)
}

fn require_prefix_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<()> {
    if bytes.len() < width {
        return Err(Error::InvalidLength);
    }
    if slice(bytes, 0, 8)? != magic {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != EXECUTION_STRATEGY_SCHEMA_VERSION_V2
        || read_u16(bytes, 10)? != EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2
    {
        return Err(Error::UnsupportedSchema);
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

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
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
    if slice(bytes, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonCanonicalReservedBytes)
    }
}

fn write_header(output: &mut [u8], magic: &[u8; 8]) {
    put(output, 0, magic);
    put_u16(output, 8, EXECUTION_STRATEGY_SCHEMA_VERSION_V2);
    put_u16(output, 10, EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2);
}

fn put_optional(
    output: &mut [u8],
    presence_offset: usize,
    value_offset: usize,
    value: Option<ContentId>,
) {
    if let Some(value) = value {
        put_byte(output, presence_offset, PRESENT);
        put(output, value_offset, value.as_bytes());
    }
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = value;
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}

const _: () = assert!(
    ACCELERATOR_ACK_HEADER_BYTES_V2 + ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2
        == SVM_RETURN_DATA_BYTES_V2
);
const _: () = assert!(capability_v3::CAPABILITY_PROGRAM_V3_BYTES == 408);

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_capability_program_contract::v4::{ArtifactReferenceV4, CapabilityArtifactsV4};
    use std::vec;

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero")
    }

    fn artifact(value: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new([value; 32]).expect("artifact")
    }

    fn strategy(disposition: StrategyDispositionV2) -> ExecutionStrategyProgramV2 {
        let certificate = match disposition {
            StrategyDispositionV2::Interpreted => None,
            StrategyDispositionV2::ShadowAot | StrategyDispositionV2::AdmittedAot => Some(id(3)),
        };
        let admission = match disposition {
            StrategyDispositionV2::AdmittedAot => Some(id(4)),
            StrategyDispositionV2::Interpreted | StrategyDispositionV2::ShadowAot => None,
        };
        let (request_schema, ack_schema) = match disposition {
            StrategyDispositionV2::ShadowAot => (
                schema_id(SHADOW_REQUEST_SCHEMA_ID_V3).expect("Shadow request schema"),
                schema_id(SHADOW_ACK_SCHEMA_ID_V3).expect("Shadow ack schema"),
            ),
            StrategyDispositionV2::Interpreted | StrategyDispositionV2::AdmittedAot => (
                schema_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("request schema"),
                schema_id(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("ack schema"),
            ),
        };
        ExecutionStrategyProgramV2::new(
            disposition,
            id(1),
            id(2),
            schema_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("schema"),
            certificate,
            schema_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("schema"),
            admission,
            request_schema,
            ack_schema,
        )
        .expect("strategy")
    }

    fn certificate() -> ExecutionStrategyCertificateV2 {
        ExecutionStrategyCertificateV2::new(
            id(10),
            id(11),
            id(12),
            id(1),
            id(2),
            id(13),
            artifact(14),
            id(15),
            id(16),
            id(17),
        )
    }

    fn descriptor(strategy_program: ContentId) -> CapabilityProgramV3 {
        CapabilityProgramV3::new(
            id(20),
            id(21),
            id(22),
            id(23),
            id(10),
            id(24),
            id(25),
            id(13),
            id(11),
            id(12),
            schema_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2).expect("strategy schema"),
            strategy_program,
            128,
        )
        .expect("descriptor")
    }

    fn artifacts() -> AuthenticatedInterpreterArtifactsV2 {
        AuthenticatedInterpreterArtifactsV2 {
            account_profile_program: id(10),
            request_profile_schema: id(11),
            request_profile_program: id(12),
            transition_schema: id(1),
            transition_program: id(2),
            effect_program: id(13),
        }
    }

    fn descriptor_v4(strategy_program: ContentId) -> CapabilityProgramV4 {
        CapabilityProgramV4::new(
            id(20),
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(id(30), id(10)),
                request_profile: ArtifactReferenceV4::new(id(11), id(12)),
                lifecycle: ArtifactReferenceV4::new(id(31), id(24)),
                strategy: ArtifactReferenceV4::new(
                    schema_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2).expect("strategy schema"),
                    strategy_program,
                ),
                transition: ArtifactReferenceV4::new(id(1), id(2)),
                effect: ArtifactReferenceV4::new(id(32), id(13)),
            },
            128,
        )
        .expect("V4 descriptor")
    }

    #[test]
    fn strategy_roundtrip_and_presence_grammar_are_exact() {
        for disposition in [
            StrategyDispositionV2::Interpreted,
            StrategyDispositionV2::ShadowAot,
            StrategyDispositionV2::AdmittedAot,
        ] {
            let strategy = strategy(disposition);
            let bytes = strategy.to_bytes();
            assert_eq!(ExecutionStrategyProgramV2::decode(&bytes), Ok(strategy));
            assert_eq!(
                strategy.transport_profile(),
                Ok(match disposition {
                    StrategyDispositionV2::ShadowAot => {
                        AcceleratorTransportProfileV2::ShadowTranscriptV3
                    }
                    StrategyDispositionV2::Interpreted | StrategyDispositionV2::AdmittedAot => {
                        AcceleratorTransportProfileV2::ChunkedBankV2
                    }
                })
            );
        }
        let mut hostile = strategy(StrategyDispositionV2::Interpreted).to_bytes();
        *hostile
            .get_mut(STRATEGY_CERTIFICATE_PRESENT_OFFSET_V2)
            .expect("presence") = PRESENT;
        hostile
            .get_mut(
                STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2..STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2 + 32,
            )
            .expect("certificate")
            .fill(9);
        assert_eq!(
            ExecutionStrategyProgramV2::decode(&hostile),
            Err(Error::InvalidDisposition)
        );

        let mut inactive = strategy(StrategyDispositionV2::Interpreted).to_bytes();
        *inactive
            .get_mut(STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2)
            .expect("inactive") = 1;
        assert_eq!(
            ExecutionStrategyProgramV2::decode(&inactive),
            Err(Error::NonCanonicalReservedBytes)
        );

        for (request_schema, ack_schema) in [
            (
                schema_id(SHADOW_REQUEST_SCHEMA_ID_V3).expect("Shadow request"),
                schema_id(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("V2 ack"),
            ),
            (
                schema_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("V2 request"),
                schema_id(SHADOW_ACK_SCHEMA_ID_V3).expect("Shadow ack"),
            ),
        ] {
            assert_eq!(
                ExecutionStrategyProgramV2::new(
                    StrategyDispositionV2::ShadowAot,
                    id(1),
                    id(2),
                    schema_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("schema"),
                    Some(id(3)),
                    schema_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("schema"),
                    None,
                    request_schema,
                    ack_schema,
                ),
                Err(Error::UnsupportedSchema)
            );
        }
        assert_eq!(
            ExecutionStrategyProgramV2::new(
                StrategyDispositionV2::ShadowAot,
                id(1),
                id(2),
                schema_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("schema"),
                Some(id(3)),
                schema_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("schema"),
                None,
                schema_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("V2 request"),
                schema_id(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("V2 ack"),
            ),
            Err(Error::UnsupportedSchema)
        );
    }

    #[test]
    fn certificate_and_admission_roundtrip_and_finalized_certificate_is_insufficient() {
        let certificate = certificate();
        assert_eq!(
            ExecutionStrategyCertificateV2::decode(&certificate.to_bytes()),
            Ok(certificate)
        );
        let admission = ExecutionStrategyAdmissionV2::new(id(3));
        assert_eq!(
            ExecutionStrategyAdmissionV2::decode(&admission.to_bytes()),
            Ok(admission)
        );
        let admitted = strategy(StrategyDispositionV2::AdmittedAot);
        assert_eq!(
            validate_admitted_aot_v2(
                id(30),
                admitted,
                descriptor(id(30)),
                id(3),
                certificate,
                artifacts(),
                artifact(14),
                None,
            ),
            Err(Error::MissingAdmission)
        );
        let authorization = validate_admitted_aot_v2(
            id(30),
            admitted,
            descriptor(id(30)),
            id(3),
            certificate,
            artifacts(),
            artifact(14),
            Some((id(4), admission)),
        )
        .expect("complete admission");
        assert_eq!(
            resolve_execution_candidate_v2(
                StrategyDispositionV2::AdmittedAot,
                None,
                Some(ExecutionCandidateV2::Accepted(&[7, 8])),
                Some(authorization),
            ),
            Ok(ExecutionCandidateV2::Accepted(&[7, 8]))
        );
        assert_eq!(
            validate_admitted_aot_v2(
                id(30),
                admitted,
                descriptor(id(30)),
                id(3),
                certificate,
                artifacts(),
                artifact(99),
                Some((id(4), admission)),
            ),
            Err(Error::ArtifactMismatch)
        );
        assert_eq!(
            validate_admitted_aot_v2(
                id(30),
                admitted,
                descriptor(id(31)),
                id(3),
                certificate,
                artifacts(),
                artifact(14),
                Some((id(4), admission)),
            ),
            Err(Error::DescriptorMismatch)
        );
        let mut swapped = artifacts();
        swapped.request_profile_schema = swapped.request_profile_program;
        assert_eq!(
            validate_admitted_aot_v2(
                id(30),
                admitted,
                descriptor(id(30)),
                id(3),
                certificate,
                swapped,
                artifact(14),
                Some((id(4), admission)),
            ),
            Err(Error::CertificateMismatch)
        );
    }

    #[test]
    fn v4_descriptor_joins_strategy_transition_and_admitted_certificate() {
        let strategy_program = id(30);
        let admitted = strategy(StrategyDispositionV2::AdmittedAot);
        let descriptor = descriptor_v4(strategy_program);
        assert_eq!(
            admitted.validate_descriptor_selection_v4(strategy_program, descriptor),
            Ok(())
        );
        assert_eq!(
            certificate().validate_v4(id(3), strategy_program, admitted, descriptor, artifacts()),
            Ok(())
        );
        assert!(
            validate_admitted_aot_v4(
                strategy_program,
                admitted,
                descriptor,
                id(3),
                certificate(),
                artifacts(),
                artifact(14),
                Some((id(4), ExecutionStrategyAdmissionV2::new(id(3)))),
            )
            .is_ok()
        );

        let hostile = CapabilityProgramV4::new(
            descriptor.kind(),
            descriptor.config_schema(),
            descriptor.request_schema(),
            descriptor.root_schema(),
            descriptor.derivation_policy(),
            descriptor.capacity_profile(),
            CapabilityArtifactsV4 {
                transition: ArtifactReferenceV4::new(id(1), id(99)),
                ..descriptor.artifacts()
            },
            descriptor.root_state_bytes(),
        )
        .expect("hostile descriptor");
        assert_eq!(
            admitted.validate_descriptor_selection_v4(strategy_program, hostile),
            Err(Error::DescriptorMismatch)
        );
        assert_eq!(
            validate_admitted_aot_v4(
                strategy_program,
                admitted,
                hostile,
                id(3),
                certificate(),
                artifacts(),
                artifact(14),
                Some((id(4), ExecutionStrategyAdmissionV2::new(id(3)))),
            ),
            Err(Error::DescriptorMismatch)
        );
    }

    #[test]
    fn interpreted_shadow_and_admitted_candidate_authority_is_disjoint() {
        let interpreted = ExecutionCandidateV2::Accepted(&[1, 2, 3]);
        let divergent = ExecutionCandidateV2::Accepted(&[1, 2, 4]);
        assert_eq!(
            resolve_execution_candidate_v2(
                StrategyDispositionV2::Interpreted,
                Some(interpreted),
                None,
                None,
            ),
            Ok(interpreted)
        );
        assert_eq!(
            resolve_execution_candidate_v2(
                StrategyDispositionV2::ShadowAot,
                Some(interpreted),
                Some(interpreted),
                None,
            ),
            Ok(interpreted)
        );
        assert_eq!(
            resolve_execution_candidate_v2(
                StrategyDispositionV2::ShadowAot,
                Some(interpreted),
                Some(divergent),
                None,
            ),
            Err(Error::StrategyDivergence)
        );
        assert_eq!(
            resolve_execution_candidate_v2(
                StrategyDispositionV2::AdmittedAot,
                None,
                Some(interpreted),
                None,
            ),
            Err(Error::InvalidDisposition)
        );
    }

    #[test]
    fn certificate_zero_and_swapped_semantic_tuple_refuse() {
        let canonical = certificate().to_bytes();
        for offset in [
            CERTIFICATE_ACCOUNT_PROFILE_PROGRAM_OFFSET_V2,
            CERTIFICATE_REQUEST_PROFILE_SCHEMA_OFFSET_V2,
            CERTIFICATE_REQUEST_PROFILE_PROGRAM_OFFSET_V2,
            CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2,
            CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2,
            CERTIFICATE_EFFECT_PROGRAM_OFFSET_V2,
            CERTIFICATE_ARTIFACT_RELEASE_OFFSET_V2,
            CERTIFICATE_COMPILER_RELEASE_OFFSET_V2,
            CERTIFICATE_TOOLCHAIN_OFFSET_V2,
            CERTIFICATE_TRANSLATION_VALIDATION_OFFSET_V2,
        ] {
            let mut hostile = canonical;
            hostile
                .get_mut(offset..offset + 32)
                .expect("identity")
                .fill(0);
            assert!(ExecutionStrategyCertificateV2::decode(&hostile).is_err());
        }
        let mut swapped = canonical;
        let transition_schema = *swapped
            .get(
                CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2
                    ..CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2 + 32,
            )
            .expect("schema")
            .first()
            .expect("first");
        let transition_program = *swapped
            .get(
                CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2
                    ..CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2 + 32,
            )
            .expect("program")
            .first()
            .expect("first");
        swapped
            .get_mut(
                CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2
                    ..CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2 + 32,
            )
            .expect("schema")
            .fill(transition_program);
        swapped
            .get_mut(
                CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2
                    ..CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2 + 32,
            )
            .expect("program")
            .fill(transition_schema);
        assert_ne!(
            ExecutionStrategyCertificateV2::decode(&swapped),
            Ok(certificate())
        );
    }

    fn request<'a>(bank: &'a [u8], chunk_index: u32) -> AcceleratorRequestV2<'a> {
        AcceleratorRequestV2::new(
            RequestTransportV2::Inline,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            70000,
            120,
            2,
            chunk_index,
            bank,
        )
        .expect("request")
    }

    #[test]
    fn runtime_transport_classifies_without_semantic_tail_cap() {
        assert_eq!(
            classify_bank_transport_v2(1, 1),
            Ok(BankTransportV2::InlineReturnData { bank_bytes: 40 })
        );
        assert_eq!(
            classify_bank_transport_v2(120, 2),
            Ok(BankTransportV2::AuthenticatedScratchPages {
                bank_bytes: 1024,
                page_count: 2,
            })
        );
        assert!(register_bank_bytes_v2(u32::MAX, u32::MAX).is_ok());
    }

    #[test]
    fn request_ack_chunks_bind_digest_length_offset_and_context() {
        let bank = vec![7_u8; 1024];
        let request = request(&bank, 1);
        let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len()];
        request
            .encode_into(&mut request_bytes)
            .expect("encode request");
        assert_eq!(AcceleratorRequestV2::decode(&request_bytes), Ok(request));
        let payload = bank.get(880..).expect("second chunk");
        let ack = AcceleratorAckV2::accepted(request, id(6), id(7), payload).expect("ack");
        let mut ack_bytes = vec![0_u8; ACCELERATOR_ACK_HEADER_BYTES_V2 + payload.len()];
        ack.encode_into(&mut ack_bytes).expect("encode ack");
        let decoded = AcceleratorAckV2::decode(&ack_bytes).expect("decode ack");
        assert_eq!(decoded, ack);
        assert_eq!(decoded.validate_request(request, id(6)), Ok(()));

        let before = ack_bytes.clone();
        let mut reordered = ack_bytes;
        reordered
            .get_mut(ACK_CHUNK_INDEX_OFFSET_V2..ACK_CHUNK_INDEX_OFFSET_V2 + 4)
            .expect("index")
            .copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            AcceleratorAckV2::decode(&reordered),
            Err(Error::BindingMismatch)
        );
        assert_ne!(reordered, before);
    }

    #[test]
    fn scratch_pages_refuse_reordering_and_mixing() {
        let first_payload = vec![1_u8; ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2];
        let second_payload = vec![2_u8; 144];
        let first = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            id(20),
            id(1),
            id(4),
            id(5),
            70000,
            120,
            2,
            0,
            &first_payload,
        )
        .expect("first page");
        let second = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            id(20),
            id(1),
            id(4),
            id(5),
            70000,
            120,
            2,
            1,
            &second_payload,
        )
        .expect("second page");
        assert_eq!(
            validate_scratch_page_sequence_v2(
                &[first, second],
                ScratchPageKindV2::Input,
                id(20),
                id(1),
                id(4),
                id(5),
                70000,
                120,
                2,
            ),
            Ok(())
        );
        assert_eq!(
            validate_scratch_page_sequence_v2(
                &[second, first],
                ScratchPageKindV2::Input,
                id(20),
                id(1),
                id(4),
                id(5),
                70000,
                120,
                2,
            ),
            Err(Error::BindingMismatch)
        );
        let mixed = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            id(20),
            id(1),
            id(99),
            id(5),
            70000,
            120,
            2,
            1,
            &second_payload,
        )
        .expect("mixed page");
        assert_eq!(
            validate_scratch_page_sequence_v2(
                &[first, mixed],
                ScratchPageKindV2::Input,
                id(20),
                id(1),
                id(4),
                id(5),
                70000,
                120,
                2,
            ),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn short_encode_targets_refuse_without_partial_writes() {
        let bank = vec![7_u8; 40];
        let request = AcceleratorRequestV2::new(
            RequestTransportV2::Inline,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            70_000,
            1,
            1,
            0,
            &bank,
        )
        .expect("request");
        let mut request_output = vec![9_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len() - 1];
        let request_before = request_output.clone();
        assert_eq!(
            request.encode_into(&mut request_output),
            Err(Error::InvalidLength)
        );
        assert_eq!(request_output, request_before);

        let ack = AcceleratorAckV2::accepted(request, id(6), id(7), &bank).expect("ack");
        let mut ack_output = vec![9_u8; ACCELERATOR_ACK_HEADER_BYTES_V2 + bank.len() - 1];
        let ack_before = ack_output.clone();
        assert_eq!(ack.encode_into(&mut ack_output), Err(Error::InvalidLength));
        assert_eq!(ack_output, ack_before);

        let page = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            id(20),
            id(1),
            id(4),
            id(5),
            70_000,
            1,
            1,
            0,
            &bank,
        )
        .expect("page");
        let mut page_output = vec![9_u8; SCRATCH_PAGE_HEADER_BYTES_V2 + bank.len() - 1];
        let page_before = page_output.clone();
        assert_eq!(
            page.encode_into(&mut page_output),
            Err(Error::InvalidLength)
        );
        assert_eq!(page_output, page_before);
    }
}
