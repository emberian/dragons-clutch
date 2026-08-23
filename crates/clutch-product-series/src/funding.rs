use crate::codec::{Reader, Writer};
use crate::{
    content_id, Error, EvidenceOnlyRecoveryPolicyId, EvidenceOnlyRecoveryPolicyV1, FixedCodec,
    MarketInstanceId, Result, SeriesAttachmentPlanId, SeriesAttachmentPlanV1, SeriesFundingQuoteId,
    MAX_RECOVERY_ATTEMPTS,
};

const FUNDING_QUOTE_MAGIC: [u8; 8] = *b"DCFQUOT1";
const SCHEMA_V1: u16 = 1;

/// SHA-256 domain for [`SeriesFundingQuoteV1`].
pub const SERIES_FUNDING_QUOTE_DOMAIN: &[u8] = b"dragons-clutch/series-funding-quote/v1";
/// Exact canonical byte length of [`SeriesFundingQuoteV1`].
pub const SERIES_FUNDING_QUOTE_BYTES: usize = 280;

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

/// Exact accepted-progress cap and price for one recovery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAttemptFundingV1 {
    /// Maximum cumulative accepted progress units in this attempt.
    pub max_progress_units: u64,
    /// Exact lamports paid per newly accepted progress unit.
    pub lamports_per_progress_unit: u64,
}

impl RecoveryAttemptFundingV1 {
    /// Canonical inactive fixed-array padding.
    pub const ZERO: Self = Self {
        max_progress_units: 0,
        lamports_per_progress_unit: 0,
    };

