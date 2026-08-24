//! Current six-compartment recurring-Series funding state.
//!
//! This is a fresh owner for QuoteV5/AttachmentV5/BundleV6. Historical V1-V3
//! bytes are never decoded as this state. Pending commits an acyclic pre-Source
//! reservation binding, never the final LinkV2 semantic ID: Source must first
//! authenticate the persisted Pending poststate, and only then may Product
//! construct the Source-bound final link and complete the reservation.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV6Id, ComponentDebitV1, ContentId, Error, FixedCodec,
    MarketInstanceV2Id, Result, SeriesAttachmentPlanV5, SeriesAttachmentPlanV5Id,
    SeriesFundingComponentV2, SeriesFundingQuoteV5, SeriesFundingQuoteV5Id,
    SeriesFundingAbortBindingV4Id, SeriesFundingCompletionBindingV4Id,
    SeriesFundingReservationBindingV4Id,
    SeriesFundingStateV4Id, SeriesFundingTermsV2Id, SeriesMarketDispositionV1, SeriesPlanV5,
    SeriesPlanV5Id, SourceOccurrenceV1Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};

const MAGIC_V4: [u8; 8] = *b"DCSFSTV4";
const SCHEMA_V4: u16 = 4;

/// Semantic identity domain of the exact current state.
pub const SERIES_FUNDING_STATE_V4_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-state/v4";
/// Semantic identity of the exact current terminal principal/donation view.
pub const SERIES_FUNDING_TERMINAL_PROJECTION_V4_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-terminal-projection/v4";
/// Acyclic Active-prestate reservation identity.
pub const SERIES_FUNDING_RESERVATION_BINDING_V4_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-reservation-binding/v4";
/// Final Source/Root/Link/replay completion identity.
pub const SERIES_FUNDING_COMPLETION_BINDING_V4_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-completion-binding/v4";
/// Exact inert/retired Source proof required before a Pending debit is restored.
pub const SERIES_FUNDING_ABORT_BINDING_V4_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-abort-binding/v4";
/// Exact bytes per separately accounted component.
pub const SERIES_COMPONENT_CAPITAL_BYTES_V4: usize = 40;
/// Exact current state width with no unnamed authority-bearing padding.
pub const SERIES_FUNDING_STATE_BYTES_V4: usize = 16
    + 5 * 32
    + 3 * 4
    + 8
    + 3 * 32
    + 4
    + 32
    + 32
    + 8
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * 16
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * SERIES_COMPONENT_CAPITAL_BYTES_V4;

const _: () = assert!(SERIES_FUNDING_STATE_BYTES_V4 == 704);

/// Exhaustive successor funding phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFundingPhaseV4 {
    /// No occurrence reservation is outstanding and ordinals remain.
    Active,
    /// Exactly `next_ordinal` has been debited and is awaiting atomic admission.
    Pending,
    /// Every ordinal was either admitted or lapsed and no reservation remains.
    Closed,
}

impl SeriesFundingPhaseV4 {
    const fn byte(self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Pending => 2,
            Self::Closed => 3,
        }
    }

    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Active),
            2 => Ok(Self::Pending),
            3 => Ok(Self::Closed),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Principal, donation, and exact-unit consumption for one compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesComponentCapitalV4 {
    /// Payer-owned principal not yet committed to an admitted ordinal.
    pub remaining_principal: ComponentDebitV1,
    /// Unowned unsolicited balance surplus, never usable as principal.
    pub donations: ComponentDebitV1,
    /// Number of exact quote units already reserved or admitted.
    pub consumed_allocations: u32,
}

impl SeriesComponentCapitalV4 {
    /// Canonical zero component.
    pub const ZERO: Self = Self {
        remaining_principal: ComponentDebitV1::ZERO,
        donations: ComponentDebitV1::ZERO,
        consumed_allocations: 0,
    };
}

/// Canonical acyclic reservation facts selected before Source capitalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFundingReservationBindingV4 {
    /// Exact writable FundingV4 account coordinate.
    pub funding_account_id: ContentId,
    /// Hostile authentication of that account in Active prestate.
    pub funding_account_authentication_before_id: ContentId,
    /// Typed semantic identity of its Active prestate.
    pub funding_state_before_id: SeriesFundingStateV4Id,
    /// Exact finite Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable principal/refund/donation owner.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Current 47-slot funding quote.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Current QuoteV5-bound attachment.
    pub attachment_plan_id: SeriesAttachmentPlanV5Id,
    /// Current compiler/Source/capability bundle.
    pub compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    /// Exact next ordinal selected from the Active state.
    pub ordinal: u32,
    /// Deterministic compiled Market identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Deterministic compiled Source occurrence identity.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Founder/converger debit partition.
    pub disposition: SeriesMarketDispositionV1,
    /// Exact debit vector reserved from the six component vaults.
    pub debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Final RootV2 immutable binding derivable before Source publication.
    pub market_binding_id: ContentId,
    /// Canonical RootV2 PDA coordinate.
    pub market_root_account_id: ContentId,
    /// Canonical future LinkV2 PDA coordinate.
    pub series_market_link_account_id: ContentId,
    /// Acyclic Product preauthorization consumed by Source and the founder.
    pub product_founder_preauthorization_id: ContentId,
    /// Exact pre-root 0xba capitalization receipt.
    pub direct_global_liveness_capitalization_id: ContentId,
    /// Deterministic Source publication projection, not a postwrite receipt.
    pub source_publication_id: ContentId,
    /// Exact authenticated route Clock policy.
    pub clock_policy_id: ContentId,
    /// Exact adapter-authenticated Clock receipt.
    pub clock_receipt_id: ContentId,
    /// Funding mutation sequence observed before reserve.
    pub funding_transition_sequence_before: u64,
    /// Bank slot retained by the Clock receipt.
    pub clock_slot: u64,
    /// Nonnegative Unix timestamp retained by the Clock receipt.
    pub clock_unix_timestamp: u64,
    /// Canonical ClockPolicy bucket used for creation eligibility.
    pub clock_bucket: u64,
}

impl SeriesFundingReservationBindingV4 {
    /// Hash every exact reservation fact under the fresh V4 domain.
    pub fn id(&self) -> Result<SeriesFundingReservationBindingV4Id> {
        self.funding_state_before_id.validate()?;
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        self.attachment_plan_id.validate()?;
        self.compiler_bundle_id.validate()?;
        self.market_instance_id.validate()?;
        self.source_occurrence_id.validate()?;
        for id in [
            self.funding_account_id,
            self.funding_account_authentication_before_id,
            self.market_binding_id,
            self.market_root_account_id,
            self.series_market_link_account_id,
            self.product_founder_preauthorization_id,
            self.direct_global_liveness_capitalization_id,
            self.source_publication_id,
            self.clock_policy_id,
            self.clock_receipt_id,
        ] {
            id.validate()?;
        }
        let mut body = [0u8; 712];
        let mut writer = Writer::new(&mut body, 712)?;
        for id in [
            self.funding_account_id,
            self.funding_account_authentication_before_id,
            self.funding_state_before_id.content_id(),
            self.series_plan_id.content_id(),
            self.funding_terms_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.compiler_bundle_id.content_id(),
            self.market_instance_id.content_id(),
            self.source_occurrence_id.content_id(),
            self.market_binding_id,
            self.market_root_account_id,
            self.series_market_link_account_id,
            self.product_founder_preauthorization_id,
            self.direct_global_liveness_capitalization_id,
            self.source_publication_id,
            self.clock_policy_id,
            self.clock_receipt_id,
        ] {
            writer.id(id);
        }
        writer.u32(self.ordinal);
        writer.u8(disposition_byte(self.disposition));
        writer.reserved(3);
        writer.u64(self.funding_transition_sequence_before);
        writer.u64(self.clock_slot);
        writer.u64(self.clock_unix_timestamp);
        writer.u64(self.clock_bucket);
        for debit in self.debits {
            writer.u64(debit.lamports);
            writer.u64(debit.collateral_atoms);
        }
        writer.finish()?;
        Ok(SeriesFundingReservationBindingV4Id::from_bytes(
            content_id(SERIES_FUNDING_RESERVATION_BINDING_V4_DOMAIN, &body).bytes(),
        ))
    }
}

