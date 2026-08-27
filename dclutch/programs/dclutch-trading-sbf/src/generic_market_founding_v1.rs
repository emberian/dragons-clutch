//! Atomic generic Market founding over the canonical Core, Custody, and Claims ABIs.
//!
//! The outer instruction carries no caller-authored economic coordinates. Four
//! readonly data accounts supply the exact selected Found request and the three
//! deterministic child requests. Core, Custody, and Claims each reauthenticate
//! those bytes against their own persisted semantic authorities before any
//! mutation. Every immediate child receipt is then joined before the next CPI;
//! the final Core Open is last, so any late refusal rolls back the entire chain.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_ACCOUNT_COUNT_V5, CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
    CLAIMS_FOUNDING_RECEIPT_BYTES_V5, CLAIMS_FOUNDING_REQUEST_BYTES_V5, ClaimsFoundingReceiptV5,
    ClaimsFoundingRequestV5,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
    PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCallerRoleV1,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyReceiptV1, ProjectedCustodyRequestV1,
};
use dclutch_market_core_codec::{
    GENERIC_FOUNDING_ACK_BYTES_V1, GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_FOUND_POST_RESOURCE_DOMAIN_V1, GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1, GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1,
    GENERIC_FOUNDING_REQUEST_BYTES_V1, GenericFoundingAckV1, GenericFoundingRequestV1,
    GenericFoundingStageV1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

/// Sole top-level generic Market founding instruction.
pub const GENERIC_MARKET_FOUNDING_MAGIC_V1: [u8; 8] = *b"DCLTGMF1";
/// Exact outer instruction width. All economic bytes live in readonly accounts.
pub const GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact readonly raw-request prefix width.
pub const GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V1: usize = 4;

const FOUND_RAW: usize = 0;
const LOCK_RAW: usize = 1;
const REALIZE_RAW: usize = 2;
const CLAIMS_RAW: usize = 3;

const CORE_FOUND_CORE_PROGRAM: usize = 19;
const CORE_FOUND_TRADING_PROGRAM: usize = 31;
const CORE_FOUND_MARKET: usize = 1;

const CORE_FOUND_PERMIT_SUFFIX: usize = 0;
const CORE_FOUND_CLAIMS_PROGRAM_SUFFIX: usize = 7;
const CORE_FOUND_CUSTODY_PROGRAM_SUFFIX: usize = 9;

const CLAIMS_AGGREGATE: usize = 2;
const CLAIMS_POSITION: usize = 3;
const CLAIMS_ADMISSION: usize = 4;

/// Return whether bytes select the sole generic founding outer.
#[must_use]
pub fn is_generic_market_founding_v1(instruction_data: &[u8]) -> bool {
    instruction_data == GENERIC_MARKET_FOUNDING_MAGIC_V1
}

/// Execute Lock→Found→Realize→Claims→Open as one rollback domain.
#[inline(never)]
pub fn process_generic_market_founding_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_generic_market_founding_v1(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let found_raw = raw_account_bytes(accounts, FOUND_RAW, GENERIC_FOUNDING_REQUEST_BYTES_V1)?;
    let found = decode_found_request(&found_raw)?;
    let frame = GenericFoundingFrameV1::parse(accounts, usize::from(found.funding_count()))?;
    let lock_raw = raw_account_bytes(accounts, LOCK_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let realize_raw = raw_account_bytes(accounts, REALIZE_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let claims_raw = raw_account_bytes(accounts, CLAIMS_RAW, CLAIMS_FOUNDING_REQUEST_BYTES_V5)?;
    let lock = decode_projected_request(&lock_raw)?;
    let realize = decode_projected_request(&realize_raw)?;
    let claims = decode_claims_request(&claims_raw)?;
    authenticate_request_join(
        program_id, &frame, &found, &lock, &realize, &claims, &lock_raw,
    )?;

    let lock_receipt = execute_lock(program_id, &frame, &lock, &lock_raw)?;
    let found_ack = execute_core_found(program_id, &frame, &found, &found_raw, &lock_receipt)?;
    authenticate_found_to_claims(&frame, &found_ack, &claims)?;
    let realize_receipt = execute_realize(program_id, &frame, &realize, &realize_raw)?;
    authenticate_realize_receipt(&frame, &found, &realize, &realize_raw, &realize_receipt)?;
    let claims_receipt = execute_claims(
        program_id,
        &frame,
        &claims,
        &claims_raw,
        &lock_receipt,
        &realize_receipt,
    )?;
    let open = found
        .with_stage(GenericFoundingStageV1::Open)
        .map_err(|_| TradingSbfError::Content)?;
    let open_raw = open.encode().map_err(|_| TradingSbfError::Content)?;
    let open_ack = execute_core_open(program_id, &frame, &open, &open_raw, &claims_receipt)?;
    set_return_data(&open_ack);
    Ok(())
}

struct GenericFoundingFrameV1<'accounts, 'info> {
    lock: &'accounts [AccountInfo<'info>],
    found: &'accounts [AccountInfo<'info>],
    realize: &'accounts [AccountInfo<'info>],
    claims: &'accounts [AccountInfo<'info>],
    open: &'accounts [AccountInfo<'info>],
    funding_count: usize,
}

