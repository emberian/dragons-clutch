//! Permissionless refund of one expired, still-unallocated Series permit PDA.
//!
//! The request carries the deterministic permit candidate because the account
//! is still System-owned and has no data. The adapter independently
//! authenticates the immutable Series records and ordered occurrence proof,
//! recomputes the retry deadline, observes the terminal Trading replay, and
//! transfers every prefunded lamport to the immutable RentCredit.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_market_core_codec::{
    Role, SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_series_v3_kernel::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, admit_occurrence_bytes, admit_ticket,
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketPhaseV3,
        TicketStateSeedsV3, TicketStateV3,
    },
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, program::invoke_signed, pubkey::Pubkey,
    rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    CoreSbfError,
    frame::require_distinct,
    infrastructure::authenticate_profile,
    records::authenticate_finalized_record,
    release::{authenticate_role, identity},
};

/// Exact permissionless expiry account count.
pub const SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1: usize = 25;

struct ExpiryAccounts<'accounts, 'info> {
    permit: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    infrastructure_profile: &'accounts AccountInfo<'info>,
    registry_artifact_raw: &'accounts AccountInfo<'info>,
    registry_artifact_staging: &'accounts AccountInfo<'info>,
    registry_program: &'accounts AccountInfo<'info>,
    registry_programdata: &'accounts AccountInfo<'info>,
    rent_artifact_raw: &'accounts AccountInfo<'info>,
    rent_artifact_staging: &'accounts AccountInfo<'info>,
    rent_programdata: &'accounts AccountInfo<'info>,
    activation_cache: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    ticket_state: &'accounts AccountInfo<'info>,
    template_raw: &'accounts AccountInfo<'info>,
    template_staging: &'accounts AccountInfo<'info>,
    occurrence_raw: &'accounts AccountInfo<'info>,
    occurrence_staging: &'accounts AccountInfo<'info>,
    ticket_raw: &'accounts AccountInfo<'info>,
    ticket_staging: &'accounts AccountInfo<'info>,
    clock: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> ExpiryAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        if accounts.len() != SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        require_distinct(accounts)?;
        let value = Self {
            permit: account(accounts, 0)?,
            rent_credit: account(accounts, 1)?,
            rent_program: account(accounts, 2)?,
            infrastructure_profile: account(accounts, 3)?,
            registry_artifact_raw: account(accounts, 4)?,
            registry_artifact_staging: account(accounts, 5)?,
            registry_program: account(accounts, 6)?,
            registry_programdata: account(accounts, 7)?,
            rent_artifact_raw: account(accounts, 8)?,
            rent_artifact_staging: account(accounts, 9)?,
            rent_programdata: account(accounts, 10)?,
            activation_cache: account(accounts, 11)?,
            trading_program: account(accounts, 12)?,
            trading_programdata: account(accounts, 13)?,
            root: account(accounts, 14)?,
            ticket_state: account(accounts, 15)?,
            template_raw: account(accounts, 16)?,
            template_staging: account(accounts, 17)?,
            occurrence_raw: account(accounts, 18)?,
            occurrence_staging: account(accounts, 19)?,
            ticket_raw: account(accounts, 20)?,
            ticket_staging: account(accounts, 21)?,
            clock: account(accounts, 22)?,
            rent: account(accounts, 23)?,
            system: account(accounts, 24)?,
        };
        value.validate_privileges()?;
        Ok(value)
    }

    fn validate_privileges(&self) -> Result<(), CoreSbfError> {
        if self.permit.is_signer
            || !self.permit.is_writable
            || self.permit.executable
            || self.rent_credit.is_signer
            || !self.rent_credit.is_writable
            || self.rent_credit.executable
            || self.rent_program.is_signer
            || self.rent_program.is_writable
            || !self.rent_program.executable
            || self.registry_program.is_signer
            || self.registry_program.is_writable
            || !self.registry_program.executable
            || self.trading_program.is_signer
            || self.trading_program.is_writable
            || !self.trading_program.executable
            || self.system.key != &system_program::ID
            || self.system.is_signer
            || self.system.is_writable
            || !self.system.executable
            || self.clock.key != &sysvar::clock::ID
            || self.clock.is_signer
            || self.clock.is_writable
            || self.clock.executable
            || self.rent.key != &sysvar::rent::ID
            || self.rent.is_signer
            || self.rent.is_writable
            || self.rent.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for account in [
            self.infrastructure_profile,
            self.registry_artifact_raw,
            self.registry_artifact_staging,
            self.registry_programdata,
            self.rent_artifact_raw,
            self.rent_artifact_staging,
            self.rent_programdata,
            self.activation_cache,
            self.trading_programdata,
            self.root,
            self.ticket_state,
            self.template_raw,
            self.template_staging,
            self.occurrence_raw,
            self.occurrence_staging,
            self.ticket_raw,
            self.ticket_staging,
        ] {
            if account.is_signer || account.is_writable || account.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(())
    }
}

