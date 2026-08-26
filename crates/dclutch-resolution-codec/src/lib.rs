#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Generated fixed-layout wire types for the successor Resolution controller.
//!
//! The request contains only optimistic concurrency coordinates. Product and
//! Source policy remain in their canonical records. The receipt is the exact
//! 312-byte certificate layout generated from Lean's Source specialization.

#[rustfmt::skip]
mod generated_source_resolution;
#[allow(missing_docs)]
#[rustfmt::skip]
mod generated_v2;
mod v2;

pub use generated_v2::{
    ACCEPT_PYTH_REQUEST_BYTES_V2, ACCEPT_PYTH_V2_ACTION, ACCEPT_PYTH_V2_MAGIC,
    ACCEPT_PYTH_V2_VERSION, RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_MAGIC_V2,
    RESOLUTION_CERTIFICATE_VERSION_V2, SOURCE_CLOSURE_RECEIPT_BYTES_V2,
    SOURCE_CLOSURE_RECEIPT_MAGIC_V2, SOURCE_CLOSURE_RECEIPT_VERSION_V2,
};
pub use v2::{
    AcceptPythRequestV2, ResolutionCertificateKindV2, ResolutionCertificateV2,
    SourceClosureReceiptV2,
};

/// Bytes in one fixed primary-Pyth admission request.
pub const ACCEPT_PYTH_REQUEST_BYTES: usize = generated_source_resolution::REQUEST_BYTES_VALUE;
/// Bytes in one fixed funded recovery/failure request.
pub const FUNDED_TRANSITION_REQUEST_BYTES: usize =
    generated_source_resolution::FUNDED_REQUEST_BYTES_VALUE;
/// Bytes in one caller-verifiable funded-transition return receipt.
pub const FUNDED_TRANSITION_RECEIPT_BYTES: usize =
    generated_source_resolution::FUNDED_RECEIPT_BYTES_VALUE;
/// Bytes in one canonical Source Resolution certificate.
pub const RESOLUTION_CERTIFICATE_BYTES: usize =
    generated_source_resolution::CERTIFICATE_BYTES_VALUE;
/// Bytes in the sole canonical Resolution role request carried by a Core effect.
pub const RESOLUTION_CORE_ROLE_REQUEST_BYTES: usize =
    generated_source_resolution::CORE_REQUEST_BYTES_VALUE;
/// Bytes in one canonical persisted Source closure receipt.
pub const SOURCE_CLOSURE_RECEIPT_BYTES: usize = generated_source_resolution::CLOSURE_BYTES_VALUE;
/// PDA domain for a deterministic, typed, ordered certificate paired with a Source state.
pub const RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3: &[u8] = b"dclutch/resolution-cert/v3";
/// PDA domain for a deterministic closure receipt paired with a Source state and sequence.
pub const SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch/source-close/v1";
/// PDA domain for a deterministic Runtime V2 closure receipt.
pub const SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2: &[u8] = b"dclutch/source-close/v2";
/// State-derived immediate successor sequence for the one primary success certificate.
pub const PRIMARY_CERTIFICATE_SEQUENCE_V3: u64 =
    generated_source_resolution::PRIMARY_CERTIFICATE_SEQUENCE_VALUE;
/// Domain separating the content identity of an authenticated Pyth update.
pub const PYTH_EVIDENCE_CONTENT_DOMAIN_V1: &[u8] = b"dclutch/pyth-evidence/v1";
/// Domain for the exact FundingState bytes and lamport custody in a funded receipt.
pub const FUNDED_POSTSTATE_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/funded-poststate/v1";
/// Domain for the exact ordered Source/Core effect poststate digest.
pub const RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/resolution-poststate/v1";
/// Domain separating the exact three-account Resolution funding-set digest.
pub const SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/source-funding-set/v1";
/// Closed semantic release preimage for the sequential funded controller profile.
pub const RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V3: &[u8] =
    b"dclutch/release/source-resolution-controller-deterministic-prepaid-cert-funded-recovery-exhaustion-failure-v3";
/// SHA-256 of [`RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V3`].
pub const RESOLUTION_CONTROLLER_RELEASE_ID_V3: [u8; 32] = [
    0x9a, 0x62, 0xc2, 0xe4, 0x6d, 0xa3, 0xb4, 0xfa, 0x80, 0xd1, 0xc7, 0x5a, 0xcd, 0xfc, 0xcb, 0x44,
    0x8c, 0x19, 0x21, 0x1a, 0x63, 0x1a, 0xbc, 0xb1, 0x29, 0xb8, 0x26, 0xb5, 0x5a, 0xa8, 0x25, 0x3b,
];
/// Closed semantic release preimage for the Core-effect and Source-closure controller profile.
pub const RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V4: &[u8] =
    b"dclutch/release/source-resolution-controller-core-effects-source-closure-v4";
/// SHA-256 of [`RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V4`].
pub const RESOLUTION_CONTROLLER_RELEASE_ID_V4: [u8; 32] = [
    0x03, 0x6f, 0xdf, 0xcd, 0x80, 0x81, 0xbc, 0x8d, 0x21, 0x9c, 0x7f, 0xd8, 0xa6, 0xf5, 0xd7, 0x7b,
    0x56, 0xbe, 0x2d, 0x51, 0x43, 0x01, 0x06, 0xa7, 0x5c, 0x26, 0x8c, 0xde, 0xd7, 0xf0, 0x73, 0xde,
];
/// Schema identity used only to finalize a canonical Pyth-release record.
pub const PYTH_RELEASE_RECORD_SCHEMA_ID_V1: [u8; 32] = [
    0xb3, 0xa9, 0x8b, 0x34, 0x26, 0x68, 0xb4, 0x63, 0x3a, 0xb2, 0xa8, 0x42, 0x73, 0x16, 0xcd, 0xe1,
    0xb8, 0xac, 0xeb, 0x01, 0xee, 0xda, 0xcc, 0x3c, 0x3e, 0x29, 0x81, 0xec, 0x3f, 0x91, 0xdb, 0xf9,
];

const _: () = assert!(RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3.len() <= 32);
const _: () = assert!(SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2.len() <= 32);

/// Stable refusal from a hostile fixed-layout decoder or encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The slice did not have its one exact generated width.
    InvalidLength,
    /// Magic bytes did not identify the requested wire type.
    InvalidMagic,
    /// The schema version was not the generated version.
    UnsupportedVersion,
    /// The action byte did not name a generated transition.
    UnknownAction,
    /// A reserved byte was nonzero.
    NonCanonicalReserved,
    /// A required generation, identity, denominator, or timestamp was zero.
    ZeroCoordinate,
    /// A result selector did not fit the physical Product profile.
    InvalidSelector,
    /// A terminal certificate did not bind the authenticated Product Runtime V2 root.
    ProductAuthorityMismatch,
    /// Canonical identities or manifest-entry indices that must differ were duplicated.
    DuplicateCoordinate,
    /// The action, receipt kind, receipt account, beneficiary, and sequence did not partition.
    InvalidReceiptShape,
    /// The closure receipt did not attest exactly the three canonical funding compartments.
    InvalidFundingCount,
}

/// Result alias for Resolution codecs.
pub type Result<T> = core::result::Result<T, Error>;

/// Optimistic concurrency coordinates for one primary-Pyth admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptPythRequestV1 {
    /// Exact immutable Market generation expected by the submitter.
    pub expected_generation: u64,
    /// Product-owned domain-separated result-domain content identity.
    pub expected_result_domain_id: [u8; 32],
    /// Exact Pyth deployment-release content identity selected by Source.
    pub expected_provider_release_id: [u8; 32],
}