impl<'accounts, 'info> GenericFoundingFrameV1<'accounts, 'info> {
    #[inline(never)]
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        funding_count: usize,
    ) -> Result<Self, ProgramError> {
        let found_count = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
            .checked_add(funding_count)
            .and_then(|value| value.checked_add(GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1))
            .ok_or(TradingSbfError::Content)?;
        let lock_start = GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V1;
        let found_start = lock_start
            .checked_add(PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1)
            .ok_or(TradingSbfError::Content)?;
        let realize_start = found_start
            .checked_add(found_count)
            .ok_or(TradingSbfError::Content)?;
        let claims_start = realize_start
            .checked_add(PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1)
            .ok_or(TradingSbfError::Content)?;
        let open_start = claims_start
            .checked_add(CLAIMS_FOUNDING_ACCOUNT_COUNT_V5)
            .ok_or(TradingSbfError::Content)?;
        let end = open_start
            .checked_add(GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1)
            .ok_or(TradingSbfError::Content)?;
        if accounts.len() != end {
            return Err(TradingSbfError::Content.into());
        }
        authenticate_raw_accounts(
            accounts
                .get(..GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V1)
                .ok_or(TradingSbfError::Content)?,
        )?;
        Ok(Self {
            lock: subslice(
                accounts,
                lock_start,
                PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1,
            )?,
            found: subslice(accounts, found_start, found_count)?,
            realize: subslice(
                accounts,
                realize_start,
                PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
            )?,
            claims: subslice(accounts, claims_start, CLAIMS_FOUNDING_ACCOUNT_COUNT_V5)?,
            open: subslice(accounts, open_start, GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1)?,
            funding_count,
        })
    }

    fn suffix_start(&self) -> Result<usize, ProgramError> {
        GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
            .checked_add(self.funding_count)
            .ok_or_else(|| TradingSbfError::Content.into())
    }

    fn core_program(&self) -> Result<&AccountInfo<'info>, ProgramError> {
        account(self.found, CORE_FOUND_CORE_PROGRAM)
    }

    fn trading_program(&self) -> Result<&AccountInfo<'info>, ProgramError> {
        account(self.found, CORE_FOUND_TRADING_PROGRAM)
    }

    fn claims_program(&self) -> Result<&AccountInfo<'info>, ProgramError> {
        account(
            self.found,
            self.suffix_start()?
                .checked_add(CORE_FOUND_CLAIMS_PROGRAM_SUFFIX)
                .ok_or(TradingSbfError::Content)?,
        )
    }

    fn custody_program(&self) -> Result<&AccountInfo<'info>, ProgramError> {
        account(
            self.found,
            self.suffix_start()?
                .checked_add(CORE_FOUND_CUSTODY_PROGRAM_SUFFIX)
                .ok_or(TradingSbfError::Content)?,
        )
    }

    fn permit(&self) -> Result<&AccountInfo<'info>, ProgramError> {
        account(
            self.found,
            self.suffix_start()?
                .checked_add(CORE_FOUND_PERMIT_SUFFIX)
                .ok_or(TradingSbfError::Content)?,
        )
    }
}

#[inline(never)]
fn authenticate_request_join(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    found: &GenericFoundingRequestV1,
    lock: &ProjectedCustodyRequestV1,
    realize: &ProjectedCustodyRequestV1,
    claims: &ClaimsFoundingRequestV5,
    lock_raw: &[u8],
) -> Result<(), ProgramError> {
    let core = frame.core_program()?;
    let trading = frame.trading_program()?;
    let claims_program = frame.claims_program()?;
    let custody = frame.custody_program()?;
    if !core.executable
        || !trading.executable
        || !claims_program.executable
        || !custody.executable
        || trading.key != program_id
    {
        return Err(TradingSbfError::Release.into());
    }
    authenticate_request_coordinates(
        program_id,
        core.key,
        claims_program.key,
        found,
        lock,
        realize,
        claims,
        lock_raw,
    )
}

