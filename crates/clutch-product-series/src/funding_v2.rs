//! Greenfield six-compartment Series funding and shared-Market foundation quote.
//!
//! A Series admission is paid for on every created ordinal. Shared Market
//! foundation capital is paid exactly once by the founding Series and is zero
//! for exact convergers. Reservations are persisted before external account
//! work starts, so an ordinal cannot advance, lapse, or spend twice while
//! permissionless founding is in progress.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ComponentDebitV1, ContentId, Error, FixedCodec, MarketFoundationScheduleV1Id,
    MarketInstanceV2Id, RecoveryAttemptFundingV1, Result, SeriesFundingQuoteV2Id,
    SeriesFundingTermsV2Id, SeriesMarketLinkV1Id, SeriesPlanV5Id, MAX_RECOVERY_ATTEMPTS,
};

const QUOTE_MAGIC_V2: [u8; 8] = *b"DCFQUOT2";
const QUOTE_SCHEMA_V2: u16 = 2;
const STATE_MAGIC_V2: [u8; 8] = *b"DCSFNDV2";
const STATE_SCHEMA_V2: u16 = 2;

/// Maximum outcome count represented by the fixed foundation schedule.
pub const MARKET_FOUNDATION_MAX_OUTCOMES_V1: usize = 16;
/// Fixed shared-core slots preceding outcome mint and custody slots.
pub const MARKET_FOUNDATION_CORE_SLOT_COUNT_V1: usize = 13;
/// Exact number of slots: thirteen core, sixteen mints, sixteen custody accounts.
pub const MARKET_FOUNDATION_SLOT_COUNT_V1: usize =
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + 2 * MARKET_FOUNDATION_MAX_OUTCOMES_V1;
/// Exact V2 quote width.
pub const SERIES_FUNDING_QUOTE_BYTES_V2: usize = 648;
/// Exact V2 mutable funding-state width.
pub const SERIES_FUNDING_STATE_BYTES_V2: usize = 512;
/// Six disjoint Series funding compartments.
pub const SERIES_FUNDING_COMPONENT_COUNT_V2: usize = 6;

/// Stable six-compartment order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesFundingComponentV2 {
    /// Shared Market foundation, consumed by the first founder only.
    MarketCore = 0,
    /// Per-ordinal `0xad` admission-link rent, consumed for every creation.
    SeriesAdmission = 1,
    /// Finite evidence-only recovery reserve.
    RecoveryReserve = 2,
    /// Source, archive, window, and evaluator work.
    SourceWork = 3,
    /// Series-scoped passive-liquidity attachment.
    LiquidityFacility = 4,
    /// Series-scoped structured wrapper set.
    WrapperSet = 5,
}

impl SeriesFundingComponentV2 {
    /// Stable fixed-array index without an unchecked cast.
    pub const fn index(self) -> usize {
        match self {
            Self::MarketCore => 0,
            Self::SeriesAdmission => 1,
            Self::RecoveryReserve => 2,
            Self::SourceWork => 3,
            Self::LiquidityFacility => 4,
            Self::WrapperSet => 5,
        }
    }
}

/// Whether one Series ordinal founds or converges into a shared Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesMarketDispositionV1 {
    /// This link owns the one shared MarketCore debit.
    Founder = 1,
    /// The exact shared Market already exists; MarketCore debit is zero.
    Converger = 2,
}

impl SeriesMarketDispositionV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Founder => 1,
            Self::Converger => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Founder),
            2 => Ok(Self::Converger),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact itemized principals for one shared Market foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationScheduleV1 {
    /// Exact active outcome count; unused fixed slots must be zero.
    pub outcome_count: u8,
    /// Ordered slot principals owned by the MarketCore quote.
    pub slot_principal_lamports: [u64; MARKET_FOUNDATION_SLOT_COUNT_V1],
    /// Finite policy-owned timeout after which an inert foundation may abort.
    pub founding_timeout_buckets: u64,
}

