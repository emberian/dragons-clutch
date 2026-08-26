//! Total commit-last plans for the executable recurring-Series lifecycle.
//!
//! Plans never mutate account bytes. The SBF adapter stages physical transfers
//! and Core CPI first, validates the immediate Core acknowledgement, then
//! persists the candidate replay bytes. Any CPI refusal therefore leaves the
//! same Series and Ticket revisions retryable under Solana transaction rollback.

use dclutch_capability_contract::{
    CapabilityManifestV1, FundingCustodyObservationV1, FundingStateV1, FundingStatus,
    RealmCollateralCustodyV1,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Identity as CoreIdentity, SeriesCoreAckV1, SeriesCoreActionV1, SeriesCoreRequestV1,
};
use solana_program::pubkey::Pubkey;

use super::{
    AdmittedOccurrenceV2, AdmittedTicketV2, OccurrenceV2, SeriesV2Error, TemplateV2, core_request,
    funding_list_id, pubkey,
    state::{SeriesStateError, SeriesStateV2, TicketPhaseV2, TicketStateV2},
};

const MAXIMUM_FUNDING_STATES: usize = 16;

/// Refusal from an executable lifecycle plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleErrorV2 {
    /// Immutable Template, occurrence, Ticket, or Market admission refused.
    Content,
    /// Mutable phase, revision, or occurrence cursor refused.
    Replay,
    /// Current Clock slot is outside the action's exact window.
    Schedule,
    /// A native or Realm-collateral compartment was reclassified or mismatched.
    Funding,
    /// The Core acknowledgement did not bind the exact request and poststate.
    CoreAck,
    /// Checked fixed-width arithmetic overflowed.
    Arithmetic,
}

impl From<SeriesV2Error> for LifecycleErrorV2 {
    fn from(_: SeriesV2Error) -> Self {
        Self::Content
    }
}

impl From<SeriesStateError> for LifecycleErrorV2 {
    fn from(_: SeriesStateError) -> Self {
        Self::Replay
    }
}

/// One proposed Trading-owned FundingState creation destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingFundingAccountV2 {
    key: Pubkey,
    state: FundingStateV1,
    current_lamports: u64,
    exact_state_rent: u64,
    realm_collateral: Option<RealmCollateralCustodyV1>,
}

impl PendingFundingAccountV2 {
    /// Bind the exact planned FundingState bytes and observed physical custody.
    pub fn new(
        key: Pubkey,
        state: FundingStateV1,
        current_lamports: u64,
        exact_state_rent: u64,
        realm_collateral: Option<RealmCollateralCustodyV1>,
    ) -> Result<Self, LifecycleErrorV2> {
        if key == Pubkey::default() {
            return Err(LifecycleErrorV2::Funding);
        }
        Ok(Self {
            key,
            state,
            current_lamports,
            exact_state_rent,
            realm_collateral,
        })
    }

    /// Planned FundingState PDA key.
    pub const fn key(self) -> Pubkey {
        self.key
    }
    /// Exact pending canonical state to persist after account creation.
    pub const fn state(self) -> FundingStateV1 {
        self.state
    }
}

/// Exact bounded native funding distribution for all occurrence capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingFundingPlanV2 {
    count: u8,
    top_up: [u64; MAXIMUM_FUNDING_STATES],
    preexisting_surplus_refund: [u64; MAXIMUM_FUNDING_STATES],
    ticket_capability_refund: u64,
    required_native: u64,
}

impl PendingFundingPlanV2 {
    /// Number of exact ordered FundingState destinations.
    pub const fn count(self) -> u8 {
        self.count
    }
    /// Ticket lamports transferred to one FundingState before its bytes commit.
    pub fn top_up(self, index: usize) -> Option<u64> {
        if index >= usize::from(self.count) {
            return None;
        }
        self.top_up.get(index).copied()
    }
    /// Pre-existing PDA dust explicitly returned rather than treated as funding.
    pub fn preexisting_surplus_refund(self, index: usize) -> Option<u64> {
        if index >= usize::from(self.count) {
            return None;
        }
        self.preexisting_surplus_refund.get(index).copied()
    }
    /// Committed Ticket capability lamports not needed because PDA dust paid part.
    pub const fn ticket_capability_refund(self) -> u64 {
        self.ticket_capability_refund
    }
    /// Exact sum of state Rent reserves and semantic native funding required.
    pub const fn required_native(self) -> u64 {
        self.required_native
    }
}

