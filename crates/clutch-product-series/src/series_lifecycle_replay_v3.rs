//! Bounded permanent replay owner for one finite Series lifecycle.
//!
//! FundingV5 proves that every ordinal was either completed or lapsed. The
//! historical replay V2 is decode-only and never accepted as this authority.
//! Each Market root proves terminality only for links admitted to that one
//! Market. This owner defines the bounded cross-Market count equations and
//! transcript without scanning an unbounded account set. It is not, by itself,
//! physical account authority: the adapter must make admission inseparable
//! from creation of the one canonical ordinal link, make retirement inseparable
//! from consumption and close of that same still-live link, and forbid every
//! completion/lapse/close bypass. The final pure projection is evidence for,
//! not a replacement for, hostile postwrite authentication of the permanent
//! replay account. Solana coordinates, rent and write authority stay in the
//! adapter; this module allocates no tag and owns only fixed semantics.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV7Id, ContentId, Error, FixedCodec,
    MarketInstanceV2Id, RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id, Result,
    SeriesAttachmentPlanV6Id, SeriesFundingQuoteV6Id, SeriesFundingTermsV2Id,
    SeriesLifecycleReplayBindingV3Id, SeriesLifecycleReplayV3Id,
    SeriesLifecycleTerminalProjectionV3Id, SeriesMarketDispositionV1, SeriesPlanV5Id,
    SourceOccurrenceV1Id,
};

const SERIES_LIFECYCLE_REPLAY_BINDING_MAGIC_V3: [u8; 8] = *b"DCSLRBV3";
const SERIES_LIFECYCLE_REPLAY_MAGIC_V3: [u8; 8] = *b"DCSLRPV3";
const SERIES_LIFECYCLE_REPLAY_SCHEMA_V3: u16 = 3;
const SERIES_LIFECYCLE_ADMISSION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-admission/v3";
const SERIES_LIFECYCLE_LAPSE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-lapse/v3";
const SERIES_LIFECYCLE_LINK_RETIREMENT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-link-retirement/v3";
const SERIES_LIFECYCLE_TRANSCRIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-transcript/v3";

/// Exact immutable binding width.
pub const SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3: usize = 398;
/// Exact counted semantic state width.
pub const SERIES_LIFECYCLE_REPLAY_BYTES_V3: usize = 501;
/// Immutable binding identity domain.
pub const SERIES_LIFECYCLE_REPLAY_BINDING_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-replay-binding/v3";
/// Complete state identity domain.
pub const SERIES_LIFECYCLE_REPLAY_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-replay/v3";
/// Exhaustive terminal projection identity domain.
pub const SERIES_LIFECYCLE_TERMINAL_PROJECTION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-terminal-projection/v3";

/// Immutable current-artifact and physical-coordinate binding for one Series.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleReplayBindingV3 {
    /// Exact current Series plan.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact current funding terms.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact current 50-slot quote.
    pub funding_quote_id: SeriesFundingQuoteV6Id,
    /// Exact current attachment plan.
    pub attachment_plan_id: SeriesAttachmentPlanV6Id,
    /// Exact current compiler output.
    pub compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    /// Loader-authenticated current Registry release.
    pub registry_release_id: RegistryProgramReleaseV2Id,
    /// Current capability profile.
    pub capability_profile_id: RegistryCapabilityProfileV4Id,
    /// Permanent Series registry/replay account.
    pub registry_account_id: ContentId,
    /// Current FundingV5 account.
    pub funding_account_id: ContentId,
    /// Permanent counted lifecycle replay account.
    pub lifecycle_replay_account_id: ContentId,
    /// Exact payer identity that supplied the permanently retained rent.
    pub permanent_rent_funder: ContentId,
    /// Exact neutral lamport sink.
    pub neutral_lamport_sink: ContentId,
    /// Finite Series cardinality.
    pub instance_count: u32,
}

