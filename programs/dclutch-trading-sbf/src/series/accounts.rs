//! Solana account boundary for recurring Series V2.
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
    lifecycle::{OccurrenceCommitPlanV2, PendingFundingAccountV2, PendingFundingPlanV2},
    state::{
        SERIES_STATE_BYTES_V2, SERIES_TICKET_STATE_BYTES_V2, SeriesStateV2, TicketStateSeedsV2,
        TicketStateV2,
    },
};

/// Exact composite-root width for the Series V2 profile.
pub const SERIES_ROOT_ACCOUNT_BYTES_V2: usize =
    CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V2;

/// Refusal from the Series account and persistence boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesAccountErrorV2 {
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

impl From<SeriesAccountErrorV2> for ProgramError {
    fn from(value: SeriesAccountErrorV2) -> Self {
        ProgramError::Custom(80_u32.saturating_add(value as u32))
    }
}

/// Exact authenticated composite root and mutable tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesRootV2 {
    header: CapabilityRootHeaderV1,
    state: SeriesStateV2,
}

impl AuthenticatedSeriesRootV2 {
    /// Immutable common capability-root header.
    pub const fn header(self) -> CapabilityRootHeaderV1 {
        self.header
    }
    /// Mutable Series replay state.
    pub const fn state(self) -> SeriesStateV2 {
        self.state
    }
}

/// Authenticate exact root owner, PDA, selector/config, width, and tail bytes.
pub fn authenticate_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    template_id: ContentId,
    occurrence_count: u32,
) -> Result<AuthenticatedSeriesRootV2, SeriesAccountErrorV2> {
    if root.owner != program_id
        || root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V2
        || root.is_signer
        || !root.is_writable
        || root.executable
    {
        return Err(SeriesAccountErrorV2::Frame);
    }
    let data = root
        .try_borrow_data()
        .map_err(|_| SeriesAccountErrorV2::State)?;
    let header = CapabilityRootHeaderV1::decode(
        data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesAccountErrorV2::State)?,
    )
    .map_err(|_| SeriesAccountErrorV2::State)?;
    if header.selection().config() != template_id
        || Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0 != *root.key
    {
        return Err(SeriesAccountErrorV2::State);
    }
    let state = SeriesStateV2::decode(
        data.get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesAccountErrorV2::State)?,
        occurrence_count,
    )
    .map_err(|_| SeriesAccountErrorV2::State)?;
    Ok(AuthenticatedSeriesRootV2 { header, state })
}