/// Validate exact ordered pending FundingStates and plan dust-tolerant funding.
pub fn plan_pending_funding(
    occurrence: OccurrenceV2,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    accounts: &[PendingFundingAccountV2],
) -> Result<PendingFundingPlanV2, LifecycleErrorV2> {
    if accounts.is_empty() || accounts.len() > MAXIMUM_FUNDING_STATES {
        return Err(LifecycleErrorV2::Funding);
    }
    let mut keys = [Pubkey::default(); MAXIMUM_FUNDING_STATES];
    let mut top_up = [0_u64; MAXIMUM_FUNDING_STATES];
    let mut surplus = [0_u64; MAXIMUM_FUNDING_STATES];
    let mut required_native = 0_u64;
    let mut transferred = 0_u64;
    let mut previous_entry: Option<u16> = None;

    for (index, account) in accounts.iter().copied().enumerate() {
        *keys.get_mut(index).ok_or(LifecycleErrorV2::Funding)? = account.key;
        let state = account.state;
        if state.status() != FundingStatus::Pending
            || state.manifest_content_id() != manifest_id
            || previous_entry.is_some_and(|previous| previous >= state.entry_index())
        {
            return Err(LifecycleErrorV2::Funding);
        }
        let required = account
            .exact_state_rent
            .checked_add(state.remaining().native_lamports_total())
            .ok_or(LifecycleErrorV2::Arithmetic)?;
        let desired_custody = match account.realm_collateral {
            Some(realm) => FundingCustodyObservationV1::with_realm_collateral(
                required,
                account.exact_state_rent,
                realm,
            ),
            None => FundingCustodyObservationV1::native_only(required, account.exact_state_rent),
        }
        .map_err(|_| LifecycleErrorV2::Funding)?;
        state
            .validate_against(manifest_id, manifest, desired_custody)
            .map_err(|_| LifecycleErrorV2::Funding)?;
        let (top_up_value, surplus_value) = if account.current_lamports <= required {
            (required - account.current_lamports, 0)
        } else {
            (0, account.current_lamports - required)
        };
        *top_up.get_mut(index).ok_or(LifecycleErrorV2::Funding)? = top_up_value;
        *surplus.get_mut(index).ok_or(LifecycleErrorV2::Funding)? = surplus_value;
        required_native = required_native
            .checked_add(required)
            .ok_or(LifecycleErrorV2::Arithmetic)?;
        transferred = transferred
            .checked_add(top_up_value)
            .ok_or(LifecycleErrorV2::Arithmetic)?;
        previous_entry = Some(state.entry_index());
    }
    if funding_list_id(
        keys.get(..accounts.len())
            .ok_or(LifecycleErrorV2::Funding)?,
    )? != occurrence.funding_list()
        || required_native != occurrence.funds().capability_native()
    {
        return Err(LifecycleErrorV2::Funding);
    }
    Ok(PendingFundingPlanV2 {
        count: u8::try_from(accounts.len()).map_err(|_| LifecycleErrorV2::Funding)?,
        top_up,
        preexisting_surplus_refund: surplus,
        ticket_capability_refund: required_native
            .checked_sub(transferred)
            .ok_or(LifecycleErrorV2::Arithmetic)?,
        required_native,
    })
}

/// Candidate bytes and Core request for one commit-last occurrence transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceCommitPlanV2 {
    core_request: Option<SeriesCoreRequestV1>,
    series_after: SeriesStateV2,
    ticket_after: TicketStateV2,
    occurrence_count: u32,
    native_from_ticket: u64,
    funding: Option<PendingFundingPlanV2>,
}

impl OccurrenceCommitPlanV2 {
    /// Exact 336-byte Core request, present only for atomic Consume/Found.
    pub const fn core_request(self) -> Option<SeriesCoreRequestV1> {
        self.core_request
    }
    /// Candidate Series state; not persisted before Core acknowledgement.
    pub const fn series_after(self) -> SeriesStateV2 {
        self.series_after
    }
    /// Candidate Ticket state; not persisted before Core acknowledgement.
    pub const fn ticket_after(self) -> TicketStateV2 {
        self.ticket_after
    }
    /// Exact native lamports drained from Ticket custody on success.
    pub const fn native_from_ticket(self) -> u64 {
        self.native_from_ticket
    }
    /// Exact FundingState distribution, present only for consumption.
    pub const fn funding(self) -> Option<PendingFundingPlanV2> {
        self.funding
    }

    /// Validate immediate Core return data and expose the only permitted writes.
    pub fn commit_after_ack(
        self,
        ack: SeriesCoreAckV1,
        expected_core_program: CoreIdentity,
        request_digest: CoreIdentity,
        observed_post_resource_digest: CoreIdentity,
    ) -> Result<([u8; 64], [u8; 64]), LifecycleErrorV2> {
        let request = self.core_request.ok_or(LifecycleErrorV2::CoreAck)?;
        ack.validate_for(
            request,
            expected_core_program,
            request_digest,
            observed_post_resource_digest,
        )
        .map_err(|_| LifecycleErrorV2::CoreAck)?;
        Ok((
            self.series_after.encode(self.occurrence_count)?,
            self.ticket_after.encode(),
        ))
    }