impl AcceptPythRequestV1 {
    /// Decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, ACCEPT_PYTH_REQUEST_BYTES)?;
        exact(
            input,
            generated_source_resolution::REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::REQUEST_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated_source_resolution::REQUEST_VERSION_OFFSET)?
            != generated_source_resolution::REQUEST_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated_source_resolution::REQUEST_ACTION_OFFSET)?
            != generated_source_resolution::REQUEST_ACCEPT_PYTH_ACTION
        {
            return Err(Error::UnknownAction);
        }
        require_zero(
            input,
            generated_source_resolution::REQUEST_RESERVED_OFFSET,
            5,
        )?;
        let value = Self {
            expected_generation: u64_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_GENERATION_OFFSET,
            )?,
            expected_result_domain_id: array_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            )?,
            expected_provider_release_id: array_at(
                input,
                generated_source_resolution::REQUEST_EXPECTED_PROVIDER_RELEASE_ID_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; ACCEPT_PYTH_REQUEST_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; ACCEPT_PYTH_REQUEST_BYTES];
        put(
            &mut output,
            generated_source_resolution::REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::REQUEST_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_VERSION_OFFSET,
            &generated_source_resolution::REQUEST_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_ACTION_OFFSET,
            &[generated_source_resolution::REQUEST_ACCEPT_PYTH_ACTION],
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_GENERATION_OFFSET,
            &self.expected_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            &self.expected_result_domain_id,
        )?;
        put(
            &mut output,
            generated_source_resolution::REQUEST_EXPECTED_PROVIDER_RELEASE_ID_OFFSET,
            &self.expected_provider_release_id,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.expected_generation == 0
            || is_zero(&self.expected_result_domain_id)
            || is_zero(&self.expected_provider_release_id)
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Lean-owned sequential funded liveness action tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundedTransitionActionV3 {
    /// Advance to the one immediate ordered recovery successor.
    FailNext,
    /// Commit exhaustion after the last active recovery leg expires.
    Exhaust,
    /// Commit Product's explicit failure selector after exhaustion.
    CommitFailure,
}

impl FundedTransitionActionV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated_source_resolution::FUNDED_REQUEST_FAIL_NEXT_ACTION => Ok(Self::FailNext),
            generated_source_resolution::FUNDED_REQUEST_EXHAUST_ACTION => Ok(Self::Exhaust),
            generated_source_resolution::FUNDED_REQUEST_COMMIT_FAILURE_ACTION => {
                Ok(Self::CommitFailure)
            }
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::FailNext => generated_source_resolution::FUNDED_REQUEST_FAIL_NEXT_ACTION,
            Self::Exhaust => generated_source_resolution::FUNDED_REQUEST_EXHAUST_ACTION,
            Self::CommitFailure => {
                generated_source_resolution::FUNDED_REQUEST_COMMIT_FAILURE_ACTION
            }
        }
    }
}

/// Optimistic coordinates for one canonically funded Source liveness step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundedTransitionRequestV3 {
    /// Recovery advancement, exhaustion, or explicit failure commitment.
    pub action: FundedTransitionActionV3,
    /// Exact immutable Market generation.
    pub expected_generation: u64,
    /// Zero-based next recovery index, or recovery count for failure.
    pub expected_recovery_index: u32,
    /// Product-owned domain-separated result-domain content identity.
    pub expected_result_domain_id: [u8; 32],
    /// Exact capability-entry configuration identity selected by Source.
    pub expected_funding_allocation_id: [u8; 32],
}

impl FundedTransitionRequestV3 {
    /// Decode one exact generated funded-transition request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, FUNDED_TRANSITION_REQUEST_BYTES)?;
        exact(
            input,
            generated_source_resolution::FUNDED_REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::FUNDED_REQUEST_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(
            input,
            generated_source_resolution::FUNDED_REQUEST_VERSION_OFFSET,
        )? != generated_source_resolution::FUNDED_REQUEST_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(
            input,
            generated_source_resolution::FUNDED_REQUEST_RESERVED_HEADER_OFFSET,
            5,
        )?;
        require_zero(
            input,
            generated_source_resolution::FUNDED_REQUEST_RESERVED_BODY_OFFSET,
            4,
        )?;
        let value = Self {
            action: FundedTransitionActionV3::decode(byte_at(
                input,
                generated_source_resolution::FUNDED_REQUEST_ACTION_OFFSET,
            )?)?,
            expected_generation: u64_at(
                input,
                generated_source_resolution::FUNDED_REQUEST_EXPECTED_GENERATION_OFFSET,
            )?,
            expected_recovery_index: u32_at(
                input,
                generated_source_resolution::FUNDED_REQUEST_EXPECTED_RECOVERY_INDEX_OFFSET,
            )?,
            expected_result_domain_id: array_at(
                input,
                generated_source_resolution::FUNDED_REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            )?,
            expected_funding_allocation_id: array_at(
                input,
                generated_source_resolution::FUNDED_REQUEST_EXPECTED_FUNDING_ALLOCATION_ID_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact generated funded-transition request.
    pub fn to_bytes(self) -> Result<[u8; FUNDED_TRANSITION_REQUEST_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; FUNDED_TRANSITION_REQUEST_BYTES];
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::FUNDED_REQUEST_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_VERSION_OFFSET,
            &generated_source_resolution::FUNDED_REQUEST_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_ACTION_OFFSET,
            &[self.action.byte()],
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_EXPECTED_GENERATION_OFFSET,
            &self.expected_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_EXPECTED_RECOVERY_INDEX_OFFSET,
            &self.expected_recovery_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET,
            &self.expected_result_domain_id,
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_REQUEST_EXPECTED_FUNDING_ALLOCATION_ID_OFFSET,
            &self.expected_funding_allocation_id,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.expected_generation == 0
            || is_zero(&self.expected_result_domain_id)
            || is_zero(&self.expected_funding_allocation_id)
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Canonical Source phase committed by a funded transition receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundedReceiptPostPhaseV1 {
    /// One ordered recovery route is now active.
    Recovery,
    /// All ordered recovery routes have been exhausted.
    Exhausted,
    /// Product's explicit failure result has been committed.
    FailureCommitted,
}

impl FundedReceiptPostPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated_source_resolution::FUNDED_RECEIPT_RECOVERY_PHASE => Ok(Self::Recovery),
            generated_source_resolution::FUNDED_RECEIPT_EXHAUSTED_PHASE => Ok(Self::Exhausted),
            generated_source_resolution::FUNDED_RECEIPT_FAILURE_COMMITTED_PHASE => {
                Ok(Self::FailureCommitted)
            }
            _ => Err(Error::InvalidReceiptShape),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Recovery => generated_source_resolution::FUNDED_RECEIPT_RECOVERY_PHASE,
            Self::Exhausted => generated_source_resolution::FUNDED_RECEIPT_EXHAUSTED_PHASE,
            Self::FailureCommitted => {
                generated_source_resolution::FUNDED_RECEIPT_FAILURE_COMMITTED_PHASE
            }
        }
    }
}

/// Projection of terminal/refund eligibility from the canonical Source phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundedTerminalRefundPhaseV1 {
    /// Recovery remains live; terminal admission and refund are unavailable.
    Continuing,
    /// Recovery is exhausted but explicit Product failure is not committed.
    AwaitingFailure,
    /// Failure is terminal and awaits the separately authenticated close/refund effect.
    TerminalRefundPending,
}

impl FundedTerminalRefundPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated_source_resolution::FUNDED_RECEIPT_CONTINUING_PHASE => Ok(Self::Continuing),
            generated_source_resolution::FUNDED_RECEIPT_AWAITING_FAILURE_PHASE => {
                Ok(Self::AwaitingFailure)
            }
            generated_source_resolution::FUNDED_RECEIPT_TERMINAL_REFUND_PENDING_PHASE => {
                Ok(Self::TerminalRefundPending)
            }
            _ => Err(Error::InvalidReceiptShape),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Continuing => generated_source_resolution::FUNDED_RECEIPT_CONTINUING_PHASE,
            Self::AwaitingFailure => {
                generated_source_resolution::FUNDED_RECEIPT_AWAITING_FAILURE_PHASE
            }
            Self::TerminalRefundPending => {
                generated_source_resolution::FUNDED_RECEIPT_TERMINAL_REFUND_PENDING_PHASE
            }
        }
    }
}

/// Caller-verifiable evidence of one atomically committed funded Source transition.
///
/// This receipt is returned by the selected Resolution program. It is not a
/// replacement for the persisted Source, FundingState, or certificate facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundedTransitionReceiptV1 {
    /// Exact funded action whose request bytes are committed below.
    pub action: FundedTransitionActionV3,
    /// Canonical post-transition Source phase.
    pub post_phase: FundedReceiptPostPhaseV1,
    /// Exact replay certificate kind.
    pub certificate_kind: ResolutionCertificateKindV1,
    /// Projection of terminal and refund eligibility.
    pub terminal_refund_phase: FundedTerminalRefundPhaseV1,
    /// Exact executing Resolution program selected by Registry authority.
    pub producer_program: [u8; 32],
    /// Exact Registry-selected Resolution semantic release identity.
    pub producer_release: [u8; 32],
    /// SHA-256 of the exact funded-transition request bytes.
    pub request_digest: [u8; 32],
    /// Canonical Source state account identity.
    pub source_state: [u8; 32],
    /// Exact action-specific FundingState account identity.
    pub funding_state: [u8; 32],
    /// Permissionless worker credited by this transition.
    pub worker: [u8; 32],
    /// Deterministic replay certificate account identity.
    pub certificate: [u8; 32],
    /// SHA-256 of the exact authenticated Source prestate bytes.
    pub pre_source_digest: [u8; 32],
    /// SHA-256 of the exact committed Source poststate bytes.
    pub post_source_digest: [u8; 32],
    /// Digest of exact FundingState post bytes and post-account lamports.
    pub funding_post_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// State-derived deterministic certificate sequence.
    pub replay_sequence: u64,
    /// Exact native-lamport bounty credited to `worker`.
    pub work_paid: u64,
    /// Exact semantic bounty funding remaining after the transition.
    pub funding_remaining: u64,
    /// Product-owned explicit failure selector; zero for recovery/exhaustion.
    pub selector: u32,
}

