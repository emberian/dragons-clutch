//! Permissionless, bounded publication of immutable Registry records.
//!
//! The selected Registry is the sole owner and PDA signer for finalized raw
//! records. Publication principal comes directly from the initiating System
//! wallet. The temporary cursor commits that same wallet as its refund
//! destination for `Finalize` and for an early sponsor `Abort`, so finalization
//! does not depend on the retired permanent per-authority RentCredit design.
//!
//! Bounded, and now provably so: `Begin` prepays a nonzero cleanup bounty into
//! the cursor, and `Abort` (action 4) is the funded, permissionless reclamation
//! that spends it. Before the cursor's `expiry_slot` only the committed sponsor
//! may abort their own in-progress publication (the bounty is withheld). At or
//! after expiry anyone may abort an abandoned record set, is paid the disclosed
//! bounty, and returns the remaining rent to the sponsor — so a half-published
//! record can never strand its accounts or its prepaid bounty. The verb lives
//! in `dclutch-registry::prepare_abort_v1`; this module drives it.

use core::convert::TryFrom;

use dclutch_registry::record::{
    AbortObservationV1, AbortRecordV1, AccountCloseV1, AccountId, AddressDerivationObligationV1,
    AppendPageV1, BeginRecordV1, CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, FinalizeRecordV1,
    PageEnvelopeV1, RAW_RECORD_PDA_SEED_V1, RawRecordValidationModeV1,
    RawRecordValidationObligationV1, RecordAdapterV1, RecordKeyV1, STAGING_CURSOR_BYTES_V1,
    STAGING_CURSOR_PDA_SEED_V1, StagingCursorV1, StagingLivenessPolicyV1, prepare_abort_v1,
    prepare_append_page_v1, prepare_begin_v1, prepare_finalize_v1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, DeploymentObservationV1};
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
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::RegistryError;

pub(crate) const BEGIN_ACCOUNT_COUNT_V1: usize = 6;
pub(crate) const APPEND_ACCOUNT_COUNT_V1: usize = 3;
pub(crate) const FINALIZE_ACCOUNT_COUNT_V1: usize = 3;
/// Finalizing an `ArtifactRelease` also carries its Program and ProgramData.
pub(crate) const FINALIZE_DEPLOYMENT_ACCOUNT_COUNT_V1: usize = FINALIZE_ACCOUNT_COUNT_V1 + 2;
pub(crate) const ABORT_ACCOUNT_COUNT_V1: usize = 5;

const _: () = assert!(RAW_RECORD_PDA_SEED_V1.len() <= 32);
const _: () = assert!(STAGING_CURSOR_PDA_SEED_V1.len() <= 32);

pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match instruction_data.get(10).copied() {
        // `5`, not `1`: `Begin` moved out of the Registry half of this shared
        // discriminant on 2026-09-01. See the partition assertions in `lib.rs`.
        Some(5) => BeginRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_begin(program_id, accounts, request)),
        Some(2) => AppendPageV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_append(program_id, accounts, request)),
        Some(3) => FinalizeRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_finalize(program_id, accounts, request)),
        Some(4) => AbortRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|request| process_abort(program_id, accounts, request)),
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
    /// The live deployment an `ArtifactRelease` finalization observes.
    ///
    /// Present exactly when the record being finalized is an `ArtifactRelease`,
    /// which `process_finalize` decides from the cursor's own schema rather
    /// than from the frame's width -- so a caller cannot buy the cheap shape by
    /// omitting accounts, and cannot attach a deployment to a record that has
    /// no address to observe.
    deployment: Option<(&'accounts AccountInfo<'info>, &'accounts AccountInfo<'info>)>,
}