    /// Expose controller-owned candidate bytes for Prepare or Expire.
    ///
    /// The physical outer calls this only after every direct Trading-owned
    /// account operation and any current-Custody receipt have authenticated.
    /// Consume cannot bypass its Core acknowledgement through this route.
    pub fn commit_controller(self) -> Result<([u8; 64], [u8; 64]), LifecycleErrorV2> {
        if self.core_request.is_some() {
            return Err(LifecycleErrorV2::CoreAck);
        }
        Ok((
            self.series_after.encode(self.occurrence_count)?,
            self.ticket_after.encode(),
        ))
    }
}

/// Plan ticket preparation after immutable occurrence admission.
#[allow(clippy::too_many_arguments)]
pub fn plan_prepare(
    admitted: AdmittedOccurrenceV2,
    admitted_ticket: AdmittedTicketV2,
    series: SeriesStateV2,
    expected_series_revision: u64,
    now_slot: u64,
    current_ticket_lamports: u64,
    ticket_state_rent: u64,
) -> Result<(OccurrenceCommitPlanV2, u64, u64), LifecycleErrorV2> {
    let ticket = admitted_ticket.ticket();
    let ticket_record_id = admitted_ticket.content_id();
    admitted.require_ticket(ticket)?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    if series.next_occurrence() != occurrence.occurrence()
        || now_slot > template.retry_through(occurrence.occurrence())?
    {
        return Err(LifecycleErrorV2::Schedule);
    }
    let native = occurrence.funds().checked_native_total()?;
    let required = ticket_state_rent
        .checked_add(native)
        .ok_or(LifecycleErrorV2::Arithmetic)?;
    let (top_up, dust_refund) = dust_tolerant_exact(current_ticket_lamports, required);
    Ok((
        OccurrenceCommitPlanV2 {
            core_request: None,
            series_after: series.prepare_ticket(expected_series_revision)?,
            ticket_after: TicketStateV2::prepared(ticket_record_id),
            occurrence_count: template.occurrence_count(),
            native_from_ticket: 0,
            funding: None,
        },
        top_up,
        dust_refund,
    ))
}

/// Plan atomic Ticket-to-Found consumption through Core/Claims/Custody.
#[allow(clippy::too_many_arguments)]
pub fn plan_consume(
    admitted: AdmittedOccurrenceV2,
    admitted_ticket: AdmittedTicketV2,
    ticket_state_key: Pubkey,
    series: SeriesStateV2,
    ticket_state: TicketStateV2,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
    funding: PendingFundingPlanV2,
) -> Result<OccurrenceCommitPlanV2, LifecycleErrorV2> {
    common_terminal_plan(
        admitted,
        admitted_ticket,
        ticket_state_key,
        series,
        ticket_state,
        expected_series_revision,
        expected_ticket_revision,
        now_slot,
        SeriesCoreActionV1::Consume,
        TicketPhaseV2::Consumed,
        Some(funding),
    )
}

