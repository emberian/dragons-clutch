//! Frozen SourcePlane V3 wire and ordered-account contract for SourceSeries 77/v2.
//!
//! This module owns instruction bytes and account-role geometry only. It does
//! not authenticate a release artifact, deployment, Source account, PDA,
//! Clock, CPI, rent quote, liveness receipt, balance, or signature. Those are
//! obligations of the SBF adapter. All twelve coordinates remain separately
//! capability-gated; decoding one of these payloads grants no authority.

use clutch_source_plane_v3_adapter::{
    Error as SourceAdapterError, IntentPreimageV3, TransitionActionV3, INTENT_PREIMAGE_BYTES,
};

use crate::{artifact::ArtifactKind, is_zero, registry, CodecError, Result, HASH_BYTES};

/// Exact canonical `RegisterRelease` payload width.
pub const REGISTER_RELEASE_PAYLOAD_BYTES_V2: usize = HASH_BYTES;
/// Exact canonical transition payload width for actions 2 through 9.
pub const SOURCE_TRANSITION_PAYLOAD_BYTES_V2: usize = INTENT_PREIMAGE_BYTES;
/// Exact canonical `EmitFailureHandoff` payload width.
pub const EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2: usize = 80;
/// Exact canonical `ReopenGeneration` payload width.
pub const REOPEN_GENERATION_PAYLOAD_BYTES_V2: usize = 144;
/// Exact canonical `CloseGeneration` payload width.
pub const CLOSE_GENERATION_PAYLOAD_BYTES_V2: usize = 112;
/// Typed artifact required by `RegisterRelease`.
pub const SOURCE_RELEASE_ARTIFACT_KIND_V2: ArtifactKind = ArtifactKind::SourceReleaseManifestV2;
/// Exact body bytes transported by the Source release artifact.
pub const SOURCE_RELEASE_ARTIFACT_BODY_BYTES_V2: usize = 1_296;

const _: () = {
    assert!(SOURCE_TRANSITION_PAYLOAD_BYTES_V2 == 160);
    assert!(SOURCE_RELEASE_ARTIFACT_BODY_BYTES_V2 == SOURCE_RELEASE_ARTIFACT_KIND_V2.exact_len());
    assert!(registry::SOURCE_V3_RELEASE_ACCOUNT_VERSION_V1 == 1);
    assert!(registry::SOURCE_V3_RELEASE_ACCOUNT_VERSION == 2);
    assert!(registry::SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION == 2);
    assert!(registry::SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_BYTES == 352);
};

fn require_exact(input: &[u8], exact: usize) -> Result<()> {
    if input.len() < exact {
        Err(CodecError::Truncated)
    } else if input.len() > exact {
        Err(CodecError::TrailingBytes)
    } else {
        Ok(())
    }
}

fn require_output(output: &[u8], exact: usize) -> Result<()> {
    if output.len() < exact {
        Err(CodecError::OutputTooSmall)
    } else if output.len() > exact {
        Err(CodecError::TrailingBytes)
    } else {
        Ok(())
    }
}

fn require_live(id: [u8; HASH_BYTES]) -> Result<()> {
    if is_zero(&id) {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_reserved(input: &[u8]) -> Result<()> {
    if input.iter().any(|byte| *byte != 0) {
        Err(CodecError::NonCanonicalPadding)
    } else {
        Ok(())
    }
}

fn id_at(input: &[u8], offset: usize) -> [u8; HASH_BYTES] {
    let mut id = [0; HASH_BYTES];
    id.copy_from_slice(&input[offset..offset + HASH_BYTES]);
    id
}

fn u64_at(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn map_source_adapter_error(error: SourceAdapterError) -> CodecError {
    match error {
        SourceAdapterError::WrongLength => CodecError::Truncated,
        SourceAdapterError::WrongMagic => CodecError::WrongTag,
        SourceAdapterError::BadVersion => CodecError::WrongVersion,
        SourceAdapterError::InvalidParameter => CodecError::InvalidCount,
        SourceAdapterError::ZeroIdentity => CodecError::ZeroIdentity,
        SourceAdapterError::NonCanonicalPadding => CodecError::NonCanonicalPadding,
        SourceAdapterError::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        SourceAdapterError::MismatchedState => CodecError::MismatchedBinding,
        _ => CodecError::MismatchedBinding,
    }
}

/// Registration of one sealed, globally content-addressed 1,296-byte release artifact.
///
/// The manifest body is deliberately absent from the instruction. The account
/// in role [`SourceAccountRoleV2::SourceReleaseArtifact`] must be a sealed
/// [`ArtifactKind::SourceReleaseManifestV2`] whose binding digest equals this
/// value; the SBF adapter then hostile-decodes those exact artifact bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterReleaseIntentV2 {
    /// Canonical `SourceReleaseManifestV2::id()` and sealed artifact digest.
    pub source_release_manifest_id: [u8; HASH_BYTES],
}

impl RegisterReleaseIntentV2 {
    /// Encode exactly [`REGISTER_RELEASE_PAYLOAD_BYTES_V2`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        require_output(output, REGISTER_RELEASE_PAYLOAD_BYTES_V2)?;
        require_live(self.source_release_manifest_id)?;
        output.copy_from_slice(&self.source_release_manifest_id);
        Ok(())
    }

    /// Hostile-decode exactly [`REGISTER_RELEASE_PAYLOAD_BYTES_V2`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, REGISTER_RELEASE_PAYLOAD_BYTES_V2)?;
        let value = Self {
            source_release_manifest_id: id_at(input, 0),
        };
        require_live(value.source_release_manifest_id)?;
        Ok(value)
    }
}

