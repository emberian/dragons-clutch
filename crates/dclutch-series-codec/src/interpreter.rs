use crate::{
    Action, Error, Identity, Phase, ReleaseReceiptV1, RequestV1, Result, SeriesStateV1, TemplateV1,
    TicketFundsV1, TicketPhase, TicketV1, generated_series as generated, wire::is_zero,
};

/// Named fixed-width execution profile. These are physical bounds, not Series
/// product restrictions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Exclusive slot bound.
    pub slot_limit: u64,
    /// Exclusive lamport/scalar bound.
    pub lamport_limit: u64,
    /// Exclusive replay-revision bound.
    pub revision_limit: u64,
}

/// Complete immutable input to one pure transition interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationV1 {
    /// Immutable template record.
    pub template: TemplateV1,
    /// Current Series cursor.
    pub series: SeriesStateV1,
    /// Current occurrence Ticket.
    pub ticket: TicketV1,
    /// Current Registry/Core receipt normalized by the adapter.
    pub release_receipt: ReleaseReceiptV1,
    /// Optimistic request.
    pub request: RequestV1,
    /// Named physical execution profile.
    pub limits: Limits,
}

/// Fixed account purpose used by exact custody plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountKind {
    /// Prepaid ticket escrow.
    TicketEscrow,
    /// Separately funded Series rent escrow.
    SeriesEscrow,
    /// New Market Hoard principal account.
    MarketHoard,
    /// New Market state account rent.
    MarketAccount,
    /// New capability-account rent collection.
    CapabilityAccounts,
    /// Persisted or permissionless beneficiary.
    Beneficiary,
}

/// One exact account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Account {
    /// Semantic account purpose.
    pub kind: AccountKind,
    /// Exact account or owner identity.
    pub identity: Identity,
}

/// One exact custody transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyTransfer {
    /// Debited compartment.
    pub source: Account,
    /// Credited compartment or beneficiary.
    pub destination: Account,
    /// Exact lamport quantity.
    pub amount: u64,
}

/// Shared-economic-kernel instruction for the initial native complete set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteSetSeed {
    /// Product outcome width interpreted dynamically by the shared kernel.
    pub outcome_count: u32,
    /// Same quantity minted in every outcome.
    pub quantity: u64,
    /// Initial native-claim holder.
    pub founder: Identity,
    /// Exact Market Hoard receiving matching principal.
    pub market_hoard: Identity,
}

/// Immutable Market founding commitment emitted only by Consume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundingV1 {
    /// Precommitted Market identity.
    pub market_id: Identity,
    /// Immutable template identity.
    pub template_id: Identity,
    /// Immutable Realm identity.
    pub realm_id: Identity,
    /// Product identity.
    pub product_id: Identity,
    /// Selected execution release-set identity.
    pub release_set_id: Identity,
    /// Zero-based occurrence.
    pub occurrence: u32,
    /// Exact due slot.
    pub scheduled_slot: u64,
    /// Shared complete-set seed instruction.
    pub complete_set_seed: CompleteSetSeed,
}

/// Owned atomic transition candidate. The adapter must either apply every
/// transfer, complete-set effect, account creation, and state byte together or
/// persist none of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCandidateV1 {
    /// Exact candidate Series cursor.
    pub series: SeriesStateV1,
    /// Exact candidate Ticket.
    pub ticket: TicketV1,
    /// At most the four mathematical funding compartments.
    pub transfers: [Option<CustodyTransfer>; 4],
    /// Number of populated leading transfers.
    pub transfer_count: u8,
    /// Market founding commitment for Consume only.
    pub market: Option<MarketFoundingV1>,
}

impl TemplateV1 {
    fn required_funds(self) -> TicketFundsV1 {
        TicketFundsV1 {
            hoard_principal: self.seed_quantity,
            market_rent: self.market_rent_lamports,
            capability_rent: self.capability_rent_lamports,
            founding_work: self.founding_work_lamports,
        }
    }

