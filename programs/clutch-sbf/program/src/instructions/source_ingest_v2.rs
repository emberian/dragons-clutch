//! Permissionless construction and extension of **v2** authenticated source
//! state.
//!
//! The v2 twin of [`super::source_ingest`], and deliberately the same four
//! steps in the same order: construct the immutable spec and its feed head,
//! construct the exact-window archive, append the uniquely admitted next
//! record, seal. What changes is *where the value comes from*.
//!
//! ## The one real difference
//!
//! V1 binds an immutable price **data account** and re-reads it. A pull oracle
//! has no such account: the price arrives in a caller-created, ephemeral,
//! closable account that the provider's receiver program wrote earlier *in the
//! same transaction*. So [`Intent::AppendSourceArchiveV2`] carries no value, no
//! confidence, and no bucket — nothing but which Terms window is being
//! extended — and the whole join is account evidence:
//!
//! * the bucket comes from the archive's own cursor;
//! * the release comes from the registry compiled into this ELF;
//! * the spec comes from the authenticated immutable account;
//! * the *adjacency* — that the immediately preceding instruction invoked the
//!   pinned receiver naming this exact update account — is read out of the
//!   Instructions sysvar, never asserted.
//!
//! [`crate::source_v2::auth::authenticate_pull_update_v2`] owns that join and is
//! complete. This module owns only the account plane it is handed and the
//! replay/PDA/Terms discipline around it.
//!
//! ## Registry, not availability
//!
//! Every route here asks [`crate::source_identity::select_release`] and refuses
//! [`ClutchError::SourceReleaseUnavailable`] when it answers `None`. That is the
//! same closed byte-equality registry the collateral boundary asks, so a market
//! whose spec this ELF does not carry cannot construct, extend, or seal a v2
//! archive any more than it can take custody.
//!
//! ## Account planes
//!
//! | intent | tag | accounts |
//! | --- | ---: | ---: |
//! | [`Intent::InitSourceSpecV2`] | 70 | 6 |
//! | [`Intent::InitSourceArchiveV2`] | 71 | 7 |
//! | [`Intent::AppendSourceArchiveV2`] | 72 | 10 |
//! | [`Intent::SealSourceArchiveV2`] | 73 | 4 |
//!
//! The append plane is the wide one and it is wide for a reason every entry can
//! name: three accounts pin the receiver deployment (program, ProgramData,
//! `Config`), one carries the ephemeral update, one is the Instructions sysvar
//! the adjacency is read from, one is canonical Clock, and the remaining four
//! are the Terms/spec/feed/archive plane every source route already carries.
//! There is deliberately **no payer**: an append moves no lamports and creates
//! nothing, exactly as the V1 append does not.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::source_ingest::{
    initial_feed_summary, read_frozen_terms, read_initial_feed, require_readonly, verify_spec_v2,
    FrozenSourceTerms,
};
use crate::instructions::{construction, genesis};
use crate::loader_state::LoaderAccountViewV1;
use crate::pyth_receiver::PriceUpdateAccountViewV1;
use crate::seeds;
use crate::source_archive_v2::{
    self, AccountViewV2, SealedArchiveReceiptV2, VerifiedSourceSpecV2,
    SOURCE_ARCHIVE_ACCOUNT_V2_BYTES, SOURCE_SPEC_ACCOUNT_V2_BYTES,
};
use crate::source_identity::{select_release, PullReleaseV2};
use crate::source_v2::auth::{
    decode_clock_view, AccountViewV2 as AuthAccountView, PullAuthenticationV2,
};
use crate::source_v2::spec::{SourceSpecV2, SOURCE_SPEC_V2_BYTES};
use clutch_solana_layout::{account_len, FeedAccount, Hash32, Intent, SOURCE_SPEC_BODY_V2_BYTES};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/* The layout crate owns the fixed instruction width; this crate owns what the
 * bytes mean.  Neither may drift from the other without a build failure. */
const _: () = assert!(SOURCE_SPEC_BODY_V2_BYTES == SOURCE_SPEC_V2_BYTES);

