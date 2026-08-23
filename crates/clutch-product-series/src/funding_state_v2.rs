//! Current six-compartment recurring-Series funding state.
//!
//! This is a fresh owner for QuoteV4/AttachmentV4/BundleV5. Historical
//! `SeriesFundingStateV1` bytes are never decoded as this state. One pending
//! ordinal is explicit so a founder can enter inert phased Market founding
//! without advancing the Series cursor or permitting a second debit.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV5Id, ComponentDebitV1, ContentId, Error, FixedCodec,
    MarketInstanceV2Id, Result, SeriesAttachmentPlanV4, SeriesAttachmentPlanV4Id,
    SeriesFundingComponentV2, SeriesFundingQuoteV4, SeriesFundingQuoteV4Id,
    SeriesFundingStateV2Id, SeriesFundingTermsV2Id, SeriesMarketDispositionV1, SeriesPlanV5,
    SeriesPlanV5Id, SourceOccurrenceV1Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};

const MAGIC_V2: [u8; 8] = *b"DCSFSTV2";
const SCHEMA_V2: u16 = 2;

/// Semantic identity domain of the exact current state.
pub const SERIES_FUNDING_STATE_V2_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-state/v2";
/// Exact bytes per separately accounted component.
pub const SERIES_COMPONENT_CAPITAL_BYTES_V2: usize = 40;
/// Exact current state width with no unnamed authority-bearing padding.
pub const SERIES_FUNDING_STATE_BYTES_V2: usize = 16
    + 5 * 32
    + 3 * 4
    + 8
    + 3 * 32
    + 4
    + 32
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * 16
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * SERIES_COMPONENT_CAPITAL_BYTES_V2;

const _: () = assert!(SERIES_FUNDING_STATE_BYTES_V2 == 664);

/// Exhaustive successor funding phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFundingPhaseV2 {
    /// No occurrence reservation is outstanding and ordinals remain.
    Active,
    /// Exactly `next_ordinal` has been debited and is awaiting atomic admission.
    Pending,
    /// Every ordinal was either admitted or lapsed and no reservation remains.
    Closed,
}

