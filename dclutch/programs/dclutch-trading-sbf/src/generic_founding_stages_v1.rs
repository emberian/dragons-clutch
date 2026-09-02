//! Two-stage generic Market founding: economic atomicity via the Core permit.
//!
//! The composed `DCLTGMF3` route executes Lock→Found→Realize→Claims→Open in
//! one transaction and pays for all five legs under one compute ceiling.
//! Measured at 264ad628 on a local validator, its four child legs alone cost
//! 1,062,298 CU (Lock 121,104 + Found 537,262 + Realize 98,053 + Claims
//! 305,879) plus ~182k of outer self-cost, leaving Core's final Open a
//! 155,281-CU remainder it exhausted. The codec has always defined the split
//! this module dispatches (`GenericFoundingStageV1::{FoundAndPermit, Open}`,
//! with distinct post-resource domains and distinct Trading caller
//! authorities per stage), and Core has always processed both stages; what
//! was missing was a Trading outer for each stage.
//!
//! - `DCLTGFP1` executes Lock→Found→Realize→Claims and stops. Its
//!   transaction commits the Market in `Founding` phase, escrows the
//!   Core-owned one-shot `SeriesFoundingPermitV1`, realizes the collateral
//!   into normal Custody, commits the three Claims accounts, and closes the
//!   consumed `CustodyStaged` controller-funding checkpoint. It is a single
//!   rollback domain: any refusal rolls all four legs back.
//! - `DCLTGMO1` executes the commit-last Core Open alone, consuming the
//!   permit. Its only inputs beyond the selected artifact bytes are live
//!   accounts; the Claims receipt Core demands is rebuilt here from the
//!   claims request bytes and the hashes of the live Claims accounts, and
//!   Core independently verifies that reconstruction against the digest the
//!   permit bound at Found time. Nothing crosses the stage boundary on trust:
//!   the permit is the sole carrier of authority between the stages.
//!
//! # The atomicity property, stated honestly
//!
//! Founding through these two routes is NOT transaction-atomic: between the
//! stages the chain durably holds a `Founding`-phase Market, an escrowed
//! permit, and realized collateral. It is *economically* atomic via the
//! permit:
//!
//! - every persisted economic coordinate of the Open — market, release set,
//!   founder, context, capability root, funding source, hoard, quantity,
//!   basis scale, expiry, rent-credit destination — is pinned by the permit
//!   Core escrowed inside the stage-1 rollback domain, so no submitter of
//!   stage 2 can steer the outcome anywhere the founder did not already
//!   commit to;
//! - before `expiry_slot`, anyone may complete stage 2 (submission is
//!   permissionless because it is effect-free beyond the pinned outcome);
//! - after `expiry_slot`, stage 2 refuses (`CoreSbfError::Reference`) and
//!   the expiry/refund family becomes the sole consumer of the escrow, so
//!   value cannot strand between the stages.
//!
//! Exactly one abort family is armed at every instant: before stage 1 the
//! controller-funding checkpoint owns cleanup (its custody-ladder digest
//! binds the pre-Lock prestate, which is why stage 1 — the transaction that
//! consumes that prestate — must be the one that closes it); from stage-1
//! commit onward the permit owns it. Stage 1 swaps one for the other inside
//! a single rollback domain, so there is no instant with two armed abort
//! paths and no instant with none.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_ACCOUNT_COUNT_V5, CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
    CLAIMS_FOUNDING_REQUEST_BYTES_V5, ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
};
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1,
};
use dclutch_market_core_codec::{
    GENERIC_FOUNDING_ACK_BYTES_V1, GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1, GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1, GENERIC_FOUNDING_REQUEST_BYTES_V1,
    GenericFoundingAckV1, GenericFoundingRequestV1, GenericFoundingStageV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;
use crate::generic_market_founding_v1::{
    CLAIMS_RAW, FOUND_RAW, GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V3,
    GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3, GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V3,
    GenericFoundingFrameV1, LOCK_RAW, REALIZE_RAW, account, authenticate_found_to_claims,
    authenticate_instructions_sysvar_v1, authenticate_raw_accounts, authenticate_realize_receipt,
    authenticate_request_join, authenticate_staged_checkpoint_v1,
    authenticate_unchanged_pending_ledgers_v1, caller_seeds, close_open_consumed_checkpoint_v1,
    decode_claims_request, decode_found_request, decode_projected_request, execute_claims,
    execute_core_found, execute_lock, execute_realize, invoke_child, raw_account_bytes,
    role_caller_from_bump_v3, subslice,
};

/// Stage-1 outer: Lock→Found→Realize→Claims, permit escrowed, no Open.
pub const GENERIC_FOUND_AND_PERMIT_MAGIC_V1: [u8; 8] = *b"DCLTGFP1";
/// Exact number of stage-1 invocation-scoped child-authority bumps.
pub const GENERIC_FOUND_AND_PERMIT_CALLER_BUMP_COUNT_V1: usize = 4;
/// Exact stage-1 outer instruction width.
pub const GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1: usize =
    8 + GENERIC_FOUND_AND_PERMIT_CALLER_BUMP_COUNT_V1;

/// Stage-2 outer: the commit-last Core Open, consuming the permit.
pub const GENERIC_MARKET_OPEN_MAGIC_V1: [u8; 8] = *b"DCLTGMO1";
/// Exact stage-2 outer instruction width: the magic and one caller bump.
pub const GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1: usize = 8 + 1;
/// Exact stage-2 readonly raw-request prefix width: Found artifact, Claims request.
pub const GENERIC_MARKET_OPEN_RAW_ACCOUNT_COUNT_V1: usize = 2;
/// Exact stage-2 outer account count: two raws and Core's Open window.
pub const GENERIC_MARKET_OPEN_ACCOUNT_COUNT_V1: usize =
    GENERIC_MARKET_OPEN_RAW_ACCOUNT_COUNT_V1 + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1;

const STAGE1_LOCK_CALLER_BUMP_INDEX_V1: usize = 0;
const STAGE1_FOUND_CALLER_BUMP_INDEX_V1: usize = 1;
const STAGE1_REALIZE_CALLER_BUMP_INDEX_V1: usize = 2;
const STAGE1_CLAIMS_CALLER_BUMP_INDEX_V1: usize = 3;

// Core's `GenericOpenFrame::parse` order (`core-sbf/generic_founding_v1.rs`).
// These indexes are relative to the Open window this outer forwards whole, so
// the only cross-program agreement they need is the count, which both sides
// read from `GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1`; every semantic binding
// below them is re-authenticated by Core against the permit and live state.
const OPEN_CALLER: usize = 0;
const OPEN_MARKET: usize = 1;
const OPEN_PERMIT: usize = 2;
const OPEN_RENT_CREDIT: usize = 3;
const OPEN_TRADING_PROGRAM: usize = 7;
const OPEN_CLAIMS_PROGRAM: usize = 9;
const OPEN_CORE_PROGRAM: usize = 13;
const OPEN_AGGREGATE: usize = 18;
const OPEN_POSITION: usize = 19;
const OPEN_ADMISSION: usize = 20;

/// Writable Open-window indices: market, permit, rent credit.
const OPEN_WRITABLE: [usize; 3] = [OPEN_MARKET, OPEN_PERMIT, OPEN_RENT_CREDIT];

/// Stage-1 invocation evidence for the four child authorities, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GenericFoundAndPermitCallerBumpsV1 {
    values: [u8; GENERIC_FOUND_AND_PERMIT_CALLER_BUMP_COUNT_V1],
}