/// Closed handoff shape emitted by action 10.
///
/// Every admitted byte has one current Source-owned fact constructor. The
/// downstream Failure runtime may consume these facts, but it cannot choose a
/// branch or mint a Source handoff from caller-supplied identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceHandoffKindV2 {
    /// Primary maturity passed while the content-addressed result slot remained absent.
    FailureAbsence = 1,
    /// The reviewed evaluator persisted a stable nonzero refusal result.
    FailureResult = 2,
    /// A persisted successful evaluation produced the downstream-review handoff.
    SuccessfulEvaluation = 3,
}

impl SourceHandoffKindV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::FailureAbsence => 1,
            Self::FailureResult => 2,
            Self::SuccessfulEvaluation => 3,
        }
    }

    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::FailureAbsence),
            2 => Ok(Self::FailureResult),
            3 => Ok(Self::SuccessfulEvaluation),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Exact action-10 handoff commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmitFailureHandoffIntentV2 {
    /// Closed semantic handoff branch.
    pub kind: SourceHandoffKindV2,
    /// Canonical failure or successful-evaluation handoff identity.
    pub handoff_id: [u8; HASH_BYTES],
    /// Persisted Source work receipt joined to the handoff.
    pub source_work_receipt_id: [u8; HASH_BYTES],
    /// Exclusive runtime slot expiry.
    pub valid_before_slot: u64,
}

impl EmitFailureHandoffIntentV2 {
    fn validate(&self) -> Result<()> {
        require_live(self.handoff_id)?;
        require_live(self.source_work_receipt_id)?;
        if self.valid_before_slot == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode exactly [`EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        require_output(output, EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2)?;
        self.validate()?;
        output.fill(0);
        output[0] = self.kind.wire_byte();
        output[8..40].copy_from_slice(&self.handoff_id);
        output[40..72].copy_from_slice(&self.source_work_receipt_id);
        output[72..80].copy_from_slice(&self.valid_before_slot.to_le_bytes());
        Ok(())
    }

    /// Hostile-decode exactly [`EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2)?;
        require_reserved(&input[1..8])?;
        let value = Self {
            kind: SourceHandoffKindV2::decode(input[0])?,
            handoff_id: id_at(input, 8),
            source_work_receipt_id: id_at(input, 40),
            valid_before_slot: u64_at(input, 72),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Mutable Source family admitted by generic reopen/close actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceMutableFamilyV2 {
    /// SourceHead generation.
    SourceHead = 1,
    /// OpenRawPage generation.
    OpenRawPage = 2,
    /// WindowWork generation.
    WindowWork = 3,
    /// Repairable StatisticResult generation.
    StatisticResult = 4,
}

impl SourceMutableFamilyV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::SourceHead => 1,
            Self::OpenRawPage => 2,
            Self::WindowWork => 3,
            Self::StatisticResult => 4,
        }
    }

    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::SourceHead),
            2 => Ok(Self::OpenRawPage),
            3 => Ok(Self::WindowWork),
            4 => Ok(Self::StatisticResult),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Exact action-11 reopen commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReopenGenerationIntentV2 {
    /// Closed mutable account family.
    pub family: SourceMutableFamilyV2,
    /// Exact release under which the new generation is authorized.
    pub source_release_manifest_id: [u8; HASH_BYTES],
    /// Digest of the complete expected lineage preimage.
    pub expected_lineage_state_id: [u8; HASH_BYTES],
    /// Immutable semantic binding selected by the family's PDA recipe.
    pub semantic_binding_id: [u8; HASH_BYTES],
    /// Canonical semantic body that must be recomputed before creation.
    pub target_body_id: [u8; HASH_BYTES],
    /// Exclusive runtime slot expiry.
    pub valid_before_slot: u64,
}

