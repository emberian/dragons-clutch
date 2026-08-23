use crate::{ContentId, Error, Result, SeriesAttachmentPlanV1};

/// One component's exact lamport and collateral-atom requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentDebitV1 {
    /// Independently owned lamports required by this component.
    pub lamports: u64,
    /// Series-vault collateral atoms required by this component.
    pub collateral_atoms: u64,
}

impl ComponentDebitV1 {
    /// No debit.
    pub const ZERO: Self = Self {
        lamports: 0,
        collateral_atoms: 0,
    };

    fn checked_add(self, other: Self) -> Result<Self> {
        Ok(Self {
            lamports: self
                .lamports
                .checked_add(other.lamports)
                .ok_or(Error::ArithmeticOverflow)?,
            collateral_atoms: self
                .collateral_atoms
                .checked_add(other.collateral_atoms)
                .ok_or(Error::ArithmeticOverflow)?,
        })
    }
}

/// Adapter-authenticated exact per-occurrence funding quote projection.
///
/// The funding-quote artifact remains the semantic owner of these amounts.
/// This core accepts the projection only when its identity exactly matches the
/// Series attachment plan; it does not define a second persisted quote codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFundingQuoteV1 {
    /// Exact immutable quote artifact identity authenticated by the adapter.
    pub funding_quote_id: ContentId,
    /// Market root, child-plane, mint, and custody-account creation principal.
    pub market_core: ComponentDebitV1,
    /// Independently prepaid finite evidence-recovery reserve.
    pub recovery_reserve: ComponentDebitV1,
    /// Source ingestion, archive, window, and evaluation work.
    pub source_work: ComponentDebitV1,
    /// Liquidity-facility attachment capital and account principal.
    pub liquidity_facility: ComponentDebitV1,
    /// Canonical structured-wrapper descriptor/mint/vault attachments.
    pub wrapper_set: ComponentDebitV1,
}

impl AuthenticatedFundingQuoteV1 {
    fn validate_binding(&self, attachment: &SeriesAttachmentPlanV1) -> Result<()> {
        attachment.validate()?;
        self.funding_quote_id.validate()?;
        if self.funding_quote_id != attachment.funding_quote_id {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }
}

/// Exact authenticated existence state for one component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentStatusV1 {
    /// The component is absent and its exact quote must be debited.
    Absent,
    /// The component exists and the adapter authenticated its exact identity/body.
    PresentExact,
}

/// Exact-existing versus absent state for all independently debited components.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FulfillmentStatusV1 {
    /// Economic market root and mandatory genesis plane.
    pub market_core: ComponentStatusV1,
    /// Mandatory recovery state/reserve belonging to that market.
    pub recovery_reserve: ComponentStatusV1,
    /// Shared source/window/evaluator work.
    pub source_work: ComponentStatusV1,
    /// Liquidity-facility attachment.
    pub liquidity_facility: ComponentStatusV1,
    /// Canonical structured-wrapper set.
    pub wrapper_set: ComponentStatusV1,
}

impl FulfillmentStatusV1 {
    fn validate(self) -> Result<()> {
        if self.market_core != self.recovery_reserve {
            return Err(Error::InvalidComponentStatus);
        }
        Ok(())
    }
}

/// Segregated principal available before one projected fulfillment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingBalancesV1 {
    /// Available refundable lamport principal.
    pub lamports: u64,
    /// Available refundable Series-vault collateral principal.
    pub collateral_atoms: u64,
}

/// Exact pure debit projection; an adapter applies it atomically or not at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebitProjectionV1 {
    /// Market-core debit, or zero when an exact market already exists.
    pub market_core: ComponentDebitV1,
    /// Recovery-reserve debit, paired with the market-core branch.
    pub recovery_reserve: ComponentDebitV1,
    /// Source-work debit, or zero for exact shared work.
    pub source_work: ComponentDebitV1,
    /// Liquidity-facility debit, or zero for an exact existing facility.
    pub liquidity_facility: ComponentDebitV1,
    /// Wrapper-set debit, or zero for exact canonical wrappers.
    pub wrapper_set: ComponentDebitV1,
    /// Checked sum of all component debits.
    pub total: ComponentDebitV1,
    /// Remaining refundable segregated principal after the projection.
    pub remaining: FundingBalancesV1,
}

fn selected(amount: ComponentDebitV1, status: ComponentStatusV1) -> ComponentDebitV1 {
    match status {
        ComponentStatusV1::Absent => amount,
        ComponentStatusV1::PresentExact => ComponentDebitV1::ZERO,
    }
}

/// Project exact per-component spending for an absent/exact-existing occurrence.
///
/// A mismatch is not a status: adapters must refuse it before calling this
/// function. Market core and its mandatory recovery state are created/reused as
/// one branch, while source, liquidity, and wrapper components converge
/// independently. The returned projection performs no external mutation.
pub fn project_component_debits(
    attachment: &SeriesAttachmentPlanV1,
    quote: &AuthenticatedFundingQuoteV1,
    status: FulfillmentStatusV1,
    available: FundingBalancesV1,
) -> Result<DebitProjectionV1> {
    quote.validate_binding(attachment)?;
    status.validate()?;
    let market_core = selected(quote.market_core, status.market_core);
    let recovery_reserve = selected(quote.recovery_reserve, status.recovery_reserve);
    let source_work = selected(quote.source_work, status.source_work);
    let liquidity_facility = selected(quote.liquidity_facility, status.liquidity_facility);
    let wrapper_set = selected(quote.wrapper_set, status.wrapper_set);
    let total = market_core
        .checked_add(recovery_reserve)?
        .checked_add(source_work)?
        .checked_add(liquidity_facility)?
        .checked_add(wrapper_set)?;
    let remaining = FundingBalancesV1 {
        lamports: available
            .lamports
            .checked_sub(total.lamports)
            .ok_or(Error::InsufficientPrepayment)?,
        collateral_atoms: available
            .collateral_atoms
            .checked_sub(total.collateral_atoms)
            .ok_or(Error::InsufficientPrepayment)?,
    };
    Ok(DebitProjectionV1 {
        market_core,
        recovery_reserve,
        source_work,
        liquidity_facility,
        wrapper_set,
        total,
        remaining,
    })
}
