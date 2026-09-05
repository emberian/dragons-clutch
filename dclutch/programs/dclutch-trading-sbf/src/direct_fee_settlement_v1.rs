//! The permissionless second transaction that pays one Direct fee.
//!
//! `docs/design/FEE_SECOND_TRANSACTION_V1.md` moves the Direct fee leg out of
//! the fill's transaction, because the two together do not fit the compute
//! ceiling. The fill records the obligation as `fee_owed` on the buyer's maker
//! replay and leaves the residual SPL delegation standing; this route moves
//! exactly that amount to the Market's configured fee recipient, clears the
//! field, and unblocks the maker.
//!
//! # What authenticates Trading here, and it is the same thing as in the fill
//!
//! `FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md` closed with "nothing about
//! Trading" as its first open question: whether Trading can authenticate
//! itself, read `replay.last_request_digest`, and project a well-formed fee
//! request in a later transaction. The answer is that **no new authentication
//! exists or is needed**, and the reason is worth stating once here rather than
//! being rediscovered.
//!
//! Custody's admission for a delegated transfer is `authenticate_common_frame`:
//! it rebuilds `CallerAuthoritySeedsV1` out of the request's own bytes and
//! compares the derived address against frame coordinate 0, and the address is
//! a PDA **of the caller program**. So the whole of Trading's self-attestation
//! is that it can produce a signature for that PDA -- which only Trading can,
//! and which it does with `invoke_signed` below. The sixth seed is the digest
//! of the request Trading has just built, not of anything committed earlier, so
//! there is nothing for the fill to have registered and nothing for this route
//! to look up. Custody binds `request.caller_program` to the Trading role of
//! the activated release set (`authenticate_calling_release` -> `Release`), and
//! the replay independently refuses a `caller_program` other than the one it
//! recorded. That is the entire chain, and every link of it already shipped.
//!
//! What this route therefore owes is not authentication but **derivation**:
//! every economic value in the fee request must come from program-owned state,
//! or a stranger's submission is not effect-free. §1.4 of the design lists
//! where each field comes from; [`derive_fee_request`] is that table as code.
//!
//! # The three refusals this route adds
//!
//! * [`TradingSbfError::FeeNotOwed`] -- the maker replay records no obligation.
//! * [`TradingSbfError::FeeDestination`] -- the destination is not a token
//!   account of the immutable config's `fee_recipient` on the Realm's mint.
//! * [`TradingSbfError::FeeSource`] -- the source is not a token account of the
//!   **debtor**. The design's §1.4 refusal table names only the first two, and
//!   the third is a hostile it does not enumerate: Custody checks
//!   `source.key == request.source` and the source's mint, and **never**
//!   `semantic.source_owner`. Without this pin, maker A could settle A's own
//!   `fee_owed` out of maker B's collateral whenever B's standing delegation
//!   happened to equal A's debt -- clearing A for free, consuming the allowance
//!   B needs to settle with, and stranding B behind the §2.4 lockout forever.
//!   The obligation is the debtor's and it comes out of the debtor's account.
//!
//! # Deliberate non-conditions
//!
//! * **The Market's phase is not checked.** The obligation is undeadlined
//!   (design §7.1) and outlives the trading it arose from; requiring `Open`
//!   here would strand a fee the moment the market resolved, and strand the
//!   maker with it.
//! * **The destination is pinned by OWNER, never by address** (design §3). A
//!   recipient token account closed between the fill and its settlement strands
//!   nobody: any account of that owner will do. That is E5's vanished-recipient
//!   condition, and it is the same rule `validate_collateral` already applies
//!   to the fill.
//! * **The frame's writability is pinned in one direction only** (§3, and
//!   `fractional_retirement_v3`'s `FrameRoleV3`). Demanding readonly would
//!   forbid batching this act with a fill that writes the same coordinate,
//!   which is exactly the defect `80b78181` and `16351a13` fixed elsewhere.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CallerRoleV1, CustodyFrameSpecV1, CustodyReplayV1, CustodyRequestV1,
    DELEGATED_CUSTODY_RECEIPT_BYTES_V2, DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2,
    OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_trading::{
    execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
    fee_settlement_v1::{
        DirectFeeProjectionV1, DirectFeeSettlementReceiptV1, DirectFeeSettlementRequestV1,
        project_direct_fee_request_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1,
        DirectExecutionConfigV1, DirectRootStateV1, MakerReplayRootV1, MakerReplaySeedsV1,
    },
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_custody::token_svm::TokenAccount;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::TradingSbfError;
use crate::child_authority_v4::child_caller_authority_v4;
use crate::child_refused_v1;

/// The Custody `Transfer` frame this route carries verbatim, from its owner.
const TRANSFER_FRAME: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;

/// Exact top-level account count.
///
/// The fourteen Custody `Transfer` coordinates, the executable callee beside
/// them (`require_custody_frame_shape_v3`: a Custody frame never carries its
/// own callee), and the four accounts Trading needs and Custody does not --
/// the debtor's maker replay, the Direct root that names the config, and the
/// config record's raw/staging pair.
///
/// Nineteen, inside the design's estimated 18-21.
pub const DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1: usize = TRANSFER_FRAME + 5;

const CALLER_AUTHORITY: usize = 0;
const CORE_MARKET: usize = 1;
const REGISTRY_PROGRAM: usize = 3;
const TRADING_PROGRAM: usize = 4;
const CUSTODY_REPLAY: usize = 8;
const MINT: usize = 9;
const FEE_SOURCE: usize = 10;
const FEE_DESTINATION: usize = 11;
const CUSTODY_AUTHORITY: usize = 12;
const TOKEN_PROGRAM: usize = 13;
const CUSTODY_PROGRAM: usize = 14;
const MAKER_REPLAY: usize = 15;
const DIRECT_ROOT: usize = 16;
const CONFIG_RAW: usize = 17;
const CONFIG_STAGING: usize = 18;

const _: () = assert!(DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1 == 19);
const _: () = assert!(CONFIG_STAGING + 1 == DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1);
// The two Custody coordinates this module names by index rather than by
// FrameSpec role. Their positions are the same three the shipped Effect walk
// depends on (`custody_composition_v3::CUSTODY_REPLAY_FRAME_COORDINATE_V1`);
// stating them here is what makes a FrameSpec reordering a compile error rather
// than a runtime mystery.
const _: () = assert!(CALLER_AUTHORITY == 0 && CUSTODY_REPLAY == 8);

/// What one frame coordinate must be, with writability pinned one way only.
///
/// Copied deliberately from `dclutch-claims-sbf`'s `fractional_retirement_v3`,
/// where the doc comment `80b78181` added carries the full argument:
/// `is_writable` merges across a transaction's instructions, so a coordinate
/// this route only reads arrives `true` whenever the caller's *other*
/// instruction had to write it. Demanding writable is a statement about this
/// instruction and is enforceable; demanding readonly is a statement about the
/// caller's whole transaction and only forbids compositions. A builder that
/// batches the fill and this settlement into one transaction is precisely that
/// case, so a two-directional pin here would make the pair unbatchable for no
/// protection at all.
#[derive(Clone, Copy)]
enum FrameRoleV3 {
    /// This route, or the child it invokes, writes it.
    Written,
    /// An executable this route only names.
    Program,
    /// This route only reads it.
    Read,
}

impl FrameRoleV3 {
    fn admits(self, observed: &AccountInfo<'_>) -> bool {
        let executable = matches!(self, Self::Program);
        let writability_is_free = matches!(self, Self::Program | Self::Read);
        // Nobody signs (design section 3): both parties with an interest are
        // disqualified for opposite reasons, so the route admits no signer at
        // any coordinate. The transaction's fee payer is not a frame member.
        !observed.is_signer
            && observed.executable == executable
            && (writability_is_free || observed.is_writable)
    }
}

const fn frame_role(index: usize) -> Option<FrameRoleV3> {
    match index {
        CUSTODY_REPLAY | FEE_SOURCE | FEE_DESTINATION | MAKER_REPLAY => Some(FrameRoleV3::Written),
        REGISTRY_PROGRAM | TRADING_PROGRAM | TOKEN_PROGRAM | CUSTODY_PROGRAM => {
            Some(FrameRoleV3::Program)
        }
        CALLER_AUTHORITY | CORE_MARKET | 2 | 5 | 6 | 7 | MINT | CUSTODY_AUTHORITY | DIRECT_ROOT
        | CONFIG_RAW | CONFIG_STAGING => Some(FrameRoleV3::Read),
        // An unnamed coordinate refuses rather than inheriting whichever arm it
        // was written beside.
        _ => None,
    }
}

/// The obligation this settlement is for, read entirely out of program-owned state.
#[derive(Clone, Copy)]
struct ObligationV1 {
    /// The debtor's maker replay: a Trading PDA, and the Custody `context`.
    maker_root: Pubkey,
    /// Exactly what tx1 recorded. Never "whatever is delegated".
    fee_owed: u64,
    /// The immutable config's external fee recipient.
    fee_recipient: [u8; 32],
}

/// Everything the receipt reports, captured across the child invocation.
#[derive(Clone, Copy)]
struct SettlementV1 {
    obligation: ObligationV1,
    custody_request_digest: [u8; 32],
    custody_poststate: [u8; 32],
    expected_revision: u64,
    resulting_revision: u64,
}

/// Execute one exact permissionless Direct fee settlement.
#[inline(never)]
pub fn process_direct_fee_settlement_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = DirectFeeSettlementRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_frame(program_id, accounts)?;
    let settlement = settle(program_id, accounts, request)?;
    emit_receipt(
        accounts,
        hash(instruction_data).to_bytes(),
        request,
        settlement,
    )
}