impl ReopenGenerationIntentV2 {
    fn validate(&self) -> Result<()> {
        for id in [
            self.source_release_manifest_id,
            self.expected_lineage_state_id,
            self.semantic_binding_id,
            self.target_body_id,
        ] {
            require_live(id)?;
        }
        if self.valid_before_slot == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode exactly [`REOPEN_GENERATION_PAYLOAD_BYTES_V2`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        require_output(output, REOPEN_GENERATION_PAYLOAD_BYTES_V2)?;
        self.validate()?;
        output.fill(0);
        output[0] = self.family.wire_byte();
        output[8..40].copy_from_slice(&self.source_release_manifest_id);
        output[40..72].copy_from_slice(&self.expected_lineage_state_id);
        output[72..104].copy_from_slice(&self.semantic_binding_id);
        output[104..136].copy_from_slice(&self.target_body_id);
        output[136..144].copy_from_slice(&self.valid_before_slot.to_le_bytes());
        Ok(())
    }

    /// Hostile-decode exactly [`REOPEN_GENERATION_PAYLOAD_BYTES_V2`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, REOPEN_GENERATION_PAYLOAD_BYTES_V2)?;
        require_reserved(&input[1..8])?;
        let value = Self {
            family: SourceMutableFamilyV2::decode(input[0])?,
            source_release_manifest_id: id_at(input, 8),
            expected_lineage_state_id: id_at(input, 40),
            semantic_binding_id: id_at(input, 72),
            target_body_id: id_at(input, 104),
            valid_before_slot: u64_at(input, 136),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact action-12 terminal-generation commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseGenerationIntentV2 {
    /// Closed mutable account family.
    pub family: SourceMutableFamilyV2,
    /// Exact release under which the generation was opened.
    pub source_release_manifest_id: [u8; HASH_BYTES],
    /// Digest of the complete expected open-lineage preimage.
    pub expected_lineage_state_id: [u8; HASH_BYTES],
    /// Terminal semantic receipt authorizing this close.
    pub semantic_terminal_receipt_id: [u8; HASH_BYTES],
    /// Exclusive runtime slot expiry.
    pub valid_before_slot: u64,
}

impl CloseGenerationIntentV2 {
    fn validate(&self) -> Result<()> {
        for id in [
            self.source_release_manifest_id,
            self.expected_lineage_state_id,
            self.semantic_terminal_receipt_id,
        ] {
            require_live(id)?;
        }
        if self.valid_before_slot == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode exactly [`CLOSE_GENERATION_PAYLOAD_BYTES_V2`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        require_output(output, CLOSE_GENERATION_PAYLOAD_BYTES_V2)?;
        self.validate()?;
        output.fill(0);
        output[0] = self.family.wire_byte();
        output[8..40].copy_from_slice(&self.source_release_manifest_id);
        output[40..72].copy_from_slice(&self.expected_lineage_state_id);
        output[72..104].copy_from_slice(&self.semantic_terminal_receipt_id);
        output[104..112].copy_from_slice(&self.valid_before_slot.to_le_bytes());
        Ok(())
    }

    /// Hostile-decode exactly [`CLOSE_GENERATION_PAYLOAD_BYTES_V2`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, CLOSE_GENERATION_PAYLOAD_BYTES_V2)?;
        require_reserved(&input[1..8])?;
        let value = Self {
            family: SourceMutableFamilyV2::decode(input[0])?,
            source_release_manifest_id: id_at(input, 8),
            expected_lineage_state_id: id_at(input, 40),
            semantic_terminal_receipt_id: id_at(input, 72),
            valid_before_slot: u64_at(input, 104),
        };
        value.validate()?;
        Ok(value)
    }
}

/// One exact action-owned SourceSeries 77/v2 payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSeriesPayloadV2 {
    /// Action 1.
    RegisterRelease(RegisterReleaseIntentV2),
    /// Actions 2 through 9; the inner preimage owns the precise transition tag.
    Transition(IntentPreimageV3),
    /// Action 10.
    EmitFailureHandoff(EmitFailureHandoffIntentV2),
    /// Action 11.
    ReopenGeneration(ReopenGenerationIntentV2),
    /// Action 12.
    CloseGeneration(CloseGenerationIntentV2),
}

/// Return the exact payload width selected by one allocated action.
pub const fn payload_bytes_v2(action: registry::SourceSeriesAction) -> usize {
    match action {
        registry::SourceSeriesAction::RegisterRelease => REGISTER_RELEASE_PAYLOAD_BYTES_V2,
        registry::SourceSeriesAction::InitializeHead
        | registry::SourceSeriesAction::OpenRawPage
        | registry::SourceSeriesAction::IngestBoundaryBatch
        | registry::SourceSeriesAction::SealRawPage
        | registry::SourceSeriesAction::InitializeWindowWork
        | registry::SourceSeriesAction::FoldWindowPages
        | registry::SourceSeriesAction::SealWindow
        | registry::SourceSeriesAction::EvaluateStatistic => SOURCE_TRANSITION_PAYLOAD_BYTES_V2,
        registry::SourceSeriesAction::EmitFailureHandoff => EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2,
        registry::SourceSeriesAction::ReopenGeneration => REOPEN_GENERATION_PAYLOAD_BYTES_V2,
        registry::SourceSeriesAction::CloseGeneration => CLOSE_GENERATION_PAYLOAD_BYTES_V2,
    }
}

fn transition_matches_action(
    action: registry::SourceSeriesAction,
    transition: TransitionActionV3,
) -> bool {
    matches!(
        (action, transition),
        (
            registry::SourceSeriesAction::InitializeHead,
            TransitionActionV3::InitializeSourceHead
        ) | (
            registry::SourceSeriesAction::OpenRawPage,
            TransitionActionV3::OpenRawPage
        ) | (
            registry::SourceSeriesAction::IngestBoundaryBatch,
            TransitionActionV3::AppendBoundary
        ) | (
            registry::SourceSeriesAction::SealRawPage,
            TransitionActionV3::SealRawPage
        ) | (
            registry::SourceSeriesAction::InitializeWindowWork,
            TransitionActionV3::CreateWindowWork
        ) | (
            registry::SourceSeriesAction::FoldWindowPages,
            TransitionActionV3::FoldWindowPage
        ) | (
            registry::SourceSeriesAction::SealWindow,
            TransitionActionV3::SealWindow
        ) | (
            registry::SourceSeriesAction::EvaluateStatistic,
            TransitionActionV3::WriteTerminalResult | TransitionActionV3::WriteDrawdownResult
        )
    )
}

/// Strictly encode the payload variant selected by `action`.
pub fn encode_payload_v2(
    action: registry::SourceSeriesAction,
    payload: SourceSeriesPayloadV2,
    output: &mut [u8],
) -> Result<()> {
    require_output(output, payload_bytes_v2(action))?;
    match (action, payload) {
        (
            registry::SourceSeriesAction::RegisterRelease,
            SourceSeriesPayloadV2::RegisterRelease(v),
        ) => v.encode(output),
        (action, SourceSeriesPayloadV2::Transition(v))
            if transition_matches_action(action, v.action()) =>
        {
            output.copy_from_slice(&v.encode().map_err(map_source_adapter_error)?);
            Ok(())
        }
        (
            registry::SourceSeriesAction::EmitFailureHandoff,
            SourceSeriesPayloadV2::EmitFailureHandoff(v),
        ) => v.encode(output),
        (
            registry::SourceSeriesAction::ReopenGeneration,
            SourceSeriesPayloadV2::ReopenGeneration(v),
        ) => v.encode(output),
        (
            registry::SourceSeriesAction::CloseGeneration,
            SourceSeriesPayloadV2::CloseGeneration(v),
        ) => v.encode(output),
        _ => Err(CodecError::MismatchedBinding),
    }
}

/// Strictly decode the exact payload shape and transition tag selected by `action`.
pub fn decode_payload_v2(
    action: registry::SourceSeriesAction,
    input: &[u8],
) -> Result<SourceSeriesPayloadV2> {
    require_exact(input, payload_bytes_v2(action))?;
    match action {
        registry::SourceSeriesAction::RegisterRelease => Ok(
            SourceSeriesPayloadV2::RegisterRelease(RegisterReleaseIntentV2::decode(input)?),
        ),
        registry::SourceSeriesAction::InitializeHead
        | registry::SourceSeriesAction::OpenRawPage
        | registry::SourceSeriesAction::IngestBoundaryBatch
        | registry::SourceSeriesAction::SealRawPage
        | registry::SourceSeriesAction::InitializeWindowWork
        | registry::SourceSeriesAction::FoldWindowPages
        | registry::SourceSeriesAction::SealWindow
        | registry::SourceSeriesAction::EvaluateStatistic => {
            let intent = IntentPreimageV3::decode(input).map_err(map_source_adapter_error)?;
            if !transition_matches_action(action, intent.action()) {
                return Err(CodecError::MismatchedBinding);
            }
            Ok(SourceSeriesPayloadV2::Transition(intent))
        }
        registry::SourceSeriesAction::EmitFailureHandoff => Ok(
            SourceSeriesPayloadV2::EmitFailureHandoff(EmitFailureHandoffIntentV2::decode(input)?),
        ),
        registry::SourceSeriesAction::ReopenGeneration => Ok(
            SourceSeriesPayloadV2::ReopenGeneration(ReopenGenerationIntentV2::decode(input)?),
        ),
        registry::SourceSeriesAction::CloseGeneration => Ok(
            SourceSeriesPayloadV2::CloseGeneration(CloseGenerationIntentV2::decode(input)?),
        ),
    }
}

/// Closed physical-account vocabulary for the SourceSeries 77/v2 seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAccountRoleV2 {
    /// Sealed typed Artifact carrying the release body.
    SourceReleaseArtifact,
    /// Immutable registered Source release PDA.
    SourceRelease,
    /// Executing reviewed Source adapter Program account.
    AdapterProgram,
    /// Executing adapter's exact upgradeable ProgramData account.
    AdapterProgramData,
    /// Reviewed parser Program account.
    ParserProgram,
    /// Parser's exact upgradeable ProgramData account.
    ParserProgramData,
    /// Immutable parser configuration account.
    ParserConfig,
    /// Immutable canonical SourceSpec account.
    SourceSpec,
    /// Content-addressed work schedule selected by the Source release.
    SourceWorkSchedule,
    /// Immutable initial or repair-generation request.
    GenerationRequest,
    /// Canonical Clock sysvar.
    ClockSysvar,
    /// Mutable external provider feed read by the parser as read-only.
    Feed,
    /// Release-selected Pyth receiver Program account.
    ReceiverProgram,
    /// Release-selected Pyth receiver ProgramData account.
    ReceiverProgramData,
    /// Exact receiver Config account pinned by the release.
    ReceiverConfig,
    /// Existing or newly allocated SourceHead.
    SourceHead,
    /// Durable SourceHead lineage.
    HeadLineage,
    /// Existing or newly allocated OpenRawPage.
    OpenRawPage,
    /// Durable OpenRawPage lineage.
    OpenPageLineage,
    /// Immutable RawPage evidence.
    RawPage,
    /// Product-owned immutable Source occurrence.
    SourceOccurrence,
    /// Exact immutable Window specification artifact.
    WindowSpec,
    /// Existing or newly allocated WindowWork.
    WindowWork,
    /// Durable WindowWork lineage.
    WorkLineage,
    /// Newly allocated immutable WindowSeal evidence.
    WindowSeal,
    /// Exact immutable StatisticKey artifact.
    StatisticKey,
    /// Exact immutable reviewed summary-program artifact.
    SummaryProgram,
    /// Reviewed evaluator Program account.
    EvaluatorProgram,
    /// Evaluator's exact upgradeable ProgramData account.
    EvaluatorProgramData,
    /// Existing, absent, or newly allocated StatisticResult PDA.
    StatisticResult,
    /// Durable StatisticResult lineage.
    ResultLineage,
    /// Existing persisted Source work or terminal receipt.
    SourceWorkReceipt,
    /// Immutable liveness policy account.
    LivenessPolicy,
    /// Mutable Source liveness compartment.
    SourceCompartment,
    /// Permissionless work recipient and transaction submitter.
    Keeper,
    /// External signer which pays only the one-time release-registration rent.
    ReleasePayer,
    /// Program-derived, fully prepaid custody for every lifecycle rent debit/refund.
    SourceFundingCustody,
    /// Stored payer-principal refund destination.
    PrincipalRefund,
    /// Frozen neutral donation and surplus sink.
    NeutralSink,
    /// Immutable Product/failure policy binding.
    FailurePolicy,
    /// Newly allocated immutable Source handoff receipt.
    HandoffReceipt,
    /// Family-selected immutable authority used to recompute a reopened body.
    GenerationAuthority,
    /// Persisted no-reopen or reconstructed reopen-request terminal policy.
    SourceTerminalPolicy,
    /// Generic mutable Source target selected by the payload family.
    GenerationTarget,
    /// Generic durable lineage selected by the payload family.
    GenerationLineage,
    /// Canonical System Program.
    SystemProgram,
    /// Canonical Rent sysvar.
    RentSysvar,
}

/// One required account at its exact instruction index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountMetaV2 {
    /// Semantic role at this index.
    pub role: SourceAccountRoleV2,
    /// Exact effective writable privilege.
    pub writable: bool,
    /// Exact effective signer privilege.
    pub signer: bool,
}