/// Final atomic Source/Root/Link/replay evidence consumed before Pending clears.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFundingCompletionBindingV4 {
    /// Exact reservation persisted in Pending.
    pub reservation_binding_id: SeriesFundingReservationBindingV4Id,
    /// Exact writable FundingV4 account coordinate.
    pub funding_account_id: ContentId,
    /// Hostile authentication of FundingV4 Pending poststate.
    pub funding_account_authentication_pending_id: ContentId,
    /// Typed semantic identity of the exact Pending poststate.
    pub funding_pending_state_id: SeriesFundingStateV4Id,
    /// Final live Source custody capitalization receipt.
    pub source_capitalization_receipt_id: ContentId,
    /// Final non-Copy pre-root Source publication receipt.
    pub pre_root_source_occurrence_id: ContentId,
    /// Canonical RootV2 account.
    pub market_root_account_id: ContentId,
    /// Exact immutable RootV2 binding.
    pub market_binding_id: ContentId,
    /// RootV2 semantic before the final activation step.
    pub root_semantic_before_id: ContentId,
    /// RootV2 semantic after the final activation step.
    pub root_semantic_after_id: ContentId,
    /// Canonical LinkV2 account.
    pub series_market_link_account_id: ContentId,
    /// LinkV2 semantic before activation.
    pub link_semantic_before_id: ContentId,
    /// LinkV2 semantic after activation.
    pub link_semantic_after_id: ContentId,
    /// Exact Product market-admission receipt.
    pub market_admission_receipt_id: ContentId,
    /// Exact Product link-activation receipt.
    pub link_activation_receipt_id: ContentId,
    /// Canonical permanent SeriesLifecycleReplayV2 account.
    pub lifecycle_replay_account_id: ContentId,
    /// Replay semantic before admission.
    pub lifecycle_replay_state_before_id: ContentId,
    /// Replay semantic after admission.
    pub lifecycle_replay_state_after_id: ContentId,
    /// Hostile replay authentication before admission.
    pub lifecycle_replay_authentication_before_id: ContentId,
    /// Hostile replay authentication after admission.
    pub lifecycle_replay_authentication_after_id: ContentId,
    /// Exact replay admission projection consumed by its writer.
    pub lifecycle_replay_admission_projection_id: ContentId,
    /// Exact accepted MarketCore/Source/Root/Link composite receipt.
    pub accepted_market_core_receipt_id: ContentId,
}

impl SeriesFundingCompletionBindingV4 {
    /// Hash the whole final Source/Root/Link/replay join.
    pub fn id(&self) -> Result<SeriesFundingCompletionBindingV4Id> {
        self.reservation_binding_id.validate()?;
        self.funding_pending_state_id.validate()?;
        let ids = [
            self.funding_account_id,
            self.funding_account_authentication_pending_id,
            self.source_capitalization_receipt_id,
            self.pre_root_source_occurrence_id,
            self.market_root_account_id,
            self.market_binding_id,
            self.root_semantic_before_id,
            self.root_semantic_after_id,
            self.series_market_link_account_id,
            self.link_semantic_before_id,
            self.link_semantic_after_id,
            self.market_admission_receipt_id,
            self.link_activation_receipt_id,
            self.lifecycle_replay_account_id,
            self.lifecycle_replay_state_before_id,
            self.lifecycle_replay_state_after_id,
            self.lifecycle_replay_authentication_before_id,
            self.lifecycle_replay_authentication_after_id,
            self.lifecycle_replay_admission_projection_id,
            self.accepted_market_core_receipt_id,
        ];
        for id in ids { id.validate()?; }
        if self.root_semantic_before_id == self.root_semantic_after_id
            || self.link_semantic_before_id == self.link_semantic_after_id
        {
            return Err(Error::WorkStateMismatch);
        }
        let mut body = [0u8; 704];
        let mut writer = Writer::new(&mut body, 704)?;
        writer.id(self.reservation_binding_id.content_id());
        writer.id(self.funding_pending_state_id.content_id());
        for id in ids { writer.id(id); }
        writer.finish()?;
        Ok(SeriesFundingCompletionBindingV4Id::from_bytes(
            content_id(SERIES_FUNDING_COMPLETION_BINDING_V4_DOMAIN, &body).bytes(),
        ))
    }
}

/// Exhaustive physical Source state admitted by an abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFundingAbortDispositionV4 {
    /// No Source capitalization/publication account was ever created.
    SourceNeverPublished,
    /// The exact published Source lifecycle was terminalized and retired first.
    SourceRetired,
}

impl SeriesFundingAbortDispositionV4 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::SourceNeverPublished => 1,
            Self::SourceRetired => 2,
        }
    }
}

/// Exact proof consumed before restoring one pending principal vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFundingAbortBindingV4 {
    /// Exact reservation persisted in Pending.
    pub reservation_binding_id: SeriesFundingReservationBindingV4Id,
    /// Exact writable FundingV4 account coordinate.
    pub funding_account_id: ContentId,
    /// Hostile authentication of the FundingV4 Pending poststate.
    pub funding_account_authentication_pending_id: ContentId,
    /// Typed semantic identity of that Pending poststate.
    pub funding_pending_state_id: SeriesFundingStateV4Id,
    /// Exhaustive Source-side state accepted by this abort.
    pub disposition: SeriesFundingAbortDispositionV4,
    /// Canonical Source occurrence account coordinate, absent or retired.
    pub source_occurrence_account_id: ContentId,
    /// Canonical Source custody account coordinate, absent or retired.
    pub source_custody_account_id: ContentId,
    /// Zero for absence; exact physical Source disposition for retirement.
    pub source_physical_disposition_receipt_id: ContentId,
    /// Zero for absence; exact Source custody retirement receipt otherwise.
    pub source_retirement_receipt_id: ContentId,
}

impl SeriesFundingAbortBindingV4 {
    /// Hash the exhaustive Source absence/retirement proof.
    pub fn id(&self) -> Result<SeriesFundingAbortBindingV4Id> {
        self.reservation_binding_id.validate()?;
        self.funding_pending_state_id.validate()?;
        for id in [
            self.funding_account_id,
            self.funding_account_authentication_pending_id,
            self.source_occurrence_account_id,
            self.source_custody_account_id,
        ] {
            id.validate()?;
        }
        let absence = self.disposition == SeriesFundingAbortDispositionV4::SourceNeverPublished;
        if absence {
            if !self.source_physical_disposition_receipt_id.is_zero()
                || !self.source_retirement_receipt_id.is_zero()
            {
                return Err(Error::NonCanonicalPadding);
            }
        } else {
            self.source_physical_disposition_receipt_id.validate()?;
            self.source_retirement_receipt_id.validate()?;
        }
        let mut body = [0u8; 260];
        let mut writer = Writer::new(&mut body, 260)?;
        writer.id(self.reservation_binding_id.content_id());
        writer.id(self.funding_account_id);
        writer.id(self.funding_account_authentication_pending_id);
        writer.id(self.funding_pending_state_id.content_id());
        writer.u8(self.disposition.wire_byte());
        writer.reserved(3);
        writer.id(self.source_occurrence_account_id);
        writer.id(self.source_custody_account_id);
        writer.id(self.source_physical_disposition_receipt_id);
        writer.id(self.source_retirement_receipt_id);
        writer.finish()?;
        Ok(SeriesFundingAbortBindingV4Id::from_bytes(
            content_id(SERIES_FUNDING_ABORT_BINDING_V4_DOMAIN, &body).bytes(),
        ))
    }
}