    fn validate_profile(self, limits: Limits) -> Result<()> {
        self.validate_basic()?;
        if limits.slot_limit == 0 || limits.lamport_limit == 0 || limits.revision_limit == 0 {
            return Err(Error::ZeroQuantity);
        }
        if self.seed_quantity >= limits.lamport_limit
            || self.series_close_rent_lamports >= limits.lamport_limit
        {
            return Err(Error::ProfileBound);
        }
        let total = self
            .seed_quantity
            .checked_add(self.market_rent_lamports)
            .and_then(|v| v.checked_add(self.capability_rent_lamports))
            .and_then(|v| v.checked_add(self.founding_work_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        if total >= limits.lamport_limit {
            return Err(Error::ProfileBound);
        }
        let last = self
            .occurrence_count
            .checked_sub(1)
            .ok_or(Error::ZeroQuantity)?;
        let (_, retry_through) = generated::schedule_window(
            self.first_occurrence_slot,
            self.period_slots,
            last,
            self.retry_window_slots,
        )
        .ok_or(Error::ArithmeticOverflow)?;
        if retry_through >= limits.slot_limit {
            return Err(Error::ProfileBound);
        }
        Ok(())
    }
}

/// Interpret one total Series transition without allocating or mutating input.
pub fn interpret(invocation: InvocationV1) -> Result<AtomicCandidateV1> {
    validate_common(invocation)?;
    match invocation.request.action {
        Action::Consume => consume(invocation),
        Action::Expire => expire(invocation),
        Action::Close => close(invocation),
    }
}

fn validate_common(invocation: InvocationV1) -> Result<()> {
    let InvocationV1 {
        template,
        series,
        ticket,
        release_receipt,
        request,
        limits,
    } = invocation;
    template.validate_profile(limits)?;
    if request.now_slot >= limits.slot_limit {
        return Err(Error::ProfileBound);
    }
    if series.template_id != template.template_id || ticket.template_id != template.template_id {
        return Err(Error::IdentityMismatch);
    }
    if release_receipt.release_set_id != template.release_set_id
        || release_receipt.registry_program != release_receipt.observed_program
    {
        return Err(Error::ReleaseAdmission);
    }
    if request.expected_series_revision != series.revision
        || request.expected_ticket_revision != ticket.revision
    {
        return Err(Error::RevisionMismatch);
    }
    series
        .revision
        .checked_add(1)
        .filter(|next| *next < limits.revision_limit)
        .ok_or(Error::ProfileBound)?;
    ticket
        .revision
        .checked_add(1)
        .filter(|next| *next < limits.revision_limit)
        .ok_or(Error::ProfileBound)?;
    if ticket.occurrence >= template.occurrence_count {
        return Err(Error::InvalidState);
    }
    match ticket.phase {
        TicketPhase::Ready if ticket.funds != template.required_funds() => {
            return Err(Error::InvalidState);
        }
        TicketPhase::Consumed | TicketPhase::Expired if !ticket.funds.is_zero() => {
            return Err(Error::InvalidState);
        }
        _ => {}
    }
    match series.phase {
        Phase::Active => {
            if series.next_occurrence >= template.occurrence_count
                || series.close_rent_lamports != template.series_close_rent_lamports
            {
                return Err(Error::InvalidState);
            }
            let ticket_projection = match ticket.phase {
                TicketPhase::Ready => ticket.occurrence == series.next_occurrence,
                TicketPhase::Consumed | TicketPhase::Expired => {
                    ticket.occurrence < series.next_occurrence
                }
            };
            if !ticket_projection {
                return Err(Error::InvalidState);
            }
        }
        Phase::Terminal => {
            if series.next_occurrence != template.occurrence_count
                || series.close_rent_lamports != template.series_close_rent_lamports
                || !ticket.phase.is_final()
            {
                return Err(Error::InvalidState);
            }
        }
        Phase::Closed => {
            if series.next_occurrence != template.occurrence_count
                || series.close_rent_lamports != 0
                || !ticket.phase.is_final()
            {
                return Err(Error::InvalidState);
            }
        }
    }
    Ok(())
}

fn consume(invocation: InvocationV1) -> Result<AtomicCandidateV1> {
    let InvocationV1 {
        template,
        series,
        ticket,
        request,
        ..
    } = invocation;
    if series.phase != Phase::Active
        || ticket.phase != TicketPhase::Ready
        || ticket.occurrence != series.next_occurrence
    {
        return Err(Error::InvalidState);
    }
    if is_zero(&request.work_recipient) {
        return Err(Error::RecipientRefusal);
    }
    let (due, retry_through) = generated::schedule_window(
        template.first_occurrence_slot,
        template.period_slots,
        ticket.occurrence,
        template.retry_window_slots,
    )
    .ok_or(Error::ArithmeticOverflow)?;
    if request.now_slot < due || request.now_slot > retry_through {
        return Err(Error::ScheduleRefusal);
    }
    let (next_series, next_ticket) = advance(template, series, ticket, TicketPhase::Consumed)?;
    let mut candidate = empty_candidate(next_series, next_ticket);
    let source = Account {
        kind: AccountKind::TicketEscrow,
        identity: ticket.ticket_id,
    };
    push_transfer(
        &mut candidate,
        source,
        Account {
            kind: AccountKind::MarketHoard,
            identity: ticket.committed_market_id,
        },
        ticket.funds.hoard_principal,
    )?;
    push_transfer(
        &mut candidate,
        source,
        Account {
            kind: AccountKind::MarketAccount,
            identity: ticket.committed_market_id,
        },
        ticket.funds.market_rent,
    )?;
    push_transfer(
        &mut candidate,
        source,
        Account {
            kind: AccountKind::CapabilityAccounts,
            identity: ticket.committed_market_id,
        },
        ticket.funds.capability_rent,
    )?;
    push_transfer(
        &mut candidate,
        source,
        Account {
            kind: AccountKind::Beneficiary,
            identity: request.work_recipient,
        },
        ticket.funds.founding_work,
    )?;
    candidate.market = Some(MarketFoundingV1 {
        market_id: ticket.committed_market_id,
        template_id: template.template_id,
        realm_id: template.realm_id,
        product_id: template.product_id,
        release_set_id: template.release_set_id,
        occurrence: ticket.occurrence,
        scheduled_slot: due,
        complete_set_seed: CompleteSetSeed {
            outcome_count: template.outcome_count,
            quantity: template.seed_quantity,
            founder: ticket.founder,
            market_hoard: ticket.committed_market_id,
        },
    });
    Ok(candidate)
}

fn expire(invocation: InvocationV1) -> Result<AtomicCandidateV1> {
    let InvocationV1 {
        template,
        series,
        ticket,
        request,
        ..
    } = invocation;
    if series.phase != Phase::Active
        || ticket.phase != TicketPhase::Ready
        || ticket.occurrence != series.next_occurrence
    {
        return Err(Error::InvalidState);
    }
    let (_, retry_through) = generated::schedule_window(
        template.first_occurrence_slot,
        template.period_slots,
        ticket.occurrence,
        template.retry_window_slots,
    )
    .ok_or(Error::ArithmeticOverflow)?;
    if request.now_slot <= retry_through {
        return Err(Error::ScheduleRefusal);
    }
    let (next_series, next_ticket) = advance(template, series, ticket, TicketPhase::Expired)?;
    let mut candidate = empty_candidate(next_series, next_ticket);
    let source = Account {
        kind: AccountKind::TicketEscrow,
        identity: ticket.ticket_id,
    };
    let refund = Account {
        kind: AccountKind::Beneficiary,
        identity: ticket.refund_owner,
    };
    push_transfer(&mut candidate, source, refund, ticket.funds.hoard_principal)?;
    push_transfer(&mut candidate, source, refund, ticket.funds.market_rent)?;
    push_transfer(&mut candidate, source, refund, ticket.funds.capability_rent)?;
    push_transfer(&mut candidate, source, refund, ticket.funds.founding_work)?;
    Ok(candidate)
}

fn close(invocation: InvocationV1) -> Result<AtomicCandidateV1> {
    let InvocationV1 {
        template,
        mut series,
        ticket,
        ..
    } = invocation;
    if series.phase != Phase::Terminal
        || series.next_occurrence != template.occurrence_count
        || !ticket.phase.is_final()
    {
        return Err(Error::InvalidState);
    }
    let close_rent = series.close_rent_lamports;
    series.phase = Phase::Closed;
    series.revision = series
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    series.close_rent_lamports = 0;
    let mut candidate = empty_candidate(series, ticket);
    push_transfer(
        &mut candidate,
        Account {
            kind: AccountKind::SeriesEscrow,
            identity: series.series_id,
        },
        Account {
            kind: AccountKind::Beneficiary,
            identity: template.series_refund_owner,
        },
        close_rent,
    )?;
    Ok(candidate)
}

fn advance(
    template: TemplateV1,
    mut series: SeriesStateV1,
    mut ticket: TicketV1,
    ticket_phase: TicketPhase,
) -> Result<(SeriesStateV1, TicketV1)> {
    let next = series
        .next_occurrence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let phase_tag =
        generated::next_series_phase(next, template.occurrence_count).ok_or(Error::InvalidState)?;
    series.phase = Phase::decode(phase_tag)?;
    series.next_occurrence = next;
    series.revision = series
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    ticket.phase = ticket_phase;
    ticket.revision = ticket
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    ticket.funds = TicketFundsV1::default();
    Ok((series, ticket))
}

const fn empty_candidate(series: SeriesStateV1, ticket: TicketV1) -> AtomicCandidateV1 {
    AtomicCandidateV1 {
        series,
        ticket,
        transfers: [None; 4],
        transfer_count: 0,
        market: None,
    }
}

fn push_transfer(
    candidate: &mut AtomicCandidateV1,
    source: Account,
    destination: Account,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let index = usize::from(candidate.transfer_count);
    let slot = candidate
        .transfers
        .get_mut(index)
        .ok_or(Error::TransferBound)?;
    *slot = Some(CustodyTransfer {
        source,
        destination,
        amount,
    });
    candidate.transfer_count = candidate
        .transfer_count
        .checked_add(1)
        .ok_or(Error::TransferBound)?;
    Ok(())
}
