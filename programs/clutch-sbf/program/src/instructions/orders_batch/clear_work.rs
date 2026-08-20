//! Staged creation of the clearing checkpoint — Tier 2 join 6, creation half.
//!
//! `Intent::InitClearWork` (tag 47) and `Intent::GrowClearWork` (tag 48).
//!
//! The [`clutch_solana_layout::clearing::ClearWorkAccount`] is 50,054 bytes —
//! the one account in the inventory above the runtime's 10,240-byte
//! per-instruction growth ceiling, so no single system-program CPI can
//! allocate it (`SOLANA_LAYOUT.md`'s staged-creation analysis; the layout
//! test `only_the_checkpoint_exceeds_the_cpi_creation_ceiling` pins the
//! arithmetic).  Of the two real paths, this module implements the one the
//! analysis recommends: **CPI create then realloc, checkpoint stays a PDA** —
//! authentication over ergonomics, because a keypair-addressed checkpoint is
//! substitutable and a PDA at `(epoch, candidate)` is not.
//!
//! ## The five-instruction protocol
//!
//! 1. `InitClearWork { market, epoch, candidate }` — the ResolutionWork
//!    creation shape verbatim ([`create_first_stage`]): refuse a non-creatable
//!    target, transfer the **full final rent principal** for all 50,054 bytes
//!    (so no later grow ever tops up), `Allocate` the first
//!    [`clearing::CLEAR_WORK_GROW_STEP`] bytes and `Assign` under the PDA
//!    seeds, post-checked.  Then write the resumable grow-stage prefix
//!    ([`clearing::init_clear_work_grow_stage`]): tag, version, `GROWING`
//!    marker, `target_len`, the identity triple, and the bump.
//! 2. `GrowClearWork` ×4 — [`AccountInfo::resize`] to
//!    `min(len + step, target)` (the shape `direct_selection_v3`'s account
//!    close already uses).  The growth cap is per *instruction*, so all five
//!    may sit in one transaction; nothing here requires that, and a partial
//!    sequence resumes from its stored prefix.  The **final** grow finishes
//!    creation atomically in its own instruction:
//!    [`clearing::init_clear_work`] writes the real header, then
//!    [`ClearWorkV1::encode_idle_into`] writes the idle body — so there is no
//!    observable state that is full-length but headerless.
//!
//! ## Why a half-grown account is economically inert
//!
//! By construction: every checkpoint reader and writer in the layout crate
//! decodes through the exact-length rule
//! ([`clutch_solana_layout::clearing::ClearWorkHeader::decode`] refuses any
//! length but `account_len::CLEAR_WORK`), and a growing account's length is a
//! strict multiple of the step below the target — the two length sets are
//! disjoint by a `const` assertion in the layout crate.  The grow-stage
//! prefix adds resumability and un-repurposability, not authority.  The gate
//! also *proves* the refusal rather than argues it: the SVM test drives every
//! consuming read against a half-grown account.
//!
//! ## Replay and permissionlessness
//!
//! Creation is permissionless keeper work, like the V3 deadline transitions:
//! the payer signs `InitClearWork` because lamports move; `GrowClearWork`
//! needs no signer at all — its authority is the account's own state.  The
//! replay counter is state, not caller assertion: init requires the envelope
//! sequence `0` (the account's non-existence is the real guard), and each
//! grow requires the sequence to equal the account's current stage index
//! `len / step` (1..=4), exactly the page-count idiom `PlaceOrder` uses.
//!
//! ## Rent
//!
//! `(128 + 50,054) × 6,960 = 349,266,720` lamports (~0.349 SOL) moves once, at
//! init, on top of any prefund.  Prefund-safe per the repo's constructor
//! rule: anyone may transfer SOL to the predictable PDA first; a donation
//! never discounts the payer's principal and never blocks creation
//! ([`require_creatable`] deliberately ignores lamports).
//!
//! ## The candidate feed mirror
//!
//! [`create_candidate_feed_account`] is the same creation discipline for the
//! 6,266-byte [`clutch_solana_layout::clearing::CandidateFeedAccount`], which
//! fits one allocation, so it is exactly the `SubmitDirectPage`/Direct-V4
//! full-principal constructor pointed at the feed PDA.  T2-7's general
//! `SubmitCandidate` is the intended caller; it lands here so both clearing
//! accounts' creation machinery lives in one reviewed place.
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), no promotion claims: this module creates accounts and
//! nothing consumes them yet (`AdvanceClearWork` is T2-6b).  The reference
//! adapter refuses both intents with `UnsupportedIntent`, so the SVM oracle
//! for this family is the layout codec byte-for-byte, per the genesis
//! precedent.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::direct_selection_v3::{
    create_pda_account_full_principal, direct_creation_funding, DIRECT_NEUTRAL_SINK_V3,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_batch::relation_v1_stream::ClearWorkV1;