impl<'accounts, 'info> FinalizeFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        let deployment = match accounts.len() {
            FINALIZE_ACCOUNT_COUNT_V1 => None,
            FINALIZE_DEPLOYMENT_ACCOUNT_COUNT_V1 => {
                let program = account(accounts, 3)?;
                let programdata = account(accounts, 4)?;
                // Read-only and non-signing, like every other observation in
                // this program: finalization looks at a deployment, it never
                // acts on one.
                require_privilege(program, false, false, true)?;
                require_privilege(programdata, false, false, false)?;
                Some((program, programdata))
            }
            _ => return Err(record_error()),
        };
        let frame = Self {
            raw: account(accounts, 0)?,
            cursor: account(accounts, 1)?,
            refund_wallet: account(accounts, 2)?,
            deployment,
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

struct AbortFrame<'accounts, 'info> {
    raw: &'accounts AccountInfo<'info>,
    cursor: &'accounts AccountInfo<'info>,
    sponsor_wallet: &'accounts AccountInfo<'info>,
    abort_actor: &'accounts AccountInfo<'info>,
    clock: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> AbortFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != ABORT_ACCOUNT_COUNT_V1 {
            return Err(record_error());
        }
        let frame = Self {
            raw: account(accounts, 0)?,
            cursor: account(accounts, 1)?,
            sponsor_wallet: account(accounts, 2)?,
            abort_actor: account(accounts, 3)?,
            clock: account(accounts, 4)?,
        };
        require_privilege(frame.raw, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        // Both wallets may also pay this transaction and so be signers after
        // message privilege union; the sponsor identity is cursor-owned and the
        // actor's signature is authenticated in `process_abort` only when the
        // contract demands it (an early, pre-expiry sponsor Abort).
        require_writable_system_wallet(frame.sponsor_wallet)?;
        require_writable_system_wallet(frame.abort_actor)?;
        require_clock_identity(frame.clock)?;
        require_privilege(frame.clock, false, false, false)?;
        // The sponsor wallet and the abort actor are the SAME account on an early
        // sponsor Abort (the contract requires `abort_actor == sponsor` before
        // expiry), so they are deliberately permitted to alias; every other pair
        // must be distinct.
        require_distinct(&[
            frame.raw.clone(),
            frame.cursor.clone(),
            frame.sponsor_wallet.clone(),
            frame.clock.clone(),
        ])?;
        require_distinct(&[
            frame.raw.clone(),
            frame.cursor.clone(),
            frame.abort_actor.clone(),
            frame.clock.clone(),
        ])?;
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
    // DECISION 0012'S PRECONDITION IS ESTABLISHED HERE, AND ONLY HERE.
    //
    // Before the cursor is closed, because a release whose deployment does not
    // check must stay unfinalized: a finalized record is permanent, and this is
    // the one moment the protocol can still say no.
    observe_artifact_release_deployment_v1(cursor, &frame)?;
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

/// Permissionless, funded reclamation of an abandoned in-progress record.
///
/// This wires the record contract's `prepare_abort_v1` verb, which had a
/// complete, tested implementation and no on-chain dispatcher: every `Begin`
/// prepays a nonzero cleanup bounty into the cursor
/// (`authenticate_begin` charges `cleanup_bounty_lamports`), and this is the
/// only route in the tree that pays a lamport bounty to its own caller. Two
/// shapes, both decided inside the contract:
///
/// - **Early** (`current_slot < cursor.expiry_slot`): only the committed sponsor
///   may cancel their own publication. The contract returns
///   `sponsor_signature_required`, the bounty is withheld (it exists to fund a
///   *stranger's* later cleanup, not the sponsor's own abort), and every lamport
///   returns to the sponsor. Here the actor and the sponsor wallet are the same
///   account.
/// - **Expired** (`current_slot >= cursor.expiry_slot`): permissionless. Anyone
///   drives it, the disclosed bounty is paid to the actor, and the remaining
///   rent returns to the committed sponsor. This is the liveness guarantee — an
///   abandoned record set can always be reclaimed and never strands its rent or
///   its prepaid bounty.
#[inline(never)]
fn process_abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _request: AbortRecordV1,
) -> Result<(), ProgramError> {
    let frame = AbortFrame::parse(accounts)?;
    require_live_record_accounts(program_id, frame.raw, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw, frame.cursor)?;
    let raw_length = u64::try_from(frame.raw.data_len()).map_err(|_| record_error())?;
    // The cursor is the sole author of the sponsor refund identity; bind the
    // frame's sponsor wallet to it before touching any lamports. Both are also
    // re-checked inside `prepare_abort_v1` via the returned obligations.
    if cursor.sponsor_rent_refund() != account_id(frame.sponsor_wallet.key)?
        || raw_length != cursor.exact_length()
    {
        return Err(record_error());
    }
    let clock = Clock::from_account_info(frame.clock).map_err(|_| record_error())?;
    let raw_lamports = frame.raw.lamports();
    let cursor_lamports = frame.cursor.lamports();
    let observation = AbortObservationV1::new(
        account_id(frame.raw.key)?,
        account_id(frame.cursor.key)?,
        raw_length,
        raw_lamports,
        cursor_lamports,
        clock.slot,
        account_id(frame.abort_actor.key)?,
    );
    let transition = prepare_abort_v1(cursor, observation).map_err(map_record_error)?;
    // Early Abort is the sponsor cancelling in-progress work: the contract only
    // reaches `sponsor_signature_required` when the actor is the committed
    // sponsor, and the SVM boundary must still see the signature. Expired
    // cleanup is permissionless — no signer identity is required of the actor.
    if transition.sponsor_signature_required() && !frame.abort_actor.is_signer {
        return Err(record_error());
    }
    let raw_close = transition.raw_record_close();
    let staging = transition.staging_close();
    // Bind every contract-computed obligation to the exact SVM accounts.
    if raw_close.account().to_bytes() != frame.raw.key.to_bytes()
        || raw_close.full_lamport_refund().to_bytes() != frame.sponsor_wallet.key.to_bytes()
        || raw_close.observed_lamports() != raw_lamports
        || staging.account().to_bytes() != frame.cursor.key.to_bytes()
        || staging.sponsor_recipient().to_bytes() != frame.sponsor_wallet.key.to_bytes()
        || staging.cleanup_recipient().to_bytes() != frame.abort_actor.key.to_bytes()
        || staging.observed_lamports() != cursor_lamports
    {
        return Err(record_error());
    }
    let aliased = frame.sponsor_wallet.key == frame.abort_actor.key;
    let sponsor_before = frame.sponsor_wallet.lamports();
    let actor_before = frame.abort_actor.lamports();
    // Sponsor receives the full raw rent plus the cursor's sponsor split; the
    // actor receives the bounty (zero on an early sponsor Abort).
    let sponsor_credit = raw_lamports
        .checked_add(staging.sponsor_refund_lamports())
        .ok_or_else(record_error)?;
    let actor_credit = staging.cleanup_bounty_lamports();

    preflight_mutable(frame.raw)?;
    preflight_mutable(frame.cursor)?;
    preflight_lamports(frame.sponsor_wallet)?;
    preflight_lamports(frame.abort_actor)?;

    // Drain both program-owned PDAs to zero and return them to the System owner.
    close_pda_to_zero(program_id, frame.raw)?;
    close_pda_to_zero(program_id, frame.cursor)?;

    if aliased {
        // One account holds both roles (the early sponsor Abort, or an expired
        // cleanup a sponsor runs for themselves): credit it the whole balance.
        let total = sponsor_credit
            .checked_add(actor_credit)
            .ok_or_else(record_error)?;
        let after = sponsor_before.checked_add(total).ok_or_else(record_error)?;
        let mut lamports = frame
            .sponsor_wallet
            .try_borrow_mut_lamports()
            .map_err(|_| record_error())?;
        **lamports = after;
    } else {
        let sponsor_after = sponsor_before
            .checked_add(sponsor_credit)
            .ok_or_else(record_error)?;
        let actor_after = actor_before
            .checked_add(actor_credit)
            .ok_or_else(record_error)?;
        {
            let mut lamports = frame
                .sponsor_wallet
                .try_borrow_mut_lamports()
                .map_err(|_| record_error())?;
            **lamports = sponsor_after;
        }
        {
            let mut lamports = frame
                .abort_actor
                .try_borrow_mut_lamports()
                .map_err(|_| record_error())?;
            **lamports = actor_after;
        }
    }

    // Postcheck exact conservation, both PDAs vacant, and the bounty landed.
    let (expect_sponsor, expect_actor) = if aliased {
        let total = sponsor_before
            .checked_add(sponsor_credit)
            .and_then(|value| value.checked_add(actor_credit))
            .ok_or_else(record_error)?;
        (total, total)
    } else {
        (
            sponsor_before
                .checked_add(sponsor_credit)
                .ok_or_else(record_error)?,
            actor_before
                .checked_add(actor_credit)
                .ok_or_else(record_error)?,
        )
    };
    if frame.sponsor_wallet.lamports() != expect_sponsor
        || frame.abort_actor.lamports() != expect_actor
        || !is_vacant(frame.raw)
        || !is_vacant(frame.cursor)
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

fn require_writable_system_wallet(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !account.is_writable
        || account.executable
        || account.owner != &system_program::ID
        || !account.try_data_is_empty().map_err(|_| record_error())?
    {
        return Err(record_error());
    }
    Ok(())
}

fn close_pda_to_zero(program_id: &Pubkey, source: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if source.owner != program_id {
        return Err(record_error());
    }
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| record_error())?;
        **source_lamports = 0;
    }
    source.resize(0).map_err(|_| record_error())?;
    source.assign(&system_program::ID);
    if !is_vacant(source) {
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

/// Observe the live deployment an `ArtifactRelease` record claims, once.
///
/// # Why the Registry pays for this and no hot route does
///
/// `ArtifactReleaseV1` carries an `elf_digest`, a `deployment_slot` and an
/// upgrade authority for a Program it names. Until this route existed, nothing
/// ever compared those three against the accounts they describe: a finalized
/// record proved `hash(bytes) == digest` about ITSELF, which says nothing about
/// the address inside it. So every reader that needed the artifact to be the
/// admitted one hashed the complete observed ELF, and
/// `dclutch-trading::shadow_accelerator_auth`'s own doc said exactly why -- *"nothing
/// has bound its `elf_digest` to the account being observed."*
///
/// Measured on real ELFs 2026-09-02: that hash cost the Dealer equity Add
/// **370,983 CU** of a 1,399,700 budget, on 744,840 bytes, inside a strategy
/// authentication that was 30% of the whole transaction -- and it was paid
/// again on every action, forever, to learn a fact that never changes.
///
/// Decision 0012 already owns the argument that makes it unnecessary
/// (`slot_pinned_release_elf_digest_v1`): a Loader V3 deployment cannot move
/// while its ProgramData still carries the slot the release bound. That
/// argument was sound and unusable, because it starts from a bound digest
/// somebody checked. Role ACTIVATION checked one; a certificate-pinned artifact
/// had no such moment. This is that moment, moved to where it costs once per
/// release instead of once per action.
///
/// The comparison is `ArtifactReleaseV1::authenticate_deployment`, which owns
/// all seven conjuncts and is decided by the Lean corpus in
/// `ProtocolInfrastructure.lean`; this function only supplies it with facts
/// read out of the live accounts in this very invocation.
fn observe_artifact_release_deployment_v1(
    cursor: StagingCursorV1,
    frame: &FinalizeFrame<'_, '_>,
) -> Result<(), ProgramError> {
    let is_release = cursor.key().schema_release_id().to_bytes() == ARTIFACT_RELEASE_SCHEMA_ID_V1;
    let (program, programdata) = match (is_release, frame.deployment) {
        (false, None) => return Ok(()),
        (true, Some(pair)) => pair,
        // A release without its deployment, or a deployment attached to a
        // record that names no address. Both are frame errors and neither is a
        // statement about the bytes, so they are one code and not the other.
        _ => return Err(deployment_frame_error()),
    };
    let data = frame
        .raw
        .try_borrow_data()
        .map_err(|_| not_deployed_error())?;
    let release = ArtifactReleaseV1::decode(&data).map_err(|_| not_deployed_error())?;
    drop(data);
    if program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
    {
        return Err(not_deployed_error());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| not_deployed_error())?;
    let program_view = ProgramV3View::parse(&program_bytes).map_err(|_| not_deployed_error())?;
    let programdata_link = program_view.programdata();
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| not_deployed_error())?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| not_deployed_error())?;
    // THE ONE ELF HASH IN THE PROTOCOL'S STEADY STATE. Everything downstream of
    // a finalized release spends a `u64` slot compare instead of this.
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        programdata_link,
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| not_deployed_error())?;
    release
        .authenticate_deployment(observation)
        .map_err(release_deployment_refusal_v1)
}

