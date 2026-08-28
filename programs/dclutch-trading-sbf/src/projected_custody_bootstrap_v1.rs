//! Family-neutral creation of the projected-Custody prestate a founding needs.
//!
//! Custody's `Initialize` and `OpenHoard` each require a signing
//! `ProjectedCustodyCallerSeedsV1` PDA derived under the Trading program, so no
//! wallet can drive them and only a Trading CPI can. Until this route existed
//! the sole in-tree constructor of those two requests was Series-shaped and had
//! no non-test caller, which left the atomic founding outer's Lock stage
//! demanding a `HoardOpen` replay that nothing could create.
//!
//! This route creates exactly that prestate and nothing else. It is bound to
//! one terminal `LockHoardAndCloseSource` request and one founding artifact,
//! and it authenticates their join with the same predicate the founding outer
//! uses, so a replay this route creates is admissible at Lock by construction
//! rather than by two constructors agreeing. Both transitions run in one
//! rollback domain: a Market is never left with a replay but no Hoard.
//!
//! No family, escrow shape, or ticket namespace enters. Every coordinate is
//! carried from the terminal request by
//! [`ProjectedCustodyRequestV1::founding_prestate_v1`], which varies exactly
//! the four transition fields Custody permits a successor to vary.
//!
//! Child CPI metas are built from this route's own authenticated frame. This is
//! a direct instruction, not an Effect-V3 route adapter, so it never consults a
//! downgraded privilege view.
//!
//! One readonly account follows the two raw requests: the instructions sysvar.
//! Four stages allocate from one bump allocator that never frees, so this route
//! is on `entrypoint_adapter::declares_extended_heap_profile_v1`'s list and
//! runs on a runtime-granted heap frame. The adapter re-derives that grant from
//! the sysvar the runtime itself serialized, and it looks for it in this
//! instruction's own account list, so the slot is part of the wire.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId, FundingLedgerStatusV2,
    FundingLedgerV2, funding_ledger_bytes_v2, validate_funding_ledger_masks_v2,
};
use dclutch_custody_contract::{
    FoundingPrestateStageV1, INITIALIZE_RESULTING_REVISION_V1, OPEN_HOARD_RESULTING_REVISION_V1,
    OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1, PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1,
    PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V2, PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1,
    PROJECTED_CUSTODY_OPEN_SOURCE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_REQUEST_BYTES_V1,
    PROJECTED_CUSTODY_STATE_BYTES_V2, ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1, ProjectedCustodyStateV2,
};
use dclutch_market_core_codec::{
    Action, FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3, GENERIC_FOUNDING_REQUEST_BYTES_V1,
    GenericFoundingRequestV1, GenericFoundingStageV1, Identity, ProjectFoundRequestV2, Request,
    generic_founding_funding_list_id_v1,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PRE_MARKET_FUNDING_RECEIPT_BYTES_V1, PreMarketFundingReceiptV1, PreMarketFundingRequestV1,
    pre_market_funding_prestate_digest_v1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::TradingSbfError;
use crate::generic_market_founding_v1::{
    authenticate_instructions_sysvar_v1, authenticate_projected_lock_join_v1,
};

/// Sole top-level projected-Custody and controller-ledger bootstrap instruction.
pub const PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2: [u8; 8] = *b"DCLTPCB2";
/// Exact outer instruction width. All economic bytes live in readonly accounts.
pub const PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTION_BYTES_V2: usize = 8;
/// Exact readonly raw-request prefix width.
pub const PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V2: usize = 2;

const FOUND_RAW: usize = 0;
const LOCK_RAW: usize = 1;

/// Index of the instructions sysvar this route presents to its own entrypoint.
///
/// `328fead` measured this route dying out of memory entering its third stage
/// with sixty percent of the compute budget unspent, and concluded that either
/// it allocates less or the program supplies its own allocator over the
/// runtime-granted heap. `entrypoint_adapter` is that allocator, and it
/// re-derives the grant from the instructions sysvar rather than taking any
/// caller's word for it — scanning **this instruction's own account list** to
/// find it. A frame that does not present it keeps the 32 KiB ceiling and dies
/// exactly where it died before, so the slot is part of the route's wire.
pub const PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTIONS_SYSVAR_INDEX_V2: usize =
    PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V2;

const CUSTODY_PROGRAM: usize = PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTIONS_SYSVAR_INDEX_V2 + 1;
const INITIALIZE_START: usize = CUSTODY_PROGRAM + 1;

/// Exact common frame width before the controller-ledger suffix.
pub const PROJECTED_CUSTODY_BOOTSTRAP_COMMON_ACCOUNT_COUNT_V2: usize = INITIALIZE_START
    + PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V2
    + PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1
    + PROJECTED_CUSTODY_OPEN_SOURCE_ACCOUNT_COUNT_V1;
const RESOLUTION_PROGRAM: usize = PROJECTED_CUSTODY_BOOTSTRAP_COMMON_ACCOUNT_COUNT_V2;
const RESOLUTION_PROGRAMDATA: usize = RESOLUTION_PROGRAM + 1;
const PRE_MARKET_CALLER_AUTHORITY: usize = RESOLUTION_PROGRAMDATA + 1;
const RESOLUTION_LEDGER: usize = PRE_MARKET_CALLER_AUTHORITY + 1;
const TRADING_LEDGER: usize = RESOLUTION_LEDGER + 1;

/// Exact total DCLTPCB2 frame: common Custody ladder plus five funding accounts.
pub const PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V2: usize = TRADING_LEDGER + 1;

// Indices shared by every projected-Custody physical frame.
const COMMON_CALLER: usize = 0;
const COMMON_STATE: usize = 1;
const COMMON_CACHE: usize = 2;
const COMMON_REGISTRY: usize = 3;
const COMMON_CALLER_PROGRAM: usize = 4;
const COMMON_CALLER_PROGRAMDATA: usize = 5;
const COMMON_RENT_CREDIT: usize = 6;

// Initialize-specific indices.
const INITIALIZE_CORE_PROGRAM: usize = 7;
const INITIALIZE_PAYER: usize = 8;

// Index of the Market-selected capability manifest raw record inside the Core
// `ProjectFound` sub-frame the Initialize stage forwards. Reusing that exact
// account is what binds the funding states to the manifest Core authenticated,
// rather than to one the caller supplies a second time.
const INITIALIZE_FOUND_START: usize = 11;
const INITIALIZE_RENT: usize = 9;
const INITIALIZE_SYSTEM: usize = 10;

// OpenHoard-specific indices.
const OPEN_HOARD_VAULT: usize = 7;
const OPEN_HOARD_PAYER: usize = 11;

// OpenSourceCompartment-specific indices.
const OPEN_SOURCE_VAULT: usize = 7;
const OPEN_SOURCE_REPLAY: usize = 8;
const OPEN_SOURCE_FUNDER: usize = 12;
const OPEN_SOURCE_FUNDER_OWNER: usize = 13;
const OPEN_SOURCE_PAYER: usize = 14;

// Exact per-stage privilege masks. Custody re-derives every address it is
// handed, so these are the outer's own assertion that the frame it was given
// carries the privileges the child will need, refusing here rather than
// opaquely inside the child.
const INITIALIZE_WRITABLE: [usize; 2] = [COMMON_STATE, INITIALIZE_PAYER];
const INITIALIZE_SIGNERS: [usize; 1] = [INITIALIZE_PAYER];
const OPEN_HOARD_WRITABLE: [usize; 3] = [COMMON_STATE, OPEN_HOARD_VAULT, OPEN_HOARD_PAYER];
const OPEN_HOARD_SIGNERS: [usize; 1] = [OPEN_HOARD_PAYER];
const OPEN_SOURCE_WRITABLE: [usize; 5] = [
    COMMON_STATE,
    OPEN_SOURCE_VAULT,
    OPEN_SOURCE_REPLAY,
    OPEN_SOURCE_FUNDER,
    OPEN_SOURCE_PAYER,
];
const OPEN_SOURCE_SIGNERS: [usize; 2] = [OPEN_SOURCE_FUNDER_OWNER, OPEN_SOURCE_PAYER];

/// Sole top-level projected-Custody founding-abort instruction.
pub const PROJECTED_CUSTODY_ABORT_MAGIC_V1: [u8; 8] = *b"DCLTPCA1";
/// Exact outer instruction width. All economic bytes live in a readonly account.
pub const PROJECTED_CUSTODY_ABORT_INSTRUCTION_BYTES_V1: usize = 8;

const ABORT_LOCK_RAW: usize = 0;
const ABORT_CUSTODY_PROGRAM: usize = 1;
const ABORT_SUB_FRAME_START: usize = 2;

/// Exact total physical frame width for one founding-source abort.
pub const PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1: usize =
    ABORT_SUB_FRAME_START + PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1;

// Custody's own abort sub-frame indices, restated so this route can assert the
// privileges the child will need instead of failing opaquely inside it.
const ABORT_SOURCE_VAULT: usize = 7;
const ABORT_SOURCE_REPLAY: usize = 8;
const ABORT_SOURCE_HOARD: usize = 9;
const ABORT_SOURCE_DESTINATION: usize = 10;
const ABORT_SOURCE_REFUND_OWNER: usize = 11;

const ABORT_WRITABLE: [usize; 6] = [
    COMMON_STATE,
    COMMON_RENT_CREDIT,
    ABORT_SOURCE_VAULT,
    ABORT_SOURCE_REPLAY,
    ABORT_SOURCE_HOARD,
    ABORT_SOURCE_DESTINATION,
];
const ABORT_SIGNERS: [usize; 1] = [ABORT_SOURCE_REFUND_OWNER];

/// Return whether bytes select the sole projected-Custody abort route.
#[must_use]
pub fn is_projected_custody_abort_v1(instruction_data: &[u8]) -> bool {
    instruction_data == PROJECTED_CUSTODY_ABORT_MAGIC_V1
}

/// Unwind an expired founding's funded source compartment.
///
/// `OpenSourceCompartment` puts real principal under a projected authority
/// against a Market that does not exist. The only way forward from there is the
/// Lock stage of an atomic founding whose Core Found and Open stages both
/// refuse once `expiry_slot` has passed — so before this route existed, a
/// founder who staged a prestate and did not found in time held collateral that
/// no route could move, ever. This is the way back out.
///
/// It carries **the same 768 bytes the founding's Lock stage carries**, and the
/// abort request is that request with exactly one field changed. Deriving it
/// rather than accepting a second artifact is what stops the unwind from being
/// a separate authority over the same funds.
///
/// **This route deliberately does not require the founding artifact**, which
/// `DCLTPCB1` does. There, the artifact is load-bearing: the bootstrap is
/// creating a prestate and must bind it to the founding it is for. Here the
/// persisted projection is already that binding — Custody's `authenticate_next`
/// compares the request against every coordinate of the request the state was
/// created with — so a Lock request that is not this projection's simply fails
/// to authenticate. Demanding the artifact as well would make reclaiming
/// principal depend on the founder still holding a record they no longer need.
///
/// **It is also deliberately not on the extended-heap list.** One CPI, one
/// request, one sixteen-account sub-frame. A route that does not need the grant
/// does not declare it, which is what keeps that list meaningful.
#[inline(never)]
pub fn process_projected_custody_abort_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_projected_custody_abort_v1(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    if accounts.len() != PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let raw_account = account(accounts, ABORT_LOCK_RAW)?;
    if raw_account.is_signer
        || raw_account.is_writable
        || raw_account.executable
        || raw_account.data_len() != PROJECTED_CUSTODY_REQUEST_BYTES_V1
    {
        return Err(TradingSbfError::Content.into());
    }
    let lock_raw = raw_account
        .try_borrow_data()
        .map(|data| data.to_vec())
        .map_err(|_| TradingSbfError::Content)?;
    // Only the terminal Lock, which is what `decode_projected_request` admits.
    // A prestate request in this slot would name a different cursor and could
    // not authenticate against the projection anyway; refusing here says so.
    let lock = decode_projected_request(&lock_raw)?;
    let custody_program = account(accounts, ABORT_CUSTODY_PROGRAM)?;
    let sub_frame = subslice(
        accounts,
        ABORT_SUB_FRAME_START,
        PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1,
    )?;
    authenticate_abort_programs_v1(program_id, custody_program, sub_frame, &lock)?;
    let abort = lock.founding_source_abort_v1();
    let raw = encode_projected_request_boxed(&abort)?;
    invoke_projected_child(
        program_id,
        custody_program,
        sub_frame,
        &abort,
        raw.as_slice(),
        &ABORT_WRITABLE,
        &ABORT_SIGNERS,
    )
}

/// Authenticate every Program identity the abort route hands a signature to.
#[inline(never)]
fn authenticate_abort_programs_v1(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'_>,
    sub_frame: &[AccountInfo<'_>],
    lock: &ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let registry = account(sub_frame, COMMON_REGISTRY)?;
    let caller_program = account(sub_frame, COMMON_CALLER_PROGRAM)?;
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || !registry.executable
        || !caller_program.executable
        || caller_program.key != program_id
        || caller_program.key.to_bytes() != lock.caller_program
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

/// Return whether bytes select the sole projected-Custody V2 bootstrap route.
#[must_use]
pub fn is_projected_custody_bootstrap_v2(instruction_data: &[u8]) -> bool {
    instruction_data == PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2
}

/// Create the projected replay, Hoard vault, and funded source compartment as
/// one rollback domain.
///
/// Each stage is executed by [`run_stage`], which owns the 768-byte encoded
/// request for the duration of its own call. Keeping those buffers out of this
/// frame is what holds the route inside the SBF verifier's stack budget; the
/// three-stage version overflowed it by four kilobytes when every stage's
/// request, encoding, and prestate coexisted here.
#[inline(never)]
pub fn process_projected_custody_bootstrap_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_projected_custody_bootstrap_v2(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let frame = BootstrapFrameV1::parse(accounts)?;
    let (lock, funding) = authenticate_and_project(program_id, &frame)?;
    let custody_program = frame.custody_program;
    run_stage(
        program_id,
        custody_program,
        frame.initialize,
        &lock,
        FoundingPrestateStageV1::Initialize,
        &INITIALIZE_WRITABLE,
        &INITIALIZE_SIGNERS,
        ProjectedCustodyPhaseV1::Initialized,
        INITIALIZE_RESULTING_REVISION_V1,
        0,
    )?;
    run_stage(
        program_id,
        custody_program,
        frame.open_hoard,
        &lock,
        FoundingPrestateStageV1::OpenHoard,
        &OPEN_HOARD_WRITABLE,
        &OPEN_HOARD_SIGNERS,
        ProjectedCustodyPhaseV1::HoardOpen,
        OPEN_HOARD_RESULTING_REVISION_V1,
        0,
    )?;
    run_stage(
        program_id,
        custody_program,
        frame.open_source,
        &lock,
        FoundingPrestateStageV1::OpenSourceCompartment,
        &OPEN_SOURCE_WRITABLE,
        &OPEN_SOURCE_SIGNERS,
        ProjectedCustodyPhaseV1::SourceFunded,
        OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
        lock.amount,
    )?;
    stage_founding_capability_funding_v2(program_id, &frame, &funding)
}

/// Decode both readonly artifacts, authenticate every program and the founding
/// join, and derive the exact prestate ladder the terminal Lock determines.
///
/// The two raw request bodies and the decoded founding artifact live only in
/// this frame; the caller keeps the boxed terminal Lock request, from which
/// each stage derives its own prestate as it runs, and the founding's
/// commitment to its capability-funding tail.
#[inline(never)]
fn authenticate_and_project(
    program_id: &Pubkey,
    frame: &BootstrapFrameV1<'_, '_>,
) -> Result<(Box<ProjectedCustodyRequestV1>, FoundingFundingFactsV1), ProgramError> {
    let found_raw = frame.raw_bytes(FOUND_RAW, GENERIC_FOUNDING_REQUEST_BYTES_V1)?;
    let lock_raw = frame.raw_bytes(LOCK_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let found = decode_found_request(&found_raw)?;
    let lock = decode_projected_request(&lock_raw)?;
    let core_program = account(frame.initialize, INITIALIZE_CORE_PROGRAM)?;
    authenticate_programs(program_id, frame, core_program, &lock)?;
    // Exactly the join the founding outer evaluates for this pair. Sharing the
    // predicate is what makes the prestate admissible at Lock: the two routes
    // cannot drift into disagreeing about what a founding's Lock request is.
    authenticate_projected_lock_join_v1(program_id, core_program.key, &found, &lock)?;
    let funding = founding_funding_facts_v2(&found)?;
    Ok((lock, funding))
}

/// Project the founding artifact's commitment to its capability-funding tail.
///
/// Its own frame: every `GenericFoundingRequestV1` accessor takes `self` by
/// value, so four of them in one frame is four copies of a four-hundred-byte
/// request against a four-kilobyte budget.
#[inline(never)]
fn founding_funding_facts_v2(
    found: &GenericFoundingRequestV1,
) -> Result<FoundingFundingFactsV1, ProgramError> {
    if found.funding_count() != 2 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(FoundingFundingFactsV1 {
        release_set: found.release_set().to_bytes(),
        market: found.market().to_bytes(),
        generation: found.generation(),
        funding_list_id: found.funding_list_id(),
        capability_entry_index: found.capability_entry_index(),
    })
}

/// The founding artifact's own commitment to its capability-funding tail.
struct FoundingFundingFactsV1 {
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    funding_list_id: Identity,
    capability_entry_index: u16,
}

/// Encode one request onto the heap.
///
/// The seven-hundred-and-sixty-eight-byte encoding and the request it is taken
/// from cannot share a frame with a derivation of the same width.
#[inline(never)]
fn encode_projected_request_boxed(
    request: &ProjectedCustodyRequestV1,
) -> Result<Box<[u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1]>, ProgramError> {
    request
        .encode()
        .map(Box::new)
        .map_err(|_| TradingSbfError::Content.into())
}

/// Execute one projected-Custody prestate transition and join its poststate.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn run_stage<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    lock: &ProjectedCustodyRequestV1,
    stage: FoundingPrestateStageV1,
    writable: &[usize],
    signers: &[usize],
    phase: ProjectedCustodyPhaseV1,
    next_revision: u64,
    locked_amount: u64,
) -> Result<(), ProgramError> {
    // Derived here rather than up front: three prestate requests cannot be
    // materialised together inside one verifier frame, and a stage derived
    // immediately before it executes still refuses an unreachable terminal
    // before any CPI, because the derivation revalidates the whole ladder.
    let request = lock
        .founding_prestate_stage_v1(stage)
        .map_err(|_| TradingSbfError::Content)?;
    let raw = encode_projected_request_boxed(&request)?;
    invoke_projected_child(
        program_id,
        custody_program,
        accounts,
        &request,
        raw.as_slice(),
        writable,
        signers,
    )?;
    authenticate_poststate(
        account(accounts, COMMON_STATE)?,
        custody_program,
        &request,
        raw.as_slice(),
        phase,
        next_revision,
        locked_amount,
    )
}

/// Initialize the canonical Resolution and Trading controller-subset ledgers.
///
/// Resolution owns and initializes its ledger through a release-authenticated
/// CPI. Trading creates only its own ledger. Both are joined back to the exact
/// manifest and the founding artifact's two-key list before this transaction
/// can commit, so neither a partial partition nor a stranded substitute can
/// survive a refusal.
#[inline(never)]
fn stage_founding_capability_funding_v2(
    program_id: &Pubkey,
    frame: &BootstrapFrameV1<'_, '_>,
    facts: &FoundingFundingFactsV1,
) -> Result<(), ProgramError> {
    let manifest_raw = account(
        frame.initialize,
        INITIALIZE_FOUND_START + FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3,
    )?;
    let payer = account(frame.initialize, INITIALIZE_PAYER)?;
    let rent_account = account(frame.initialize, INITIALIZE_RENT)?;
    let system = account(frame.initialize, INITIALIZE_SYSTEM)?;
    let resolution_program = frame.resolution_program;
    let resolution_ledger = frame.resolution_ledger;
    let trading_ledger = frame.trading_ledger;
    if manifest_raw.is_signer
        || manifest_raw.is_writable
        || manifest_raw.executable
        || !payer.is_signer
        || !payer.is_writable
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| TradingSbfError::Content)?;
    let manifest_data = manifest_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| TradingSbfError::Content)?;
    let manifest_id =
        ContentId::new(hash(&manifest_data).to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let cache = account(frame.initialize, COMMON_CACHE)?;
    let registry = account(frame.initialize, COMMON_REGISTRY)?;
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| TradingSbfError::Release)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| TradingSbfError::Release)?;
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| TradingSbfError::Release)?;
    if cache.owner != registry.key
        || resolution.release().program().to_bytes() != resolution_program.key.to_bytes()
        || trading.release().program().to_bytes() != program_id.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    let [resolution_mask, trading_mask] = controller_masks(
        manifest,
        resolution.release().semantic_release_id().to_bytes(),
        facts.capability_entry_index,
    )?;
    let required_union = manifest_required_union(manifest.entry_count())?;
    let ordered_masks = canonical_funding_mask_order_v2(resolution_mask, trading_mask)?;
    validate_funding_ledger_masks_v2(manifest.entry_count(), required_union, &ordered_masks)
        .map_err(|_| TradingSbfError::Content)?;
    let project_found = ProjectFoundRequestV2::new(Request::administrative(
        Action::Found,
        facts.generation,
        Identity::new(facts.market).map_err(|_| TradingSbfError::Content)?,
    ))
    .map_err(|_| TradingSbfError::Content)?;
    initialize_resolution_ledger_v2(
        program_id,
        frame,
        facts,
        project_found,
        manifest,
        manifest_id,
        resolution_mask,
        &rent,
    )?;
    create_trading_ledger_v2(
        program_id,
        trading_ledger,
        payer,
        system,
        manifest,
        manifest_id,
        facts,
        trading_mask,
        &rent,
    )?;
    let list = canonical_funding_list_id_v2(
        resolution_mask,
        resolution_ledger.key,
        trading_mask,
        trading_ledger.key,
    )?;
    if list != facts.funding_list_id {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Reconstruct the founding artifact's physical FundingLedgerV2 order.
///
/// Controller identity is not an ordering convention. The semantic owner in
/// the founding compiler orders each nonempty controller subset by the lowest
/// manifest bit in its authenticated mask. Repeating that rule here keeps the
/// post-CPI join invariant when the selected Trading entry moves to any of the
/// four canonical manifest positions.
fn canonical_funding_list_id_v2(
    resolution_mask: u16,
    resolution_ledger: &Pubkey,
    trading_mask: u16,
    trading_ledger: &Pubkey,
) -> Result<Identity, ProgramError> {
    let ordered_masks = canonical_funding_mask_order_v2(resolution_mask, trading_mask)?;
    let resolution =
        Identity::new(resolution_ledger.to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let trading = Identity::new(trading_ledger.to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let ordered = if ordered_masks[0] == resolution_mask {
        [resolution, trading]
    } else {
        [trading, resolution]
    };
    generic_founding_funding_list_id_v1(&ordered).map_err(|_| TradingSbfError::Content.into())
}

/// Put the two named controller subsets in the canonical physical order.
///
/// FundingLedgerV2's shared validator and the founding artifact both order
/// ledgers by their lowest selected manifest bit. Direct is entry zero in the
/// live compiler today, so fixed controller order is specifically wrong there:
/// `[Resolution=0b1110, Trading=0b0001]` must become `[0b0001, 0b1110]` before
/// partition validation or the honest frame refuses before Resolution's CPI.
fn canonical_funding_mask_order_v2(
    resolution_mask: u16,
    trading_mask: u16,
) -> Result<[u16; 2], ProgramError> {
    if resolution_mask == 0 || trading_mask == 0 {
        return Err(TradingSbfError::Content.into());
    }
    let resolution_bit = resolution_mask.trailing_zeros();
    let trading_bit = trading_mask.trailing_zeros();
    if resolution_bit == trading_bit {
        return Err(TradingSbfError::Content.into());
    }
    Ok(if resolution_bit < trading_bit {
        [resolution_mask, trading_mask]
    } else {
        [trading_mask, resolution_mask]
    })
}

/// Partition the exact four-entry founding manifest by semantic ownership.
///
/// Decision 0003 keeps the generic Trading interpreter semantic release
/// distinct from a capability family's content-addressed release. The one
/// manifest entry selected by the authenticated founding artifact is therefore
/// Trading-owned, while every companion entry must name the activated
/// Resolution controller release exactly. Physical bit positions are derived
/// from that selected index; no ordering convention is authority.
fn controller_masks(
    manifest: CapabilityManifestV1<'_>,
    resolution_semantic_release: [u8; 32],
    trading_entry_index: u16,
) -> Result<[u16; 2], ProgramError> {
    let trading_entry = manifest
        .entry(trading_entry_index)
        .map_err(|_| TradingSbfError::Content)?;
    if trading_entry.release_id().to_bytes() == resolution_semantic_release {
        return Err(TradingSbfError::Content.into());
    }
    let trading_mask = 1_u16
        .checked_shl(u32::from(trading_entry_index))
        .ok_or(TradingSbfError::Content)?;
    let mut resolution_mask = 0_u16;
    for entry_index in 0_u16..manifest.entry_count() {
        if entry_index == trading_entry_index {
            continue;
        }
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| TradingSbfError::Content)?;
        if entry.release_id().to_bytes() != resolution_semantic_release {
            return Err(TradingSbfError::Content.into());
        }
        resolution_mask |= 1_u16
            .checked_shl(u32::from(entry_index))
            .ok_or(TradingSbfError::Content)?;
    }
    if manifest.entry_count() != 4 || resolution_mask.count_ones() != 3 {
        return Err(TradingSbfError::Content.into());
    }
    Ok([resolution_mask, trading_mask])
}

fn manifest_required_union(entry_count: u16) -> Result<u16, ProgramError> {
    if entry_count == 0 || entry_count > 16 {
        return Err(TradingSbfError::Content.into());
    }
    if entry_count == 16 {
        Ok(u16::MAX)
    } else {
        1_u16
            .checked_shl(u32::from(entry_count))
            .and_then(|bound| bound.checked_sub(1))
            .ok_or_else(|| TradingSbfError::Content.into())
    }
}

struct PlannedFundingLedgerV2 {
    address: Pubkey,
    bump: u8,
    bytes: Vec<u8>,
    exact_lamports: u64,
    derivation: CapabilityFundingLedgerDerivationV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedResolutionLedgerPoststateV2 {
    poststate_digest: [u8; 32],
    exact_rent_lamports: u64,
    exact_native_principal: u64,
}

fn plan_funding_ledger_v2(
    controller: &Pubkey,
    manifest: CapabilityManifestV1<'_>,
    manifest_id: ContentId,
    facts: &FoundingFundingFactsV1,
    selected_mask: u16,
    rent: &Rent,
) -> Result<PlannedFundingLedgerV2, ProgramError> {
    let width = funding_ledger_bytes_v2(
        u16::try_from(selected_mask.count_ones()).map_err(|_| TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let mut bytes = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut bytes, manifest_id, manifest, selected_mask)
        .map_err(|_| TradingSbfError::Content)?;
    let ledger = FundingLedgerV2::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .map_err(|_| TradingSbfError::Content)?;
    for entry_index in 0_u16..manifest.entry_count() {
        if selected_mask & (1_u16 << entry_index) != 0
            && manifest
                .entry(entry_index)
                .map_err(|_| TradingSbfError::Content)?
                .funding_quote()
                .realm_collateral()
                .is_some()
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    let exact_rent = rent.minimum_balance(width);
    let exact_native_principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| TradingSbfError::Content)?;
    let exact_lamports = exact_rent
        .checked_add(exact_native_principal)
        .ok_or(TradingSbfError::Content)?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        controller.to_bytes(),
        facts.market,
        facts.generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (address, bump) = Pubkey::find_program_address(&derivation.seed_components(), controller);
    Ok(PlannedFundingLedgerV2 {
        address,
        bump,
        bytes,
        exact_lamports,
        derivation,
    })
}

/// Authenticate the exact initial Resolution ledger the child CPI committed.
///
/// Resolution is the semantic owner of ledger construction and its typed
/// receipt. Reconstructing the full canonical ledger in Trading before that
/// CPI duplicated the child's dominant work and left the 1.4M-CU founding
/// outer without a usable margin. This verifier instead decodes the live
/// poststate once, binds every row to the immutable manifest, requires the
/// exact initial Pending state, re-derives the PDA, and checks physical Rent
/// plus native principal. The caller separately joins these facts to the exact
/// Resolution return-data receipt. Any mismatch returns an error from the
/// outer instruction and rolls the child mutation back atomically.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_resolution_ledger_poststate_v2(
    resolution_program: &Pubkey,
    target: &AccountInfo<'_>,
    manifest: CapabilityManifestV1<'_>,
    manifest_id: ContentId,
    facts: &FoundingFundingFactsV1,
    selected_mask: u16,
    rent: &Rent,
) -> Result<AuthenticatedResolutionLedgerPoststateV2, ProgramError> {
    if target.owner != resolution_program
        || target.is_signer
        || !target.is_writable
        || target.executable
    {
        return Err(TradingSbfError::Transition.into());
    }
    let data = target
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    authenticate_resolution_ledger_poststate_bytes_v2(
        resolution_program,
        target.key,
        target.owner,
        target.lamports(),
        &data,
        manifest,
        manifest_id,
        facts,
        selected_mask,
        rent,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_resolution_ledger_poststate_bytes_v2(
    resolution_program: &Pubkey,
    target_key: &Pubkey,
    target_owner: &Pubkey,
    target_lamports: u64,
    data: &[u8],
    manifest: CapabilityManifestV1<'_>,
    manifest_id: ContentId,
    facts: &FoundingFundingFactsV1,
    selected_mask: u16,
    rent: &Rent,
) -> Result<AuthenticatedResolutionLedgerPoststateV2, ProgramError> {
    if target_owner != resolution_program {
        return Err(TradingSbfError::Transition.into());
    }
    let ledger = FundingLedgerV2::decode(&data).map_err(|_| TradingSbfError::Transition)?;
    if ledger.selected_mask() != selected_mask {
        return Err(TradingSbfError::Transition.into());
    }
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .map_err(|_| TradingSbfError::Transition)?;
    for entry_index in 0_u16..manifest.entry_count() {
        if selected_mask & (1_u16 << entry_index) == 0 {
            continue;
        }
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| TradingSbfError::Transition)?;
        if entry.funding_quote().realm_collateral().is_some() {
            return Err(TradingSbfError::Transition.into());
        }
        let slot = authenticated
            .slot(entry_index)
            .map_err(|_| TradingSbfError::Transition)?;
        if slot.status() != FundingLedgerStatusV2::Pending || slot.activation_slot() != 0 {
            return Err(TradingSbfError::Transition.into());
        }
    }
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        resolution_program.to_bytes(),
        facts.market,
        facts.generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| TradingSbfError::Transition)?;
    if Pubkey::find_program_address(&derivation.seed_components(), resolution_program).0
        != *target_key
    {
        return Err(TradingSbfError::Transition.into());
    }
    let exact_rent_lamports = rent.minimum_balance(data.len());
    let exact_native_principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| TradingSbfError::Transition)?;
    let exact_lamports = exact_rent_lamports
        .checked_add(exact_native_principal)
        .ok_or(TradingSbfError::Transition)?;
    if target_lamports != exact_lamports {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(AuthenticatedResolutionLedgerPoststateV2 {
        poststate_digest: hash(&data).to_bytes(),
        exact_rent_lamports,
        exact_native_principal,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_trading_ledger_v2<'info>(
    program_id: &Pubkey,
    target: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    manifest: CapabilityManifestV1<'_>,
    manifest_id: ContentId,
    facts: &FoundingFundingFactsV1,
    selected_mask: u16,
    rent: &Rent,
) -> Result<(), ProgramError> {
    if target.owner != &system_program::ID
        || target.data_len() != 0
        || target.lamports() != 0
        || target.is_signer
        || !target.is_writable
        || target.executable
        || target.key == payer.key
        || !payer.is_signer
        || !payer.is_writable
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let planned = plan_funding_ledger_v2(
        program_id,
        manifest,
        manifest_id,
        facts,
        selected_mask,
        rent,
    )?;
    if planned.address != *target.key || payer.lamports() < planned.exact_lamports {
        return Err(TradingSbfError::Content.into());
    }
    invoke(
        &transfer(payer.key, target.key, planned.exact_lamports),
        &[payer.clone(), target.clone(), system.clone()],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let bump_seed = [planned.bump];
    let [domain, controller, market, generation, manifest_seed, mask] =
        planned.derivation.seed_components();
    let signer = [
        domain,
        controller,
        market,
        generation,
        manifest_seed,
        mask,
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            target.key,
            u64::try_from(planned.bytes.len()).map_err(|_| TradingSbfError::Content)?,
        ),
        &[target.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    invoke_signed(
        &assign(target.key, program_id),
        &[target.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    if target.owner != program_id
        || target.data_len() != planned.bytes.len()
        || target.lamports() != planned.exact_lamports
    {
        return Err(TradingSbfError::Transition.into());
    }
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Transition)?;
    if data.len() != planned.bytes.len() || data.iter().any(|byte| *byte != 0) {
        return Err(TradingSbfError::Transition.into());
    }
    data.copy_from_slice(&planned.bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn initialize_resolution_ledger_v2<'info>(
    program_id: &Pubkey,
    frame: &BootstrapFrameV1<'_, 'info>,
    facts: &FoundingFundingFactsV1,
    project_found: ProjectFoundRequestV2,
    manifest: CapabilityManifestV1<'_>,
    manifest_id: ContentId,
    selected_mask: u16,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let resolution_program = frame.resolution_program;
    let resolution_programdata = frame.resolution_programdata;
    let caller_program = account(frame.initialize, COMMON_CALLER_PROGRAM)?;
    let caller_programdata = account(frame.initialize, COMMON_CALLER_PROGRAMDATA)?;
    let funding_source = account(frame.initialize, INITIALIZE_PAYER)?;
    let target = frame.resolution_ledger;
    let authority = frame.pre_market_caller_authority;
    if !resolution_program.executable
        || resolution_program.is_signer
        || resolution_program.is_writable
        || caller_program.key != program_id
        || !caller_program.executable
        || caller_programdata.is_signer
        || caller_programdata.is_writable
        || caller_programdata.executable
        || resolution_programdata.is_signer
        || resolution_programdata.is_writable
        || resolution_programdata.executable
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || target.owner != &system_program::ID
        || target.data_len() != 0
        || target.lamports() != 0
        || target.is_signer
        || !target.is_writable
        || target.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let prestate_digest = pre_market_funding_prestate_digest_v1(
        target.key.to_bytes(),
        target.owner.to_bytes(),
        target.lamports(),
        u64::try_from(target.data_len()).map_err(|_| TradingSbfError::Content)?,
    );
    let request = PreMarketFundingRequestV1 {
        project_found,
        manifest: manifest_id.to_bytes(),
        selected_mask,
        funding_source: funding_source.key.to_bytes(),
        ledger: target.key.to_bytes(),
        prestate_digest,
    };
    let request_bytes = request.encode().map_err(|_| TradingSbfError::Content)?;
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        facts.release_set,
        facts.market,
        ExecutionRoleV1::Trading,
        manifest_id.to_bytes(),
        hash(&request_bytes).to_bytes(),
    )
    .map_err(|_| TradingSbfError::Release)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if expected_authority != *authority.key {
        return Err(TradingSbfError::Release.into());
    }
    let found = subslice(frame.initialize, INITIALIZE_FOUND_START, 37)?;
    let mut metas = Vec::with_capacity(44);
    metas.push(AccountMeta::new_readonly(*authority.key, true));
    metas.push(AccountMeta::new_readonly(*caller_program.key, false));
    metas.push(AccountMeta::new_readonly(*caller_programdata.key, false));
    metas.push(AccountMeta::new_readonly(*resolution_program.key, false));
    metas.push(AccountMeta::new_readonly(
        *resolution_programdata.key,
        false,
    ));
    metas.push(AccountMeta::new(*funding_source.key, true));
    metas.push(AccountMeta::new(*target.key, false));
    metas.extend(
        found
            .iter()
            .map(|value| AccountMeta::new_readonly(*value.key, false)),
    );
    let instruction = Instruction {
        program_id: *resolution_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(44);
    infos.extend([
        authority.clone(),
        caller_program.clone(),
        caller_programdata.clone(),
        resolution_program.clone(),
        resolution_programdata.clone(),
        funding_source.clone(),
        target.clone(),
    ]);
    infos.extend_from_slice(found);
    let bump_seed = [bump];
    let [domain, release_set, market, role, context, request_digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release_set,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *resolution_program.key
        || receipt_bytes.len() != PRE_MARKET_FUNDING_RECEIPT_BYTES_V1
    {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt = PreMarketFundingReceiptV1::decode(&receipt_bytes)
        .map_err(|_| TradingSbfError::Transition)?;
    let poststate = authenticate_resolution_ledger_poststate_v2(
        resolution_program.key,
        target,
        manifest,
        manifest_id,
        facts,
        selected_mask,
        rent,
    )?;
    let found_request = project_found
        .found
        .encode()
        .map_err(|_| TradingSbfError::Content)?;
    let expected_receipt = PreMarketFundingReceiptV1 {
        market: facts.market,
        generation: facts.generation,
        manifest: manifest_id.to_bytes(),
        selected_mask,
        ledger: target.key.to_bytes(),
        prestate_digest,
        poststate_digest: poststate.poststate_digest,
        exact_rent_lamports: poststate.exact_rent_lamports,
        exact_native_principal: poststate.exact_native_principal,
        found_request_digest: hash(&found_request).to_bytes(),
        funding_source: funding_source.key.to_bytes(),
        rent_credit: account(found, 2)?.key.to_bytes(),
    };
    if receipt != expected_receipt {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

struct BootstrapFrameV1<'accounts, 'info> {
    raw: &'accounts [AccountInfo<'info>],
    custody_program: &'accounts AccountInfo<'info>,
    initialize: &'accounts [AccountInfo<'info>],
    open_hoard: &'accounts [AccountInfo<'info>],
    open_source: &'accounts [AccountInfo<'info>],
    resolution_program: &'accounts AccountInfo<'info>,
    resolution_programdata: &'accounts AccountInfo<'info>,
    pre_market_caller_authority: &'accounts AccountInfo<'info>,
    resolution_ledger: &'accounts AccountInfo<'info>,
    trading_ledger: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> BootstrapFrameV1<'accounts, 'info> {
    #[inline(never)]
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V2 {
            return Err(TradingSbfError::Content.into());
        }
        let raw = subslice(
            accounts,
            0,
            PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V2,
        )?;
        for (index, value) in raw.iter().enumerate() {
            if value.is_signer
                || value.is_writable
                || value.executable
                || raw
                    .get(..index)
                    .is_some_and(|prior| prior.iter().any(|other| other.key == value.key))
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        authenticate_instructions_sysvar_v1(account(
            accounts,
            PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTIONS_SYSVAR_INDEX_V2,
        )?)?;
        let open_start = INITIALIZE_START
            .checked_add(PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V2)
            .ok_or(TradingSbfError::Content)?;
        let source_start = open_start
            .checked_add(PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1)
            .ok_or(TradingSbfError::Content)?;
        Ok(Self {
            raw,
            custody_program: account(accounts, CUSTODY_PROGRAM)?,
            initialize: subslice(
                accounts,
                INITIALIZE_START,
                PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V2,
            )?,
            open_hoard: subslice(
                accounts,
                open_start,
                PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1,
            )?,
            open_source: subslice(
                accounts,
                source_start,
                PROJECTED_CUSTODY_OPEN_SOURCE_ACCOUNT_COUNT_V1,
            )?,
            resolution_program: account(accounts, RESOLUTION_PROGRAM)?,
            resolution_programdata: account(accounts, RESOLUTION_PROGRAMDATA)?,
            pre_market_caller_authority: account(accounts, PRE_MARKET_CALLER_AUTHORITY)?,
            resolution_ledger: account(accounts, RESOLUTION_LEDGER)?,
            trading_ledger: account(accounts, TRADING_LEDGER)?,
        })
    }

    fn raw_bytes(&self, index: usize, width: usize) -> Result<Vec<u8>, ProgramError> {
        let value = account(self.raw, index)?;
        if value.data_len() != width {
            return Err(TradingSbfError::Content.into());
        }
        value
            .try_borrow_data()
            .map(|data| data.to_vec())
            .map_err(|_| TradingSbfError::Content.into())
    }
}

/// Authenticate every Program identity this route hands a signature to.
///
/// The Custody program is taken from the Market-selected release set the
/// Registry has already activated, never from the caller's word for it, so a
/// substituted program cannot receive a Trading-derived caller signature. Both
/// child frames must name the same activation cache and Registry, so the two
/// transitions cannot be authenticated against different release sets.
#[inline(never)]
fn authenticate_programs(
    program_id: &Pubkey,
    frame: &BootstrapFrameV1<'_, '_>,
    core_program: &AccountInfo<'_>,
    lock: &ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let cache = account(frame.initialize, COMMON_CACHE)?;
    let registry = account(frame.initialize, COMMON_REGISTRY)?;
    let caller_program = account(frame.initialize, COMMON_CALLER_PROGRAM)?;
    if !frame.custody_program.executable
        || frame.custody_program.is_signer
        || frame.custody_program.is_writable
        || !core_program.executable
        || !registry.executable
        || !caller_program.executable
        || caller_program.key != program_id
        || core_program.key.to_bytes() != lock.core_program
        || account(frame.open_hoard, COMMON_CACHE)?.key != cache.key
        || account(frame.open_hoard, COMMON_REGISTRY)?.key != registry.key
        || account(frame.open_hoard, COMMON_CALLER_PROGRAM)?.key != caller_program.key
        || account(frame.open_hoard, COMMON_STATE)?.key
            != account(frame.initialize, COMMON_STATE)?.key
        || account(frame.open_source, COMMON_CACHE)?.key != cache.key
        || account(frame.open_source, COMMON_REGISTRY)?.key != registry.key
        || account(frame.open_source, COMMON_CALLER_PROGRAM)?.key != caller_program.key
        || account(frame.open_source, COMMON_STATE)?.key
            != account(frame.initialize, COMMON_STATE)?.key
    {
        return Err(TradingSbfError::Release.into());
    }
    if cache.key
        != &Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &lock.release_set],
            registry.key,
        )
        .0
        || cache.owner != registry.key
    {
        return Err(TradingSbfError::Release.into());
    }
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .as_bytes()
        != &lock.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &frame.custody_program.key.to_bytes()
        || activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &lock.core_program
        || activated
            .role(ExecutionRoleV1::Trading)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

/// Invoke one projected-Custody transition under its single-use caller PDA.
///
/// Privileges come from this route's own authenticated frame, not from the
/// runtime's view of the incoming accounts: the writable and signer masks are
/// asserted, so a frame that under-privileges an account refuses here instead
/// of failing opaquely inside Custody. Index zero is always the caller PDA and
/// is the only account this program signs for.
#[inline(never)]
fn invoke_projected_child<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
    writable: &[usize],
    signers: &[usize],
) -> Result<(), ProgramError> {
    let digest = hash(raw).to_bytes();
    let seeds = ProjectedCustodyCallerSeedsV1::new(*request, digest);
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(accounts, COMMON_CALLER)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut metas = Vec::with_capacity(accounts.len());
    for (index, value) in accounts.iter().enumerate() {
        let is_writable = writable.contains(&index);
        let is_signer = index == COMMON_CALLER || signers.contains(&index);
        if (is_writable && !value.is_writable)
            || (is_signer && index != COMMON_CALLER && !value.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        metas.push(if is_writable {
            AccountMeta::new(*value.key, is_signer)
        } else {
            AccountMeta::new_readonly(*value.key, is_signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: raw.to_vec(),
    };
    let mut infos = accounts.to_vec();
    infos.push(custody_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, root, context, request_digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market,
            root,
            context,
            request_digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(())
}

/// Join the persisted replay against the exact request that produced it.
///
/// Neither transition returns data, so the persisted state is the receipt. It
/// is read back from the Custody-owned account and required to be exactly the
/// poststate of the request this route just signed.
#[inline(never)]
fn authenticate_poststate(
    state_account: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
    phase: ProjectedCustodyPhaseV1,
    next_revision: u64,
    locked_amount: u64,
) -> Result<(), ProgramError> {
    if state_account.owner != custody_program.key
        || state_account.data_len() != PROJECTED_CUSTODY_STATE_BYTES_V2
    {
        return Err(TradingSbfError::Transition.into());
    }
    let data = state_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let state = ProjectedCustodyStateV2::decode(&data).map_err(|_| TradingSbfError::Transition)?;
    if state.phase != phase
        || state.next_revision != next_revision
        || state.locked_amount != locked_amount
        || state.last_request_digest != hash(raw).to_bytes()
        || state.request != *request
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn decode_found_request(bytes: &[u8]) -> Result<Box<GenericFoundingRequestV1>, ProgramError> {
    let request = GenericFoundingRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    if request.stage() != GenericFoundingStageV1::FoundAndPermit {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(request))
}

fn decode_projected_request(bytes: &[u8]) -> Result<Box<ProjectedCustodyRequestV1>, ProgramError> {
    let request = ProjectedCustodyRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    if request.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(request))
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
    use dclutch_custody_contract::{
        CompartmentV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
    };
    use dclutch_market_core_codec::Identity;
    use solana_program::hash::hashv;

    use super::*;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn trading() -> Pubkey {
        Pubkey::new_from_array([21; 32])
    }

    fn core() -> Pubkey {
        Pubkey::new_from_array([22; 32])
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

    fn lock() -> ProjectedCustodyRequestV1 {
        let found = found();
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            caller_role: dclutch_custody_contract::ProjectedCallerRoleV1::TradingCapability,
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
            funding_source_compartment: CompartmentV1::Settlement,
            mint: [0x39; 32],
            token_program: [0x3a; 32],
            collateral_release: [0x3b; 32],
            expiry_slot: found.expiry_slot(),
            expected_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
            resulting_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1 + 1,
            amount: found.hoard_principal().expect("principal"),
            state_rent_lamports: 41,
            vault_rent_lamports: 42,
            funding_source_replay_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
            funding_source_state_rent_lamports: 44,
            funding_source_vault_rent_lamports: 45,
        }
    }

    #[test]
    fn bootstrap_abi_is_data_account_only_and_frame_width_is_fixed() {
        assert!(is_projected_custody_bootstrap_v2(
            &PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2
        ));
        assert!(!is_projected_custody_bootstrap_v2(&[0; 8]));
        // A prefix of the tag is not the tag: the route carries no payload.
        assert!(!is_projected_custody_bootstrap_v2(&[
            b'D', b'C', b'L', b'T', b'P', b'C', b'B', b'2', 0
        ]));
        assert_eq!(PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTION_BYTES_V2, 8);
        assert_eq!(PROJECTED_CUSTODY_BOOTSTRAP_COMMON_ACCOUNT_COUNT_V2, 85);
        assert_eq!(PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V2, 90);
        assert_eq!(
            PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTIONS_SYSVAR_INDEX_V2,
            PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V2
        );
    }

    #[test]
    fn only_the_terminal_lock_request_is_admitted() {
        let lock = lock();
        assert_eq!(
            decode_projected_request(&lock.encode().expect("bytes"))
                .expect("terminal")
                .operation,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource
        );
        let mut open = lock;
        open.operation = ProjectedCustodyOperationV1::OpenHoard;
        open.expected_revision = INITIALIZE_RESULTING_REVISION_V1;
        open.resulting_revision = OPEN_HOARD_RESULTING_REVISION_V1;
        open.amount = 0;
        assert_eq!(
            decode_projected_request(&open.encode().expect("bytes")).err(),
            Some(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn the_bootstrap_evaluates_the_founding_outers_own_lock_join() {
        let found = found();
        let lock = lock();
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &lock),
            Ok(())
        );
        // The derived root is the only thing binding this replay's Custody
        // signer namespace to that Market, so a substituted one must refuse
        // before the bootstrap creates any state.
        let mut rerooted = lock;
        rerooted.parent_capability_root = [0x7b; 32];
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &rerooted),
            Err(TradingSbfError::Content.into())
        );
        // So must a substituted Hoard vault, which is the account OpenHoard
        // creates and the account the Lock stage later credits.
        let mut rehoarded = lock;
        rehoarded.hoard_vault = [0x7c; 32];
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &rehoarded),
            Err(TradingSbfError::Content.into())
        );
    }

    /// The route's two named hostile inputs, at the level they are decidable
    /// without a validator.
    ///
    /// A substituted caller-seeds account and a substituted Hoard vault are both
    /// refused twice over, and the second refusal is the one that matters: the
    /// caller authority is derived from the exact request bytes, so substituting
    /// any coordinate moves the signer. There is no signature in existence for a
    /// hostile request, so the CPI could not be made even if the frame check
    /// were bypassed. This is a pure derivation argument, not execution
    /// evidence; the on-chain rollback case still waits on a runnable outer.
    #[test]
    fn a_substituted_caller_or_hoard_vault_has_no_signature_in_existence() {
        let honest = lock();
        let caller = |request: ProjectedCustodyRequestV1| {
            let raw = request.encode().expect("bytes");
            Pubkey::find_program_address(
                &ProjectedCustodyCallerSeedsV1::new(request, hash(&raw).to_bytes()).as_slices(),
                &trading(),
            )
            .0
        };
        let honest_prestate = honest.founding_prestate_v1().expect("prestate");

        let mut rehoarded = honest;
        rehoarded.hoard_vault = [0x7c; 32];
        // Refusal one: the shared founding join owns the Hoard coordinate.
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found(), &rehoarded),
            Err(TradingSbfError::Content.into())
        );
        // Refusal two: even reached, both of its prestates need signers that do
        // not exist, because the vault is a caller-seed input through the
        // request digest.
        let hostile_prestate = rehoarded.founding_prestate_v1().expect("hostile prestate");
        assert_ne!(
            caller(hostile_prestate.initialize),
            caller(honest_prestate.initialize)
        );
        assert_ne!(
            caller(hostile_prestate.open_hoard),
            caller(honest_prestate.open_hoard)
        );
        assert_ne!(
            caller(hostile_prestate.open_source),
            caller(honest_prestate.open_source)
        );

        // Substituting the caller-seeds account directly is the same argument
        // read the other way: the route requires frame index zero to equal the
        // address derived from the request it is about to send, so any other
        // key refuses before the CPI, and the runtime would refuse the
        // signature after it.
        for hostile in [
            caller(honest_prestate.open_hoard),
            caller(honest),
            Pubkey::new_from_array([0x7d; 32]),
        ] {
            assert_ne!(hostile, caller(honest_prestate.initialize));
        }
    }

    fn demo_manifest(entries: usize) -> Vec<u8> {
        let foreign = (3..entries).collect::<Vec<_>>();
        demo_manifest_with_foreign_entries(entries, &foreign)
    }

    fn demo_manifest_with_trading(entries: usize, trading_index: Option<usize>) -> Vec<u8> {
        match trading_index {
            Some(index) => demo_manifest_with_foreign_entries(entries, &[index]),
            None => demo_manifest_with_foreign_entries(entries, &[]),
        }
    }

    fn demo_manifest_with_foreign_entries(entries: usize, foreign: &[usize]) -> Vec<u8> {
        use dclutch_capability_contract::{
            ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
            FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
            MAX_DEPENDENCIES_PER_CAPABILITY,
        };
        let native = CompartmentFundingV1::native_lamports(1).expect("native");
        let none = CompartmentFundingV1::not_applicable();
        let amounts =
            FundingAmountsV1::new(native, native, none, none, native, none, none).expect("amounts");
        let quote = FundingQuoteV1::new(amounts, None).expect("quote");
        let mut built = Vec::new();
        for index in 0..entries {
            let byte = u8::try_from(index).expect("index");
            built.push(
                CapabilityEntryV1::new(
                    ContentId::new([0x40 + byte; 32]).expect("kind"),
                    ContentId::new(if foreign.contains(&index) {
                        [0x51; 32]
                    } else {
                        [0x50; 32]
                    })
                    .expect("release"),
                    ContentId::new([0x60 + byte; 32]).expect("config"),
                    ContentId::new([0x70 + byte; 32]).expect("capacity"),
                    ContentId::new([0x80; 32]).expect("schema"),
                    ContentId::new([0x90; 32]).expect("derivation"),
                    ActivationPolicy::RequiredAtFounding,
                    0,
                    0,
                    [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                    quote,
                )
                .expect("entry"),
            );
        }
        let mut bytes =
            alloc::vec![0_u8; MANIFEST_HEADER_BYTES + built.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&built, &mut bytes).expect("manifest");
        bytes
    }

    /// The bounded mask is the exact low-entry prefix. Entry count zero and
    /// every width above the manifest profile refuse rather than collapsing to
    /// an empty or wrapped partition.
    #[test]
    fn required_union_is_exact_at_zero_one_four_and_sixteen() {
        assert_eq!(
            manifest_required_union(0),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(manifest_required_union(1), Ok(0b1));
        assert_eq!(manifest_required_union(4), Ok(0b1111));
        assert_eq!(manifest_required_union(16), Ok(u16::MAX));
        assert_eq!(
            manifest_required_union(17),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            manifest_required_union(u16::MAX),
            Err(TradingSbfError::Content.into())
        );
    }

    /// Canonical manifest ordering is by kind identity, not by controller.
    /// Moving the selected Trading entry across every physical position moves
    /// the two masks and nothing else; no literal `0b0111/0b1000` convention
    /// participates in authority.
    #[test]
    fn controller_masks_follow_the_authenticated_selected_entry() {
        for trading_index in 0_usize..4 {
            let bytes = demo_manifest_with_trading(4, Some(trading_index));
            let manifest = CapabilityManifestV1::decode(&bytes).expect("manifest");
            let trading_mask = 1_u16 << trading_index;
            assert_eq!(
                controller_masks(
                    manifest,
                    [0x50; 32],
                    u16::try_from(trading_index).expect("index")
                ),
                Ok([0b1111 ^ trading_mask, trading_mask])
            );

            // A different singleton is a syntactically valid partition, but
            // it is not this artifact's authenticated Trading selection. The
            // ownership projection must refuse every such substitution before
            // physical ordering can make the masks look canonical.
            for substituted_index in 0_usize..4 {
                if substituted_index == trading_index {
                    continue;
                }
                assert_eq!(
                    controller_masks(
                        manifest,
                        [0x50; 32],
                        u16::try_from(substituted_index).expect("substituted index"),
                    ),
                    Err(TradingSbfError::Content.into())
                );
            }
        }
    }

    /// The selected entry must be the sole non-Resolution entry and the exact
    /// four-entry cover is closed. A Resolution-selected root, a fifth row, or
    /// a second foreign release all refuse before either ledger is created.
    #[test]
    fn controller_masks_refuse_ambiguous_or_non_exhaustive_ownership() {
        let all_resolution = demo_manifest_with_trading(4, None);
        assert_eq!(
            controller_masks(
                CapabilityManifestV1::decode(&all_resolution).expect("manifest"),
                [0x50; 32],
                0,
            ),
            Err(TradingSbfError::Content.into())
        );

        let five = demo_manifest_with_trading(5, Some(4));
        assert_eq!(
            controller_masks(
                CapabilityManifestV1::decode(&five).expect("manifest"),
                [0x50; 32],
                4,
            ),
            Err(TradingSbfError::Content.into())
        );

        let two_foreign = demo_manifest_with_foreign_entries(4, &[1, 3]);
        assert_eq!(
            controller_masks(
                CapabilityManifestV1::decode(&two_foreign).expect("manifest"),
                [0x50; 32],
                3,
            ),
            Err(TradingSbfError::Content.into())
        );
    }

    fn funding_list(
        manifest_bytes: &[u8],
        market: [u8; 32],
        generation: u64,
        trading_index: u16,
    ) -> Identity {
        let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest");
        let manifest_id = ContentId::new(hash(manifest_bytes).to_bytes()).expect("manifest id");
        let facts = FoundingFundingFactsV1 {
            release_set: [0xaa; 32],
            market,
            generation,
            funding_list_id: Identity::new([1; 32]).expect("placeholder"),
            capability_entry_index: trading_index,
        };
        let rent = Rent::default();
        let resolution = plan_funding_ledger_v2(
            &Pubkey::new_from_array([0x52; 32]),
            manifest,
            manifest_id,
            &facts,
            0b1111 ^ (1_u16 << trading_index),
            &rent,
        )
        .expect("Resolution ledger");
        let trading_mask = 1_u16 << trading_index;
        let trading = plan_funding_ledger_v2(
            &trading(),
            manifest,
            manifest_id,
            &facts,
            trading_mask,
            &rent,
        )
        .expect("Trading ledger");
        canonical_funding_list_id_v2(
            0b1111 ^ trading_mask,
            &resolution.address,
            trading_mask,
            &trading.address,
        )
        .expect("list")
    }

    /// Trading accepts Resolution's construction only by re-authenticating the
    /// live ledger after the CPI. The poststate must still be the exact initial
    /// Pending ledger, at the one PDA and with the exact Rent plus native
    /// principal; owner, address, lamports, mask, or lifecycle substitutions
    /// all refuse inside the outer rollback domain.
    #[test]
    fn resolution_poststate_authenticates_one_exact_initial_ledger() {
        let manifest_bytes = demo_manifest_with_trading(4, Some(0));
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        let manifest_id = ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest id");
        let facts = FoundingFundingFactsV1 {
            release_set: [0xaa; 32],
            market: [0x11; 32],
            generation: 7,
            funding_list_id: Identity::new([1; 32]).expect("placeholder"),
            capability_entry_index: 0,
        };
        let resolution_program = Pubkey::new_from_array([0x52; 32]);
        let rent = Rent::default();
        let selected_mask = 0b1110;
        let planned = plan_funding_ledger_v2(
            &resolution_program,
            manifest,
            manifest_id,
            &facts,
            selected_mask,
            &rent,
        )
        .expect("canonical Resolution ledger");
        let observed = authenticate_resolution_ledger_poststate_bytes_v2(
            &resolution_program,
            &planned.address,
            &resolution_program,
            planned.exact_lamports,
            &planned.bytes,
            manifest,
            manifest_id,
            &facts,
            selected_mask,
            &rent,
        )
        .expect("authenticated live poststate");
        assert_eq!(observed.poststate_digest, hash(&planned.bytes).to_bytes());
        let exact_rent = rent.minimum_balance(planned.bytes.len());
        assert_eq!(observed.exact_rent_lamports, exact_rent);
        assert_eq!(
            observed.exact_native_principal,
            planned.exact_lamports - exact_rent
        );
        let foreign_owner = Pubkey::new_from_array([0x53; 32]);
        let foreign_address = Pubkey::new_from_array([0x54; 32]);
        for (address, owner, lamports, mask) in [
            (
                planned.address,
                foreign_owner,
                planned.exact_lamports,
                selected_mask,
            ),
            (
                foreign_address,
                resolution_program,
                planned.exact_lamports,
                selected_mask,
            ),
            (
                planned.address,
                resolution_program,
                planned.exact_lamports - 1,
                selected_mask,
            ),
            (
                planned.address,
                resolution_program,
                planned.exact_lamports + 1,
                selected_mask,
            ),
            (
                planned.address,
                resolution_program,
                planned.exact_lamports,
                0b1101,
            ),
        ] {
            assert!(
                authenticate_resolution_ledger_poststate_bytes_v2(
                    &resolution_program,
                    &address,
                    &owner,
                    lamports,
                    &planned.bytes,
                    manifest,
                    manifest_id,
                    &facts,
                    mask,
                    &rent,
                )
                .is_err()
            );
        }

        let mut advanced = planned.bytes.clone();
        FundingLedgerV2::activate_in_place(&mut advanced, manifest_id, manifest, 1, 9)
            .expect("semantically valid advanced ledger");
        assert!(
            authenticate_resolution_ledger_poststate_bytes_v2(
                &resolution_program,
                &planned.address,
                &resolution_program,
                planned.exact_lamports,
                &advanced,
                manifest,
                manifest_id,
                &facts,
                selected_mask,
                &rent,
            )
            .is_err()
        );
    }

    /// The capability-funding tail this route creates is pinned to exactly one
    /// founding on every coordinate Core will re-derive it from.
    ///
    /// A funding state is a Trading-owned program address, so no host can ever
    /// create one — there is no private key — and until this route existed the
    /// only allocator in the protocol was the Series ticket-consume path, which
    /// has no caller. These are the substitutions that must move the artifact's
    /// `funding_list_id` and therefore refuse.
    #[test]
    fn the_capability_funding_tail_is_pinned_to_one_founding() {
        let manifest = demo_manifest(4);
        let market = [0x11; 32];
        let honest = funding_list(&manifest, market, 7, 3);

        // A different Market, or a different generation, is a different tail.
        assert_ne!(honest, funding_list(&manifest, [0x12; 32], 7, 3));
        assert_ne!(honest, funding_list(&manifest, market, 8, 3));
        // A manifest whose entries differ is a different tail, because the
        // config and release identities are PDA seeds.
        assert_ne!(honest, funding_list(&demo_manifest(5), market, 7, 3));

        // Order is load-bearing: the list identity refuses a permuted set.
        let bytes = manifest.clone();
        let decoded = CapabilityManifestV1::decode(&bytes).expect("manifest");
        let manifest_id = ContentId::new(hash(&bytes).to_bytes()).expect("id");
        let facts = FoundingFundingFactsV1 {
            release_set: [0xaa; 32],
            market,
            generation: 7,
            funding_list_id: honest,
            capability_entry_index: 3,
        };
        let rent = Rent::default();
        let resolution = plan_funding_ledger_v2(
            &Pubkey::new_from_array([0x52; 32]),
            decoded,
            manifest_id,
            &facts,
            0b0111,
            &rent,
        )
        .expect("Resolution");
        let trading =
            plan_funding_ledger_v2(&trading(), decoded, manifest_id, &facts, 0b1000, &rent)
                .expect("Trading");
        let mut keys = vec![
            Identity::new(resolution.address.to_bytes()).expect("Resolution key"),
            Identity::new(trading.address.to_bytes()).expect("Trading key"),
        ];
        assert_eq!(
            generic_founding_funding_list_id_v1(&keys).expect("ordered"),
            honest
        );
        keys.reverse();
        assert_ne!(
            generic_founding_funding_list_id_v1(&keys).expect("reversed"),
            honest
        );
        // And an aliased tail is refused outright rather than hashing to
        // something.
        let aliased = *keys.first().expect("the funding key list is non-empty");
        assert!(generic_founding_funding_list_id_v1(&[aliased, aliased]).is_err());
    }

    /// The controller names never decide the physical tail order. For each
    /// possible selected Direct bit, the helper must reproduce the compiler's
    /// lowest-bit ordering; bit zero is the regression that a fresh validator
    /// exposed because it puts Trading before Resolution.
    #[test]
    fn funding_list_order_follows_lowest_authenticated_mask_bit() {
        let resolution = Pubkey::new_from_array([0x52; 32]);
        let trading = Pubkey::new_from_array([0x53; 32]);
        for trading_index in 0_u16..4 {
            let trading_mask = 1_u16 << trading_index;
            let resolution_mask = 0b1111 ^ trading_mask;
            let ordered_masks =
                canonical_funding_mask_order_v2(resolution_mask, trading_mask).expect("order");
            assert!(validate_funding_ledger_masks_v2(4, 0b1111, &ordered_masks).is_ok());
            let resolution_id = Identity::new(resolution.to_bytes()).expect("Resolution");
            let trading_id = Identity::new(trading.to_bytes()).expect("Trading");
            let expected = if resolution_mask.trailing_zeros() < trading_mask.trailing_zeros() {
                [resolution_id, trading_id]
            } else {
                [trading_id, resolution_id]
            };
            assert_eq!(
                canonical_funding_list_id_v2(resolution_mask, &resolution, trading_mask, &trading,),
                generic_founding_funding_list_id_v1(&expected)
                    .map_err(|_| TradingSbfError::Content.into())
            );
        }
    }

    /// The live Direct compiler selects manifest bit zero. Controller-name
    /// order therefore presents Resolution's `0b1110` before Trading's
    /// `0b0001`, which the FundingLedger contract correctly refuses as a
    /// noncanonical physical partition. Canonicalizing before that validator is
    /// load-bearing: the old order dies before the Resolution CPI and leaves
    /// the otherwise honest DCLTPCB2 frame unable to stage.
    #[test]
    fn live_direct_bit_zero_partition_is_ordered_before_validation() {
        let resolution_mask = 0b1110;
        let trading_mask = 0b0001;
        assert!(
            validate_funding_ledger_masks_v2(4, 0b1111, &[resolution_mask, trading_mask],).is_err()
        );
        let ordered =
            canonical_funding_mask_order_v2(resolution_mask, trading_mask).expect("canonical");
        assert_eq!(ordered, [trading_mask, resolution_mask]);
        assert!(validate_funding_ledger_masks_v2(4, 0b1111, &ordered).is_ok());
    }

    #[test]
    fn funding_list_order_refuses_empty_or_ambiguous_lowest_bits() {
        let resolution = Pubkey::new_from_array([0x52; 32]);
        let trading = Pubkey::new_from_array([0x53; 32]);
        for (resolution_mask, trading_mask) in [(0, 1), (1, 0), (0b0011, 0b0001)] {
            assert!(canonical_funding_mask_order_v2(resolution_mask, trading_mask).is_err());
            assert_eq!(
                canonical_funding_list_id_v2(resolution_mask, &resolution, trading_mask, &trading,),
                Err(TradingSbfError::Content.into())
            );
        }
    }

    #[test]
    fn each_prestate_has_its_own_single_use_caller_authority() {
        let lock = lock();
        let prestate = lock.founding_prestate_v1().expect("prestate");
        let caller = |request: ProjectedCustodyRequestV1| {
            let raw = request.encode().expect("bytes");
            Pubkey::find_program_address(
                &ProjectedCustodyCallerSeedsV1::new(request, hash(&raw).to_bytes()).as_slices(),
                &trading(),
            )
            .0
        };
        let authorities = [
            caller(prestate.initialize),
            caller(prestate.open_hoard),
            caller(prestate.open_source),
            caller(lock),
        ];
        for (index, left) in authorities.iter().enumerate() {
            for right in authorities.iter().skip(index + 1) {
                assert_ne!(left, right, "two ladder stages share one caller authority");
            }
        }
    }

    /// The source compartment the third stage creates is the exact one the
    /// terminal Lock consumes and closes, on every coordinate that names it.
    #[test]
    fn the_third_stage_creates_exactly_the_compartment_the_lock_consumes() {
        let lock = lock();
        let prestate = lock.founding_prestate_v1().expect("prestate");
        let source = prestate.open_source;
        assert_eq!(
            source.operation,
            ProjectedCustodyOperationV1::OpenSourceCompartment
        );
        assert_eq!(source.funding_source_vault, lock.funding_source_vault);
        assert_eq!(source.funding_source_context, lock.funding_source_context);
        assert_eq!(
            source.funding_source_compartment,
            lock.funding_source_compartment
        );
        assert_eq!(
            source.funding_source_replay_revision,
            lock.funding_source_replay_revision
        );
        assert_eq!(
            source.funding_source_vault_rent_lamports,
            lock.funding_source_vault_rent_lamports
        );
        assert_eq!(
            source.funding_source_state_rent_lamports,
            lock.funding_source_state_rent_lamports
        );
        assert_eq!(source.amount, lock.amount);
        assert_eq!(source.rent_credit, lock.rent_credit);
        assert_eq!(source.refund_owner, lock.refund_owner);
        assert_eq!(source.payer, lock.payer);
        // And it is bound to the founding artifact through the same join the
        // outer evaluates: the Lock's funding source is the founding's.
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found(), &lock),
            Ok(())
        );
        let mut resourced = lock;
        resourced.funding_source_vault = [0x7e; 32];
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found(), &resourced),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn the_abort_route_is_a_distinct_tag_and_an_exact_frame() {
        assert!(is_projected_custody_abort_v1(
            &PROJECTED_CUSTODY_ABORT_MAGIC_V1
        ));
        assert!(!is_projected_custody_abort_v1(&[0; 8]));
        // A prefix of the tag is not the tag, and the two routes in this module
        // must never be selected by one another's bytes.
        assert!(!is_projected_custody_abort_v1(&[
            b'D', b'C', b'L', b'T', b'P', b'C', b'A', b'1', 0
        ]));
        assert!(!is_projected_custody_abort_v1(
            &PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2
        ));
        assert!(!is_projected_custody_bootstrap_v2(
            &PROJECTED_CUSTODY_ABORT_MAGIC_V1
        ));
        assert_eq!(PROJECTED_CUSTODY_ABORT_INSTRUCTION_BYTES_V1, 8);
        // Two readonly-and-program accounts, then Custody's own abort frame.
        assert_eq!(PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1, 18);
        // Unlike the bootstrap, this frame is a CONSTANT: the abort closes the
        // projection and touches no FundingState, so there is no tail whose
        // length a founding artifact would have to pin.
        assert_eq!(
            PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1,
            ABORT_SUB_FRAME_START + PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1
        );
    }

    #[test]
    fn the_abort_derives_the_terminal_lock_at_the_same_cursor() {
        let lock = lock();
        let abort = lock.founding_source_abort_v1();
        // One field. The abort is the Lock's own decision point taken the other
        // way, so it cannot name a coordinate the founding does not.
        assert_eq!(
            abort,
            ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::AbortSourceAndClose,
                ..lock
            }
        );
        assert_eq!(abort.amount, lock.amount);
        assert_eq!(abort.refund_owner, lock.refund_owner);
        assert_eq!(abort.rent_credit, lock.rent_credit);
        assert_eq!(abort.expiry_slot, lock.expiry_slot);
        assert_eq!(abort.expected_revision, lock.expected_revision);
        assert_eq!(abort.resulting_revision, lock.resulting_revision);
    }

    #[test]
    fn the_abort_route_privileges_are_exactly_what_the_child_needs() {
        // Six writable slots and one signer, and no overlap between them: the
        // principal's owner signs while staying non-writable, which is what
        // stops it from being the rent payer, and every account that is closed
        // or credited is writable.
        for writable in ABORT_WRITABLE {
            assert!(
                !ABORT_SIGNERS.contains(&writable),
                "slot {writable} cannot be both the signing owner and writable"
            );
            assert!(writable < PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1);
        }
        for signer in ABORT_SIGNERS {
            assert!(signer < PROJECTED_CUSTODY_ABORT_SOURCE_ACCOUNT_COUNT_V1);
        }
        assert!(ABORT_WRITABLE.contains(&COMMON_STATE));
        assert!(ABORT_WRITABLE.contains(&COMMON_RENT_CREDIT));
        assert_eq!(ABORT_SIGNERS, [ABORT_SOURCE_REFUND_OWNER]);
        // The caller PDA is never in either list: it signs only inside the CPI,
        // under `invoke_signed`, exactly as every other stage in this module.
        assert!(!ABORT_WRITABLE.contains(&COMMON_CALLER));
        assert!(!ABORT_SIGNERS.contains(&COMMON_CALLER));
    }
}
