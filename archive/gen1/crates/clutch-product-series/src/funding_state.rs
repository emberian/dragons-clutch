use crate::codec::{Reader, Writer};
use crate::{
    project_component_debits_v2, AdapterFulfillmentProjectionV2, CompiledSourceOccurrenceV3,
    ComponentDebitV1, ContentId, DebitProjectionV1, Error, EvidenceOnlyRecoveryPolicyV1,
    FixedCodec, FundingBalancesV1, MarketGenesisProfileV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, ProjectedComponentPresenceV2,
    RegistryCapabilityProjectionV2, Result, SeriesAttachmentPlanV1, SeriesFundingQuoteId,
    SeriesFundingQuoteV1, SeriesFundingTermsV2, SeriesFundingTermsV2Id, SeriesPlanV5,
    SeriesPlanV5Id,
};

const SERIES_FUNDING_STATE_MAGIC: [u8; 8] = *b"DCSFUND1";
const SERIES_FUNDING_STATE_SCHEMA: u16 = 1;

/// Exact persisted width of [`SeriesFundingStateV1`].
pub const SERIES_FUNDING_STATE_BYTES: usize = 324;
/// Number of independently capitalized and debited Series components.
pub const SERIES_FUNDING_COMPONENT_COUNT: usize = 5;

/// Stable component order shared by requirements, state, and debit projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesFundingComponentV1 {
    /// Market root, child plane, mints, and custody creation.
    MarketCore = 0,
    /// Finite evidence-only recovery reserve.
    RecoveryReserve = 1,
    /// Source, archive, window, and evaluator work.
    SourceWork = 2,
    /// Passive-liquidity facility attachment.
    LiquidityFacility = 3,
    /// Canonical wrapper descriptor, mint, and vault attachment.
    WrapperSet = 4,
}

impl SeriesFundingComponentV1 {
    /// Stable array index.
    pub const fn index(self) -> usize {
        match self {
            Self::MarketCore => 0,
            Self::RecoveryReserve => 1,
            Self::SourceWork => 2,
            Self::LiquidityFacility => 3,
            Self::WrapperSet => 4,
        }
    }

    /// Stable hostile-wire/PDA byte without an unchecked enum cast.
    pub const fn byte(self) -> u8 {
        match self {
            Self::MarketCore => 0,
            Self::RecoveryReserve => 1,
            Self::SourceWork => 2,
            Self::LiquidityFacility => 3,
            Self::WrapperSet => 4,
        }
    }
}

/// Exact immutable per-occurrence and whole-Series funding requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingRequirementsV1 {
    /// Exact successor Series identity.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact component quote identity.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Finite occurrence count multiplied into every total.
    pub instance_count: u32,
    /// Per-occurrence requirements in [`SeriesFundingComponentV1`] order.
    pub per_occurrence: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
    /// Exact activation principal in the same component order.
    pub activation_principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
}

impl SeriesFundingRequirementsV1 {
    /// Derive exact requirements from the immutable Series, attachment, and quote.
    pub fn derive(
        series: &SeriesPlanV5,
        attachment: &SeriesAttachmentPlanV1,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<Self> {
        series.validate_shape()?;
        attachment.validate()?;
        quote.validate()?;
        let series_plan_id = series.id()?;
        let funding_quote_id = quote.id()?;
        if series.attachment_plan_id != attachment.id()?
            || attachment.funding_quote_id != funding_quote_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let per_occurrence = quote_components(quote);
        let mut activation_principal = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            activation_principal[index] =
                checked_mul(per_occurrence[index], series.instance_count)?;
            index += 1;
        }
        Ok(Self {
            series_plan_id,
            funding_quote_id,
            instance_count: series.instance_count,
            per_occurrence,
            activation_principal,
        })
    }
}

/// Lifecycle phase of one finite activated funding state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesFundingPhaseV1 {
    /// At least one ordinal remains to be created or lapsed.
    Active = 1,
    /// Every finite ordinal was either created or lapsed.
    Closed = 2,
}

/// One component's segregated principal and unsolicited donations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesComponentCapitalV1 {
    /// Refundable payer principal not yet moved into an occurrence.
    pub remaining_principal: ComponentDebitV1,
    /// Unsolicited residue excluded from principal and sent to the neutral sink.
    pub donations: ComponentDebitV1,
    /// Number of absent-component allocations consumed by creation.
    pub consumed_allocations: u32,
}