/// `InitSourceSpecV2` accounts: payer, SourceSpec target, Feed target, Terms,
/// System, Rent.
pub const INIT_SOURCE_SPEC_V2_ACCOUNT_COUNT: usize = 6;
/// `InitSourceArchiveV2` accounts: payer, SourceSpec, Feed, Terms, archive
/// target, System, Rent.
pub const INIT_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT: usize = 7;
/// `AppendSourceArchiveV2` accounts: SourceSpec, Feed, Terms, archive, receiver
/// program, receiver ProgramData, receiver `Config`, ephemeral price update,
/// Instructions sysvar, Clock.
pub const APPEND_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT: usize = 10;
/// `SealSourceArchiveV2` accounts: SourceSpec, Feed, Terms, archive.
pub const SEAL_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT: usize = 4;

const IX_INIT_PAYER: usize = 0;
const IX_INIT_SPEC_TARGET: usize = 1;
const IX_INIT_FEED_TARGET: usize = 2;
const IX_INIT_TERMS: usize = 3;
const IX_INIT_SYSTEM: usize = 4;
const IX_INIT_RENT: usize = 5;

const IX_ARCHIVE_PAYER: usize = 0;
const IX_ARCHIVE_SPEC: usize = 1;
const IX_ARCHIVE_FEED: usize = 2;
const IX_ARCHIVE_TERMS: usize = 3;
const IX_ARCHIVE_TARGET: usize = 4;
const IX_ARCHIVE_SYSTEM: usize = 5;
const IX_ARCHIVE_RENT: usize = 6;

/* The append and seal planes share their first four positions, so a caller
 * builds one prefix and a reader checks one set of roles. */
const IX_MUTATE_SPEC: usize = 0;
const IX_MUTATE_FEED: usize = 1;
const IX_MUTATE_TERMS: usize = 2;
const IX_MUTATE_ARCHIVE: usize = 3;
const IX_APPEND_RECEIVER_PROGRAM: usize = 4;
const IX_APPEND_RECEIVER_PROGRAMDATA: usize = 5;
const IX_APPEND_RECEIVER_CONFIG: usize = 6;
const IX_APPEND_UPDATE: usize = 7;
const IX_APPEND_INSTRUCTIONS: usize = 8;
const IX_APPEND_CLOCK: usize = 9;

/// Route exactly the four v2 authenticated-source intents.
#[inline(never)]
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::InitSourceSpecV2 { terms, spec_body }) => {
            init_source_spec_v2(program_id, accounts, request.sequence, terms, &spec_body)
        }
        Action::Layout(Intent::InitSourceArchiveV2 { terms }) => {
            init_source_archive_v2(program_id, accounts, request.sequence, terms)
        }
        Action::Layout(Intent::AppendSourceArchiveV2 { terms }) => {
            append_source_archive_v2(program_id, accounts, request.sequence, terms)
        }
        Action::Layout(Intent::SealSourceArchiveV2 { terms }) => {
            seal_source_archive_v2(program_id, accounts, request.sequence, terms)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

fn archive_refusal(_: source_archive_v2::ArchiveV2Error) -> Refusal {
    Refusal::Adapter(ClutchError::SourceAdmissionFailed)
}

/// Ask the closed compiled registry, and refuse rather than adapt.
fn registered_release(spec: SourceSpecV2) -> Outcome<PullReleaseV2> {
    select_release(spec).ok_or_else(|| ClutchError::SourceReleaseUnavailable.into())
}

/// Decode one canonical v2 body presented in instruction bytes.
///
/// The body's own codec is the only reader: field ranges, reserved padding, the
/// magic and the schema version are all its business, and a body that decodes
/// here still has to hash to the Terms feed identity before anything is built.
fn decode_spec_body(body: &[u8; SOURCE_SPEC_BODY_V2_BYTES]) -> Outcome<SourceSpecV2> {
    SourceSpecV2::decode_canonical(body)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))
}

/// The v2 counterpart of `source_ingest::bind_spec`, in the construction
/// direction.
///
/// At construction there is no stored account to re-derive the identity from,
/// so the identity is recomputed here from the presented body and compared to
/// the Terms feed. Because the two generations hash under disjoint domains, a
/// Terms account naming a V1 feed can never be satisfied by any v2 body.
fn bind_constructed_spec(spec: SourceSpecV2, terms: FrozenSourceTerms) -> Outcome<()> {
    let fields = spec.fields();
    require(
        terms.feed.bytes() == spec.feed_id(),
        ClutchError::SourceAdmissionFailed,
    )?;
    require(
        terms.source_adapter_id.bytes() == fields.source_adapter_id
            && terms.source_version == fields.source_adapter_version,
        ClutchError::SourceAdmissionFailed,
    )
}