impl FundedTransitionReceiptV1 {
    /// Decode one exact Lean-generated funded-transition receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, FUNDED_TRANSITION_RECEIPT_BYTES)?;
        exact(
            input,
            generated_source_resolution::FUNDED_RECEIPT_MAGIC_OFFSET,
            &generated_source_resolution::FUNDED_RECEIPT_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(
            input,
            generated_source_resolution::FUNDED_RECEIPT_VERSION_OFFSET,
        )? != generated_source_resolution::FUNDED_RECEIPT_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(
            input,
            generated_source_resolution::FUNDED_RECEIPT_RESERVED_HEADER_OFFSET,
            2,
        )?;
        require_zero(
            input,
            generated_source_resolution::FUNDED_RECEIPT_RESERVED_BODY_OFFSET,
            4,
        )?;
        let value = Self {
            action: FundedTransitionActionV3::decode(byte_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_ACTION_OFFSET,
            )?)?,
            post_phase: FundedReceiptPostPhaseV1::decode(byte_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_POST_PHASE_OFFSET,
            )?)?,
            certificate_kind: ResolutionCertificateKindV1::decode(byte_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_CERTIFICATE_KIND_OFFSET,
            )?)?,
            terminal_refund_phase: FundedTerminalRefundPhaseV1::decode(byte_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_TERMINAL_REFUND_PHASE_OFFSET,
            )?)?,
            producer_program: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_PRODUCER_PROGRAM_OFFSET,
            )?,
            producer_release: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_PRODUCER_RELEASE_OFFSET,
            )?,
            request_digest: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_REQUEST_DIGEST_OFFSET,
            )?,
            source_state: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_SOURCE_STATE_OFFSET,
            )?,
            funding_state: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_STATE_OFFSET,
            )?,
            worker: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_WORKER_OFFSET,
            )?,
            certificate: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_CERTIFICATE_OFFSET,
            )?,
            pre_source_digest: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_PRE_SOURCE_DIGEST_OFFSET,
            )?,
            post_source_digest: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_POST_SOURCE_DIGEST_OFFSET,
            )?,
            funding_post_digest: array_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_POST_DIGEST_OFFSET,
            )?,
            generation: u64_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_GENERATION_OFFSET,
            )?,
            replay_sequence: u64_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_REPLAY_SEQUENCE_OFFSET,
            )?,
            work_paid: u64_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_WORK_PAID_OFFSET,
            )?,
            funding_remaining: u64_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_REMAINING_OFFSET,
            )?,
            selector: u32_at(
                input,
                generated_source_resolution::FUNDED_RECEIPT_SELECTOR_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact Lean-generated funded-transition receipt.
    pub fn to_bytes(self) -> Result<[u8; FUNDED_TRANSITION_RECEIPT_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; FUNDED_TRANSITION_RECEIPT_BYTES];
        put(
            &mut output,
            generated_source_resolution::FUNDED_RECEIPT_MAGIC_OFFSET,
            &generated_source_resolution::FUNDED_RECEIPT_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::FUNDED_RECEIPT_VERSION_OFFSET,
            &generated_source_resolution::FUNDED_RECEIPT_ABI_VERSION.to_le_bytes(),
        )?;
        for (offset, value) in [
            (
                generated_source_resolution::FUNDED_RECEIPT_ACTION_OFFSET,
                self.action.byte(),
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_POST_PHASE_OFFSET,
                self.post_phase.byte(),
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_CERTIFICATE_KIND_OFFSET,
                self.certificate_kind.byte(),
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_TERMINAL_REFUND_PHASE_OFFSET,
                self.terminal_refund_phase.byte(),
            ),
        ] {
            put(&mut output, offset, &[value])?;
        }
        for (offset, value) in [
            (
                generated_source_resolution::FUNDED_RECEIPT_PRODUCER_PROGRAM_OFFSET,
                &self.producer_program,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_PRODUCER_RELEASE_OFFSET,
                &self.producer_release,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_REQUEST_DIGEST_OFFSET,
                &self.request_digest,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_SOURCE_STATE_OFFSET,
                &self.source_state,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_STATE_OFFSET,
                &self.funding_state,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_WORKER_OFFSET,
                &self.worker,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_CERTIFICATE_OFFSET,
                &self.certificate,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_PRE_SOURCE_DIGEST_OFFSET,
                &self.pre_source_digest,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_POST_SOURCE_DIGEST_OFFSET,
                &self.post_source_digest,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_POST_DIGEST_OFFSET,
                &self.funding_post_digest,
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        for (offset, value) in [
            (
                generated_source_resolution::FUNDED_RECEIPT_GENERATION_OFFSET,
                self.generation,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_REPLAY_SEQUENCE_OFFSET,
                self.replay_sequence,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_WORK_PAID_OFFSET,
                self.work_paid,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_FUNDING_REMAINING_OFFSET,
                self.funding_remaining,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            generated_source_resolution::FUNDED_RECEIPT_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.producer_program)
            || is_zero(&self.producer_release)
            || is_zero(&self.request_digest)
            || is_zero(&self.source_state)
            || is_zero(&self.funding_state)
            || is_zero(&self.worker)
            || is_zero(&self.certificate)
            || is_zero(&self.pre_source_digest)
            || is_zero(&self.post_source_digest)
            || is_zero(&self.funding_post_digest)
            || self.generation == 0
            || self.replay_sequence == 0
            || self.work_paid == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        let partition_is_valid = match self.action {
            FundedTransitionActionV3::FailNext => {
                self.post_phase == FundedReceiptPostPhaseV1::Recovery
                    && self.certificate_kind == ResolutionCertificateKindV1::RecoveryAdvanced
                    && self.terminal_refund_phase == FundedTerminalRefundPhaseV1::Continuing
                    && self.selector == 0
            }
            FundedTransitionActionV3::Exhaust => {
                self.post_phase == FundedReceiptPostPhaseV1::Exhausted
                    && self.certificate_kind == ResolutionCertificateKindV1::Exhausted
                    && self.terminal_refund_phase == FundedTerminalRefundPhaseV1::AwaitingFailure
                    && self.selector == 0
            }
            FundedTransitionActionV3::CommitFailure => {
                self.post_phase == FundedReceiptPostPhaseV1::FailureCommitted
                    && self.certificate_kind == ResolutionCertificateKindV1::ResolutionFailure
                    && self.terminal_refund_phase
                        == FundedTerminalRefundPhaseV1::TerminalRefundPending
                    && u8::try_from(self.selector).is_ok()
            }
        };
        if !partition_is_valid {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(())
    }
}

/// Canonical Core effect action delegated to the Resolution role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionCoreActionV1 {
    /// Create the Source and its exact three funding compartments.
    CreateFund,
    /// Activate and authenticate the exact three funded compartments.
    VerifyFundReady,
    /// Project an authenticated terminal Resolution certificate to Core.
    AdmitTerminal,
    /// Discharge Source funding and persist its canonical closure receipt.
    CloseFund,
}

impl ResolutionCoreActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated_source_resolution::CORE_REQUEST_CREATE_FUND_ACTION => Ok(Self::CreateFund),
            generated_source_resolution::CORE_REQUEST_VERIFY_FUND_READY_ACTION => {
                Ok(Self::VerifyFundReady)
            }
            generated_source_resolution::CORE_REQUEST_ADMIT_TERMINAL_ACTION => {
                Ok(Self::AdmitTerminal)
            }
            generated_source_resolution::CORE_REQUEST_CLOSE_FUND_ACTION => Ok(Self::CloseFund),
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::CreateFund => generated_source_resolution::CORE_REQUEST_CREATE_FUND_ACTION,
            Self::VerifyFundReady => {
                generated_source_resolution::CORE_REQUEST_VERIFY_FUND_READY_ACTION
            }
            Self::AdmitTerminal => generated_source_resolution::CORE_REQUEST_ADMIT_TERMINAL_ACTION,
            Self::CloseFund => generated_source_resolution::CORE_REQUEST_CLOSE_FUND_ACTION,
        }
    }
}

