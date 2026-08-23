//! Frozen disabled wire/account contract for failure recovery 78/v1.
//!
//! This module owns hostile instruction bytes, main-program account framing,
//! and ordered account-role contracts. It does not authenticate Source facts,
//! Product/Series artifacts, liveness receipts, Clock, owners, PDAs, balances,
//! or signatures. Those are obligations of the SBF adapter. Allocation also
//! grants no capability: all thirteen Recovery actions remain disabled.

use crate::{is_zero, registry, CodecError, Result, HASH_BYTES};

/// Main-program framing before one semantic-owner body.
pub const FAILURE_ACCOUNT_HEADER_BYTES_V1: usize = 4;
/// Exact failure semantic-root body owned by the failure adapter.
pub const FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2: usize = 2_168;
/// Exact framed failure semantic-root account width.
pub const FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1: usize =
    FAILURE_ACCOUNT_HEADER_BYTES_V1 + FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2;
/// Exact immutable liveness-policy body owned by `clutch-liveness`.
pub const FAILURE_LIVENESS_POLICY_BODY_BYTES_V1: usize = 1_132;
/// Exact framed immutable liveness-policy account width.
pub const FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1: usize =
    FAILURE_ACCOUNT_HEADER_BYTES_V1 + FAILURE_LIVENESS_POLICY_BODY_BYTES_V1;
/// Exact mutable Recovery-compartment body owned by `clutch-liveness`.
pub const FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1: usize = 464;
/// Exact framed Recovery-compartment account width.
pub const FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1: usize =
    FAILURE_ACCOUNT_HEADER_BYTES_V1 + FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1;
/// Exact permanent failure replay-tombstone width.
pub const FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1: usize = 256;

/// Fields common to every Recovery action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryCommonV1 {
    /// Immutable failure-policy binding identity.
    pub binding_id: [u8; HASH_BYTES],
    /// Full-width MarketInstanceV2 identity.
    pub market_instance_v2_id: [u8; HASH_BYTES],
    /// Exact Source/failure/liveness generation.
    pub generation: u64,
    /// Exact semantic-root transition nonce expected in prestate.
    pub expected_transition_nonce: u64,
}

impl RecoveryCommonV1 {
    fn validate(&self) -> Result<()> {
        require_live(self.binding_id)?;
        require_live(self.market_instance_v2_id)?;
        if self.generation == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }
}

/// Strict `InitializeFailureRoot` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeFailureRootV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact SeriesPlanV5 identity.
    pub series_plan_v5_id: [u8; HASH_BYTES],
    /// Exact finite Series ordinal.
    pub ordinal: u32,
    /// Exact presently funded Series quote.
    pub series_funding_quote_id: [u8; HASH_BYTES],
    /// Exact Series-owned MarketCore subcomponent-debit receipt.
    pub market_core_funding_receipt_id: [u8; HASH_BYTES],
    /// Root-account rent principal supplied now by its immutable payer.
    pub root_rent_principal_lamports: u64,
    /// Permanent replay-account rent principal supplied in the same debit.
    pub replay_rent_principal_lamports: u64,
}

/// Strict `TriggerSourceFailure` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerSourceFailureV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned failure handoff receipt.
    pub source_failure_handoff_id: [u8; HASH_BYTES],
}

/// Strict `TriggerRelationRefusal` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerRelationRefusalV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful-evaluation handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Exact immutable Failure relation policy selected by Product.
    pub relation_policy_id: [u8; HASH_BYTES],
    /// Exact deterministic relation-execution record.
    pub relation_record_id: [u8; HASH_BYTES],
    /// Exact atomic execution capability consumed by this transition.
    pub relation_execution_id: [u8; HASH_BYTES],
    /// Closed deterministic refusal code selected by the frozen relation.
    pub refusal_code: u32,
}

/// Strict `AdvanceRecoverySchedule` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceRecoveryScheduleV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact attempt index expected in semantic prestate.
    pub expected_attempt_index: u8,
}

/// Strict `AcceptRecoveryWork` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptRecoveryWorkV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful-evaluation handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Sole keeper/reward recipient.
    pub reward_recipient: [u8; HASH_BYTES],
    /// Exact scheduled liveness ceiling removed from remaining work.
    pub scheduled_ceiling_lamports: u64,
}

/// Strict `ResolveCallerFunded` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveCallerFundedV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful-evaluation handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Exact immutable Failure relation policy selected by Product.
    pub relation_policy_id: [u8; HASH_BYTES],
    /// Exact deterministic accepted relation-execution record.
    pub relation_record_id: [u8; HASH_BYTES],
    /// Exact atomic execution capability consumed by this transition.
    pub relation_execution_id: [u8; HASH_BYTES],
}

/// Strict `ResolvePaidRecovery` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvePaidRecoveryV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful-evaluation handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Exact immutable Failure relation policy selected by Product.
    pub relation_policy_id: [u8; HASH_BYTES],
    /// Exact deterministic accepted relation-execution record.
    pub relation_record_id: [u8; HASH_BYTES],
    /// Exact atomic execution capability consumed by this transition.
    pub relation_execution_id: [u8; HASH_BYTES],
    /// Sole keeper/reward recipient.
    pub reward_recipient: [u8; HASH_BYTES],
    /// Exact scheduled liveness ceiling removed from remaining work.
    pub scheduled_ceiling_lamports: u64,
}

/// Strict `CloseRecoveryFunding` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseRecoveryFundingV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Failure-owned Recovery terminal receipt mapped into liveness.
    pub recovery_terminal_receipt_id: [u8; HASH_BYTES],
}

/// Strict `CloseFailureRoot` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseFailureRootV1 {
    /// Shared action identity and replay fields.
    pub common: RecoveryCommonV1,
    /// Full failure terminal join identity.
    pub failure_terminal_join_id: [u8; HASH_BYTES],
    /// Separately authenticated retirement root.
    pub retirement_root_id: [u8; HASH_BYTES],
    /// Predictable replay tombstone allocated before terminal work and
    /// preserved after close.
    pub replay_tombstone_id: [u8; HASH_BYTES],
    /// Final Source release/lineage receipt.
    pub source_release_receipt_id: [u8; HASH_BYTES],
}

/// Strict `BeginIntervalConsensus` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginIntervalConsensusV1 {
    /// Shared Failure generation and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful interval handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Product-selected bounded interval work profile.
    pub interval_profile_id: [u8; HASH_BYTES],
    /// Typed present-funding receipt for ab/ac rent and bounded work.
    pub funding_admission_receipt_id: [u8; HASH_BYTES],
    /// Canonical initial Product structural-work identity.
    pub initial_work_id: [u8; HASH_BYTES],
    /// Exact refundable ab work-account rent principal.
    pub work_rent_principal_lamports: u64,
    /// Exact nonrefundable permanent ac replay rent principal.
    pub replay_rent_principal_lamports: u64,
}

/// Strict `AdvanceIntervalConsensus` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceIntervalConsensusV1 {
    /// Shared Failure generation and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact ab transition nonce expected before this chunk.
    pub expected_work_transition_nonce: u64,
    /// Bounded number of integer coordinates requested in this call.
    pub requested_coordinates: u16,
    /// Sole keeper paid by liveness for this exact chunk.
    pub reward_recipient: [u8; HASH_BYTES],
    /// Exact scheduled liveness ceiling removed from Recovery capital.
    pub scheduled_ceiling_lamports: u64,
    /// Complete Product work preimage identity.
    pub before_work_id: [u8; HASH_BYTES],
    /// Complete Product work postimage identity.
    pub after_work_id: [u8; HASH_BYTES],
    /// Permanent replay postimage identity for this transition.
    pub replay_receipt_id: [u8; HASH_BYTES],
}

