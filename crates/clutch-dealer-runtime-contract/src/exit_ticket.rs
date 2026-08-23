// SPDX-License-Identifier: AGPL-3.0-or-later

//! Owner-scoped queue intent without mutating immutable LP ownership pages.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    lp_funding_v2::prepare_funding_replay_v2, CountedDealerChildV2,
    DealerActionLivenessAuthorizationV1, DealerChildKindV2,
    DealerEmptyAssetTransferBundleV1, DealerFacilityReplayV1,
    DealerLivenessScheduleV1, DealerPhaseV2, DealerPolicyV1,
    DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DealerTransitionLivenessModeV1,
    DeletableRentOwnerV1, Error, FixedCodec, Id, LpPageV2,
    PreparedDealerReplayTransitionV1, Result, DEALER_EXIT_TICKET_CONTENT_DOMAIN_V1,
    DELETABLE_RENT_OWNER_BYTES,
};

/// Local semantic magic for one exit ticket.
pub const DEALER_EXIT_TICKET_MAGIC_V1: [u8; 8] = *b"DCDEXIT1";
/// Exact local semantic version.
pub const DEALER_EXIT_TICKET_VERSION_V1: u16 = 1;
/// Exact canonical body bytes.
pub const DEALER_EXIT_TICKET_BYTES_V1: usize =
    HEADER_BYTES + (7 * 32) + 40 + DELETABLE_RENT_OWNER_BYTES;

/// One mutable queue fact keyed uniquely by facility and owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerExitTicketV1 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Canonical Position V3 purpose binding.
    pub facility_position_binding_id: Id,
    /// Authoritative State account.
    pub dealer_state_account_id: Id,
    /// Immutable LP page account containing this owner.
    pub lp_page_account_id: Id,
    /// Exact immutable LP page semantic identity.
    pub lp_page_content_id: Id,
    /// Ordinary Position owner and PDA coordinate.
    pub owner: Id,
    /// Parent generation at admission.
    pub counted_generation: u64,
    /// Immutable page ordinal.
    pub page_ordinal: u32,
    /// Immutable entry index in that page.
    pub entry_index: u8,
    /// Canonical zero padding.
    pub reserved: [u8; 3],
    /// Exact immutable shares owned by the page entry.
    pub immutable_shares: u64,
    /// Irrevocably queued shares.
    pub queued_shares: u64,
    /// Monotone ticket revision.
    pub revision: u64,
    /// Independently funded ticket rent.
    pub rent: DeletableRentOwnerV1,
}

impl DealerExitTicketV1 {
    /// Validate canonical ticket bytes independently of runtime account checks.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.lp_page_account_id,
            self.lp_page_content_id,
            self.owner,
        ] {
            identity.validate_live()?;
        }
        if self.counted_generation == 0
            || usize::from(self.entry_index) >= crate::LP_ENTRIES_PER_PAGE
            || self.reserved != [0; 3]
            || self.immutable_shares == 0
            || self.queued_shares == 0
            || self.queued_shares > self.immutable_shares
        {
            return Err(Error::InvalidParameter);
        }
        self.rent.validate()
    }

    /// Exact semantic identity.
    pub fn ticket_id(&self) -> Result<Id> {
        self.content_id(DEALER_EXIT_TICKET_CONTENT_DOMAIN_V1)
    }

    /// Counted root edge.
    pub const fn counted_child(&self) -> CountedDealerChildV2 {
        CountedDealerChildV2 {
            facility_id: self.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: DealerChildKindV2::ExitTicket,
            counted_generation: self.counted_generation,
        }
    }

    fn validate_against(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        state_account_id: Id,
        page_account_id: Id,
        page: &LpPageV2,
    ) -> Result<()> {
        self.validate()?;
        page.validate_against(policy, state, state_account_id)?;
        let index = usize::from(self.entry_index);
        if self.policy_id != policy.policy_id()?
            || self.facility_id != state.facility_id
            || self.facility_position_binding_id != state.facility_position_binding_id
            || self.dealer_state_account_id != state_account_id
            || self.lp_page_account_id != page_account_id
            || self.lp_page_content_id != page.page_content_id()?
            || self.counted_generation != state.generation
            || self.page_ordinal != page.page_ordinal
            || self.page_ordinal >= state.children.lp_pages
            || index >= usize::from(page.entry_count)
            || page.entries[index].owner != self.owner
            || page.entries[index].shares != self.immutable_shares
            || !page.sealed
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

impl FixedCodec for DealerExitTicketV1 {
    const ENCODED_LEN: usize = DEALER_EXIT_TICKET_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_EXIT_TICKET_MAGIC_V1, DEALER_EXIT_TICKET_VERSION_V1);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.lp_page_account_id,
            self.lp_page_content_id,
            self.owner,
        ] {
            writer.id(identity);
        }
        writer.u64(self.counted_generation);
        writer.u32(self.page_ordinal);
        writer.u8(self.entry_index);
        writer.bytes(&self.reserved);
        writer.u64(self.immutable_shares);
        writer.u64(self.queued_shares);
        writer.u64(self.revision);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_EXIT_TICKET_MAGIC_V1, DEALER_EXIT_TICKET_VERSION_V1)?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            facility_position_binding_id: reader.id(),
            dealer_state_account_id: reader.id(),
            lp_page_account_id: reader.id(),
            lp_page_content_id: reader.id(),
            owner: reader.id(),
            counted_generation: reader.u64(),
            page_ordinal: reader.u32(),
            entry_index: reader.u8(),
            reserved: reader.bytes::<3>(),
            immutable_shares: reader.u64(),
            queued_shares: reader.u64(),
            revision: reader.u64(),
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Checked liveness evidence for one queue mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQueueExitLivenessV1 {
    receipt_semantic_id: Id,
    mode: DealerTransitionLivenessModeV1,
}