/// Name what a finalization observation disagreed about.
///
/// A DISCARDED CAUSE IS A SEARCH, and `authenticate_deployment` has already
/// decided which of its eight conjuncts failed. Three accusations, not one:
/// the deployment is not there, the substrate moved under its own authority,
/// or the bytes at that address are not these bytes. `ReleaseSuperseded` is the
/// Registry's existing name for the middle one and keeps it here, so an
/// operator reads the same word at finalization that a hot route will read
/// later if the substrate moves again.
///
/// `ProtocolInfrastructure.lean`'s `ReleaseObservation.outcome` is the author
/// of this partition; `generated_release_finalization_corpus.rs` replays every
/// case through this function.
const fn release_deployment_refusal_v1(error: dclutch_registry::Error) -> ProgramError {
    use dclutch_registry::Error as ReleaseError;
    match error {
        ReleaseError::DeploymentIdentityMismatch
        | ReleaseError::ProgramDataLinkMismatch
        | ReleaseError::LoaderOwnerMismatch
        | ReleaseError::ProgramNotExecutable
        | ReleaseError::ProgramDataExecutable => not_deployed_error(),
        ReleaseError::ReleaseSupersededByUpgrade => {
            ProgramError::Custom(RegistryError::ReleaseSuperseded as u32)
        }
        _ => ProgramError::Custom(RegistryError::ArtifactReleaseElfMismatch as u32),
    }
}

