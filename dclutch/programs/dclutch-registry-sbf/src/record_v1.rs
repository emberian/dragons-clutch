//! Permissionless, bounded publication of immutable Registry records.
//!
//! The selected Registry is the sole owner and PDA signer for finalized raw
//! records. Publication principal comes directly from the initiating System
//! wallet. The temporary cursor commits that same wallet as its only refund
//! destination, so finalization does not depend on the retired permanent
//! per-authority RentCredit design.

use core::convert::TryFrom;

use dclutch_record_contract::{
    AccountCloseV1, AccountId, AddressDerivationObligationV1, AppendPageV1, BeginRecordV1,
    CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, FinalizeRecordV1, PageEnvelopeV1,
    RAW_RECORD_PDA_SEED_V1, RawRecordValidationModeV1, RawRecordValidationObligationV1,
    RecordAdapterV1, RecordKeyV1, STAGING_CURSOR_BYTES_V1, STAGING_CURSOR_PDA_SEED_V1,
    StagingCursorV1, StagingLivenessPolicyV1, prepare_append_page_v1, prepare_begin_v1,
    prepare_finalize_v1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::RegistryError;

pub(crate) const BEGIN_ACCOUNT_COUNT_V1: usize = 6;
pub(crate) const APPEND_ACCOUNT_COUNT_V1: usize = 3;
pub(crate) const FINALIZE_ACCOUNT_COUNT_V1: usize = 3;

const _: () = assert!(RAW_RECORD_PDA_SEED_V1.len() <= 32);
const _: () = assert!(STAGING_CURSOR_PDA_SEED_V1.len() <= 32);

pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match instruction_data.get(10).copied() {
        Some(1) => BeginRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_begin(program_id, accounts, request)),
        Some(2) => AppendPageV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_append(program_id, accounts, request)),
        Some(3) => FinalizeRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_finalize(program_id, accounts, request)),
        _ => Err(record_error()),
    }
}

