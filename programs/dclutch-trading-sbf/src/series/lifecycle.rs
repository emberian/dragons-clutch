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
use dclutch_series_v3_kernel::plan::{
    ReplayCandidateV3, SeriesReplayActionV3, SeriesReplayWitnessV3, evaluate_replay_v3,
};
pub use dclutch_series_v3_kernel::terminal::{
    SeriesLifecycleRentSinkV3, SeriesRootClosurePlanV3 as ClosePlanV3, TicketNativeRemaindersV3,
    TicketRetirementPlanV3 as RetirePlanV3,
};
use dclutch_series_v3_kernel::terminal::{
    SeriesTerminalErrorV3, plan_series_root_closure_v3, plan_ticket_retirement_v3,
};
use solana_program::pubkey::Pubkey;

use super::{
    AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2, OccurrenceV3,
    SeriesV3Error, TemplateV3, core_request, funding_list_id,
    state::{SeriesStateError, SeriesStateV3, TicketStateV3},
};

const MAXIMUM_FUNDING_STATES: usize = 16;

/// Refusal from an executable lifecycle plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleErrorV3 {
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

impl From<SeriesV3Error> for LifecycleErrorV3 {
    fn from(_: SeriesV3Error) -> Self {
        Self::Content
    }
}

impl From<SeriesStateError> for LifecycleErrorV3 {
    fn from(_: SeriesStateError) -> Self {
        Self::Replay
    }
}

impl From<SeriesTerminalErrorV3> for LifecycleErrorV3 {
    fn from(value: SeriesTerminalErrorV3) -> Self {
        match value {
            SeriesTerminalErrorV3::RentEncoding | SeriesTerminalErrorV3::RentBinding => {
                Self::Funding
            }
            SeriesTerminalErrorV3::Replay => Self::Replay,
            SeriesTerminalErrorV3::Balance => Self::Funding,
            SeriesTerminalErrorV3::Arithmetic => Self::Arithmetic,
        }
    }
}

/// One proposed Trading-owned FundingState creation destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingFundingAccountV3 {
    key: Pubkey,
    state: FundingStateV1,
    current_lamports: u64,
    exact_state_rent: u64,
    realm_collateral: Option<RealmCollateralCustodyV1>,
}