const fn deployment_frame_error() -> ProgramError {
    ProgramError::Custom(RegistryError::ArtifactReleaseDeploymentFrame as u32)
}

const fn not_deployed_error() -> ProgramError {
    ProgramError::Custom(RegistryError::ArtifactReleaseNotDeployed as u32)
}

fn map_record_error(_: dclutch_registry::record::Error) -> ProgramError {
    record_error()
}

const fn record_error() -> ProgramError {
    ProgramError::Custom(RegistryError::Record as u32)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_registry::record::{
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

    /// Build a still-`Building` cursor and its geometry, as an abandoned record
    /// set looks between `Begin` and the page(s) that never came.
    fn building_cursor(
        registry: Pubkey,
        sponsor: Pubkey,
        content: &[u8],
    ) -> (StagingCursorV1, Pubkey, Pubkey, u64, u64) {
        let (request, begin_accounts) = begin_fixture(registry, sponsor, content);
        let frame = BeginFrame::parse(&begin_accounts).expect("Begin frame");
        let cursor = authenticate_begin(&registry, &frame, request)
            .expect("Begin plan")
            .cursor;
        let raw_key = *begin_accounts.get(1).expect("raw role").key;
        let cursor_key = *begin_accounts.get(2).expect("cursor role").key;
        let raw_rent = Rent::default().minimum_balance(content.len());
        let cursor_rent = Rent::default().minimum_balance(STAGING_CURSOR_BYTES_V1);
        (cursor, raw_key, cursor_key, raw_rent, cursor_rent)
    }

    #[test]
    fn abort_after_expiry_is_permissionless_and_pays_the_bounty_to_the_actor() {
        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content = b"an abandoned in-progress record";
        let (cursor, raw_key, cursor_key, raw_rent, cursor_rent) =
            building_cursor(registry, sponsor, content);
        // Begin charged cursor_rent plus the bounty (== cursor_rent) into the cursor.
        let bounty = cursor.cleanup_bounty_lamports();
        assert_eq!(bounty, cursor_rent);
        let cursor_balance = cursor_rent + bounty;
        let raw = account(
            raw_key,
            false,
            true,
            raw_rent,
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_balance,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let sponsor_before = 11;
        let sponsor_wallet = account(
            sponsor,
            false,
            true,
            sponsor_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        // A stranger, not even a signer, strictly at/after expiry (slot 200 >= 101).
        let actor_key = Pubkey::new_unique();
        let actor_before = 7;
        let actor = account(
            actor_key,
            false,
            true,
            actor_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        let clock = account(
            sysvar::clock::ID,
            false,
            false,
            1,
            clock_data(200),
            sysvar::ID,
            false,
        );
        process_abort(
            &registry,
            &[
                raw.clone(),
                stage.clone(),
                sponsor_wallet.clone(),
                actor.clone(),
                clock,
            ],
            AbortRecordV1,
        )
        .expect("permissionless expired abort");
        assert_eq!(actor.lamports(), actor_before + bounty);
        assert_eq!(
            sponsor_wallet.lamports(),
            sponsor_before + raw_rent + cursor_rent
        );
        assert!(is_vacant(&raw));
        assert!(is_vacant(&stage));

        // A wrong sponsor wallet (not the cursor-committed refund) refuses.
        let raw = account(
            raw_key,
            false,
            true,
            raw_rent,
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_balance,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let wrong_sponsor = account(
            Pubkey::new_unique(),
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let actor = account(
            Pubkey::new_unique(),
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let clock = account(
            sysvar::clock::ID,
            false,
            false,
            1,
            clock_data(200),
            sysvar::ID,
            false,
        );
        assert_eq!(
            process_abort(
                &registry,
                &[raw, stage, wrong_sponsor, actor, clock],
                AbortRecordV1,
            ),
            Err(record_error())
        );
    }

    #[test]
    fn abort_before_expiry_requires_the_sponsor_signature_and_withholds_the_bounty() {
        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content = b"a sponsor cancels their own work";
        let (cursor, raw_key, cursor_key, raw_rent, cursor_rent) =
            building_cursor(registry, sponsor, content);
        let bounty = cursor.cleanup_bounty_lamports();
        let cursor_balance = cursor_rent + bounty;
        let sponsor_before = 5;

        // The committed sponsor, signing, before expiry (slot 50 < 101): the same
        // account is both the sponsor wallet and the abort actor. Cloning one
        // AccountInfo into both slots shares its lamport cell, exactly as the
        // runtime deduplicates a twice-passed account.
        let raw = account(
            raw_key,
            false,
            true,
            raw_rent,
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_balance,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let sponsor_signed = account(
            sponsor,
            true,
            true,
            sponsor_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        let clock = account(
            sysvar::clock::ID,
            false,
            false,
            1,
            clock_data(50),
            sysvar::ID,
            false,
        );
        process_abort(
            &registry,
            &[
                raw.clone(),
                stage.clone(),
                sponsor_signed.clone(),
                sponsor_signed.clone(),
                clock,
            ],
            AbortRecordV1,
        )
        .expect("early sponsor abort");
        // No bounty is paid on an early abort; every lamport returns to sponsor.
        assert_eq!(
            sponsor_signed.lamports(),
            sponsor_before + raw_rent + cursor_balance
        );
        assert!(is_vacant(&raw));
        assert!(is_vacant(&stage));

        // Same shape, but the sponsor did not sign: refused before any mutation.
        let raw = account(
            raw_key,
            false,
            true,
            raw_rent,
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_balance,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let sponsor_unsigned = account(
            sponsor,
            false,
            true,
            sponsor_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        let clock = account(
            sysvar::clock::ID,
            false,
            false,
            1,
            clock_data(50),
            sysvar::ID,
            false,
        );
        assert_eq!(
            process_abort(
                &registry,
                &[
                    raw.clone(),
                    stage.clone(),
                    sponsor_unsigned.clone(),
                    sponsor_unsigned,
                    clock,
                ],
                AbortRecordV1,
            ),
            Err(record_error())
        );
        // Nothing moved: the record is still live.
        assert!(!is_vacant(&raw));
        assert!(!is_vacant(&stage));

        // A stranger who is not the sponsor cannot abort before expiry, even
        // signing: the contract refuses `AbortBeforeExpiry`.
        let raw = account(
            raw_key,
            false,
            true,
            raw_rent,
            vec![0; content.len()],
            registry,
            false,
        );
        let stage = account(
            cursor_key,
            false,
            true,
            cursor_balance,
            cursor.to_bytes().to_vec(),
            registry,
            false,
        );
        let sponsor_wallet = account(
            sponsor,
            false,
            true,
            sponsor_before,
            Vec::new(),
            system_program::ID,
            false,
        );
        let stranger = account(
            Pubkey::new_unique(),
            true,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let clock = account(
            sysvar::clock::ID,
            false,
            false,
            1,
            clock_data(50),
            sysvar::ID,
            false,
        );
        assert_eq!(
            process_abort(
                &registry,
                &[raw, stage, sponsor_wallet, stranger, clock],
                AbortRecordV1,
            ),
            Err(record_error())
        );
    }

    /// **S-3 TRIPWIRE. Read this before you write a record-reclamation route.**
    ///
    /// `docs/design/TRUST_RATCHET_V1.md` §7 names the one staleness class the
    /// capability seal cannot address by re-deriving something: the account
    /// whose properties were sealed is not in the sealed frame at all. The
    /// shipped seal carries exactly one such proposition —
    ///
    /// > at seal time, the canonical staging cursor for this
    /// > `(schema, digest)` was vacant and System-owned
    ///
    /// — because `borrow_sealed_record` aliases the raw account into the
    /// staging slot and never looks at the real cursor. That proposition is
    /// sound only while **finalization is the point of no return**: a finalized
    /// record's bytes must never again be mutable, and the way that is enforced
    /// is that its raw account can never be re-`Begin`-ed.
    ///
    /// This test pins WHICH refusal does that, because §7.1's claim is precise
    /// and easy to get backwards. `Finalize` destroys the cursor, so after
    /// finalization the *cursor*'s `require_prefunded_vacant` is satisfied and
    /// refuses nothing. The load-bearing check is the *raw* account's, at
    /// `authenticate_begin`: a finalized raw record is Registry-owned with
    /// `exact_length` bytes, and `is_prefunded_vacant` demands System-owned,
    /// non-executable and empty.
    ///
    /// The two halves below are a one-variable control. Same fixture, same
    /// request, same frame, same vacant cursor; the raw account is finalized in
    /// one and vacant in the other, and only that moves the answer. So this is
    /// not a restatement of `is_prefunded_vacant` — it is the assertion that
    /// removing the raw conjunct would ADMIT, which is what a reclamation route
    /// would have to do to hand a finalized record back to `Begin`.
    ///
    /// **If you are here because this went red**: you have made a finalized raw
    /// record vacant again, or removed the check that refuses one. Either way
    /// the seal's finality window is now open, and the seal will not notice —
    /// the cursor is not in its frame, and its digest re-pin cannot tell a
    /// complete-but-unfinalized record from a finalized one. Re-argue §7 before
    /// you re-run this, and take
    /// `programs/dclutch-trading-sbf/src/hot_v3/seal.rs::borrow_sealed_record`
    /// with you.
    #[test]
    fn finalization_is_the_point_of_no_return_and_the_raw_account_is_what_enforces_it() {
        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content = b"a record that has been finalized";

        // Control: the ordinary prestate, both PDAs prefunded-vacant. `Begin`
        // authenticates. Everything below differs from this in one account.
        let (request, accounts) = begin_fixture(registry, sponsor, content);
        let frame = BeginFrame::parse(&accounts).expect("Begin frame");
        authenticate_begin(&registry, &frame, request)
            .expect("a vacant raw record admits Begin: this is the control");

        // The seal's proposition, as a reachable state: the record is
        // finalized. Registry-owned, exactly `exact_length` bytes, rent-exempt
        // -- and its cursor is gone, which is what finalization means.
        let (request, mut finalized) = begin_fixture(registry, sponsor, content);
        let raw_key = *finalized.get(1).expect("raw role").key;
        *finalized.get_mut(1).expect("raw role") = account(
            raw_key,
            false,
            true,
            Rent::default().minimum_balance(content.len()),
            vec![0; content.len()],
            registry,
            false,
        );
        let frame = BeginFrame::parse(&finalized).expect("finalized-raw frame shape");
        // The cursor half of the conjunction is SATISFIED here and refuses
        // nothing: `Finalize` closed it, so it is System-owned and empty.
        assert!(
            is_prefunded_vacant(frame.cursor),
            "a finalized record's cursor is vacant, so the cursor check cannot be the one that refuses"
        );
        assert!(
            !is_prefunded_vacant(frame.raw),
            "a finalized raw record must not read as prefunded-vacant"
        );
        assert_eq!(
            authenticate_begin(&registry, &frame, request).err(),
            Some(record_error()),
            "a finalized record was re-Begun: the seal's finality window is open"
        );

        // And the same thing one step weaker, because a reclamation route that
        // closed a finalized record without zeroing it would leave this shape:
        // Registry-owned but empty. Still not vacant, still refused.
        let (request, mut reclaimed) = begin_fixture(registry, sponsor, content);
        *reclaimed.get_mut(1).expect("raw role") =
            account(raw_key, false, true, 0, Vec::new(), registry, false);
        let frame = BeginFrame::parse(&reclaimed).expect("reclaimed-raw frame shape");
        assert_eq!(
            authenticate_begin(&registry, &frame, request).err(),
            Some(record_error()),
            "a Registry-owned empty raw record admitted Begin"
        );
    }
}

#[cfg(test)]
mod release_finalization_corpus {
    extern crate std;

    use dclutch_core_contract::ContentId;
    use dclutch_registry::release_set::ProgramIdentityV1;
    use dclutch_registry::{ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1};
    use solana_program::program_error::ProgramError;
    use solana_sdk_ids::bpf_loader_upgradeable;

    use crate::RegistryError;
    use crate::generated_release_finalization_corpus::{
        RELEASE_FINALIZATION_OUTCOME_ADMIT, RELEASE_FINALIZATION_OUTCOME_ELF_MISMATCH,
        RELEASE_FINALIZATION_OUTCOME_NOT_DEPLOYED, RELEASE_FINALIZATION_OUTCOME_SUPERSEDED,
        RELEASE_FINALIZATION_VECTORS_V1, ReleaseFinalizationVectorV1,
    };

    fn fill(value: u8) -> [u8; 32] {
        [value; 32]
    }

    /// The exact outcome this program publishes for one Lean-decided case.
    ///
    /// Drives the REAL adapter: the release constructor, the observation
    /// constructor, `authenticate_deployment`'s eight conjuncts in their own
    /// order, and `release_deployment_refusal_v1`'s partition. Nothing about
    /// the rule is restated here -- only the vector is turned into accounts.
    fn observed_outcome(vector: ReleaseFinalizationVectorV1) -> u8 {
        let program = ProgramIdentityV1::new(fill(0x11)).expect("program identity");
        let loader =
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity");
        let programdata = fill(0x22);
        let release = ArtifactReleaseV1::new(
            program,
            loader,
            programdata,
            ContentId::new(fill(0x33)).expect("semantic release"),
            fill(vector.bound_elf_digest),
            vector.bound_slot,
            if vector.bound_policy_immutable {
                ArtifactUpgradePolicyV1::Immutable
            } else {
                ArtifactUpgradePolicyV1::ExactAuthority
            },
            vector.bound_authority.map(fill),
        )
        .expect("canonical release");
        // A substituted identity is the adapter's own parse boundary, so the
        // vector's boolean becomes a different observed program id rather than
        // a second copy of the equality.
        let observed_program = if vector.observed_identity_matches {
            program.to_bytes()
        } else {
            fill(0xee)
        };
        let observed_link = if vector.observed_programdata_link_matches {
            programdata
        } else {
            fill(0xef)
        };
        let observed_owner = if vector.observed_loader_owns_both {
            loader.to_bytes()
        } else {
            fill(0xf0)
        };
        let observation = DeploymentObservationV1::new(
            observed_program,
            observed_owner,
            vector.observed_program_executable,
            programdata,
            observed_owner,
            vector.observed_programdata_executable,
            observed_link,
            loader.to_bytes(),
            vector.observed_slot,
            fill(vector.observed_elf_digest),
            vector.observed_authority.map(fill),
        )
        .expect("canonical observation");
        match release.authenticate_deployment(observation) {
            Ok(()) => RELEASE_FINALIZATION_OUTCOME_ADMIT,
            Err(error) => match super::release_deployment_refusal_v1(error) {
                ProgramError::Custom(code)
                    if code == RegistryError::ArtifactReleaseNotDeployed as u32 =>
                {
                    RELEASE_FINALIZATION_OUTCOME_NOT_DEPLOYED
                }
                ProgramError::Custom(code) if code == RegistryError::ReleaseSuperseded as u32 => {
                    RELEASE_FINALIZATION_OUTCOME_SUPERSEDED
                }
                ProgramError::Custom(code)
                    if code == RegistryError::ArtifactReleaseElfMismatch as u32 =>
                {
                    RELEASE_FINALIZATION_OUTCOME_ELF_MISMATCH
                }
                other => panic!("unpartitioned finalization refusal {other:?}"),
            },
        }
    }

    #[test]
    fn every_lean_decided_finalization_case_replays_through_this_program() {
        for vector in RELEASE_FINALIZATION_VECTORS_V1 {
            assert_eq!(
                observed_outcome(vector),
                vector.outcome,
                "{} disagreed with ProtocolInfrastructure.lean",
                vector.name
            );
        }
    }

    /// A corpus that answered one way throughout would satisfy the replay
    /// above and prove nothing. Lean pins the coverage; this pins that the
    /// corpus which reached Rust is the one Lean pinned.
    #[test]
    fn the_corpus_decides_every_outcome() {
        for expected in [
            RELEASE_FINALIZATION_OUTCOME_ADMIT,
            RELEASE_FINALIZATION_OUTCOME_NOT_DEPLOYED,
            RELEASE_FINALIZATION_OUTCOME_SUPERSEDED,
            RELEASE_FINALIZATION_OUTCOME_ELF_MISMATCH,
        ] {
            assert!(
                RELEASE_FINALIZATION_VECTORS_V1
                    .iter()
                    .any(|vector| vector.outcome == expected),
                "no vector decides outcome {expected}"
            );
        }
    }
}

/// The Registry's finalization observation, replayed over the ONE deployment
/// this protocol actually has on a public chain.
///
/// `release_finalization_corpus` above proves the partition against twelve
/// Lean-decided vectors built from `[0x11; 32]`-shaped identities. That is the
/// right instrument for the rule and the wrong one for the question an operator
/// asks on deploy day, which is whether THESE bytes at THIS address on devnet
/// will be admitted -- a question that has been answered by hand, from a CLI
/// summary line, every time it has been asked.
///
/// So the facts below are transcribed once, from the finalized ProgramData
/// image of the General accelerator deployed to devnet on 2026-09-02
/// (`docs/evidence/GENERAL_ACCELERATOR_DEVNET_2026_09_02.md`), and every
/// assertion here is the program's own `authenticate_deployment` and its own
/// `release_deployment_refusal_v1` over them. Nothing about the rule is
/// restated; only one real observation is turned into accounts.
///
/// What this DOES NOT prove: that a devnet Registry has finalized this record.
/// No deployed Registry carries this code -- cohort-12 and cohort-13 both
/// predate `90a8563f` -- so the on-chain half is cohort-14's, and the runbook
/// step in the evidence doc is where it is claimed.
#[cfg(test)]
mod devnet_general_accelerator_observation {
    extern crate std;

    use dclutch_core_contract::ContentId;
    use dclutch_registry::release_set::ProgramIdentityV1;
    use dclutch_registry::{ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1};
    use solana_program::program_error::ProgramError;
    use solana_sdk_ids::bpf_loader_upgradeable;

    use crate::RegistryError;

    /// `8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY`.
    const PROGRAM: [u8; 32] = [
        0x74, 0x39, 0x19, 0xa4, 0xd4, 0x0e, 0x02, 0x98, 0x91, 0x8e, 0xbf, 0xbd, 0x83, 0xb2, 0xab,
        0x58, 0x77, 0x95, 0x49, 0xf1, 0xcd, 0x43, 0x11, 0xa3, 0x9a, 0xcf, 0x9d, 0x13, 0x84, 0xcd,
        0xdd, 0x81,
    ];
    /// `HcxFzWKaFzrVVnvgx6BWuNbo278pgpYY5CrxyVe67Sxb`, the derived ProgramData.
    const PROGRAMDATA: [u8; 32] = [
        0xf6, 0xf0, 0xcc, 0xd3, 0x01, 0x0f, 0x87, 0xe5, 0x28, 0x97, 0x94, 0x14, 0x51, 0xc7, 0x8b,
        0x04, 0x6e, 0x22, 0xe2, 0x61, 0xc6, 0x07, 0x66, 0x02, 0x7e, 0x0a, 0x7d, 0xd1, 0x7f, 0x60,
        0xfb, 0x7c,
    ];
    /// `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`, the deployer, read out of
    /// the ProgramData header's authority slot rather than off the CLI.
    const AUTHORITY: [u8; 32] = [
        0x3b, 0x65, 0xa9, 0x3a, 0x66, 0x53, 0x46, 0x99, 0x3e, 0x31, 0xfd, 0x6e, 0xd5, 0x27, 0x7a,
        0x98, 0x14, 0xc3, 0x7f, 0x43, 0x07, 0x6c, 0x36, 0x33, 0x72, 0xc8, 0xa1, 0x04, 0x1d, 0xf3,
        0x7a, 0xde,
    ];
    /// SHA-256 of the observed ELF tail, which for this deployment carries no
    /// padding and therefore equals the built artifact's own digest.
    const ELF_DIGEST: [u8; 32] = [
        0x61, 0xb2, 0xd7, 0x3d, 0x44, 0xf2, 0x47, 0x00, 0x51, 0xb4, 0x0e, 0x39, 0xcd, 0xa1, 0xd3,
        0x1a, 0x5f, 0x67, 0x67, 0x94, 0x29, 0xea, 0xcd, 0x54, 0x48, 0xd5, 0xe5, 0xac, 0x58, 0x3b,
        0x74, 0xae,
    ];
    /// `0x1d52b2fe`, from bytes 4..12 of the ProgramData header.
    const DEPLOYMENT_SLOT: u64 = 491_959_038;

    fn release(
        elf_digest: [u8; 32],
        deployment_slot: u64,
        authority: Option<[u8; 32]>,
    ) -> ArtifactReleaseV1 {
        ArtifactReleaseV1::new(
            ProgramIdentityV1::new(PROGRAM).expect("program identity"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            PROGRAMDATA,
            // The semantic release identity is an operator-stated fact and is
            // not one of `authenticate_deployment`'s eight conjuncts, so it is
            // varied nowhere here. The accelerator having no protocol-owned
            // semantic identity of its own is named as debt in the evidence doc.
            ContentId::new(ELF_DIGEST).expect("semantic release"),
            elf_digest,
            deployment_slot,
            match authority {
                Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
                None => ArtifactUpgradePolicyV1::Immutable,
            },
            authority,
        )
        .expect("canonical release")
    }

    fn observation(
        program: [u8; 32],
        elf_digest: [u8; 32],
        deployment_slot: u64,
        authority: Option<[u8; 32]>,
    ) -> DeploymentObservationV1 {
        DeploymentObservationV1::new(
            program,
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            PROGRAMDATA,
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            PROGRAMDATA,
            bpf_loader_upgradeable::ID.to_bytes(),
            deployment_slot,
            elf_digest,
            authority,
        )
        .expect("canonical observation")
    }

    fn outcome(
        release: ArtifactReleaseV1,
        observed: DeploymentObservationV1,
    ) -> Result<(), ProgramError> {
        release
            .authenticate_deployment(observed)
            .map_err(super::release_deployment_refusal_v1)
    }

    /// The two transcribed addresses are checked against each other by a
    /// different author before anything else is claimed about them.
    ///
    /// A mistyped `PROGRAM` would otherwise make every test below a test of
    /// two consistent inventions -- and one was mistyped while this module was
    /// being written. `find_program_address` is the Loader's own derivation and
    /// it does not know what these bytes were supposed to be.
    #[test]
    fn the_transcribed_program_derives_the_transcribed_programdata() {
        assert_eq!(
            solana_program::pubkey::Pubkey::find_program_address(
                &[PROGRAM.as_slice()],
                &bpf_loader_upgradeable::ID,
            )
            .0
            .to_bytes(),
            PROGRAMDATA,
        );
    }

    /// The live devnet accelerator is ADMITTED by the exact code a `Finalize`
    /// runs, with the record's slot and authority taken from the header this
    /// deployment really carries.
    #[test]
    fn the_live_devnet_accelerator_is_admitted_at_finalization() {
        outcome(
            release(ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
            observation(PROGRAM, ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
        )
        .expect("the deployed General accelerator authenticates its own record");
    }

    /// And each substitution refuses BY ITS OWN NAME, which is the property
    /// that makes the three codes worth having: an operator reads a remedy.
    ///
    /// The `ExactAuthority` slot arm is the one this deployment can actually
    /// reach -- the deployer still holds the authority, so a redeploy is a
    /// thing that can happen -- and it is `ReleaseSuperseded`, not a mystery.
    #[test]
    fn every_substitution_of_the_live_deployment_refuses_by_its_own_name() {
        let custom = |code: RegistryError| ProgramError::Custom(code as u32);

        // The record names a program that is not the one observed.
        assert_eq!(
            outcome(
                release(ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
                observation(PROGRAMDATA, ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
            ),
            Err(custom(RegistryError::ArtifactReleaseNotDeployed)),
        );

        // The substrate moved forward under the authority the record names.
        assert_eq!(
            outcome(
                release(ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
                observation(PROGRAM, ELF_DIGEST, DEPLOYMENT_SLOT + 1, Some(AUTHORITY)),
            ),
            Err(custom(RegistryError::ReleaseSuperseded)),
        );

        // One byte of the ELF digest.
        let mut other_digest = ELF_DIGEST;
        other_digest[0] ^= 0x01;
        assert_eq!(
            outcome(
                release(ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
                observation(PROGRAM, other_digest, DEPLOYMENT_SLOT, Some(AUTHORITY)),
            ),
            Err(custom(RegistryError::ArtifactReleaseElfMismatch)),
        );

        // A record claiming this accelerator is immutable. It is not: an
        // omitted upgrade-authority flag would mint exactly this record, and
        // the devnet General compiler refuses that flag's absence for this
        // reason.
        assert_eq!(
            outcome(
                release(ELF_DIGEST, DEPLOYMENT_SLOT, None),
                observation(PROGRAM, ELF_DIGEST, DEPLOYMENT_SLOT, Some(AUTHORITY)),
            ),
            Err(custom(RegistryError::ArtifactReleaseElfMismatch)),
        );
    }
}