impl SeriesComponentCapitalV1 {
    const ZERO: Self = Self {
        remaining_principal: ComponentDebitV1::ZERO,
        donations: ComponentDebitV1::ZERO,
        consumed_allocations: 0,
    };

    fn validate_shape(self, created_count: u32) -> Result<()> {
        if self.consumed_allocations > created_count {
            return Err(Error::InvalidComponentStatus);
        }
        Ok(())
    }

    fn validate_against(
        self,
        per_occurrence: ComponentDebitV1,
        instance_count: u32,
        created_count: u32,
    ) -> Result<()> {
        self.validate_shape(created_count)?;
        let expected_remaining = checked_mul(
            per_occurrence,
            instance_count
                .checked_sub(self.consumed_allocations)
                .ok_or(Error::ArithmeticOverflow)?,
        )?;
        if self.remaining_principal != expected_remaining {
            return Err(Error::InvalidComponentStatus);
        }
        Ok(())
    }
}

/// Adapter-authenticated inputs for one exact Series activation.
///
/// The pure core validates every immutable join again. The adapter remains
/// responsible for authenticating account identity, owner, exact body,
/// registry provenance, custody accounts, and actual segregated balances.
#[derive(Clone, Copy, Debug)]
pub struct SeriesActivationContextV1<'a> {
    /// Immutable finite Series.
    pub series: &'a SeriesPlanV5,
    /// Reusable Product semantics.
    pub template: &'a ProductTemplateV4,
    /// Exact payout basis.
    pub basis: &'a NativeClaimBasisV1,
    /// Evidence-only recovery policy.
    pub recovery: &'a EvidenceOnlyRecoveryPolicyV1,
    /// Exact quantized price semantics.
    pub price_policy: &'a PriceMeasurePolicyV1,
    /// Immutable Realm/Profile market semantics.
    pub genesis: &'a MarketGenesisProfileV2,
    /// Operational attachment identities.
    pub attachment: &'a SeriesAttachmentPlanV1,
    /// Exact per-occurrence quote.
    pub quote: &'a SeriesFundingQuoteV1,
    /// Immutable refund, sink, mint, and token-program ownership terms.
    pub funding_terms: &'a SeriesFundingTermsV2,
    /// Complete adapter-authenticated registry projection.
    pub registry: &'a RegistryCapabilityProjectionV2,
    /// Exact component-separated payer principal presented for activation.
    pub principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
    /// Separately identified unsolicited residue present at activation.
    pub donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
}

/// Default-deny adapter boundary for value-bearing funding transitions.
///
/// Implementing this Rust trait is not itself cryptographic authentication.
/// A live SBF adapter must make its implementation constructible only after
/// checking the named accounts, owners, PDAs, exact bodies, Clock, transfers,
/// token program, mint, and component custody balances.
pub trait AuthenticatedSeriesFundingAuthorityV1 {
    /// Authenticate registry provenance, funding transfers, and segregated
    /// custody for an otherwise fully validated activation.
    fn authenticate_activation(
        &self,
        _context: &SeriesActivationContextV1<'_>,
        _requirements: &SeriesFundingRequirementsV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Return the current bucket from an authenticated Clock mapping.
    fn authenticated_current_bucket(&self, _series: &SeriesPlanV5) -> Result<u64> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the exact occurrence accounts and return their
    /// exact-existing/absent component projection.
    fn authenticated_fulfillment_projection(
        &self,
        _state: &SeriesFundingStateV1,
        _occurrence: &CompiledSourceOccurrenceV3,
        _attachment: &SeriesAttachmentPlanV1,
        _quote: &SeriesFundingQuoteV1,
    ) -> Result<AdapterFulfillmentProjectionV2> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate an unsolicited transfer into the named component custody.
    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV1,
        _quote: &SeriesFundingQuoteV1,
        _component: SeriesFundingComponentV1,
        _amount: ComponentDebitV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Mutable finite Series capitalization and ordinal cursor.
///
/// This is pure transition state, not authentication. A live adapter must
/// authenticate every immutable artifact, registry selector, Realm/Profile,
/// component account, token vault, and exact-existing occurrence before
/// applying one returned transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingStateV1 {
    /// Immutable Series whose finite cursor this state advances.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable refund, sink, mint, and token-program ownership terms.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Immutable per-occurrence quote.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Frozen finite ordinal count.
    pub instance_count: u32,
    /// Only ordinal that may next be created or lapsed.
    pub next_ordinal: u32,
    /// Ordinals advanced after their creation window elapsed, without spending.
    pub lapsed_count: u32,
    /// Five exact, non-fungible accounting compartments.
    pub components: [SeriesComponentCapitalV1; SERIES_FUNDING_COMPONENT_COUNT],
}

impl SeriesFundingStateV1 {
    /// Decode exact bytes and immediately join all mutable amounts to the
    /// immutable quote. Live adapters should prefer this to bare structural
    /// [`FixedCodec::decode`].
    pub fn decode_against_quote(input: &[u8], quote: &SeriesFundingQuoteV1) -> Result<Self> {
        let value = Self::decode(input)?;
        value.validate_against_quote(quote)?;
        Ok(value)
    }