/// Exact count, named roles, one-directional writability, distinct addresses.
#[inline(never)]
fn authenticate_frame(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, observed) in accounts.iter().enumerate() {
        if !frame_role(index).is_some_and(|role| role.admits(observed)) {
            return Err(TradingSbfError::Content.into());
        }
    }
    // Every coordinate is a distinct role, so any alias is a caller handing one
    // account two jobs. `Transfer`'s own `AliasedTransferAccounts` covers the
    // source/destination pair; this covers the other 169 pairs.
    for (offset, left) in accounts.iter().enumerate() {
        if accounts
            .get(offset.saturating_add(1)..)
            .is_some_and(|tail| tail.iter().any(|right| right.key == left.key))
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    // The executable-ness of these two is already pinned by `frame_role`; what
    // is pinned here is WHICH executables they are, which the roles cannot say.
    if account(accounts, TRADING_PROGRAM)?.key != program_id
        || account(accounts, CUSTODY_PROGRAM)?.key == program_id
    {
        return Err(TradingSbfError::Content.into());
    }
    // The privileges the child's own FrameSpec declares, checked against the
    // fourteen coordinates that will be forwarded verbatim. Writability is
    // taken one-directionally above; what this adds is that no coordinate the
    // child needs WRITABLE arrives readonly, stated by the contract that owns
    // the frame rather than restated here.
    let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
    if usize::from(spec.account_count()) != TRANSFER_FRAME {
        return Err(TradingSbfError::Content.into());
    }
    for index in 0..TRANSFER_FRAME {
        let declared = spec
            .account(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
            .map_err(|_| TradingSbfError::Content)?
            .privileges();
        let observed = account(accounts, index)?;
        if (declared.writable() && !observed.is_writable)
            || declared.executable() != observed.executable
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

/// Derive the fee request from state, settle the obligation, prove it landed.
#[inline(never)]
fn settle(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DirectFeeSettlementRequestV1,
) -> Result<SettlementV1, ProgramError> {
    let obligation = authenticate_obligation(program_id, accounts, request)?;
    let fee_request = derive_fee_request(accounts, request, program_id, obligation)?;
    let expected_revision = fee_request.custody.expected_revision;
    let resulting_revision = fee_request.custody.resulting_revision;
    let custody_request_digest = sign_and_invoke(
        accounts,
        program_id,
        fee_request,
        obligation.fee_owed,
        request.caller_authority_hint(),
        request.custody_relay(),
    )?;
    authenticate_child_result(
        accounts,
        fee_request,
        custody_request_digest,
        resulting_revision,
    )
    .map(|custody_poststate| SettlementV1 {
        obligation,
        custody_request_digest,
        custody_poststate,
        expected_revision,
        resulting_revision,
    })
}

/// Read the debt, the debtor, and the creditor, each from the record that owns it.
#[inline(never)]
fn authenticate_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DirectFeeSettlementRequestV1,
) -> Result<ObligationV1, ProgramError> {
    let coordinates = DirectCoordinatesV1::new(request.market, request.generation)
        .map_err(|_| TradingSbfError::Content)?;

    let replay_account = account(accounts, MAKER_REPLAY)?;
    let fee_owed = {
        let data = replay_account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        let root = MakerReplayRootV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
        let seeds = MakerReplaySeedsV1::new(coordinates, request.maker)
            .map_err(|_| TradingSbfError::Content)?;
        let [domain, market, generation, maker] = seeds.as_slices();
        let bump = [root.bump()];
        let expected =
            Pubkey::create_program_address(&[domain, market, generation, maker, &bump], program_id)
                .map_err(|_| TradingSbfError::Content)?;
        if replay_account.owner != program_id
            || replay_account.key != &expected
            || root.market() != request.market
            || root.generation() != request.generation
            || root.maker() != request.maker
            || !funded_rent_persists_v1(replay_account.lamports())
        {
            return Err(TradingSbfError::Content.into());
        }
        // No width guard here, deliberately. `MakerReplayRootV1::decode`
        // accepts the pre-`fee_owed` width and reads zero from it, which is
        // exactly the state this route has nothing to do in -- so a legacy
        // replay refuses one line below as `FeeNotOwed`, which is the true
        // reason, rather than as a width complaint that would read like a
        // migration problem. Nothing can be written to such an account either:
        // `clear_fee_owed` refuses a length its encoding does not fill.
        root.fee_owed()
    };
    if fee_owed == 0 {
        return Err(TradingSbfError::FeeNotOwed.into());
    }

    let fee_recipient = authenticate_config(program_id, accounts, request)?;
    Ok(ObligationV1 {
        maker_root: *replay_account.key,
        fee_owed,
        fee_recipient,
    })
}

/// The Direct root names the config; the config names the creditor.
///
/// Neither is instruction data. The root is a Trading PDA over the activation
/// projection the Market selected, and its `selection().config()` is the only
/// config id this route will read -- so a caller cannot point the fee at a
/// config of their own. The record is content-addressed and its staging cursor
/// is vacant, which is how this tree spells "immutable"
/// (`immutable_registry.rs`, and ADR 0014 section 2 on why it can never move).
#[inline(never)]
fn authenticate_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DirectFeeSettlementRequestV1,
) -> Result<[u8; 32], ProgramError> {
    let root = account(accounts, DIRECT_ROOT)?;
    let config_id = {
        let data = root.try_borrow_data().map_err(|_| TradingSbfError::Root)?;
        let expected_width = CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
            .ok_or(TradingSbfError::Root)?;
        if root.owner != program_id || data.len() != expected_width {
            return Err(TradingSbfError::Root.into());
        }
        let header = CapabilityRootHeaderV1::decode(
            data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                .ok_or(TradingSbfError::Root)?,
        )
        .map_err(|_| TradingSbfError::Root)?;
        // Decoded, and its phase deliberately unused: see the module header.
        // An undecodable tail is still a root this route will not read a config
        // out of.
        DirectRootStateV1::decode(
            data.get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .ok_or(TradingSbfError::Root)?,
        )
        .map_err(|_| TradingSbfError::Root)?;
        if root.key != &Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0
            || !funded_rent_persists_v1(root.lamports())
            || header.market() != request.market
            || header.generation() != request.generation
            || header.selection().kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        {
            return Err(TradingSbfError::Root.into());
        }
        let bumps = header.record_bumps();
        authenticate_finalized_config_record(
            accounts,
            header.selection().config().to_bytes(),
            bumps.config_raw(),
            bumps.config_staging(),
        )?;
        header.selection().config().to_bytes()
    };
    let raw = account(accounts, CONFIG_RAW)?;
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let config = DirectExecutionConfigV1::decode_selected(config_id, hash(&data).to_bytes(), &data)
        .map_err(|_| TradingSbfError::Content)?;
    Ok(config.fee_recipient())
}

/// The raw record at its content address, with its staging cursor vacant.
///
/// The two bumps come from the root's own `SelectedRecordBumpsV1`, which the
/// activation derived and pinned, so this reproduces two addresses instead of
/// searching for them -- the same trade `direct_begin_retiring_v1` makes for
/// the manifest.
#[inline(never)]
fn authenticate_finalized_config_record(
    accounts: &[AccountInfo<'_>],
    config_id: [u8; 32],
    raw_bump: u8,
    staging_bump: u8,
) -> ProgramResult {
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let raw = account(accounts, CONFIG_RAW)?;
    let staging = account(accounts, CONFIG_STAGING)?;
    let raw_bump = [raw_bump];
    let staging_bump = [staging_bump];
    let expected_raw = Pubkey::create_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1.as_slice(),
            config_id.as_slice(),
            &raw_bump,
        ],
        registry.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_staging = Pubkey::create_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1.as_slice(),
            config_id.as_slice(),
            &staging_bump,
        ],
        registry.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if raw.key != &expected_raw
        || raw.owner != registry.key
        || hash(&data).to_bytes() != config_id
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Section 1.4's field table, as code. No economic value comes off the wire.
#[inline(never)]
fn derive_fee_request(
    accounts: &[AccountInfo<'_>],
    request: DirectFeeSettlementRequestV1,
    program_id: &Pubkey,
    obligation: ObligationV1,
) -> Result<DelegatedCustodyRequestV2, ProgramError> {
    let source = account(accounts, FEE_SOURCE)?;
    let destination = account(accounts, FEE_DESTINATION)?;
    let mint = account(accounts, MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let custody_authority = account(accounts, CUSTODY_AUTHORITY)?;

    // The debtor's account, and the creditor's. Both are pinned by OWNER and
    // by mint, never by address: see the module header on why the destination
    // must not be address-bound, and on why the source must be owner-bound.
    let source_owner = {
        let data = source
            .try_borrow_data()
            .map_err(|_| TradingSbfError::FeeSource)?;
        let token = TokenAccount::parse(&data).map_err(|_| TradingSbfError::FeeSource)?;
        if source.owner != token_program.key
            || token.owner != request.maker
            || token.mint != mint.key.to_bytes()
        {
            return Err(TradingSbfError::FeeSource.into());
        }
        token.owner
    };
    let destination_owner = {
        let data = destination
            .try_borrow_data()
            .map_err(|_| TradingSbfError::FeeDestination)?;
        let token = TokenAccount::parse(&data).map_err(|_| TradingSbfError::FeeDestination)?;
        if destination.owner != token_program.key
            || token.owner != obligation.fee_recipient
            || token.mint != mint.key.to_bytes()
        {
            return Err(TradingSbfError::FeeDestination.into());
        }
        token.owner
    };

    // Every binding field comes off the Custody replay, which `advance` then
    // re-checks for equality against the same seven values it recorded -- so a
    // caller who substituted a replay names a different PDA and Custody's own
    // `authenticate_replay_identity` refuses before any of this matters.
    let replay_account = account(accounts, CUSTODY_REPLAY)?;
    let replay = {
        let data = replay_account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        if replay_account.owner != account(accounts, CUSTODY_PROGRAM)?.key {
            return Err(TradingSbfError::Content.into());
        }
        CustodyReplayV1::decode(&data).map_err(|_| TradingSbfError::Content)?
    };
    if replay.caller_role != CallerRoleV1::Trading
        || replay.market != request.market
        || replay.generation != request.generation
        || replay.context != obligation.maker_root.to_bytes()
        || replay.caller_program != program_id.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    // The projection is the codec crate's, and that is a correctness
    // requirement rather than tidiness: the caller-authority PDA's sixth seed
    // is the digest of these bytes, so a builder that reproduced the field
    // table separately would address an authority nothing signs the moment the
    // two drifted by a byte -- and the refusal would name the authority, not
    // the drift. One function, both readers.
    project_direct_fee_request_v1(DirectFeeProjectionV1 {
        replay,
        fee_owed: obligation.fee_owed,
        source: source.key.to_bytes(),
        source_owner,
        destination: destination.key.to_bytes(),
        destination_owner,
        mint: mint.key.to_bytes(),
        token_program: token_program.key.to_bytes(),
        custody_authority: custody_authority.key.to_bytes(),
    })
    .map_err(|_| TradingSbfError::Content.into())
}

/// Reproduce the caller authority the request's own digest names.
/// Encode the request, address the authority it names, and spend it.
///
/// **THIS FUNCTION EXISTS TO OWN 776 BYTES.** `DelegatedCustodyRequestV2::encode`
/// returns a `DELEGATED_CUSTODY_REQUEST_BYTES_V2` array, and while that array
/// lived in `settle` it made `settle` the deepest frame in the whole Trading
/// link -- 4,032 of 4,096, sixty-four bytes from a toolchain that calls the
/// overflow undefined behaviour. The bytes are needed at exactly three
/// consecutive points and nowhere else: to be hashed, to address the caller
/// authority through that hash, and to be handed to the child. So they live in
/// the frame of the function that does those three things, and `settle` holds
/// only the digest that outlives them.
///
/// The order is load-bearing and is the order it was written in. The obligation
/// is cleared BEFORE the child moves anything, so no path through the CPI can
/// observe a maker replay that still owes what the transfer is already
/// spending. The transaction is the rollback boundary either way; the ordering
/// is what makes that argument unnecessary.
#[inline(never)]
fn sign_and_invoke(
    accounts: &[AccountInfo<'_>],
    program_id: &Pubkey,
    fee_request: DelegatedCustodyRequestV2,
    fee_owed: u64,
    caller_authority_hint: Option<u8>,
    custody_relay: [u8; 2],
) -> Result<[u8; 32], ProgramError> {
    let request_bytes = fee_request.encode().map_err(|_| TradingSbfError::Content)?;
    let request_digest = hash(&request_bytes).to_bytes();
    let bump = authenticate_caller_authority(
        program_id,
        accounts,
        &fee_request.custody,
        request_digest,
        caller_authority_hint,
    )?;
    clear_fee_owed(accounts, fee_owed)?;
    invoke_custody(
        accounts,
        &request_bytes,
        &fee_request.custody,
        request_digest,
        bump,
        custody_relay,
    )?;
    Ok(request_digest)
}

#[inline(never)]
fn authenticate_caller_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    custody: &CustodyRequestV1,
    request_digest: [u8; 32],
    hint: Option<u8>,
) -> Result<u8, ProgramError> {
    let seeds = caller_seeds(custody, request_digest)?;
    let (expected, bump) = child_caller_authority_v4(&seeds, program_id, hint)?;
    if account(accounts, CALLER_AUTHORITY)?.key != &expected {
        return Err(TradingSbfError::Release.into());
    }
    Ok(bump)
}

fn caller_seeds(
    custody: &CustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| TradingSbfError::Content)?,
        custody.market,
        ExecutionRoleV1::Trading,
        custody.context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content.into())
}

/// Clear the obligation for exactly the amount recorded, and no other.
#[inline(never)]
fn clear_fee_owed(accounts: &[AccountInfo<'_>], fee_owed: u64) -> ProgramResult {
    let replay_account = account(accounts, MAKER_REPLAY)?;
    let mut data = replay_account
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    let settled = MakerReplayRootV1::decode(&data)
        .map_err(|_| TradingSbfError::Commit)?
        // `settle_fee_owed` refuses any amount other than the recorded one,
        // which is section 2.4's invariant 4 stated where it is enforced: a
        // buyer who re-approved a smaller allowance must not settle short and
        // clear the flag.
        .settle_fee_owed(fee_owed)
        .map_err(|_| TradingSbfError::Commit)?
        .encode()
        .map_err(|_| TradingSbfError::Commit)?;
    if data.len() != settled.len() {
        return Err(TradingSbfError::Commit.into());
    }
    data.copy_from_slice(&settled);
    Ok(())
}

/// Forward the fourteen coordinates and the three-byte bump relay to Custody.
#[inline(never)]
fn invoke_custody(
    accounts: &[AccountInfo<'_>],
    request_bytes: &[u8],
    custody: &CustodyRequestV1,
    request_digest: [u8; 32],
    bump: u8,
    relay: [u8; 2],
) -> ProgramResult {
    let frame = accounts
        .get(..TRANSFER_FRAME)
        .ok_or(TradingSbfError::Content)?;
    let mut metas = Vec::with_capacity(TRANSFER_FRAME);
    for (index, info) in frame.iter().enumerate() {
        // Coordinate zero's signer bit is the one this program supplies and no
        // keypair can; every other privilege is the transaction's.
        let signer = index == CALLER_AUTHORITY;
        metas.push(if info.is_writable {
            AccountMeta::new(*info.key, signer)
        } else {
            AccountMeta::new_readonly(*info.key, signer)
        });
    }
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    // The relay rides AFTER the request, unconditionally three bytes wide, and
    // never inside it: the caller-authority seeds end in a digest of the
    // request, so a bump carried within would change its own address. Custody's
    // `split_caller_authority_bump_v1` reads it back at the width it finds, and
    // a zero slot means "unmined" and is searched for.
    let mut data = Vec::with_capacity(request_bytes.len() + 3);
    data.extend_from_slice(request_bytes);
    data.push(bump);
    data.extend_from_slice(&relay);
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data,
    };
    let mut infos = Vec::with_capacity(TRANSFER_FRAME + 1);
    infos.extend(frame.iter().cloned());
    infos.push(custody_program.clone());
    let seeds = caller_seeds(custody, request_digest)?;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(child_refused_v1)?;
    Ok(())
}

/// The receipt Custody returned, and the replay it left behind.
#[inline(never)]
fn authenticate_child_result(
    accounts: &[AccountInfo<'_>],
    request: DelegatedCustodyRequestV2,
    request_digest: [u8; 32],
    resulting_revision: u64,
) -> Result<[u8; 32], ProgramError> {
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let replay_account = account(accounts, CUSTODY_REPLAY)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *custody_program.key || receipt_bytes.len() != DELEGATED_CUSTODY_RECEIPT_BYTES_V2
    {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt = DelegatedCustodyReceiptV2::decode(&receipt_bytes)
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let data = replay_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let replay_digest = hash(&data).to_bytes();
    let replay = CustodyReplayV1::decode(&data).map_err(|_| TradingSbfError::AccountData)?;
    receipt
        .custody
        .verify_for(request.custody, request_digest, replay_digest)
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    if receipt.starts_atomic_debit != request.starts_atomic_debit
        || receipt.terminal != request.terminal
        || receipt.delegate_before != request.delegate_before
        || receipt.delegate_after != request.delegate_after
        || receipt.total_debit != request.total_debit
        || receipt.allowance_before != request.allowance_before
        || receipt.allowance_after != request.allowance_after
        || replay.next_revision != resulting_revision
        || replay.last_request_digest != request_digest
        || replay.last_poststate_commitment != receipt.custody.evidence.poststate_commitment
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(receipt.custody.evidence.poststate_commitment)
}

#[inline(never)]
fn emit_receipt(
    accounts: &[AccountInfo<'_>],
    request_digest: [u8; 32],
    request: DirectFeeSettlementRequestV1,
    settlement: SettlementV1,
) -> ProgramResult {
    let receipt = DirectFeeSettlementReceiptV1 {
        request_digest,
        market: request.market,
        maker: request.maker,
        maker_root: settlement.obligation.maker_root.to_bytes(),
        custody_replay: account(accounts, CUSTODY_REPLAY)?.key.to_bytes(),
        fee_source: account(accounts, FEE_SOURCE)?.key.to_bytes(),
        fee_destination: account(accounts, FEE_DESTINATION)?.key.to_bytes(),
        fee_recipient: settlement.obligation.fee_recipient,
        custody_request_digest: settlement.custody_request_digest,
        custody_poststate: settlement.custody_poststate,
        settled_amount: settlement.obligation.fee_owed,
        expected_revision: settlement.expected_revision,
        resulting_revision: settlement.resulting_revision,
    }
    .to_bytes()
    .map_err(|_| TradingSbfError::Width)?;
    set_return_data(&receipt);
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every coordinate of the nineteen has a role, and none of them is a signer.
    ///
    /// The `None` arm is what makes an unnamed coordinate refuse; this pins that
    /// the arm is unreachable inside the frame and reachable outside it, so a
    /// widening that forgot to name its new coordinate fails here.
    #[test]
    fn every_frame_coordinate_is_named_and_nothing_past_the_frame_is() {
        for index in 0..DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1 {
            assert!(frame_role(index).is_some(), "coordinate {index} unnamed");
        }
        for index in [
            DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1,
            DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1 + 1,
            usize::MAX,
        ] {
            assert!(frame_role(index).is_none(), "coordinate {index} named");
        }
    }

    /// The four coordinates this route or its child writes, and only those.
    #[test]
    fn exactly_the_four_written_coordinates_demand_writability() {
        let written = (0..DIRECT_FEE_SETTLEMENT_ACCOUNT_COUNT_V1)
            .filter(|index| matches!(frame_role(*index), Some(FrameRoleV3::Written)))
            .collect::<Vec<_>>();
        assert_eq!(
            written,
            [CUSTODY_REPLAY, FEE_SOURCE, FEE_DESTINATION, MAKER_REPLAY]
        );
    }

    /// The Custody Transfer frame's own writable set is a SUBSET of this
    /// route's `Written` roles inside the first fourteen coordinates.
    ///
    /// Not equality: the child's spec is the authority on what it needs, and
    /// this route may not name a coordinate `Read` that the child writes. Read
    /// the other way it would be wrong -- this route writes the maker replay,
    /// which is past the child's frame entirely.
    #[test]
    fn the_child_frames_writable_coordinates_are_all_written_here() {
        let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
        assert_eq!(usize::from(spec.account_count()), TRANSFER_FRAME);
        for index in 0..TRANSFER_FRAME {
            let declared = spec
                .account(u16::try_from(index).expect("frame index"))
                .expect("declared coordinate")
                .privileges();
            if declared.writable() {
                assert!(
                    matches!(frame_role(index), Some(FrameRoleV3::Written)),
                    "coordinate {index} is writable in the child frame and not here",
                );
            }
            if declared.executable() {
                assert!(
                    matches!(frame_role(index), Some(FrameRoleV3::Program)),
                    "coordinate {index} is executable in the child frame and not here",
                );
            }
        }
    }
}
