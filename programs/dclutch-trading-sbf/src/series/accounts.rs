//! Solana account boundary for recurring Series V3.
//!
//! The functions in this module authenticate Trading-owned replay accounts,
//! stage prepaid FundingState PDAs, and expose one acknowledgement-gated final
//! write. Core/Claims/Custody orchestration remains in the Core-owned route.

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_market_core_codec::{Identity as CoreIdentity, SeriesCoreAckV1};
use solana_program::{
    account_info::AccountInfo,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use super::{
    lifecycle::{
        ClosePlanV3, OccurrenceCommitPlanV3, PendingFundingAccountV3, PendingFundingPlanV3,
        RetirePlanV3,
    },
    state::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketStateSeedsV3,
        TicketStateV3,
    },
};

/// Exact composite-root width for the Series V3 profile.
pub const SERIES_ROOT_ACCOUNT_BYTES_V3: usize =
    CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3;

/// Refusal from the Series account and persistence boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesAccountErrorV3 {
    /// Owner, width, key, phase, or canonical bytes refused.
    State,
    /// Signer, writable, executable, System, or alias contract refused.
    Frame,
    /// Exact native funding or checked arithmetic refused.
    Funding,
    /// System creation or direct lamport transfer failed.
    Creation,
    /// Core acknowledgement or final state write refused.
    Commit,
}

impl From<SeriesAccountErrorV3> for ProgramError {
    fn from(value: SeriesAccountErrorV3) -> Self {
        ProgramError::Custom(80_u32.saturating_add(value as u32))
    }
}

/// Exact authenticated composite root and mutable tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesRootV3 {
    header: CapabilityRootHeaderV1,
    state: SeriesStateV3,
}

impl AuthenticatedSeriesRootV3 {
    /// Immutable common capability-root header.
    pub const fn header(self) -> CapabilityRootHeaderV1 {
        self.header
    }
    /// Mutable Series replay state.
    pub const fn state(self) -> SeriesStateV3 {
        self.state
    }
}

/// Authenticate exact root owner, PDA, selector/config, width, and tail bytes.
pub fn authenticate_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    template_id: ContentId,
    occurrence_count: u32,
) -> Result<AuthenticatedSeriesRootV3, SeriesAccountErrorV3> {
    if root.owner != program_id
        || root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || root.is_signer
        || !root.is_writable
        || root.executable
    {
        return Err(SeriesAccountErrorV3::Frame);
    }
    let data = root
        .try_borrow_data()
        .map_err(|_| SeriesAccountErrorV3::State)?;
    let header = CapabilityRootHeaderV1::decode(
        data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesAccountErrorV3::State)?,
    )
    .map_err(|_| SeriesAccountErrorV3::State)?;
    if header.selection().config() != template_id
        || Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0 != *root.key
    {
        return Err(SeriesAccountErrorV3::State);
    }
    let state = SeriesStateV3::decode(
        data.get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesAccountErrorV3::State)?,
        occurrence_count,
    )
    .map_err(|_| SeriesAccountErrorV3::State)?;
    Ok(AuthenticatedSeriesRootV3 { header, state })
}

/// Authenticate one exact mutable Ticket replay account and its PDA.
pub fn authenticate_ticket(
    program_id: &Pubkey,
    root: &Pubkey,
    ticket: &AccountInfo<'_>,
    ticket_record_id: ContentId,
) -> Result<TicketStateV3, SeriesAccountErrorV3> {
    if ticket.owner != program_id
        || ticket.data_len() != SERIES_TICKET_STATE_BYTES_V3
        || ticket.is_signer
        || !ticket.is_writable
        || ticket.executable
    {
        return Err(SeriesAccountErrorV3::Frame);
    }
    let seeds = TicketStateSeedsV3::new(root.to_bytes(), ticket_record_id);
    if Pubkey::find_program_address(&seeds.as_slices(), program_id).0 != *ticket.key {
        return Err(SeriesAccountErrorV3::State);
    }
    let data = ticket
        .try_borrow_data()
        .map_err(|_| SeriesAccountErrorV3::State)?;
    let state = TicketStateV3::decode(&data).map_err(|_| SeriesAccountErrorV3::State)?;
    if state.ticket_record_id() != ticket_record_id || state.encode().as_slice() != data.as_ref() {
        return Err(SeriesAccountErrorV3::State);
    }
    Ok(state)
}