use clutch_solana_layout::{account_len, clearing, Hash32};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts in an `InitClearWork` instruction, exactly.
pub const INIT_CLEAR_WORK_ACCOUNT_COUNT: usize = 4;
/// Authenticated payer funding the full final rent principal.
pub const IX_CLEAR_WORK_PAYER: usize = 0;
/// The canonical, not-yet-created checkpoint PDA.
pub const IX_CLEAR_WORK_WORK: usize = 1;
/// System program.
pub const IX_CLEAR_WORK_SYSTEM: usize = 2;
/// Rent sysvar.
pub const IX_CLEAR_WORK_RENT: usize = 3;

/// Accounts in a `GrowClearWork` instruction, exactly.
///
/// One: the growing checkpoint.  A resize of an account this program owns is
/// not a CPI, no lamports move, and the authority is the account's own
/// grow-stage prefix, so there is no payer, no system program, and no signer.
pub const GROW_CLEAR_WORK_ACCOUNT_COUNT: usize = 1;
/// The growing checkpoint PDA.
pub const IX_GROW_CLEAR_WORK_WORK: usize = 0;

/// Begin staged creation: fund fully, allocate the first step, write the
/// grow-stage prefix.
#[inline(never)]
pub(super) fn init_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    market: &Hash32,
    epoch: &Hash32,
    candidate: &Hash32,
) -> Outcome<()> {
    require_count(accounts, INIT_CLEAR_WORK_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_CLEAR_WORK_PAYER])?;
    require(
        accounts[IX_CLEAR_WORK_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    require_creatable(&accounts[IX_CLEAR_WORK_WORK])?;
    require_system_program(&accounts[IX_CLEAR_WORK_SYSTEM])?;
    let rent = read_rent(&accounts[IX_CLEAR_WORK_RENT])?;
    /* The real replay guard is the target's non-existence
     * (`require_creatable` above); the zero sequence states the intent's
     * position in the protocol on the wire, as `SubmitDirectPage` does. */
    require(sequence == 0, ClutchError::Replay)?;

    let epoch_bytes = epoch.bytes();
    let candidate_bytes = candidate.bytes();
    let (address, bump) = seeds::clear_work_pda(program_id, &epoch_bytes, &candidate_bytes);
    expect_pda(accounts[IX_CLEAR_WORK_WORK].key, (address, bump), None)?;

    /* The FULL final principal, at the FIRST stage: rent for the whole
     * final account length moves here, once, so the four grows never touch
     * lamports and a grow can never strand a checkpoint under-funded. */
    let final_principal = rent.minimum_balance(account_len::CLEAR_WORK)?;
    let bump_seed = [bump];
    let signer_seeds = [
        seeds::SEED_CLEAR_WORK,
        epoch_bytes.as_ref(),
        candidate_bytes.as_ref(),
        bump_seed.as_ref(),
    ];
    create_first_stage(
        program_id,
        &accounts[IX_CLEAR_WORK_PAYER],
        &accounts[IX_CLEAR_WORK_WORK],
        &accounts[IX_CLEAR_WORK_SYSTEM],
        final_principal,
        &signer_seeds,
    )?;
    write_grow_stage(
        &accounts[IX_CLEAR_WORK_WORK],
        *market,
        *epoch,
        *candidate,
        bump,
    )
}

/// Grow a staged checkpoint by one step; the final step finishes creation.
#[inline(never)]
pub(super) fn grow_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    market: &Hash32,
    epoch: &Hash32,
    candidate: &Hash32,
) -> Outcome<()> {
    require_count(accounts, GROW_CLEAR_WORK_ACCOUNT_COUNT)?;
    let work = &accounts[IX_GROW_CLEAR_WORK_WORK];
    require(work.is_writable, ClutchError::NotWritable)?;
    require(!work.executable, ClutchError::ExecutableAccount)?;
    require(work.owner == program_id, ClutchError::WrongProgramOwner)?;

    /* The prefix authenticates the account: the staged-length rule refuses a
     * finished checkpoint (and every foreign program-owned account length),
     * the marker refuses every other family, and the stored triple must both
     * match the wire and re-derive this exact address. */
    let stage = read_grow_stage(work)?;
    require(
        stage.market == *market && stage.epoch == *epoch && stage.candidate == *candidate,
        ClutchError::MismatchedState,
    )?;
    let epoch_bytes = epoch.bytes();
    let candidate_bytes = candidate.bytes();
    expect_pda(
        work.key,
        seeds::clear_work_pda(program_id, &epoch_bytes, &candidate_bytes),
        Some(stage.stored_bump),
    )?;

    /* Replay by state: the account's own staged length is the counter, so a
     * replayed or out-of-order grow refuses against what the account already
     * is, exactly as a placement refuses against the page's order count. */
    let len = work.data_len();
    require(
        sequence == (len / clearing::CLEAR_WORK_GROW_STEP) as u64,
        ClutchError::Replay,
    )?;

    let grown = clearing::clear_work_grown_len(len);
    work.resize(grown)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(work.data_len() == grown, ClutchError::AccountCreationFailed)?;
    if grown == account_len::CLEAR_WORK {
        finish_creation(work, &stage)?;
    }
    Ok(())
}