/// Strict `ResolveIntervalConsensus` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveIntervalConsensusV1 {
    /// Shared Failure generation and replay fields.
    pub common: RecoveryCommonV1,
    /// Exact Source-owned successful interval handoff.
    pub source_success_handoff_id: [u8; HASH_BYTES],
    /// Complete terminal Product work identity.
    pub completed_work_id: [u8; HASH_BYTES],
    /// Exhaustive Product interval certificate identity.
    pub certificate_id: [u8; HASH_BYTES],
    /// Permanent replay authentication consumed by capability restoration.
    pub replay_receipt_id: [u8; HASH_BYTES],
}

/// Strict `CloseIntervalConsensusWork` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseIntervalConsensusWorkV1 {
    /// Shared Failure generation and replay fields.
    pub common: RecoveryCommonV1,
    /// Exhaustive Product interval certificate identity.
    pub certificate_id: [u8; HASH_BYTES],
    /// Failure resolution receipt which consumed that certificate.
    pub resolution_receipt_id: [u8; HASH_BYTES],
    /// Permanent replay postimage retained after ab close.
    pub replay_receipt_id: [u8; HASH_BYTES],
    /// Exact authenticated ab close authorization identity.
    pub work_close_authorization_id: [u8; HASH_BYTES],
}

/// One exact decoded Recovery action payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureRecoveryPayloadV1 {
    /// Initialize one semantic root.
    InitializeFailureRoot(InitializeFailureRootV1),
    /// Enter degraded recovery from a Source-owned failure.
    TriggerSourceFailure(TriggerSourceFailureV1),
    /// Enter degraded recovery from a relation refusal.
    TriggerRelationRefusal(TriggerRelationRefusalV1),
    /// Expire/advance the immutable repair schedule.
    AdvanceRecoverySchedule(AdvanceRecoveryScheduleV1),
    /// Accept one repair unit and pay only through liveness custody.
    AcceptRecoveryWork(AcceptRecoveryWorkV1),
    /// Resolve from evidence without a keeper payment.
    ResolveCallerFunded(ResolveCallerFundedV1),
    /// Resolve while paying one final repair unit through liveness.
    ResolvePaidRecovery(ResolvePaidRecoveryV1),
    /// Close only the liveness Recovery compartment.
    CloseRecoveryFunding(CloseRecoveryFundingV1),
    /// Close only the resolved semantic root.
    CloseFailureRoot(CloseFailureRootV1),
    /// Begin one dedicated exhaustive interval-consensus lifecycle.
    BeginIntervalConsensus(BeginIntervalConsensusV1),
    /// Evaluate one bounded integer-coordinate chunk.
    AdvanceIntervalConsensus(AdvanceIntervalConsensusV1),
    /// Restore a verified Product payout and resolve Failure.
    ResolveIntervalConsensus(ResolveIntervalConsensusV1),
    /// Close deletable work while preserving permanent replay.
    CloseIntervalConsensusWork(CloseIntervalConsensusWorkV1),
}

/// Exact payload widths, indexed by allocated Recovery action.
pub const INITIALIZE_FAILURE_ROOT_PAYLOAD_BYTES_V1: usize = 200;
/// Exact Source-trigger payload width.
pub const TRIGGER_SOURCE_FAILURE_PAYLOAD_BYTES_V1: usize = 112;
/// Exact relation-refusal trigger payload width.
pub const TRIGGER_RELATION_REFUSAL_PAYLOAD_BYTES_V1: usize = 216;
/// Exact schedule-advance payload width.
pub const ADVANCE_RECOVERY_SCHEDULE_PAYLOAD_BYTES_V1: usize = 88;
/// Exact accepted-work payload width.
pub const ACCEPT_RECOVERY_WORK_PAYLOAD_BYTES_V1: usize = 152;
/// Exact caller-funded resolution payload width.
pub const RESOLVE_CALLER_FUNDED_PAYLOAD_BYTES_V1: usize = 208;
/// Exact paid-resolution payload width.
pub const RESOLVE_PAID_RECOVERY_PAYLOAD_BYTES_V1: usize = 248;
/// Exact Recovery-close payload width.
pub const CLOSE_RECOVERY_FUNDING_PAYLOAD_BYTES_V1: usize = 112;
/// Exact semantic-root-close payload width.
pub const CLOSE_FAILURE_ROOT_PAYLOAD_BYTES_V1: usize = 208;
/// Exact interval-consensus Begin payload width.
pub const BEGIN_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1: usize = 224;
/// Exact bounded interval-consensus Advance payload width.
pub const ADVANCE_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1: usize = 232;
/// Exact interval-consensus Resolve payload width.
pub const RESOLVE_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1: usize = 208;
/// Exact interval-consensus work-close payload width.
pub const CLOSE_INTERVAL_CONSENSUS_WORK_PAYLOAD_BYTES_V1: usize = 208;
/// Largest payload admitted by Recovery78/v1.
pub const MAX_FAILURE_RECOVERY_PAYLOAD_BYTES_V1: usize = RESOLVE_PAID_RECOVERY_PAYLOAD_BYTES_V1;

/// Borrowed, strictly framed semantic-owner account body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAccountBodyV1<'a> {
    /// Canonical PDA bump stored by the main-program frame.
    pub stored_bump: u8,
    /// Complete semantic-owner bytes with no copied fields.
    pub body: &'a [u8],
}

/// Decode one exact framed failure/liveness account.
pub fn decode_failure_account_body_v1(
    input: &[u8],
    expected_tag: u8,
    expected_version: u8,
    expected_body_bytes: usize,
) -> Result<FailureAccountBodyV1<'_>> {
    require_exact(input, FAILURE_ACCOUNT_HEADER_BYTES_V1 + expected_body_bytes)?;
    if input[0] != expected_tag {
        return Err(CodecError::WrongTag);
    }
    if input[1] != expected_version {
        return Err(CodecError::WrongVersion);
    }
    if input[3] != 0 {
        return Err(CodecError::NonCanonicalPadding);
    }
    Ok(FailureAccountBodyV1 {
        stored_bump: input[2],
        body: &input[FAILURE_ACCOUNT_HEADER_BYTES_V1..],
    })
}

/// Write the exact main-program frame before a separately encoded owner body.
pub fn encode_failure_account_header_v1(
    output: &mut [u8],
    tag: u8,
    version: u8,
    stored_bump: u8,
    body_bytes: usize,
) -> Result<()> {
    require_exact(output, FAILURE_ACCOUNT_HEADER_BYTES_V1 + body_bytes)?;
    output[..FAILURE_ACCOUNT_HEADER_BYTES_V1].copy_from_slice(&[tag, version, stored_bump, 0]);
    Ok(())
}

/// Lifecycle of the permanent generation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureReplayTombstonePhaseV1 {
    /// Rent and immutable generation facts are present before terminal work.
    Pending = 1,
    /// Exact retirement, Source release, and full terminal join are sealed.
    Terminal = 2,
}

impl FailureReplayTombstonePhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Pending),
            2 => Ok(Self::Terminal),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Permanent funded generation record, terminalized exactly once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureReplayTombstoneV1 {
    /// Canonical account PDA bump.
    pub stored_bump: u8,
    /// Pending before closure; terminal after all external joins exist.
    pub phase: FailureReplayTombstonePhaseV1,
    /// Exact nonrefundable present rent used to create the permanent record.
    pub permanent_rent_lamports: u64,
    /// Lamports already present before the admitted rent debit. They remain a
    /// permanent donation and cannot be reclassified as rent principal.
    pub prior_donation_lamports: u64,
    /// Exact signer which supplied the permanent-rent principal.
    pub permanent_rent_funder: [u8; HASH_BYTES],
    /// Typed terminal-funding admission receipt whose semantic owner must
    /// exclude Recovery work, Hoard, collateral, and future-fee principal.
    pub funding_admission_receipt_id: [u8; HASH_BYTES],
    /// Immutable failure-policy binding.
    pub binding_id: [u8; HASH_BYTES],
    /// Full-width MarketInstanceV2 identity.
    pub market_instance_v2_id: [u8; HASH_BYTES],
    /// Exact terminal generation.
    pub generation: u64,
    /// Full failure terminal join identity.
    pub failure_terminal_join_id: [u8; HASH_BYTES],
    /// Separately authenticated retirement root.
    pub retirement_root_id: [u8; HASH_BYTES],
    /// Final Source release/lineage receipt.
    pub source_release_receipt_id: [u8; HASH_BYTES],
}