/// Runtime-observed metadata checked against one role-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedSourceAccountMetaV2 {
    /// Runtime account key.
    pub key: [u8; HASH_BYTES],
    /// Effective writable privilege observed by the instruction.
    pub writable: bool,
    /// Effective signer privilege observed by the instruction.
    pub signer: bool,
}

/// One explicitly permitted same-key pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountAliasV2 {
    /// First logical role.
    pub left: SourceAccountRoleV2,
    /// Second logical role.
    pub right: SourceAccountRoleV2,
}

/// Exact ordered role contract, represented as one mandatory common prefix and
/// one action-specific suffix. Concatenation is the instruction account order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountContractV2 {
    prefix: &'static [SourceAccountMetaV2],
    suffix: &'static [SourceAccountMetaV2],
    aliases: &'static [SourceAccountAliasV2],
}

impl SourceAccountContractV2 {
    /// Exact total account count.
    pub const fn len(self) -> usize {
        self.prefix.len() + self.suffix.len()
    }

    /// Whether the exact table is empty.
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Required metadata at one exact instruction index.
    pub fn meta(self, index: usize) -> Option<SourceAccountMetaV2> {
        if index < self.prefix.len() {
            self.prefix.get(index).copied()
        } else {
            self.suffix.get(index - self.prefix.len()).copied()
        }
    }