impl DealerQueueExitLivenessV1 {
    /// Caller-funded queue maintenance.
    pub const fn caller_funded() -> Self {
        Self {
            receipt_semantic_id: Id::ZERO,
            mode: DealerTransitionLivenessModeV1::CallerFunded,
        }
    }

    /// Externally funded optional queue maintenance.
    pub fn external(
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        authorization: &DealerActionLivenessAuthorizationV1,
        state: &DealerStateV2,
        state_account_id: Id,
    ) -> Result<Self> {
        authorization.validate_against(schedule, runtime)?;
        if authorization.action != DealerRuntimeActionV1::QueueExit
            || authorization.owner != state_account_id
            || authorization.lifecycle_id != state.facility_id
            || authorization.facility_generation != state.generation
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(Self {
            receipt_semantic_id: authorization.receipt_semantic_id,
            mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        })
    }
}

/// Atomic creation of a unique owner-scoped queue ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerNewExitTicketV1 {
    /// New ticket body.
    pub ticket: DealerExitTicketV1,
    /// State after exact aggregate increment and possible UnwindOnly entry.
    pub state_after: DealerStateV2,
    /// Replay advance binding the ticket and State write.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Atomic increase of one existing queue ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerExitTicketIncreaseV1 {
    /// Ticket after its irreversible increase.
    pub ticket_after: DealerExitTicketV1,
    /// State after exact aggregate increment and possible UnwindOnly entry.
    pub state_after: DealerStateV2,
    /// Replay advance binding the ticket and State write.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Create one ticket; the adapter authenticates its unique facility+owner PDA.
#[allow(clippy::too_many_arguments)]
pub fn prepare_new_exit_ticket_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    page: &LpPageV2,
    entry_index: u8,
    owner: Id,
    queued_shares: u64,
    rent: DeletableRentOwnerV1,
    liveness: DealerQueueExitLivenessV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerNewExitTicketV1> {
    state.validate_against_policy(policy)?;
    let index = usize::from(entry_index);
    if state.phase != DealerPhaseV2::Trading
        || index >= usize::from(page.entry_count)
        || page.entries[index].owner != owner
        || queued_shares == 0
        || queued_shares > page.entries[index].shares
    {
        return Err(Error::InvalidPhase);
    }
    let ticket = DealerExitTicketV1 {
        policy_id: policy.policy_id()?,
        facility_id: state.facility_id,
        facility_position_binding_id: state.facility_position_binding_id,
        dealer_state_account_id: state_account_id,
        lp_page_account_id: page_account_id,
        lp_page_content_id: page.page_content_id()?,
        owner,
        counted_generation: state.generation,
        page_ordinal: page.page_ordinal,
        entry_index,
        reserved: [0; 3],
        immutable_shares: page.entries[index].shares,
        queued_shares,
        revision: 0,
        rent,
    };
    ticket.validate_against(policy, state, state_account_id, page_account_id, page)?;
    let mut state_after = *state;
    state_after.queued_shares = state_after
        .queued_shares
        .checked_add(queued_shares)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.children.exit_tickets = state_after
        .children
        .exit_tickets
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.child_sequence = next(state.child_sequence)?;
    enter_unwind_if_threshold(policy, &mut state_after)?;
    state_after.validate_against_policy(policy)?;
    let replay = queue_replay(state, &state_after, replay, replay_binding, liveness)?;
    Ok(PreparedDealerNewExitTicketV1 {
        ticket,
        state_after,
        replay,
    })
}