impl FailureReplayTombstoneV1 {
    /// Validate present permanent funding and phase-canonical terminal fields.
    pub fn validate(&self) -> Result<()> {
        for id in [
            self.permanent_rent_funder,
            self.funding_admission_receipt_id,
            self.binding_id,
            self.market_instance_v2_id,
        ] {
            require_live(id)?;
        }
        if self.generation == 0 || self.permanent_rent_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        let terminal = [
            self.failure_terminal_join_id,
            self.retirement_root_id,
            self.source_release_receipt_id,
        ];
        match self.phase {
            FailureReplayTombstonePhaseV1::Pending => {
                if terminal.iter().any(|id| !is_zero(id)) {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
            FailureReplayTombstonePhaseV1::Terminal => {
                for id in terminal {
                    require_live(id)?;
                }
            }
        }
        Ok(())
    }

    /// Seal exact terminal joins without changing funding or generation facts.
    pub fn terminalized(
        self,
        failure_terminal_join_id: [u8; HASH_BYTES],
        retirement_root_id: [u8; HASH_BYTES],
        source_release_receipt_id: [u8; HASH_BYTES],
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != FailureReplayTombstonePhaseV1::Pending {
            return Err(CodecError::InvalidEnum);
        }
        let next = Self {
            phase: FailureReplayTombstonePhaseV1::Terminal,
            failure_terminal_join_id,
            retirement_root_id,
            source_release_receipt_id,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Encode the exact permanent tombstone body.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        require_exact(output, FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1)?;
        output.fill(0);
        output[0] = registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG;
        output[1] = registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION;
        output[2] = self.stored_bump;
        output[3] = self.phase as u8;
        output[4..12].copy_from_slice(&self.permanent_rent_lamports.to_le_bytes());
        output[12..20].copy_from_slice(&self.prior_donation_lamports.to_le_bytes());
        output[20..52].copy_from_slice(&self.permanent_rent_funder);
        output[52..84].copy_from_slice(&self.funding_admission_receipt_id);
        output[84..116].copy_from_slice(&self.binding_id);
        output[116..148].copy_from_slice(&self.market_instance_v2_id);
        output[148..156].copy_from_slice(&self.generation.to_le_bytes());
        output[156..188].copy_from_slice(&self.failure_terminal_join_id);
        output[188..220].copy_from_slice(&self.retirement_root_id);
        output[220..252].copy_from_slice(&self.source_release_receipt_id);
        Ok(())
    }

    /// Decode one exact hostile permanent tombstone body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[252..])?;
        let value = Self {
            stored_bump: input[2],
            phase: FailureReplayTombstonePhaseV1::decode(input[3])?,
            permanent_rent_lamports: u64_at(input, 4),
            prior_donation_lamports: u64_at(input, 12),
            permanent_rent_funder: id_at(input, 20),
            funding_admission_receipt_id: id_at(input, 52),
            binding_id: id_at(input, 84),
            market_instance_v2_id: id_at(input, 116),
            generation: u64_at(input, 148),
            failure_terminal_join_id: id_at(input, 156),
            retirement_root_id: id_at(input, 188),
            source_release_receipt_id: id_at(input, 220),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Ordered account role in one future-enabled Recovery instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAccountMetaV1 {
    /// Semantic role; its array index is the instruction account index.
    pub role: RecoveryAccountRoleV1,
    /// Whether the account must be writable.
    pub writable: bool,
    /// Whether the account must sign.
    pub signer: bool,
}

/// Closed account-role vocabulary for the Recovery SBF seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAccountRoleV1 {
    MarketCoreLamportVault,
    RootRentPayer,
    FailureRoot,
    LivenessPolicy,
    RecoveryCompartment,
    SeriesRegistry,
    SeriesFunding,
    RegistryProgram,
    RegistryProgramData,
    RegistryRelease,
    CapabilityProfile,
    SeriesArtifact,
    FundingTermsArtifact,
    ProductTemplateArtifact,
    ClaimBasisArtifact,
    RecoveryPolicyArtifact,
    PricePolicyArtifact,
    GenesisArtifact,
    FundingQuoteArtifact,
    AttachmentArtifact,
    SourceRelease,
    SourceAdapterProgram,
    SourceAdapterProgramData,
    ParserProgram,
    ParserProgramData,
    ParserConfig,
    SourceSpec,
    SourceOccurrence,
    SourceWindow,
    StatisticKey,
    SummaryProgramArtifact,
    SourceResult,
    SourceWorkReceipt,
    Keeper,
    RecoveryPayer,
    NeutralSink,
    RetirementRoot,
    ReplayTombstone,
    ProductOccurrenceRoot,
    IntervalConsensusWork,
    IntervalConsensusReplay,
    ResolutionV5,
    HoardV2,
    ClaimLedgerV3,
    ClockSysvar,
    RentSysvar,
    SystemProgram,
}

const fn meta(role: RecoveryAccountRoleV1, writable: bool, signer: bool) -> RecoveryAccountMetaV1 {
    RecoveryAccountMetaV1 {
        role,
        writable,
        signer,
    }
}

/// Exact ordered account contract for `InitializeFailureRoot`.
pub const INITIALIZE_FAILURE_ROOT_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::MarketCoreLamportVault, true, false),
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, false, false),
    meta(RecoveryAccountRoleV1::NeutralSink, false, false),
    meta(RecoveryAccountRoleV1::SeriesRegistry, false, false),
    meta(RecoveryAccountRoleV1::SeriesFunding, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgram, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgramData, false, false),
    meta(RecoveryAccountRoleV1::RegistryRelease, false, false),
    meta(RecoveryAccountRoleV1::CapabilityProfile, false, false),
    meta(RecoveryAccountRoleV1::SeriesArtifact, false, false),
    meta(RecoveryAccountRoleV1::FundingTermsArtifact, false, false),
    meta(RecoveryAccountRoleV1::ProductTemplateArtifact, false, false),
    meta(RecoveryAccountRoleV1::ClaimBasisArtifact, false, false),
    meta(RecoveryAccountRoleV1::RecoveryPolicyArtifact, false, false),
    meta(RecoveryAccountRoleV1::PricePolicyArtifact, false, false),
    meta(RecoveryAccountRoleV1::GenesisArtifact, false, false),
    meta(RecoveryAccountRoleV1::FundingQuoteArtifact, false, false),
    meta(RecoveryAccountRoleV1::AttachmentArtifact, false, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceAdapterProgram, false, false),
    meta(
        RecoveryAccountRoleV1::SourceAdapterProgramData,
        false,
        false,
    ),
    meta(RecoveryAccountRoleV1::ParserProgram, false, false),
    meta(RecoveryAccountRoleV1::ParserProgramData, false, false),
    meta(RecoveryAccountRoleV1::ParserConfig, false, false),
    meta(RecoveryAccountRoleV1::SourceSpec, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceWindow, false, false),
    meta(RecoveryAccountRoleV1::StatisticKey, false, false),
    meta(RecoveryAccountRoleV1::SummaryProgramArtifact, false, false),
    meta(RecoveryAccountRoleV1::ReplayTombstone, true, false),
    meta(RecoveryAccountRoleV1::RentSysvar, false, false),
    meta(RecoveryAccountRoleV1::SystemProgram, false, false),
];

const SOURCE_FAILURE_METAS: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
];