impl SeriesFundingPhaseV2 {
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
pub struct SeriesComponentCapitalV2 {
    /// Payer-owned principal not yet committed to an admitted ordinal.
    pub remaining_principal: ComponentDebitV1,
    /// Unowned unsolicited balance surplus, never usable as principal.
    pub donations: ComponentDebitV1,
    /// Number of exact quote units already reserved or admitted.
    pub consumed_allocations: u32,
}

impl SeriesComponentCapitalV2 {
    /// Canonical zero component.
    pub const ZERO: Self = Self {
        remaining_principal: ComponentDebitV1::ZERO,
        donations: ComponentDebitV1::ZERO,
        consumed_allocations: 0,
    };
}

/// Private adapter authority for current Series funding transitions.
///
/// SBF implementations must derive each success from authenticated accounts;
/// caller-shaped facts must not implement this trait in value-bearing code.
pub trait AuthenticatedSeriesFundingAuthorityV2 {
    /// Authenticate the exact initial bodies and physical deposits.
    fn authenticate_activation(
        &self,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV5Id,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<()>;

    /// Current authenticated Clock bucket.
    fn current_bucket(&self, series: &SeriesPlanV5) -> Result<u64>;

    /// Authenticate one exact reservation and its linked Source/Market facts.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_reservation(
        &self,
        state: &SeriesFundingStateV2,
        ordinal: u32,
        market_instance_id: MarketInstanceV2Id,
        source_occurrence_id: SourceOccurrenceV1Id,
        series_market_link_id: ContentId,
        disposition: SeriesMarketDispositionV1,
        debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        reservation_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate completion of the exact pending link/founding transition.
    fn authenticate_pending_completion(
        &self,
        state: &SeriesFundingStateV2,
        completion_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate an abort which returned every pending principal unit.
    fn authenticate_pending_abort(
        &self,
        state: &SeriesFundingStateV2,
        abort_receipt_id: ContentId,
    ) -> Result<()>;

    /// Authenticate physical surplus for exactly one component.
    fn authenticate_donation(
        &self,
        state: &SeriesFundingStateV2,
        component: SeriesFundingComponentV2,
        amount: ComponentDebitV1,
    ) -> Result<()>;

    /// Authenticate all terminal custody poststates and destinations.
    fn authenticate_close(
        &self,
        state: &SeriesFundingStateV2,
        terminal_receipt_id: ContentId,
    ) -> Result<()>;
}

/// Exact current mutable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingStateV2 {
    /// Registered finite Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable refund/sink/collateral ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Current 46-slot quote.
    pub funding_quote_id: SeriesFundingQuoteV4Id,
    /// Current QuoteV4-bound attachment.
    pub attachment_plan_id: SeriesAttachmentPlanV4Id,
    /// Exact compiler/Source/capability graph retained by SeriesRegistry V2.
    pub compiler_bundle_id: CompiledProductSeriesBundleV5Id,
    /// Frozen finite ordinal count.
    pub instance_count: u32,
    /// Only ordinal which may be reserved or lapsed next.
    pub next_ordinal: u32,
    /// Number of cursor advances which spent no principal.
    pub lapsed_count: u32,
    /// Monotone mutation sequence.
    pub transition_sequence: u64,
    /// Explicit exhaustive lifecycle phase.
    pub phase: SeriesFundingPhaseV2,
    /// Founder/converger classification of the pending ordinal.
    pub pending_disposition: Option<SeriesMarketDispositionV1>,
    /// Exact pending Market, or zero outside Pending.
    pub pending_market_instance_id: ContentId,
    /// Exact pending compiled Source occurrence, or zero outside Pending.
    pub pending_source_occurrence_id: ContentId,
    /// Exact pending 0xad semantic state, or zero outside Pending.
    pub pending_series_market_link_id: ContentId,
    /// Exact pending ordinal; zero outside Pending.
    pub pending_ordinal: u32,
    /// Private adapter receipt which authorized the debit.
    pub pending_reservation_receipt_id: ContentId,
    /// Exact component debits held by the pending transition.
    pub pending_debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Six disjoint principal/donation ledgers.
    pub components: [SeriesComponentCapitalV2; SERIES_FUNDING_COMPONENT_COUNT_V2],
}

impl SeriesFundingStateV2 {
    /// Activate current state from exact V4 artifacts and physical deposits.
    #[allow(clippy::too_many_arguments)]
    pub fn activate<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        authority: &A,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV5Id,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
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
        let mut components = [SeriesComponentCapitalV2::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let expected = multiply_debit(quote.components[index], series.instance_count)?;
            if principal[index] != expected {
                return Err(Error::InsufficientPrepayment);
            }
            components[index] = SeriesComponentCapitalV2 {
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
            phase: SeriesFundingPhaseV2::Active,
            pending_disposition: None,
            pending_market_instance_id: ContentId::ZERO,
            pending_source_occurrence_id: ContentId::ZERO,
            pending_series_market_link_id: ContentId::ZERO,
            pending_ordinal: 0,
            pending_reservation_receipt_id: ContentId::ZERO,
            pending_debits: [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
            components,
        };
        value.validate_against(series, quote, attachment)?;
        Ok(value)
    }

    /// Reserve exactly the next eligible created ordinal.
    #[allow(clippy::too_many_arguments)]
    pub fn reserve_created<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        market_instance_id: MarketInstanceV2Id,
        source_occurrence_id: SourceOccurrenceV1Id,
        series_market_link_id: ContentId,
        disposition: SeriesMarketDispositionV1,
        debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        reservation_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV2::Active || self.next_ordinal >= self.instance_count {
            return Err(Error::SeriesNotActive);
        }
        market_instance_id.validate()?;
        source_occurrence_id.validate()?;
        series_market_link_id.validate()?;
        reservation_receipt_id.validate()?;
        let ordinal = self.next_ordinal;
        if !series.is_creation_eligible(ordinal, authority.current_bucket(series)?)? {
            return Err(Error::OutsideCreationWindow);
        }
        validate_reservation_debits(quote, disposition, &debits)?;
        authority.authenticate_reservation(
            self,
            ordinal,
            market_instance_id,
            source_occurrence_id,
            series_market_link_id,
            disposition,
            &debits,
            reservation_receipt_id,
        )?;
        let mut next = *self;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            next.components[index].remaining_principal =
                subtract_debit(next.components[index].remaining_principal, debits[index])?;
            if debits[index] != ComponentDebitV1::ZERO {
                next.components[index].consumed_allocations = next.components[index]
                    .consumed_allocations
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.phase = SeriesFundingPhaseV2::Pending;
        next.pending_disposition = Some(disposition);
        next.pending_market_instance_id = market_instance_id.content_id();
        next.pending_source_occurrence_id = source_occurrence_id.content_id();
        next.pending_series_market_link_id = series_market_link_id;
        next.pending_ordinal = ordinal;
        next.pending_reservation_receipt_id = reservation_receipt_id;
        next.pending_debits = debits;
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Commit the exact pending admission and advance the cursor once.
    pub fn complete_pending<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        completion_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV2::Pending {
            return Err(Error::WorkStateMismatch);
        }
        completion_receipt_id.validate()?;
        authority.authenticate_pending_completion(self, completion_receipt_id)?;
        let ordinal = self.pending_ordinal;
        let mut next = *self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.clear_pending();
        next.phase = if next.next_ordinal == next.instance_count {
            SeriesFundingPhaseV2::Closed
        } else {
            SeriesFundingPhaseV2::Active
        };
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Abort inert founding after an authenticated reverse-close and restore
    /// only the exact pending principal. Donations never enter this equation.
    pub fn abort_pending<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        abort_receipt_id: ContentId,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV2::Pending {
            return Err(Error::WorkStateMismatch);
        }
        abort_receipt_id.validate()?;
        authority.authenticate_pending_abort(self, abort_receipt_id)?;
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
        next.phase = SeriesFundingPhaseV2::Active;
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Advance one elapsed ordinal without spending or reserving principal.
    pub fn lapse<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
    ) -> Result<u32> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV2::Active || self.next_ordinal >= self.instance_count {
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
            SeriesFundingPhaseV2::Closed
        } else {
            SeriesFundingPhaseV2::Active
        };
        next.transition_sequence = increment(next.transition_sequence)?;
        next.validate_against(series, quote, attachment)?;
        *self = next;
        Ok(ordinal)
    }