#[inline(never)]
fn init_source_spec_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
    body: &[u8; SOURCE_SPEC_BODY_V2_BYTES],
) -> Outcome<()> {
    require_count(accounts, INIT_SOURCE_SPEC_V2_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_INIT_PAYER])?;
    require(
        accounts[IX_INIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    genesis::require_system_program(&accounts[IX_INIT_SYSTEM])?;
    let rent = genesis::read_rent(&accounts[IX_INIT_RENT])?;
    let terms = read_frozen_terms(program_id, &accounts[IX_INIT_TERMS], intent_terms)?;
    let spec = decode_spec_body(body)?;
    bind_constructed_spec(spec, terms)?;
    /* The registry gate is here rather than only at append: an ELF that cannot
     * ever authenticate this release should not be able to found its state
     * either, and a founded-but-inert spec is a rent obligation with no route
     * out. */
    registered_release(spec)?;

    let (spec_address, spec_bump) = seeds::source_spec_pda(program_id, &terms.feed.bytes());
    let (feed_address, feed_bump) = seeds::feed_pda(program_id, &terms.feed.bytes());
    expect_pda(
        accounts[IX_INIT_SPEC_TARGET].key,
        (spec_address, spec_bump),
        None,
    )?;
    expect_pda(
        accounts[IX_INIT_FEED_TARGET].key,
        (feed_address, feed_bump),
        None,
    )?;
    construction::require_absent_target(&accounts[IX_INIT_SPEC_TARGET])?;
    construction::require_absent_target(&accounts[IX_INIT_FEED_TARGET])?;

    let mut spec_image = [0_u8; SOURCE_SPEC_ACCOUNT_V2_BYTES];
    source_archive_v2::initialize_source_spec_v2_account(&mut spec_image, spec, spec_bump)
        .map_err(archive_refusal)?;

    let spec_bump_seed = [spec_bump];
    genesis::create_pda_account(
        program_id,
        &accounts[IX_INIT_PAYER],
        &accounts[IX_INIT_SPEC_TARGET],
        &accounts[IX_INIT_SYSTEM],
        &rent,
        SOURCE_SPEC_ACCOUNT_V2_BYTES,
        &[
            seeds::SEED_SOURCE_SPEC,
            &terms.feed.bytes(),
            &spec_bump_seed,
        ],
    )?;
    let feed_bump_seed = [feed_bump];
    genesis::create_pda_account(
        program_id,
        &accounts[IX_INIT_PAYER],
        &accounts[IX_INIT_FEED_TARGET],
        &accounts[IX_INIT_SYSTEM],
        &rent,
        account_len::FEED,
        &[seeds::SEED_FEED, &terms.feed.bytes(), &feed_bump_seed],
    )?;

    accounts[IX_INIT_SPEC_TARGET]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&spec_image);
    let feed = FeedAccount {
        feed: terms.feed,
        realm: terms.realm,
        cursor: terms.window.start_bucket(),
        next_boundary: terms.window.start_bucket(),
        archive_pages: 0,
        summary: initial_feed_summary(terms),
        stored_bump: feed_bump,
        flags: 0,
    };
    feed.encode(
        &mut accounts[IX_INIT_FEED_TARGET]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    Ok(())
}

#[inline(never)]
fn init_source_archive_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, INIT_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_ARCHIVE_PAYER])?;
    require(
        accounts[IX_ARCHIVE_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    genesis::require_system_program(&accounts[IX_ARCHIVE_SYSTEM])?;
    let rent = genesis::read_rent(&accounts[IX_ARCHIVE_RENT])?;
    let terms = read_frozen_terms(program_id, &accounts[IX_ARCHIVE_TERMS], intent_terms)?;
    let verified_spec = verify_spec_v2(program_id, &accounts[IX_ARCHIVE_SPEC], terms)?;
    let release = registered_release(verified_spec.spec())?;
    read_initial_feed(program_id, &accounts[IX_ARCHIVE_FEED], terms, false)?;

    let window_id = source_archive_v2::canonical_window_id(terms.window);
    let (archive_address, archive_bump) =
        seeds::source_archive_pda(program_id, &terms.feed.bytes(), &window_id.bytes());
    expect_pda(
        accounts[IX_ARCHIVE_TARGET].key,
        (archive_address, archive_bump),
        None,
    )?;
    construction::require_absent_target(&accounts[IX_ARCHIVE_TARGET])?;

    let mut archive_image = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
    source_archive_v2::initialize_genesis_archive_v2(
        &mut archive_image,
        verified_spec,
        release,
        terms.window,
        archive_bump,
    )
    .map_err(archive_refusal)?;
    let archive_bump_seed = [archive_bump];
    genesis::create_pda_account(
        program_id,
        &accounts[IX_ARCHIVE_PAYER],
        &accounts[IX_ARCHIVE_TARGET],
        &accounts[IX_ARCHIVE_SYSTEM],
        &rent,
        SOURCE_ARCHIVE_ACCOUNT_V2_BYTES,
        &[
            seeds::SEED_SOURCE_ARCHIVE,
            &terms.feed.bytes(),
            &window_id.bytes(),
            &archive_bump_seed,
        ],
    )?;
    accounts[IX_ARCHIVE_TARGET]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&archive_image);
    Ok(())
}