    /// Activate with exact payer principal and separately named donations.
    ///
    /// Principal must equal the immutable whole-Series requirement component by
    /// component. Donations neither cure a shortfall nor become refundable.
    pub fn activate<A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
        authority: &A,
        context: &SeriesActivationContextV1<'_>,
    ) -> Result<Self> {
        let requirements =
            SeriesFundingRequirementsV1::derive(context.series, context.attachment, context.quote)?;
        context.quote.validate_recovery_binding(context.recovery)?;
        context.funding_terms.validate_bindings(
            context.series,
            context.template,
            context.basis,
            context.recovery,
            context.price_policy,
            context.genesis,
            context.registry,
        )?;
        if context.principal != requirements.activation_principal {
            return Err(Error::InsufficientPrepayment);
        }
        authority.authenticate_activation(context, &requirements)?;
        let mut components = [SeriesComponentCapitalV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            components[index] = SeriesComponentCapitalV1 {
                remaining_principal: context.principal[index],
                donations: context.donations[index],
                consumed_allocations: 0,
            };
            index += 1;
        }
        let value = Self {
            series_plan_id: requirements.series_plan_id,
            funding_terms_id: context.funding_terms.id()?,
            funding_quote_id: requirements.funding_quote_id,
            instance_count: requirements.instance_count,
            next_ordinal: 0,
            lapsed_count: 0,
            components,
        };
        value.validate_against_quote(context.quote)?;
        Ok(value)
    }

    /// Derive the lifecycle phase without persisting a duplicate flag.
    pub fn phase(&self) -> Result<SeriesFundingPhaseV1> {
        self.validate()?;
        Ok(self.derived_phase())
    }

    /// Derive creations/convergences from the cursor and lapse count.
    pub fn created_count(&self) -> Result<u32> {
        self.validate()?;
        self.derived_created_count()
    }

    fn derived_created_count(&self) -> Result<u32> {
        self.next_ordinal
            .checked_sub(self.lapsed_count)
            .ok_or(Error::InvalidSchedule)
    }

    fn derived_phase(&self) -> SeriesFundingPhaseV1 {
        if self.next_ordinal == self.instance_count {
            SeriesFundingPhaseV1::Closed
        } else {
            SeriesFundingPhaseV1::Active
        }
    }