/// The ResolutionWork creation shape, verbatim, at the checkpoint's numbers:
/// full-principal transfer for the **final** length, `Allocate` for the
/// **first** step only, `Assign`, exact post-checks.
///
/// Prefund-safe: the target moves from `B` to exactly `B + P`, so a hostile
/// donation to the predictable PDA neither blocks creation nor discounts the
/// payer's principal.
#[inline(never)]
fn create_first_stage<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    final_rent_principal: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    let before = target.lamports();
    let expected = before
        .checked_add(final_rent_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(final_rent_principal),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(clearing::CLEAR_WORK_GROW_STEP),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == expected
            && target.owner == program_id
            && target.data_len() == clearing::CLEAR_WORK_GROW_STEP,
        ClutchError::AccountCreationFailed,
    )
}

/// Write the resumable grow-stage prefix over the fresh first-stage account.
#[inline(never)]
fn write_grow_stage(
    account: &AccountInfo,
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    stored_bump: u8,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    clearing::init_clear_work_grow_stage(&mut data, market, epoch, candidate, stored_bump)?;
    Ok(())
}

/// Decode the grow-stage prefix; the borrow never outlives this frame, so
/// the caller's later [`AccountInfo::resize`] can take its own.
#[inline(never)]
fn read_grow_stage(account: &AccountInfo) -> Outcome<clearing::ClearWorkGrowStage> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(clearing::ClearWorkGrowStage::decode(&data)?)
}

/// Finish creation atomically inside the final grow: the real header, then
/// the idle body.
///
/// The idle body is `ClearWorkV1::NEW`'s byte image written in place —
/// nothing here or in `clutch-batch` ever holds the 48 KB value on a frame.
/// The `WrongDataLength` mapping is stated, not expected: `init_clear_work`
/// just proved the exact account length that `encode_idle_into` re-checks.
#[inline(never)]
fn finish_creation(account: &AccountInfo, stage: &clearing::ClearWorkGrowStage) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    clearing::init_clear_work(
        &mut data,
        stage.market,
        stage.epoch,
        stage.candidate,
        stage.stored_bump,
    )?;
    let body = clearing::clear_work_body_mut(&mut data)?;
    ClearWorkV1::encode_idle_into(body)
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    Ok(())
}

/// Create the canonical 6,266-byte candidate-feed account in one allocation.
///
/// The `InitClearWork` mirror for the account family's other member: the
/// feed fits under the CPI ceiling, so this is exactly the in-family
/// full-principal constructor (`SubmitDirectPage`'s creation shape through
/// `create_pda_account_full_principal`) pointed at the canonical
/// `(epoch, candidate)` feed PDA.  It creates the *account*; writing the
/// feed's content is [`clutch_solana_layout::clearing::init_candidate_feed`]
/// plus the fill/slice writers, which need the solver's coordinates.
///
/// T2-7 COORDINATION: the general `SubmitCandidate` lane is the intended
/// caller; no shipped instruction calls this yet, and the returned bump is
/// what that caller stamps into the feed header.
// Landed with T2-3 for T2-7 (the general SubmitCandidate lane); dead until
// that lane wires it, and the allow is what that lane deletes.
#[allow(dead_code)]
pub(crate) fn create_candidate_feed_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    feed: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    epoch: &Hash32,
    candidate: &Hash32,
) -> Outcome<u8> {
    require_creatable(feed)?;
    let epoch_bytes = epoch.bytes();
    let candidate_bytes = candidate.bytes();
    let (address, bump) = seeds::candidate_feed_pda(program_id, &epoch_bytes, &candidate_bytes);
    expect_pda(feed.key, (address, bump), None)?;
    let funding = direct_creation_funding(
        payer,
        feed,
        rent,
        account_len::CANDIDATE_FEED,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let bump_seed = [bump];
    create_pda_account_full_principal(
        program_id,
        payer,
        feed,
        system_program,
        rent,
        account_len::CANDIDATE_FEED,
        funding,
        0,
        &[
            seeds::SEED_CANDIDATE_FEED,
            epoch_bytes.as_ref(),
            candidate_bytes.as_ref(),
            bump_seed.as_ref(),
        ],
    )?;
    Ok(bump)
}