/// Create the vacant/dust-prefunded Ticket PDA without writing replay state.
///
/// The caller invokes Core after this staging step and calls
/// [`commit_occurrence_after_ack`] only after authenticating its return data.
#[allow(clippy::too_many_arguments)]
pub fn stage_ticket_account<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    root: &Pubkey,
    ticket_record_id: ContentId,
    ticket: &AccountInfo<'info>,
    refund_owner: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    top_up: u64,
    dust_refund: u64,
) -> Result<(), ProgramError> {
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || ticket.owner != &system_program::ID
        || ticket.data_len() != 0
        || ticket.is_signer
        || !ticket.is_writable
        || ticket.executable
        || !refund_owner.is_writable
        || refund_owner.executable
        || system.key != &system_program::ID
        || !system.executable
        || [payer.key, ticket.key, refund_owner.key, system.key]
            .iter()
            .enumerate()
            .any(|(index, key)| {
                [payer.key, ticket.key, refund_owner.key, system.key]
                    .get(index.saturating_add(1)..)
                    .is_some_and(|rest| rest.contains(key))
            })
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let seeds = TicketStateSeedsV3::new(root.to_bytes(), ticket_record_id);
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if expected != *ticket.key {
        return Err(SeriesAccountErrorV3::State.into());
    }
    if top_up > 0 {
        invoke(
            &transfer(payer.key, ticket.key, top_up),
            &[payer.clone(), ticket.clone(), system.clone()],
        )
        .map_err(|_| SeriesAccountErrorV3::Creation)?;
    }
    let [domain, root_seed, record] = seeds.as_slices();
    let bump_seed = [bump];
    let signer = [domain, root_seed, record, bump_seed.as_slice()];
    invoke_signed(
        &allocate(
            ticket.key,
            u64::try_from(SERIES_TICKET_STATE_BYTES_V3)
                .map_err(|_| SeriesAccountErrorV3::Creation)?,
        ),
        &[ticket.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| SeriesAccountErrorV3::Creation)?;
    invoke_signed(
        &assign(ticket.key, program_id),
        &[ticket.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| SeriesAccountErrorV3::Creation)?;
    transfer_owned(program_id, ticket, refund_owner, dust_refund)?;
    Ok(())
}

/// Stage every exact FundingState PDA from Ticket native custody.
///
/// Realm-collateral vaults are authenticated in the pure plan and are never
/// added to or substituted for these lamports. Any later Core refusal rolls
/// all allocations, assignments, transfers, and bytes back atomically.
#[allow(clippy::too_many_arguments)]
pub fn stage_pending_funding<'info>(
    program_id: &Pubkey,
    ticket: &AccountInfo<'info>,
    refund_owner: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    market: Pubkey,
    generation: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    observations: &[PendingFundingAccountV3],
    accounts: &[AccountInfo<'info>],
    plan: PendingFundingPlanV3,
) -> Result<(), ProgramError> {
    if ticket.owner != program_id
        || !ticket.is_writable
        || ticket.executable
        || !refund_owner.is_writable
        || refund_owner.executable
        || system.key != &system_program::ID
        || !system.executable
        || observations.len() != accounts.len()
        || observations.len() != usize::from(plan.count())
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    for (index, (observation, account)) in observations
        .iter()
        .copied()
        .zip(accounts.iter())
        .enumerate()
    {
        if observation.key() != *account.key
            || account.owner != &system_program::ID
            || account.data_len() != 0
            || account.is_signer
            || !account.is_writable
            || account.executable
            || account.key == ticket.key
            || account.key == refund_owner.key
            || accounts
                .get(..index)
                .ok_or(SeriesAccountErrorV3::Frame)?
                .iter()
                .any(|prior| prior.key == account.key)
        {
            return Err(SeriesAccountErrorV3::Frame.into());
        }
        let state = observation.state();
        let derivation = CapabilityFundingDerivationV1::new(
            market.to_bytes(),
            generation,
            manifest_id,
            manifest,
            state,
        )
        .map_err(|_| SeriesAccountErrorV3::State)?;
        let (expected, bump) =
            Pubkey::find_program_address(&derivation.seed_components(), program_id);
        if expected != *account.key {
            return Err(SeriesAccountErrorV3::State.into());
        }
        transfer_owned(
            program_id,
            ticket,
            account,
            plan.top_up(index).ok_or(SeriesAccountErrorV3::Funding)?,
        )?;
        let [domain, market_seed, generation_seed, entry, config, release] =
            derivation.seed_components();
        let bump_seed = [bump];
        let signer = [
            domain,
            market_seed,
            generation_seed,
            entry,
            config,
            release,
            bump_seed.as_slice(),
        ];
        invoke_signed(
            &allocate(
                account.key,
                u64::try_from(FUNDING_STATE_BYTES).map_err(|_| SeriesAccountErrorV3::Creation)?,
            ),
            &[account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| SeriesAccountErrorV3::Creation)?;
        invoke_signed(
            &assign(account.key, program_id),
            &[account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| SeriesAccountErrorV3::Creation)?;
        transfer_owned(
            program_id,
            account,
            refund_owner,
            plan.preexisting_surplus_refund(index)
                .ok_or(SeriesAccountErrorV3::Funding)?,
        )?;
        let encoded = state.to_bytes();
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV3::Commit)?;
        if data.len() != FUNDING_STATE_BYTES {
            return Err(SeriesAccountErrorV3::Commit.into());
        }
        data.copy_from_slice(&encoded);
    }
    transfer_owned(
        program_id,
        ticket,
        refund_owner,
        plan.ticket_capability_refund(),
    )
}

/// Transfer one classified native compartment out of Trading-owned custody.
pub fn transfer_ticket_lamports(
    program_id: &Pubkey,
    ticket: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    transfer_owned(program_id, ticket, destination, amount)
}

/// Validate exact Core return data, then persist Ticket and root candidates.
///
/// This is the only successful occurrence write boundary. Root is written
/// after Ticket; a local failure still returns an instruction error and the
/// runtime rolls both accounts and all staged child effects back.
#[allow(clippy::too_many_arguments)]
pub fn commit_occurrence_after_ack(
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    plan: OccurrenceCommitPlanV3,
    acknowledgement: SeriesCoreAckV1,
    expected_core_program: CoreIdentity,
    request_digest: CoreIdentity,
    observed_post_resource_digest: CoreIdentity,
) -> Result<(), ProgramError> {
    let (root_tail, ticket_bytes) = plan
        .commit_after_ack(
            acknowledgement,
            expected_core_program,
            request_digest,
            observed_post_resource_digest,
        )
        .map_err(|_| SeriesAccountErrorV3::Commit)?;
    write_occurrence_candidates(root, ticket, &root_tail, &ticket_bytes)
}

/// Persist a Prepare or Expire candidate after direct controller effects.
///
/// The canonical hot outer calls this only after finalized controller
/// authentication and any selected Custody CPI/receipt postchecks. Consume
/// cannot use this function because its plan requires a Core acknowledgment.
pub fn commit_controller_occurrence(
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    plan: OccurrenceCommitPlanV3,
) -> Result<(), ProgramError> {
    let (root_tail, ticket_bytes) = plan
        .commit_controller()
        .map_err(|_| SeriesAccountErrorV3::Commit)?;
    write_occurrence_candidates(root, ticket, &root_tail, &ticket_bytes)
}

fn write_occurrence_candidates(
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    root_tail: &[u8; SERIES_STATE_BYTES_V3],
    ticket_bytes: &[u8; SERIES_TICKET_STATE_BYTES_V3],
) -> Result<(), ProgramError> {
    {
        let mut data = ticket
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV3::Commit)?;
        if data.len() != SERIES_TICKET_STATE_BYTES_V3 {
            return Err(SeriesAccountErrorV3::Commit.into());
        }
        data.copy_from_slice(ticket_bytes);
    }
    {
        let mut data = root
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV3::Commit)?;
        let tail = data
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesAccountErrorV3::Commit)?;
        if tail.len() != SERIES_STATE_BYTES_V3 {
            return Err(SeriesAccountErrorV3::Commit.into());
        }
        tail.copy_from_slice(root_tail);
    }
    Ok(())
}