struct BeginFrame<'accounts, 'info> {
    sponsor: &'accounts AccountInfo<'info>,
    raw: &'accounts AccountInfo<'info>,
    cursor: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    clock: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> BeginFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != BEGIN_ACCOUNT_COUNT_V1 {
            return Err(record_error());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            raw: account(accounts, 1)?,
            cursor: account(accounts, 2)?,
            system: account(accounts, 3)?,
            rent: account(accounts, 4)?,
            clock: account(accounts, 5)?,
        };
        require_privilege(frame.sponsor, true, true, false)?;
        require_privilege(frame.raw, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_privilege(frame.system, false, false, true)?;
        require_privilege(frame.rent, false, false, false)?;
        require_privilege(frame.clock, false, false, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct AppendFrame<'accounts, 'info> {
    sponsor: &'accounts AccountInfo<'info>,
    raw: &'accounts AccountInfo<'info>,
    cursor: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> AppendFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != APPEND_ACCOUNT_COUNT_V1 {
            return Err(record_error());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            raw: account(accounts, 1)?,
            cursor: account(accounts, 2)?,
        };
        // A transaction fee payer is writable after message privilege union
        // even when this instruction declares the sponsor read-only. Append
        // authenticates only its signature and cursor-bound System identity;
        // it never mutates the sponsor.
        if !frame.sponsor.is_signer
            || frame.sponsor.executable
            || frame.sponsor.owner != &system_program::ID
            || !frame
                .sponsor
                .try_data_is_empty()
                .map_err(|_| record_error())?
        {
            return Err(record_error());
        }
        require_privilege(frame.raw, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct FinalizeFrame<'accounts, 'info> {
    raw: &'accounts AccountInfo<'info>,
    cursor: &'accounts AccountInfo<'info>,
    refund_wallet: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FinalizeFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != FINALIZE_ACCOUNT_COUNT_V1 {
            return Err(record_error());
        }
        let frame = Self {
            raw: account(accounts, 0)?,
            cursor: account(accounts, 1)?,
            refund_wallet: account(accounts, 2)?,
        };
        require_privilege(frame.raw, false, false, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        // The committed refund wallet may also pay this transaction and is
        // therefore a signer after message privilege union. Signature presence
        // cannot change the cursor-owned refund identity.
        if !frame.refund_wallet.is_writable
            || frame.refund_wallet.executable
            || frame.refund_wallet.owner != &system_program::ID
            || !frame
                .refund_wallet
                .try_data_is_empty()
                .map_err(|_| record_error())?
        {
            return Err(record_error());
        }
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct BeginPlan {
    cursor: StagingCursorV1,
    raw_bump: u8,
    cursor_bump: u8,
    raw_rent: u64,
    cursor_balance: u64,
    raw_before: u64,
    cursor_before: u64,
    raw_top_up: u64,
    cursor_top_up: u64,
    sponsor_before: u64,
}

#[inline(never)]
fn process_begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: BeginRecordV1,
) -> Result<(), ProgramError> {
    let frame = BeginFrame::parse(accounts)?;
    let plan = authenticate_begin(program_id, &frame, request)?;
    let key = plan.cursor.key();
    let schema = key.schema_release_id().to_bytes();
    let digest = key.expected_digest().to_bytes();
    let raw_bump = [plan.raw_bump];
    let raw_signer = [
        RAW_RECORD_PDA_SEED_V1,
        schema.as_slice(),
        digest.as_slice(),
        raw_bump.as_slice(),
    ];
    create_or_allocate_prefunded_pda(
        frame.sponsor,
        frame.raw,
        frame.system,
        plan.raw_rent,
        plan.cursor.exact_length(),
        program_id,
        &raw_signer,
    )?;
    let sponsor_after_raw = plan
        .sponsor_before
        .checked_sub(plan.raw_top_up)
        .ok_or_else(record_error)?;
    if frame.sponsor.lamports() != sponsor_after_raw
        || frame.raw.lamports()
            != plan
                .raw_before
                .checked_add(plan.raw_top_up)
                .ok_or_else(record_error)?
        || frame.raw.lamports() < plan.raw_rent
        || frame.raw.owner != program_id
        || u64::try_from(frame.raw.data_len()).map_err(|_| record_error())?
            != plan.cursor.exact_length()
    {
        return Err(record_error());
    }

    let cursor_space = u64::try_from(STAGING_CURSOR_BYTES_V1).map_err(|_| record_error())?;
    let cursor_bump = [plan.cursor_bump];
    let cursor_signer = [
        STAGING_CURSOR_PDA_SEED_V1,
        schema.as_slice(),
        digest.as_slice(),
        cursor_bump.as_slice(),
    ];
    create_or_allocate_prefunded_pda(
        frame.sponsor,
        frame.cursor,
        frame.system,
        plan.cursor_balance,
        cursor_space,
        program_id,
        &cursor_signer,
    )?;
    let sponsor_after = sponsor_after_raw
        .checked_sub(plan.cursor_top_up)
        .ok_or_else(record_error)?;
    if frame.sponsor.lamports() != sponsor_after
        || frame.cursor.lamports()
            != plan
                .cursor_before
                .checked_add(plan.cursor_top_up)
                .ok_or_else(record_error)?
        || frame.cursor.lamports() < plan.cursor_balance
        || frame.cursor.owner != program_id
        || frame.cursor.data_len() != STAGING_CURSOR_BYTES_V1
    {
        return Err(record_error());
    }
    {
        let mut data = frame
            .cursor
            .try_borrow_mut_data()
            .map_err(|_| record_error())?;
        data.copy_from_slice(&plan.cursor.to_bytes());
    }
    if decode_cursor(frame.cursor)? != plan.cursor {
        return Err(record_error());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_begin(
    program_id: &Pubkey,
    frame: &BeginFrame<'_, '_>,
    request: BeginRecordV1,
) -> Result<BeginPlan, ProgramError> {
    require_system_identity(frame.system)?;
    require_rent_identity(frame.rent)?;
    require_clock_identity(frame.clock)?;
    require_system_wallet(frame.sponsor, true)?;
    require_prefunded_vacant(frame.raw)?;
    require_prefunded_vacant(frame.cursor)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| record_error())?;
    let clock = Clock::from_account_info(frame.clock).map_err(|_| record_error())?;
    let raw_length = usize::try_from(request.exact_length()).map_err(|_| record_error())?;
    let raw_rent = rent.minimum_balance(raw_length);
    let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);
    let cursor_balance = cursor_rent
        .checked_add(request.cleanup_bounty_lamports())
        .ok_or_else(record_error)?;
    let raw_top_up = raw_rent.saturating_sub(frame.raw.lamports());
    let cursor_top_up = cursor_balance.saturating_sub(frame.cursor.lamports());
    let total_debit = raw_top_up
        .checked_add(cursor_top_up)
        .ok_or_else(record_error)?;
    if frame.sponsor.lamports() < total_debit {
        return Err(record_error());
    }
    let adapter = SbfRecordAdapter::begin(program_id, frame.raw, frame.cursor, cursor_rent);
    let liveness = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1
        .staging_liveness_policy(cursor_rent)
        .map_err(map_record_error)?;
    let transition = prepare_begin_v1(
        &adapter,
        request,
        liveness,
        clock.slot,
        account_id(frame.raw.key)?,
        account_id(frame.cursor.key)?,
        account_id(frame.sponsor.key)?,
    )
    .map_err(map_record_error)?;
    let allocation = transition.allocation();
    if allocation.raw_record_account() != account_id(frame.raw.key)?
        || allocation.raw_data_length() != request.exact_length()
        || allocation.staging_account() != account_id(frame.cursor.key)?
        || allocation.staging_data_length()
            != u64::try_from(STAGING_CURSOR_BYTES_V1).map_err(|_| record_error())?
        || allocation.sponsor_rent_refund() != account_id(frame.sponsor.key)?
        || allocation.cleanup_bounty_lamports() != request.cleanup_bounty_lamports()
    {
        return Err(record_error());
    }
    let (expected_raw, raw_bump) = derive_record_pda(program_id, request.key(), false);
    let (expected_cursor, cursor_bump) = derive_record_pda(program_id, request.key(), true);
    if frame.raw.key != &expected_raw || frame.cursor.key != &expected_cursor {
        return Err(record_error());
    }
    preflight_lamports(frame.sponsor)?;
    preflight_mutable(frame.raw)?;
    preflight_mutable(frame.cursor)?;
    Ok(BeginPlan {
        cursor: transition.cursor(),
        raw_bump,
        cursor_bump,
        raw_rent,
        cursor_balance,
        raw_before: frame.raw.lamports(),
        cursor_before: frame.cursor.lamports(),
        raw_top_up,
        cursor_top_up,
        sponsor_before: frame.sponsor.lamports(),
    })
}

#[inline(never)]
fn process_append(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: AppendPageV1<'_>,
) -> Result<(), ProgramError> {
    let frame = AppendFrame::parse(accounts)?;
    require_live_record_accounts(program_id, frame.raw, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw, frame.cursor)?;
    if cursor.sponsor_rent_refund() != account_id(frame.sponsor.key)?
        || u64::try_from(frame.raw.data_len()).map_err(|_| record_error())? != cursor.exact_length()
    {
        return Err(record_error());
    }
    let transition = prepare_append_page_v1(
        cursor,
        account_id(frame.raw.key)?,
        account_id(frame.cursor.key)?,
        cursor.exact_length(),
        request,
    )
    .map_err(map_record_error)?;
    preflight_data(frame.raw)?;
    preflight_data(frame.cursor)?;
    let start = usize::try_from(transition.write().offset()).map_err(|_| record_error())?;
    let end = start
        .checked_add(transition.write().page().len())
        .ok_or_else(record_error)?;
    let next = transition.next_cursor().to_bytes();
    {
        let mut raw = frame
            .raw
            .try_borrow_mut_data()
            .map_err(|_| record_error())?;
        let mut cursor_data = frame
            .cursor
            .try_borrow_mut_data()
            .map_err(|_| record_error())?;
        raw.get_mut(start..end)
            .ok_or_else(record_error)?
            .copy_from_slice(transition.write().page());
        cursor_data.copy_from_slice(&next);
    }
    if decode_cursor(frame.cursor)? != transition.next_cursor() {
        return Err(record_error());
    }
    Ok(())
}

#[inline(never)]
fn process_finalize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _request: FinalizeRecordV1,
) -> Result<(), ProgramError> {
    let frame = FinalizeFrame::parse(accounts)?;
    require_live_record_accounts(program_id, frame.raw, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw, frame.cursor)?;
    if cursor.sponsor_rent_refund() != account_id(frame.refund_wallet.key)?
        || u64::try_from(frame.raw.data_len()).map_err(|_| record_error())? != cursor.exact_length()
    {
        return Err(record_error());
    }
    let cursor_balance = frame.cursor.lamports();
    let wallet_before = frame.refund_wallet.lamports();
    let close = {
        let raw_data = frame.raw.try_borrow_data().map_err(|_| record_error())?;
        let adapter = SbfRecordAdapter::finalize(program_id, frame.raw, frame.cursor);
        let transition = prepare_finalize_v1(
            &adapter,
            cursor,
            account_id(frame.raw.key)?,
            account_id(frame.cursor.key)?,
            cursor_balance,
            &raw_data,
        )
        .map_err(map_record_error)?;
        if transition.authenticated_record().key() != cursor.key()
            || transition.authenticated_record().raw_record_account() != account_id(frame.raw.key)?
        {
            return Err(record_error());
        }
        transition.staging_close()
    };
    preflight_lamports(frame.refund_wallet)?;
    preflight_mutable(frame.cursor)?;
    close_full_to_wallet(program_id, frame.cursor, frame.refund_wallet, close)?;
    if frame.refund_wallet.lamports()
        != wallet_before
            .checked_add(cursor_balance)
            .ok_or_else(record_error)?
        || !is_vacant(frame.cursor)
        || frame.raw.owner != program_id
        || hash(&frame.raw.try_borrow_data().map_err(|_| record_error())?).to_bytes()
            != cursor.key().expected_digest().to_bytes()
    {
        return Err(record_error());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdapterLifecycle {
    Begin,
    Finalize,
}

struct SbfRecordAdapter<'accounts, 'info> {
    program_id: &'accounts Pubkey,
    raw: &'accounts AccountInfo<'info>,
    cursor: &'accounts AccountInfo<'info>,
    cursor_rent: u64,
    lifecycle: AdapterLifecycle,
}

impl<'accounts, 'info> SbfRecordAdapter<'accounts, 'info> {
    fn begin(
        program_id: &'accounts Pubkey,
        raw: &'accounts AccountInfo<'info>,
        cursor: &'accounts AccountInfo<'info>,
        cursor_rent: u64,
    ) -> Self {
        Self {
            program_id,
            raw,
            cursor,
            cursor_rent,
            lifecycle: AdapterLifecycle::Begin,
        }
    }

    fn finalize(
        program_id: &'accounts Pubkey,
        raw: &'accounts AccountInfo<'info>,
        cursor: &'accounts AccountInfo<'info>,
    ) -> Self {
        Self {
            program_id,
            raw,
            cursor,
            cursor_rent: 0,
            lifecycle: AdapterLifecycle::Finalize,
        }
    }
}

impl RecordAdapterV1 for SbfRecordAdapter<'_, '_> {
    fn validate_page_envelope(&self, envelope: &PageEnvelopeV1) -> bool {
        CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.validates_page_envelope(*envelope)
    }

    fn validate_staging_liveness_policy(&self, policy: &StagingLivenessPolicyV1) -> bool {
        self.lifecycle == AdapterLifecycle::Begin
            && self.cursor_rent > 0
            && CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1
                .validates_staging_liveness_policy(*policy, self.cursor_rent)
    }

    fn validate_canonical_addresses(&self, obligation: &AddressDerivationObligationV1) -> bool {
        let (raw, _) = derive_record_pda(self.program_id, obligation.key(), false);
        let (cursor, _) = derive_record_pda(self.program_id, obligation.key(), true);
        obligation.raw_record_account().to_bytes() == raw.to_bytes()
            && obligation.staging_account().to_bytes() == cursor.to_bytes()
            && self.raw.key == &raw
            && self.cursor.key == &cursor
    }

    fn validate_raw_record(&self, obligation: &RawRecordValidationObligationV1<'_>) -> bool {
        self.lifecycle == AdapterLifecycle::Finalize
            && obligation.mode() == RawRecordValidationModeV1::Finalization
            && obligation.raw_record_account().to_bytes() == self.raw.key.to_bytes()
            && obligation.staging_account().to_bytes() == self.cursor.key.to_bytes()
            && self.raw.owner == self.program_id
            && !self.raw.executable
            && !self.raw.is_writable
            && self.raw.data_len() == obligation.exact_content().len()
            && hash(obligation.exact_content()).to_bytes()
                == obligation.key().expected_digest().to_bytes()
            && self
                .raw
                .try_borrow_data()
                .map(|data| data.as_ref() == obligation.exact_content())
                .unwrap_or(false)
            && self.cursor.owner == self.program_id
            && self.cursor.is_writable
            && !self.cursor.executable
            && self.cursor.data_len() == STAGING_CURSOR_BYTES_V1
            && self.cursor.lamports() > 0
    }
}

fn derive_record_pda(program_id: &Pubkey, key: RecordKeyV1, staging: bool) -> (Pubkey, u8) {
    let domain = if staging {
        STAGING_CURSOR_PDA_SEED_V1
    } else {
        RAW_RECORD_PDA_SEED_V1
    };
    Pubkey::find_program_address(
        &[
            domain,
            key.schema_release_id().as_bytes(),
            key.expected_digest().as_bytes(),
        ],
        program_id,
    )
}

fn require_canonical_record_addresses(
    program_id: &Pubkey,
    cursor: StagingCursorV1,
    raw: &AccountInfo<'_>,
    cursor_account: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let (expected_raw, _) = derive_record_pda(program_id, cursor.key(), false);
    let (expected_cursor, _) = derive_record_pda(program_id, cursor.key(), true);
    if raw.key != &expected_raw
        || cursor_account.key != &expected_cursor
        || cursor.raw_record_account() != account_id(raw.key)?
        || cursor.staging_account() != account_id(cursor_account.key)?
    {
        return Err(record_error());
    }
    Ok(())
}

fn decode_cursor(account: &AccountInfo<'_>) -> Result<StagingCursorV1, ProgramError> {
    let data = account.try_borrow_data().map_err(|_| record_error())?;
    StagingCursorV1::decode(&data).map_err(map_record_error)
}

fn require_live_record_accounts(
    program_id: &Pubkey,
    raw: &AccountInfo<'_>,
    cursor: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if raw.owner != program_id
        || cursor.owner != program_id
        || raw.executable
        || cursor.executable
        || cursor.data_len() != STAGING_CURSOR_BYTES_V1
        || cursor.lamports() == 0
    {
        return Err(record_error());
    }
    Ok(())
}

fn close_full_to_wallet(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    wallet: &AccountInfo<'_>,
    plan: AccountCloseV1,
) -> Result<(), ProgramError> {
    if source.owner != program_id
        || wallet.owner != &system_program::ID
        || wallet.executable
        || !wallet.try_data_is_empty().map_err(|_| record_error())?
        || plan.account().to_bytes() != source.key.to_bytes()
        || plan.full_lamport_refund().to_bytes() != wallet.key.to_bytes()
        || plan.observed_lamports() != source.lamports()
    {
        return Err(record_error());
    }
    let wallet_after = wallet
        .lamports()
        .checked_add(source.lamports())
        .ok_or_else(record_error)?;
    {
        let mut wallet_lamports = wallet
            .try_borrow_mut_lamports()
            .map_err(|_| record_error())?;
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| record_error())?;
        **wallet_lamports = wallet_after;
        **source_lamports = 0;
    }
    source.resize(0).map_err(|_| record_error())?;
    source.assign(&system_program::ID);
    if wallet.lamports() != wallet_after || !is_vacant(source) {
        return Err(record_error());
    }
    Ok(())
}

fn create_or_allocate_prefunded_pda<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    minimum_balance: u64,
    space: u64,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let before = created.lamports();
    if !is_prefunded_vacant(created) {
        return Err(record_error());
    }
    let top_up = minimum_balance.saturating_sub(before);
    if before == 0 {
        invoke_signed(
            &create_account(payer.key, created.key, minimum_balance, space, owner),
            &[payer.clone(), created.clone(), system.clone()],
            &[signer_seeds],
        )?;
    } else {
        if top_up != 0 {
            invoke(
                &transfer(payer.key, created.key, top_up),
                &[payer.clone(), created.clone(), system.clone()],
            )?;
        }
        invoke_signed(
            &allocate(created.key, space),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )?;
        invoke_signed(
            &assign(created.key, owner),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )?;
    }
    let expected = before.checked_add(top_up).ok_or_else(record_error)?;
    if created.owner != owner
        || created.executable
        || created.data_len() != usize::try_from(space).map_err(|_| record_error())?
        || created.lamports() != expected
        || created.lamports() < minimum_balance
    {
        return Err(record_error());
    }
    Ok(())
}

fn require_privilege(
    account: &AccountInfo<'_>,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<(), ProgramError> {
    if account.is_signer != signer
        || account.is_writable != writable
        || account.executable != executable
    {
        return Err(record_error());
    }
    Ok(())
}

fn require_system_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &system_program::ID
        || account.owner != &native_loader::ID
        || !account.executable
    {
        return Err(record_error());
    }
    Ok(())
}

fn require_rent_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID || account.executable {
        return Err(record_error());
    }
    Ok(())
}

fn require_clock_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID || account.executable {
        return Err(record_error());
    }
    Ok(())
}

fn require_system_wallet(account: &AccountInfo<'_>, signer: bool) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID
        || account.executable
        || account.is_signer != signer
        || !account.is_writable
        || !account.try_data_is_empty().map_err(|_| record_error())?
    {
        return Err(record_error());
    }
    Ok(())
}

fn require_prefunded_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !is_prefunded_vacant(account) {
        return Err(record_error());
    }
    Ok(())
}

fn is_vacant(account: &AccountInfo<'_>) -> bool {
    is_prefunded_vacant(account) && account.lamports() == 0
}

fn is_prefunded_vacant(account: &AccountInfo<'_>) -> bool {
    account.owner == &system_program::ID
        && !account.executable
        && account.try_data_is_empty().unwrap_or(false)
}

fn preflight_lamports(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| record_error())?,
    );
    Ok(())
}