impl GenericFoundAndPermitCallerBumpsV1 {
    fn decode(instruction_data: &[u8]) -> Result<Self, ProgramError> {
        if instruction_data.len() != GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1
            || instruction_data.get(..8) != Some(GENERIC_FOUND_AND_PERMIT_MAGIC_V1.as_slice())
        {
            return Err(TradingSbfError::UnsupportedContent.into());
        }
        let values = instruction_data
            .get(8..)
            .ok_or(TradingSbfError::UnsupportedContent)?
            .try_into()
            .map_err(|_| TradingSbfError::UnsupportedContent)?;
        Ok(Self { values })
    }

    const fn lock(self) -> u8 {
        self.values[STAGE1_LOCK_CALLER_BUMP_INDEX_V1]
    }

    const fn found(self) -> u8 {
        self.values[STAGE1_FOUND_CALLER_BUMP_INDEX_V1]
    }

    const fn realize(self) -> u8 {
        self.values[STAGE1_REALIZE_CALLER_BUMP_INDEX_V1]
    }

    const fn claims(self) -> u8 {
        self.values[STAGE1_CLAIMS_CALLER_BUMP_INDEX_V1]
    }
}

/// Return whether bytes select the stage-1 Found-and-permit outer.
#[must_use]
pub fn is_generic_found_and_permit_v1(instruction_data: &[u8]) -> bool {
    GenericFoundAndPermitCallerBumpsV1::decode(instruction_data).is_ok()
}