/// Retire one terminal Ticket and persist its root replay decrement last.
///
/// The caller supplies only a plan produced from the authenticated immutable
/// Ticket and mutable prestates. The runtime rolls back the Ticket deletion and
/// refund if the final root write fails.
pub fn commit_retire_ticket(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    refund_owner: &AccountInfo<'_>,
    occurrence_count: u32,
    plan: RetirePlanV3,
) -> Result<(), ProgramError> {
    if root.owner != program_id
        || root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || root.is_signer
        || !root.is_writable
        || root.executable
        || ticket.owner != program_id
        || ticket.data_len() != SERIES_TICKET_STATE_BYTES_V3
        || ticket.is_signer
        || !ticket.is_writable
        || ticket.executable
        || !refund_owner.is_writable
        || refund_owner.executable
        || refund_owner.key != &plan.refund_owner()
        || ticket.lamports() != plan.lamports_to_refund_owner()
        || root.key == ticket.key
        || root.key == refund_owner.key
        || ticket.key == refund_owner.key
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let root_tail = plan
        .series_after()
        .encode(occurrence_count)
        .map_err(|_| SeriesAccountErrorV3::State)?;
    transfer_owned(
        program_id,
        ticket,
        refund_owner,
        plan.lamports_to_refund_owner(),
    )?;
    ticket
        .try_borrow_mut_data()
        .map_err(|_| SeriesAccountErrorV3::Commit)?
        .fill(0);
    ticket.resize(0).map_err(|_| SeriesAccountErrorV3::Commit)?;
    ticket.assign(&system_program::ID);
    write_root_tail(root, &root_tail)?;
    if ticket.lamports() != 0 || !ticket.data_is_empty() || ticket.owner != &system_program::ID {
        return Err(SeriesAccountErrorV3::Commit.into());
    }
    Ok(())
}