/// Authenticate the four selected requests against one another.
///
/// This is the whole cross-request join and it reads no account memory, so the
/// coordinate authentication can be exercised adversarially without a frame.
/// The caller still owns every account flag, executability, and ownership
/// check before this runs.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_request_coordinates(
    program_id: &Pubkey,
    core_program: &Pubkey,
    claims_program: &Pubkey,
    found: &GenericFoundingRequestV1,
    lock: &ProjectedCustodyRequestV1,
    realize: &ProjectedCustodyRequestV1,
    claims: &ClaimsFoundingRequestV5,
    lock_raw: &[u8],
) -> Result<(), ProgramError> {
    if realize.operation != ProjectedCustodyOperationV1::RealizeAndClose
        || realize.caller_role != ProjectedCallerRoleV1::TradingCapability
        || realize.caller_program != program_id.to_bytes()
        || realize.core_program != core_program.to_bytes()
        || claims.claims_program() != claims_program.to_bytes()
        || claims.trading_program() != program_id.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    authenticate_projected_lock_join_v1(program_id, core_program, found, lock)?;
    authenticate_projected_sequence(found, lock, realize)?;
    if realize.resulting_revision != found.projected_resulting_revision()
        || claims.release_set() != found.release_set().to_bytes()
        || claims.market() != found.market().to_bytes()
        || claims.founder() != found.founder().to_bytes()
        || claims.funding_source() != found.funding_source().to_bytes()
        || claims.hoard() != found.hoard().to_bytes()
        || claims.custody_replay() != found.projected_replay().to_bytes()
        || claims.rent_credit() != lock.rent_credit
        || claims.generation() != found.generation()
        || claims.quantity() != found.quantity()
        || claims.basis_scale() != found.basis_scale()
        || claims.custody_request_digest() != hash(lock_raw).to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Authenticate one terminal Lock request against its founding artifact.
///
/// This is the sole definition of what it means for a projected-Custody Lock
/// request to belong to a founding. The atomic outer evaluates it before its
/// first CPI, and `projected_custody_bootstrap_v1` evaluates the same predicate
/// before creating the replay that Lock will consume, so the prestate route and
/// the founding route cannot drift into disagreeing about the same pair.
///
/// It reads no account memory, so it can be exercised adversarially without a
/// frame. The caller still owns every account flag, executability, and
/// ownership check.
#[inline(never)]
pub(crate) fn authenticate_projected_lock_join_v1(
    program_id: &Pubkey,
    core_program: &Pubkey,
    found: &GenericFoundingRequestV1,
    lock: &ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    if lock.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
        || lock.caller_role != ProjectedCallerRoleV1::TradingCapability
        || lock.caller_program != program_id.to_bytes()
        || lock.core_program != core_program.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    let expected_context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        found.context().to_bytes().as_slice(),
    ])
    .to_bytes();
    if lock.market != found.market().to_bytes()
        || lock.generation != found.generation()
        || lock.release_set != found.release_set().to_bytes()
        || lock.parent_capability_root != found.capability_root().to_bytes()
        || lock.context_digest != expected_context
        || lock.funding_source_vault != found.funding_source().to_bytes()
        || lock.hoard_vault != found.hoard().to_bytes()
        || lock.refund_owner != found.beneficiary().to_bytes()
        || lock.expiry_slot != found.expiry_slot()
        || lock.amount
            != found
                .hoard_principal()
                .map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_projected_sequence(
    found: &GenericFoundingRequestV1,
    lock: &ProjectedCustodyRequestV1,
    realize: &ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let mut expected = *lock;
    expected.operation = ProjectedCustodyOperationV1::RealizeAndClose;
    expected.expected_revision = lock.resulting_revision;
    expected.resulting_revision = lock
        .resulting_revision
        .checked_add(1)
        .ok_or(TradingSbfError::Content)?;
    if &expected != realize || expected.resulting_revision != found.projected_resulting_revision() {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn execute_lock(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let custody_program = frame.custody_program()?;
    let digest = hash(raw).to_bytes();
    let seeds = ProjectedCustodyCallerSeedsV1::new(*request, digest);
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(frame.lock, 0)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let bump_seed = [bump];
    let [domain, release, market, root, context, request_digest] = seeds.as_slices();
    let returned = invoke_child(
        custody_program,
        frame.lock,
        raw,
        &[1, 6, 7, 8, 12],
        &[
            domain,
            release,
            market,
            root,
            context,
            request_digest,
            &bump_seed,
        ],
    )?;
    let receipt = decode_lock_receipt(&returned)?;
    if returned.len() != PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1
        || receipt.market != request.market
        || receipt.release_set != request.release_set
        || receipt.context_digest != request.context_digest
        || receipt.source_vault != request.funding_source_vault
        || receipt.hoard_vault != request.hoard_vault
        || receipt.rent_credit != request.rent_credit
        || receipt.request_digest != digest
        || receipt.amount != request.amount
        || receipt.resulting_revision != request.resulting_revision
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(returned)
}

#[inline(never)]
fn execute_core_found(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &GenericFoundingRequestV1,
    raw: &[u8],
    lock_receipt: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let core_program = frame.core_program()?;
    let digest = hash(raw).to_bytes();
    let seeds = caller_seeds(request, digest)?;
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(frame.found, 0)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut child_data = Vec::with_capacity(raw.len() + lock_receipt.len());
    child_data.extend_from_slice(raw);
    child_data.extend_from_slice(lock_receipt);
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest] = seeds.as_slices();
    let permit_index = frame.suffix_start()?;
    let returned = invoke_child(
        core_program,
        frame.found,
        &child_data,
        &[0, CORE_FOUND_MARKET, permit_index],
        &[
            domain,
            release,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ],
    )?;
    authenticate_core_ack(frame, request, raw, &returned, None, true)?;
    Ok(returned)
}

#[inline(never)]
fn authenticate_found_to_claims(
    frame: &GenericFoundingFrameV1<'_, '_>,
    found_ack_raw: &[u8],
    claims: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    let ack =
        GenericFoundingAckV1::decode(found_ack_raw).map_err(|_| TradingSbfError::Transition)?;
    if ack.stage() != GenericFoundingStageV1::FoundAndPermit
        || ack.permit().to_bytes() != frame.permit()?.key.to_bytes()
        || account(frame.claims, 1)?.key != frame.permit()?.key
        || claims.custody_receipt_digest() == [0; 32]
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[inline(never)]
fn execute_realize(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let custody_program = frame.custody_program()?;
    let digest = hash(raw).to_bytes();
    let seeds = ProjectedCustodyCallerSeedsV1::new(*request, digest);
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(frame.realize, 0)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let bump_seed = [bump];
    let [domain, release, market, root, context, request_digest] = seeds.as_slices();
    invoke_child(
        custody_program,
        frame.realize,
        raw,
        &[1],
        &[
            domain,
            release,
            market,
            root,
            context,
            request_digest,
            &bump_seed,
        ],
    )
}

#[inline(never)]
fn authenticate_realize_receipt(
    frame: &GenericFoundingFrameV1<'_, '_>,
    found: &GenericFoundingRequestV1,
    realize: &ProjectedCustodyRequestV1,
    realize_raw: &[u8],
    returned: &[u8],
) -> Result<(), ProgramError> {
    if returned.len() != PROJECTED_CUSTODY_RECEIPT_BYTES_V1 {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt = decode_realize_receipt(returned)?;
    let market_data = account(frame.found, CORE_FOUND_MARKET)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let market_digest = hash(&market_data).to_bytes();
    if !receipt.realized
        || receipt.aborted_open
        || receipt.market != found.market().to_bytes()
        || receipt.release_set != found.release_set().to_bytes()
        || receipt.parent_capability_root != found.capability_root().to_bytes()
        || receipt.context_digest != realize.context_digest
        || receipt.hoard_vault != found.hoard().to_bytes()
        || receipt.amount
            != found
                .hoard_principal()
                .map_err(|_| TradingSbfError::Content)?
        || receipt.request_digest != hash(realize_raw).to_bytes()
        || receipt.market_state_digest != market_digest
        || receipt.rent_credit != realize.rent_credit
        || receipt.resulting_revision != found.projected_resulting_revision()
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn execute_claims(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    raw: &[u8],
    lock_receipt: &[u8],
    realize_receipt: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let claims_program = frame.claims_program()?;
    if request.custody_receipt_digest() != hash(lock_receipt).to_bytes() {
        return Err(TradingSbfError::Content.into());
    }
    let digest = hash(raw).to_bytes();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set()).map_err(|_| TradingSbfError::Content)?,
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(frame.claims, 0)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut child_data = Vec::with_capacity(
        raw.len()
            .checked_add(lock_receipt.len())
            .and_then(|value| value.checked_add(realize_receipt.len()))
            .ok_or(TradingSbfError::Content)?,
    );
    child_data.extend_from_slice(raw);
    child_data.extend_from_slice(lock_receipt);
    child_data.extend_from_slice(realize_receipt);
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest] = seeds.as_slices();
    let returned = invoke_child(
        claims_program,
        frame.claims,
        &child_data,
        &[CLAIMS_AGGREGATE, CLAIMS_POSITION, CLAIMS_ADMISSION],
        &[
            domain,
            release,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ],
    )?;
    authenticate_claims_receipt(frame, request, raw, &returned)?;
    Ok(returned)
}

#[inline(never)]
fn authenticate_claims_receipt(
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    raw: &[u8],
    returned: &[u8],
) -> Result<(), ProgramError> {
    if returned.len() != CLAIMS_FOUNDING_RECEIPT_BYTES_V5 {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt = decode_claims_receipt(returned)?;
    receipt
        .verify_for(request, hash(raw).to_bytes())
        .map_err(|_| TradingSbfError::Transition)?;
    let aggregate = account(frame.claims, CLAIMS_AGGREGATE)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let position = account(frame.claims, CLAIMS_POSITION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let admission = account(frame.claims, CLAIMS_ADMISSION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let combined = hashv(&[
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
        &aggregate,
        &position,
        &admission,
    ])
    .to_bytes();
    if receipt.aggregate_digest() != hash(&aggregate).to_bytes()
        || receipt.position_digest() != hash(&position).to_bytes()
        || receipt.admission_digest() != hash(&admission).to_bytes()
        || receipt.post_resource_digest() != combined
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[inline(never)]
fn execute_core_open(
    program_id: &Pubkey,
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &GenericFoundingRequestV1,
    raw: &[u8],
    claims_receipt: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let core_program = frame.core_program()?;
    let digest = hash(raw).to_bytes();
    let seeds = caller_seeds(request, digest)?;
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(frame.open, 0)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut child_data = Vec::with_capacity(raw.len() + claims_receipt.len());
    child_data.extend_from_slice(raw);
    child_data.extend_from_slice(claims_receipt);
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest] = seeds.as_slices();
    let returned = invoke_child(
        core_program,
        frame.open,
        &child_data,
        &[1, 2, 3],
        &[
            domain,
            release,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ],
    )?;
    authenticate_core_ack(frame, request, raw, &returned, Some(claims_receipt), false)?;
    Ok(returned)
}

#[inline(never)]
fn authenticate_core_ack(
    frame: &GenericFoundingFrameV1<'_, '_>,
    request: &GenericFoundingRequestV1,
    raw: &[u8],
    returned: &[u8],
    post_dependency: Option<&[u8]>,
    found_stage: bool,
) -> Result<(), ProgramError> {
    if returned.len() != GENERIC_FOUNDING_ACK_BYTES_V1 {
        return Err(TradingSbfError::Transition.into());
    }
    let ack = GenericFoundingAckV1::decode(returned).map_err(|_| TradingSbfError::Transition)?;
    let core_program = frame.core_program()?;
    let permit = frame.permit()?;
    let post = if found_stage {
        let market_data = account(frame.found, CORE_FOUND_MARKET)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        let permit_data = permit
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        hashv(&[
            GENERIC_FOUNDING_FOUND_POST_RESOURCE_DOMAIN_V1,
            &market_data,
            &permit_data,
        ])
        .to_bytes()
    } else {
        let market_data = account(frame.open, 1)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        let claims_receipt = post_dependency.ok_or(TradingSbfError::Transition)?;
        hashv(&[
            GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1,
            &market_data,
            claims_receipt,
        ])
        .to_bytes()
    };
    if ack.stage() != request.stage()
        || ack.funding_count() != request.funding_count()
        || ack.core_program().to_bytes() != core_program.key.to_bytes()
        || ack.release_set() != request.release_set()
        || ack.market() != request.market()
        || ack.permit().to_bytes() != permit.key.to_bytes()
        || ack.request_digest().to_bytes() != hash(raw).to_bytes()
        || ack.funding_list_id() != request.funding_list_id()
        || ack.generation() != request.generation()
        || ack.post_resource_digest().to_bytes() != post
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[inline(never)]
fn invoke_child<'info>(
    child_program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    data: &[u8],
    writable: &[usize],
    signer_seeds: &[&[u8]],
) -> Result<Vec<u8>, ProgramError> {
    if !child_program.executable || child_program.is_writable || child_program.is_signer {
        return Err(TradingSbfError::Release.into());
    }
    let mut metas = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.iter().enumerate() {
        let is_writable = writable.contains(&index);
        if is_writable && !account.is_writable {
            return Err(TradingSbfError::Content.into());
        }
        metas.push(if is_writable {
            AccountMeta::new(*account.key, index == 0)
        } else {
            AccountMeta::new_readonly(*account.key, index == 0)
        });
    }
    let instruction = Instruction {
        program_id: *child_program.key,
        accounts: metas,
        data: data.to_vec(),
    };
    let mut infos = accounts.to_vec();
    infos.push(child_program.clone());
    invoke_signed(&instruction, &infos, &[signer_seeds])
        .map_err(|_| TradingSbfError::Transition)?;
    let (producer, returned) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *child_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(returned)
}

fn caller_seeds(
    request: &GenericFoundingRequestV1,
    request_digest: [u8; 32],
) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set().to_bytes()).map_err(|_| TradingSbfError::Content)?,
        request.market().to_bytes(),
        ExecutionRoleV1::Trading,
        request.context().to_bytes(),
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content.into())
}

fn authenticate_raw_accounts(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    if accounts.len() != GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer
            || account.is_writable
            || account.executable
            || accounts
                .get(..index)
                .is_some_and(|prior| prior.iter().any(|value| value.key == account.key))
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn raw_account_bytes(
    accounts: &[AccountInfo<'_>],
    index: usize,
    width: usize,
) -> Result<Vec<u8>, ProgramError> {
    let account = account(accounts, index)?;
    if account.is_signer || account.is_writable || account.executable || account.data_len() != width
    {
        return Err(TradingSbfError::Content.into());
    }
    account
        .try_borrow_data()
        .map(|data| data.to_vec())
        .map_err(|_| TradingSbfError::Content.into())
}

fn decode_found_request(bytes: &[u8]) -> Result<Box<GenericFoundingRequestV1>, ProgramError> {
    let request = GenericFoundingRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    if request.stage() != GenericFoundingStageV1::FoundAndPermit {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(request))
}

fn decode_projected_request(bytes: &[u8]) -> Result<Box<ProjectedCustodyRequestV1>, ProgramError> {
    ProjectedCustodyRequestV1::decode(bytes)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Content.into())
}

fn decode_claims_request(bytes: &[u8]) -> Result<Box<ClaimsFoundingRequestV5>, ProgramError> {
    ClaimsFoundingRequestV5::decode(bytes)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Content.into())
}

fn decode_lock_receipt(bytes: &[u8]) -> Result<Box<ProjectedCustodyLockReceiptV1>, ProgramError> {
    ProjectedCustodyLockReceiptV1::decode(bytes)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Transition.into())
}

fn decode_realize_receipt(bytes: &[u8]) -> Result<Box<ProjectedCustodyReceiptV1>, ProgramError> {
    ProjectedCustodyReceiptV1::decode(bytes)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Transition.into())
}