/// Action-partitioned receipt coordinate carried by a Resolution Core request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionCoreReceiptKindV1 {
    /// Create and readiness effects have no receipt account.
    None,
    /// Terminal admission names an ordinary Resolution success certificate.
    TerminalSuccess,
    /// Terminal admission names Product's explicit failure certificate.
    TerminalFailure,
    /// Close names the canonical persisted Source closure receipt.
    Closure,
}

impl ResolutionCoreReceiptKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            generated_source_resolution::CORE_REQUEST_TERMINAL_SUCCESS_KIND => {
                Ok(Self::TerminalSuccess)
            }
            generated_source_resolution::CORE_REQUEST_TERMINAL_FAILURE_KIND => {
                Ok(Self::TerminalFailure)
            }
            generated_source_resolution::CORE_REQUEST_CLOSURE_KIND => Ok(Self::Closure),
            _ => Err(Error::InvalidReceiptShape),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::TerminalSuccess => {
                generated_source_resolution::CORE_REQUEST_TERMINAL_SUCCESS_KIND
            }
            Self::TerminalFailure => {
                generated_source_resolution::CORE_REQUEST_TERMINAL_FAILURE_KIND
            }
            Self::Closure => generated_source_resolution::CORE_REQUEST_CLOSURE_KIND,
        }
    }
}

/// Sole fixed Resolution role request authenticated inside a canonical Core effect envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionRoleRequestV1 {
    /// Exact Core action tag; no private Resolution envelope exists.
    pub action: ResolutionCoreActionV1,
    /// Action-partitioned terminal or closure receipt kind.
    pub receipt_kind: ResolutionCoreReceiptKindV1,
    /// Canonical Source state account.
    pub source_state: [u8; 32],
    /// Finalized Source-material content identity.
    pub source_material: [u8; 32],
    /// Finalized capability-manifest content identity.
    pub capability_manifest: [u8; 32],
    /// Canonical recovery funding compartment.
    pub recovery_funding: [u8; 32],
    /// Canonical exhaustion funding compartment.
    pub exhaustion_funding: [u8; 32],
    /// Canonical explicit-failure funding compartment.
    pub failure_funding: [u8; 32],
    /// Terminal certificate, closure receipt, or zero according to `action`.
    pub receipt: [u8; 32],
    /// Funding beneficiary, or zero for terminal admission.
    pub beneficiary: [u8; 32],
    /// Capability-manifest entry selecting recovery funding.
    pub recovery_entry_index: u16,
    /// Capability-manifest entry selecting exhaustion funding.
    pub exhaustion_entry_index: u16,
    /// Capability-manifest entry selecting explicit-failure funding.
    pub failure_entry_index: u16,
    /// Exact terminal or closure sequence, or zero before terminal admission.
    pub receipt_sequence: u64,
}

impl ResolutionRoleRequestV1 {
    /// Decode one exact canonical Resolution role request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, RESOLUTION_CORE_ROLE_REQUEST_BYTES)?;
        exact(
            input,
            generated_source_resolution::CORE_REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::CORE_REQUEST_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(
            input,
            generated_source_resolution::CORE_REQUEST_VERSION_OFFSET,
        )? != generated_source_resolution::CORE_REQUEST_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(
            input,
            generated_source_resolution::CORE_REQUEST_RESERVED_HEADER_OFFSET,
            4,
        )?;
        require_zero(
            input,
            generated_source_resolution::CORE_REQUEST_RESERVED_BODY_OFFSET,
            2,
        )?;
        let value = Self {
            action: ResolutionCoreActionV1::decode(byte_at(
                input,
                generated_source_resolution::CORE_REQUEST_ACTION_OFFSET,
            )?)?,
            receipt_kind: ResolutionCoreReceiptKindV1::decode(byte_at(
                input,
                generated_source_resolution::CORE_REQUEST_RECEIPT_KIND_OFFSET,
            )?)?,
            source_state: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_SOURCE_STATE_OFFSET,
            )?,
            source_material: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_SOURCE_MATERIAL_OFFSET,
            )?,
            capability_manifest: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_CAPABILITY_MANIFEST_OFFSET,
            )?,
            recovery_funding: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_RECOVERY_FUNDING_OFFSET,
            )?,
            exhaustion_funding: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_EXHAUSTION_FUNDING_OFFSET,
            )?,
            failure_funding: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_FAILURE_FUNDING_OFFSET,
            )?,
            receipt: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_RECEIPT_OFFSET,
            )?,
            beneficiary: array_at(
                input,
                generated_source_resolution::CORE_REQUEST_BENEFICIARY_OFFSET,
            )?,
            recovery_entry_index: u16_at(
                input,
                generated_source_resolution::CORE_REQUEST_RECOVERY_ENTRY_INDEX_OFFSET,
            )?,
            exhaustion_entry_index: u16_at(
                input,
                generated_source_resolution::CORE_REQUEST_EXHAUSTION_ENTRY_INDEX_OFFSET,
            )?,
            failure_entry_index: u16_at(
                input,
                generated_source_resolution::CORE_REQUEST_FAILURE_ENTRY_INDEX_OFFSET,
            )?,
            receipt_sequence: u64_at(
                input,
                generated_source_resolution::CORE_REQUEST_RECEIPT_SEQUENCE_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical Resolution role request.
    pub fn to_bytes(self) -> Result<[u8; RESOLUTION_CORE_ROLE_REQUEST_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; RESOLUTION_CORE_ROLE_REQUEST_BYTES];
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_MAGIC_OFFSET,
            &generated_source_resolution::CORE_REQUEST_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_VERSION_OFFSET,
            &generated_source_resolution::CORE_REQUEST_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_ACTION_OFFSET,
            &[self.action.byte()],
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_RECEIPT_KIND_OFFSET,
            &[self.receipt_kind.byte()],
        )?;
        for (offset, value) in [
            (
                generated_source_resolution::CORE_REQUEST_SOURCE_STATE_OFFSET,
                &self.source_state,
            ),
            (
                generated_source_resolution::CORE_REQUEST_SOURCE_MATERIAL_OFFSET,
                &self.source_material,
            ),
            (
                generated_source_resolution::CORE_REQUEST_CAPABILITY_MANIFEST_OFFSET,
                &self.capability_manifest,
            ),
            (
                generated_source_resolution::CORE_REQUEST_RECOVERY_FUNDING_OFFSET,
                &self.recovery_funding,
            ),
            (
                generated_source_resolution::CORE_REQUEST_EXHAUSTION_FUNDING_OFFSET,
                &self.exhaustion_funding,
            ),
            (
                generated_source_resolution::CORE_REQUEST_FAILURE_FUNDING_OFFSET,
                &self.failure_funding,
            ),
            (
                generated_source_resolution::CORE_REQUEST_RECEIPT_OFFSET,
                &self.receipt,
            ),
            (
                generated_source_resolution::CORE_REQUEST_BENEFICIARY_OFFSET,
                &self.beneficiary,
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_RECOVERY_ENTRY_INDEX_OFFSET,
            &self.recovery_entry_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_EXHAUSTION_ENTRY_INDEX_OFFSET,
            &self.exhaustion_entry_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_FAILURE_ENTRY_INDEX_OFFSET,
            &self.failure_entry_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CORE_REQUEST_RECEIPT_SEQUENCE_OFFSET,
            &self.receipt_sequence.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.source_state)
            || is_zero(&self.source_material)
            || is_zero(&self.capability_manifest)
            || is_zero(&self.recovery_funding)
            || is_zero(&self.exhaustion_funding)
            || is_zero(&self.failure_funding)
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.recovery_funding == self.exhaustion_funding
            || self.recovery_funding == self.failure_funding
            || self.exhaustion_funding == self.failure_funding
            || self.recovery_entry_index == self.exhaustion_entry_index
            || self.recovery_entry_index == self.failure_entry_index
            || self.exhaustion_entry_index == self.failure_entry_index
        {
            return Err(Error::DuplicateCoordinate);
        }
        let shape_is_valid = match self.action {
            ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
                self.receipt_kind == ResolutionCoreReceiptKindV1::None
                    && is_zero(&self.receipt)
                    && !is_zero(&self.beneficiary)
                    && self.receipt_sequence == 0
            }
            ResolutionCoreActionV1::AdmitTerminal => {
                matches!(
                    self.receipt_kind,
                    ResolutionCoreReceiptKindV1::TerminalSuccess
                        | ResolutionCoreReceiptKindV1::TerminalFailure
                ) && !is_zero(&self.receipt)
                    && is_zero(&self.beneficiary)
                    && self.receipt_sequence != 0
            }
            ResolutionCoreActionV1::CloseFund => {
                self.receipt_kind == ResolutionCoreReceiptKindV1::Closure
                    && !is_zero(&self.receipt)
                    && !is_zero(&self.beneficiary)
                    && self.receipt_sequence != 0
            }
        };
        if !shape_is_valid {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(())
    }
}