    /// Validate the complete canonical state and all compartment equations.
    pub fn validate(&self) -> Result<()> {
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        if self.instance_count == 0
            || self.next_ordinal > self.instance_count
            || self.lapsed_count > self.next_ordinal
        {
            return Err(Error::InvalidSchedule);
        }
        let created_count = self.derived_created_count()?;
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            self.components[index].validate_shape(created_count)?;
            index += 1;
        }
        let market = self.components[SeriesFundingComponentV1::MarketCore.index()];
        let recovery = self.components[SeriesFundingComponentV1::RecoveryReserve.index()];
        if market.consumed_allocations != recovery.consumed_allocations {
            return Err(Error::InvalidComponentStatus);
        }
        Ok(())
    }

    /// Add an unsolicited component donation without changing principal.
    pub fn add_donation<A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
        &mut self,
        authority: &A,
        quote: &SeriesFundingQuoteV1,
        component: SeriesFundingComponentV1,
        amount: ComponentDebitV1,
    ) -> Result<()> {
        self.validate_against_quote(quote)?;
        authority.authenticate_donation(self, quote, component, amount)?;
        let mut next = *self;
        let slot = &mut next.components[component.index()];
        slot.donations = checked_add(slot.donations, amount)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Advance the one eligible ordinal and debit only authenticated absent components.
    pub fn advance_created<A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        recovery: &EvidenceOnlyRecoveryPolicyV1,
        attachment: &SeriesAttachmentPlanV1,
        quote: &SeriesFundingQuoteV1,
        occurrence: &CompiledSourceOccurrenceV3,
    ) -> Result<(u32, DebitProjectionV1)> {
        self.validate_series_and_quote(series, quote)?;
        if self.derived_phase() != SeriesFundingPhaseV1::Active {
            return Err(Error::SeriesNotActive);
        }
        let ordinal = self.next_ordinal;
        if occurrence.series_plan_id != self.series_plan_id
            || occurrence.ordinal != ordinal
            || occurrence.attachment_plan_id != series.attachment_plan_id
        {
            return Err(Error::MismatchedArtifact);
        }
        occurrence.validate_shape()?;
        recovery.validate()?;
        attachment.validate()?;
        if attachment.id()? != series.attachment_plan_id
            || attachment.funding_quote_id != self.funding_quote_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let current_bucket = authority.authenticated_current_bucket(series)?;
        if !series.is_creation_eligible(ordinal, current_bucket)? {
            return Err(Error::OutsideCreationWindow);
        }
        let fulfillment =
            authority.authenticated_fulfillment_projection(self, occurrence, attachment, quote)?;
        let absent = fulfillment_absence(fulfillment);
        let projection = project_component_debits_v2(
            occurrence.market_instance_id,
            recovery,
            attachment,
            quote,
            fulfillment,
            self.remaining_principal()?,
        )?;
        let projected = projection_components(projection);
        let requirements = quote_components(quote);
        let expected_total = checked_component_sum(projected)?;
        if projection.total != expected_total {
            return Err(Error::InvalidComponentStatus);
        }
        let mut next = *self;
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            let expected = requirements[index];
            let debit = projected[index];
            if (absent[index] && debit != expected)
                || (!absent[index] && debit != ComponentDebitV1::ZERO)
            {
                return Err(Error::InvalidComponentStatus);
            }
            if absent[index] {
                let component = &mut next.components[index];
                component.remaining_principal =
                    checked_sub(component.remaining_principal, expected)?;
                component.consumed_allocations = component
                    .consumed_allocations
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.advance_cursor()?;
        next.validate_against_quote(quote)?;
        let remaining = next.remaining_principal()?;
        if projection.remaining.lamports != remaining.lamports
            || projection.remaining.collateral_atoms != remaining.collateral_atoms
        {
            return Err(Error::InvalidComponentStatus);
        }
        *self = next;
        Ok((ordinal, projection))
    }

    /// Advance an elapsed next ordinal without spending any component principal.
    pub fn lapse<A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
        &mut self,
        authority: &A,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<u32> {
        self.validate_series_and_quote(series, quote)?;
        if self.derived_phase() != SeriesFundingPhaseV1::Active {
            return Err(Error::SeriesNotActive);
        }
        let ordinal = self.next_ordinal;
        let current_bucket = authority.authenticated_current_bucket(series)?;
        if current_bucket < series.start_bucket(ordinal)? {
            return Err(Error::OutsideCreationWindow);
        }
        let mut next = *self;
        next.lapsed_count = next
            .lapsed_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.advance_cursor()?;
        next.validate_against_quote(quote)?;
        *self = next;
        Ok(ordinal)
    }

    /// Project terminal principal refunds and donation residue after closure.
    pub fn terminal_projection(
        &self,
        funding_terms: &SeriesFundingTermsV2,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<SeriesFundingTerminalProjectionV1> {
        self.validate_against_quote(quote)?;
        funding_terms.validate_shape()?;
        if self.derived_phase() != SeriesFundingPhaseV1::Closed {
            return Err(Error::SeriesNotClosed);
        }
        if funding_terms.id()? != self.funding_terms_id
            || funding_terms.series_plan_id != self.series_plan_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let mut refundable_principal = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut donation_residue = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            refundable_principal[index] = self.components[index].remaining_principal;
            donation_residue[index] = self.components[index].donations;
            index += 1;
        }
        let value = SeriesFundingTerminalProjectionV1 {
            lamport_principal_refund: funding_terms.lamport_principal_refund,
            collateral_principal_refund_token_account: funding_terms
                .collateral_principal_refund_token_account,
            neutral_collateral_disposition_token_account: funding_terms
                .neutral_collateral_disposition_token_account,
            neutral_lamport_sink: funding_terms.neutral_lamport_sink,
            refundable_principal,
            donation_residue,
        };
        value.refundable_total()?;
        value.donation_total()?;
        Ok(value)
    }

    /// Join every mutable amount back to the immutable quote semantic owner.
    pub fn validate_against_quote(&self, quote: &SeriesFundingQuoteV1) -> Result<()> {
        self.validate()?;
        quote.validate()?;
        if quote.id()? != self.funding_quote_id {
            return Err(Error::MismatchedArtifact);
        }
        let requirements = quote_components(quote);
        let created_count = self.derived_created_count()?;
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            self.components[index].validate_against(
                requirements[index],
                self.instance_count,
                created_count,
            )?;
            index += 1;
        }
        Ok(())
    }

    fn remaining_principal(&self) -> Result<FundingBalancesV1> {
        let mut total = ComponentDebitV1::ZERO;
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            total = checked_add(total, self.components[index].remaining_principal)?;
            index += 1;
        }
        Ok(FundingBalancesV1 {
            lamports: total.lamports,
            collateral_atoms: total.collateral_atoms,
        })
    }

    fn validate_series_and_quote(
        &self,
        series: &SeriesPlanV5,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<()> {
        self.validate_against_quote(quote)?;
        series.validate_shape()?;
        if series.id()? != self.series_plan_id || series.instance_count != self.instance_count {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    fn advance_cursor(&mut self) -> Result<()> {
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Terminal ownership projection with principal and donations kept distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingTerminalProjectionV1 {
    /// Destination for refundable lamport principal.
    pub lamport_principal_refund: ContentId,
    /// Destination token account for refundable collateral principal.
    pub collateral_principal_refund_token_account: ContentId,
    /// Receive-only token account for unowned collateral donation residue.
    pub neutral_collateral_disposition_token_account: ContentId,
    /// System-owned destination for unowned lamport donation residue.
    pub neutral_lamport_sink: ContentId,
    /// Unspent payer principal in exact component order.
    pub refundable_principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
    /// Unsolicited residue in exact component order.
    pub donation_residue: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
}

impl SeriesFundingTerminalProjectionV1 {
    /// Checked aggregate payer refund for reporting and post-delta checks.
    pub fn refundable_total(&self) -> Result<ComponentDebitV1> {
        checked_component_sum(self.refundable_principal)
    }

    /// Checked aggregate donation disposition for reporting and post-delta checks.
    pub fn donation_total(&self) -> Result<ComponentDebitV1> {
        checked_component_sum(self.donation_residue)
    }
}

impl FixedCodec for SeriesFundingStateV1 {
    const ENCODED_LEN: usize = SERIES_FUNDING_STATE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_FUNDING_STATE_MAGIC);
        writer.u16(SERIES_FUNDING_STATE_SCHEMA);
        writer.reserved(6);
        writer.id(self.series_plan_id.content_id());
        writer.id(self.funding_terms_id.content_id());
        writer.id(self.funding_quote_id.content_id());
        writer.u32(self.instance_count);
        writer.u32(self.next_ordinal);
        writer.u32(self.lapsed_count);
        for component in self.components {
            write_debit(&mut writer, component.remaining_principal);
            write_debit(&mut writer, component.donations);
            writer.u32(component.consumed_allocations);
            writer.reserved(4);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SERIES_FUNDING_STATE_MAGIC)?;
        if reader.u16() != SERIES_FUNDING_STATE_SCHEMA {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let series_plan_id = SeriesPlanV5Id::from_bytes(reader.id().bytes());
        let funding_terms_id = SeriesFundingTermsV2Id::from_bytes(reader.id().bytes());
        let funding_quote_id = SeriesFundingQuoteId::from_bytes(reader.id().bytes());
        let instance_count = reader.u32();
        let next_ordinal = reader.u32();
        let lapsed_count = reader.u32();
        let mut components = [SeriesComponentCapitalV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            components[index] = SeriesComponentCapitalV1 {
                remaining_principal: read_debit(&mut reader),
                donations: read_debit(&mut reader),
                consumed_allocations: reader.u32(),
            };
            reader.reserved(4)?;
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            series_plan_id,
            funding_terms_id,
            funding_quote_id,
            instance_count,
            next_ordinal,
            lapsed_count,
            components,
        };
        value.validate()?;
        Ok(value)
    }
}

fn quote_components(
    quote: &SeriesFundingQuoteV1,
) -> [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT] {
    [
        quote.market_core,
        quote.recovery_reserve,
        quote.source_work,
        quote.liquidity_facility,
        quote.wrapper_set,
    ]
}

fn projection_components(
    projection: DebitProjectionV1,
) -> [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT] {
    [
        projection.market_core,
        projection.recovery_reserve,
        projection.source_work,
        projection.liquidity_facility,
        projection.wrapper_set,
    ]
}

fn fulfillment_absence(
    fulfillment: AdapterFulfillmentProjectionV2,
) -> [bool; SERIES_FUNDING_COMPONENT_COUNT] {
    [
        is_absent(fulfillment.market_core),
        is_absent(fulfillment.recovery_reserve),
        is_absent(fulfillment.source_work),
        is_absent(fulfillment.liquidity_facility),
        is_absent(fulfillment.wrapper_set),
    ]
}

fn is_absent(presence: ProjectedComponentPresenceV2) -> bool {
    presence == ProjectedComponentPresenceV2::Absent
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

fn checked_component_sum(
    components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT],
) -> Result<ComponentDebitV1> {
    let mut total = ComponentDebitV1::ZERO;
    let mut index = 0_usize;
    while index < SERIES_FUNDING_COMPONENT_COUNT {
        total = checked_add(total, components[index])?;
        index += 1;
    }
    Ok(total)
}

fn write_debit(writer: &mut Writer<'_>, value: ComponentDebitV1) {
    writer.u64(value.lamports);
    writer.u64(value.collateral_atoms);
}

fn read_debit(reader: &mut Reader<'_>) -> ComponentDebitV1 {
    ComponentDebitV1 {
        lamports: reader.u64(),
        collateral_atoms: reader.u64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvidenceOnlyRecoveryPolicyId, RecoveryAttemptFundingV1, MAX_RECOVERY_ATTEMPTS};

    fn quote() -> SeriesFundingQuoteV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 2,
            lamports_per_progress_unit: 3,
        };
        SeriesFundingQuoteV1 {
            evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes([1; 32]),
            market_core: ComponentDebitV1 {
                lamports: 10,
                collateral_atoms: 20,
            },
            failure_root_rent_principal_lamports: 3,
            failure_replay_tombstone_rent_principal_lamports: 2,
            recovery_reserve: ComponentDebitV1 {
                lamports: 13,
                collateral_atoms: 0,
            },
            source_work: ComponentDebitV1 {
                lamports: 30,
                collateral_atoms: 40,
            },
            liquidity_facility: ComponentDebitV1 {
                lamports: 50,
                collateral_atoms: 60,
            },
            wrapper_set: ComponentDebitV1 {
                lamports: 70,
                collateral_atoms: 80,
            },
            recovery_attempt_count: 1,
            recovery_attempt_funding: attempts,
            recovery_rent_principal_lamports: 7,
        }
    }

    fn state() -> SeriesFundingStateV1 {
        let quote = quote();
        let requirements = quote_components(&quote);
        let mut components = [SeriesComponentCapitalV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            components[index].remaining_principal = checked_mul(requirements[index], 2).unwrap();
            index += 1;
        }
        SeriesFundingStateV1 {
            series_plan_id: SeriesPlanV5Id::from_bytes([2; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([3; 32]),
            funding_quote_id: quote.id().unwrap(),
            instance_count: 2,
            next_ordinal: 0,
            lapsed_count: 0,
            components,
        }
    }

    fn funding_terms(series_plan_id: SeriesPlanV5Id) -> SeriesFundingTermsV2 {
        SeriesFundingTermsV2 {
            series_plan_id,
            lamport_principal_refund: ContentId::from_bytes([10; 32]),
            collateral_principal_refund_token_account: ContentId::from_bytes([11; 32]),
            neutral_collateral_disposition_token_account: ContentId::from_bytes([12; 32]),
            neutral_lamport_sink: ContentId::from_bytes([13; 32]),
            collateral_mint: ContentId::from_bytes([14; 32]),
            token_program: ContentId::from_bytes([15; 32]),
        }
    }

    #[test]
    fn funding_codec_is_exact_and_refuses_hostile_state() {
        let value = state();
        assert_eq!(value.validate_against_quote(&quote()), Ok(()));
        let mut bytes = [0; SERIES_FUNDING_STATE_BYTES];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(SeriesFundingStateV1::decode(&bytes), Ok(value));
        assert_eq!(
            SeriesFundingStateV1::decode_against_quote(&bytes, &quote()),
            Ok(value)
        );

        let mut bad = bytes;
        bad[10] = 1;
        assert_eq!(
            SeriesFundingStateV1::decode(&bad),
            Err(Error::NonCanonicalReserved)
        );
        assert_eq!(
            SeriesFundingStateV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::Truncated)
        );
        let mut trailing = [0; SERIES_FUNDING_STATE_BYTES + 1];
        trailing[..SERIES_FUNDING_STATE_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            SeriesFundingStateV1::decode(&trailing),
            Err(Error::TrailingBytes)
        );
    }

    #[test]
    fn quote_join_refuses_balance_or_paired_component_forgery() {
        let quote = quote();
        let mut bad_balance = state();
        bad_balance.components[SeriesFundingComponentV1::SourceWork.index()]
            .remaining_principal
            .lamports += 1;
        assert_eq!(
            bad_balance.validate_against_quote(&quote),
            Err(Error::InvalidComponentStatus)
        );

        let mut unpaired = state();
        unpaired.next_ordinal = 1;
        unpaired.components[SeriesFundingComponentV1::MarketCore.index()].consumed_allocations = 1;
        assert_eq!(unpaired.validate(), Err(Error::InvalidComponentStatus));

        let mut impossible_lapse = state();
        impossible_lapse.lapsed_count = 1;
        assert_eq!(impossible_lapse.validate(), Err(Error::InvalidSchedule));
    }

    #[test]
    fn lifecycle_counts_and_phase_are_derived_not_persisted_twice() {
        let mut value = state();
        value.next_ordinal = 1;
        value.lapsed_count = 1;
        assert_eq!(value.created_count(), Ok(0));
        assert_eq!(value.phase(), Ok(SeriesFundingPhaseV1::Active));
        assert_eq!(value.validate_against_quote(&quote()), Ok(()));

        value.next_ordinal = 2;
        value.lapsed_count = 2;
        assert_eq!(value.created_count(), Ok(0));
        assert_eq!(value.phase(), Ok(SeriesFundingPhaseV1::Closed));
        assert_eq!(value.validate_against_quote(&quote()), Ok(()));
    }

    #[test]
    fn terminal_projection_preserves_component_segregation() {
        let quote = quote();
        let mut value = state();
        let terms = funding_terms(value.series_plan_id);
        value.funding_terms_id = terms.id().unwrap();
        value.next_ordinal = 2;
        value.lapsed_count = 2;
        value.components[SeriesFundingComponentV1::WrapperSet.index()].donations =
            ComponentDebitV1 {
                lamports: 9,
                collateral_atoms: 8,
            };

        let projection = value.terminal_projection(&terms, &quote).unwrap();
        let quoted = quote_components(&quote);
        let mut expected = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT];
        let mut index = 0_usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT {
            expected[index] = checked_mul(quoted[index], 2).unwrap();
            index += 1;
        }
        assert_eq!(projection.refundable_principal, expected);
        assert_eq!(
            projection.donation_residue[SeriesFundingComponentV1::WrapperSet.index()],
            ComponentDebitV1 {
                lamports: 9,
                collateral_atoms: 8,
            }
        );
        assert_eq!(
            projection.donation_total(),
            Ok(ComponentDebitV1 {
                lamports: 9,
                collateral_atoms: 8,
            })
        );
    }

    #[test]
    fn funding_authority_defaults_to_refusal() {
        struct NoAuthority;
        impl AuthenticatedSeriesFundingAuthorityV1 for NoAuthority {}

        assert_eq!(
            NoAuthority.authenticated_current_bucket(&SeriesPlanV5 {
                product_template_id: crate::ProductTemplateId::from_bytes([1; 32]),
                market_genesis_profile_id: crate::MarketGenesisProfileV2Id::from_bytes([2; 32]),
                attachment_plan_id: crate::SeriesAttachmentPlanId::from_bytes([3; 32]),
                first_start_bucket: 10,
                stride_buckets: 0,
                instance_count: 1,
                creation_lead_buckets: 1,
                market_collateral_cap: 1,
            }),
            Err(Error::UnauthenticatedAuthority)
        );
    }
}