/// Exact ordered account contract for a Source failure trigger.
pub const TRIGGER_SOURCE_FAILURE_METAS_V1: &[RecoveryAccountMetaV1] = SOURCE_FAILURE_METAS;
/// Exact ordered account contract for a relation-refusal trigger.
pub const TRIGGER_RELATION_REFUSAL_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::SeriesRegistry, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgram, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgramData, false, false),
    meta(RecoveryAccountRoleV1::RegistryRelease, false, false),
    meta(RecoveryAccountRoleV1::CapabilityProfile, false, false),
    meta(RecoveryAccountRoleV1::SeriesArtifact, false, false),
    meta(RecoveryAccountRoleV1::ProductTemplateArtifact, false, false),
    meta(RecoveryAccountRoleV1::ClaimBasisArtifact, false, false),
    meta(RecoveryAccountRoleV1::PricePolicyArtifact, false, false),
    meta(RecoveryAccountRoleV1::GenesisArtifact, false, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
];
/// Exact ordered account contract for a schedule advance.
pub const ADVANCE_RECOVERY_SCHEDULE_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
];
/// Exact ordered account contract for accepted paid work.
pub const ACCEPT_RECOVERY_WORK_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, true, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::Keeper, true, false),
    meta(RecoveryAccountRoleV1::RecoveryPayer, true, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
];
/// Exact ordered account contract for caller-funded resolution.
pub const RESOLVE_CALLER_FUNDED_METAS_V1: &[RecoveryAccountMetaV1] = &[
    TRIGGER_RELATION_REFUSAL_METAS_V1[0],
    TRIGGER_RELATION_REFUSAL_METAS_V1[1],
    TRIGGER_RELATION_REFUSAL_METAS_V1[2],
    TRIGGER_RELATION_REFUSAL_METAS_V1[3],
    TRIGGER_RELATION_REFUSAL_METAS_V1[4],
    TRIGGER_RELATION_REFUSAL_METAS_V1[5],
    TRIGGER_RELATION_REFUSAL_METAS_V1[6],
    TRIGGER_RELATION_REFUSAL_METAS_V1[7],
    TRIGGER_RELATION_REFUSAL_METAS_V1[8],
    TRIGGER_RELATION_REFUSAL_METAS_V1[9],
    TRIGGER_RELATION_REFUSAL_METAS_V1[10],
    TRIGGER_RELATION_REFUSAL_METAS_V1[11],
    TRIGGER_RELATION_REFUSAL_METAS_V1[12],
    TRIGGER_RELATION_REFUSAL_METAS_V1[13],
    TRIGGER_RELATION_REFUSAL_METAS_V1[14],
    TRIGGER_RELATION_REFUSAL_METAS_V1[15],
];
/// Exact ordered account contract for paid resolution.
pub const RESOLVE_PAID_RECOVERY_METAS_V1: &[RecoveryAccountMetaV1] = &[
    ACCEPT_RECOVERY_WORK_METAS_V1[0],
    ACCEPT_RECOVERY_WORK_METAS_V1[1],
    ACCEPT_RECOVERY_WORK_METAS_V1[2],
    TRIGGER_RELATION_REFUSAL_METAS_V1[1],
    TRIGGER_RELATION_REFUSAL_METAS_V1[2],
    TRIGGER_RELATION_REFUSAL_METAS_V1[3],
    TRIGGER_RELATION_REFUSAL_METAS_V1[4],
    TRIGGER_RELATION_REFUSAL_METAS_V1[5],
    TRIGGER_RELATION_REFUSAL_METAS_V1[6],
    TRIGGER_RELATION_REFUSAL_METAS_V1[7],
    TRIGGER_RELATION_REFUSAL_METAS_V1[8],
    TRIGGER_RELATION_REFUSAL_METAS_V1[9],
    TRIGGER_RELATION_REFUSAL_METAS_V1[10],
    ACCEPT_RECOVERY_WORK_METAS_V1[3],
    ACCEPT_RECOVERY_WORK_METAS_V1[4],
    ACCEPT_RECOVERY_WORK_METAS_V1[5],
    ACCEPT_RECOVERY_WORK_METAS_V1[6],
    ACCEPT_RECOVERY_WORK_METAS_V1[7],
    ACCEPT_RECOVERY_WORK_METAS_V1[8],
    ACCEPT_RECOVERY_WORK_METAS_V1[9],
];
/// Exact ordered account contract for closing only Recovery funding.
pub const CLOSE_RECOVERY_FUNDING_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, false, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, true, false),
    meta(RecoveryAccountRoleV1::RecoveryPayer, true, false),
    meta(RecoveryAccountRoleV1::NeutralSink, true, false),
];
/// Exact ordered account contract for closing only the resolved root.
pub const CLOSE_FAILURE_ROOT_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::RootRentPayer, true, false),
    meta(RecoveryAccountRoleV1::NeutralSink, true, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, true, false),
    meta(RecoveryAccountRoleV1::RetirementRoot, false, false),
    meta(RecoveryAccountRoleV1::ReplayTombstone, true, false),
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
];

const INTERVAL_PRODUCT_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::SeriesRegistry, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgram, false, false),
    meta(RecoveryAccountRoleV1::RegistryProgramData, false, false),
    meta(RecoveryAccountRoleV1::RegistryRelease, false, false),
    meta(RecoveryAccountRoleV1::CapabilityProfile, false, false),
    meta(RecoveryAccountRoleV1::SeriesArtifact, false, false),
    meta(RecoveryAccountRoleV1::ProductTemplateArtifact, false, false),
    meta(RecoveryAccountRoleV1::ClaimBasisArtifact, false, false),
    meta(RecoveryAccountRoleV1::PricePolicyArtifact, false, false),
    meta(RecoveryAccountRoleV1::GenesisArtifact, false, false),
];

/// Exact ordered contract for creating ab work and permanent ac replay.
pub const BEGIN_INTERVAL_CONSENSUS_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::ProductOccurrenceRoot, false, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusWork, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusReplay, true, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, false, false),
    INTERVAL_PRODUCT_METAS_V1[0],
    INTERVAL_PRODUCT_METAS_V1[1],
    INTERVAL_PRODUCT_METAS_V1[2],
    INTERVAL_PRODUCT_METAS_V1[3],
    INTERVAL_PRODUCT_METAS_V1[4],
    INTERVAL_PRODUCT_METAS_V1[5],
    INTERVAL_PRODUCT_METAS_V1[6],
    INTERVAL_PRODUCT_METAS_V1[7],
    INTERVAL_PRODUCT_METAS_V1[8],
    INTERVAL_PRODUCT_METAS_V1[9],
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceWindow, false, false),
    meta(RecoveryAccountRoleV1::StatisticKey, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::RootRentPayer, false, false),
    meta(RecoveryAccountRoleV1::NeutralSink, false, false),
    meta(RecoveryAccountRoleV1::RentSysvar, false, false),
    meta(RecoveryAccountRoleV1::SystemProgram, false, false),
];

/// Exact ordered contract for one bounded paid ab/ac transition.
pub const ADVANCE_INTERVAL_CONSENSUS_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusWork, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusReplay, true, false),
    meta(RecoveryAccountRoleV1::LivenessPolicy, false, false),
    meta(RecoveryAccountRoleV1::RecoveryCompartment, true, false),
    meta(RecoveryAccountRoleV1::Keeper, true, false),
    meta(RecoveryAccountRoleV1::RecoveryPayer, true, false),
    INTERVAL_PRODUCT_METAS_V1[0],
    INTERVAL_PRODUCT_METAS_V1[1],
    INTERVAL_PRODUCT_METAS_V1[2],
    INTERVAL_PRODUCT_METAS_V1[3],
    INTERVAL_PRODUCT_METAS_V1[4],
    INTERVAL_PRODUCT_METAS_V1[5],
    INTERVAL_PRODUCT_METAS_V1[6],
    INTERVAL_PRODUCT_METAS_V1[7],
    INTERVAL_PRODUCT_METAS_V1[8],
    INTERVAL_PRODUCT_METAS_V1[9],
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceWindow, false, false),
    meta(RecoveryAccountRoleV1::StatisticKey, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
];