/// Persisted receipt proving terminal Source state and all funding accounts were discharged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceClosureReceiptV1 {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Canonical Source state account that was closed.
    pub source_state: [u8; 32],
    /// Finalized Source-material content identity.
    pub source_material: [u8; 32],
    /// Finalized capability-manifest content identity.
    pub capability_manifest: [u8; 32],
    /// Authenticated terminal Resolution certificate.
    pub terminal_certificate: [u8; 32],
    /// This deterministic closure receipt account.
    pub receipt_account: [u8; 32],
    /// Exact beneficiary receiving the discharged lamports.
    pub beneficiary: [u8; 32],
    /// Digest of the authenticated terminal Source pre-state.
    pub source_state_digest: [u8; 32],
    /// Digest of the authenticated terminal certificate bytes.
    pub terminal_certificate_digest: [u8; 32],
    /// Digest of the exact ordered three-compartment funding pre-state.
    pub funding_set_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact terminal certificate sequence.
    pub terminal_sequence: u64,
    /// Product-owned terminal result selector.
    pub selector: u32,
    /// Exact source and funding lamports discharged to `beneficiary`.
    pub refund_lamports: u64,
    /// Clock timestamp at which the atomic discharge committed.
    pub closed_at: u64,
}

impl SourceClosureReceiptV1 {
    /// Decode one exact canonical Source closure receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, SOURCE_CLOSURE_RECEIPT_BYTES)?;
        exact(
            input,
            generated_source_resolution::CLOSURE_MAGIC_OFFSET,
            &generated_source_resolution::CLOSURE_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(input, generated_source_resolution::CLOSURE_VERSION_OFFSET)?
            != generated_source_resolution::CLOSURE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, generated_source_resolution::CLOSURE_KIND_OFFSET)?
            != generated_source_resolution::CLOSURE_KIND_VALUE
        {
            return Err(Error::InvalidReceiptShape);
        }
        require_zero(
            input,
            generated_source_resolution::CLOSURE_RESERVED_HEADER_OFFSET,
            5,
        )?;
        if u32_at(
            input,
            generated_source_resolution::CLOSURE_FUNDING_COUNT_OFFSET,
        )? != generated_source_resolution::CLOSURE_FUNDING_COUNT_VALUE
        {
            return Err(Error::InvalidFundingCount);
        }
        require_zero(
            input,
            generated_source_resolution::CLOSURE_RESERVED_BODY_OFFSET,
            8,
        )?;
        let value = Self {
            market: array_at(input, generated_source_resolution::CLOSURE_MARKET_OFFSET)?,
            source_state: array_at(
                input,
                generated_source_resolution::CLOSURE_SOURCE_STATE_OFFSET,
            )?,
            source_material: array_at(
                input,
                generated_source_resolution::CLOSURE_SOURCE_MATERIAL_OFFSET,
            )?,
            capability_manifest: array_at(
                input,
                generated_source_resolution::CLOSURE_CAPABILITY_MANIFEST_OFFSET,
            )?,
            terminal_certificate: array_at(
                input,
                generated_source_resolution::CLOSURE_TERMINAL_CERTIFICATE_OFFSET,
            )?,
            receipt_account: array_at(
                input,
                generated_source_resolution::CLOSURE_RECEIPT_ACCOUNT_OFFSET,
            )?,
            beneficiary: array_at(
                input,
                generated_source_resolution::CLOSURE_BENEFICIARY_OFFSET,
            )?,
            source_state_digest: array_at(
                input,
                generated_source_resolution::CLOSURE_SOURCE_STATE_DIGEST_OFFSET,
            )?,
            terminal_certificate_digest: array_at(
                input,
                generated_source_resolution::CLOSURE_TERMINAL_CERTIFICATE_DIGEST_OFFSET,
            )?,
            funding_set_digest: array_at(
                input,
                generated_source_resolution::CLOSURE_FUNDING_SET_DIGEST_OFFSET,
            )?,
            generation: u64_at(
                input,
                generated_source_resolution::CLOSURE_GENERATION_OFFSET,
            )?,
            terminal_sequence: u64_at(
                input,
                generated_source_resolution::CLOSURE_TERMINAL_SEQUENCE_OFFSET,
            )?,
            selector: u32_at(input, generated_source_resolution::CLOSURE_SELECTOR_OFFSET)?,
            refund_lamports: u64_at(
                input,
                generated_source_resolution::CLOSURE_REFUND_LAMPORTS_OFFSET,
            )?,
            closed_at: u64_at(input, generated_source_resolution::CLOSURE_CLOSED_AT_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical Source closure receipt.
    pub fn to_bytes(self) -> Result<[u8; SOURCE_CLOSURE_RECEIPT_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; SOURCE_CLOSURE_RECEIPT_BYTES];
        put(
            &mut output,
            generated_source_resolution::CLOSURE_MAGIC_OFFSET,
            &generated_source_resolution::CLOSURE_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_VERSION_OFFSET,
            &generated_source_resolution::CLOSURE_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_KIND_OFFSET,
            &[generated_source_resolution::CLOSURE_KIND_VALUE],
        )?;
        for (offset, value) in [
            (
                generated_source_resolution::CLOSURE_MARKET_OFFSET,
                &self.market,
            ),
            (
                generated_source_resolution::CLOSURE_SOURCE_STATE_OFFSET,
                &self.source_state,
            ),
            (
                generated_source_resolution::CLOSURE_SOURCE_MATERIAL_OFFSET,
                &self.source_material,
            ),
            (
                generated_source_resolution::CLOSURE_CAPABILITY_MANIFEST_OFFSET,
                &self.capability_manifest,
            ),
            (
                generated_source_resolution::CLOSURE_TERMINAL_CERTIFICATE_OFFSET,
                &self.terminal_certificate,
            ),
            (
                generated_source_resolution::CLOSURE_RECEIPT_ACCOUNT_OFFSET,
                &self.receipt_account,
            ),
            (
                generated_source_resolution::CLOSURE_BENEFICIARY_OFFSET,
                &self.beneficiary,
            ),
            (
                generated_source_resolution::CLOSURE_SOURCE_STATE_DIGEST_OFFSET,
                &self.source_state_digest,
            ),
            (
                generated_source_resolution::CLOSURE_TERMINAL_CERTIFICATE_DIGEST_OFFSET,
                &self.terminal_certificate_digest,
            ),
            (
                generated_source_resolution::CLOSURE_FUNDING_SET_DIGEST_OFFSET,
                &self.funding_set_digest,
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        put(
            &mut output,
            generated_source_resolution::CLOSURE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_TERMINAL_SEQUENCE_OFFSET,
            &self.terminal_sequence.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_FUNDING_COUNT_OFFSET,
            &generated_source_resolution::CLOSURE_FUNDING_COUNT_VALUE.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_REFUND_LAMPORTS_OFFSET,
            &self.refund_lamports.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CLOSURE_CLOSED_AT_OFFSET,
            &self.closed_at.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.market)
            || is_zero(&self.source_state)
            || is_zero(&self.source_material)
            || is_zero(&self.capability_manifest)
            || is_zero(&self.terminal_certificate)
            || is_zero(&self.receipt_account)
            || is_zero(&self.beneficiary)
            || is_zero(&self.source_state_digest)
            || is_zero(&self.terminal_certificate_digest)
            || is_zero(&self.funding_set_digest)
            || self.generation == 0
            || self.terminal_sequence == 0
            || self.refund_lamports == 0
            || self.closed_at == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.selector > u32::from(u8::MAX) {
            return Err(Error::InvalidSelector);
        }
        Ok(())
    }
}

/// Candidate binding for the Registry-owned Resolution execution role.
///
/// This value is deliberately not accepted as runtime authority by the
/// controller. The Registry activation binding must eventually authenticate
/// this exact pair before lending the controller authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionReleaseCandidateV1 {
    /// Candidate Resolution program account identity.
    pub program_id: [u8; 32],
    /// Candidate checked artifact-release content identity.
    pub artifact_release_id: [u8; 32],
}

impl ResolutionReleaseCandidateV1 {
    /// Construct a nonzero role-binding candidate.
    pub fn new(program_id: [u8; 32], artifact_release_id: [u8; 32]) -> Result<Self> {
        if is_zero(&program_id) || is_zero(&artifact_release_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(Self {
            program_id,
            artifact_release_id,
        })
    }
}

/// Lean-owned kind of one Source Resolution certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionCertificateKindV1 {
    /// Provider evidence resolved an ordinary Product result.
    ResolutionSuccess,
    /// Funding was consumed and the next ordered recovery became active.
    RecoveryAdvanced,
    /// Funding was consumed and the finite recovery sequence was exhausted.
    Exhausted,
    /// Funding was consumed and Product's explicit failure result was committed.
    ResolutionFailure,
}

impl ResolutionCertificateKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated_source_resolution::CERTIFICATE_RESOLUTION_SUCCESS_KIND => {
                Ok(Self::ResolutionSuccess)
            }
            generated_source_resolution::CERTIFICATE_RECOVERY_ADVANCED_KIND => {
                Ok(Self::RecoveryAdvanced)
            }
            generated_source_resolution::CERTIFICATE_EXHAUSTED_KIND => Ok(Self::Exhausted),
            generated_source_resolution::CERTIFICATE_RESOLUTION_FAILURE_KIND => {
                Ok(Self::ResolutionFailure)
            }
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ResolutionSuccess => {
                generated_source_resolution::CERTIFICATE_RESOLUTION_SUCCESS_KIND
            }
            Self::RecoveryAdvanced => {
                generated_source_resolution::CERTIFICATE_RECOVERY_ADVANCED_KIND
            }
            Self::Exhausted => generated_source_resolution::CERTIFICATE_EXHAUSTED_KIND,
            Self::ResolutionFailure => {
                generated_source_resolution::CERTIFICATE_RESOLUTION_FAILURE_KIND
            }
        }
    }
}