/// Refund one expired prefunded permit which Core never allocated.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesPermitExpiryRequestV1,
    proof_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = ExpiryAccounts::parse(accounts)?;
    let permit = request.permit();
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let profile = authenticate_profile(
        program_id,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        &rent,
    )?;
    authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.trading_program,
        frame.trading_programdata,
        identity(profile.registry().program().to_bytes())?,
        permit.intent().release_set().to_bytes(),
        Role::Trading,
    )?;
    let refund_owner = authenticate_series(&frame, permit, proof_bytes, &rent)?;
    authenticate_unallocated_permit(program_id, &frame, permit, refund_owner, &rent)?;
    refund(&frame, permit)?;
    Ok(())
}

#[inline(never)]
fn authenticate_series(
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
    proof_bytes: &[u8],
    rent: &Rent,
) -> Result<[u8; 32], CoreSbfError> {
    let intent = permit.intent();
    if intent.trading_program().to_bytes() != frame.trading_program.key.to_bytes()
        || intent.parent_root().to_bytes() != frame.root.key.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let template_bytes = finalized_series_record(
        frame,
        frame.template_raw,
        frame.template_staging,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        rent,
    )?;
    let occurrence_bytes = finalized_series_record(
        frame,
        frame.occurrence_raw,
        frame.occurrence_staging,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
        rent,
    )?;
    let ticket_bytes = finalized_series_record(
        frame,
        frame.ticket_raw,
        frame.ticket_staging,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
        rent,
    )?;
    let occurrence = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, proof_bytes)
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket = admit_ticket(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| CoreSbfError::Reference)?;
    let occurrence_record = occurrence.occurrence();
    let ticket_record = ticket.ticket();
    let retry_through = occurrence
        .template()
        .retry_through(occurrence_record.occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    if intent.release_set().to_bytes() != occurrence.template().release_set().to_bytes()
        || intent.market().to_bytes() != occurrence_record.market().to_bytes()
        || intent.product_record().to_bytes() != occurrence_record.product_record().to_bytes()
        || intent.founder().to_bytes() != ticket_record.founder().to_bytes()
        || intent.ticket_context().to_bytes() != ticket.content_id().to_bytes()
        || intent.generation() != u64::from(occurrence_record.occurrence()) + 1
        || intent.expiry_slot() != retry_through
        || intent.rent_credit().to_bytes() != frame.rent_credit.key.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    authenticate_replay(frame, occurrence, ticket)?;
    let clock = Clock::from_account_info(frame.clock).map_err(|_| CoreSbfError::Creation)?;
    require_expired(clock.slot, retry_through, intent.expiry_slot())?;
    Ok(ticket_record.refund_owner().to_bytes())
}