fn preflight_data(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(account.try_borrow_mut_data().map_err(|_| record_error())?);
    Ok(())
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    preflight_lamports(account)?;
    preflight_data(account)
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(record_error());
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or_else(record_error)
}

fn account_id(key: &Pubkey) -> Result<AccountId, ProgramError> {
    AccountId::new(key.to_bytes()).map_err(map_record_error)
}

fn map_record_error(_: dclutch_record_contract::Error) -> ProgramError {
    record_error()
}

const fn record_error() -> ProgramError {
    ProgramError::Custom(RegistryError::Record as u32)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_record_contract::{
        AppendPageV1, BeginRecordV1, ContentDigest, FinalizeRecordV1, RecordKeyV1, SchemaReleaseId,
    };
    use solana_program::{clock::Clock, hash::hash};

    use super::*;

    fn account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn rent_data() -> Vec<u8> {
        let rent = Rent::default();
        let mut lamports = 1;
        let mut data = vec![0; Rent::size_of()];
        let key = sysvar::rent::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        rent.to_account_info(&mut info).expect("serialize Rent");
        data
    }

    fn clock_data(slot: u64) -> Vec<u8> {
        let clock = Clock {
            slot,
            ..Clock::default()
        };
        let mut lamports = 1;
        let mut data = vec![0; Clock::size_of()];
        let key = sysvar::clock::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        clock.to_account_info(&mut info).expect("serialize Clock");
        data
    }

    fn begin_fixture(
        registry: Pubkey,
        sponsor: Pubkey,
        content: &[u8],
    ) -> (BeginRecordV1, Vec<AccountInfo<'static>>) {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new([0x41; 32]).expect("schema"),
            ContentDigest::new(hash(content).to_bytes()).expect("digest"),
        );
        let (raw, _) = derive_record_pda(&registry, key, false);
        let (cursor, _) = derive_record_pda(&registry, key, true);
        let rent = Rent::default();
        let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
        let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);
        let request = BeginRecordV1::new(
            key,
            u64::try_from(content.len()).expect("content width"),
            profile.page_envelope().expect("page envelope"),
            profile
                .staging_liveness_policy(cursor_rent)
                .expect("liveness")
                .policy_id(),
            101,
            cursor_rent,
        )
        .expect("Begin");
        let accounts = vec![
            account(
                sponsor,
                true,
                true,
                10_000_000_000,
                Vec::new(),
                system_program::ID,
                false,
            ),
            account(raw, false, true, 0, Vec::new(), system_program::ID, false),
            account(
                cursor,
                false,
                true,
                0,
                Vec::new(),
                system_program::ID,
                false,
            ),
            account(
                system_program::ID,
                false,
                false,
                1,
                Vec::new(),
                native_loader::ID,
                true,
            ),
            account(
                sysvar::rent::ID,
                false,
                false,
                1,
                rent_data(),
                sysvar::ID,
                false,
            ),
            account(
                sysvar::clock::ID,
                false,
                false,
                1,
                clock_data(1),
                sysvar::ID,
                false,
            ),
        ];
        (request, accounts)
    }

    #[test]
    fn begin_authenticates_canonical_profile_and_refund_wallet() {
        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content = b"canonical immutable record";
        let (request, accounts) = begin_fixture(registry, sponsor, content);
        let frame = BeginFrame::parse(&accounts).expect("Begin frame");
        let plan = authenticate_begin(&registry, &frame, request).expect("Begin authority");
        assert_eq!(
            plan.cursor.sponsor_rent_refund().to_bytes(),
            sponsor.to_bytes()
        );
        assert_eq!(plan.cursor.key(), request.key());

        let hostile_raw = Pubkey::new_unique();
        let (_, mut hostile) = begin_fixture(registry, sponsor, content);
        *hostile.get_mut(1).expect("hostile raw role") = account(
            hostile_raw,
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let frame = BeginFrame::parse(&hostile).expect("hostile frame shape");
        assert_eq!(
            authenticate_begin(&registry, &frame, request).err(),
            Some(record_error())
        );
    }

    #[test]
    fn append_is_exactly_ordered_and_finalize_refunds_only_committed_wallet() {
        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content = b"one exact page";
        let (request, begin_accounts) = begin_fixture(registry, sponsor, content);
        let frame = BeginFrame::parse(&begin_accounts).expect("Begin frame");
        let cursor = authenticate_begin(&registry, &frame, request)
            .expect("Begin plan")
            .cursor;
        let raw_key = *begin_accounts.get(1).expect("raw role").key;
        let cursor_key = *begin_accounts.get(2).expect("cursor role").key;
        let cursor_lamports = Rent::default()
            .minimum_balance(STAGING_CURSOR_BYTES_V1)
            .checked_mul(2)
            .expect("cursor balance");
        let raw = account(
            raw_key,
            false,
            true,
            Rent::default().minimum_balance(content.len()),
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_lamports,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let signer = account(
            sponsor,
            true,
            false,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        let append = AppendPageV1::new(0, 0, content).expect("Append");
        process_append(
            &registry,
            &[signer.clone(), raw.clone(), stage.clone()],
            append,
        )
        .expect("append exact page");
        assert_eq!(raw.try_borrow_data().expect("raw bytes").as_ref(), content);
        let complete_cursor = decode_cursor(&stage).expect("complete cursor");
        assert!(complete_cursor.is_complete());
        let replay = AppendPageV1::new(0, 0, content).expect("replay wire");
        assert_eq!(
            process_append(&registry, &[signer, raw.clone(), stage], replay),
            Err(record_error())
        );
        let raw = account(
            raw_key,
            false,
            false,
            Rent::default().minimum_balance(content.len()),
            content.to_vec(),
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_lamports,
            complete_cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let refund_before = 77;
        let refund = account(
            sponsor,
            true,
            true,
            refund_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        process_finalize(
            &registry,
            &[raw, stage.clone(), refund.clone()],
            FinalizeRecordV1,
        )
        .expect("finalize");
        assert_eq!(refund.lamports(), refund_before + cursor_lamports);
        assert!(is_vacant(&stage));

        let hostile = account(
            Pubkey::new_unique(),
            false,
            true,
            99,
            Vec::new(),
            system_program::ID,
            false,
        );
        let raw = account(
            raw_key,
            false,
            false,
            Rent::default().minimum_balance(content.len()),
            content.to_vec(),
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_lamports,
            complete_cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        assert_eq!(
            process_finalize(&registry, &[raw, stage, hostile], FinalizeRecordV1),
            Err(record_error())
        );
    }
}