/// Canonical physical projection of one Source Resolution certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionCertificateV1 {
    /// Exact Lean-owned certificate kind.
    pub kind: ResolutionCertificateKindV1,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Exact active Source/provider route; explicit failure uses all zeroes.
    pub route: [u8; 32],
    /// Canonical Source-material content identity.
    pub source_material: [u8; 32],
    /// Canonical Product-instance content identity.
    pub product: [u8; 32],
    /// Authenticated provider evidence; liveness transitions use all zeroes.
    pub provider_evidence: [u8; 32],
    /// Exact funding-allocation identity; the legacy hot primary uses zeroes.
    pub funding_allocation: [u8; 32],
    /// Canonical certificate account identity.
    pub receipt_account: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Ordered recovery index; primary admission uses zero.
    pub attempt_index: u32,
    /// Exact Source schedule index.
    pub schedule_index: u32,
    /// Product-owned result selector.
    pub selector: u32,
    /// Exact work paid by this transition; zero in the primary hot profile.
    pub work_paid: u64,
    /// Authenticated remaining work funding; zero in the primary hot profile.
    pub funding_remaining: u64,
    /// Exact signed normalized result numerator.
    pub result_numerator: i128,
    /// Positive exact result denominator for success; liveness uses zero.
    pub result_denominator: u64,
    /// Provider publication or recovery-transition time; failure uses zero.
    pub observed_at: u64,
}