/// Delete a terminal Series root and refund every classified native lamport.
///
/// Root Rent, prepaid close Rent, and unsolicited donations remain separately
/// classified in `plan`; Hoard collateral is never part of this account path.
pub fn commit_close_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    beneficiary: &AccountInfo<'_>,
    plan: ClosePlanV3,
) -> Result<(), ProgramError> {
    let total = plan
        .root_rent()
        .checked_add(plan.close_rent())
        .and_then(|value| value.checked_add(plan.donation()))
        .ok_or(SeriesAccountErrorV3::Funding)?;
    if root.owner != program_id
        || root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || root.is_signer
        || !root.is_writable
        || root.executable
        || !beneficiary.is_writable
        || beneficiary.executable
        || beneficiary.key != &plan.beneficiary()
        || root.key == beneficiary.key
        || root.lamports() != total
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    transfer_owned(program_id, root, beneficiary, total)?;
    root.try_borrow_mut_data()
        .map_err(|_| SeriesAccountErrorV3::Commit)?
        .fill(0);
    root.resize(0).map_err(|_| SeriesAccountErrorV3::Commit)?;
    root.assign(&system_program::ID);
    if root.lamports() != 0 || !root.data_is_empty() || root.owner != &system_program::ID {
        return Err(SeriesAccountErrorV3::Commit.into());
    }
    Ok(())
}

fn write_root_tail(
    root: &AccountInfo<'_>,
    root_tail: &[u8; SERIES_STATE_BYTES_V3],
) -> Result<(), ProgramError> {
    let mut data = root
        .try_borrow_mut_data()
        .map_err(|_| SeriesAccountErrorV3::Commit)?;
    let tail = data
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(SeriesAccountErrorV3::Commit)?;
    if tail.len() != SERIES_STATE_BYTES_V3 {
        return Err(SeriesAccountErrorV3::Commit.into());
    }
    tail.copy_from_slice(root_tail);
    Ok(())
}