/// Private adapter authority for current Series funding transitions.
///
/// SBF implementations must derive each success from authenticated accounts;
/// caller-shaped facts must not implement this trait in value-bearing code.
pub trait AuthenticatedSeriesFundingAuthorityV4 {
    /// Authenticate the exact initial bodies and physical deposits.
    fn authenticate_activation(
        &self,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV6Id,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<()>;

    /// Current authenticated Clock bucket for lapse-only transitions.
    fn current_bucket(&self, series: &SeriesPlanV5) -> Result<u64>;

    /// Authenticate one exact reservation and its linked Source/Market facts.
    fn authenticate_reservation(
        &self,
        state: &SeriesFundingStateV4,
        binding: &SeriesFundingReservationBindingV4,
        reservation_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate completion of the exact pending link/founding transition.
    fn authenticate_pending_completion(
        &self,
        state: &SeriesFundingStateV4,
        binding: &SeriesFundingCompletionBindingV4,
        completion_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate an abort which returned every pending principal unit.
    fn authenticate_pending_abort(
        &self,
        state: &SeriesFundingStateV4,
        binding: &SeriesFundingAbortBindingV4,
        abort_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate physical surplus for exactly one component.
    fn authenticate_donation(
        &self,
        state: &SeriesFundingStateV4,
        component: SeriesFundingComponentV2,
        amount: ComponentDebitV1,
    ) -> Result<()>;

    /// Authenticate all terminal custody poststates and destinations.
    fn authenticate_close(
        &self,
        state: &SeriesFundingStateV4,
        terminal_receipt_id: ContentId,
    ) -> Result<()>;
}

/// Exact current mutable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingStateV4 {
    /// Registered finite Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable refund/sink/collateral ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Current 47-slot quote.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Current QuoteV5-bound attachment.
    pub attachment_plan_id: SeriesAttachmentPlanV5Id,
    /// Exact compiler/Source/capability graph retained by SeriesRegistry V4.
    pub compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    /// Frozen finite ordinal count.
    pub instance_count: u32,
    /// Only ordinal which may be reserved or lapsed next.
    pub next_ordinal: u32,
    /// Number of cursor advances which spent no principal.
    pub lapsed_count: u32,
    /// Monotone mutation sequence.
    pub transition_sequence: u64,
    /// Explicit exhaustive lifecycle phase.
    pub phase: SeriesFundingPhaseV4,
    /// Founder/converger classification of the pending ordinal.
    pub pending_disposition: Option<SeriesMarketDispositionV1>,
    /// Exact pending Market, or zero outside Pending.
    pub pending_market_instance_id: ContentId,
    /// Exact pending compiled Source occurrence, or zero outside Pending.
    pub pending_source_occurrence_id: ContentId,
    /// Exact pending 0xad semantic state, or zero outside Pending.
    pub pending_pre_source_reservation_binding_id: ContentId,
    /// Exact pending ordinal; zero outside Pending.
    pub pending_ordinal: u32,
    /// Private adapter receipt which authorized the debit.
    pub pending_reservation_receipt_id: ContentId,
    /// Exact authenticated Clock receipt which authorized this reservation.
    pub pending_clock_receipt_id: ContentId,
    /// Canonical eligible bucket derived by that Clock receipt.
    pub pending_clock_bucket: u64,
    /// Exact component debits held by the pending transition.
    pub pending_debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Six disjoint principal/donation ledgers.
    pub components: [SeriesComponentCapitalV4; SERIES_FUNDING_COMPONENT_COUNT_V2],
}

impl SeriesFundingStateV4 {
    /// Activate current state from exact V5/V6 artifacts and physical deposits.
    #[allow(clippy::too_many_arguments)]
    pub fn activate<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        authority: &A,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV6Id,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<Self> {
        series.validate_shape()?;
        funding_terms_id.validate()?;
        compiler_bundle_id.validate()?;
        quote.validate()?;
        attachment.validate()?;
        if attachment.funding_quote_id != quote.id()? {
            return Err(Error::MismatchedArtifact);
        }
        let mut components = [SeriesComponentCapitalV4::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let expected = multiply_debit(quote.components[index], series.instance_count)?;
            if principal[index] != expected {
                return Err(Error::InsufficientPrepayment);
            }
            components[index] = SeriesComponentCapitalV4 {
                remaining_principal: principal[index],
                donations: donations[index],
                consumed_allocations: 0,
            };
            index += 1;
        }
        authority.authenticate_activation(
            series,
            funding_terms_id,
            compiler_bundle_id,
            quote,
            attachment,
            &principal,
            &donations,
        )?;
        let value = Self {
            series_plan_id: series.id()?,
            funding_terms_id,
            funding_quote_id: quote.id()?,
            attachment_plan_id: attachment.id()?,
            compiler_bundle_id,
            instance_count: series.instance_count,
            next_ordinal: 0,
            lapsed_count: 0,
            transition_sequence: 0,
            phase: SeriesFundingPhaseV4::Active,
            pending_disposition: None,
            pending_market_instance_id: ContentId::ZERO,
            pending_source_occurrence_id: ContentId::ZERO,
            pending_pre_source_reservation_binding_id: ContentId::ZERO,
            pending_ordinal: 0,
            pending_reservation_receipt_id: ContentId::ZERO,
            pending_clock_receipt_id: ContentId::ZERO,
            pending_clock_bucket: 0,
            pending_debits: [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
            components,
        };
        value.validate_against(series, quote, attachment)?;
        Ok(value)
    }

    /// Reserve exactly the next eligible created ordinal.
    #[allow(clippy::too_many_arguments)]
    pub fn reserve_created<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        binding: &SeriesFundingReservationBindingV4,
        reservation_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV4::Active || self.next_ordinal >= self.instance_count {
            return Err(Error::SeriesNotActive);
        }
        reservation_receipt_id.validate()?;
        let ordinal = self.next_ordinal;
        let state_before_id = self.id()?;
        if binding.funding_state_before_id != state_before_id
            || binding.series_plan_id != self.series_plan_id
            || binding.funding_terms_id != self.funding_terms_id
            || binding.funding_quote_id != self.funding_quote_id
            || binding.attachment_plan_id != self.attachment_plan_id
            || binding.compiler_bundle_id != self.compiler_bundle_id
            || binding.ordinal != ordinal
            || binding.funding_transition_sequence_before != self.transition_sequence
        {
            return Err(Error::MismatchedArtifact);
        }
        binding.id()?;
        if !series.is_creation_eligible(ordinal, binding.clock_bucket)? {
            return Err(Error::OutsideCreationWindow);
        }
        validate_reservation_debits(quote, binding.disposition, &binding.debits)?;
        authority.authenticate_reservation(self, binding, reservation_receipt_id)?;
        let mut next = *self;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            next.components[index].remaining_principal =
                subtract_debit(next.components[index].remaining_principal, binding.debits[index])?;
            if binding.debits[index] != ComponentDebitV1::ZERO {
                next.components[index].consumed_allocations = next.components[index]
                    .consumed_allocations
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.phase = SeriesFundingPhaseV4::Pending;
        next.pending_disposition = Some(binding.disposition);
        next.pending_market_instance_id = binding.market_instance_id.content_id();
        next.pending_source_occurrence_id = binding.source_occurrence_id.content_id();
        next.pending_pre_source_reservation_binding_id = binding.id()?.content_id();
        next.pending_ordinal = ordinal;
        next.pending_reservation_receipt_id = reservation_receipt_id;
        next.pending_clock_receipt_id = binding.clock_receipt_id;
        next.pending_clock_bucket = binding.clock_bucket;
        next.pending_debits = binding.debits;
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Commit the exact pending admission and advance the cursor once.
    pub fn complete_pending<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        binding: &SeriesFundingCompletionBindingV4,
        completion_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV4::Pending {
            return Err(Error::WorkStateMismatch);
        }
        completion_receipt_id.validate()?;
        if binding.reservation_binding_id.content_id()
            != self.pending_pre_source_reservation_binding_id
            || binding.funding_pending_state_id != self.id()?
            || binding.market_root_account_id.is_zero()
            || binding.series_market_link_account_id.is_zero()
        {
            return Err(Error::MismatchedArtifact);
        }
        binding.id()?;
        authority.authenticate_pending_completion(self, binding, completion_receipt_id)?;
        let ordinal = self.pending_ordinal;
        let mut next = *self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.clear_pending();
        next.phase = if next.next_ordinal == next.instance_count {
            SeriesFundingPhaseV4::Closed
        } else {
            SeriesFundingPhaseV4::Active
        };
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Abort inert founding after an authenticated reverse-close and restore
    /// only the exact pending principal. Donations never enter this equation.
    pub fn abort_pending<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        binding: &SeriesFundingAbortBindingV4,
        abort_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV4::Pending {
            return Err(Error::WorkStateMismatch);
        }
        abort_receipt_id.validate()?;
        if binding.reservation_binding_id.content_id()
            != self.pending_pre_source_reservation_binding_id
            || binding.funding_pending_state_id != self.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
        binding.id()?;
        authority.authenticate_pending_abort(self, binding, abort_receipt_id)?;
        let ordinal = self.pending_ordinal;
        let mut next = *self;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            next.components[index].remaining_principal = add_debit(
                next.components[index].remaining_principal,
                next.pending_debits[index],
            )?;
            if next.pending_debits[index] != ComponentDebitV1::ZERO {
                next.components[index].consumed_allocations = next.components[index]
                    .consumed_allocations
                    .checked_sub(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.clear_pending();
        next.phase = SeriesFundingPhaseV4::Active;
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Advance one elapsed ordinal without spending or reserving principal.
    pub fn lapse<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV4::Active || self.next_ordinal >= self.instance_count {
            return Err(Error::SeriesNotActive);
        }
        let ordinal = self.next_ordinal;
        if authority.current_bucket(series)? < series.start_bucket(ordinal)? {
            return Err(Error::OutsideCreationWindow);
        }
        let mut next = *self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.lapsed_count = next
            .lapsed_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.phase = if next.next_ordinal == next.instance_count {
            SeriesFundingPhaseV4::Closed
        } else {
            SeriesFundingPhaseV4::Active
        };
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Record authenticated physical surplus without changing principal.
    pub fn add_donation<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        component: SeriesFundingComponentV2,
        amount: ComponentDebitV1,
    ) -> Result<()> {
        self.validate_against(series, quote, attachment)?;
        if self.phase == SeriesFundingPhaseV4::Pending {
            return Err(Error::WorkStateMismatch);
        }
        if amount == ComponentDebitV1::ZERO {
            return Err(Error::InvalidParameter);
        }
        authority.authenticate_donation(self, component, amount)?;
        let mut next = *self;
        let slot = &mut next.components[component.index()];
        slot.donations = add_debit(slot.donations, amount)?;
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(())
    }

    /// Mint the sole terminal disposition after authenticated custody closure.
    pub fn close<A: AuthenticatedSeriesFundingAuthorityV4 + ?Sized>(
        &self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        terminal_receipt_id: ContentId,
    ) -> Result<SeriesFundingTerminalProjectionV4> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV4::Closed {
            return Err(Error::SeriesNotClosed);
        }
        terminal_receipt_id.validate()?;
        authority.authenticate_close(self, terminal_receipt_id)?;
        let mut principal = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut donations = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            principal[index] = self.components[index].remaining_principal;
            donations[index] = self.components[index].donations;
            index += 1;
        }
        Ok(SeriesFundingTerminalProjectionV4 {
            series_plan_id: self.series_plan_id,
            funding_terms_id: self.funding_terms_id,
            compiler_bundle_id: self.compiler_bundle_id,
            transition_sequence: self.transition_sequence,
            refundable_principal: principal,
            donation_residue: donations,
            terminal_receipt_id,
        })
    }

    /// Validate structural equations against the exact current artifacts.
    pub fn validate_against(
        &self,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
    ) -> Result<()> {
        self.validate()?;
        series.validate_shape()?;
        quote.validate()?;
        attachment.validate()?;
        if series.id()? != self.series_plan_id
            || series.instance_count != self.instance_count
            || quote.id()? != self.funding_quote_id
            || attachment.id()? != self.attachment_plan_id
            || series.attachment_plan_id.content_id() != attachment.id()?.content_id()
            || attachment.funding_quote_id != self.funding_quote_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let admitted = self.admitted_created_count()?;
        let minimum_sequence = u64::from(self.next_ordinal)
            .checked_add(u64::from(admitted))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.transition_sequence < minimum_sequence {
            return Err(Error::InvalidSchedule);
        }
        if self.phase == SeriesFundingPhaseV4::Pending {
            validate_reservation_debits(
                quote,
                self.pending_disposition.ok_or(Error::InvalidComponentStatus)?,
                &self.pending_debits,
            )?;
        }
        let completed_created = self
            .next_ordinal
            .checked_sub(self.lapsed_count)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.components[SeriesFundingComponentV2::MarketCore.index()].consumed_allocations
            != self.components[SeriesFundingComponentV2::RecoveryReserve.index()]
                .consumed_allocations
        {
            return Err(Error::InvalidComponentStatus);
        }
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let unit = quote.components[index];
            let consumed = self.components[index].consumed_allocations;
            if consumed > admitted {
                return Err(Error::InvalidComponentStatus);
            }
            let pending_delta = if self.phase == SeriesFundingPhaseV4::Pending
                && self.pending_debits[index] != ComponentDebitV1::ZERO
            {
                1
            } else {
                0
            };
            let prior_consumed = consumed
                .checked_sub(pending_delta)
                .ok_or(Error::InvalidComponentStatus)?;
            if prior_consumed > completed_created {
                return Err(Error::InvalidComponentStatus);
            }
            if index == SeriesFundingComponentV2::SeriesAdmission.index()
                && consumed != admitted
            {
                return Err(Error::InvalidComponentStatus);
            }
            let initial = multiply_debit(unit, self.instance_count)?;
            let spent = multiply_debit(unit, consumed)?;
            if add_debit(self.components[index].remaining_principal, spent)? != initial {
                return Err(Error::InvalidComponentStatus);
            }
            index += 1;
        }
        Ok(())
    }

    /// Typed semantic identity of the complete state.
    pub fn id(&self) -> Result<SeriesFundingStateV4Id> {
        let mut body = [0u8; SERIES_FUNDING_STATE_BYTES_V4];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingStateV4Id::from_bytes(
            content_id(SERIES_FUNDING_STATE_V4_DOMAIN, &body).bytes(),
        ))
    }

    fn validate(&self) -> Result<()> {
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        self.attachment_plan_id.validate()?;
        self.compiler_bundle_id.validate()?;
        if self.instance_count == 0
            || self.next_ordinal > self.instance_count
            || self.lapsed_count > self.next_ordinal
        {
            return Err(Error::InvalidSchedule);
        }
        let pending = self.phase == SeriesFundingPhaseV4::Pending;
        if pending {
            if self.next_ordinal >= self.instance_count
                || self.pending_ordinal != self.next_ordinal
                || self.pending_disposition.is_none()
            {
                return Err(Error::InvalidComponentStatus);
            }
            for id in [
                self.pending_market_instance_id,
                self.pending_source_occurrence_id,
                self.pending_pre_source_reservation_binding_id,
                self.pending_reservation_receipt_id,
                self.pending_clock_receipt_id,
            ] {
                id.validate()?;
            }
        } else if self.pending_disposition.is_some()
            || !self.pending_market_instance_id.is_zero()
            || !self.pending_source_occurrence_id.is_zero()
            || !self.pending_pre_source_reservation_binding_id.is_zero()
            || self.pending_ordinal != 0
            || !self.pending_reservation_receipt_id.is_zero()
            || !self.pending_clock_receipt_id.is_zero()
            || self.pending_clock_bucket != 0
            || self
                .pending_debits
                .iter()
                .any(|debit| *debit != ComponentDebitV1::ZERO)
        {
            return Err(Error::NonCanonicalPadding);
        }
        match self.phase {
            SeriesFundingPhaseV4::Active if self.next_ordinal < self.instance_count => {}
            SeriesFundingPhaseV4::Pending if self.next_ordinal < self.instance_count => {}
            SeriesFundingPhaseV4::Closed if self.next_ordinal == self.instance_count => {}
            _ => return Err(Error::InvalidSchedule),
        }
        Ok(())
    }

    fn admitted_created_count(&self) -> Result<u32> {
        self.next_ordinal
            .checked_sub(self.lapsed_count)
            .and_then(|created| {
                created.checked_add(if self.phase == SeriesFundingPhaseV4::Pending {
                    1
                } else {
                    0
                })
            })
            .ok_or(Error::ArithmeticOverflow)
    }

    fn clear_pending(&mut self) {
        self.pending_disposition = None;
        self.pending_market_instance_id = ContentId::ZERO;
        self.pending_source_occurrence_id = ContentId::ZERO;
        self.pending_pre_source_reservation_binding_id = ContentId::ZERO;
        self.pending_ordinal = 0;
        self.pending_reservation_receipt_id = ContentId::ZERO;
        self.pending_clock_receipt_id = ContentId::ZERO;
        self.pending_clock_bucket = 0;
        self.pending_debits = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    }
}

impl FixedCodec for SeriesFundingStateV4 {
    const ENCODED_LEN: usize = SERIES_FUNDING_STATE_BYTES_V4;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC_V4);
        writer.u16(SCHEMA_V4);
        writer.u8(self.phase.byte());
        writer.u8(match self.pending_disposition {
            None => 0,
            Some(SeriesMarketDispositionV1::Founder) => 1,
            Some(SeriesMarketDispositionV1::Converger) => 2,
        });
        writer.reserved(4);
        for id in [
            self.series_plan_id.content_id(),
            self.funding_terms_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.compiler_bundle_id.content_id(),
        ] {
            writer.id(id);
        }
        writer.u32(self.instance_count);
        writer.u32(self.next_ordinal);
        writer.u32(self.lapsed_count);
        writer.u64(self.transition_sequence);
        writer.id(self.pending_market_instance_id);
        writer.id(self.pending_source_occurrence_id);
        writer.id(self.pending_pre_source_reservation_binding_id);
        writer.u32(self.pending_ordinal);
        writer.id(self.pending_reservation_receipt_id);
        writer.id(self.pending_clock_receipt_id);
        writer.u64(self.pending_clock_bucket);
        for debit in self.pending_debits {
            writer.u64(debit.lamports);
            writer.u64(debit.collateral_atoms);
        }
        for component in self.components {
            writer.u64(component.remaining_principal.lamports);
            writer.u64(component.remaining_principal.collateral_atoms);
            writer.u64(component.donations.lamports);
            writer.u64(component.donations.collateral_atoms);
            writer.u32(component.consumed_allocations);
            writer.reserved(4);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MAGIC_V4)?;
        if reader.u16() != SCHEMA_V4 {
            return Err(Error::BadVersion);
        }
        let phase = SeriesFundingPhaseV4::decode(reader.u8())?;
        let pending_disposition = match reader.u8() {
            0 => None,
            1 => Some(SeriesMarketDispositionV1::Founder),
            2 => Some(SeriesMarketDispositionV1::Converger),
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(4)?;
        let series_plan_id = SeriesPlanV5Id::from_bytes(reader.id().bytes());
        let funding_terms_id = SeriesFundingTermsV2Id::from_bytes(reader.id().bytes());
        let funding_quote_id = SeriesFundingQuoteV5Id::from_bytes(reader.id().bytes());
        let attachment_plan_id = SeriesAttachmentPlanV5Id::from_bytes(reader.id().bytes());
        let compiler_bundle_id = CompiledProductSeriesBundleV6Id::from_bytes(reader.id().bytes());
        let instance_count = reader.u32();
        let next_ordinal = reader.u32();
        let lapsed_count = reader.u32();
        let transition_sequence = reader.u64();
        let pending_market_instance_id = reader.id();
        let pending_source_occurrence_id = reader.id();
        let pending_pre_source_reservation_binding_id = reader.id();
        let pending_ordinal = reader.u32();
        let pending_reservation_receipt_id = reader.id();
        let pending_clock_receipt_id = reader.id();
        let pending_clock_bucket = reader.u64();
        let mut pending_debits = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for debit in &mut pending_debits {
            debit.lamports = reader.u64();
            debit.collateral_atoms = reader.u64();
        }
        let mut components = [SeriesComponentCapitalV4::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in &mut components {
            component.remaining_principal.lamports = reader.u64();
            component.remaining_principal.collateral_atoms = reader.u64();
            component.donations.lamports = reader.u64();
            component.donations.collateral_atoms = reader.u64();
            component.consumed_allocations = reader.u32();
            reader.reserved(4)?;
        }
        reader.finish()?;
        let value = Self {
            series_plan_id,
            funding_terms_id,
            funding_quote_id,
            attachment_plan_id,
            compiler_bundle_id,
            instance_count,
            next_ordinal,
            lapsed_count,
            transition_sequence,
            phase,
            pending_disposition,
            pending_market_instance_id,
            pending_source_occurrence_id,
            pending_pre_source_reservation_binding_id,
            pending_ordinal,
            pending_reservation_receipt_id,
            pending_clock_receipt_id,
            pending_clock_bucket,
            pending_debits,
            components,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Terminal principal/donation projection. FundingTerms owns destinations;
/// this projection owns only exact component amounts and retained authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingTerminalProjectionV4 {
    /// Exact closed Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact immutable destination owner.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact current compiler graph.
    pub compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    /// Last committed mutable sequence.
    pub transition_sequence: u64,
    /// Remaining payer principal by V2 component order.
    pub refundable_principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Donation residue by V2 component order.
    pub donation_residue: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Private terminal postwrite authorization.
    pub terminal_receipt_id: ContentId,
}

impl SeriesFundingTerminalProjectionV4 {
    /// Hash the complete current terminal projection under its sole pure owner.
    pub fn id(self) -> Result<ContentId> {
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()?;
        self.compiler_bundle_id.validate()?;
        self.terminal_receipt_id.validate()?;
        let mut body = [0u8; 328];
        let mut writer = Writer::new(&mut body, 328)?;
        writer.id(self.series_plan_id.content_id());
        writer.id(self.funding_terms_id.content_id());
        writer.id(self.compiler_bundle_id.content_id());
        writer.u64(self.transition_sequence);
        for component in self.refundable_principal {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        for component in self.donation_residue {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        writer.id(self.terminal_receipt_id);
        writer.finish()?;
        Ok(content_id(
            SERIES_FUNDING_TERMINAL_PROJECTION_V4_DOMAIN,
            &body,
        ))
    }
}

fn validate_reservation_debits(
    quote: &SeriesFundingQuoteV5,
    disposition: SeriesMarketDispositionV1,
    debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Result<()> {
    let admission = SeriesFundingComponentV2::SeriesAdmission.index();
    let market = SeriesFundingComponentV2::MarketCore.index();
    let recovery = SeriesFundingComponentV2::RecoveryReserve.index();
    if debits[admission] != quote.components[admission] {
        return Err(Error::InvalidComponentStatus);
    }
    match disposition {
        SeriesMarketDispositionV1::Founder
            if debits[market] == quote.components[market]
                && debits[recovery] == quote.components[recovery] => {}
        SeriesMarketDispositionV1::Converger
            if debits[market] == ComponentDebitV1::ZERO
                && debits[recovery] == ComponentDebitV1::ZERO => {}
        _ => return Err(Error::InvalidComponentStatus),
    }
    let mut index = 0usize;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        if index != admission
            && index != market
            && index != recovery
            && debits[index] != ComponentDebitV1::ZERO
            && debits[index] != quote.components[index]
        {
            return Err(Error::InvalidComponentStatus);
        }
        index += 1;
    }
    Ok(())
}

const fn disposition_byte(disposition: SeriesMarketDispositionV1) -> u8 {
    match disposition {
        SeriesMarketDispositionV1::Founder => 1,
        SeriesMarketDispositionV1::Converger => 2,
    }
}

fn multiply_debit(value: ComponentDebitV1, count: u32) -> Result<ComponentDebitV1> {
    Ok(ComponentDebitV1 {
        lamports: value
            .lamports
            .checked_mul(u64::from(count))
            .ok_or(Error::ArithmeticOverflow)?,
        collateral_atoms: value
            .collateral_atoms
            .checked_mul(u64::from(count))
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

fn add_debit(left: ComponentDebitV1, right: ComponentDebitV1) -> Result<ComponentDebitV1> {
    Ok(ComponentDebitV1 {
        lamports: left
            .lamports
            .checked_add(right.lamports)
            .ok_or(Error::ArithmeticOverflow)?,
        collateral_atoms: left
            .collateral_atoms
            .checked_add(right.collateral_atoms)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

fn subtract_debit(left: ComponentDebitV1, right: ComponentDebitV1) -> Result<ComponentDebitV1> {
    Ok(ComponentDebitV1 {
        lamports: left
            .lamports
            .checked_sub(right.lamports)
            .ok_or(Error::InsufficientPrepayment)?,
        collateral_atoms: left
            .collateral_atoms
            .checked_sub(right.collateral_atoms)
            .ok_or(Error::InsufficientPrepayment)?,
    })
}

fn increment(value: u64) -> Result<u64> {
    value.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DonationOnlyAuthority;

    impl AuthenticatedSeriesFundingAuthorityV4 for DonationOnlyAuthority {
        fn authenticate_activation(
            &self,
            _series: &SeriesPlanV5,
            _funding_terms_id: SeriesFundingTermsV2Id,
            _compiler_bundle_id: CompiledProductSeriesBundleV6Id,
            _quote: &SeriesFundingQuoteV5,
            _attachment: &SeriesAttachmentPlanV5,
            _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
            _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        ) -> Result<()> {
            Err(Error::UnauthenticatedAuthority)
        }

        fn current_bucket(&self, _series: &SeriesPlanV5) -> Result<u64> {
            Err(Error::UnauthenticatedAuthority)
        }

        fn authenticate_reservation(
            &self,
            _state: &SeriesFundingStateV4,
            _binding: &SeriesFundingReservationBindingV4,
            _reservation_receipt_id: ContentId,
        ) -> Result<()> {
            Err(Error::UnauthenticatedAuthority)
        }

        fn authenticate_pending_completion(
            &self,
            _state: &SeriesFundingStateV4,
            _binding: &SeriesFundingCompletionBindingV4,
            _completion_receipt_id: ContentId,
        ) -> Result<()> {
            Err(Error::UnauthenticatedAuthority)
        }

        fn authenticate_pending_abort(
            &self,
            _state: &SeriesFundingStateV4,
            _binding: &SeriesFundingAbortBindingV4,
            _abort_receipt_id: ContentId,
        ) -> Result<()> {
            Err(Error::UnauthenticatedAuthority)
        }

        fn authenticate_donation(
            &self,
            _state: &SeriesFundingStateV4,
            _component: SeriesFundingComponentV2,
            _amount: ComponentDebitV1,
        ) -> Result<()> {
            Ok(())
        }

        fn authenticate_close(
            &self,
            _state: &SeriesFundingStateV4,
            _terminal_receipt_id: ContentId,
        ) -> Result<()> {
            Err(Error::UnauthenticatedAuthority)
        }
    }

    fn active_state() -> SeriesFundingStateV4 {
        SeriesFundingStateV4 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; 32]),
            funding_quote_id: SeriesFundingQuoteV5Id::from_bytes([3; 32]),
            attachment_plan_id: SeriesAttachmentPlanV5Id::from_bytes([4; 32]),
            compiler_bundle_id: CompiledProductSeriesBundleV6Id::from_bytes([5; 32]),
            instance_count: 1,
            next_ordinal: 0,
            lapsed_count: 0,
            transition_sequence: 0,
            phase: SeriesFundingPhaseV4::Active,
            pending_disposition: None,
            pending_market_instance_id: ContentId::ZERO,
            pending_source_occurrence_id: ContentId::ZERO,
            pending_pre_source_reservation_binding_id: ContentId::ZERO,
            pending_ordinal: 0,
            pending_reservation_receipt_id: ContentId::ZERO,
            pending_clock_receipt_id: ContentId::ZERO,
            pending_clock_bucket: 0,
            pending_debits: [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
            components: [SeriesComponentCapitalV4::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
        }
    }

    fn joined_fixtures() -> (SeriesPlanV5, SeriesFundingQuoteV5, SeriesAttachmentPlanV5) {
        let mut slot_principal_lamports = [0u64; crate::MARKET_FOUNDATION_SLOT_COUNT_V3];
        for principal in &mut slot_principal_lamports
            [..crate::MARKET_FOUNDATION_CORE_SLOT_COUNT_V3 + 2]
        {
            *principal = 10;
        }
        let custody_start = crate::MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
            + crate::MARKET_FOUNDATION_MAX_OUTCOMES_V3;
        for principal in &mut slot_principal_lamports[custody_start..custody_start + 2] {
            *principal = 10;
        }
        let foundation = crate::MarketFoundationScheduleV3 {
            outcome_count: 2,
            slot_principal_lamports,
            founding_timeout_buckets: 40,
        };
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        components[SeriesFundingComponentV2::MarketCore.index()].lamports =
            foundation.total_principal_lamports().unwrap();
        components[SeriesFundingComponentV2::SeriesAdmission.index()].lamports = 20;
        components[SeriesFundingComponentV2::RecoveryReserve.index()].lamports = 31;
        components[SeriesFundingComponentV2::SourceWork.index()].lamports = 7;
        let quote = SeriesFundingQuoteV5 {
            evidence_only_recovery_policy_id: ContentId::from_bytes([11; 32]),
            failure_liveness_policy_id: ContentId::from_bytes([12; 32]),
            failure_recovery_quote_schedule_id: ContentId::from_bytes([13; 32]),
            components,
            foundation,
            recovery_rent_principal_lamports: 10,
        };
        let attachment = SeriesAttachmentPlanV5 {
            funding_quote_id: quote.id().unwrap(),
            liquidity_facility_plan_id: ContentId::from_bytes([14; 32]),
            wrapper_recipe_set_id: ContentId::from_bytes([15; 32]),
        };
        let series = SeriesPlanV5 {
            product_template_id: crate::ProductTemplateId::from_bytes([16; 32]),
            market_genesis_profile_id: crate::MarketGenesisProfileV2Id::from_bytes([17; 32]),
            attachment_plan_id: crate::SeriesAttachmentPlanId::from_bytes(
                attachment.id().unwrap().bytes(),
            ),
            first_start_bucket: 10,
            stride_buckets: 5,
            instance_count: 2,
            creation_lead_buckets: 1,
            market_collateral_cap: 1,
        };
        (series, quote, attachment)
    }

    fn pending_state() -> (SeriesFundingStateV4, SeriesPlanV5, SeriesFundingQuoteV5, SeriesAttachmentPlanV5) {
        let (series, quote, attachment) = joined_fixtures();
        let mut components = [SeriesComponentCapitalV4::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            components[index].remaining_principal = multiply_debit(quote.components[index], 2).unwrap();
            index += 1;
        }
        let mut pending_debits = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in [
            SeriesFundingComponentV2::MarketCore,
            SeriesFundingComponentV2::SeriesAdmission,
            SeriesFundingComponentV2::RecoveryReserve,
        ] {
            let index = component.index();
            pending_debits[index] = quote.components[index];
            components[index].remaining_principal = subtract_debit(
                components[index].remaining_principal,
                pending_debits[index],
            )
            .unwrap();
            components[index].consumed_allocations = 1;
        }
        let state = SeriesFundingStateV4 {
            series_plan_id: series.id().unwrap(),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([18; 32]),
            funding_quote_id: quote.id().unwrap(),
            attachment_plan_id: attachment.id().unwrap(),
            compiler_bundle_id: CompiledProductSeriesBundleV6Id::from_bytes([19; 32]),
            instance_count: 2,
            next_ordinal: 0,
            lapsed_count: 0,
            transition_sequence: 1,
            phase: SeriesFundingPhaseV4::Pending,
            pending_disposition: Some(SeriesMarketDispositionV1::Founder),
            pending_market_instance_id: ContentId::from_bytes([20; 32]),
            pending_source_occurrence_id: ContentId::from_bytes([21; 32]),
            pending_pre_source_reservation_binding_id: ContentId::from_bytes([22; 32]),
            pending_ordinal: 0,
            pending_reservation_receipt_id: ContentId::from_bytes([23; 32]),
            pending_clock_receipt_id: ContentId::from_bytes([24; 32]),
            pending_clock_bucket: 9,
            pending_debits,
            components,
        };
        (state, series, quote, attachment)
    }

    fn reservation_binding(state: &SeriesFundingStateV4) -> SeriesFundingReservationBindingV4 {
        SeriesFundingReservationBindingV4 {
            funding_account_id: ContentId::from_bytes([30; 32]),
            funding_account_authentication_before_id: ContentId::from_bytes([31; 32]),
            funding_state_before_id: state.id().unwrap(),
            series_plan_id: state.series_plan_id,
            funding_terms_id: state.funding_terms_id,
            funding_quote_id: state.funding_quote_id,
            attachment_plan_id: state.attachment_plan_id,
            compiler_bundle_id: state.compiler_bundle_id,
            ordinal: state.next_ordinal,
            market_instance_id: MarketInstanceV2Id::from_bytes([32; 32]),
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([33; 32]),
            disposition: SeriesMarketDispositionV1::Founder,
            debits: state.pending_debits,
            market_binding_id: ContentId::from_bytes([34; 32]),
            market_root_account_id: ContentId::from_bytes([35; 32]),
            series_market_link_account_id: ContentId::from_bytes([36; 32]),
            product_founder_preauthorization_id: ContentId::from_bytes([37; 32]),
            direct_global_liveness_capitalization_id: ContentId::from_bytes([38; 32]),
            source_publication_id: ContentId::from_bytes([39; 32]),
            clock_policy_id: ContentId::from_bytes([40; 32]),
            clock_receipt_id: ContentId::from_bytes([41; 32]),
            funding_transition_sequence_before: state.transition_sequence,
            clock_slot: 0,
            clock_unix_timestamp: 0,
            clock_bucket: 9,
        }
    }

    #[test]
    fn current_width_is_exact_and_not_the_historical_state_width() {
        assert_eq!(SERIES_FUNDING_STATE_BYTES_V4, 704);
        assert_ne!(
            SERIES_FUNDING_STATE_BYTES_V4,
            crate::SERIES_FUNDING_STATE_BYTES
        );
    }

    #[test]
    fn codec_round_trips_and_refuses_a_caller_shaped_pending_phase() {
        let value = active_state();
        let mut body = [0; SERIES_FUNDING_STATE_BYTES_V4];
        value.encode_into(&mut body).unwrap();
        assert_eq!(SeriesFundingStateV4::decode(&body), Ok(value));
        body[10] = SeriesFundingPhaseV4::Pending.byte();
        body[11] = 1;
        assert_eq!(
            SeriesFundingStateV4::decode(&body),
            Err(Error::ZeroIdentity)
        );
    }

    #[test]
    fn reservation_identity_binds_account_auth_clock_and_future_coordinates() {
        let (state, _, _, _) = pending_state();
        let expected = reservation_binding(&state).id().unwrap();
        let mut changed = reservation_binding(&state);
        changed.funding_account_authentication_before_id = ContentId::from_bytes([51; 32]);
        assert_ne!(changed.id().unwrap(), expected);
        changed = reservation_binding(&state);
        changed.clock_slot = 1;
        assert_ne!(changed.id().unwrap(), expected);
        changed = reservation_binding(&state);
        changed.clock_receipt_id = ContentId::from_bytes([52; 32]);
        assert_ne!(changed.id().unwrap(), expected);
        changed = reservation_binding(&state);
        changed.series_market_link_account_id = ContentId::from_bytes([53; 32]);
        assert_ne!(changed.id().unwrap(), expected);
        changed = reservation_binding(&state);
        changed.source_publication_id = ContentId::from_bytes([54; 32]);
        assert_ne!(changed.id().unwrap(), expected);
    }

    #[test]
    fn abort_partition_requires_zero_absence_receipts_or_both_retirement_receipts() {
        let (state, _, _, _) = pending_state();
        let reservation_binding_id = reservation_binding(&state).id().unwrap();
        let base = SeriesFundingAbortBindingV4 {
            reservation_binding_id,
            funding_account_id: ContentId::from_bytes([60; 32]),
            funding_account_authentication_pending_id: ContentId::from_bytes([61; 32]),
            funding_pending_state_id: state.id().unwrap(),
            disposition: SeriesFundingAbortDispositionV4::SourceNeverPublished,
            source_occurrence_account_id: ContentId::from_bytes([62; 32]),
            source_custody_account_id: ContentId::from_bytes([63; 32]),
            source_physical_disposition_receipt_id: ContentId::ZERO,
            source_retirement_receipt_id: ContentId::ZERO,
        };
        assert!(base.id().is_ok());
        let mut hostile = base.clone();
        hostile.source_retirement_receipt_id = ContentId::from_bytes([64; 32]);
        assert_eq!(hostile.id(), Err(Error::NonCanonicalPadding));
        hostile = base;
        hostile.disposition = SeriesFundingAbortDispositionV4::SourceRetired;
        hostile.source_physical_disposition_receipt_id = ContentId::from_bytes([65; 32]);
        assert_eq!(hostile.id(), Err(Error::ZeroIdentity));
    }

    #[test]
    fn pending_debits_refuse_wrong_disposition_geometry() {
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        components[SeriesFundingComponentV2::MarketCore.index()].lamports = 20;
        components[SeriesFundingComponentV2::SeriesAdmission.index()].lamports = 10;
        components[SeriesFundingComponentV2::RecoveryReserve.index()].lamports = 30;
        components[SeriesFundingComponentV2::SourceWork.index()].lamports = 7;
        let quote = SeriesFundingQuoteV5 {
            evidence_only_recovery_policy_id: ContentId::from_bytes([11; 32]),
            failure_liveness_policy_id: ContentId::from_bytes([12; 32]),
            failure_recovery_quote_schedule_id: ContentId::from_bytes([13; 32]),
            components,
            foundation: crate::MarketFoundationScheduleV3 {
                outcome_count: 2,
                slot_principal_lamports: [0; crate::MARKET_FOUNDATION_SLOT_COUNT_V3],
                founding_timeout_buckets: 1,
            },
            recovery_rent_principal_lamports: 1,
        };
        let mut pending = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        assert_eq!(
            validate_reservation_debits(&quote, SeriesMarketDispositionV1::Founder, &pending),
            Err(Error::InvalidComponentStatus)
        );
        pending[SeriesFundingComponentV2::SeriesAdmission.index()] =
            components[SeriesFundingComponentV2::SeriesAdmission.index()];
        pending[SeriesFundingComponentV2::MarketCore.index()] =
            components[SeriesFundingComponentV2::MarketCore.index()];
        pending[SeriesFundingComponentV2::RecoveryReserve.index()] =
            components[SeriesFundingComponentV2::RecoveryReserve.index()];
        pending[SeriesFundingComponentV2::SourceWork.index()].lamports = 6;
        assert_eq!(
            validate_reservation_debits(&quote, SeriesMarketDispositionV1::Founder, &pending),
            Err(Error::InvalidComponentStatus)
        );
        pending[SeriesFundingComponentV2::SourceWork.index()] = ComponentDebitV1::ZERO;
        assert_eq!(
            validate_reservation_debits(&quote, SeriesMarketDispositionV1::Founder, &pending),
            Ok(())
        );
        pending[SeriesFundingComponentV2::MarketCore.index()] = ComponentDebitV1::ZERO;
        pending[SeriesFundingComponentV2::RecoveryReserve.index()] = ComponentDebitV1::ZERO;
        assert_eq!(
            validate_reservation_debits(&quote, SeriesMarketDispositionV1::Converger, &pending),
            Ok(())
        );
    }

    #[test]
    fn hostile_pending_vectors_rewinds_and_balance_rewrites_refuse() {
        let (state, series, quote, attachment) = pending_state();
        assert_eq!(state.validate_against(&series, &quote, &attachment), Ok(()));

        let mut wrong_vector = state;
        wrong_vector.pending_debits[SeriesFundingComponentV2::SeriesAdmission.index()] =
            ComponentDebitV1::ZERO;
        assert_eq!(
            wrong_vector.validate_against(&series, &quote, &attachment),
            Err(Error::InvalidComponentStatus)
        );

        let mut rewind = state;
        rewind.transition_sequence = 0;
        assert_eq!(
            rewind.validate_against(&series, &quote, &attachment),
            Err(Error::InvalidSchedule)
        );

        let mut balance_rewrite = state;
        balance_rewrite.components[SeriesFundingComponentV2::MarketCore.index()]
            .remaining_principal
            .lamports += 1;
        assert_eq!(
            balance_rewrite.validate_against(&series, &quote, &attachment),
            Err(Error::InvalidComponentStatus)
        );

        let mut missing_pending_count = state;
        missing_pending_count.components[SeriesFundingComponentV2::MarketCore.index()]
            .consumed_allocations = 0;
        missing_pending_count.components[SeriesFundingComponentV2::MarketCore.index()]
            .remaining_principal = multiply_debit(
            quote.components[SeriesFundingComponentV2::MarketCore.index()],
            2,
        )
        .unwrap();
        assert_eq!(
            missing_pending_count.validate_against(&series, &quote, &attachment),
            Err(Error::InvalidComponentStatus)
        );

        let mut prior_count_from_no_pending_debit = state;
        prior_count_from_no_pending_debit.components
            [SeriesFundingComponentV2::SourceWork.index()]
            .consumed_allocations = 1;
        prior_count_from_no_pending_debit.components
            [SeriesFundingComponentV2::SourceWork.index()]
            .remaining_principal = quote.components[SeriesFundingComponentV2::SourceWork.index()];
        assert_eq!(
            prior_count_from_no_pending_debit.validate_against(&series, &quote, &attachment),
            Err(Error::InvalidComponentStatus)
        );
    }

    #[test]
    fn mismatched_series_attachment_refuses_even_when_quote_pair_is_valid() {
        let (state, mut series, quote, attachment) = pending_state();
        series.attachment_plan_id = crate::SeriesAttachmentPlanId::from_bytes([99; 32]);
        assert_eq!(
            state.validate_against(&series, &quote, &attachment),
            Err(Error::MismatchedArtifact)
        );
    }

    #[test]
    fn pending_donation_refuses_before_receipt_authority_is_consulted() {
        let (mut state, series, quote, attachment) = pending_state();
        let before = state;
        assert_eq!(
            state.add_donation(
                &DonationOnlyAuthority,
                &series,
                &quote,
                &attachment,
                SeriesFundingComponentV2::MarketCore,
                ComponentDebitV1 {
                    lamports: 1,
                    collateral_atoms: 0,
                },
            ),
            Err(Error::WorkStateMismatch)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn terminal_projection_identity_binds_every_value_and_authority() {
        let principal = [
            ComponentDebitV1 {
                lamports: 3,
                collateral_atoms: 5,
            };
            SERIES_FUNDING_COMPONENT_COUNT_V2
        ];
        let donations = [
            ComponentDebitV1 {
                lamports: 7,
                collateral_atoms: 11,
            };
            SERIES_FUNDING_COMPONENT_COUNT_V2
        ];
        let projection = SeriesFundingTerminalProjectionV4 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; 32]),
            compiler_bundle_id: CompiledProductSeriesBundleV6Id::from_bytes([3; 32]),
            transition_sequence: 13,
            refundable_principal: principal,
            donation_residue: donations,
            terminal_receipt_id: ContentId::from_bytes([4; 32]),
        };
        let expected = projection.id().unwrap();
        let mut changed = projection;
        changed.transition_sequence += 1;
        assert_ne!(expected, changed.id().unwrap());
        changed = projection;
        changed.refundable_principal[0].lamports += 1;
        assert_ne!(expected, changed.id().unwrap());
        changed = projection;
        changed.donation_residue[SERIES_FUNDING_COMPONENT_COUNT_V2 - 1].collateral_atoms += 1;
        assert_ne!(expected, changed.id().unwrap());
        changed = projection;
        changed.terminal_receipt_id = ContentId::from_bytes([5; 32]);
        assert_ne!(expected, changed.id().unwrap());
        changed = projection;
        changed.terminal_receipt_id = ContentId::ZERO;
        assert_eq!(changed.id(), Err(Error::ZeroIdentity));
    }
}