fn decode_claims_receipt(bytes: &[u8]) -> Result<Box<ClaimsFoundingReceiptV5>, ProgramError> {
    ClaimsFoundingReceiptV5::decode(bytes)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Transition.into())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn subslice<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    start: usize,
    count: usize,
) -> Result<&'accounts [AccountInfo<'info>], ProgramError> {
    accounts
        .get(start..start.checked_add(count).ok_or(TradingSbfError::Content)?)
        .ok_or_else(|| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{GenericFoundingStageV1, Identity};

    use super::*;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn found() -> GenericFoundingRequestV1 {
        GenericFoundingRequestV1::new(
            GenericFoundingStageV1::FoundAndPermit,
            3,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            11,
            12,
            13,
            14,
            15,
            16,
            4,
            1,
        )
        .expect("found")
    }

    #[test]
    fn outer_abi_is_data_account_only_and_stage_distinct() {
        assert!(is_generic_market_founding_v1(
            &GENERIC_MARKET_FOUNDING_MAGIC_V1
        ));
        assert!(!is_generic_market_founding_v1(&[0; 8]));
        assert_eq!(GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1, 8);
        let request = found();
        assert_ne!(
            request.encode().expect("found bytes"),
            request
                .with_stage(GenericFoundingStageV1::Open)
                .expect("open")
                .encode()
                .expect("open bytes")
        );
    }

    fn trading() -> Pubkey {
        Pubkey::new_from_array([21; 32])
    }

    fn core() -> Pubkey {
        Pubkey::new_from_array([22; 32])
    }

    fn claims_program() -> Pubkey {
        Pubkey::new_from_array([23; 32])
    }

    fn lock_request() -> ProjectedCustodyRequestV1 {
        let found = found();
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: found.market().to_bytes(),
            generation: found.generation(),
            realm: [0x31; 32],
            product_record: [0x32; 32],
            product: [0x33; 32],
            source: [0x34; 32],
            release_set: found.release_set().to_bytes(),
            projection_receipt_digest: [0x35; 32],
            parent_capability_root: found.capability_root().to_bytes(),
            context_digest: hashv(&[
                PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
                found.context().to_bytes().as_slice(),
            ])
            .to_bytes(),
            caller_program: trading().to_bytes(),
            payer: [0x36; 32],
            core_program: core().to_bytes(),
            rent_program: [0x37; 32],
            refund_owner: found.beneficiary().to_bytes(),
            rent_credit: [0x38; 32],
            hoard_vault: found.hoard().to_bytes(),
            funding_source_vault: found.funding_source().to_bytes(),
            funding_source_context: found.context().to_bytes(),
            funding_source_compartment: dclutch_custody_contract::CompartmentV1::SeriesEscrow,
            mint: [0x39; 32],
            token_program: [0x3a; 32],
            collateral_release: [0x3b; 32],
            expiry_slot: found.expiry_slot(),
            expected_revision: 2,
            resulting_revision: 3,
            amount: found.hoard_principal().expect("principal"),
            state_rent_lamports: 41,
            vault_rent_lamports: 42,
            funding_source_replay_revision: 43,
            funding_source_state_rent_lamports: 44,
            funding_source_vault_rent_lamports: 45,
        }
    }

    fn realize_request(lock: &ProjectedCustodyRequestV1) -> ProjectedCustodyRequestV1 {
        let mut realize = *lock;
        realize.operation = ProjectedCustodyOperationV1::RealizeAndClose;
        realize.expected_revision = lock.resulting_revision;
        realize.resulting_revision = lock.resulting_revision + 1;
        realize
    }

    fn claims_request(
        lock: &ProjectedCustodyRequestV1,
        lock_raw: &[u8],
    ) -> ClaimsFoundingRequestV5 {
        use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestInputV5;
        let found = found();
        let collateral = found.hoard_principal().expect("principal");
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: found.release_set().to_bytes(),
            market: found.market().to_bytes(),
            product_record_digest: [0x51; 32],
            product_instance_id: [0x52; 32],
            linked_basis_record_digest: [0x53; 32],
            semantic_basis_id: [0x54; 32],
            founder: found.founder().to_bytes(),
            founding_intent_digest: [0x55; 32],
            aggregate: [0x56; 32],
            position: [0x57; 32],
            admission: [0x58; 32],
            hoard: found.hoard().to_bytes(),
            rent_credit: lock.rent_credit,
            rent_program: lock.rent_program,
            claims_program: claims_program().to_bytes(),
            trading_program: trading().to_bytes(),
            funding_source: found.funding_source().to_bytes(),
            custody_replay: found.projected_replay().to_bytes(),
            custody_request_digest: hash(lock_raw).to_bytes(),
            custody_receipt_digest: [0x59; 32],
            generation: found.generation(),
            claim_count: 4,
            quantity: found.quantity(),
            basis_scale: found.basis_scale(),
            pre_source_amount: collateral,
            post_source_amount: 0,
            pre_hoard_amount: 0,
            post_hoard_amount: collateral,
            pre_custody_revision: 3,
            post_custody_revision: 4,
            aggregate_rent_principal: 61,
            position_rent_principal: 62,
            admission_rent_principal: 63,
            observed_aggregate_lamports: 61,
            observed_position_lamports: 62,
            observed_admission_lamports: 63,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("claims request")
    }

    #[test]
    fn request_join_binds_every_claims_coordinate_to_the_selected_found_and_lock() {
        let found = found();
        let lock = lock_request();
        let lock_raw = lock.encode().expect("lock bytes");
        let realize = realize_request(&lock);
        let claims = claims_request(&lock, &lock_raw);
        let join = |claims: &ClaimsFoundingRequestV5| {
            authenticate_request_coordinates(
                &trading(),
                &core(),
                &claims_program(),
                &found,
                &lock,
                &realize,
                claims,
                &lock_raw,
            )
        };
        assert_eq!(join(&claims), Ok(()));

        // A substituted Claims request account is the outer's sharpest hostile
        // input: it is readonly, unsigned, and byte-identical in width. Every
        // coordinate it could move must refuse before the first CPI.
        let substituted = |mutate: &dyn Fn(&mut ClaimsFoundingRequestInputV5)| {
            let mut input = claims.input();
            mutate(&mut input);
            ClaimsFoundingRequestV5::new(input).expect("substituted claims request")
        };
        use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestInputV5;
        for mutate in [
            &|input: &mut ClaimsFoundingRequestInputV5| input.market = [0x71; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.founder = [0x72; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.hoard = [0x73; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.funding_source = [0x74; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.custody_replay = [0x75; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.rent_credit = [0x76; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.release_set = [0x77; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.custody_request_digest = [0x78; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.generation += 1,
        ] as [&dyn Fn(&mut ClaimsFoundingRequestInputV5); 9]
        {
            assert_eq!(
                join(&substituted(mutate)),
                Err(TradingSbfError::Content.into())
            );
        }

        // A Claims request naming a different Claims or Trading program refuses
        // at the release boundary rather than the content boundary.
        for mutate in [
            &|input: &mut ClaimsFoundingRequestInputV5| input.claims_program = [0x79; 32],
            &|input: &mut ClaimsFoundingRequestInputV5| input.trading_program = [0x7a; 32],
        ] as [&dyn Fn(&mut ClaimsFoundingRequestInputV5); 2]
        {
            assert_eq!(
                join(&substituted(mutate)),
                Err(TradingSbfError::Release.into())
            );
        }
    }

    #[test]
    fn request_join_refuses_a_substituted_capability_root_or_lock_coordinate() {
        let found = found();
        let lock = lock_request();
        let lock_raw = lock.encode().expect("lock bytes");
        let realize = realize_request(&lock);
        let claims = claims_request(&lock, &lock_raw);
        let join = |lock: &ProjectedCustodyRequestV1, realize: &ProjectedCustodyRequestV1| {
            authenticate_request_coordinates(
                &trading(),
                &core(),
                &claims_program(),
                &found,
                lock,
                realize,
                &claims,
                &lock.encode().expect("lock bytes"),
            )
        };
        assert_eq!(join(&lock, &realize), Ok(()));

        // Decision 0004 derives the founding capability root and never reads a
        // root account, so the request's coordinate is the only thing binding
        // the Custody signer namespace to that Market. A substituted root must
        // refuse here, before any CPI, and must also move the Lock and Realize
        // caller PDAs so no signature exists for the substituted request.
        let mut rerooted = lock;
        rerooted.parent_capability_root = [0x7b; 32];
        assert_eq!(
            join(&rerooted, &realize_request(&rerooted)),
            Err(TradingSbfError::Content.into())
        );
        assert_ne!(
            ProjectedCustodyCallerSeedsV1::new(
                rerooted,
                hash(&rerooted.encode().expect("bytes")).to_bytes()
            )
            .as_slices(),
            ProjectedCustodyCallerSeedsV1::new(lock, hash(&lock_raw).to_bytes()).as_slices()
        );

        // Every other coordinate the shared Lock join owns.
        for mutate in [
            &|value: &mut ProjectedCustodyRequestV1| value.market = [0x7c; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.release_set = [0x7d; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.context_digest = [0x7e; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.hoard_vault = [0x7f; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.funding_source_vault = [0x80; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.refund_owner = [0x81; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.generation += 1,
            &|value: &mut ProjectedCustodyRequestV1| value.expiry_slot += 1,
            &|value: &mut ProjectedCustodyRequestV1| value.amount += 1,
        ] as [&dyn Fn(&mut ProjectedCustodyRequestV1); 9]
        {
            let mut hostile = lock;
            mutate(&mut hostile);
            assert_eq!(
                join(&hostile, &realize_request(&hostile)),
                Err(TradingSbfError::Content.into())
            );
        }

        // A Lock naming another Trading or Core program refuses at the release
        // boundary, and a Lock that is not the terminal operation refuses too.
        for mutate in [
            &|value: &mut ProjectedCustodyRequestV1| value.caller_program = [0x82; 32],
            &|value: &mut ProjectedCustodyRequestV1| value.core_program = [0x83; 32],
            &|value: &mut ProjectedCustodyRequestV1| {
                value.operation = ProjectedCustodyOperationV1::LockHoard;
            },
        ] as [&dyn Fn(&mut ProjectedCustodyRequestV1); 3]
        {
            let mut hostile = lock;
            mutate(&mut hostile);
            assert_eq!(
                join(&hostile, &realize_request(&hostile)),
                Err(TradingSbfError::Release.into())
            );
        }
    }

    #[test]
    fn request_join_refuses_a_broken_projected_sequence() {
        let found = found();
        let lock = lock_request();
        let lock_raw = lock.encode().expect("lock bytes");
        let claims = claims_request(&lock, &lock_raw);
        let mut realize = realize_request(&lock);
        realize.resulting_revision += 1;
        realize.expected_revision += 1;
        assert_eq!(
            authenticate_request_coordinates(
                &trading(),
                &core(),
                &claims_program(),
                &found,
                &lock,
                &realize,
                &claims,
                &lock_raw,
            ),
            Err(TradingSbfError::Content.into())
        );

        let mut hostile_lock = lock;
        hostile_lock.amount += 1;
        let hostile_raw = hostile_lock.encode().expect("hostile lock bytes");
        assert_eq!(
            authenticate_request_coordinates(
                &trading(),
                &core(),
                &claims_program(),
                &found,
                &hostile_lock,
                &realize_request(&hostile_lock),
                &claims,
                &hostile_raw,
            ),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn frame_width_is_runtime_funding_polymorphic() {
        let count = |funding: usize| {
            GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V1
                + PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1
                + GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
                + funding
                + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1
                + PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1
                + CLAIMS_FOUNDING_ACCOUNT_COUNT_V5
                + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1
        };
        // Decision 0004 removed the capability-root account from both the Found
        // and the Open frame; the root is derived, never read.
        assert_eq!(count(3), 137);
        assert_eq!(count(16), 150);
    }
}