fn authenticate_replay(
    frame: &ExpiryAccounts<'_, '_>,
    occurrence: dclutch_series_v3_kernel::AdmittedOccurrenceV3,
    ticket: dclutch_series_v3_kernel::AdmittedTicketV3,
) -> Result<(), CoreSbfError> {
    if frame.root.owner != frame.trading_program.key
        || frame.root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || frame.ticket_state.owner != frame.trading_program.key
        || frame.ticket_state.data_len() != SERIES_TICKET_STATE_BYTES_V3
    {
        return Err(CoreSbfError::Reference);
    }
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), frame.trading_program.key).0;
    let series = SeriesStateV3::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Reference)?,
        occurrence.template().occurrence_count(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let settled_occurrence = occurrence
        .occurrence()
        .occurrence()
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    if frame.root.key != &expected_root
        || header.release_set().to_bytes() != occurrence.template().release_set().to_bytes()
        || header.selection().config().to_bytes() != occurrence.template_id().to_bytes()
        || series.next_occurrence() != settled_occurrence
        || series.current_ticket_prepared()
    {
        return Err(CoreSbfError::Reference);
    }
    let ticket_state_data = frame
        .ticket_state
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let state = TicketStateV3::decode(&ticket_state_data).map_err(|_| CoreSbfError::Reference)?;
    let seeds = TicketStateSeedsV3::new(frame.root.key.to_bytes(), ticket.content_id());
    let expected = Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0;
    if frame.ticket_state.key != &expected
        || state.ticket_record_id() != ticket.content_id()
        || state.phase() != TicketPhaseV3::Expired
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

fn finalized_series_record(
    frame: &ExpiryAccounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    rent: &Rent,
) -> Result<alloc::vec::Vec<u8>, CoreSbfError> {
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        frame.registry_program.key,
        raw,
        staging,
        rent,
        schema,
        hash(&bytes).to_bytes(),
        &bytes,
    )?;
    Ok(bytes.to_vec())
}

fn authenticate_unallocated_permit(
    program_id: &Pubkey,
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
    expected_refund_owner: [u8; 32],
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    let seeds = permit.seeds();
    let bump = [intent.bump()];
    let base = seeds.as_slices();
    let expected = Pubkey::create_program_address(
        &[base[0], base[1], base[2], base[3], bump.as_slice()],
        program_id,
    )
    .map_err(|_| CoreSbfError::Creation)?;
    if frame.permit.key != &expected
        || frame.permit.owner != &system_program::ID
        || frame.permit.data_len() != 0
        || frame.permit.lamports() < rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Creation);
    }
    let credit_data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_wallet().to_bytes() != expected_refund_owner
        || credit.market().to_bytes() != intent.market().to_bytes()
        || credit.release_set().to_bytes() != intent.release_set().to_bytes()
        || credit.generation() != intent.generation()
    {
        return Err(CoreSbfError::RentCredit);
    }
    let credit_seeds = credit.pda_seeds();
    let credit_bump = [credit_seeds.bump()];
    let market = credit_seeds.market().to_bytes();
    let generation = credit_seeds.generation();
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            &market,
            &generation,
            credit_bump.as_slice(),
        ],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if frame.rent_credit.owner != frame.rent_program.key
        || frame.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || frame.rent_credit.key != &expected_credit
        || !rent.is_exempt(frame.rent_credit.lamports(), LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

fn refund(
    frame: &ExpiryAccounts<'_, '_>,
    permit: SeriesFoundingPermitV1,
) -> Result<(), CoreSbfError> {
    let seeds = permit.seeds();
    let base = seeds.as_slices();
    let bump = [permit.intent().bump()];
    let signer = [base[0], base[1], base[2], base[3], bump.as_slice()];
    invoke_signed(
        &transfer(
            frame.permit.key,
            frame.rent_credit.key,
            frame.permit.lamports(),
        ),
        &[
            frame.permit.clone(),
            frame.rent_credit.clone(),
            frame.system.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    if frame.permit.lamports() != 0
        || frame.permit.owner != &system_program::ID
        || frame.permit.data_len() != 0
    {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

fn require_expired(
    current_slot: u64,
    retry_through: u64,
    permit_expiry_slot: u64,
) -> Result<(), CoreSbfError> {
    if permit_expiry_slot != retry_through || current_slot <= retry_through {
        Err(CoreSbfError::Reference)
    } else {
        Ok(())
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_supplied_expiry_cannot_replace_the_recomputed_retry_deadline() {
        assert_eq!(require_expired(101, 100, 100), Ok(()));
        assert_eq!(require_expired(101, 100, 99), Err(CoreSbfError::Reference));
        assert_eq!(require_expired(100, 100, 100), Err(CoreSbfError::Reference));
    }
}