/// Authenticate one exact mutable Ticket replay account and its PDA.
pub fn authenticate_ticket(
    program_id: &Pubkey,
    root: &Pubkey,
    ticket: &AccountInfo<'_>,
    ticket_record_id: ContentId,
) -> Result<TicketStateV2, SeriesAccountErrorV2> {
    if ticket.owner != program_id
        || ticket.data_len() != SERIES_TICKET_STATE_BYTES_V2
        || ticket.is_signer
        || !ticket.is_writable
        || ticket.executable
    {
        return Err(SeriesAccountErrorV2::Frame);
    }
    let seeds = TicketStateSeedsV2::new(root.to_bytes(), ticket_record_id);
    if Pubkey::find_program_address(&seeds.as_slices(), program_id).0 != *ticket.key {
        return Err(SeriesAccountErrorV2::State);
    }
    let data = ticket
        .try_borrow_data()
        .map_err(|_| SeriesAccountErrorV2::State)?;
    let state = TicketStateV2::decode(&data).map_err(|_| SeriesAccountErrorV2::State)?;
    if state.ticket_record_id() != ticket_record_id || state.encode().as_slice() != data.as_ref() {
        return Err(SeriesAccountErrorV2::State);
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
        return Err(SeriesAccountErrorV2::Frame.into());
    }
    let seeds = TicketStateSeedsV2::new(root.to_bytes(), ticket_record_id);
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if expected != *ticket.key {
        return Err(SeriesAccountErrorV2::State.into());
    }
    if top_up > 0 {
        invoke(
            &transfer(payer.key, ticket.key, top_up),
            &[payer.clone(), ticket.clone(), system.clone()],
        )
        .map_err(|_| SeriesAccountErrorV2::Creation)?;
    }
    let [domain, root_seed, record] = seeds.as_slices();
    let bump_seed = [bump];
    let signer = [domain, root_seed, record, bump_seed.as_slice()];
    invoke_signed(
        &allocate(
            ticket.key,
            u64::try_from(SERIES_TICKET_STATE_BYTES_V2)
                .map_err(|_| SeriesAccountErrorV2::Creation)?,
        ),
        &[ticket.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| SeriesAccountErrorV2::Creation)?;
    invoke_signed(
        &assign(ticket.key, program_id),
        &[ticket.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| SeriesAccountErrorV2::Creation)?;
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
    observations: &[PendingFundingAccountV2],
    accounts: &[AccountInfo<'info>],
    plan: PendingFundingPlanV2,
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
        return Err(SeriesAccountErrorV2::Frame.into());
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
                .ok_or(SeriesAccountErrorV2::Frame)?
                .iter()
                .any(|prior| prior.key == account.key)
        {
            return Err(SeriesAccountErrorV2::Frame.into());
        }
        let state = observation.state();
        let derivation = CapabilityFundingDerivationV1::new(
            market.to_bytes(),
            generation,
            manifest_id,
            manifest,
            state,
        )
        .map_err(|_| SeriesAccountErrorV2::State)?;
        let (expected, bump) =
            Pubkey::find_program_address(&derivation.seed_components(), program_id);
        if expected != *account.key {
            return Err(SeriesAccountErrorV2::State.into());
        }
        transfer_owned(
            program_id,
            ticket,
            account,
            plan.top_up(index).ok_or(SeriesAccountErrorV2::Funding)?,
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
                u64::try_from(FUNDING_STATE_BYTES).map_err(|_| SeriesAccountErrorV2::Creation)?,
            ),
            &[account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| SeriesAccountErrorV2::Creation)?;
        invoke_signed(
            &assign(account.key, program_id),
            &[account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| SeriesAccountErrorV2::Creation)?;
        transfer_owned(
            program_id,
            account,
            refund_owner,
            plan.preexisting_surplus_refund(index)
                .ok_or(SeriesAccountErrorV2::Funding)?,
        )?;
        let encoded = state.to_bytes();
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV2::Commit)?;
        if data.len() != FUNDING_STATE_BYTES {
            return Err(SeriesAccountErrorV2::Commit.into());
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
    plan: OccurrenceCommitPlanV2,
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
        .map_err(|_| SeriesAccountErrorV2::Commit)?;
    {
        let mut data = ticket
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV2::Commit)?;
        if data.len() != SERIES_TICKET_STATE_BYTES_V2 {
            return Err(SeriesAccountErrorV2::Commit.into());
        }
        data.copy_from_slice(&ticket_bytes);
    }
    {
        let mut data = root
            .try_borrow_mut_data()
            .map_err(|_| SeriesAccountErrorV2::Commit)?;
        let tail = data
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesAccountErrorV2::Commit)?;
        if tail.len() != SERIES_STATE_BYTES_V2 {
            return Err(SeriesAccountErrorV2::Commit.into());
        }
        tail.copy_from_slice(&root_tail);
    }
    Ok(())
}

/// Retire one terminal ticket, returning Rent and donations to its beneficiary.
pub fn retire_ticket_account(
    program_id: &Pubkey,
    ticket: &AccountInfo<'_>,
    refund_owner: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if ticket.owner != program_id
        || ticket.data_len() != SERIES_TICKET_STATE_BYTES_V2
        || !ticket.is_writable
        || ticket.executable
        || !refund_owner.is_writable
        || refund_owner.executable
        || ticket.key == refund_owner.key
    {
        return Err(SeriesAccountErrorV2::Frame.into());
    }
    let lamports = ticket.lamports();
    transfer_owned(program_id, ticket, refund_owner, lamports)?;
    ticket.resize(0).map_err(|_| SeriesAccountErrorV2::Commit)?;
    ticket.assign(&system_program::ID);
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
        return Err(SeriesAccountErrorV2::Frame.into());
    }
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(SeriesAccountErrorV2::Funding)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(SeriesAccountErrorV2::Funding)?;
    **source
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesAccountErrorV2::Funding)? = source_after;
    **destination
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesAccountErrorV2::Funding)? = destination_after;
    Ok(())
}