/// Exact ordered contract for private Product capability restoration and the
/// atomic full-width Resolution V5/Hoard V2/ClaimLedger V3 postimage.
pub const RESOLVE_INTERVAL_CONSENSUS_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, true, false),
    meta(RecoveryAccountRoleV1::ProductOccurrenceRoot, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusWork, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusReplay, true, false),
    meta(RecoveryAccountRoleV1::ResolutionV5, true, false),
    meta(RecoveryAccountRoleV1::HoardV2, true, false),
    meta(RecoveryAccountRoleV1::ClaimLedgerV3, true, false),
    INTERVAL_PRODUCT_METAS_V1[0],
    INTERVAL_PRODUCT_METAS_V1[1],
    INTERVAL_PRODUCT_METAS_V1[2],
    INTERVAL_PRODUCT_METAS_V1[3],
    INTERVAL_PRODUCT_METAS_V1[4],
    INTERVAL_PRODUCT_METAS_V1[5],
    INTERVAL_PRODUCT_METAS_V1[6],
    INTERVAL_PRODUCT_METAS_V1[7],
    INTERVAL_PRODUCT_METAS_V1[8],
    INTERVAL_PRODUCT_METAS_V1[9],
    meta(RecoveryAccountRoleV1::SourceRelease, false, false),
    meta(RecoveryAccountRoleV1::SourceOccurrence, false, false),
    meta(RecoveryAccountRoleV1::SourceWindow, false, false),
    meta(RecoveryAccountRoleV1::StatisticKey, false, false),
    meta(RecoveryAccountRoleV1::SourceResult, false, false),
    meta(RecoveryAccountRoleV1::SourceWorkReceipt, false, false),
    meta(RecoveryAccountRoleV1::ClockSysvar, false, false),
    meta(RecoveryAccountRoleV1::SystemProgram, false, false),
];

/// Exact ordered contract for closing only deletable ab work.
pub const CLOSE_INTERVAL_CONSENSUS_WORK_METAS_V1: &[RecoveryAccountMetaV1] = &[
    meta(RecoveryAccountRoleV1::FailureRoot, false, false),
    meta(RecoveryAccountRoleV1::ProductOccurrenceRoot, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusWork, true, false),
    meta(RecoveryAccountRoleV1::IntervalConsensusReplay, true, false),
    meta(RecoveryAccountRoleV1::RootRentPayer, true, false),
    meta(RecoveryAccountRoleV1::NeutralSink, true, false),
];

/// Return the exact ordered account contract for one allocated action.
pub const fn account_metas_v1(
    action: registry::RecoveryAction,
) -> &'static [RecoveryAccountMetaV1] {
    match action {
        registry::RecoveryAction::InitializeFailureRoot => INITIALIZE_FAILURE_ROOT_METAS_V1,
        registry::RecoveryAction::TriggerSourceFailure => TRIGGER_SOURCE_FAILURE_METAS_V1,
        registry::RecoveryAction::TriggerRelationRefusal => TRIGGER_RELATION_REFUSAL_METAS_V1,
        registry::RecoveryAction::AdvanceRecoverySchedule => ADVANCE_RECOVERY_SCHEDULE_METAS_V1,
        registry::RecoveryAction::AcceptRecoveryWork => ACCEPT_RECOVERY_WORK_METAS_V1,
        registry::RecoveryAction::ResolveCallerFunded => RESOLVE_CALLER_FUNDED_METAS_V1,
        registry::RecoveryAction::ResolvePaidRecovery => RESOLVE_PAID_RECOVERY_METAS_V1,
        registry::RecoveryAction::CloseRecoveryFunding => CLOSE_RECOVERY_FUNDING_METAS_V1,
        registry::RecoveryAction::CloseFailureRoot => CLOSE_FAILURE_ROOT_METAS_V1,
        registry::RecoveryAction::BeginIntervalConsensus => BEGIN_INTERVAL_CONSENSUS_METAS_V1,
        registry::RecoveryAction::AdvanceIntervalConsensus => ADVANCE_INTERVAL_CONSENSUS_METAS_V1,
        registry::RecoveryAction::ResolveIntervalConsensus => RESOLVE_INTERVAL_CONSENSUS_METAS_V1,
        registry::RecoveryAction::CloseIntervalConsensusWork => {
            CLOSE_INTERVAL_CONSENSUS_WORK_METAS_V1
        }
    }
}