/// Authenticate the shared Terms/spec/feed/archive prefix both mutating routes
/// carry, and return the archive's canonical bump.
///
/// Kept out of each caller's frame on purpose: the Terms decode is 1,656 bytes
/// of hostile input and the append's own frame already holds ten account views.
#[inline(never)]
fn mutate_prefix(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_terms: Hash32,
) -> Outcome<(
    FrozenSourceTerms,
    VerifiedSourceSpecV2,
    PullReleaseV2,
    Hash32,
    u8,
)> {
    let terms = read_frozen_terms(program_id, &accounts[IX_MUTATE_TERMS], intent_terms)?;
    let verified_spec = verify_spec_v2(program_id, &accounts[IX_MUTATE_SPEC], terms)?;
    let release = registered_release(verified_spec.spec())?;
    let window_id = source_archive_v2::canonical_window_id(terms.window);
    let (archive_address, archive_bump) =
        seeds::source_archive_pda(program_id, &terms.feed.bytes(), &window_id.bytes());
    expect_pda(
        accounts[IX_MUTATE_ARCHIVE].key,
        (archive_address, archive_bump),
        None,
    )?;
    require(
        accounts[IX_MUTATE_ARCHIVE].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        accounts[IX_MUTATE_ARCHIVE].is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !accounts[IX_MUTATE_ARCHIVE].executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        accounts[IX_MUTATE_ARCHIVE].data_len() == SOURCE_ARCHIVE_ACCOUNT_V2_BYTES,
        ClutchError::WrongDataLength,
    )?;
    Ok((terms, verified_spec, release, window_id, archive_bump))
}

#[inline(never)]
fn append_source_archive_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, APPEND_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    require_readonly(&accounts[IX_APPEND_RECEIVER_PROGRAM])?;
    require_readonly(&accounts[IX_APPEND_RECEIVER_PROGRAMDATA])?;
    require_readonly(&accounts[IX_APPEND_RECEIVER_CONFIG])?;
    require_readonly(&accounts[IX_APPEND_UPDATE])?;
    require_readonly(&accounts[IX_APPEND_INSTRUCTIONS])?;
    require_readonly(&accounts[IX_APPEND_CLOCK])?;
    let (terms, verified_spec, release, _window_id, _bump) =
        mutate_prefix(program_id, accounts, intent_terms)?;
    read_initial_feed(program_id, &accounts[IX_MUTATE_FEED], terms, false)?;
    append_authenticated(accounts, sequence, verified_spec, release, terms)
}