    /// Record authenticated physical surplus without changing principal.
    pub fn add_donation<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        component: SeriesFundingComponentV2,
        amount: ComponentDebitV1,
    ) -> Result<()> {
        self.validate_against(series, quote, attachment)?;
        if self.phase == SeriesFundingPhaseV2::Pending {
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
    pub fn close<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
        terminal_receipt_id: ContentId,
    ) -> Result<SeriesFundingTerminalProjectionV2> {
        self.validate_against(series, quote, attachment)?;
        if self.phase != SeriesFundingPhaseV2::Closed {
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
        Ok(SeriesFundingTerminalProjectionV2 {
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
        quote: &SeriesFundingQuoteV4,
        attachment: &SeriesAttachmentPlanV4,
    ) -> Result<()> {
        self.validate()?;
        series.validate_shape()?;
        quote.validate()?;
        attachment.validate()?;
        if series.id()? != self.series_plan_id
            || series.instance_count != self.instance_count
            || quote.id()? != self.funding_quote_id
            || attachment.id()? != self.attachment_plan_id
            || attachment.funding_quote_id != self.funding_quote_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let admitted = self.admitted_created_count()?;
        if self.phase == SeriesFundingPhaseV2::Pending {
            validate_reservation_debits(
                quote,
                self.pending_disposition.ok_or(Error::InvalidComponentStatus)?,
                &self.pending_debits,
            )?;
        }
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let unit = quote.components[index];
            let consumed = self.components[index].consumed_allocations;
            if consumed > admitted {
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
    pub fn id(&self) -> Result<SeriesFundingStateV2Id> {
        let mut body = [0u8; SERIES_FUNDING_STATE_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingStateV2Id::from_bytes(
            content_id(SERIES_FUNDING_STATE_V2_DOMAIN, &body).bytes(),
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
        let pending = self.phase == SeriesFundingPhaseV2::Pending;
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
                self.pending_series_market_link_id,
                self.pending_reservation_receipt_id,
            ] {
                id.validate()?;
            }
        } else if self.pending_disposition.is_some()
            || !self.pending_market_instance_id.is_zero()
            || !self.pending_source_occurrence_id.is_zero()
            || !self.pending_series_market_link_id.is_zero()
            || self.pending_ordinal != 0
            || !self.pending_reservation_receipt_id.is_zero()
            || self
                .pending_debits
                .iter()
                .any(|debit| *debit != ComponentDebitV1::ZERO)
        {
            return Err(Error::NonCanonicalPadding);
        }
        match self.phase {
            SeriesFundingPhaseV2::Active if self.next_ordinal < self.instance_count => {}
            SeriesFundingPhaseV2::Pending if self.next_ordinal < self.instance_count => {}
            SeriesFundingPhaseV2::Closed if self.next_ordinal == self.instance_count => {}
            _ => return Err(Error::InvalidSchedule),
        }
        Ok(())
    }

    fn admitted_created_count(&self) -> Result<u32> {
        self.next_ordinal
            .checked_sub(self.lapsed_count)
            .and_then(|created| {
                created.checked_add(if self.phase == SeriesFundingPhaseV2::Pending {
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
        self.pending_series_market_link_id = ContentId::ZERO;
        self.pending_ordinal = 0;
        self.pending_reservation_receipt_id = ContentId::ZERO;
        self.pending_debits = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    }
}

impl FixedCodec for SeriesFundingStateV2 {
    const ENCODED_LEN: usize = SERIES_FUNDING_STATE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC_V2);
        writer.u16(SCHEMA_V2);
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
        writer.id(self.pending_series_market_link_id);
        writer.u32(self.pending_ordinal);
        writer.id(self.pending_reservation_receipt_id);
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
        reader.magic(&MAGIC_V2)?;
        if reader.u16() != SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        let phase = SeriesFundingPhaseV2::decode(reader.u8())?;
        let pending_disposition = match reader.u8() {
            0 => None,
            1 => Some(SeriesMarketDispositionV1::Founder),
            2 => Some(SeriesMarketDispositionV1::Converger),
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(4)?;
        let series_plan_id = SeriesPlanV5Id::from_bytes(reader.id().bytes());
        let funding_terms_id = SeriesFundingTermsV2Id::from_bytes(reader.id().bytes());
        let funding_quote_id = SeriesFundingQuoteV4Id::from_bytes(reader.id().bytes());
        let attachment_plan_id = SeriesAttachmentPlanV4Id::from_bytes(reader.id().bytes());
        let compiler_bundle_id = CompiledProductSeriesBundleV5Id::from_bytes(reader.id().bytes());
        let instance_count = reader.u32();
        let next_ordinal = reader.u32();
        let lapsed_count = reader.u32();
        let transition_sequence = reader.u64();
        let pending_market_instance_id = reader.id();
        let pending_source_occurrence_id = reader.id();
        let pending_series_market_link_id = reader.id();
        let pending_ordinal = reader.u32();
        let pending_reservation_receipt_id = reader.id();
        let mut pending_debits = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for debit in &mut pending_debits {
            debit.lamports = reader.u64();
            debit.collateral_atoms = reader.u64();
        }
        let mut components = [SeriesComponentCapitalV2::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
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
            pending_series_market_link_id,
            pending_ordinal,
            pending_reservation_receipt_id,
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
pub struct SeriesFundingTerminalProjectionV2 {
    /// Exact closed Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact immutable destination owner.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact current compiler graph.
    pub compiler_bundle_id: CompiledProductSeriesBundleV5Id,
    /// Last committed mutable sequence.
    pub transition_sequence: u64,
    /// Remaining payer principal by V2 component order.
    pub refundable_principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Donation residue by V2 component order.
    pub donation_residue: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Private terminal postwrite authorization.
    pub terminal_receipt_id: ContentId,
}

fn validate_reservation_debits(
    quote: &SeriesFundingQuoteV4,
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

    fn active_state() -> SeriesFundingStateV2 {
        SeriesFundingStateV2 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; 32]),
            funding_quote_id: SeriesFundingQuoteV4Id::from_bytes([3; 32]),
            attachment_plan_id: SeriesAttachmentPlanV4Id::from_bytes([4; 32]),
            compiler_bundle_id: CompiledProductSeriesBundleV5Id::from_bytes([5; 32]),
            instance_count: 1,
            next_ordinal: 0,
            lapsed_count: 0,
            transition_sequence: 0,
            phase: SeriesFundingPhaseV2::Active,
            pending_disposition: None,
            pending_market_instance_id: ContentId::ZERO,
            pending_source_occurrence_id: ContentId::ZERO,
            pending_series_market_link_id: ContentId::ZERO,
            pending_ordinal: 0,
            pending_reservation_receipt_id: ContentId::ZERO,
            pending_debits: [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
            components: [SeriesComponentCapitalV2::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
        }
    }

    #[test]
    fn current_width_is_exact_and_not_the_historical_state_width() {
        assert_eq!(SERIES_FUNDING_STATE_BYTES_V2, 664);
        assert_ne!(
            SERIES_FUNDING_STATE_BYTES_V2,
            crate::SERIES_FUNDING_STATE_BYTES
        );
    }

    #[test]
    fn codec_round_trips_and_refuses_a_caller_shaped_pending_phase() {
        let value = active_state();
        let mut body = [0; SERIES_FUNDING_STATE_BYTES_V2];
        value.encode_into(&mut body).unwrap();
        assert_eq!(SeriesFundingStateV2::decode(&body), Ok(value));
        body[10] = SeriesFundingPhaseV2::Pending.byte();
        body[11] = 1;
        assert_eq!(
            SeriesFundingStateV2::decode(&body),
            Err(Error::ZeroIdentity)
        );
    }

    #[test]
    fn pending_debits_refuse_wrong_disposition_geometry() {
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        components[SeriesFundingComponentV2::MarketCore.index()].lamports = 20;
        components[SeriesFundingComponentV2::SeriesAdmission.index()].lamports = 10;
        components[SeriesFundingComponentV2::RecoveryReserve.index()].lamports = 30;
        components[SeriesFundingComponentV2::SourceWork.index()].lamports = 7;
        let quote = SeriesFundingQuoteV4 {
            evidence_only_recovery_policy_id: ContentId::from_bytes([11; 32]),
            failure_liveness_policy_id: ContentId::from_bytes([12; 32]),
            failure_recovery_quote_schedule_id: ContentId::from_bytes([13; 32]),
            components,
            foundation: crate::MarketFoundationScheduleV2 {
                outcome_count: 2,
                slot_principal_lamports: [0; crate::MARKET_FOUNDATION_SLOT_COUNT_V2],
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
}