/// Decode exactly the payload shape selected by the Recovery action tag.
pub fn decode_payload_v1(
    action: registry::RecoveryAction,
    input: &[u8],
) -> Result<FailureRecoveryPayloadV1> {
    let exact = payload_bytes_v1(action);
    require_exact(input, exact)?;
    let common = decode_common(input)?;
    match action {
        registry::RecoveryAction::InitializeFailureRoot => {
            require_reserved(&input[116..120])?;
            let value = InitializeFailureRootV1 {
                common,
                series_plan_v5_id: id_at(input, 80),
                ordinal: u32_at(input, 112),
                series_funding_quote_id: id_at(input, 120),
                market_core_funding_receipt_id: id_at(input, 152),
                root_rent_principal_lamports: u64_at(input, 184),
                replay_rent_principal_lamports: u64_at(input, 192),
            };
            require_live(value.series_plan_v5_id)?;
            require_live(value.series_funding_quote_id)?;
            require_live(value.market_core_funding_receipt_id)?;
            if value.root_rent_principal_lamports == 0 || value.replay_rent_principal_lamports == 0
            {
                return Err(CodecError::ZeroValue);
            }
            Ok(FailureRecoveryPayloadV1::InitializeFailureRoot(value))
        }
        registry::RecoveryAction::TriggerSourceFailure => {
            let id = id_at(input, 80);
            require_live(id)?;
            Ok(FailureRecoveryPayloadV1::TriggerSourceFailure(
                TriggerSourceFailureV1 {
                    common,
                    source_failure_handoff_id: id,
                },
            ))
        }
        registry::RecoveryAction::TriggerRelationRefusal => {
            require_reserved(&input[212..216])?;
            let value = TriggerRelationRefusalV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                relation_policy_id: id_at(input, 112),
                relation_record_id: id_at(input, 144),
                relation_execution_id: id_at(input, 176),
                refusal_code: u32_at(input, 208),
            };
            for id in [
                value.source_success_handoff_id,
                value.relation_policy_id,
                value.relation_record_id,
                value.relation_execution_id,
            ] {
                require_live(id)?;
            }
            if !(1..=5).contains(&value.refusal_code) {
                return Err(CodecError::InvalidEnum);
            }
            Ok(FailureRecoveryPayloadV1::TriggerRelationRefusal(value))
        }
        registry::RecoveryAction::AdvanceRecoverySchedule => {
            require_reserved(&input[81..88])?;
            Ok(FailureRecoveryPayloadV1::AdvanceRecoverySchedule(
                AdvanceRecoveryScheduleV1 {
                    common,
                    expected_attempt_index: input[80],
                },
            ))
        }
        registry::RecoveryAction::AcceptRecoveryWork => {
            let value = AcceptRecoveryWorkV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                reward_recipient: id_at(input, 112),
                scheduled_ceiling_lamports: u64_at(input, 144),
            };
            require_work(
                value.source_success_handoff_id,
                value.reward_recipient,
                value.scheduled_ceiling_lamports,
            )?;
            Ok(FailureRecoveryPayloadV1::AcceptRecoveryWork(value))
        }
        registry::RecoveryAction::ResolveCallerFunded => {
            let value = ResolveCallerFundedV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                relation_policy_id: id_at(input, 112),
                relation_record_id: id_at(input, 144),
                relation_execution_id: id_at(input, 176),
            };
            for id in [
                value.source_success_handoff_id,
                value.relation_policy_id,
                value.relation_record_id,
                value.relation_execution_id,
            ] {
                require_live(id)?;
            }
            Ok(FailureRecoveryPayloadV1::ResolveCallerFunded(value))
        }
        registry::RecoveryAction::ResolvePaidRecovery => {
            let value = ResolvePaidRecoveryV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                relation_policy_id: id_at(input, 112),
                relation_record_id: id_at(input, 144),
                relation_execution_id: id_at(input, 176),
                reward_recipient: id_at(input, 208),
                scheduled_ceiling_lamports: u64_at(input, 240),
            };
            for id in [
                value.relation_policy_id,
                value.relation_record_id,
                value.relation_execution_id,
            ] {
                require_live(id)?;
            }
            require_work(
                value.source_success_handoff_id,
                value.reward_recipient,
                value.scheduled_ceiling_lamports,
            )?;
            Ok(FailureRecoveryPayloadV1::ResolvePaidRecovery(value))
        }
        registry::RecoveryAction::CloseRecoveryFunding => {
            let id = id_at(input, 80);
            require_live(id)?;
            Ok(FailureRecoveryPayloadV1::CloseRecoveryFunding(
                CloseRecoveryFundingV1 {
                    common,
                    recovery_terminal_receipt_id: id,
                },
            ))
        }
        registry::RecoveryAction::CloseFailureRoot => {
            let value = CloseFailureRootV1 {
                common,
                failure_terminal_join_id: id_at(input, 80),
                retirement_root_id: id_at(input, 112),
                replay_tombstone_id: id_at(input, 144),
                source_release_receipt_id: id_at(input, 176),
            };
            for id in [
                value.failure_terminal_join_id,
                value.retirement_root_id,
                value.replay_tombstone_id,
                value.source_release_receipt_id,
            ] {
                require_live(id)?;
            }
            Ok(FailureRecoveryPayloadV1::CloseFailureRoot(value))
        }
        registry::RecoveryAction::BeginIntervalConsensus => {
            let value = BeginIntervalConsensusV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                interval_profile_id: id_at(input, 112),
                funding_admission_receipt_id: id_at(input, 144),
                initial_work_id: id_at(input, 176),
                work_rent_principal_lamports: u64_at(input, 208),
                replay_rent_principal_lamports: u64_at(input, 216),
            };
            for id in [
                value.source_success_handoff_id,
                value.interval_profile_id,
                value.funding_admission_receipt_id,
                value.initial_work_id,
            ] {
                require_live(id)?;
            }
            if value.work_rent_principal_lamports == 0 || value.replay_rent_principal_lamports == 0
            {
                return Err(CodecError::ZeroValue);
            }
            Ok(FailureRecoveryPayloadV1::BeginIntervalConsensus(value))
        }
        registry::RecoveryAction::AdvanceIntervalConsensus => {
            require_reserved(&input[90..96])?;
            let value = AdvanceIntervalConsensusV1 {
                common,
                expected_work_transition_nonce: u64_at(input, 80),
                requested_coordinates: u16::from_le_bytes([input[88], input[89]]),
                reward_recipient: id_at(input, 96),
                scheduled_ceiling_lamports: u64_at(input, 128),
                before_work_id: id_at(input, 136),
                after_work_id: id_at(input, 168),
                replay_receipt_id: id_at(input, 200),
            };
            for id in [
                value.reward_recipient,
                value.before_work_id,
                value.after_work_id,
                value.replay_receipt_id,
            ] {
                require_live(id)?;
            }
            if value.requested_coordinates == 0
                || value.scheduled_ceiling_lamports == 0
                || value.expected_work_transition_nonce == u64::MAX
                || value.before_work_id == value.after_work_id
            {
                return Err(CodecError::ZeroValue);
            }
            Ok(FailureRecoveryPayloadV1::AdvanceIntervalConsensus(value))
        }
        registry::RecoveryAction::ResolveIntervalConsensus => {
            let value = ResolveIntervalConsensusV1 {
                common,
                source_success_handoff_id: id_at(input, 80),
                completed_work_id: id_at(input, 112),
                certificate_id: id_at(input, 144),
                replay_receipt_id: id_at(input, 176),
            };
            for id in [
                value.source_success_handoff_id,
                value.completed_work_id,
                value.certificate_id,
                value.replay_receipt_id,
            ] {
                require_live(id)?;
            }
            Ok(FailureRecoveryPayloadV1::ResolveIntervalConsensus(value))
        }
        registry::RecoveryAction::CloseIntervalConsensusWork => {
            let value = CloseIntervalConsensusWorkV1 {
                common,
                certificate_id: id_at(input, 80),
                resolution_receipt_id: id_at(input, 112),
                replay_receipt_id: id_at(input, 144),
                work_close_authorization_id: id_at(input, 176),
            };
            for id in [
                value.certificate_id,
                value.resolution_receipt_id,
                value.replay_receipt_id,
                value.work_close_authorization_id,
            ] {
                require_live(id)?;
            }
            Ok(FailureRecoveryPayloadV1::CloseIntervalConsensusWork(value))
        }
    }
}