impl SeriesLifecycleReplayBindingV3 {
    /// Validate the immutable graph without claiming account authority.
    pub fn validate(self) -> Result<()> {
        if self.instance_count == 0 {
            return Err(Error::InvalidParameter);
        }
        let ids = self.ids();
        let mut left = 0usize;
        while left < ids.len() {
            ids[left].validate()?;
            let mut right = left + 1;
            while right < ids.len() {
                if ids[left] == ids[right] {
                    return Err(Error::MismatchedArtifact);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    /// Domain-separated immutable binding identity.
    pub fn id(self) -> Result<SeriesLifecycleReplayBindingV3Id> {
        let mut body = [0u8; SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3];
        self.encode_into(&mut body)?;
        Ok(SeriesLifecycleReplayBindingV3Id::from_bytes(
            content_id(SERIES_LIFECYCLE_REPLAY_BINDING_DOMAIN_V3, &body).bytes(),
        ))
    }

    fn ids(self) -> [ContentId; 12] {
        [
            self.series_plan_id.content_id(),
            self.funding_terms_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.compiler_bundle_id.content_id(),
            self.registry_release_id.content_id(),
            self.capability_profile_id.content_id(),
            self.registry_account_id,
            self.funding_account_id,
            self.lifecycle_replay_account_id,
            self.permanent_rent_funder,
            self.neutral_lamport_sink,
        ]
    }
}

impl FixedCodec for SeriesLifecycleReplayBindingV3 {
    const ENCODED_LEN: usize = SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_LIFECYCLE_REPLAY_BINDING_MAGIC_V3);
        writer.u16(SERIES_LIFECYCLE_REPLAY_SCHEMA_V3);
        for id in self.ids() {
            writer.id(id);
        }
        writer.u32(self.instance_count);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SERIES_LIFECYCLE_REPLAY_BINDING_MAGIC_V3)?;
        if reader.u16() != SERIES_LIFECYCLE_REPLAY_SCHEMA_V3 {
            return Err(Error::BadVersion);
        }
        let ids = [
            reader.id(), reader.id(), reader.id(), reader.id(), reader.id(), reader.id(),
            reader.id(), reader.id(), reader.id(), reader.id(), reader.id(), reader.id(),
        ];
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(ids[0].bytes()),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(ids[1].bytes()),
            funding_quote_id: SeriesFundingQuoteV6Id::from_bytes(ids[2].bytes()),
            attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes(ids[3].bytes()),
            compiler_bundle_id: CompiledProductSeriesBundleV7Id::from_bytes(ids[4].bytes()),
            registry_release_id: RegistryProgramReleaseV2Id::from_bytes(ids[5].bytes()),
            capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(ids[6].bytes()),
            registry_account_id: ids[7],
            funding_account_id: ids[8],
            lifecycle_replay_account_id: ids[9],
            permanent_rent_funder: ids[10],
            neutral_lamport_sink: ids[11],
            instance_count: reader.u32(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Counted lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesLifecycleReplayPhaseV3 {
    /// Ordinal admission/lapse and link retirement remain possible.
    Open,
    /// Every ordinal and every admitted link is exhaustively terminal.
    Terminal,
}

impl SeriesLifecycleReplayPhaseV3 {
    const fn byte(self) -> u8 {
        match self {
            Self::Open => 1,
            Self::Terminal => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Open),
            2 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact Product/Funding admission evidence for one completed ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleAdmissionProjectionV3 {
    /// Immutable aggregate binding.
    pub binding_id: SeriesLifecycleReplayBindingV3Id,
    /// Exact Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Sequential ordinal.
    pub ordinal: u32,
    /// Exact current FundingV5 account.
    pub funding_account_id: ContentId,
    /// FundingV5 Pending semantic prestate.
    pub funding_state_before_id: ContentId,
    /// Deterministically previewed FundingV5 completion poststate.
    pub funding_state_after_id: ContentId,
    /// Private acyclic Product completion authorization consumed after Replay.
    pub occurrence_completion_receipt_id: ContentId,
    /// Physical activated `0xad` link.
    pub link_account_id: ContentId,
    /// Private Product link activation receipt.
    pub link_activation_receipt_id: ContentId,
    /// Exact shared-root Market admission receipt.
    pub market_admission_receipt_id: ContentId,
    /// Exact economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact compiled Source occurrence.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Current compiler bundle.
    pub compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    /// Founder or converger.
    pub disposition: SeriesMarketDispositionV1,
    /// Nonzero Market generation.
    pub generation: u64,
}

impl SeriesLifecycleAdmissionProjectionV3 {
    /// Validate and hash complete admission evidence.
    pub fn id(self) -> Result<ContentId> {
        self.binding_id.validate()?;
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()?;
        self.source_occurrence_id.validate()?;
        self.compiler_bundle_id.validate()?;
        if self.generation == 0 {
            return Err(Error::InvalidParameter);
        }
        for id in [
            self.funding_account_id,
            self.funding_state_before_id,
            self.funding_state_after_id,
            self.occurrence_completion_receipt_id,
            self.link_account_id,
            self.link_activation_receipt_id,
            self.market_admission_receipt_id,
        ] {
            id.validate()?;
        }
        if self.funding_state_before_id == self.funding_state_after_id {
            return Err(Error::WorkStateMismatch);
        }
        let mut body = [0u8; 397];
        let mut writer = Writer::new(&mut body, 397)?;
        writer.id(self.binding_id.content_id());
        writer.id(self.series_plan_id.content_id());
        writer.u32(self.ordinal);
        writer.id(self.funding_account_id);
        writer.id(self.funding_state_before_id);
        writer.id(self.funding_state_after_id);
        writer.id(self.occurrence_completion_receipt_id);
        writer.id(self.link_account_id);
        writer.id(self.link_activation_receipt_id);
        writer.id(self.market_admission_receipt_id);
        writer.id(self.market_instance_id.content_id());
        writer.id(self.source_occurrence_id.content_id());
        writer.id(self.compiler_bundle_id.content_id());
        writer.u8(disposition_byte(self.disposition));
        writer.u64(self.generation);
        writer.finish()?;
        Ok(content_id(SERIES_LIFECYCLE_ADMISSION_DOMAIN_V3, &body))
    }
}

/// Exact Funding/Clock evidence for one lapsed ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleLapseProjectionV3 {
    /// Immutable aggregate binding.
    pub binding_id: SeriesLifecycleReplayBindingV3Id,
    /// Exact Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Sequential ordinal.
    pub ordinal: u32,
    /// Exact FundingV5 account.
    pub funding_account_id: ContentId,
    /// FundingV5 semantic prestate.
    pub funding_state_before_id: ContentId,
    /// FundingV5 semantic poststate.
    pub funding_state_after_id: ContentId,
    /// Exact Clock policy.
    pub clock_policy_id: ContentId,
    /// Private hostile Clock receipt.
    pub clock_receipt_id: ContentId,
    /// Private Product lapse postwrite receipt.
    pub lapse_receipt_id: ContentId,
    /// Current compiler bundle.
    pub compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    /// Authenticated bucket strictly beyond the occurrence.
    pub current_bucket: u64,
}

impl SeriesLifecycleLapseProjectionV3 {
    /// Validate and hash complete lapse evidence.
    pub fn id(self) -> Result<ContentId> {
        self.binding_id.validate()?;
        self.series_plan_id.validate()?;
        self.compiler_bundle_id.validate()?;
        for id in [
            self.funding_account_id,
            self.funding_state_before_id,
            self.funding_state_after_id,
            self.clock_policy_id,
            self.clock_receipt_id,
            self.lapse_receipt_id,
        ] {
            id.validate()?;
        }
        if self.funding_state_before_id == self.funding_state_after_id {
            return Err(Error::WorkStateMismatch);
        }
        let mut body = [0u8; 300];
        let mut writer = Writer::new(&mut body, 300)?;
        writer.id(self.binding_id.content_id());
        writer.id(self.series_plan_id.content_id());
        writer.u32(self.ordinal);
        writer.id(self.funding_account_id);
        writer.id(self.funding_state_before_id);
        writer.id(self.funding_state_after_id);
        writer.id(self.clock_policy_id);
        writer.id(self.clock_receipt_id);
        writer.id(self.lapse_receipt_id);
        writer.id(self.compiler_bundle_id.content_id());
        writer.u64(self.current_bucket);
        writer.finish()?;
        Ok(content_id(SERIES_LIFECYCLE_LAPSE_DOMAIN_V3, &body))
    }
}

/// Exact Product root/link postwrite evidence for one retired physical link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleLinkRetirementProjectionV3 {
    /// Immutable aggregate binding.
    pub binding_id: SeriesLifecycleReplayBindingV3Id,
    /// Exact Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Retired ordinal.
    pub ordinal: u32,
    /// Physical closed `0xad` account.
    pub link_account_id: ContentId,
    /// Physical still-live shared `0xaa` account.
    pub market_root_account_id: ContentId,
    /// Economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Complete Product retirement postwrite facts ID.
    pub product_retirement_facts_id: ContentId,
    /// Exact pure retiring-link projection consumed by the root.
    pub link_retirement_projection_id: ContentId,
    /// Original root admission receipt persisted by the link.
    pub market_admission_receipt_id: ContentId,
    /// Nonzero Market generation.
    pub generation: u64,
}

impl SeriesLifecycleLinkRetirementProjectionV3 {
    /// Validate and hash complete link retirement evidence.
    pub fn id(self) -> Result<ContentId> {
        self.binding_id.validate()?;
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()?;
        if self.generation == 0 {
            return Err(Error::InvalidParameter);
        }
        for id in [
            self.link_account_id,
            self.market_root_account_id,
            self.product_retirement_facts_id,
            self.link_retirement_projection_id,
            self.market_admission_receipt_id,
        ] {
            id.validate()?;
        }
        let mut body = [0u8; 268];
        let mut writer = Writer::new(&mut body, 268)?;
        writer.id(self.binding_id.content_id());
        writer.id(self.series_plan_id.content_id());
        writer.u32(self.ordinal);
        writer.id(self.link_account_id);
        writer.id(self.market_root_account_id);
        writer.id(self.market_instance_id.content_id());
        writer.id(self.product_retirement_facts_id);
        writer.id(self.link_retirement_projection_id);
        writer.id(self.market_admission_receipt_id);
        writer.u64(self.generation);
        writer.finish()?;
        Ok(content_id(
            SERIES_LIFECYCLE_LINK_RETIREMENT_DOMAIN_V3,
            &body,
        ))
    }
}

/// Exact current Funding/Registry evidence needed to seal Series terminality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleTerminalEvidenceV3 {
    /// Immutable aggregate binding.
    pub binding_id: SeriesLifecycleReplayBindingV3Id,
    /// Exact FundingV5 account.
    pub funding_account_id: ContentId,
    /// Exact hostile-authenticated Closed FundingV5 semantic state.
    pub funding_state_id: ContentId,
    /// Exact FundingV5 terminal principal/donation projection.
    pub funding_terminal_projection_id: ContentId,
    /// Permanent RegistryV4 account.
    pub registry_account_id: ContentId,
    /// Full current RegistryV4 authentication receipt.
    pub registry_authentication_id: ContentId,
    /// Private same-instruction terminal authority receipt.
    pub terminal_authority_receipt_id: ContentId,
}

impl SeriesLifecycleTerminalEvidenceV3 {
    fn validate(self, binding: SeriesLifecycleReplayBindingV3) -> Result<()> {
        self.binding_id.validate()?;
        for id in [
            self.funding_account_id,
            self.funding_state_id,
            self.funding_terminal_projection_id,
            self.registry_account_id,
            self.registry_authentication_id,
            self.terminal_authority_receipt_id,
        ] {
            id.validate()?;
        }
        if self.binding_id != binding.id()?
            || self.funding_account_id != binding.funding_account_id
            || self.registry_account_id != binding.registry_account_id
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }
}

/// Complete counted per-Series replay owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleReplayV3 {
    binding: SeriesLifecycleReplayBindingV3,
    phase: SeriesLifecycleReplayPhaseV3,
    transition_sequence: u64,
    processed_ordinals: u32,
    admitted_links: u32,
    live_links: u32,
    retired_links: u32,
    lapsed_ordinals: u32,
    transition_transcript_id: ContentId,
    terminal_projection_id: ContentId,
}

impl SeriesLifecycleReplayV3 {
    /// Initialize the sole empty replay owner at FundingV5 activation.
    pub fn initialize(binding: SeriesLifecycleReplayBindingV3) -> Result<Self> {
        binding.validate()?;
        let value = Self {
            binding,
            phase: SeriesLifecycleReplayPhaseV3::Open,
            transition_sequence: 0,
            processed_ordinals: 0,
            admitted_links: 0,
            live_links: 0,
            retired_links: 0,
            lapsed_ordinals: 0,
            transition_transcript_id: ContentId::ZERO,
            terminal_projection_id: ContentId::ZERO,
        };
        value.validate()?;
        Ok(value)
    }

    /// Count one exact completed ordinal and its activated link.
    pub fn record_admission(self, event: SeriesLifecycleAdmissionProjectionV3) -> Result<Self> {
        self.validate()?;
        if self.phase != SeriesLifecycleReplayPhaseV3::Open
            || event.binding_id != self.binding.id()?
            || event.series_plan_id != self.binding.series_plan_id
            || event.funding_account_id != self.binding.funding_account_id
            || event.compiler_bundle_id != self.binding.compiler_bundle_id
            || event.ordinal != self.processed_ordinals
            || event.ordinal >= self.binding.instance_count
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let event_id = event.id()?;
        let sequence = self.next_sequence()?;
        let next = Self {
            transition_sequence: sequence,
            processed_ordinals: self
                .processed_ordinals
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            admitted_links: self
                .admitted_links
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            live_links: self.live_links.checked_add(1).ok_or(Error::ArithmeticOverflow)?,
            transition_transcript_id: roll(self.transition_transcript_id, event_id, sequence),
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Count one exact lapsed ordinal with no link admission.
    pub fn record_lapse(self, event: SeriesLifecycleLapseProjectionV3) -> Result<Self> {
        self.validate()?;
        if self.phase != SeriesLifecycleReplayPhaseV3::Open
            || event.binding_id != self.binding.id()?
            || event.series_plan_id != self.binding.series_plan_id
            || event.funding_account_id != self.binding.funding_account_id
            || event.compiler_bundle_id != self.binding.compiler_bundle_id
            || event.ordinal != self.processed_ordinals
            || event.ordinal >= self.binding.instance_count
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let event_id = event.id()?;
        let sequence = self.next_sequence()?;
        let next = Self {
            transition_sequence: sequence,
            processed_ordinals: self
                .processed_ordinals
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            lapsed_ordinals: self
                .lapsed_ordinals
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            transition_transcript_id: roll(self.transition_transcript_id, event_id, sequence),
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Apply the count transition for one physically closed Product link.
    ///
    /// Fixed counts cannot prove ordinal membership or physical uniqueness.
    /// The sole SBF caller must consume the still-live canonical ordinal link,
    /// close it in the same atomic operation, and bind this event to the
    /// private close postwrite. Replaying an event into this pure method alone
    /// is not terminal authority.
    pub fn record_link_retirement(
        self,
        event: SeriesLifecycleLinkRetirementProjectionV3,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != SeriesLifecycleReplayPhaseV3::Open
            || event.binding_id != self.binding.id()?
            || event.series_plan_id != self.binding.series_plan_id
            || event.ordinal >= self.processed_ordinals
            || self.live_links == 0
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let event_id = event.id()?;
        let sequence = self.next_sequence()?;
        let next = Self {
            transition_sequence: sequence,
            live_links: self.live_links.checked_sub(1).ok_or(Error::ArithmeticOverflow)?,
            retired_links: self
                .retired_links
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            transition_transcript_id: roll(self.transition_transcript_id, event_id, sequence),
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Seal exhaustive Series terminality from the current Closed FundingV5.
    pub fn terminalize(
        self,
        evidence: SeriesLifecycleTerminalEvidenceV3,
    ) -> Result<(Self, SeriesLifecycleTerminalProjectionV3)> {
        self.validate()?;
        evidence.validate(self.binding)?;
        if self.phase != SeriesLifecycleReplayPhaseV3::Open
            || self.processed_ordinals != self.binding.instance_count
            || self.live_links != 0
            || self.retired_links != self.admitted_links
            || self
                .admitted_links
                .checked_add(self.lapsed_ordinals)
                .ok_or(Error::ArithmeticOverflow)?
                != self.binding.instance_count
        {
            return Err(Error::WorkIncomplete);
        }
        let pre_state_id = self.id()?;
        let sequence = self.next_sequence()?;
        let projection = SeriesLifecycleTerminalProjectionV3::derive(
            self.binding,
            pre_state_id,
            sequence,
            self.processed_ordinals,
            self.admitted_links,
            self.retired_links,
            self.lapsed_ordinals,
            self.transition_transcript_id,
            evidence,
        )?;
        let terminal_transcript_id = roll(
            self.transition_transcript_id,
            projection.id.content_id(),
            sequence,
        );
        let next = Self {
            phase: SeriesLifecycleReplayPhaseV3::Terminal,
            transition_sequence: sequence,
            transition_transcript_id: terminal_transcript_id,
            terminal_projection_id: projection.id.content_id(),
            ..self
        };
        next.validate()?;
        Ok((next, projection))
    }

    /// Immutable binding.
    pub const fn binding(self) -> SeriesLifecycleReplayBindingV3 {
        self.binding
    }
    /// Current phase.
    pub const fn phase(self) -> SeriesLifecycleReplayPhaseV3 {
        self.phase
    }
    /// Monotone transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }
    /// Ordinals already completed or lapsed.
    pub const fn processed_ordinals(self) -> u32 {
        self.processed_ordinals
    }
    /// Links ever admitted.
    pub const fn admitted_links(self) -> u32 {
        self.admitted_links
    }
    /// Admitted links not yet retired.
    pub const fn live_links(self) -> u32 {
        self.live_links
    }
    /// Admitted links physically retired.
    pub const fn retired_links(self) -> u32 {
        self.retired_links
    }
    /// Ordinals lapsed without a link.
    pub const fn lapsed_ordinals(self) -> u32 {
        self.lapsed_ordinals
    }
    /// Hash chain of every counted transition.
    pub const fn transition_transcript_id(self) -> ContentId {
        self.transition_transcript_id
    }
    /// Nonzero only after exhaustive terminal sealing.
    pub const fn terminal_projection_id(self) -> ContentId {
        self.terminal_projection_id
    }

    /// Complete semantic state identity.
    pub fn id(self) -> Result<SeriesLifecycleReplayV3Id> {
        let mut body = [0u8; SERIES_LIFECYCLE_REPLAY_BYTES_V3];
        self.encode_into(&mut body)?;
        Ok(SeriesLifecycleReplayV3Id::from_bytes(
            content_id(SERIES_LIFECYCLE_REPLAY_DOMAIN_V3, &body).bytes(),
        ))
    }

    fn next_sequence(self) -> Result<u64> {
        self.transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)
    }

    fn validate(self) -> Result<()> {
        self.binding.validate()?;
        if self.processed_ordinals > self.binding.instance_count
            || self
                .admitted_links
                .checked_add(self.lapsed_ordinals)
                .ok_or(Error::ArithmeticOverflow)?
                != self.processed_ordinals
            || self
                .live_links
                .checked_add(self.retired_links)
                .ok_or(Error::ArithmeticOverflow)?
                != self.admitted_links
            || self.transition_sequence
                != u64::from(self.processed_ordinals)
                    .checked_add(u64::from(self.retired_links))
                    .and_then(|value| {
                        value.checked_add(if self.phase == SeriesLifecycleReplayPhaseV3::Terminal {
                            1
                        } else {
                            0
                        })
                    })
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::WorkStateMismatch);
        }
        if (self.transition_sequence == 0) != self.transition_transcript_id.is_zero() {
            return Err(Error::WorkStateMismatch);
        }
        match self.phase {
            SeriesLifecycleReplayPhaseV3::Open => {
                if self.terminal_projection_id != ContentId::ZERO {
                    return Err(Error::WorkStateMismatch);
                }
            }
            SeriesLifecycleReplayPhaseV3::Terminal => {
                self.terminal_projection_id.validate()?;
                if self.processed_ordinals != self.binding.instance_count
                    || self.live_links != 0
                    || self.retired_links != self.admitted_links
                {
                    return Err(Error::WorkIncomplete);
                }
            }
        }
        Ok(())
    }
}

impl FixedCodec for SeriesLifecycleReplayV3 {
    const ENCODED_LEN: usize = SERIES_LIFECYCLE_REPLAY_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut binding = [0u8; SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3];
        self.binding.encode_into(&mut binding)?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_LIFECYCLE_REPLAY_MAGIC_V3);
        writer.u16(SERIES_LIFECYCLE_REPLAY_SCHEMA_V3);
        writer.bytes(&binding);
        writer.u8(self.phase.byte());
        writer.u64(self.transition_sequence);
        writer.u32(self.processed_ordinals);
        writer.u32(self.admitted_links);
        writer.u32(self.live_links);
        writer.u32(self.retired_links);
        writer.u32(self.lapsed_ordinals);
        writer.id(self.transition_transcript_id);
        writer.id(self.terminal_projection_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SERIES_LIFECYCLE_REPLAY_MAGIC_V3)?;
        if reader.u16() != SERIES_LIFECYCLE_REPLAY_SCHEMA_V3 {
            return Err(Error::BadVersion);
        }
        let binding_bytes = reader.bytes::<SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3>();
        let value = Self {
            binding: SeriesLifecycleReplayBindingV3::decode(&binding_bytes)?,
            phase: SeriesLifecycleReplayPhaseV3::decode(reader.u8())?,
            transition_sequence: reader.u64(),
            processed_ordinals: reader.u32(),
            admitted_links: reader.u32(),
            live_links: reader.u32(),
            retired_links: reader.u32(),
            lapsed_ordinals: reader.u32(),
            transition_transcript_id: reader.id(),
            terminal_projection_id: reader.id(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Pure exhaustive terminal evidence.
///
/// Physical FundingV5 close must consume a private adapter receipt minted only
/// after this projection and the exact Terminal replay successor are written
/// and hostile-reopened. This value alone authorizes no account mutation or
/// value disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleTerminalProjectionV3 {
    id: SeriesLifecycleTerminalProjectionV3Id,
    binding_id: SeriesLifecycleReplayBindingV3Id,
    series_plan_id: SeriesPlanV5Id,
    lifecycle_replay_account_id: ContentId,
    funding_account_id: ContentId,
    funding_state_id: ContentId,
    funding_terminal_projection_id: ContentId,
    registry_account_id: ContentId,
    registry_authentication_id: ContentId,
    terminal_authority_receipt_id: ContentId,
    pre_terminal_state_id: SeriesLifecycleReplayV3Id,
    pre_terminal_transcript_id: ContentId,
    terminal_transcript_id: ContentId,
    terminal_transition_sequence: u64,
    processed_ordinals: u32,
    admitted_links: u32,
    retired_links: u32,
    lapsed_ordinals: u32,
}

impl SeriesLifecycleTerminalProjectionV3 {
    #[allow(clippy::too_many_arguments)]
    fn derive(
        binding: SeriesLifecycleReplayBindingV3,
        pre_terminal_state_id: SeriesLifecycleReplayV3Id,
        terminal_transition_sequence: u64,
        processed_ordinals: u32,
        admitted_links: u32,
        retired_links: u32,
        lapsed_ordinals: u32,
        transition_transcript_id: ContentId,
        evidence: SeriesLifecycleTerminalEvidenceV3,
    ) -> Result<Self> {
        let binding_id = binding.id()?;
        let mut body = [0u8; 376];
        let mut writer = Writer::new(&mut body, 376)?;
        writer.id(binding_id.content_id());
        writer.id(binding.series_plan_id.content_id());
        writer.id(binding.lifecycle_replay_account_id);
        writer.id(evidence.funding_account_id);
        writer.id(evidence.funding_state_id);
        writer.id(evidence.funding_terminal_projection_id);
        writer.id(evidence.registry_account_id);
        writer.id(evidence.registry_authentication_id);
        writer.id(evidence.terminal_authority_receipt_id);
        writer.id(pre_terminal_state_id.content_id());
        writer.id(transition_transcript_id);
        writer.u64(terminal_transition_sequence);
        writer.u32(processed_ordinals);
        writer.u32(admitted_links);
        writer.u32(retired_links);
        writer.u32(lapsed_ordinals);
        writer.finish()?;
        let id = SeriesLifecycleTerminalProjectionV3Id::from_bytes(
            content_id(SERIES_LIFECYCLE_TERMINAL_PROJECTION_DOMAIN_V3, &body).bytes(),
        );
        id.validate()?;
        let terminal_transcript_id = roll(
            transition_transcript_id,
            id.content_id(),
            terminal_transition_sequence,
        );
        Ok(Self {
            id,
            binding_id,
            series_plan_id: binding.series_plan_id,
            lifecycle_replay_account_id: binding.lifecycle_replay_account_id,
            funding_account_id: evidence.funding_account_id,
            funding_state_id: evidence.funding_state_id,
            funding_terminal_projection_id: evidence.funding_terminal_projection_id,
            registry_account_id: evidence.registry_account_id,
            registry_authentication_id: evidence.registry_authentication_id,
            terminal_authority_receipt_id: evidence.terminal_authority_receipt_id,
            pre_terminal_state_id,
            pre_terminal_transcript_id: transition_transcript_id,
            terminal_transcript_id,
            terminal_transition_sequence,
            processed_ordinals,
            admitted_links,
            retired_links,
            lapsed_ordinals,
        })
    }

    /// Projection identity.
    pub const fn id(self) -> SeriesLifecycleTerminalProjectionV3Id {
        self.id
    }
    /// Immutable binding.
    pub const fn binding_id(self) -> SeriesLifecycleReplayBindingV3Id {
        self.binding_id
    }
    /// Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    /// Permanent lifecycle replay account.
    pub const fn lifecycle_replay_account_id(self) -> ContentId {
        self.lifecycle_replay_account_id
    }
    /// FundingV5 account.
    pub const fn funding_account_id(self) -> ContentId {
        self.funding_account_id
    }
    /// Closed FundingV5 state.
    pub const fn funding_state_id(self) -> ContentId {
        self.funding_state_id
    }
    /// Exact Funding terminal projection.
    pub const fn funding_terminal_projection_id(self) -> ContentId {
        self.funding_terminal_projection_id
    }
    /// Permanent RegistryV4 account.
    pub const fn registry_account_id(self) -> ContentId {
        self.registry_account_id
    }
    /// Exact Registry authentication at terminal sealing.
    pub const fn registry_authentication_id(self) -> ContentId {
        self.registry_authentication_id
    }
    /// Private terminal authority.
    pub const fn terminal_authority_receipt_id(self) -> ContentId {
        self.terminal_authority_receipt_id
    }
    /// Open pre-terminal replay state.
    pub const fn pre_terminal_state_id(self) -> SeriesLifecycleReplayV3Id {
        self.pre_terminal_state_id
    }
    /// Transcript immediately before terminal sealing.
    pub const fn pre_terminal_transcript_id(self) -> ContentId {
        self.pre_terminal_transcript_id
    }
    /// Complete transcript including terminal sealing.
    pub const fn terminal_transcript_id(self) -> ContentId {
        self.terminal_transcript_id
    }
    /// Final sequence.
    pub const fn terminal_transition_sequence(self) -> u64 {
        self.terminal_transition_sequence
    }
    /// Exhaustive ordinal count.
    pub const fn processed_ordinals(self) -> u32 {
        self.processed_ordinals
    }
    /// Total admitted links.
    pub const fn admitted_links(self) -> u32 {
        self.admitted_links
    }
    /// Total retired links; equal to admitted.
    pub const fn retired_links(self) -> u32 {
        self.retired_links
    }
    /// Total lapsed ordinals.
    pub const fn lapsed_ordinals(self) -> u32 {
        self.lapsed_ordinals
    }
}

fn disposition_byte(value: SeriesMarketDispositionV1) -> u8 {
    match value {
        SeriesMarketDispositionV1::Founder => 1,
        SeriesMarketDispositionV1::Converger => 2,
    }
}

fn roll(previous: ContentId, event: ContentId, sequence: u64) -> ContentId {
    let mut body = [0u8; 72];
    body[..32].copy_from_slice(&previous.bytes());
    body[32..64].copy_from_slice(&event.bytes());
    body[64..72].copy_from_slice(&sequence.to_le_bytes());
    content_id(SERIES_LIFECYCLE_TRANSCRIPT_DOMAIN_V3, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn binding() -> SeriesLifecycleReplayBindingV3 {
        SeriesLifecycleReplayBindingV3 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; 32]),
            funding_quote_id: SeriesFundingQuoteV6Id::from_bytes([3; 32]),
            attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes([4; 32]),
            compiler_bundle_id: CompiledProductSeriesBundleV7Id::from_bytes([5; 32]),
            registry_release_id: RegistryProgramReleaseV2Id::from_bytes([6; 32]),
            capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes([7; 32]),
            registry_account_id: id(8),
            funding_account_id: id(9),
            lifecycle_replay_account_id: id(10),
            permanent_rent_funder: id(11),
            neutral_lamport_sink: id(12),
            instance_count: 3,
        }
    }

    fn admission(ordinal: u32, byte: u8) -> SeriesLifecycleAdmissionProjectionV3 {
        let binding = binding();
        SeriesLifecycleAdmissionProjectionV3 {
            binding_id: binding.id().unwrap(),
            series_plan_id: binding.series_plan_id,
            ordinal,
            funding_account_id: binding.funding_account_id,
            funding_state_before_id: id(byte),
            funding_state_after_id: id(byte + 1),
            occurrence_completion_receipt_id: id(byte + 2),
            link_account_id: id(byte + 3),
            link_activation_receipt_id: id(byte + 4),
            market_admission_receipt_id: id(byte + 5),
            market_instance_id: MarketInstanceV2Id::from_bytes([byte + 6; 32]),
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([byte + 7; 32]),
            compiler_bundle_id: binding.compiler_bundle_id,
            disposition: if ordinal == 0 {
                SeriesMarketDispositionV1::Founder
            } else {
                SeriesMarketDispositionV1::Converger
            },
            generation: u64::from(byte) + 1,
        }
    }

    fn lapse(ordinal: u32) -> SeriesLifecycleLapseProjectionV3 {
        let binding = binding();
        SeriesLifecycleLapseProjectionV3 {
            binding_id: binding.id().unwrap(),
            series_plan_id: binding.series_plan_id,
            ordinal,
            funding_account_id: binding.funding_account_id,
            funding_state_before_id: id(40),
            funding_state_after_id: id(41),
            clock_policy_id: id(42),
            clock_receipt_id: id(43),
            lapse_receipt_id: id(44),
            compiler_bundle_id: binding.compiler_bundle_id,
            current_bucket: 45,
        }
    }

    fn retirement(ordinal: u32, byte: u8) -> SeriesLifecycleLinkRetirementProjectionV3 {
        let binding = binding();
        SeriesLifecycleLinkRetirementProjectionV3 {
            binding_id: binding.id().unwrap(),
            series_plan_id: binding.series_plan_id,
            ordinal,
            link_account_id: id(byte),
            market_root_account_id: id(byte + 1),
            market_instance_id: MarketInstanceV2Id::from_bytes([byte + 2; 32]),
            product_retirement_facts_id: id(byte + 3),
            link_retirement_projection_id: id(byte + 4),
            market_admission_receipt_id: id(byte + 5),
            generation: u64::from(byte) + 1,
        }
    }

    #[test]
    fn counted_partition_seals_only_after_all_ordinals_and_links() {
        let state = SeriesLifecycleReplayV3::initialize(binding()).unwrap();
        let state = state.record_admission(admission(0, 20)).unwrap();
        let state = state.record_lapse(lapse(1)).unwrap();
        let state = state.record_admission(admission(2, 50)).unwrap();
        assert_eq!(state.processed_ordinals(), 3);
        assert_eq!(state.admitted_links(), 2);
        assert_eq!(state.lapsed_ordinals(), 1);
        assert!(state
            .terminalize(SeriesLifecycleTerminalEvidenceV3 {
                binding_id: binding().id().unwrap(),
                funding_account_id: binding().funding_account_id,
                funding_state_id: id(70),
                funding_terminal_projection_id: id(71),
                registry_account_id: binding().registry_account_id,
                registry_authentication_id: id(72),
                terminal_authority_receipt_id: id(73),
            })
            .is_err());
        let state = state.record_link_retirement(retirement(2, 80)).unwrap();
        let state = state.record_link_retirement(retirement(0, 90)).unwrap();
        let (terminal, projection) = state
            .terminalize(SeriesLifecycleTerminalEvidenceV3 {
                binding_id: binding().id().unwrap(),
                funding_account_id: binding().funding_account_id,
                funding_state_id: id(70),
                funding_terminal_projection_id: id(71),
                registry_account_id: binding().registry_account_id,
                registry_authentication_id: id(72),
                terminal_authority_receipt_id: id(73),
            })
            .unwrap();
        assert_eq!(terminal.phase(), SeriesLifecycleReplayPhaseV3::Terminal);
        assert_eq!(projection.admitted_links(), projection.retired_links());
        assert_eq!(projection.processed_ordinals(), 3);
    }

    #[test]
    fn ordinal_replay_splice_and_count_corruption_refuse() {
        let state = SeriesLifecycleReplayV3::initialize(binding()).unwrap();
        assert!(state.record_admission(admission(1, 20)).is_err());
        assert!(state.record_lapse(lapse(1)).is_err());
        let admitted = state.record_admission(admission(0, 20)).unwrap();
        let mut wrong = admission(1, 50);
        wrong.series_plan_id = SeriesPlanV5Id::from_bytes([100; 32]);
        assert!(admitted.record_admission(wrong).is_err());
        assert!(admitted.record_link_retirement(retirement(1, 80)).is_err());

        let mut bytes = [0u8; SERIES_LIFECYCLE_REPLAY_BYTES_V3];
        admitted.encode_into(&mut bytes).unwrap();
        bytes[417..421].copy_from_slice(&7u32.to_le_bytes());
        assert!(SeriesLifecycleReplayV3::decode(&bytes).is_err());
    }

    #[test]
    fn codecs_are_exact_and_binding_aliases_refuse() {
        let binding = binding();
        let mut binding_bytes = [0u8; SERIES_LIFECYCLE_REPLAY_BINDING_BYTES_V3];
        binding.encode_into(&mut binding_bytes).unwrap();
        assert_eq!(SeriesLifecycleReplayBindingV3::decode(&binding_bytes), Ok(binding));
        assert!(SeriesLifecycleReplayBindingV3::decode(&binding_bytes[..397]).is_err());

        let mut aliased = binding;
        aliased.neutral_lamport_sink = aliased.permanent_rent_funder;
        assert!(aliased.validate().is_err());

        let state = SeriesLifecycleReplayV3::initialize(binding).unwrap();
        let mut bytes = [0u8; SERIES_LIFECYCLE_REPLAY_BYTES_V3];
        state.encode_into(&mut bytes).unwrap();
        assert_eq!(SeriesLifecycleReplayV3::decode(&bytes), Ok(state));
        let mut trailing = [0u8; SERIES_LIFECYCLE_REPLAY_BYTES_V3 + 1];
        trailing[..SERIES_LIFECYCLE_REPLAY_BYTES_V3].copy_from_slice(&bytes);
        assert!(SeriesLifecycleReplayV3::decode(&trailing).is_err());
    }
}