impl PendingFundingAccountV3 {
    /// Bind the exact planned FundingState bytes and observed physical custody.
    pub fn new(
        key: Pubkey,
        state: FundingStateV1,
        current_lamports: u64,
        exact_state_rent: u64,
        realm_collateral: Option<RealmCollateralCustodyV1>,
    ) -> Result<Self, LifecycleErrorV3> {
        if key == Pubkey::default() {
            return Err(LifecycleErrorV3::Funding);
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
pub struct PendingFundingPlanV3 {
    count: u8,
    top_up: [u64; MAXIMUM_FUNDING_STATES],
    preexisting_surplus_refund: [u64; MAXIMUM_FUNDING_STATES],
    ticket_capability_refund: u64,
    required_native: u64,
}

impl PendingFundingPlanV3 {
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
    occurrence: OccurrenceV3,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    accounts: &[PendingFundingAccountV3],
) -> Result<PendingFundingPlanV3, LifecycleErrorV3> {
    if accounts.is_empty() || accounts.len() > MAXIMUM_FUNDING_STATES {
        return Err(LifecycleErrorV3::Funding);
    }
    let mut keys = [Pubkey::default(); MAXIMUM_FUNDING_STATES];
    let mut top_up = [0_u64; MAXIMUM_FUNDING_STATES];
    let mut surplus = [0_u64; MAXIMUM_FUNDING_STATES];
    let mut required_native = 0_u64;
    let mut transferred = 0_u64;
    let mut previous_entry: Option<u16> = None;

    for (index, account) in accounts.iter().copied().enumerate() {
        *keys.get_mut(index).ok_or(LifecycleErrorV3::Funding)? = account.key;
        let state = account.state;
        if state.status() != FundingStatus::Pending
            || state.manifest_content_id() != manifest_id
            || previous_entry.is_some_and(|previous| previous >= state.entry_index())
        {
            return Err(LifecycleErrorV3::Funding);
        }
        let required = account
            .exact_state_rent
            .checked_add(state.remaining().native_lamports_total())
            .ok_or(LifecycleErrorV3::Arithmetic)?;
        let desired_custody = match account.realm_collateral {
            Some(realm) => FundingCustodyObservationV1::with_realm_collateral(
                required,
                account.exact_state_rent,
                realm,
            ),
            None => FundingCustodyObservationV1::native_only(required, account.exact_state_rent),
        }
        .map_err(|_| LifecycleErrorV3::Funding)?;
        state
            .validate_against(manifest_id, manifest, desired_custody)
            .map_err(|_| LifecycleErrorV3::Funding)?;
        let (top_up_value, surplus_value) = if account.current_lamports <= required {
            (required - account.current_lamports, 0)
        } else {
            (0, account.current_lamports - required)
        };
        *top_up.get_mut(index).ok_or(LifecycleErrorV3::Funding)? = top_up_value;
        *surplus.get_mut(index).ok_or(LifecycleErrorV3::Funding)? = surplus_value;
        required_native = required_native
            .checked_add(required)
            .ok_or(LifecycleErrorV3::Arithmetic)?;
        transferred = transferred
            .checked_add(top_up_value)
            .ok_or(LifecycleErrorV3::Arithmetic)?;
        previous_entry = Some(state.entry_index());
    }
    if funding_list_id(
        keys.get(..accounts.len())
            .ok_or(LifecycleErrorV3::Funding)?,
    )? != occurrence.funding_list()
        || required_native != occurrence.funds().capability_native()
    {
        return Err(LifecycleErrorV3::Funding);
    }
    Ok(PendingFundingPlanV3 {
        count: u8::try_from(accounts.len()).map_err(|_| LifecycleErrorV3::Funding)?,
        top_up,
        preexisting_surplus_refund: surplus,
        ticket_capability_refund: required_native
            .checked_sub(transferred)
            .ok_or(LifecycleErrorV3::Arithmetic)?,
        required_native,
    })
}

/// Candidate bytes and Core request for one commit-last occurrence transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceCommitPlanV3 {
    core_request: Option<SeriesCoreRequestV1>,
    series_after: SeriesStateV3,
    ticket_after: TicketStateV3,
    occurrence_count: u32,
    native_from_ticket: u64,
    native_remainders: TicketNativeRemaindersV3,
    terminal_rent_sink: Option<SeriesLifecycleRentSinkV3>,
    funding: Option<PendingFundingPlanV3>,
}

impl OccurrenceCommitPlanV3 {
    /// Exact 336-byte Core request, present only for atomic Consume/Found.
    pub const fn core_request(self) -> Option<SeriesCoreRequestV1> {
        self.core_request
    }
    /// Candidate Series state; not persisted before Core acknowledgement.
    pub const fn series_after(self) -> SeriesStateV3 {
        self.series_after
    }
    /// Candidate Ticket state; not persisted before Core acknowledgement.
    pub const fn ticket_after(self) -> TicketStateV3 {
        self.ticket_after
    }
    /// Exact native lamports drained from Ticket custody on success.
    pub const fn native_from_ticket(self) -> u64 {
        self.native_from_ticket
    }
    /// Exact native compartment classification; Hoard collateral is excluded.
    pub const fn native_remainders(self) -> TicketNativeRemaindersV3 {
        self.native_remainders
    }
    /// Lifecycle Rent V2 destination for unused native funds on Expire.
    pub const fn terminal_rent_sink(self) -> Option<SeriesLifecycleRentSinkV3> {
        self.terminal_rent_sink
    }
    /// Exact FundingState distribution, present only for consumption.
    pub const fn funding(self) -> Option<PendingFundingPlanV3> {
        self.funding
    }

    /// Canonical candidate root tail and Ticket bytes without write authority.
    ///
    /// This view lets a physical adapter authenticate a child acknowledgement
    /// against the exact proposed poststate. It neither validates an
    /// acknowledgement nor authorizes persistence; Consume remains writable
    /// only through [`Self::commit_after_ack`].
    pub fn candidate_bytes(self) -> Result<([u8; 64], [u8; 64]), LifecycleErrorV3> {
        Ok((
            self.series_after.encode(self.occurrence_count)?,
            self.ticket_after.encode(),
        ))
    }

    /// Validate immediate Core return data and expose the only permitted writes.
    pub fn commit_after_ack(
        self,
        ack: SeriesCoreAckV1,
        expected_core_program: CoreIdentity,
        request_digest: CoreIdentity,
        observed_post_resource_digest: CoreIdentity,
    ) -> Result<([u8; 64], [u8; 64]), LifecycleErrorV3> {
        let request = self.core_request.ok_or(LifecycleErrorV3::CoreAck)?;
        ack.validate_for(
            request,
            expected_core_program,
            request_digest,
            observed_post_resource_digest,
        )
        .map_err(|_| LifecycleErrorV3::CoreAck)?;
        self.candidate_bytes()
    }

    /// Expose controller-owned candidate bytes for Prepare or Expire.
    ///
    /// The physical outer calls this only after every direct Trading-owned
    /// account operation and any current-Custody receipt have authenticated.
    /// Consume cannot bypass its Core acknowledgement through this route.
    pub fn commit_controller(self) -> Result<([u8; 64], [u8; 64]), LifecycleErrorV3> {
        if self.core_request.is_some() {
            return Err(LifecycleErrorV3::CoreAck);
        }
        self.candidate_bytes()
    }
}

/// Plan ticket preparation after immutable occurrence admission.
#[allow(clippy::too_many_arguments)]
pub fn plan_prepare(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    series: SeriesStateV3,
    expected_series_revision: u64,
    now_slot: u64,
    current_ticket_lamports: u64,
    ticket_state_rent: u64,
) -> Result<(OccurrenceCommitPlanV3, u64, u64), LifecycleErrorV3> {
    let ticket = admitted_ticket.ticket();
    let ticket_record_id = admitted_ticket.content_id();
    admitted.require_ticket(ticket)?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    if series.next_occurrence() != occurrence.occurrence()
        || now_slot > template.retry_through(occurrence.occurrence())?
    {
        return Err(LifecycleErrorV3::Schedule);
    }
    let native = occurrence.funds().checked_native_total()?;
    let required = ticket_state_rent
        .checked_add(native)
        .ok_or(LifecycleErrorV3::Arithmetic)?;
    let (top_up, dust_refund) = dust_tolerant_exact(current_ticket_lamports, required);
    let witness = evaluate_joint_replay(
        SeriesReplayActionV3::Prepare {
            ticket_record: ticket_record_id,
        },
        template.occurrence_count(),
        expected_series_revision,
        series,
        None,
    )?;
    let (series_after, ticket_after) = replacement_pair(witness, template.occurrence_count())?;
    Ok((
        OccurrenceCommitPlanV3 {
            core_request: None,
            series_after,
            ticket_after,
            occurrence_count: template.occurrence_count(),
            native_from_ticket: 0,
            native_remainders: TicketNativeRemaindersV3::from_founding_funds(occurrence.funds()),
            terminal_rent_sink: None,
            funding: None,
        },
        top_up,
        dust_refund,
    ))
}

/// Plan atomic Ticket-to-Found consumption through Core/Claims/Custody.
#[allow(clippy::too_many_arguments)]
pub fn plan_consume(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    ticket_state_key: Pubkey,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
    funding: PendingFundingPlanV3,
) -> Result<OccurrenceCommitPlanV3, LifecycleErrorV3> {
    common_terminal_plan(
        admitted,
        admitted_ticket,
        Some(product),
        ticket_state_key,
        series,
        ticket_state,
        expected_series_revision,
        expected_ticket_revision,
        now_slot,
        SeriesCoreActionV1::Consume,
        Some(funding),
        None,
    )
}

/// Plan exact expiry refund after the immutable retry window.
#[allow(clippy::too_many_arguments)]
pub fn plan_expire(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    ticket_state_key: Pubkey,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
    rent_sink: SeriesLifecycleRentSinkV3,
) -> Result<OccurrenceCommitPlanV3, LifecycleErrorV3> {
    rent_sink
        .admit_refund_owner(admitted_ticket.ticket().refund_owner())
        .map_err(LifecycleErrorV3::from)?;
    common_terminal_plan(
        admitted,
        admitted_ticket,
        None,
        ticket_state_key,
        series,
        ticket_state,
        expected_series_revision,
        expected_ticket_revision,
        now_slot,
        SeriesCoreActionV1::Expire,
        None,
        Some(rent_sink),
    )
}

#[allow(clippy::too_many_arguments)]
fn common_terminal_plan(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: Option<AuthenticatedProductProjectionV2>,
    ticket_state_key: Pubkey,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    now_slot: u64,
    action: SeriesCoreActionV1,
    funding: Option<PendingFundingPlanV3>,
    terminal_rent_sink: Option<SeriesLifecycleRentSinkV3>,
) -> Result<OccurrenceCommitPlanV3, LifecycleErrorV3> {
    let ticket = admitted_ticket.ticket();
    let ticket_record_id = admitted_ticket.content_id();
    admitted.require_ticket(ticket)?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    if series.next_occurrence() != occurrence.occurrence()
        || ticket_state.ticket_record_id() != ticket_record_id
    {
        return Err(LifecycleErrorV3::Replay);
    }
    let retry_through = template.retry_through(occurrence.occurrence())?;
    match action {
        SeriesCoreActionV1::Consume
            if now_slot < occurrence.scheduled_slot() || now_slot > retry_through =>
        {
            return Err(LifecycleErrorV3::Schedule);
        }
        SeriesCoreActionV1::Expire if now_slot <= retry_through => {
            return Err(LifecycleErrorV3::Schedule);
        }
        SeriesCoreActionV1::Consume => {
            if funding
                .is_none_or(|plan| plan.required_native() != occurrence.funds().capability_native())
            {
                return Err(LifecycleErrorV3::Funding);
            }
        }
        SeriesCoreActionV1::Expire => {}
        _ => return Err(LifecycleErrorV3::Content),
    }
    let core_request = if action == SeriesCoreActionV1::Consume {
        Some(core_request(
            admitted,
            product.ok_or(LifecycleErrorV3::Content)?,
            action,
            admitted_ticket,
            ticket_state_key,
            expected_series_revision,
            expected_ticket_revision,
        )?)
    } else {
        None
    };
    let replay_action = match action {
        SeriesCoreActionV1::Consume => SeriesReplayActionV3::Consume {
            ticket_record: ticket_record_id,
            expected_ticket_revision,
        },
        SeriesCoreActionV1::Expire => SeriesReplayActionV3::Expire {
            ticket_record: ticket_record_id,
            expected_ticket_revision,
        },
        _ => return Err(LifecycleErrorV3::Content),
    };
    let witness = evaluate_joint_replay(
        replay_action,
        template.occurrence_count(),
        expected_series_revision,
        series,
        Some(ticket_state),
    )?;
    let (series_after, ticket_after) = replacement_pair(witness, template.occurrence_count())?;
    Ok(OccurrenceCommitPlanV3 {
        core_request,
        series_after,
        ticket_after,
        occurrence_count: template.occurrence_count(),
        native_from_ticket: occurrence.funds().checked_native_total()?,
        native_remainders: TicketNativeRemaindersV3::from_founding_funds(occurrence.funds()),
        terminal_rent_sink,
        funding,
    })
}

/// Plan deletion of one non-replayable ticket account.
#[allow(clippy::too_many_arguments)]
pub fn plan_retire(
    occurrence_count: u32,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    admitted_ticket: AdmittedTicketV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    observed_ticket_lamports: u64,
    exact_ticket_rent: u64,
    rent_sink: SeriesLifecycleRentSinkV3,
) -> Result<RetirePlanV3, LifecycleErrorV3> {
    plan_ticket_retirement_v3(
        occurrence_count,
        series,
        ticket_state,
        admitted_ticket,
        expected_series_revision,
        expected_ticket_revision,
        observed_ticket_lamports,
        exact_ticket_rent,
        rent_sink,
    )
    .map_err(Into::into)
}

/// Plan terminal close after every replay account has been retired.
pub fn plan_close(
    template: TemplateV3,
    series: SeriesStateV3,
    expected_series_revision: u64,
    observed_root_lamports: u64,
    exact_root_rent: u64,
    rent_sink: SeriesLifecycleRentSinkV3,
) -> Result<ClosePlanV3, LifecycleErrorV3> {
    plan_series_root_closure_v3(
        template,
        series,
        expected_series_revision,
        observed_root_lamports,
        exact_root_rent,
        rent_sink,
    )
    .map_err(Into::into)
}

fn evaluate_joint_replay(
    action: SeriesReplayActionV3,
    occurrence_count: u32,
    expected_series_revision: u64,
    series: SeriesStateV3,
    ticket: Option<TicketStateV3>,
) -> Result<SeriesReplayWitnessV3, LifecycleErrorV3> {
    let series_bytes = series.encode(occurrence_count)?;
    let ticket_bytes = ticket.map(TicketStateV3::encode);
    evaluate_replay_v3(
        action,
        occurrence_count,
        expected_series_revision,
        &series_bytes,
        ticket_bytes.as_ref().map(<[u8; 64]>::as_slice),
    )
    .map_err(|_| LifecycleErrorV3::Replay)
}

fn replacement_pair(
    witness: SeriesReplayWitnessV3,
    occurrence_count: u32,
) -> Result<(SeriesStateV3, TicketStateV3), LifecycleErrorV3> {
    let series = match witness.series() {
        ReplayCandidateV3::Replace(bytes) => SeriesStateV3::decode(&bytes, occurrence_count)?,
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => {
            return Err(LifecycleErrorV3::Replay);
        }
    };
    let ticket = match witness.ticket() {
        ReplayCandidateV3::Replace(bytes) => TicketStateV3::decode(&bytes)?,
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => {
            return Err(LifecycleErrorV3::Replay);
        }
    };
    Ok((series, ticket))
}

fn dust_tolerant_exact(observed: u64, required: u64) -> (u64, u64) {
    if observed <= required {
        (required - observed, 0)
    } else {
        (0, observed - required)
    }
}