/// Encode one exact Recovery action payload.
pub fn encode_payload_v1(value: &FailureRecoveryPayloadV1, output: &mut [u8]) -> Result<usize> {
    let (action, common) = match value {
        FailureRecoveryPayloadV1::InitializeFailureRoot(v) => {
            (registry::RecoveryAction::InitializeFailureRoot, v.common)
        }
        FailureRecoveryPayloadV1::TriggerSourceFailure(v) => {
            (registry::RecoveryAction::TriggerSourceFailure, v.common)
        }
        FailureRecoveryPayloadV1::TriggerRelationRefusal(v) => {
            (registry::RecoveryAction::TriggerRelationRefusal, v.common)
        }
        FailureRecoveryPayloadV1::AdvanceRecoverySchedule(v) => {
            (registry::RecoveryAction::AdvanceRecoverySchedule, v.common)
        }
        FailureRecoveryPayloadV1::AcceptRecoveryWork(v) => {
            (registry::RecoveryAction::AcceptRecoveryWork, v.common)
        }
        FailureRecoveryPayloadV1::ResolveCallerFunded(v) => {
            (registry::RecoveryAction::ResolveCallerFunded, v.common)
        }
        FailureRecoveryPayloadV1::ResolvePaidRecovery(v) => {
            (registry::RecoveryAction::ResolvePaidRecovery, v.common)
        }
        FailureRecoveryPayloadV1::CloseRecoveryFunding(v) => {
            (registry::RecoveryAction::CloseRecoveryFunding, v.common)
        }
        FailureRecoveryPayloadV1::CloseFailureRoot(v) => {
            (registry::RecoveryAction::CloseFailureRoot, v.common)
        }
        FailureRecoveryPayloadV1::BeginIntervalConsensus(v) => {
            (registry::RecoveryAction::BeginIntervalConsensus, v.common)
        }
        FailureRecoveryPayloadV1::AdvanceIntervalConsensus(v) => {
            (registry::RecoveryAction::AdvanceIntervalConsensus, v.common)
        }
        FailureRecoveryPayloadV1::ResolveIntervalConsensus(v) => {
            (registry::RecoveryAction::ResolveIntervalConsensus, v.common)
        }
        FailureRecoveryPayloadV1::CloseIntervalConsensusWork(v) => (
            registry::RecoveryAction::CloseIntervalConsensusWork,
            v.common,
        ),
    };
    let exact = payload_bytes_v1(action);
    require_exact(output, exact)?;
    output.fill(0);
    encode_common(common, output)?;
    match value {
        FailureRecoveryPayloadV1::InitializeFailureRoot(v) => {
            output[80..112].copy_from_slice(&v.series_plan_v5_id);
            output[112..116].copy_from_slice(&v.ordinal.to_le_bytes());
            output[120..152].copy_from_slice(&v.series_funding_quote_id);
            output[152..184].copy_from_slice(&v.market_core_funding_receipt_id);
            output[184..192].copy_from_slice(&v.root_rent_principal_lamports.to_le_bytes());
            output[192..200].copy_from_slice(&v.replay_rent_principal_lamports.to_le_bytes());
        }
        FailureRecoveryPayloadV1::TriggerSourceFailure(v) => {
            output[80..112].copy_from_slice(&v.source_failure_handoff_id)
        }
        FailureRecoveryPayloadV1::TriggerRelationRefusal(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.relation_policy_id);
            output[144..176].copy_from_slice(&v.relation_record_id);
            output[176..208].copy_from_slice(&v.relation_execution_id);
            output[208..212].copy_from_slice(&v.refusal_code.to_le_bytes());
        }
        FailureRecoveryPayloadV1::AdvanceRecoverySchedule(v) => {
            output[80] = v.expected_attempt_index
        }
        FailureRecoveryPayloadV1::AcceptRecoveryWork(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.reward_recipient);
            output[144..152].copy_from_slice(&v.scheduled_ceiling_lamports.to_le_bytes());
        }
        FailureRecoveryPayloadV1::ResolveCallerFunded(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.relation_policy_id);
            output[144..176].copy_from_slice(&v.relation_record_id);
            output[176..208].copy_from_slice(&v.relation_execution_id);
        }
        FailureRecoveryPayloadV1::ResolvePaidRecovery(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.relation_policy_id);
            output[144..176].copy_from_slice(&v.relation_record_id);
            output[176..208].copy_from_slice(&v.relation_execution_id);
            output[208..240].copy_from_slice(&v.reward_recipient);
            output[240..248].copy_from_slice(&v.scheduled_ceiling_lamports.to_le_bytes());
        }
        FailureRecoveryPayloadV1::CloseRecoveryFunding(v) => {
            output[80..112].copy_from_slice(&v.recovery_terminal_receipt_id)
        }
        FailureRecoveryPayloadV1::CloseFailureRoot(v) => {
            output[80..112].copy_from_slice(&v.failure_terminal_join_id);
            output[112..144].copy_from_slice(&v.retirement_root_id);
            output[144..176].copy_from_slice(&v.replay_tombstone_id);
            output[176..208].copy_from_slice(&v.source_release_receipt_id);
        }
        FailureRecoveryPayloadV1::BeginIntervalConsensus(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.interval_profile_id);
            output[144..176].copy_from_slice(&v.funding_admission_receipt_id);
            output[176..208].copy_from_slice(&v.initial_work_id);
            output[208..216].copy_from_slice(&v.work_rent_principal_lamports.to_le_bytes());
            output[216..224].copy_from_slice(&v.replay_rent_principal_lamports.to_le_bytes());
        }
        FailureRecoveryPayloadV1::AdvanceIntervalConsensus(v) => {
            output[80..88].copy_from_slice(&v.expected_work_transition_nonce.to_le_bytes());
            output[88..90].copy_from_slice(&v.requested_coordinates.to_le_bytes());
            output[96..128].copy_from_slice(&v.reward_recipient);
            output[128..136].copy_from_slice(&v.scheduled_ceiling_lamports.to_le_bytes());
            output[136..168].copy_from_slice(&v.before_work_id);
            output[168..200].copy_from_slice(&v.after_work_id);
            output[200..232].copy_from_slice(&v.replay_receipt_id);
        }
        FailureRecoveryPayloadV1::ResolveIntervalConsensus(v) => {
            output[80..112].copy_from_slice(&v.source_success_handoff_id);
            output[112..144].copy_from_slice(&v.completed_work_id);
            output[144..176].copy_from_slice(&v.certificate_id);
            output[176..208].copy_from_slice(&v.replay_receipt_id);
        }
        FailureRecoveryPayloadV1::CloseIntervalConsensusWork(v) => {
            output[80..112].copy_from_slice(&v.certificate_id);
            output[112..144].copy_from_slice(&v.resolution_receipt_id);
            output[144..176].copy_from_slice(&v.replay_receipt_id);
            output[176..208].copy_from_slice(&v.work_close_authorization_id);
        }
    }
    decode_payload_v1(action, output)?;
    Ok(exact)
}

/// Return the exact payload width for one allocated Recovery action.
pub const fn payload_bytes_v1(action: registry::RecoveryAction) -> usize {
    match action {
        registry::RecoveryAction::InitializeFailureRoot => INITIALIZE_FAILURE_ROOT_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::TriggerSourceFailure => TRIGGER_SOURCE_FAILURE_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::TriggerRelationRefusal => {
            TRIGGER_RELATION_REFUSAL_PAYLOAD_BYTES_V1
        }
        registry::RecoveryAction::AdvanceRecoverySchedule => {
            ADVANCE_RECOVERY_SCHEDULE_PAYLOAD_BYTES_V1
        }
        registry::RecoveryAction::AcceptRecoveryWork => ACCEPT_RECOVERY_WORK_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::ResolveCallerFunded => RESOLVE_CALLER_FUNDED_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::ResolvePaidRecovery => RESOLVE_PAID_RECOVERY_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::CloseRecoveryFunding => CLOSE_RECOVERY_FUNDING_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::CloseFailureRoot => CLOSE_FAILURE_ROOT_PAYLOAD_BYTES_V1,
        registry::RecoveryAction::BeginIntervalConsensus => {
            BEGIN_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1
        }
        registry::RecoveryAction::AdvanceIntervalConsensus => {
            ADVANCE_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1
        }
        registry::RecoveryAction::ResolveIntervalConsensus => {
            RESOLVE_INTERVAL_CONSENSUS_PAYLOAD_BYTES_V1
        }
        registry::RecoveryAction::CloseIntervalConsensusWork => {
            CLOSE_INTERVAL_CONSENSUS_WORK_PAYLOAD_BYTES_V1
        }
    }
}

fn decode_common(input: &[u8]) -> Result<RecoveryCommonV1> {
    let value = RecoveryCommonV1 {
        binding_id: id_at(input, 0),
        market_instance_v2_id: id_at(input, 32),
        generation: u64_at(input, 64),
        expected_transition_nonce: u64_at(input, 72),
    };
    value.validate()?;
    Ok(value)
}

fn encode_common(value: RecoveryCommonV1, output: &mut [u8]) -> Result<()> {
    value.validate()?;
    output[..32].copy_from_slice(&value.binding_id);
    output[32..64].copy_from_slice(&value.market_instance_v2_id);
    output[64..72].copy_from_slice(&value.generation.to_le_bytes());
    output[72..80].copy_from_slice(&value.expected_transition_nonce.to_le_bytes());
    Ok(())
}

fn require_work(source: [u8; HASH_BYTES], recipient: [u8; HASH_BYTES], ceiling: u64) -> Result<()> {
    require_live(source)?;
    require_live(recipient)?;
    if ceiling == 0 {
        return Err(CodecError::ZeroValue);
    }
    Ok(())
}

fn require_exact(input: &[u8], exact: usize) -> Result<()> {
    if input.len() < exact {
        Err(CodecError::Truncated)
    } else if input.len() > exact {
        Err(CodecError::TrailingBytes)
    } else {
        Ok(())
    }
}

fn require_live(bytes: [u8; HASH_BYTES]) -> Result<()> {
    if is_zero(&bytes) {
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
    let mut value = [0; HASH_BYTES];
    value.copy_from_slice(&input[offset..offset + HASH_BYTES]);
    value
}

fn u32_at(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn u64_at(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
        input[offset + 4],
        input[offset + 5],
        input[offset + 6],
        input[offset + 7],
    ])
}

const _: () =
    assert!(MAX_FAILURE_RECOVERY_PAYLOAD_BYTES_V1 <= registry::MAX_EXTENSION_PAYLOAD_BYTES);

#[cfg(test)]
mod tests {
    use super::*;

    fn common() -> RecoveryCommonV1 {
        RecoveryCommonV1 {
            binding_id: [1; 32],
            market_instance_v2_id: [2; 32],
            generation: 3,
            expected_transition_nonce: 4,
        }
    }