    /// Complete closed alias exception list.
    pub const fn aliases(self) -> &'static [SourceAccountAliasV2] {
        self.aliases
    }
}

const fn meta(role: SourceAccountRoleV2, writable: bool, signer: bool) -> SourceAccountMetaV2 {
    SourceAccountMetaV2 {
        role,
        writable,
        signer,
    }
}

/// Mandatory immutable release/deployment/config prefix for actions 2 through 12.
pub const AUTHENTICATED_SOURCE_ROUTE_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceRelease, false, false),
    meta(SourceAccountRoleV2::AdapterProgram, false, false),
    meta(SourceAccountRoleV2::AdapterProgramData, false, false),
    meta(SourceAccountRoleV2::ParserProgram, false, false),
    meta(SourceAccountRoleV2::ParserProgramData, false, false),
    meta(SourceAccountRoleV2::ParserConfig, false, false),
    meta(SourceAccountRoleV2::SourceSpec, false, false),
    meta(SourceAccountRoleV2::SourceWorkSchedule, false, false),
];

const REGISTER_RELEASE_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceReleaseArtifact, false, false),
    meta(SourceAccountRoleV2::SourceRelease, true, false),
    meta(SourceAccountRoleV2::ReleasePayer, true, true),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const INITIALIZE_HEAD_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::GenerationRequest, false, false),
    meta(SourceAccountRoleV2::SourceHead, true, false),
    meta(SourceAccountRoleV2::HeadLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const OPEN_RAW_PAGE_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceHead, false, false),
    meta(SourceAccountRoleV2::HeadLineage, false, false),
    meta(SourceAccountRoleV2::OpenRawPage, true, false),
    meta(SourceAccountRoleV2::OpenPageLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const INGEST_BOUNDARY_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::ClockSysvar, false, false),
    meta(SourceAccountRoleV2::Feed, false, false),
    meta(SourceAccountRoleV2::ReceiverProgram, false, false),
    meta(SourceAccountRoleV2::ReceiverProgramData, false, false),
    meta(SourceAccountRoleV2::ReceiverConfig, false, false),
    meta(SourceAccountRoleV2::SourceHead, false, false),
    meta(SourceAccountRoleV2::HeadLineage, false, false),
    meta(SourceAccountRoleV2::OpenRawPage, true, false),
    meta(SourceAccountRoleV2::OpenPageLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const SEAL_RAW_PAGE_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceHead, true, false),
    meta(SourceAccountRoleV2::HeadLineage, true, false),
    meta(SourceAccountRoleV2::OpenRawPage, true, false),
    meta(SourceAccountRoleV2::OpenPageLineage, true, false),
    meta(SourceAccountRoleV2::RawPage, true, false),
    meta(SourceAccountRoleV2::PrincipalRefund, true, false),
    meta(SourceAccountRoleV2::NeutralSink, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const INITIALIZE_WINDOW_WORK_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceOccurrence, false, false),
    meta(SourceAccountRoleV2::WindowSpec, false, false),
    meta(SourceAccountRoleV2::WindowWork, true, false),
    meta(SourceAccountRoleV2::WorkLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const FOLD_WINDOW_PAGE_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceOccurrence, false, false),
    meta(SourceAccountRoleV2::WindowSpec, false, false),
    meta(SourceAccountRoleV2::WindowWork, true, false),
    meta(SourceAccountRoleV2::WorkLineage, true, false),
    meta(SourceAccountRoleV2::RawPage, false, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const SEAL_WINDOW_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::ClockSysvar, false, false),
    meta(SourceAccountRoleV2::SourceOccurrence, false, false),
    meta(SourceAccountRoleV2::WindowSpec, false, false),
    meta(SourceAccountRoleV2::WindowWork, true, false),
    meta(SourceAccountRoleV2::WorkLineage, true, false),
    meta(SourceAccountRoleV2::RawPage, false, false),
    meta(SourceAccountRoleV2::WindowSeal, true, false),
    meta(SourceAccountRoleV2::PrincipalRefund, true, false),
    meta(SourceAccountRoleV2::NeutralSink, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const EVALUATE_STATISTIC_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::ClockSysvar, false, false),
    meta(SourceAccountRoleV2::SourceOccurrence, false, false),
    meta(SourceAccountRoleV2::WindowSpec, false, false),
    meta(SourceAccountRoleV2::StatisticKey, false, false),
    meta(SourceAccountRoleV2::SummaryProgram, false, false),
    meta(SourceAccountRoleV2::WindowSeal, false, false),
    meta(SourceAccountRoleV2::EvaluatorProgram, false, false),
    meta(SourceAccountRoleV2::EvaluatorProgramData, false, false),
    meta(SourceAccountRoleV2::StatisticResult, true, false),
    meta(SourceAccountRoleV2::ResultLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const EMIT_FAILURE_HANDOFF_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::ClockSysvar, false, false),
    meta(SourceAccountRoleV2::SourceOccurrence, false, false),
    meta(SourceAccountRoleV2::WindowSpec, false, false),
    meta(SourceAccountRoleV2::StatisticKey, false, false),
    meta(SourceAccountRoleV2::WindowSeal, false, false),
    meta(SourceAccountRoleV2::StatisticResult, false, false),
    meta(SourceAccountRoleV2::ResultLineage, false, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::FailurePolicy, false, false),
    meta(SourceAccountRoleV2::HandoffReceipt, true, false),
    meta(SourceAccountRoleV2::LivenessPolicy, false, false),
    meta(SourceAccountRoleV2::SourceCompartment, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const REOPEN_GENERATION_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::GenerationAuthority, false, false),
    meta(SourceAccountRoleV2::GenerationTarget, true, false),
    meta(SourceAccountRoleV2::GenerationLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, true, false),
    meta(SourceAccountRoleV2::Keeper, true, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::SystemProgram, false, false),
    meta(SourceAccountRoleV2::RentSysvar, false, false),
];