/// Increase an existing ticket without changing its immutable owner/page facts.
#[allow(clippy::too_many_arguments)]
pub fn prepare_increase_exit_ticket_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    page: &LpPageV2,
    ticket: &DealerExitTicketV1,
    additional_queued_shares: u64,
    liveness: DealerQueueExitLivenessV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerExitTicketIncreaseV1> {
    state.validate_against_policy(policy)?;
    ticket.validate_against(policy, state, state_account_id, page_account_id, page)?;
    if state.phase != DealerPhaseV2::Trading || additional_queued_shares == 0 {
        return Err(Error::InvalidPhase);
    }
    let mut ticket_after = *ticket;
    ticket_after.queued_shares = ticket_after
        .queued_shares
        .checked_add(additional_queued_shares)
        .ok_or(Error::ArithmeticOverflow)?;
    ticket_after.revision = next(ticket.revision)?;
    ticket_after.validate_against(policy, state, state_account_id, page_account_id, page)?;
    let mut state_after = *state;
    state_after.queued_shares = state_after
        .queued_shares
        .checked_add(additional_queued_shares)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.child_sequence = next(state.child_sequence)?;
    enter_unwind_if_threshold(policy, &mut state_after)?;
    state_after.validate_against_policy(policy)?;
    let replay = queue_replay(state, &state_after, replay, replay_binding, liveness)?;
    Ok(PreparedDealerExitTicketIncreaseV1 {
        ticket_after,
        state_after,
        replay,
    })
}

/// Exact rent observation and disposition for permissionless ticket close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerExitTicketCloseRentV1 {
    /// Ticket lamports before deletion.
    pub account_lamports_before: u64,
    /// Ticket lamports after deletion; exactly zero.
    pub account_lamports_after: u64,
    /// Exact refundable-principal recipient.
    pub payer: Id,
    /// Exact donation/surplus recipient.
    pub neutral_sink: Id,
    /// Exact principal credit.
    pub payer_refund_lamports: u64,
    /// Exact donation-floor and later-surplus credit.
    pub neutral_sink_lamports: u64,
}

/// Permissionlessly close one queue ticket after UnwindOnly became canonical.
#[allow(clippy::too_many_arguments)]
pub fn close_exit_ticket_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    page: &LpPageV2,
    ticket: &DealerExitTicketV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    rent: DealerExitTicketCloseRentV1,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    ticket.validate_against(policy, state, state_account_id, page_account_id, page)?;
    authorization.validate_against(schedule, runtime)?;
    let protected = ticket
        .rent
        .refundable_principal
        .checked_add(ticket.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if state.phase != DealerPhaseV2::UnwindOnly
        || authorization.action != DealerRuntimeActionV1::Retire
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || state.children.exit_tickets == 0
        || state.queued_shares < ticket.queued_shares
        || rent.account_lamports_after != 0
        || rent.account_lamports_before < protected
        || rent.payer != ticket.rent.payer
        || rent.neutral_sink != ticket.rent.neutral_sink
        || rent.payer_refund_lamports != ticket.rent.refundable_principal
        || rent.neutral_sink_lamports
            != rent
                .account_lamports_before
                .checked_sub(ticket.rent.refundable_principal)
                .ok_or(Error::ConservationFailure)?
    {
        return Err(Error::MismatchedBinding);
    }
    let mut state_after = *state;
    state_after.queued_shares -= ticket.queued_shares;
    state_after.children.exit_tickets -= 1;
    state_after.child_sequence = next(state.child_sequence)?;
    state_after.validate_against_policy(policy)?;
    Ok(state_after)
}

fn enter_unwind_if_threshold(policy: &DealerPolicyV1, state: &mut DealerStateV2) -> Result<()> {
    if policy.shutdown_queue_threshold_met_validated(state.queued_shares, state.total_shares)? {
        state.phase = DealerPhaseV2::UnwindOnly;
    }
    Ok(())
}

fn queue_replay(
    state: &DealerStateV2,
    state_after: &DealerStateV2,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    liveness: DealerQueueExitLivenessV1,
) -> Result<PreparedDealerReplayTransitionV1> {
    prepare_funding_replay_v2(
        state,
        state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::QueueExit,
        liveness.receipt_semantic_id,
        liveness.mode,
        DealerEmptyAssetTransferBundleV1 {
            action: DealerRuntimeActionV1::QueueExit,
        }
        .bundle_id()?,
        state.facility_position_id,
        state.facility_position_id,
    )
}

fn next(value: u64) -> Result<u64> {
    value.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

const _: () = assert!(DEALER_EXIT_TICKET_BYTES_V1 == 356);
const _: () = assert!(DEALER_EXIT_TICKET_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