fn transfer_owned(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    if amount == 0 {
        return Ok(());
    }
    if source.owner != program_id
        || !source.is_writable
        || !destination.is_writable
        || source.executable
        || destination.executable
        || source.key == destination.key
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(SeriesAccountErrorV3::Funding)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(SeriesAccountErrorV3::Funding)?;
    **source
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesAccountErrorV3::Funding)? = source_after;
    **destination
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesAccountErrorV3::Funding)? = destination_after;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec};

    use dclutch_series_v3_kernel::{TemplateV3, admit_ticket};

    use super::*;
    use crate::series::{
        generated,
        lifecycle::{plan_close, plan_retire},
        state::TicketPhaseV3,
    };

    #[repr(C)]
    struct SerializedKey {
        original_data_len: u32,
        key: Pubkey,
    }

    fn runtime_account(
        key: Pubkey,
        owner: Pubkey,
        writable: bool,
        lamports: u64,
        initial_data: &[u8],
    ) -> AccountInfo<'static> {
        let serialized_key = Box::leak(Box::new(SerializedKey {
            original_data_len: u32::try_from(initial_data.len()).expect("test account width"),
            key,
        }));
        let mut serialized_data = vec![0_u8; 8 + initial_data.len()];
        serialized_data
            .get_mut(..8)
            .expect("serialized account length prefix")
            .copy_from_slice(
                &u64::try_from(initial_data.len())
                    .expect("test account width")
                    .to_le_bytes(),
            );
        serialized_data
            .get_mut(8..)
            .expect("serialized account data")
            .copy_from_slice(initial_data);
        let serialized_data = Box::leak(serialized_data.into_boxed_slice());
        let account_data = serialized_data
            .get_mut(8..)
            .expect("serialized account data");
        AccountInfo::new(
            &serialized_key.key,
            false,
            writable,
            Box::leak(Box::new(lamports)),
            account_data,
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn terminal_series(close_rent: u64, occurrence_count: u32) -> SeriesStateV3 {
        let mut state = SeriesStateV3::new(close_rent);
        for _ in 0..occurrence_count {
            state = state
                .prepare_ticket(state.revision())
                .expect("prepare terminal fixture");
            state = state
                .settle_current(state.revision(), occurrence_count)
                .expect("settle terminal fixture");
            state = state
                .retire_ticket(state.revision())
                .expect("retire terminal fixture");
        }
        state
    }

    #[test]
    fn retire_refunds_exact_ticket_and_commits_root_decrement_last() {
        let program_id = Pubkey::new_from_array([41; 32]);
        let root_key = Pubkey::new_from_array([42; 32]);
        let ticket_key = Pubkey::new_from_array([43; 32]);
        let admitted_ticket = admit_ticket(&generated::SERIES_EXAMPLE_TICKET_V3).expect("Ticket");
        let refund_key = Pubkey::new_from_array(admitted_ticket.ticket().refund_owner().to_bytes());
        let before = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare")
            .settle_current(1, 1)
            .expect("settle");
        let ticket_state = TicketStateV3::prepared(admitted_ticket.content_id())
            .settle(0, TicketPhaseV3::Consumed)
            .expect("terminal Ticket");
        let plan =
            plan_retire(before, ticket_state, admitted_ticket, 2, 1, 11).expect("retire plan");
        let mut root_data = vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3];
        root_data
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail")
            .copy_from_slice(&before.encode(1).expect("root before"));
        let root = runtime_account(root_key, program_id, true, 19, &root_data);
        let ticket = runtime_account(ticket_key, program_id, true, 11, &ticket_state.encode());
        let refund = runtime_account(refund_key, system_program::ID, true, 5, &[]);

        commit_retire_ticket(&program_id, &root, &ticket, &refund, 1, plan).expect("commit retire");

        assert_eq!(ticket.lamports(), 0);
        assert_eq!(ticket.owner, &system_program::ID);
        assert!(ticket.data_is_empty());
        assert_eq!(refund.lamports(), 16);
        let root_bytes = root.try_borrow_data().expect("root data");
        assert_eq!(
            SeriesStateV3::decode(
                root_bytes
                    .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                    .expect("root tail"),
                1,
            ),
            Ok(plan.series_after())
        );
    }

    #[test]
    fn retire_refuses_wrong_beneficiary_and_amount_before_mutation() {
        let program_id = Pubkey::new_from_array([51; 32]);
        let admitted_ticket = admit_ticket(&generated::SERIES_EXAMPLE_TICKET_V3).expect("Ticket");
        let before = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare")
            .settle_current(1, 1)
            .expect("settle");
        let ticket_state = TicketStateV3::prepared(admitted_ticket.content_id())
            .settle(0, TicketPhaseV3::Expired)
            .expect("terminal Ticket");
        assert_eq!(
            plan_retire(before, ticket_state, admitted_ticket, 2, 0, 11),
            Err(crate::series::lifecycle::LifecycleErrorV3::Replay)
        );
        let plan =
            plan_retire(before, ticket_state, admitted_ticket, 2, 1, 11).expect("retire plan");
        let root = runtime_account(
            Pubkey::new_from_array([52; 32]),
            program_id,
            true,
            19,
            &vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3],
        );
        let ticket = runtime_account(
            Pubkey::new_from_array([53; 32]),
            program_id,
            true,
            12,
            &ticket_state.encode(),
        );
        let wrong_refund = runtime_account(
            Pubkey::new_from_array([54; 32]),
            system_program::ID,
            true,
            5,
            &[],
        );

        assert_eq!(
            commit_retire_ticket(&program_id, &root, &ticket, &wrong_refund, 1, plan),
            Err(SeriesAccountErrorV3::Frame.into())
        );
        assert_eq!(ticket.lamports(), 12);
        assert_eq!(wrong_refund.lamports(), 5);
        assert_eq!(ticket.owner, &program_id);
    }

    #[test]
    fn terminal_close_refunds_exact_classifications_and_deletes_root() {
        let program_id = Pubkey::new_from_array([61; 32]);
        let template =
            TemplateV3::decode(&generated::SERIES_EXAMPLE_TEMPLATE_V3).expect("Template");
        let state = terminal_series(template.close_rent(), template.occurrence_count());
        let plan = plan_close(template, state, state.revision(), 20, 10).expect("close plan");
        assert_eq!(plan.close_rent(), 7);
        assert_eq!(plan.root_rent(), 10);
        assert_eq!(plan.donation(), 3);
        let beneficiary_key = Pubkey::new_from_array(template.refund_owner().to_bytes());
        let root = runtime_account(
            Pubkey::new_from_array([62; 32]),
            program_id,
            true,
            20,
            &vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3],
        );
        let beneficiary = runtime_account(beneficiary_key, system_program::ID, true, 4, &[]);

        commit_close_root(&program_id, &root, &beneficiary, plan).expect("close root");

        assert_eq!(root.lamports(), 0);
        assert!(root.data_is_empty());
        assert_eq!(root.owner, &system_program::ID);
        assert_eq!(beneficiary.lamports(), 24);
    }

    #[test]
    fn terminal_close_refuses_unclassified_lamport_mismatch() {
        let program_id = Pubkey::new_from_array([71; 32]);
        let template =
            TemplateV3::decode(&generated::SERIES_EXAMPLE_TEMPLATE_V3).expect("Template");
        let state = terminal_series(template.close_rent(), template.occurrence_count());
        let plan = plan_close(template, state, state.revision(), 20, 10).expect("close plan");
        let root = runtime_account(
            Pubkey::new_from_array([72; 32]),
            program_id,
            true,
            21,
            &vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3],
        );
        let beneficiary = runtime_account(
            Pubkey::new_from_array(template.refund_owner().to_bytes()),
            system_program::ID,
            true,
            4,
            &[],
        );

        assert_eq!(
            commit_close_root(&program_id, &root, &beneficiary, plan),
            Err(SeriesAccountErrorV3::Frame.into())
        );
        assert_eq!(root.lamports(), 21);
        assert_eq!(beneficiary.lamports(), 4);
        assert_eq!(root.owner, &program_id);
    }
}