const CLOSE_GENERATION_METAS_V2: &[SourceAccountMetaV2] = &[
    meta(SourceAccountRoleV2::SourceTerminalPolicy, false, false),
    meta(SourceAccountRoleV2::GenerationTarget, true, false),
    meta(SourceAccountRoleV2::GenerationLineage, true, false),
    meta(SourceAccountRoleV2::SourceWorkReceipt, false, false),
    meta(SourceAccountRoleV2::Keeper, false, true),
    meta(SourceAccountRoleV2::SourceFundingCustody, true, false),
    meta(SourceAccountRoleV2::PrincipalRefund, true, false),
    meta(SourceAccountRoleV2::NeutralSink, true, false),
];

const NO_ALIASES_V2: &[SourceAccountAliasV2] = &[];
const CUSTODY_REFUND_ALIAS_V2: &[SourceAccountAliasV2] = &[SourceAccountAliasV2 {
    left: SourceAccountRoleV2::SourceFundingCustody,
    right: SourceAccountRoleV2::PrincipalRefund,
}];

/// Return the exact ordered account-role and alias contract for one action.
pub const fn account_contract_v2(action: registry::SourceSeriesAction) -> SourceAccountContractV2 {
    let prefix: &'static [SourceAccountMetaV2] = match action {
        registry::SourceSeriesAction::RegisterRelease => &[],
        _ => AUTHENTICATED_SOURCE_ROUTE_METAS_V2,
    };
    let (suffix, aliases) = match action {
        registry::SourceSeriesAction::RegisterRelease => (REGISTER_RELEASE_METAS_V2, NO_ALIASES_V2),
        registry::SourceSeriesAction::InitializeHead => {
            (INITIALIZE_HEAD_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::OpenRawPage => {
            (OPEN_RAW_PAGE_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::IngestBoundaryBatch => {
            (INGEST_BOUNDARY_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::SealRawPage => {
            (SEAL_RAW_PAGE_METAS_V2, CUSTODY_REFUND_ALIAS_V2)
        }
        registry::SourceSeriesAction::InitializeWindowWork => {
            (INITIALIZE_WINDOW_WORK_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::FoldWindowPages => {
            (FOLD_WINDOW_PAGE_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::SealWindow => {
            (SEAL_WINDOW_METAS_V2, CUSTODY_REFUND_ALIAS_V2)
        }
        registry::SourceSeriesAction::EvaluateStatistic => {
            (EVALUATE_STATISTIC_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::EmitFailureHandoff => {
            (EMIT_FAILURE_HANDOFF_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::ReopenGeneration => {
            (REOPEN_GENERATION_METAS_V2, NO_ALIASES_V2)
        }
        registry::SourceSeriesAction::CloseGeneration => {
            (CLOSE_GENERATION_METAS_V2, CUSTODY_REFUND_ALIAS_V2)
        }
    };
    SourceAccountContractV2 {
        prefix,
        suffix,
        aliases,
    }
}

fn alias_allowed(
    contract: SourceAccountContractV2,
    left: SourceAccountRoleV2,
    right: SourceAccountRoleV2,
) -> bool {
    contract.aliases.iter().any(|alias| {
        (alias.left == left && alias.right == right) || (alias.left == right && alias.right == left)
    })
}

/// Validate exact count, order-derived privileges, live keys, and the closed
/// alias exception list. Privileges are compared after unioning every allowed
/// same-key role, matching Solana's effective privilege behavior.
pub fn validate_account_metas_v2(
    action: registry::SourceSeriesAction,
    observed: &[ObservedSourceAccountMetaV2],
) -> Result<()> {
    let contract = account_contract_v2(action);
    if observed.len() < contract.len() {
        return Err(CodecError::Truncated);
    }
    if observed.len() > contract.len() {
        return Err(CodecError::TrailingBytes);
    }
    for (index, account) in observed.iter().enumerate() {
        require_live(account.key)?;
        let requirement = contract.meta(index).ok_or(CodecError::InvalidCount)?;
        let mut effective_writable = requirement.writable;
        let mut effective_signer = requirement.signer;
        for (other_index, other) in observed.iter().enumerate() {
            if index == other_index || account.key != other.key {
                continue;
            }
            let other_requirement = contract.meta(other_index).ok_or(CodecError::InvalidCount)?;
            if !alias_allowed(contract, requirement.role, other_requirement.role) {
                return Err(CodecError::MismatchedBinding);
            }
            effective_writable |= other_requirement.writable;
            effective_signer |= other_requirement.signer;
        }
        if account.writable != effective_writable || account.signer != effective_signer {
            return Err(CodecError::MismatchedBinding);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec;
    use std::vec::Vec;

    const ACTIONS: [registry::SourceSeriesAction; 12] = [
        registry::SourceSeriesAction::RegisterRelease,
        registry::SourceSeriesAction::InitializeHead,
        registry::SourceSeriesAction::OpenRawPage,
        registry::SourceSeriesAction::IngestBoundaryBatch,
        registry::SourceSeriesAction::SealRawPage,
        registry::SourceSeriesAction::InitializeWindowWork,
        registry::SourceSeriesAction::FoldWindowPages,
        registry::SourceSeriesAction::SealWindow,
        registry::SourceSeriesAction::EvaluateStatistic,
        registry::SourceSeriesAction::EmitFailureHandoff,
        registry::SourceSeriesAction::ReopenGeneration,
        registry::SourceSeriesAction::CloseGeneration,
    ];

    fn transition_tag(action: registry::SourceSeriesAction) -> u16 {
        match action {
            registry::SourceSeriesAction::InitializeHead => 1,
            registry::SourceSeriesAction::OpenRawPage => 2,
            registry::SourceSeriesAction::IngestBoundaryBatch => 3,
            registry::SourceSeriesAction::SealRawPage => 4,
            registry::SourceSeriesAction::InitializeWindowWork => 5,
            registry::SourceSeriesAction::FoldWindowPages => 6,
            registry::SourceSeriesAction::SealWindow => 7,
            registry::SourceSeriesAction::EvaluateStatistic => 8,
            _ => panic!("not a transition action"),
        }
    }

    fn transition_bytes(action: registry::SourceSeriesAction) -> [u8; 160] {
        let mut bytes = [0_u8; 160];
        bytes[..8].copy_from_slice(b"DCSP3INT");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&transition_tag(action).to_le_bytes());
        bytes[12..44].fill(0x21);
        bytes[44..76].fill(0x32);
        bytes[76..108].fill(0x43);
        bytes[120..128].copy_from_slice(&900_u64.to_le_bytes());
        bytes
    }

    fn payload(action: registry::SourceSeriesAction) -> SourceSeriesPayloadV2 {
        match action {
            registry::SourceSeriesAction::RegisterRelease => {
                SourceSeriesPayloadV2::RegisterRelease(RegisterReleaseIntentV2 {
                    source_release_manifest_id: [0x11; 32],
                })
            }
            registry::SourceSeriesAction::InitializeHead
            | registry::SourceSeriesAction::OpenRawPage
            | registry::SourceSeriesAction::IngestBoundaryBatch
            | registry::SourceSeriesAction::SealRawPage
            | registry::SourceSeriesAction::InitializeWindowWork
            | registry::SourceSeriesAction::FoldWindowPages
            | registry::SourceSeriesAction::SealWindow
            | registry::SourceSeriesAction::EvaluateStatistic => SourceSeriesPayloadV2::Transition(
                IntentPreimageV3::decode(&transition_bytes(action)).unwrap(),
            ),
            registry::SourceSeriesAction::EmitFailureHandoff => {
                SourceSeriesPayloadV2::EmitFailureHandoff(EmitFailureHandoffIntentV2 {
                    kind: SourceHandoffKindV2::FailureResult,
                    handoff_id: [0x22; 32],
                    source_work_receipt_id: [0x33; 32],
                    valid_before_slot: 901,
                })
            }
            registry::SourceSeriesAction::ReopenGeneration => {
                SourceSeriesPayloadV2::ReopenGeneration(ReopenGenerationIntentV2 {
                    family: SourceMutableFamilyV2::WindowWork,
                    source_release_manifest_id: [0x44; 32],
                    expected_lineage_state_id: [0x55; 32],
                    semantic_binding_id: [0x66; 32],
                    target_body_id: [0x77; 32],
                    valid_before_slot: 902,
                })
            }
            registry::SourceSeriesAction::CloseGeneration => {
                SourceSeriesPayloadV2::CloseGeneration(CloseGenerationIntentV2 {
                    family: SourceMutableFamilyV2::StatisticResult,
                    source_release_manifest_id: [0x88; 32],
                    expected_lineage_state_id: [0x99; 32],
                    semantic_terminal_receipt_id: [0xaa; 32],
                    valid_before_slot: 903,
                })
            }
        }
    }

    #[test]
    fn all_twelve_payload_widths_and_round_trips_are_frozen() {
        assert_eq!(REGISTER_RELEASE_PAYLOAD_BYTES_V2, 32);
        assert_eq!(SOURCE_TRANSITION_PAYLOAD_BYTES_V2, 160);
        assert_eq!(EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2, 80);
        assert_eq!(REOPEN_GENERATION_PAYLOAD_BYTES_V2, 144);
        assert_eq!(CLOSE_GENERATION_PAYLOAD_BYTES_V2, 112);
        assert_eq!(SOURCE_RELEASE_ARTIFACT_BODY_BYTES_V2, 1_296);
        assert_eq!(SOURCE_RELEASE_ARTIFACT_KIND_V2.byte(), 47);

        for action in ACTIONS {
            let value = payload(action);
            let mut bytes = vec![0_u8; payload_bytes_v2(action)];
            encode_payload_v2(action, value, &mut bytes).unwrap();
            assert_eq!(decode_payload_v2(action, &bytes), Ok(value));
        }
    }

    #[test]
    fn payload_decoders_refuse_truncation_trailing_and_cross_action_substitution() {
        for action in ACTIONS {
            let value = payload(action);
            let mut bytes = vec![0_u8; payload_bytes_v2(action)];
            encode_payload_v2(action, value, &mut bytes).unwrap();
            assert_eq!(
                decode_payload_v2(action, &bytes[..bytes.len() - 1]),
                Err(CodecError::Truncated),
                "{action:?} truncated"
            );
            bytes.push(0);
            assert_eq!(
                decode_payload_v2(action, &bytes),
                Err(CodecError::TrailingBytes),
                "{action:?} trailing"
            );
        }

        let initialize = transition_bytes(registry::SourceSeriesAction::InitializeHead);
        assert_eq!(
            decode_payload_v2(registry::SourceSeriesAction::OpenRawPage, &initialize),
            Err(CodecError::MismatchedBinding)
        );
        let value = payload(registry::SourceSeriesAction::InitializeHead);
        let mut output = [0_u8; 160];
        assert_eq!(
            encode_payload_v2(
                registry::SourceSeriesAction::OpenRawPage,
                value,
                &mut output,
            ),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn lifecycle_payloads_refuse_zero_unknown_and_noncanonical_reserved_bytes() {
        let mut handoff = [0_u8; EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2];
        EmitFailureHandoffIntentV2 {
            kind: SourceHandoffKindV2::FailureAbsence,
            handoff_id: [1; 32],
            source_work_receipt_id: [2; 32],
            valid_before_slot: 3,
        }
        .encode(&mut handoff)
        .unwrap();
        let mut hostile = handoff;
        hostile[1] = 1;
        assert_eq!(
            EmitFailureHandoffIntentV2::decode(&hostile),
            Err(CodecError::NonCanonicalPadding)
        );
        hostile = handoff;
        hostile[0] = 9;
        assert_eq!(
            EmitFailureHandoffIntentV2::decode(&hostile),
            Err(CodecError::InvalidEnum)
        );
        for kind in [
            SourceHandoffKindV2::FailureAbsence,
            SourceHandoffKindV2::FailureResult,
            SourceHandoffKindV2::SuccessfulEvaluation,
        ] {
            hostile = handoff;
            hostile[0] = kind.wire_byte();
            assert_eq!(
                EmitFailureHandoffIntentV2::decode(&hostile).unwrap().kind,
                kind,
            );
        }
        hostile = handoff;
        hostile[8..40].fill(0);
        assert_eq!(
            EmitFailureHandoffIntentV2::decode(&hostile),
            Err(CodecError::ZeroIdentity)
        );

        let mut close = [0_u8; CLOSE_GENERATION_PAYLOAD_BYTES_V2];
        if let SourceSeriesPayloadV2::CloseGeneration(value) =
            payload(registry::SourceSeriesAction::CloseGeneration)
        {
            value.encode(&mut close).unwrap();
        }
        close[1] = 1;
        assert_eq!(
            CloseGenerationIntentV2::decode(&close),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    fn observed(action: registry::SourceSeriesAction) -> Vec<ObservedSourceAccountMetaV2> {
        let contract = account_contract_v2(action);
        let mut accounts = Vec::with_capacity(contract.len());
        for index in 0..contract.len() {
            let required = contract.meta(index).unwrap();
            let mut key = [0_u8; 32];
            let ordinal = u64::try_from(index).unwrap().checked_add(1).unwrap();
            key[..8].copy_from_slice(&ordinal.to_le_bytes());
            accounts.push(ObservedSourceAccountMetaV2 {
                key,
                writable: required.writable,
                signer: required.signer,
            });
        }
        accounts
    }

    fn role_index(contract: SourceAccountContractV2, role: SourceAccountRoleV2) -> usize {
        (0..contract.len())
            .find(|index| contract.meta(*index).unwrap().role == role)
            .unwrap()
    }

    #[test]
    fn every_account_table_is_exact_and_rejects_privilege_widening() {
        for action in ACTIONS {
            let accounts = observed(action);
            assert_eq!(validate_account_metas_v2(action, &accounts), Ok(()));
            assert_eq!(
                validate_account_metas_v2(action, &accounts[..accounts.len() - 1]),
                Err(CodecError::Truncated)
            );
            let mut trailing = accounts.clone();
            trailing.push(ObservedSourceAccountMetaV2 {
                key: [0xfe; 32],
                writable: false,
                signer: false,
            });
            assert_eq!(
                validate_account_metas_v2(action, &trailing),
                Err(CodecError::TrailingBytes)
            );
            let mut widened = accounts;
            widened[0].writable = !widened[0].writable;
            assert_eq!(
                validate_account_metas_v2(action, &widened),
                Err(CodecError::MismatchedBinding)
            );
        }
    }

    #[test]
    fn only_named_aliases_are_admitted_and_use_effective_union_privileges() {
        let action = registry::SourceSeriesAction::SealRawPage;
        let contract = account_contract_v2(action);
        let custody = role_index(contract, SourceAccountRoleV2::SourceFundingCustody);
        let keeper = role_index(contract, SourceAccountRoleV2::Keeper);
        let refund = role_index(contract, SourceAccountRoleV2::PrincipalRefund);
        let mut accounts = observed(action);
        let custody_key = accounts[custody].key;
        accounts[refund].key = custody_key;
        assert_eq!(validate_account_metas_v2(action, &accounts), Ok(()));

        let mut keeper_alias = observed(action);
        keeper_alias[keeper].key = custody_key;
        assert_eq!(
            validate_account_metas_v2(action, &keeper_alias),
            Err(CodecError::MismatchedBinding)
        );

        let release = role_index(contract, SourceAccountRoleV2::SourceRelease);
        let adapter = role_index(contract, SourceAccountRoleV2::AdapterProgram);
        let mut forbidden = observed(action);
        let release_key = forbidden[release].key;
        forbidden[adapter].key = release_key;
        assert_eq!(
            validate_account_metas_v2(action, &forbidden),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn postterminal_reopen_uses_only_prepaid_custody_and_keeper_signature() {
        let contract = account_contract_v2(registry::SourceSeriesAction::ReopenGeneration);
        assert!((0..contract.len()).all(|index| {
            !matches!(
                contract.meta(index).unwrap().role,
                SourceAccountRoleV2::LivenessPolicy | SourceAccountRoleV2::SourceCompartment
            )
        }));
        let custody = contract
            .meta(role_index(contract, SourceAccountRoleV2::SourceFundingCustody))
            .unwrap();
        let keeper = contract
            .meta(role_index(contract, SourceAccountRoleV2::Keeper))
            .unwrap();
        assert!(custody.writable);
        assert!(!custody.signer);
        assert!(keeper.writable);
        assert!(keeper.signer);
    }
}