/// Assemble the pull-authentication plane and hand it to the kernel.
///
/// Every borrow the join needs is taken here, in one frame, and dropped when it
/// returns. Nothing in this function decides anything: the release, the spec and
/// the bucket are all overwritten by the kernel from authenticated state, and
/// the account views carry runtime metadata rather than caller claims.
#[inline(never)]
fn append_authenticated(
    accounts: &[AccountInfo],
    sequence: u64,
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    terms: FrozenSourceTerms,
) -> Outcome<()> {
    let program_data = accounts[IX_APPEND_RECEIVER_PROGRAM].data.borrow();
    let programdata_data = accounts[IX_APPEND_RECEIVER_PROGRAMDATA].data.borrow();
    let config_data = accounts[IX_APPEND_RECEIVER_CONFIG].data.borrow();
    let update_data = accounts[IX_APPEND_UPDATE].data.borrow();
    let instructions_data = accounts[IX_APPEND_INSTRUCTIONS].data.borrow();
    let clock_data = accounts[IX_APPEND_CLOCK].data.borrow();

    let clock = decode_clock_view(AuthAccountView::new(
        accounts[IX_APPEND_CLOCK].key.to_bytes(),
        accounts[IX_APPEND_CLOCK].owner.to_bytes(),
        accounts[IX_APPEND_CLOCK].executable,
        &clock_data,
    ))
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;

    let auth = PullAuthenticationV2 {
        /* Overwritten by `append_authenticated_v2` from authenticated state;
         * present only because the struct has no partial form. */
        release,
        spec: verified_spec.spec(),
        receiver_program: LoaderAccountViewV1::new(
            accounts[IX_APPEND_RECEIVER_PROGRAM].key.to_bytes(),
            accounts[IX_APPEND_RECEIVER_PROGRAM].owner.to_bytes(),
            accounts[IX_APPEND_RECEIVER_PROGRAM].executable,
            &program_data,
        ),
        receiver_programdata: LoaderAccountViewV1::new(
            accounts[IX_APPEND_RECEIVER_PROGRAMDATA].key.to_bytes(),
            accounts[IX_APPEND_RECEIVER_PROGRAMDATA].owner.to_bytes(),
            accounts[IX_APPEND_RECEIVER_PROGRAMDATA].executable,
            &programdata_data,
        ),
        receiver_config: AuthAccountView::new(
            accounts[IX_APPEND_RECEIVER_CONFIG].key.to_bytes(),
            accounts[IX_APPEND_RECEIVER_CONFIG].owner.to_bytes(),
            accounts[IX_APPEND_RECEIVER_CONFIG].executable,
            &config_data,
        ),
        update: PriceUpdateAccountViewV1::new(
            accounts[IX_APPEND_UPDATE].key.to_bytes(),
            accounts[IX_APPEND_UPDATE].owner.to_bytes(),
            accounts[IX_APPEND_UPDATE].executable,
            &update_data,
        ),
        instructions_sysvar: AuthAccountView::new(
            accounts[IX_APPEND_INSTRUCTIONS].key.to_bytes(),
            accounts[IX_APPEND_INSTRUCTIONS].owner.to_bytes(),
            accounts[IX_APPEND_INSTRUCTIONS].executable,
            &instructions_data,
        ),
        clock,
        bucket: 0,
    };

    let mut archive = accounts[IX_MUTATE_ARCHIVE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    /* The replay nonce is the page's own record count, read out of
     * authenticated state rather than accepted, so a resubmitted append is a
     * refusal instead of a duplicated boundary. */
    let expected_sequence =
        source_archive_v2::open_archive_v2_sequence(&archive, verified_spec, release, terms.window)
            .map_err(archive_refusal)?;
    require(sequence == expected_sequence, ClutchError::Replay)?;
    source_archive_v2::append_authenticated_v2(
        &mut archive,
        verified_spec,
        release,
        terms.window,
        auth,
    )
    .map_err(archive_refusal)?;
    Ok(())
}

#[inline(never)]
fn seal_source_archive_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, SEAL_SOURCE_ARCHIVE_V2_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    let (terms, verified_spec, release, window_id, archive_bump) =
        mutate_prefix(program_id, accounts, intent_terms)?;
    let mut feed = read_initial_feed(program_id, &accounts[IX_MUTATE_FEED], terms, true)?;

    {
        let mut archive = accounts[IX_MUTATE_ARCHIVE]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let expected_sequence = source_archive_v2::open_archive_v2_sequence(
            &archive,
            verified_spec,
            release,
            terms.window,
        )
        .map_err(archive_refusal)?;
        require(sequence == expected_sequence, ClutchError::Replay)?;
        /* No Clock is presented and none is needed.  V1's seal demands a
         * bucket-`end` reading as its maturity witness and therefore a trusted
         * clock to judge it; v2's maturity is already discharged, per record,
         * by the crossing rule's boundary-plus-grace admission, and a page one
         * record short of its window cannot be sealed at all. */
        source_archive_v2::seal_archive_v2(&mut archive, verified_spec, release, terms.window)
            .map_err(archive_refusal)?;
    }

    let archive_data = accounts[IX_MUTATE_ARCHIVE].data.borrow();
    let receipt = source_archive_v2::verify_recorded_sealed_archive_v2(
        program_id.to_bytes(),
        accounts[IX_MUTATE_ARCHIVE].key.to_bytes(),
        AccountViewV2::new(
            accounts[IX_MUTATE_ARCHIVE].key.to_bytes(),
            accounts[IX_MUTATE_ARCHIVE].owner.to_bytes(),
            accounts[IX_MUTATE_ARCHIVE].executable,
            &archive_data,
        ),
        verified_spec,
        release,
        terms.window,
    )
    .map_err(archive_refusal)?;
    require(
        receipt.stored_bump() == archive_bump
            && receipt.feed() == terms.feed
            && receipt.window() == window_id,
        ClutchError::MismatchedState,
    )?;
    apply_sealed_feed_v2(&mut feed, receipt);
    feed.encode(
        &mut accounts[IX_MUTATE_FEED]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    Ok(())
}

/// Advance the feed head to what the sealed v2 page fixed.
///
/// The cursor is the page's own `sealed_feed_cursor`, which for v2 is the
/// window end rather than the maturity bucket: there is no bucket-`end` record,
/// because v2 does not take one. The resolution join reads that difference
/// through
/// [`crate::source_generation::SealedArchiveBindingV1::window_has_matured`]
/// rather than by comparing the raw number against a V1 expectation.
fn apply_sealed_feed_v2(feed: &mut FeedAccount, receipt: SealedArchiveReceiptV2) {
    feed.cursor = receipt.sealed_feed_cursor();
    feed.next_boundary = receipt.sealed_feed_cursor();
    feed.archive_pages = 1;
    feed.summary = receipt.page_commitment();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_identity::fixture;
    use crate::source_v2::crossing::SELECTION_CROSSING_V1;
    use crate::source_v2::spec::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
    };

    fn fields() -> SourceSpecFieldsV2 {
        SourceSpecFieldsV2 {
            source_adapter_id: fixture::SOURCE_ADAPTER_ID,
            source_adapter_version: fixture::SOURCE_ADAPTER_VERSION,
            parser_id: fixture::PARSER_ID,
            parser_version: fixture::PARSER_VERSION,
            receiver_program: fixture::RECEIVER_PROGRAM,
            receiver_programdata: fixture::RECEIVER_PROGRAMDATA,
            receiver_config: fixture::RECEIVER_CONFIG,
            config_digest: [0x7c; 32],
            provider_feed_id: fixture::PROVIDER_FEED_ID,
            programdata_deployment_slot: fixture::PROGRAMDATA_DEPLOYMENT_SLOT,
            base_asset_id: fixture::BASE_ASSET_ID,
            quote_asset_id: fixture::QUOTE_ASSET_ID,
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 7,
            grid_version: 1,
            grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
            bucket_seconds: 60,
            boundary_grace_seconds: 5,
            max_staleness_slots: 500,
            max_staleness_seconds: 600,
            max_future_seconds: 15,
            max_confidence_atoms: 1_000_000_000_000,
            max_confidence_bps: 500,
            confidence_multiplier: 3,
            selection_rule: SELECTION_CROSSING_V1,
        }
    }

    #[test]
    fn the_wire_body_width_is_the_codec_width() {
        assert_eq!(SOURCE_SPEC_BODY_V2_BYTES, 368);
        assert_eq!(SOURCE_SPEC_BODY_V2_BYTES, SOURCE_SPEC_V2_BYTES);
    }

    #[test]
    fn a_wire_body_decodes_to_the_spec_it_encodes() {
        let spec = SourceSpecV2::new(fields()).expect("the fixture identity is a valid spec");
        let body = spec.encode_canonical();
        assert_eq!(decode_spec_body(&body).expect("body decodes"), spec);
    }

    #[test]
    fn the_registry_admits_the_fixture_release_and_nothing_beside_it() {
        let spec = SourceSpecV2::new(fields()).expect("the fixture identity is a valid spec");
        assert!(registered_release(spec).is_ok());

        let mut other = fields();
        other.parser_version = fixture::PARSER_VERSION + 1;
        let unregistered = SourceSpecV2::new(other).expect("a well-formed unregistered spec");
        assert!(registered_release(unregistered).is_err());
    }

    #[test]
    fn a_v1_body_is_not_a_v2_body() {
        /* The V1 body is 256 bytes and this decoder takes exactly 368, so the
         * generations cannot be confused at the construction wire even before
         * the digest domains are consulted.  A zero-filled 368-byte buffer is
         * the closest a caller gets, and it is refused. */
        assert!(decode_spec_body(&[0_u8; SOURCE_SPEC_BODY_V2_BYTES]).is_err());
    }
}