impl MarketFoundationScheduleV1 {
    /// Validate active nonzero slots, zero tail, and exact finite timeout.
    pub fn validate(self) -> Result<()> {
        let outcomes = usize::from(self.outcome_count);
        if outcomes == 0
            || outcomes > MARKET_FOUNDATION_MAX_OUTCOMES_V1
            || self.founding_timeout_buckets == 0
        {
            return Err(Error::InvalidParameter);
        }
        let mint_end = MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_start = MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
            .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V1)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_end = custody_start
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0usize;
        while index < MARKET_FOUNDATION_SLOT_COUNT_V1 {
            let active = index < MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
                || (index >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 && index < mint_end)
                || (index >= custody_start && index < custody_end);
            if active != (self.slot_principal_lamports[index] != 0) {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.total_principal_lamports()?;
        Ok(())
    }

    /// Checked exact sum of every active foundation slot.
    pub fn total_principal_lamports(self) -> Result<u64> {
        let mut total = 0u64;
        for amount in self.slot_principal_lamports {
            total = total.checked_add(amount).ok_or(Error::ArithmeticOverflow)?;
        }
        if total == 0 {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(total)
    }

    /// Typed identity of the exact itemized schedule.
    pub fn id(self) -> Result<MarketFoundationScheduleV1Id> {
        self.validate()?;
        let mut body = [0u8; 376];
        body[0] = self.outcome_count;
        body[8..16].copy_from_slice(&self.founding_timeout_buckets.to_le_bytes());
        let mut at = 16usize;
        for amount in self.slot_principal_lamports {
            body[at..at + 8].copy_from_slice(&amount.to_le_bytes());
            at += 8;
        }
        Ok(MarketFoundationScheduleV1Id::from_bytes(
            content_id(b"dragons-clutch/market-foundation-schedule/v1", &body).bytes(),
        ))
    }
}

/// Exact V2 quote. V1 bytes are never reinterpreted as this schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV2 {
    /// Exact evidence-only recovery policy.
    pub evidence_only_recovery_policy_id: ContentId,
    /// Six independently accounted components.
    pub components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Sole itemization of the Founder-only MarketCore lamports.
    pub foundation: MarketFoundationScheduleV1,
    /// Active recovery attempt count.
    pub recovery_attempt_count: u8,
    /// Exact attempts followed by canonical zero padding.
    pub recovery_attempt_funding: [RecoveryAttemptFundingV1; MAX_RECOVERY_ATTEMPTS],
    /// Separately named recovery-account rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV2 {
    /// Validate six-way separation and the exact foundation/recovery sums.
    pub fn validate(&self) -> Result<()> {
        self.evidence_only_recovery_policy_id.validate()?;
        self.foundation.validate()?;
        let market_core = self.components[SeriesFundingComponentV2::MarketCore.index()];
        let admission = self.components[SeriesFundingComponentV2::SeriesAdmission.index()];
        let recovery = self.components[SeriesFundingComponentV2::RecoveryReserve.index()];
        if market_core.collateral_atoms != 0
            || admission.collateral_atoms != 0
            || admission.lamports == 0
            || market_core.lamports != self.foundation.total_principal_lamports()?
            || recovery.collateral_atoms != 0
            || self.recovery_rent_principal_lamports == 0
        {
            return Err(Error::InvalidParameter);
        }
        let count = usize::from(self.recovery_attempt_count);
        if count == 0 || count > MAX_RECOVERY_ATTEMPTS {
            return Err(Error::InvalidParameter);
        }
        let mut work = 0u64;
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let attempt = self.recovery_attempt_funding[index];
            if index < count {
                if attempt.max_progress_units == 0 || attempt.lamports_per_progress_unit == 0 {
                    return Err(Error::InvalidParameter);
                }
                work = work
                    .checked_add(attempt.maximum_lamports()?)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if attempt != RecoveryAttemptFundingV1::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if work
            .checked_add(self.recovery_rent_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?
            != recovery.lamports
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Typed quote identity.
    pub fn id(&self) -> Result<SeriesFundingQuoteV2Id> {
        let mut body = [0u8; SERIES_FUNDING_QUOTE_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteV2Id::from_bytes(
            content_id(b"dragons-clutch/series-funding-quote/v2", &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingQuoteV2 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&QUOTE_MAGIC_V2);
        writer.u16(QUOTE_SCHEMA_V2);
        writer.u8(self.foundation.outcome_count);
        writer.u8(self.recovery_attempt_count);
        writer.reserved(4);
        writer.id(self.evidence_only_recovery_policy_id);
        for component in self.components {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        for amount in self.foundation.slot_principal_lamports {
            writer.u64(amount);
        }
        writer.u64(self.foundation.founding_timeout_buckets);
        writer.u64(self.recovery_rent_principal_lamports);
        for attempt in self.recovery_attempt_funding {
            writer.u64(attempt.max_progress_units);
            writer.u64(attempt.lamports_per_progress_unit);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&QUOTE_MAGIC_V2)?;
        if reader.u16() != QUOTE_SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        let outcome_count = reader.u8();
        let recovery_attempt_count = reader.u8();
        reader.reserved(4)?;
        let evidence_only_recovery_policy_id = reader.id();
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in &mut components {
            component.lamports = reader.u64();
            component.collateral_atoms = reader.u64();
        }
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V1];
        for amount in &mut slot_principal_lamports {
            *amount = reader.u64();
        }
        let founding_timeout_buckets = reader.u64();
        let recovery_rent_principal_lamports = reader.u64();
        let mut recovery_attempt_funding = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        for attempt in &mut recovery_attempt_funding {
            attempt.max_progress_units = reader.u64();
            attempt.lamports_per_progress_unit = reader.u64();
        }
        reader.finish()?;
        let value = Self {
            evidence_only_recovery_policy_id,
            components,
            foundation: MarketFoundationScheduleV1 {
                outcome_count,
                slot_principal_lamports,
                founding_timeout_buckets,
            },
            recovery_attempt_count,
            recovery_attempt_funding,
            recovery_rent_principal_lamports,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Untrusted exact-existing/absent claim for the six Series compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesOrdinalFulfillmentV2 {
    /// Exact Market selected by the compiled ordinal.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact link semantic identity to be created.
    pub series_market_link_id: SeriesMarketLinkV1Id,
    /// `true` means the exact component must be debited.
    pub debit_component: [bool; SERIES_FUNDING_COMPONENT_COUNT_V2],
}

/// Persisted pending reservation which makes external founding replay-safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingReservationV2 {
    /// Reserved ordinal.
    pub ordinal: u32,
    /// Founder or exact converger.
    pub disposition: SeriesMarketDispositionV1,
    /// Exact shared Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact `0xad` semantic identity.
    pub series_market_link_id: SeriesMarketLinkV1Id,
    /// Private adapter debit receipt identity.
    pub debit_receipt_id: ContentId,
    /// Exact shared foundation schedule for Founder, zero for Converger.
    pub foundation_schedule_id: ContentId,
    /// Six-bit exact debit bitmap.
    pub debit_bitmap: u8,
}

/// Default-deny adapter seam for funding V2 account, Clock, PDA, and transfer facts.
pub trait AuthenticatedSeriesFundingAuthorityV2 {
    /// Authenticate initial custody principal and donation observations.
    fn authenticate_activation(
        &self,
        _quote: &SeriesFundingQuoteV2,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate exact occurrence/link/core presence and atomic debits.
    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV2,
        _quote: &SeriesFundingQuoteV2,
        _reservation: &SeriesFundingReservationV2,
        _fulfillment: &SeriesOrdinalFulfillmentV2,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate Active Market + Active link before committing the cursor.
    fn authenticate_commit(
        &self,
        _state: &SeriesFundingStateV2,
        _reservation: &SeriesFundingReservationV2,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate timeout, dependency-ordered inert close, exact refund, and sink deltas.
    fn authenticate_abort(
        &self,
        _state: &SeriesFundingStateV2,
        _quote: &SeriesFundingQuoteV2,
        _reservation: &SeriesFundingReservationV2,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate that the current ordinal's creation window elapsed.
    fn authenticate_lapse(&self, _state: &SeriesFundingStateV2, _ordinal: u32) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Derived finite Series funding phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFundingPhaseV2 {
    /// No pending reservation and an ordinal remains.
    Active,
    /// One ordinal is reserved and cannot be spent/lapsed again.
    Pending,
    /// Every ordinal committed or lapsed.
    Closed,
}

/// One mutable six-compartment principal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingStateV2 {
    /// Immutable Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable refund/sink ownership terms.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Immutable V2 quote.
    pub funding_quote_id: SeriesFundingQuoteV2Id,
    /// Finite ordinal count.
    pub instance_count: u32,
    /// Only ordinal eligible for reservation or lapse.
    pub next_ordinal: u32,
    /// Ordinals lapsed without a debit.
    pub lapsed_count: u32,
    /// Monotone funding transition sequence.
    pub transition_sequence: u64,
    /// Pending reservation, if any.
    pub pending: Option<SeriesFundingReservationV2>,
    /// Six disjoint capital compartments.
    pub components: [crate::SeriesComponentCapitalV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
}

impl SeriesFundingStateV2 {
    /// Activate exact whole-Series capital.
    #[allow(clippy::too_many_arguments)]
    pub fn activate<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        authority: &A,
        series_plan_id: SeriesPlanV5Id,
        funding_terms_id: SeriesFundingTermsV2Id,
        instance_count: u32,
        quote: &SeriesFundingQuoteV2,
        principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<Self> {
        quote.validate()?;
        series_plan_id.validate()?;
        funding_terms_id.validate()?;
        if instance_count == 0 {
            return Err(Error::InvalidSchedule);
        }
        let mut required = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            required[index] = checked_mul(quote.components[index], instance_count)?;
            index += 1;
        }
        if principal != required {
            return Err(Error::InsufficientPrepayment);
        }
        authority.authenticate_activation(quote, &principal, &donations)?;
        let mut components = [crate::SeriesComponentCapitalV1 {
            remaining_principal: ComponentDebitV1::ZERO,
            donations: ComponentDebitV1::ZERO,
            consumed_allocations: 0,
        }; SERIES_FUNDING_COMPONENT_COUNT_V2];
        index = 0;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            components[index].remaining_principal = principal[index];
            components[index].donations = donations[index];
            index += 1;
        }
        let state = Self {
            series_plan_id,
            funding_terms_id,
            funding_quote_id: quote.id()?,
            instance_count,
            next_ordinal: 0,
            lapsed_count: 0,
            transition_sequence: 0,
            pending: None,
            components,
        };
        state.validate_against_quote(quote)?;
        Ok(state)
    }

    /// Reserve one ordinal and atomically debit SeriesAdmission plus exact absent components.
    #[allow(clippy::too_many_arguments)]
    pub fn reserve<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        quote: &SeriesFundingQuoteV2,
        disposition: SeriesMarketDispositionV1,
        fulfillment: SeriesOrdinalFulfillmentV2,
        debit_receipt_id: ContentId,
    ) -> Result<SeriesFundingReservationV2> {
        self.validate_against_quote(quote)?;
        if self.phase()? != SeriesFundingPhaseV2::Active {
            return Err(Error::SeriesNotActive);
        }
        fulfillment.market_instance_id.validate()?;
        fulfillment.series_market_link_id.validate()?;
        debit_receipt_id.validate()?;
        let market_core = SeriesFundingComponentV2::MarketCore.index();
        let admission = SeriesFundingComponentV2::SeriesAdmission.index();
        let recovery = SeriesFundingComponentV2::RecoveryReserve.index();
        if !fulfillment.debit_component[admission]
            || fulfillment.debit_component[market_core]
                != (disposition == SeriesMarketDispositionV1::Founder)
            || fulfillment.debit_component[recovery] != fulfillment.debit_component[market_core]
        {
            return Err(Error::InvalidComponentStatus);
        }
        let foundation_schedule_id = match disposition {
            SeriesMarketDispositionV1::Founder => quote.foundation.id()?.content_id(),
            SeriesMarketDispositionV1::Converger => ContentId::ZERO,
        };
        let mut bitmap = 0u8;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            if fulfillment.debit_component[index] {
                bitmap |= 1u8 << index;
            }
            index += 1;
        }
        let reservation = SeriesFundingReservationV2 {
            ordinal: self.next_ordinal,
            disposition,
            market_instance_id: fulfillment.market_instance_id,
            series_market_link_id: fulfillment.series_market_link_id,
            debit_receipt_id,
            foundation_schedule_id,
            debit_bitmap: bitmap,
        };
        authority.authenticate_reservation(self, quote, &reservation, &fulfillment)?;
        let mut next = *self;
        index = 0;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            if bit(bitmap, index) {
                next.components[index].remaining_principal = checked_sub(
                    next.components[index].remaining_principal,
                    quote.components[index],
                )?;
                next.components[index].consumed_allocations = next.components[index]
                    .consumed_allocations
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.pending = Some(reservation);
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate_against_quote(quote)?;
        *self = next;
        Ok(reservation)
    }

    /// Commit one Active link and advance the ordinal exactly once.
    pub fn commit_pending<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        quote: &SeriesFundingQuoteV2,
    ) -> Result<u32> {
        self.validate_against_quote(quote)?;
        let reservation = self.pending.ok_or(Error::WorkStateMismatch)?;
        authority.authenticate_commit(self, &reservation)?;
        let mut next = *self;
        next.pending = None;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate_against_quote(quote)?;
        *self = next;
        Ok(reservation.ordinal)
    }

    /// Abort an inert timed-out foundation/link and restore the same ordinal's principal.
    pub fn abort_pending<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        quote: &SeriesFundingQuoteV2,
    ) -> Result<u32> {
        self.validate_against_quote(quote)?;
        let reservation = self.pending.ok_or(Error::WorkStateMismatch)?;
        authority.authenticate_abort(self, quote, &reservation)?;
        let mut next = *self;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            if bit(reservation.debit_bitmap, index) {
                next.components[index].remaining_principal = checked_add(
                    next.components[index].remaining_principal,
                    quote.components[index],
                )?;
                next.components[index].consumed_allocations = next.components[index]
                    .consumed_allocations
                    .checked_sub(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.pending = None;
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate_against_quote(quote)?;
        *self = next;
        Ok(reservation.ordinal)
    }

    /// Advance one elapsed, unreserved ordinal without spending principal.
    pub fn lapse<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
        &mut self,
        authority: &A,
        quote: &SeriesFundingQuoteV2,
    ) -> Result<u32> {
        self.validate_against_quote(quote)?;
        if self.phase()? != SeriesFundingPhaseV2::Active {
            return Err(Error::SeriesNotActive);
        }
        let ordinal = self.next_ordinal;
        authority.authenticate_lapse(self, ordinal)?;
        let mut next = *self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.lapsed_count = next
            .lapsed_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate_against_quote(quote)?;
        *self = next;
        Ok(ordinal)
    }

    /// Current derived phase.
    pub fn phase(&self) -> Result<SeriesFundingPhaseV2> {
        self.validate()?;
        if self.pending.is_some() {
            Ok(SeriesFundingPhaseV2::Pending)
        } else if self.next_ordinal == self.instance_count {
            Ok(SeriesFundingPhaseV2::Closed)
        } else {
            Ok(SeriesFundingPhaseV2::Active)
        }
    }

    /// Validate structure and all quote-owned principal equations.
    pub fn validate_against_quote(&self, quote: &SeriesFundingQuoteV2) -> Result<()> {
        self.validate()?;
        quote.validate()?;
        if self.funding_quote_id != quote.id()? {
            return Err(Error::MismatchedArtifact);
        }
        let committed_created = self
            .next_ordinal
            .checked_sub(self.lapsed_count)
            .ok_or(Error::InvalidSchedule)?;
        let pending = if self.pending.is_some() { 1u32 } else { 0u32 };
        let allocations = committed_created
            .checked_add(pending)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let consumed = self.components[index].consumed_allocations;
            if consumed > allocations {
                return Err(Error::InvalidComponentStatus);
            }
            let expected = checked_mul(
                quote.components[index],
                self.instance_count
                    .checked_sub(consumed)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            if self.components[index].remaining_principal != expected {
                return Err(Error::InvalidComponentStatus);
            }
            index += 1;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        if self.instance_count == 0
            || self.next_ordinal > self.instance_count
            || self.lapsed_count > self.next_ordinal
            || (self.next_ordinal == self.instance_count && self.pending.is_some())
        {
            return Err(Error::InvalidSchedule);
        }
        if let Some(pending) = self.pending {
            pending.market_instance_id.validate()?;
            pending.series_market_link_id.validate()?;
            pending.debit_receipt_id.validate()?;
            if pending.ordinal != self.next_ordinal
                || !bit(
                    pending.debit_bitmap,
                    SeriesFundingComponentV2::SeriesAdmission.index(),
                )
                || (pending.disposition == SeriesMarketDispositionV1::Founder)
                    != bit(
                        pending.debit_bitmap,
                        SeriesFundingComponentV2::MarketCore.index(),
                    )
                || (pending.disposition == SeriesMarketDispositionV1::Founder)
                    != (pending.foundation_schedule_id != ContentId::ZERO)
            {
                return Err(Error::WorkStateMismatch);
            }
        }
        Ok(())
    }
}

impl FixedCodec for SeriesFundingStateV2 {
    const ENCODED_LEN: usize = SERIES_FUNDING_STATE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&STATE_MAGIC_V2);
        writer.u16(STATE_SCHEMA_V2);
        writer.reserved(6);
        writer.id(self.series_plan_id.content_id());
        writer.id(self.funding_terms_id.content_id());
        writer.id(self.funding_quote_id.content_id());
        match self.pending {
            Some(pending) => {
                writer.id(pending.market_instance_id.content_id());
                writer.id(pending.series_market_link_id.content_id());
                writer.id(pending.debit_receipt_id);
                writer.id(pending.foundation_schedule_id);
                writer.u32(pending.ordinal);
                writer.u8(1);
                writer.u8(pending.disposition.byte());
                writer.u8(pending.debit_bitmap);
                writer.reserved(1);
            }
            None => {
                writer.id(ContentId::ZERO);
                writer.id(ContentId::ZERO);
                writer.id(ContentId::ZERO);
                writer.id(ContentId::ZERO);
                writer.u32(u32::MAX);
                writer.u8(0);
                writer.reserved(3);
            }
        }
        writer.u32(self.instance_count);
        writer.u32(self.next_ordinal);
        writer.u32(self.lapsed_count);
        writer.u64(self.transition_sequence);
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
        reader.magic(&STATE_MAGIC_V2)?;
        if reader.u16() != STATE_SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let series_plan_id = SeriesPlanV5Id::from_bytes(reader.id().bytes());
        let funding_terms_id = SeriesFundingTermsV2Id::from_bytes(reader.id().bytes());
        let funding_quote_id = SeriesFundingQuoteV2Id::from_bytes(reader.id().bytes());
        let market = reader.id();
        let link = reader.id();
        let debit_receipt_id = reader.id();
        let foundation_schedule_id = reader.id();
        let pending_ordinal = reader.u32();
        let has_pending = reader.u8();
        let disposition = reader.u8();
        let debit_bitmap = reader.u8();
        reader.reserved(1)?;
        let instance_count = reader.u32();
        let next_ordinal = reader.u32();
        let lapsed_count = reader.u32();
        let transition_sequence = reader.u64();
        let mut components = [crate::SeriesComponentCapitalV1 {
            remaining_principal: ComponentDebitV1::ZERO,
            donations: ComponentDebitV1::ZERO,
            consumed_allocations: 0,
        }; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in &mut components {
            component.remaining_principal = ComponentDebitV1 {
                lamports: reader.u64(),
                collateral_atoms: reader.u64(),
            };
            component.donations = ComponentDebitV1 {
                lamports: reader.u64(),
                collateral_atoms: reader.u64(),
            };
            component.consumed_allocations = reader.u32();
            reader.reserved(4)?;
        }
        reader.finish()?;
        let pending = match has_pending {
            0 => {
                if market != ContentId::ZERO
                    || link != ContentId::ZERO
                    || debit_receipt_id != ContentId::ZERO
                    || foundation_schedule_id != ContentId::ZERO
                    || pending_ordinal != u32::MAX
                    || disposition != 0
                    || debit_bitmap != 0
                {
                    return Err(Error::NonCanonicalPadding);
                }
                None
            }
            1 => Some(SeriesFundingReservationV2 {
                ordinal: pending_ordinal,
                disposition: SeriesMarketDispositionV1::decode(disposition)?,
                market_instance_id: MarketInstanceV2Id::from_bytes(market.bytes()),
                series_market_link_id: SeriesMarketLinkV1Id::from_bytes(link.bytes()),
                debit_receipt_id,
                foundation_schedule_id,
                debit_bitmap,
            }),
            _ => return Err(Error::InvalidParameter),
        };
        let value = Self {
            series_plan_id,
            funding_terms_id,
            funding_quote_id,
            instance_count,
            next_ordinal,
            lapsed_count,
            transition_sequence,
            pending,
            components,
        };
        value.validate()?;
        Ok(value)
    }
}

fn bit(bitmap: u8, index: usize) -> bool {
    (bitmap & (1u8 << index)) != 0
}

fn checked_add(left: ComponentDebitV1, right: ComponentDebitV1) -> Result<ComponentDebitV1> {
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

fn checked_sub(left: ComponentDebitV1, right: ComponentDebitV1) -> Result<ComponentDebitV1> {
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

fn checked_mul(value: ComponentDebitV1, count: u32) -> Result<ComponentDebitV1> {
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