impl ResolutionCertificateV1 {
    /// Decode one exact canonical certificate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_width(input, RESOLUTION_CERTIFICATE_BYTES)?;
        exact(
            input,
            generated_source_resolution::CERTIFICATE_MAGIC_OFFSET,
            &generated_source_resolution::CERTIFICATE_MAGIC_BYTES,
            Error::InvalidMagic,
        )?;
        if u16_at(
            input,
            generated_source_resolution::CERTIFICATE_VERSION_OFFSET,
        )? != generated_source_resolution::CERTIFICATE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        let kind = ResolutionCertificateKindV1::decode(byte_at(
            input,
            generated_source_resolution::CERTIFICATE_KIND_OFFSET,
        )?)?;
        require_zero(
            input,
            generated_source_resolution::CERTIFICATE_RESERVED_HEADER_OFFSET,
            5,
        )?;
        require_zero(
            input,
            generated_source_resolution::CERTIFICATE_RESERVED_BODY_OFFSET,
            4,
        )?;
        let value = Self {
            kind,
            market: array_at(
                input,
                generated_source_resolution::CERTIFICATE_MARKET_OFFSET,
            )?,
            route: array_at(input, generated_source_resolution::CERTIFICATE_ROUTE_OFFSET)?,
            source_material: array_at(
                input,
                generated_source_resolution::CERTIFICATE_SOURCE_MATERIAL_OFFSET,
            )?,
            product: array_at(
                input,
                generated_source_resolution::CERTIFICATE_PRODUCT_OFFSET,
            )?,
            provider_evidence: array_at(
                input,
                generated_source_resolution::CERTIFICATE_PROVIDER_EVIDENCE_OFFSET,
            )?,
            funding_allocation: array_at(
                input,
                generated_source_resolution::CERTIFICATE_FUNDING_ALLOCATION_OFFSET,
            )?,
            receipt_account: array_at(
                input,
                generated_source_resolution::CERTIFICATE_RECEIPT_ACCOUNT_OFFSET,
            )?,
            generation: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_GENERATION_OFFSET,
            )?,
            attempt_index: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_ATTEMPT_INDEX_OFFSET,
            )?,
            schedule_index: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_SCHEDULE_INDEX_OFFSET,
            )?,
            selector: u32_at(
                input,
                generated_source_resolution::CERTIFICATE_SELECTOR_OFFSET,
            )?,
            work_paid: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_WORK_PAID_OFFSET,
            )?,
            funding_remaining: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_FUNDING_REMAINING_OFFSET,
            )?,
            result_numerator: i128_at(
                input,
                generated_source_resolution::CERTIFICATE_RESULT_NUMERATOR_OFFSET,
            )?,
            result_denominator: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_RESULT_DENOMINATOR_OFFSET,
            )?,
            observed_at: u64_at(
                input,
                generated_source_resolution::CERTIFICATE_OBSERVED_AT_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical certificate.
    pub fn to_bytes(self) -> Result<[u8; RESOLUTION_CERTIFICATE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; RESOLUTION_CERTIFICATE_BYTES];
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_MAGIC_OFFSET,
            &generated_source_resolution::CERTIFICATE_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_VERSION_OFFSET,
            &generated_source_resolution::CERTIFICATE_ABI_VERSION.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_KIND_OFFSET,
            &[self.kind.byte()],
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_MARKET_OFFSET,
            &self.market,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_ROUTE_OFFSET,
            &self.route,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SOURCE_MATERIAL_OFFSET,
            &self.source_material,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_PRODUCT_OFFSET,
            &self.product,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_PROVIDER_EVIDENCE_OFFSET,
            &self.provider_evidence,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_FUNDING_ALLOCATION_OFFSET,
            &self.funding_allocation,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RECEIPT_ACCOUNT_OFFSET,
            &self.receipt_account,
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_ATTEMPT_INDEX_OFFSET,
            &self.attempt_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SCHEDULE_INDEX_OFFSET,
            &self.schedule_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_WORK_PAID_OFFSET,
            &self.work_paid.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_FUNDING_REMAINING_OFFSET,
            &self.funding_remaining.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RESULT_NUMERATOR_OFFSET,
            &self.result_numerator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_RESULT_DENOMINATOR_OFFSET,
            &self.result_denominator.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_source_resolution::CERTIFICATE_OBSERVED_AT_OFFSET,
            &self.observed_at.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if is_zero(&self.market)
            || is_zero(&self.source_material)
            || is_zero(&self.product)
            || is_zero(&self.receipt_account)
            || self.generation == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.selector > u32::from(u8::MAX) {
            return Err(Error::InvalidSelector);
        }
        match self.kind {
            ResolutionCertificateKindV1::ResolutionSuccess => {
                if is_zero(&self.route)
                    || is_zero(&self.provider_evidence)
                    || self.result_denominator == 0
                    || self.observed_at == 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
            ResolutionCertificateKindV1::RecoveryAdvanced
            | ResolutionCertificateKindV1::Exhausted => {
                if is_zero(&self.route)
                    || is_zero(&self.funding_allocation)
                    || !is_zero(&self.provider_evidence)
                    || self.work_paid == 0
                    || self.result_numerator != 0
                    || self.result_denominator != 0
                    || self.observed_at == 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
            ResolutionCertificateKindV1::ResolutionFailure => {
                if !is_zero(&self.route)
                    || is_zero(&self.funding_allocation)
                    || !is_zero(&self.provider_evidence)
                    || self.work_paid == 0
                    || self.schedule_index != 0
                    || self.result_numerator != 0
                    || self.result_denominator != 0
                    || self.observed_at != 0
                {
                    return Err(Error::ZeroCoordinate);
                }
            }
        }
        Ok(())
    }
}

fn exact_width(input: &[u8], expected: usize) -> Result<()> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8], error: Error) -> Result<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(Error::InvalidLength)?;
    if input.get(offset..end) == Some(expected) {
        Ok(())
    } else {
        Err(error)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn i128_at(input: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array_at(input, offset)?))
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AcceptPythRequestV1 {
        AcceptPythRequestV1 {
            expected_generation: 7,
            expected_result_domain_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x22;
                value[1] = 0x11;
                value
            },
            expected_provider_release_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x44;
                value[1] = 0x33;
                value
            },
        }
    }

    fn certificate() -> ResolutionCertificateV1 {
        ResolutionCertificateV1 {
            kind: ResolutionCertificateKindV1::ResolutionSuccess,
            market: [1; 32],
            route: [2; 32],
            source_material: [3; 32],
            product: [4; 32],
            provider_evidence: [5; 32],
            funding_allocation: [0; 32],
            receipt_account: [6; 32],
            generation: 7,
            attempt_index: 0,
            schedule_index: 0,
            selector: 2,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: -123_456_789,
            result_denominator: 1,
            observed_at: 1_700_000_000,
        }
    }

    fn funded_request() -> FundedTransitionRequestV3 {
        FundedTransitionRequestV3 {
            action: FundedTransitionActionV3::FailNext,
            expected_generation: 7,
            expected_recovery_index: 0,
            expected_result_domain_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x22;
                value[1] = 0x11;
                value
            },
            expected_funding_allocation_id: {
                let mut value = [0_u8; 32];
                value[0] = 0x44;
                value[1] = 0x33;
                value
            },
        }
    }

    fn funded_receipt() -> FundedTransitionReceiptV1 {
        FundedTransitionReceiptV1 {
            action: FundedTransitionActionV3::FailNext,
            post_phase: FundedReceiptPostPhaseV1::Recovery,
            certificate_kind: ResolutionCertificateKindV1::RecoveryAdvanced,
            terminal_refund_phase: FundedTerminalRefundPhaseV1::Continuing,
            producer_program: id(1),
            producer_release: id(2),
            request_digest: id(3),
            source_state: id(4),
            funding_state: id(5),
            worker: id(6),
            certificate: id(7),
            pre_source_digest: id(8),
            post_source_digest: id(9),
            funding_post_digest: id(10),
            generation: 11,
            replay_sequence: 12,
            work_paid: 13,
            funding_remaining: 14,
            selector: 0,
        }
    }

    fn core_request() -> ResolutionRoleRequestV1 {
        ResolutionRoleRequestV1 {
            action: ResolutionCoreActionV1::CloseFund,
            receipt_kind: ResolutionCoreReceiptKindV1::Closure,
            source_state: id(1),
            source_material: id(2),
            capability_manifest: id(3),
            recovery_funding: id(4),
            exhaustion_funding: id(5),
            failure_funding: id(6),
            receipt: id(7),
            beneficiary: id(8),
            recovery_entry_index: 0,
            exhaustion_entry_index: 1,
            failure_entry_index: 2,
            receipt_sequence: 4,
        }
    }

    fn closure() -> SourceClosureReceiptV1 {
        SourceClosureReceiptV1 {
            market: id(1),
            source_state: id(2),
            source_material: id(3),
            capability_manifest: id(4),
            terminal_certificate: id(5),
            receipt_account: id(6),
            beneficiary: id(7),
            source_state_digest: id(8),
            terminal_certificate_digest: id(9),
            funding_set_digest: id(10),
            generation: 11,
            terminal_sequence: 12,
            selector: 2,
            refund_lamports: 13,
            closed_at: 14,
        }
    }

    fn id(first_byte: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = first_byte;
        value
    }

    #[test]
    fn generated_request_vector_and_round_trip_match() -> Result<()> {
        let encoded = request().to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::REQUEST_EXAMPLE);
        assert_eq!(AcceptPythRequestV1::decode(&encoded), Ok(request()));
        Ok(())
    }

    #[test]
    fn every_request_truncation_and_hostile_header_refuses() -> Result<()> {
        let encoded = request().to_bytes()?;
        for length in 0..ACCEPT_PYTH_REQUEST_BYTES {
            assert_eq!(
                AcceptPythRequestV1::decode(encoded.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        let mut long = [0_u8; ACCEPT_PYTH_REQUEST_BYTES + 1];
        long.get_mut(..ACCEPT_PYTH_REQUEST_BYTES)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&encoded);
        assert_eq!(
            AcceptPythRequestV1::decode(&long),
            Err(Error::InvalidLength)
        );

        for (offset, error) in [
            (
                generated_source_resolution::REQUEST_MAGIC_OFFSET,
                Error::InvalidMagic,
            ),
            (
                generated_source_resolution::REQUEST_VERSION_OFFSET,
                Error::UnsupportedVersion,
            ),
            (
                generated_source_resolution::REQUEST_ACTION_OFFSET,
                Error::UnknownAction,
            ),
            (
                generated_source_resolution::REQUEST_RESERVED_OFFSET,
                Error::NonCanonicalReserved,
            ),
        ] {
            let mut hostile = encoded;
            *hostile.get_mut(offset).ok_or(Error::InvalidLength)? ^= 1;
            assert_eq!(AcceptPythRequestV1::decode(&hostile), Err(error));
        }
        Ok(())
    }

    #[test]
    fn generated_funded_request_vector_and_hostile_bytes_match() -> Result<()> {
        let encoded = funded_request().to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::FUNDED_REQUEST_EXAMPLE);
        assert_eq!(
            FundedTransitionRequestV3::decode(&encoded),
            Ok(funded_request())
        );
        for length in 0..FUNDED_TRANSITION_REQUEST_BYTES {
            assert_eq!(
                FundedTransitionRequestV3::decode(
                    encoded.get(..length).ok_or(Error::InvalidLength)?,
                ),
                Err(Error::InvalidLength)
            );
        }
        for (offset, error) in [
            (
                generated_source_resolution::FUNDED_REQUEST_ACTION_OFFSET,
                Error::UnknownAction,
            ),
            (
                generated_source_resolution::FUNDED_REQUEST_RESERVED_BODY_OFFSET,
                Error::NonCanonicalReserved,
            ),
        ] {
            let mut hostile = encoded;
            *hostile.get_mut(offset).ok_or(Error::InvalidLength)? = 0xff;
            assert_eq!(FundedTransitionRequestV3::decode(&hostile), Err(error));
        }
        for action in [
            FundedTransitionActionV3::FailNext,
            FundedTransitionActionV3::Exhaust,
            FundedTransitionActionV3::CommitFailure,
        ] {
            let request = FundedTransitionRequestV3 {
                action,
                ..funded_request()
            };
            assert_eq!(
                FundedTransitionRequestV3::decode(&request.to_bytes()?),
                Ok(request)
            );
        }
        Ok(())
    }

    #[test]
    fn generated_funded_receipt_partition_and_hostile_bytes_match() -> Result<()> {
        let value = funded_receipt();
        let encoded = value.to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::FUNDED_RECEIPT_EXAMPLE);
        assert_eq!(FundedTransitionReceiptV1::decode(&encoded), Ok(value));
        for length in 0..FUNDED_TRANSITION_RECEIPT_BYTES {
            assert_eq!(
                FundedTransitionReceiptV1::decode(
                    encoded.get(..length).ok_or(Error::InvalidLength)?,
                ),
                Err(Error::InvalidLength)
            );
        }

        for (offset, expected) in [
            (
                generated_source_resolution::FUNDED_RECEIPT_POST_PHASE_OFFSET,
                Error::InvalidReceiptShape,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_CERTIFICATE_KIND_OFFSET,
                Error::UnknownAction,
            ),
            (
                generated_source_resolution::FUNDED_RECEIPT_TERMINAL_REFUND_PHASE_OFFSET,
                Error::InvalidReceiptShape,
            ),
        ] {
            let mut hostile = encoded;
            *hostile.get_mut(offset).expect("generated receipt offset") = 0xff;
            assert_eq!(FundedTransitionReceiptV1::decode(&hostile), Err(expected));
        }
        let mut hostile = encoded;
        hostile[generated_source_resolution::FUNDED_RECEIPT_RESERVED_BODY_OFFSET] = 1;
        assert_eq!(
            FundedTransitionReceiptV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );

        let exhausted = FundedTransitionReceiptV1 {
            action: FundedTransitionActionV3::Exhaust,
            post_phase: FundedReceiptPostPhaseV1::Exhausted,
            certificate_kind: ResolutionCertificateKindV1::Exhausted,
            terminal_refund_phase: FundedTerminalRefundPhaseV1::AwaitingFailure,
            ..value
        };
        assert_eq!(
            FundedTransitionReceiptV1::decode(&exhausted.to_bytes()?),
            Ok(exhausted)
        );
        let failure = FundedTransitionReceiptV1 {
            action: FundedTransitionActionV3::CommitFailure,
            post_phase: FundedReceiptPostPhaseV1::FailureCommitted,
            certificate_kind: ResolutionCertificateKindV1::ResolutionFailure,
            terminal_refund_phase: FundedTerminalRefundPhaseV1::TerminalRefundPending,
            selector: u32::from(u8::MAX),
            ..value
        };
        assert_eq!(
            FundedTransitionReceiptV1::decode(&failure.to_bytes()?),
            Ok(failure)
        );
        let wrong_phase = FundedTransitionReceiptV1 {
            post_phase: FundedReceiptPostPhaseV1::Exhausted,
            ..value
        };
        assert_eq!(wrong_phase.to_bytes(), Err(Error::InvalidReceiptShape));
        let missing_payout = FundedTransitionReceiptV1 {
            work_paid: 0,
            ..value
        };
        assert_eq!(missing_payout.to_bytes(), Err(Error::ZeroCoordinate));
        Ok(())
    }

    #[test]
    fn generated_core_request_partition_and_hostile_coordinates_match() -> Result<()> {
        let value = core_request();
        let encoded = value.to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::CORE_REQUEST_EXAMPLE);
        assert_eq!(ResolutionRoleRequestV1::decode(&encoded), Ok(value));

        for length in 0..RESOLUTION_CORE_ROLE_REQUEST_BYTES {
            assert_eq!(
                ResolutionRoleRequestV1::decode(encoded.get(..length).ok_or(Error::InvalidLength)?,),
                Err(Error::InvalidLength)
            );
        }

        for (action, receipt_kind, receipt, beneficiary, receipt_sequence) in [
            (
                ResolutionCoreActionV1::CreateFund,
                ResolutionCoreReceiptKindV1::None,
                [0; 32],
                id(8),
                0,
            ),
            (
                ResolutionCoreActionV1::VerifyFundReady,
                ResolutionCoreReceiptKindV1::None,
                [0; 32],
                id(8),
                0,
            ),
            (
                ResolutionCoreActionV1::AdmitTerminal,
                ResolutionCoreReceiptKindV1::TerminalSuccess,
                id(7),
                [0; 32],
                4,
            ),
            (
                ResolutionCoreActionV1::AdmitTerminal,
                ResolutionCoreReceiptKindV1::TerminalFailure,
                id(7),
                [0; 32],
                4,
            ),
            (
                ResolutionCoreActionV1::CloseFund,
                ResolutionCoreReceiptKindV1::Closure,
                id(7),
                id(8),
                4,
            ),
        ] {
            let request = ResolutionRoleRequestV1 {
                action,
                receipt_kind,
                receipt,
                beneficiary,
                receipt_sequence,
                ..value
            };
            assert_eq!(
                ResolutionRoleRequestV1::decode(&request.to_bytes()?),
                Ok(request)
            );
        }

        let mut duplicate = value;
        duplicate.failure_funding = duplicate.recovery_funding;
        assert_eq!(duplicate.to_bytes(), Err(Error::DuplicateCoordinate));
        let mut duplicate = value;
        duplicate.failure_entry_index = duplicate.recovery_entry_index;
        assert_eq!(duplicate.to_bytes(), Err(Error::DuplicateCoordinate));
        let mut wrong_shape = value;
        wrong_shape.receipt_kind = ResolutionCoreReceiptKindV1::TerminalFailure;
        assert_eq!(wrong_shape.to_bytes(), Err(Error::InvalidReceiptShape));

        for (offset, expected) in [
            (
                generated_source_resolution::CORE_REQUEST_ACTION_OFFSET,
                Error::UnknownAction,
            ),
            (
                generated_source_resolution::CORE_REQUEST_RESERVED_HEADER_OFFSET,
                Error::NonCanonicalReserved,
            ),
            (
                generated_source_resolution::CORE_REQUEST_RESERVED_BODY_OFFSET,
                Error::NonCanonicalReserved,
            ),
        ] {
            let mut hostile = encoded;
            *hostile.get_mut(offset).ok_or(Error::InvalidLength)? = 0xff;
            assert_eq!(ResolutionRoleRequestV1::decode(&hostile), Err(expected));
        }
        Ok(())
    }

    #[test]
    fn generated_closure_round_trip_and_hostile_discharge_facts_match() -> Result<()> {
        let value = closure();
        let encoded = value.to_bytes()?;
        assert_eq!(encoded, generated_source_resolution::CLOSURE_EXAMPLE);
        assert_eq!(SourceClosureReceiptV1::decode(&encoded), Ok(value));

        for length in 0..SOURCE_CLOSURE_RECEIPT_BYTES {
            assert_eq!(
                SourceClosureReceiptV1::decode(encoded.get(..length).ok_or(Error::InvalidLength)?,),
                Err(Error::InvalidLength)
            );
        }

        let mut hostile = encoded;
        hostile[generated_source_resolution::CLOSURE_FUNDING_COUNT_OFFSET] = 2;
        assert_eq!(
            SourceClosureReceiptV1::decode(&hostile),
            Err(Error::InvalidFundingCount)
        );
        let mut hostile = encoded;
        hostile[generated_source_resolution::CLOSURE_RESERVED_BODY_OFFSET] = 1;
        assert_eq!(
            SourceClosureReceiptV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
        let mut hostile = encoded;
        hostile[generated_source_resolution::CLOSURE_KIND_OFFSET] = 0xff;
        assert_eq!(
            SourceClosureReceiptV1::decode(&hostile),
            Err(Error::InvalidReceiptShape)
        );
        let mut hostile_value = value;
        hostile_value.refund_lamports = 0;
        assert_eq!(hostile_value.to_bytes(), Err(Error::ZeroCoordinate));
        let mut hostile_value = value;
        hostile_value.selector = 256;
        assert_eq!(hostile_value.to_bytes(), Err(Error::InvalidSelector));
        Ok(())
    }

    #[test]
    fn certificate_round_trip_is_exact_and_reserved_bytes_are_hostile() -> Result<()> {
        let value = certificate();
        let encoded = value.to_bytes()?;
        assert_eq!(encoded.len(), RESOLUTION_CERTIFICATE_BYTES);
        assert_eq!(ResolutionCertificateV1::decode(&encoded), Ok(value));
        assert!(encoded[11..16].iter().all(|byte| *byte == 0));
        assert!(encoded[260..264].iter().all(|byte| *byte == 0));

        let mut hostile = encoded;
        hostile[generated_source_resolution::CERTIFICATE_RESERVED_BODY_OFFSET] = 1;
        assert_eq!(
            ResolutionCertificateV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
        Ok(())
    }

    #[test]
    fn liveness_certificate_kinds_partition_inactive_fields() -> Result<()> {
        let recovery = ResolutionCertificateV1 {
            kind: ResolutionCertificateKindV1::RecoveryAdvanced,
            provider_evidence: [0; 32],
            funding_allocation: [7; 32],
            work_paid: 9,
            result_numerator: 0,
            result_denominator: 0,
            observed_at: 101,
            ..certificate()
        };
        assert_eq!(
            ResolutionCertificateV1::decode(&recovery.to_bytes()?),
            Ok(recovery)
        );

        let failure = ResolutionCertificateV1 {
            kind: ResolutionCertificateKindV1::ResolutionFailure,
            route: [0; 32],
            provider_evidence: [0; 32],
            funding_allocation: [8; 32],
            schedule_index: 0,
            work_paid: 11,
            result_numerator: 0,
            result_denominator: 0,
            observed_at: 0,
            ..certificate()
        };
        assert_eq!(
            ResolutionCertificateV1::decode(&failure.to_bytes()?),
            Ok(failure)
        );

        let mut hostile = recovery;
        hostile.provider_evidence = [9; 32];
        assert_eq!(hostile.to_bytes(), Err(Error::ZeroCoordinate));
        let mut hostile = failure;
        hostile.route = [9; 32];
        assert_eq!(hostile.to_bytes(), Err(Error::ZeroCoordinate));
        Ok(())
    }

    #[test]
    fn release_candidate_is_not_an_implicit_zero_authority() {
        assert_eq!(
            ResolutionReleaseCandidateV1::new([0; 32], [1; 32]),
            Err(Error::ZeroCoordinate)
        );
        assert_eq!(
            ResolutionReleaseCandidateV1::new([1; 32], [0; 32]),
            Err(Error::ZeroCoordinate)
        );
        assert!(ResolutionReleaseCandidateV1::new([1; 32], [2; 32]).is_ok());
    }
}
