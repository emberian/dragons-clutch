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
///
/// Trading's sub-band `0x4100` (decision 0007). The discriminants are written
/// out rather than left implicit and shifted inside the `From` impl: the
/// shifted form reported `80 + n` on chain while reporting `0 + n` to anything
/// reading the source, and because the enum carried no `#[repr]` the gauntlet
/// census -- which admits a refusal taxonomy only from `#[repr]`-annotated
/// enums -- could not see this boundary at all.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesAccountErrorV3 {
    /// Owner, width, key, phase, or canonical bytes refused.
    State = 0x4100,
    /// Signer, writable, executable, System, or alias contract refused.
    Frame = 0x4101,
    /// Exact native funding or checked arithmetic refused.
    Funding = 0x4102,
    /// System creation or direct lamport transfer failed.
    Creation = 0x4103,
    /// Core acknowledgement or final state write refused.
    Commit = 0x4104,
}

const _: () = assert!(
    SeriesAccountErrorV3::State as u32 == dclutch_refusal_registry::TRADING_REFUSAL_BASE + 0x100,
    "the Series account boundary must sit in Trading's registered band"
);

impl From<SeriesAccountErrorV3> for ProgramError {
    fn from(value: SeriesAccountErrorV3) -> Self {
        ProgramError::Custom(value as u32)
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

/// Credit every unused expired-Ticket native compartment to lifecycle Rent V2.
///
/// This deliberately exposes no arbitrary destination/amount capability.
/// Hoard collateral is absent from the native remainder plan and is returned
/// only through the authenticated Custody expiration routes.
pub fn credit_expired_ticket_remainders(
    program_id: &Pubkey,
    ticket: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    plan: OccurrenceCommitPlanV3,
) -> Result<(), ProgramError> {
    let sink = plan
        .terminal_rent_sink()
        .ok_or(SeriesAccountErrorV3::Funding)?;
    let amount = plan.native_from_ticket();
    if rent_credit.key.to_bytes() != sink.credit_account().to_bytes()
        || plan.core_request().is_some()
        || plan
            .native_remainders()
            .total()
            .map_err(|_| SeriesAccountErrorV3::Funding)?
            != amount
    {
        return Err(SeriesAccountErrorV3::Funding.into());
    }
    transfer_owned(program_id, ticket, rent_credit, amount)
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

/// Retire one terminal Ticket into Rent V2 and persist its root decrement last.
///
/// The caller supplies only a plan produced from the authenticated immutable
/// Ticket and mutable prestates. The runtime rolls back the Ticket deletion and
/// refund if the final root write fails.
pub fn commit_retire_ticket(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
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
        || !rent_credit.is_writable
        || rent_credit.executable
        || rent_credit.key.to_bytes() != plan.rent_sink().credit_account().to_bytes()
        || ticket.lamports()
            != plan
                .total_credit()
                .map_err(|_| SeriesAccountErrorV3::Funding)?
        || root.key == ticket.key
        || root.key == rent_credit.key
        || ticket.key == rent_credit.key
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
        rent_credit,
        plan.total_credit()
            .map_err(|_| SeriesAccountErrorV3::Funding)?,
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

/// Delete a terminal Series root and credit every typed lamport to Rent V2.
///
/// Root Rent, prepaid close Rent, and unsolicited donations remain separately
/// classified in `plan`; Hoard collateral is never part of this account path.
pub fn commit_close_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    plan: ClosePlanV3,
) -> Result<(), ProgramError> {
    let total = plan
        .total_credit()
        .map_err(|_| SeriesAccountErrorV3::Funding)?;
    if root.owner != program_id
        || root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || root.is_signer
        || !root.is_writable
        || root.executable
        || !rent_credit.is_writable
        || rent_credit.executable
        || rent_credit.key.to_bytes() != plan.rent_sink().credit_account().to_bytes()
        || root.key == rent_credit.key
        || root.lamports() != total
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    transfer_owned(program_id, root, rent_credit, total)?;
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

    use dclutch_core_contract::ContentId;
    use dclutch_rent_contract::{
        RefundAuthority,
        lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
    };
    use dclutch_series_v3_kernel::{AccountKeyV3, TemplateV3, admit_ticket};

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

    fn rent_sink(
        credit: Pubkey,
        refund_wallet: AccountKeyV3,
    ) -> crate::series::lifecycle::SeriesLifecycleRentSinkV3 {
        let state = LifecycleRentCreditV2::new(
            RefundAuthority::new(refund_wallet.to_bytes()).expect("refund wallet"),
            LifecycleAccountIdV2::new([91; 32]).expect("Market"),
            LifecycleAccountIdV2::new([92; 32]).expect("release"),
            3,
            4,
        )
        .expect("Rent V2");
        crate::series::lifecycle::SeriesLifecycleRentSinkV3::admit(
            AccountKeyV3::new(credit.to_bytes()).expect("credit"),
            &state.to_bytes(),
            AccountKeyV3::new([91; 32]).expect("Market"),
            ContentId::new([92; 32]).expect("release"),
            3,
            refund_wallet,
        )
        .expect("sink")
    }

    #[test]
    fn retire_credits_exact_ticket_balance_and_commits_root_decrement_last() {
        let program_id = Pubkey::new_from_array([41; 32]);
        let root_key = Pubkey::new_from_array([42; 32]);
        let ticket_key = Pubkey::new_from_array([43; 32]);
        let admitted_ticket = admit_ticket(&generated::SERIES_EXAMPLE_TICKET_V3).expect("Ticket");
        let credit_key = Pubkey::new_from_array([44; 32]);
        let sink = rent_sink(credit_key, admitted_ticket.ticket().refund_owner());
        let before = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare")
            .settle_current(1, 1)
            .expect("settle");
        let ticket_state = TicketStateV3::prepared(admitted_ticket.content_id())
            .settle(0, TicketPhaseV3::Consumed)
            .expect("terminal Ticket");
        let plan = plan_retire(1, before, ticket_state, admitted_ticket, 2, 1, 11, 10, sink)
            .expect("retire plan");
        let mut root_data = vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3];
        root_data
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail")
            .copy_from_slice(&before.encode(1).expect("root before"));
        let root = runtime_account(root_key, program_id, true, 19, &root_data);
        let ticket = runtime_account(ticket_key, program_id, true, 11, &ticket_state.encode());
        let credit = runtime_account(credit_key, system_program::ID, true, 5, &[]);

        commit_retire_ticket(&program_id, &root, &ticket, &credit, 1, plan).expect("commit retire");

        assert_eq!(ticket.lamports(), 0);
        assert_eq!(ticket.owner, &system_program::ID);
        assert!(ticket.data_is_empty());
        assert_eq!(credit.lamports(), 16);
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
    fn retire_refuses_stale_revision_or_substituted_credit_before_mutation() {
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
        let credit_key = Pubkey::new_from_array([55; 32]);
        let sink = rent_sink(credit_key, admitted_ticket.ticket().refund_owner());
        assert_eq!(
            plan_retire(1, before, ticket_state, admitted_ticket, 2, 0, 11, 10, sink),
            Err(crate::series::lifecycle::LifecycleErrorV3::Replay)
        );
        let plan = plan_retire(1, before, ticket_state, admitted_ticket, 2, 1, 11, 10, sink)
            .expect("retire plan");
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
        let wrong_credit = runtime_account(
            Pubkey::new_from_array([54; 32]),
            system_program::ID,
            true,
            5,
            &[],
        );

        assert_eq!(
            commit_retire_ticket(&program_id, &root, &ticket, &wrong_credit, 1, plan),
            Err(SeriesAccountErrorV3::Frame.into())
        );
        assert_eq!(ticket.lamports(), 12);
        assert_eq!(wrong_credit.lamports(), 5);
        assert_eq!(ticket.owner, &program_id);
    }

    #[test]
    fn terminal_close_credits_exact_classifications_and_deletes_root() {
        let program_id = Pubkey::new_from_array([61; 32]);
        let template =
            TemplateV3::decode(&generated::SERIES_EXAMPLE_TEMPLATE_V3).expect("Template");
        let state = terminal_series(template.close_rent(), template.occurrence_count());
        let credit_key = Pubkey::new_from_array([63; 32]);
        let sink = rent_sink(credit_key, template.refund_owner());
        let plan = plan_close(template, state, state.revision(), 20, 10, sink).expect("close plan");
        assert_eq!(plan.close_rent(), 7);
        assert_eq!(plan.root_rent(), 10);
        assert_eq!(plan.donation(), 3);
        let root = runtime_account(
            Pubkey::new_from_array([62; 32]),
            program_id,
            true,
            20,
            &vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3],
        );
        let credit = runtime_account(credit_key, system_program::ID, true, 4, &[]);

        commit_close_root(&program_id, &root, &credit, plan).expect("close root");

        assert_eq!(root.lamports(), 0);
        assert!(root.data_is_empty());
        assert_eq!(root.owner, &system_program::ID);
        assert_eq!(credit.lamports(), 24);
    }

    #[test]
    fn terminal_close_refuses_unclassified_lamport_mismatch() {
        let program_id = Pubkey::new_from_array([71; 32]);
        let template =
            TemplateV3::decode(&generated::SERIES_EXAMPLE_TEMPLATE_V3).expect("Template");
        let state = terminal_series(template.close_rent(), template.occurrence_count());
        let credit_key = Pubkey::new_from_array([73; 32]);
        let sink = rent_sink(credit_key, template.refund_owner());
        let plan = plan_close(template, state, state.revision(), 20, 10, sink).expect("close plan");
        let root = runtime_account(
            Pubkey::new_from_array([72; 32]),
            program_id,
            true,
            21,
            &vec![0_u8; SERIES_ROOT_ACCOUNT_BYTES_V3],
        );
        let credit = runtime_account(credit_key, system_program::ID, true, 4, &[]);

        assert_eq!(
            commit_close_root(&program_id, &root, &credit, plan),
            Err(SeriesAccountErrorV3::Frame.into())
        );
        assert_eq!(root.lamports(), 21);
        assert_eq!(credit.lamports(), 4);
        assert_eq!(root.owner, &program_id);
    }
}