/// Plan exact expiry refund after the immutable retry window.
#[allow(clippy::too_many_arguments)]
pub fn plan_expire(
    admitted: AdmittedOccurrenceV2,
    admitted_ticket: AdmittedTicketV2,
    ticket_state_key: Pubkey,
    series: SeriesStateV2,
    ticket_state: TicketStateV2,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
) -> Result<OccurrenceCommitPlanV2, LifecycleErrorV2> {
    common_terminal_plan(
        admitted,
        admitted_ticket,
        ticket_state_key,
        series,
        ticket_state,
        expected_series_revision,
        expected_ticket_revision,
        now_slot,
        SeriesCoreActionV1::Expire,
        TicketPhaseV2::Expired,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn common_terminal_plan(
    admitted: AdmittedOccurrenceV2,
    admitted_ticket: AdmittedTicketV2,
    ticket_state_key: Pubkey,
    series: SeriesStateV2,
    ticket_state: TicketStateV2,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
    action: SeriesCoreActionV1,
    terminal: TicketPhaseV2,
    funding: Option<PendingFundingPlanV2>,
) -> Result<OccurrenceCommitPlanV2, LifecycleErrorV2> {
    let ticket = admitted_ticket.ticket();
    let ticket_record_id = admitted_ticket.content_id();
    admitted.require_ticket(ticket)?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    if series.next_occurrence() != occurrence.occurrence()
        || ticket_state.ticket_record_id() != ticket_record_id
    {
        return Err(LifecycleErrorV2::Replay);
    }
    let retry_through = template.retry_through(occurrence.occurrence())?;
    match action {
        SeriesCoreActionV1::Consume
            if now_slot < occurrence.scheduled_slot() || now_slot > retry_through =>
        {
            return Err(LifecycleErrorV2::Schedule);
        }
        SeriesCoreActionV1::Expire if now_slot <= retry_through => {
            return Err(LifecycleErrorV2::Schedule);
        }
        SeriesCoreActionV1::Consume => {
            if funding
                .is_none_or(|plan| plan.required_native() != occurrence.funds().capability_native())
            {
                return Err(LifecycleErrorV2::Funding);
            }
        }
        SeriesCoreActionV1::Expire => {}
        _ => return Err(LifecycleErrorV2::Content),
    }
    let core_request = if action == SeriesCoreActionV1::Consume {
        Some(core_request(
            admitted,
            action,
            ticket,
            ticket_state_key,
            expected_series_revision,
            expected_ticket_revision,
        )?)
    } else {
        None
    };
    Ok(OccurrenceCommitPlanV2 {
        core_request,
        series_after: series
            .settle_current(expected_series_revision, template.occurrence_count())?,
        ticket_after: ticket_state.settle(expected_ticket_revision, terminal)?,
        occurrence_count: template.occurrence_count(),
        native_from_ticket: occurrence.funds().checked_native_total()?,
        funding,
    })
}

/// Pure ticket-retirement result after a terminal occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirePlanV2 {
    series_after: SeriesStateV2,
    refund_owner: Pubkey,
    lamports_to_refund_owner: u64,
}

impl RetirePlanV2 {
    /// Candidate root state written only after the ticket account closes.
    pub const fn series_after(self) -> SeriesStateV2 {
        self.series_after
    }
    /// Immutable Ticket-record beneficiary.
    pub const fn refund_owner(self) -> Pubkey {
        self.refund_owner
    }
    /// Ticket Rent and explicitly classified unsolicited lamport donation.
    pub const fn lamports_to_refund_owner(self) -> u64 {
        self.lamports_to_refund_owner
    }
}

/// Plan deletion of one non-replayable ticket account.
pub fn plan_retire(
    series: SeriesStateV2,
    ticket_state: TicketStateV2,
    admitted_ticket: AdmittedTicketV2,
    expected_series_revision: u64,
    observed_ticket_lamports: u64,
) -> Result<RetirePlanV2, LifecycleErrorV2> {
    if !ticket_state.phase().terminal()
        || ticket_state.ticket_record_id() != admitted_ticket.content_id()
    {
        return Err(LifecycleErrorV2::Replay);
    }
    Ok(RetirePlanV2 {
        series_after: series.retire_ticket(expected_series_revision)?,
        refund_owner: pubkey(admitted_ticket.ticket().refund_owner()),
        lamports_to_refund_owner: observed_ticket_lamports,
    })
}

/// Root-close classification; Hoard principal is never in these lamport fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosePlanV2 {
    beneficiary: Pubkey,
    close_rent: u64,
    root_rent: u64,
    donation: u64,
}

impl ClosePlanV2 {
    /// Finalized Template beneficiary receiving classified native refunds.
    pub const fn beneficiary(self) -> Pubkey {
        self.beneficiary
    }
    /// Separately classified close-rent principal returned to the Template owner.
    pub const fn close_rent(self) -> u64 {
        self.close_rent
    }
    /// Exact composite-root Rent reserve returned on deletion.
    pub const fn root_rent(self) -> u64 {
        self.root_rent
    }
    /// Unsolicited root lamports, classified only as a refund gift.
    pub const fn donation(self) -> u64 {
        self.donation
    }
}

/// Plan terminal close after every replay account has been retired.
pub fn plan_close(
    template: TemplateV2,
    series: SeriesStateV2,
    expected_series_revision: u64,
    observed_root_lamports: u64,
    exact_root_rent: u64,
) -> Result<ClosePlanV2, LifecycleErrorV2> {
    series.admit_close(expected_series_revision)?;
    let classified = exact_root_rent
        .checked_add(series.close_rent_remaining())
        .ok_or(LifecycleErrorV2::Arithmetic)?;
    let donation = observed_root_lamports
        .checked_sub(classified)
        .ok_or(LifecycleErrorV2::Funding)?;
    Ok(ClosePlanV2 {
        beneficiary: pubkey(template.refund_owner()),
        close_rent: series.close_rent_remaining(),
        root_rent: exact_root_rent,
        donation,
    })
}

fn dust_tolerant_exact(observed: u64, required: u64) -> (u64, u64) {
    if observed <= required {
        (required - observed, 0)
    } else {
        (0, observed - required)
    }
}