    #[test]
    fn every_payload_round_trips_and_refuses_trailing_bytes() {
        let values = [
            FailureRecoveryPayloadV1::InitializeFailureRoot(InitializeFailureRootV1 {
                common: common(),
                series_plan_v5_id: [5; 32],
                ordinal: 6,
                series_funding_quote_id: [7; 32],
                market_core_funding_receipt_id: [8; 32],
                root_rent_principal_lamports: 9,
                replay_rent_principal_lamports: 10,
            }),
            FailureRecoveryPayloadV1::TriggerSourceFailure(TriggerSourceFailureV1 {
                common: common(),
                source_failure_handoff_id: [9; 32],
            }),
            FailureRecoveryPayloadV1::TriggerRelationRefusal(TriggerRelationRefusalV1 {
                common: common(),
                source_success_handoff_id: [10; 32],
                relation_policy_id: [11; 32],
                relation_record_id: [12; 32],
                relation_execution_id: [13; 32],
                refusal_code: 2,
            }),
            FailureRecoveryPayloadV1::AdvanceRecoverySchedule(AdvanceRecoveryScheduleV1 {
                common: common(),
                expected_attempt_index: 1,
            }),
            FailureRecoveryPayloadV1::AcceptRecoveryWork(AcceptRecoveryWorkV1 {
                common: common(),
                source_success_handoff_id: [12; 32],
                reward_recipient: [13; 32],
                scheduled_ceiling_lamports: 14,
            }),
            FailureRecoveryPayloadV1::ResolveCallerFunded(ResolveCallerFundedV1 {
                common: common(),
                source_success_handoff_id: [15; 32],
                relation_policy_id: [16; 32],
                relation_record_id: [17; 32],
                relation_execution_id: [18; 32],
            }),
            FailureRecoveryPayloadV1::ResolvePaidRecovery(ResolvePaidRecoveryV1 {
                common: common(),
                source_success_handoff_id: [19; 32],
                relation_policy_id: [20; 32],
                relation_record_id: [21; 32],
                relation_execution_id: [22; 32],
                reward_recipient: [23; 32],
                scheduled_ceiling_lamports: 20,
            }),
            FailureRecoveryPayloadV1::CloseRecoveryFunding(CloseRecoveryFundingV1 {
                common: common(),
                recovery_terminal_receipt_id: [21; 32],
            }),
            FailureRecoveryPayloadV1::CloseFailureRoot(CloseFailureRootV1 {
                common: common(),
                failure_terminal_join_id: [22; 32],
                retirement_root_id: [23; 32],
                replay_tombstone_id: [24; 32],
                source_release_receipt_id: [25; 32],
            }),
            FailureRecoveryPayloadV1::BeginIntervalConsensus(BeginIntervalConsensusV1 {
                common: common(),
                source_success_handoff_id: [26; 32],
                interval_profile_id: [27; 32],
                funding_admission_receipt_id: [28; 32],
                initial_work_id: [29; 32],
                work_rent_principal_lamports: 30,
                replay_rent_principal_lamports: 31,
            }),
            FailureRecoveryPayloadV1::AdvanceIntervalConsensus(AdvanceIntervalConsensusV1 {
                common: common(),
                expected_work_transition_nonce: 1,
                requested_coordinates: 2,
                reward_recipient: [32; 32],
                scheduled_ceiling_lamports: 33,
                before_work_id: [34; 32],
                after_work_id: [35; 32],
                replay_receipt_id: [36; 32],
            }),
            FailureRecoveryPayloadV1::ResolveIntervalConsensus(ResolveIntervalConsensusV1 {
                common: common(),
                source_success_handoff_id: [37; 32],
                completed_work_id: [38; 32],
                certificate_id: [39; 32],
                replay_receipt_id: [40; 32],
            }),
            FailureRecoveryPayloadV1::CloseIntervalConsensusWork(CloseIntervalConsensusWorkV1 {
                common: common(),
                certificate_id: [41; 32],
                resolution_receipt_id: [42; 32],
                replay_receipt_id: [43; 32],
                work_close_authorization_id: [44; 32],
            }),
        ];
        for value in values {
            let action = match value {
                FailureRecoveryPayloadV1::InitializeFailureRoot(_) => {
                    registry::RecoveryAction::InitializeFailureRoot
                }
                FailureRecoveryPayloadV1::TriggerSourceFailure(_) => {
                    registry::RecoveryAction::TriggerSourceFailure
                }
                FailureRecoveryPayloadV1::TriggerRelationRefusal(_) => {
                    registry::RecoveryAction::TriggerRelationRefusal
                }
                FailureRecoveryPayloadV1::AdvanceRecoverySchedule(_) => {
                    registry::RecoveryAction::AdvanceRecoverySchedule
                }
                FailureRecoveryPayloadV1::AcceptRecoveryWork(_) => {
                    registry::RecoveryAction::AcceptRecoveryWork
                }
                FailureRecoveryPayloadV1::ResolveCallerFunded(_) => {
                    registry::RecoveryAction::ResolveCallerFunded
                }
                FailureRecoveryPayloadV1::ResolvePaidRecovery(_) => {
                    registry::RecoveryAction::ResolvePaidRecovery
                }
                FailureRecoveryPayloadV1::CloseRecoveryFunding(_) => {
                    registry::RecoveryAction::CloseRecoveryFunding
                }
                FailureRecoveryPayloadV1::CloseFailureRoot(_) => {
                    registry::RecoveryAction::CloseFailureRoot
                }
                FailureRecoveryPayloadV1::BeginIntervalConsensus(_) => {
                    registry::RecoveryAction::BeginIntervalConsensus
                }
                FailureRecoveryPayloadV1::AdvanceIntervalConsensus(_) => {
                    registry::RecoveryAction::AdvanceIntervalConsensus
                }
                FailureRecoveryPayloadV1::ResolveIntervalConsensus(_) => {
                    registry::RecoveryAction::ResolveIntervalConsensus
                }
                FailureRecoveryPayloadV1::CloseIntervalConsensusWork(_) => {
                    registry::RecoveryAction::CloseIntervalConsensusWork
                }
            };
            let mut bytes = [0; MAX_FAILURE_RECOVERY_PAYLOAD_BYTES_V1 + 1];
            let exact = payload_bytes_v1(action);
            assert_eq!(encode_payload_v1(&value, &mut bytes[..exact]), Ok(exact));
            assert_eq!(decode_payload_v1(action, &bytes[..exact]), Ok(value));
            assert_eq!(
                decode_payload_v1(action, &bytes[..exact - 1]),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                decode_payload_v1(action, &bytes[..exact + 1]),
                Err(CodecError::TrailingBytes)
            );
        }
    }

    #[test]
    fn interval_resolution_requires_the_atomic_liability_postimage() {
        let metas = account_metas_v1(registry::RecoveryAction::ResolveIntervalConsensus);
        assert_eq!(metas.len(), 25);
        assert_eq!(metas[1].role, RecoveryAccountRoleV1::ProductOccurrenceRoot);
        assert!(metas[1].writable);
        assert_eq!(metas[4].role, RecoveryAccountRoleV1::ResolutionV5);
        assert!(metas[4].writable);
        assert_eq!(metas[5].role, RecoveryAccountRoleV1::HoardV2);
        assert!(metas[5].writable);
        assert_eq!(metas[6].role, RecoveryAccountRoleV1::ClaimLedgerV3);
        assert!(metas[6].writable);
        assert_eq!(metas[24].role, RecoveryAccountRoleV1::SystemProgram);
        assert!(!metas.iter().any(|meta| matches!(
            meta.role,
            RecoveryAccountRoleV1::RecoveryCompartment
                | RecoveryAccountRoleV1::RootRentPayer
                | RecoveryAccountRoleV1::NeutralSink
        )));
    }
}