/// Return whether bytes select the stage-2 Market-Open outer.
#[must_use]
pub fn is_generic_market_open_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1
        && instruction_data.get(..8) == Some(GENERIC_MARKET_OPEN_MAGIC_V1.as_slice())
}

/// Execute Lock→Found→Realize→Claims as one rollback domain, permit escrowed.
///
/// The wire is the `DCLTGMF3` frame with the 21-account Open window and the
/// Open caller bump removed: the same four readonly raw requests, the same
/// instructions sysvar (this route is on the extended-heap list for the same
/// reason the composed route is), the same Lock/Found/Realize/Claims windows,
/// and the checkpoint last. The staged `CustodyStaged` checkpoint is
/// authenticated against the pre-Lock prestate before the first CPI and
/// closed after the Claims receipt and unchanged Pending controller ledgers
/// are proven — from this transaction's commit onward the escrowed permit,
/// not the checkpoint, is the sole abort authority for this founding.
#[inline(never)]
pub fn process_generic_found_and_permit_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let caller_bumps = GenericFoundAndPermitCallerBumpsV1::decode(instruction_data)?;
    let found_raw = raw_account_bytes(accounts, FOUND_RAW, GENERIC_FOUNDING_REQUEST_BYTES_V1)?;
    let found = decode_found_request(&found_raw)?;
    let frame = parse_found_and_permit_frame(accounts, usize::from(found.funding_count()))?;
    let lock_raw = raw_account_bytes(accounts, LOCK_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let realize_raw = raw_account_bytes(accounts, REALIZE_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let claims_raw = raw_account_bytes(accounts, CLAIMS_RAW, CLAIMS_FOUNDING_REQUEST_BYTES_V5)?;
    let lock = decode_projected_request(&lock_raw)?;
    let realize = decode_projected_request(&realize_raw)?;
    let claims = decode_claims_request(&claims_raw)?;
    authenticate_request_join(
        program_id, &frame, &found, &lock, &realize, &claims, &lock_raw,
    )?;
    let staged = authenticate_staged_checkpoint_v1(program_id, &frame, &found, &lock, &lock_raw)?;

    let lock_receipt = execute_lock(program_id, &frame, &lock, &lock_raw, caller_bumps.lock())?;
    let found_ack = execute_core_found(
        program_id,
        &frame,
        &found,
        &found_raw,
        &lock_receipt,
        caller_bumps.found(),
    )?;
    authenticate_found_to_claims(&frame, &found_ack, &claims)?;
    let realize_receipt = execute_realize(
        program_id,
        &frame,
        &realize,
        &realize_raw,
        caller_bumps.realize(),
    )?;
    authenticate_realize_receipt(&frame, &found, &realize, &realize_raw, &realize_receipt)?;
    execute_claims(
        program_id,
        &frame,
        &claims,
        &claims_raw,
        &lock_receipt,
        &realize_receipt,
        caller_bumps.claims(),
    )?;
    authenticate_unchanged_pending_ledgers_v1(&frame, staged)?;
    close_open_consumed_checkpoint_v1(program_id, &frame, staged)?;
    // The Found acknowledgement still binds the live poststate here: the
    // Realize receipt proved the Market bytes unchanged since Found, and the
    // permit is immutable Core-owned data from its creation, so the ack's
    // post-resource digest over (market, permit) remains exact at commit.
    set_return_data(&found_ack);
    Ok(())
}

/// Parse the stage-1 frame: the composed frame without its Open window.
#[inline(never)]
fn parse_found_and_permit_frame<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    funding_count: usize,
) -> Result<GenericFoundingFrameV1<'accounts, 'info>, ProgramError> {
    let found_count = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
        .checked_add(funding_count)
        .and_then(|value| value.checked_add(GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1))
        .ok_or(TradingSbfError::Content)?;
    let lock_start = GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3;
    let found_start = lock_start
        .checked_add(PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let realize_start = found_start
        .checked_add(found_count)
        .ok_or(TradingSbfError::Content)?;
    let claims_start = realize_start
        .checked_add(PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let checkpoint_index = claims_start
        .checked_add(CLAIMS_FOUNDING_ACCOUNT_COUNT_V5)
        .ok_or(TradingSbfError::Content)?;
    let end = checkpoint_index
        .checked_add(1)
        .ok_or(TradingSbfError::Content)?;
    if accounts.len() != end {
        return Err(TradingSbfError::Content.into());
    }
    authenticate_raw_accounts(
        accounts
            .get(..GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V3)
            .ok_or(TradingSbfError::Content)?,
    )?;
    authenticate_instructions_sysvar_v1(account(
        accounts,
        GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V3,
    )?)?;
    Ok(GenericFoundingFrameV1 {
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
        // Stage 1 has no Open window. Nothing on the stage-1 path reads
        // `frame.open`; `execute_core_open` and the ack's Open branch are the
        // only readers and they are unreachable from this route.
        open: subslice(accounts, end, 0)?,
        checkpoint: account(accounts, checkpoint_index)?,
        funding_count,
    })
}

/// Execute the commit-last Core Open, consuming the escrowed permit.
///
/// The submitter supplies no economic truth. The Found artifact and the
/// Claims request are the same content-addressed bytes stage 1 consumed; the
/// Claims receipt Core demands is rebuilt from those bytes plus the hashes
/// of the live Claims accounts, and Core verifies that reconstruction
/// against the request digest the permit bound at Found time and against the
/// live account data itself. Submission is therefore permissionless by
/// construction: every persisted outcome is pinned by the permit, and the
/// permit refuses after its own `expiry_slot`, at which point the refund
/// family owns the escrow instead.
#[inline(never)]
pub fn process_generic_market_open_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_generic_market_open_v1(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let open_bump = *instruction_data
        .get(8)
        .ok_or(TradingSbfError::UnsupportedContent)?;
    if accounts.len() != GENERIC_MARKET_OPEN_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let found_raw = raw_account_bytes(accounts, 0, GENERIC_FOUNDING_REQUEST_BYTES_V1)?;
    let claims_raw = raw_account_bytes(accounts, 1, CLAIMS_FOUNDING_REQUEST_BYTES_V5)?;
    if account(accounts, 0)?.key == account(accounts, 1)?.key {
        return Err(TradingSbfError::Content.into());
    }
    // The artifact is always the selected Found-and-permit bytes; the Open
    // request is derived, exactly as the composed route derives it. Feeding
    // an Open-stage encoding as the artifact refuses here.
    let found = decode_found_request(&found_raw)?;
    let claims = decode_claims_request(&claims_raw)?;
    let open_window = subslice(
        accounts,
        GENERIC_MARKET_OPEN_RAW_ACCOUNT_COUNT_V1,
        GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
    )?;
    authenticate_open_join_v1(program_id, open_window, &found, &claims)?;
    let claims_receipt = rebuild_claims_receipt_v1(open_window, &claims, &claims_raw)?;
    let open = found
        .with_stage(GenericFoundingStageV1::Open)
        .map_err(|_| TradingSbfError::Content)?;
    let open_raw = open.encode().map_err(|_| TradingSbfError::Content)?;
    let ack = execute_open_stage_v1(
        program_id,
        open_window,
        &open,
        &open_raw,
        &claims_receipt,
        open_bump,
    )?;
    set_return_data(&ack);
    Ok(())
}

/// Authenticate the stage-2 request join and frame programs before any CPI.
///
/// Core re-authenticates every one of these bindings against the permit and
/// live state; this outer refuses the cheap contradictions first so a
/// malformed submission dies before paying for the CPI.
#[inline(never)]
fn authenticate_open_join_v1(
    program_id: &Pubkey,
    open_window: &[AccountInfo<'_>],
    found: &GenericFoundingRequestV1,
    claims: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    let trading = account(open_window, OPEN_TRADING_PROGRAM)?;
    let claims_program = account(open_window, OPEN_CLAIMS_PROGRAM)?;
    let core = account(open_window, OPEN_CORE_PROGRAM)?;
    if !trading.executable
        || !claims_program.executable
        || !core.executable
        || trading.key != program_id
    {
        return Err(TradingSbfError::Release.into());
    }
    let market = account(open_window, OPEN_MARKET)?;
    if claims.claims_program() != claims_program.key.to_bytes()
        || claims.trading_program() != program_id.to_bytes()
        || claims.release_set() != found.release_set().to_bytes()
        || claims.market() != found.market().to_bytes()
        || claims.founder() != found.founder().to_bytes()
        || claims.funding_source() != found.funding_source().to_bytes()
        || claims.hoard() != found.hoard().to_bytes()
        || claims.custody_replay() != found.projected_replay().to_bytes()
        || claims.generation() != found.generation()
        || claims.quantity() != found.quantity()
        || claims.basis_scale() != found.basis_scale()
        || market.key.to_bytes() != found.market().to_bytes()
        // The market this stage opens must already be the one stage 1 created
        // and Core owns. A stage-2 submission that arrives before stage 1 finds
        // a still-vacant, system-owned market here and refuses fast at this
        // named check, rather than paying for a CPI only to have Core's Market
        // authenticator refuse the same fact behind a remapped code.
        || market.owner != core.key
        || claims.aggregate() != account(open_window, OPEN_AGGREGATE)?.key.to_bytes()
        || claims.position() != account(open_window, OPEN_POSITION)?.key.to_bytes()
        || claims.admission() != account(open_window, OPEN_ADMISSION)?.key.to_bytes()
        || claims.rent_credit() != account(open_window, OPEN_RENT_CREDIT)?.key.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Rebuild the exact Claims founding receipt from live poststate.
///
/// `ClaimsFoundingReceiptV5` is a pure projection: the accepted request, the
/// digest of its bytes, and the digests of the three Claims accounts it
/// committed. Every field is recomputable from the readonly claims-request
/// account and the live Claims accounts in the Open window, so the receipt
/// carries no authority of its own here — Core requires the embedded request
/// digest to equal the one the permit bound at Found time and requires each
/// account digest to equal the hash of the account's live bytes.
#[inline(never)]
fn rebuild_claims_receipt_v1(
    open_window: &[AccountInfo<'_>],
    claims: &ClaimsFoundingRequestV5,
    claims_raw: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    let aggregate = account(open_window, OPEN_AGGREGATE)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let position = account(open_window, OPEN_POSITION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let admission = account(open_window, OPEN_ADMISSION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let combined = hashv(&[
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
        &aggregate,
        &position,
        &admission,
    ])
    .to_bytes();
    let receipt = ClaimsFoundingReceiptV5::new(
        *claims,
        hash(claims_raw).to_bytes(),
        hash(&aggregate).to_bytes(),
        hash(&position).to_bytes(),
        hash(&admission).to_bytes(),
        combined,
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(receipt.to_bytes().to_vec())
}

/// Invoke Core's Open stage and authenticate its acknowledgement.
#[inline(never)]
fn execute_open_stage_v1(
    program_id: &Pubkey,
    open_window: &[AccountInfo<'_>],
    open: &GenericFoundingRequestV1,
    open_raw: &[u8],
    claims_receipt: &[u8],
    bump: u8,
) -> Result<Vec<u8>, ProgramError> {
    let core_program = account(open_window, OPEN_CORE_PROGRAM)?;
    let digest = hash(open_raw).to_bytes();
    let seeds = caller_seeds(open, digest)?;
    let caller = role_caller_from_bump_v3(&seeds, program_id, bump)?;
    if account(open_window, OPEN_CALLER)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut child_data = Vec::with_capacity(
        open_raw
            .len()
            .checked_add(claims_receipt.len())
            .ok_or(TradingSbfError::Content)?,
    );
    child_data.extend_from_slice(open_raw);
    child_data.extend_from_slice(claims_receipt);
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest] = seeds.as_slices();
    let returned = invoke_child(
        core_program,
        open_window,
        &child_data,
        &OPEN_WRITABLE,
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
    authenticate_open_ack_v1(open_window, open, open_raw, claims_receipt, &returned)?;
    Ok(returned)
}

/// Authenticate the Open acknowledgement against the frame and poststate.
///
/// Mirrors the composed route's Open-branch acknowledgement check, reading
/// the Market and permit from the Open window because this route carries no
/// Found window.
#[inline(never)]
fn authenticate_open_ack_v1(
    open_window: &[AccountInfo<'_>],
    open: &GenericFoundingRequestV1,
    open_raw: &[u8],
    claims_receipt: &[u8],
    returned: &[u8],
) -> Result<(), ProgramError> {
    if returned.len() != GENERIC_FOUNDING_ACK_BYTES_V1 {
        return Err(TradingSbfError::Transition.into());
    }
    let ack = GenericFoundingAckV1::decode(returned).map_err(|_| TradingSbfError::ChildReceipt)?;
    let market_data = account(open_window, OPEN_MARKET)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let post = hashv(&[
        GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1,
        &market_data,
        claims_receipt,
    ])
    .to_bytes();
    drop(market_data);
    if ack.stage() != GenericFoundingStageV1::Open
        || ack.funding_count() != open.funding_count()
        || ack.core_program().to_bytes() != account(open_window, OPEN_CORE_PROGRAM)?.key.to_bytes()
        || ack.release_set() != open.release_set()
        || ack.market() != open.market()
        || ack.permit().to_bytes() != account(open_window, OPEN_PERMIT)?.key.to_bytes()
        || ack.request_digest().to_bytes() != hash(open_raw).to_bytes()
        || ack.funding_list_id() != open.funding_list_id()
        || ack.generation() != open.generation()
        || ack.post_resource_digest().to_bytes() != post
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage1_wire_decodes_only_its_exact_magic_and_width() {
        let mut data = [0_u8; GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1];
        data[..8].copy_from_slice(&GENERIC_FOUND_AND_PERMIT_MAGIC_V1);
        data[8..].copy_from_slice(&[251, 252, 253, 254]);
        let bumps = GenericFoundAndPermitCallerBumpsV1::decode(&data).expect("decode");
        assert_eq!(bumps.lock(), 251);
        assert_eq!(bumps.found(), 252);
        assert_eq!(bumps.realize(), 253);
        assert_eq!(bumps.claims(), 254);
        assert!(is_generic_found_and_permit_v1(&data));

        let mut wrong_magic = data;
        wrong_magic[7] = wrong_magic[7].wrapping_add(1);
        assert!(!is_generic_found_and_permit_v1(&wrong_magic));
        assert!(!is_generic_found_and_permit_v1(&data[..data.len() - 1]));
        let mut wide = data.to_vec();
        wide.push(0);
        assert!(!is_generic_found_and_permit_v1(&wide));
        // The composed route's five-bump wire is not this route's wire.
        let mut composed =
            [0_u8; crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3];
        composed[..8]
            .copy_from_slice(&crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_MAGIC_V3);
        assert!(!is_generic_found_and_permit_v1(&composed));
    }

    #[test]
    fn stage2_wire_decodes_only_its_exact_magic_and_width() {
        let mut data = [0_u8; GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1];
        data[..8].copy_from_slice(&GENERIC_MARKET_OPEN_MAGIC_V1);
        data[8] = 255;
        assert!(is_generic_market_open_v1(&data));
        let mut wrong_magic = data;
        wrong_magic[0] = wrong_magic[0].wrapping_add(1);
        assert!(!is_generic_market_open_v1(&wrong_magic));
        assert!(!is_generic_market_open_v1(&data[..8]));
        let mut wide = data.to_vec();
        wide.push(0);
        assert!(!is_generic_market_open_v1(&wide));
    }

    #[test]
    fn the_three_outer_magics_are_pairwise_distinct() {
        let magics = [
            GENERIC_FOUND_AND_PERMIT_MAGIC_V1,
            GENERIC_MARKET_OPEN_MAGIC_V1,
            crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_MAGIC_V3,
        ];
        for (i, a) in magics.iter().enumerate() {
            for b in magics.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn stage2_frame_width_is_the_raw_prefix_plus_the_codec_open_count() {
        // Core sizes its Open frame from the same codec constant, so the two
        // programs cannot drift about the window width.
        assert_eq!(GENERIC_MARKET_OPEN_ACCOUNT_COUNT_V1, 2 + 21);
        assert_eq!(
            GENERIC_MARKET_OPEN_ACCOUNT_COUNT_V1,
            GENERIC_MARKET_OPEN_RAW_ACCOUNT_COUNT_V1 + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1
        );
        assert!(OPEN_ADMISSION < GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1);
    }
}