    /// Maximum lamports payable by this active attempt.
    pub fn maximum_lamports(self) -> Result<u64> {
        self.max_progress_units
            .checked_mul(self.lamports_per_progress_unit)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Exact per-occurrence funding quote and sole owner of component amounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV1 {
    /// Exact evidence-only recovery policy priced by this quote.
    pub evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Market root, child-plane, mint, and custody-account creation principal.
    pub market_core: ComponentDebitV1,
    /// Exact refundable rent principal for the occurrence's failure root.
    pub failure_root_rent_principal_lamports: u64,
    /// Exact refundable rent principal for its permanent replay tombstone.
    pub failure_replay_tombstone_rent_principal_lamports: u64,
    /// Independently prepaid finite evidence-recovery reserve.
    pub recovery_reserve: ComponentDebitV1,
    /// Source ingestion, archive, window, and evaluation work.
    pub source_work: ComponentDebitV1,
    /// Liquidity-facility attachment capital and account principal.
    pub liquidity_facility: ComponentDebitV1,
    /// Canonical structured-wrapper descriptor/mint/vault attachments.
    pub wrapper_set: ComponentDebitV1,
    /// Active recovery attempt-funding count.
    pub recovery_attempt_count: u8,
    /// Exact active attempt funding followed by canonical zero padding.
    pub recovery_attempt_funding: [RecoveryAttemptFundingV1; MAX_RECOVERY_ATTEMPTS],
    /// Exact separately owned recovery-reserve rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV1 {
    fn checked_recovery_work_principal_lamports(&self) -> Result<u64> {
        let count = usize::from(self.recovery_attempt_count);
        let mut total = 0_u64;
        let mut index = 0_usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let terms = self.recovery_attempt_funding[index];
            if index < count {
                if terms.max_progress_units == 0 || terms.lamports_per_progress_unit == 0 {
                    return Err(Error::InvalidParameter);
                }
                total = total
                    .checked_add(terms.maximum_lamports()?)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if terms != RecoveryAttemptFundingV1::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if total == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(total)
    }

    /// Validate the quote's canonical shape and exact recovery decomposition.
    pub fn validate(&self) -> Result<()> {
        self.evidence_only_recovery_policy_id.validate()?;
        let count = usize::from(self.recovery_attempt_count);
        if count == 0
            || count > MAX_RECOVERY_ATTEMPTS
            || self.recovery_rent_principal_lamports == 0
            || self.recovery_reserve.collateral_atoms != 0
        {
            return Err(Error::InvalidParameter);
        }
        let failure_rent = self
            .failure_root_rent_principal_lamports
            .checked_add(self.failure_replay_tombstone_rent_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if (self.market_core.lamports == 0 && failure_rent != 0)
            || (self.market_core.lamports != 0
                && (self.failure_root_rent_principal_lamports == 0
                    || self.failure_replay_tombstone_rent_principal_lamports == 0
                    || failure_rent > self.market_core.lamports))
        {
            return Err(Error::InvalidParameter);
        }
        let work = self.checked_recovery_work_principal_lamports()?;
        if work
            .checked_add(self.recovery_rent_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?
            != self.recovery_reserve.lamports
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Exact work principal derived from the authoritative attempt rows.
    pub fn recovery_work_principal_lamports(&self) -> Result<u64> {
        self.validate()?;
        self.checked_recovery_work_principal_lamports()
    }

    /// Bind priced attempts to the exact reusable evidence-only policy.
    pub fn validate_recovery_binding(&self, recovery: &EvidenceOnlyRecoveryPolicyV1) -> Result<()> {
        self.validate()?;
        recovery.validate()?;
        if self.evidence_only_recovery_policy_id != recovery.id()?
            || self.recovery_attempt_count != recovery.attempt_count
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Typed digest of the exact canonical component amounts.
    pub fn id(&self) -> Result<SeriesFundingQuoteId> {
        let mut body = [0; SERIES_FUNDING_QUOTE_BYTES];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteId::from_bytes(
            content_id(SERIES_FUNDING_QUOTE_DOMAIN, &body).bytes(),
        ))
    }

    fn validate_binding(&self, attachment: &SeriesAttachmentPlanV1) -> Result<()> {
        attachment.validate()?;
        if self.id()? != attachment.funding_quote_id {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }
}

impl FixedCodec for SeriesFundingQuoteV1 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&FUNDING_QUOTE_MAGIC);
        writer.u16(SCHEMA_V1);
        writer.u8(self.recovery_attempt_count);
        writer.reserved(5);
        writer.id(self.evidence_only_recovery_policy_id.content_id());
        for component in [
            self.market_core,
            self.recovery_reserve,
            self.source_work,
            self.liquidity_facility,
            self.wrapper_set,
        ] {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        writer.u64(self.failure_root_rent_principal_lamports);
        writer.u64(self.failure_replay_tombstone_rent_principal_lamports);
        writer.u64(self.recovery_rent_principal_lamports);
        for terms in self.recovery_attempt_funding {
            writer.u64(terms.max_progress_units);
            writer.u64(terms.lamports_per_progress_unit);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&FUNDING_QUOTE_MAGIC)?;
        if reader.u16() != SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let recovery_attempt_count = reader.u8();
        reader.reserved(5)?;
        let evidence_only_recovery_policy_id =
            EvidenceOnlyRecoveryPolicyId::from_bytes(reader.id().bytes());
        let mut component = || ComponentDebitV1 {
            lamports: reader.u64(),
            collateral_atoms: reader.u64(),
        };
        let value = Self {
            evidence_only_recovery_policy_id,
            market_core: component(),
            recovery_reserve: component(),
            source_work: component(),
            liquidity_facility: component(),
            wrapper_set: component(),
            failure_root_rent_principal_lamports: reader.u64(),
            failure_replay_tombstone_rent_principal_lamports: reader.u64(),
            recovery_attempt_count,
            recovery_rent_principal_lamports: reader.u64(),
            recovery_attempt_funding: {
                let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
                let mut index = 0_usize;
                while index < MAX_RECOVERY_ATTEMPTS {
                    attempts[index] = RecoveryAttemptFundingV1 {
                        max_progress_units: reader.u64(),
                        lamports_per_progress_unit: reader.u64(),
                    };
                    index += 1;
                }
                attempts
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Exact authenticated existence state for one component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterAuthenticatedComponentStatusV1 {
    /// The component is absent and its exact quote must be debited.
    Absent,
    /// The component exists and the adapter authenticated its exact identity/body.
    PresentExactAndCapitalized,
}

/// Exact-existing versus absent state for all independently debited components.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterAuthenticatedFulfillmentStatusV1 {
    /// Exact economic occurrence whose accounts were inspected.
    pub market_instance_id: MarketInstanceId,
    /// Exact operational attachment whose accounts were inspected.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Exact quote whose component balances were inspected.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Economic market root and mandatory genesis plane.
    pub market_core: AdapterAuthenticatedComponentStatusV1,
    /// Mandatory recovery state/reserve belonging to that market.
    pub recovery_reserve: AdapterAuthenticatedComponentStatusV1,
    /// Shared source/window/evaluator work.
    pub source_work: AdapterAuthenticatedComponentStatusV1,
    /// Liquidity-facility attachment.
    pub liquidity_facility: AdapterAuthenticatedComponentStatusV1,
    /// Canonical structured-wrapper set.
    pub wrapper_set: AdapterAuthenticatedComponentStatusV1,
}

impl AdapterAuthenticatedFulfillmentStatusV1 {
    fn validate(
        self,
        expected_market_instance_id: MarketInstanceId,
        attachment: &SeriesAttachmentPlanV1,
        quote: &SeriesFundingQuoteV1,
    ) -> Result<()> {
        if self.market_instance_id != expected_market_instance_id
            || self.attachment_plan_id != attachment.id()?
            || self.funding_quote_id != quote.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
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

fn selected(
    amount: ComponentDebitV1,
    status: AdapterAuthenticatedComponentStatusV1,
) -> ComponentDebitV1 {
    match status {
        AdapterAuthenticatedComponentStatusV1::Absent => amount,
        AdapterAuthenticatedComponentStatusV1::PresentExactAndCapitalized => ComponentDebitV1::ZERO,
    }
}

/// Project exact per-component spending for an absent/exact-existing occurrence.
///
/// A mismatch is not a status: adapters must refuse it before calling this
/// function. Market core and its mandatory recovery state are created/reused as
/// one branch, while source, liquidity, and wrapper components converge
/// independently. The returned projection performs no external mutation.
pub fn project_component_debits(
    market_instance_id: MarketInstanceId,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    attachment: &SeriesAttachmentPlanV1,
    quote: &SeriesFundingQuoteV1,
    status: AdapterAuthenticatedFulfillmentStatusV1,
    available: FundingBalancesV1,
) -> Result<DebitProjectionV1> {
    quote.validate_recovery_binding(recovery)?;
    quote.validate_binding(attachment)?;
    status.validate(market_instance_id, attachment, quote)?;
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
