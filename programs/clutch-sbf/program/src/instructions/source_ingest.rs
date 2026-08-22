//! Permissionless construction and mutation of authenticated source state.
//!
//! This family joins immutable sealed Terms to one content-addressed
//! [`crate::source::SourceSpecV1`], one canonical [`FeedAccount`], and one
//! exact-window source archive.  Source values never arrive in instruction
//! bytes: append and seal read the frozen source account through a
//! compile-time parser selected by a closed registry.
//!
//! The default build deliberately registers no source release.  It therefore
//! refuses every action with [`ClutchError::SourceReleaseUnavailable`] before
//! a CPI or state write.  The `non-production-mock-source` feature changes the
//! ELF and installs one deterministic laboratory parser/deployment pair for
//! local-bank lifecycle evidence.  It is not a provider ABI and is not a
//! production deployment candidate.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::{artifact, construction, genesis};
use crate::seeds;
use crate::source::{check_terms_binding, SourceSpecV1, TermsSourceBindingV1, TrustedClockV1};
#[cfg(feature = "non-production-mock-source")]
use crate::source::{PriceParserV1, SourceAccountView, SourceError};
use crate::source_archive::{
    self, ArchiveAccountViewV1, SealedArchiveReceiptV1, SourceArchiveError,
    SourceSpecAccountViewV1, VerifiedSourceSpecAccountV1, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
    SOURCE_SPEC_ACCOUNT_V1_BYTES,
};
#[cfg(feature = "non-production-mock-source")]
use crate::source_archive::{DeploymentAuthenticatorV1, RuntimeAccountViewV1};
use crate::source_archive_v2::{
    self, AccountViewV2 as SourceSpecV2AccountView, VerifiedSourceSpecV2,
};
use crate::source_v2::spec::SourceSpecV2;
use clutch_accumulator::{CoveragePolicy, FeedIdentity, Grid, WindowDomain};
use clutch_solana_layout::{
    account_len, FeedAccount, Hash32, Intent, TermsAccount, SOURCE_SPEC_BODY_V1_BYTES,
};
use clutch_solana_reference::{Action, Request, GEN_EXACT_01};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const _: () = assert!(SOURCE_SPEC_BODY_V1_BYTES == crate::source::SOURCE_SPEC_V1_BYTES);

/// InitSourceSpec accounts: payer, SourceSpec target, Feed target, Terms,
/// provider program, deployment evidence, source account, System, Rent.
pub const INIT_SOURCE_SPEC_ACCOUNT_COUNT: usize = 9;
/// InitSourceArchive accounts: payer, SourceSpec, Feed, Terms, archive target,
/// System, Rent.
pub const INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT: usize = 7;
/// Append/Seal accounts: SourceSpec, Feed, Terms, archive, provider program,
/// deployment evidence, source account, Clock.
pub const MUTATE_SOURCE_ARCHIVE_ACCOUNT_COUNT: usize = 8;

const IX_INIT_PAYER: usize = 0;
const IX_INIT_SPEC_TARGET: usize = 1;
const IX_INIT_FEED_TARGET: usize = 2;
const IX_INIT_TERMS: usize = 3;
const IX_INIT_PROVIDER: usize = 4;
const IX_INIT_DEPLOYMENT: usize = 5;
const IX_INIT_SOURCE: usize = 6;
const IX_INIT_SYSTEM: usize = 7;
const IX_INIT_RENT: usize = 8;

const IX_ARCHIVE_PAYER: usize = 0;
const IX_ARCHIVE_SPEC: usize = 1;
const IX_ARCHIVE_FEED: usize = 2;
const IX_ARCHIVE_TERMS: usize = 3;
const IX_ARCHIVE_TARGET: usize = 4;
const IX_ARCHIVE_SYSTEM: usize = 5;
const IX_ARCHIVE_RENT: usize = 6;

const IX_MUTATE_SPEC: usize = 0;
const IX_MUTATE_FEED: usize = 1;
const IX_MUTATE_TERMS: usize = 2;
const IX_MUTATE_ARCHIVE: usize = 3;
const IX_MUTATE_PROVIDER: usize = 4;
const IX_MUTATE_DEPLOYMENT: usize = 5;
const IX_MUTATE_SOURCE: usize = 6;
const IX_MUTATE_CLOCK: usize = 7;

const INITIAL_FEED_SUMMARY_DOMAIN: &[u8] = b"dragons-clutch/source-feed-empty/v1";

/// The exact facts one sealed Terms account fixes for its source plane.
///
/// Shared with [`super::source_ingest_v2`]: Terms are generation-neutral by
/// construction — they name a feed identity, an adapter and an observation
/// window, and it is the *spec* that is domain-separated — so both families
/// read them through this one reader rather than through two.
#[derive(Clone, Copy)]
pub(crate) struct FrozenSourceTerms {
    pub(crate) terms: Hash32,
    pub(crate) realm: Hash32,
    pub(crate) feed: Hash32,
    pub(crate) source_adapter_id: Hash32,
    pub(crate) source_version: u32,
    pub(crate) window: WindowDomain,
}

/// Route exactly the four authenticated-source intents.
#[inline(never)]
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::InitSourceSpec { terms, spec_body }) => {
            init_source_spec(program_id, accounts, request.sequence, terms, &spec_body)
        }
        Action::Layout(Intent::InitSourceArchive { terms }) => {
            init_source_archive(program_id, accounts, request.sequence, terms)
        }
        Action::Layout(Intent::AppendSourceArchive { terms }) => {
            append_source_archive(program_id, accounts, request.sequence, terms)
        }
        Action::Layout(Intent::SealSourceArchive { terms }) => {
            seal_source_archive(program_id, accounts, request.sequence, terms)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[inline(never)]
pub(crate) fn read_frozen_terms(
    program_id: &Pubkey,
    account: &AccountInfo,
    expected_terms: Hash32,
) -> Outcome<FrozenSourceTerms> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() == account_len::TERMS,
        ClutchError::WrongDataLength,
    )?;

    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_into(&account.data.borrow(), &mut terms)?;
    require(terms.terms == expected_terms, ClutchError::MismatchedState)?;
    expect_pda(
        account.key,
        seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    require(
        terms.repair_policy_id == u32::from(GEN_EXACT_01) && terms.repair_generation == 0,
        ClutchError::SourceAdmissionFailed,
    )?;
    let coverage_id = u16::try_from(terms.coverage_policy_id)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let coverage = CoveragePolicy::from_registry(coverage_id, terms.coverage_policy_parameter)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    require(
        coverage == CoveragePolicy::COMPLETE_REQUIRED,
        ClutchError::SourceAdmissionFailed,
    )?;
    let feed = FeedIdentity::new(
        terms.source_adapter_id.bytes(),
        terms.feed.bytes(),
        terms.source_version,
        terms.evaluator_version,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let grid = Grid::new(
        terms.grid_family_id,
        terms.grid_version,
        terms.bucket_seconds,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let maturity = terms
        .expected_start_bucket
        .checked_add(terms.maturity_horizon_buckets)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let window = WindowDomain::new(
        feed,
        grid,
        terms.expected_start_bucket,
        terms.expected_end_bucket_exclusive,
        maturity,
        terms.repair_generation,
        coverage,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    require(
        window.maturity_bucket_exclusive() == window.end_bucket_exclusive().saturating_add(1),
        ClutchError::SourceAdmissionFailed,
    )?;
    Ok(FrozenSourceTerms {
        terms: terms.terms,
        realm: terms.realm,
        feed: terms.feed,
        source_adapter_id: terms.source_adapter_id,
        source_version: terms.source_version,
        window,
    })
}

fn bind_spec(spec: SourceSpecV1, terms: FrozenSourceTerms) -> Outcome<()> {
    check_terms_binding(
        spec,
        TermsSourceBindingV1 {
            feed: terms.feed,
            source_adapter_id: terms.source_adapter_id,
            source_version: terms.source_version,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))
}

pub(crate) fn initial_feed_summary(terms: FrozenSourceTerms) -> Hash32 {
    Hash32::from_bytes(
        solana_sha256_hasher::hashv(&[
            INITIAL_FEED_SUMMARY_DOMAIN,
            &terms.feed.bytes(),
            &terms.terms.bytes(),
        ])
        .to_bytes(),
    )
}

pub(crate) fn require_readonly(account: &AccountInfo) -> Outcome<()> {
    require(!account.is_writable, ClutchError::UnexpectedWritable)
}

fn trusted_clock(account: &AccountInfo) -> Outcome<TrustedClockV1> {
    let slot = artifact::read_clock_slot(account)?;
    let data = account.data.borrow();
    let mut unix = [0_u8; 8];
    unix.copy_from_slice(&data[32..40]);
    let unix = i64::from_le_bytes(unix);
    let unix_seconds =
        u64::try_from(unix).map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    Ok(TrustedClockV1 { slot, unix_seconds })
}

#[inline(never)]
fn verify_spec(
    program_id: &Pubkey,
    account: &AccountInfo,
    terms: FrozenSourceTerms,
) -> Outcome<VerifiedSourceSpecAccountV1> {
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    let (address, bump) = seeds::source_spec_pda(program_id, &terms.feed.bytes());
    expect_pda(account.key, (address, bump), None)?;
    let data = account.data.borrow();
    let verified = source_archive::verify_source_spec_account(
        program_id.to_bytes(),
        address.to_bytes(),
        SourceSpecAccountViewV1::new(
            account.key.to_bytes(),
            account.owner.to_bytes(),
            account.executable,
            &data,
        ),
    )
    .map_err(source_refusal)?;
    require(
        verified.feed() == terms.feed && verified.stored_bump() == bump,
        ClutchError::MismatchedState,
    )?;
    bind_spec(verified.spec(), terms)?;
    Ok(verified)
}

/// Refuse value admission unless one canonical immutable SourceSpec binds the
/// market's Terms and selects a release compiled into this exact ELF.
///
/// This is deliberately a result-returning capability check, not a caller
/// supplied availability bit.  The same closed registry that dispatches every
/// source-ingest operation is the sole owner of the decision.
///
/// ## The two spec generations
///
/// The presented account's **length** selects its generation, and the two are
/// disjoint by construction — different tag byte, different digest domain,
/// different body length — so neither can be read as the other:
///
/// * V1 (292 bytes) asks the compiled [`release_registered`] predicate.  In the
///   default artifact it is empty, so a valid V1 Terms/SourceSpec pair still
///   refuses with [`ClutchError::SourceReleaseUnavailable`].
/// * v2 (404 bytes) asks [`crate::source_identity::select_release`].  The
///   default artifact carries exactly one entry — the fabricated laboratory
///   identity of [`crate::source_identity::fixture`], whose receiver program is
///   a program-derived address that no party can deploy to — so exactly one spec
///   is admitted and every other still refuses.
///
/// **The refusal boundary narrows here; it does not disappear.** That is the
/// shape `R2_PULL_PROMOTION_PLAN.md` §4 item 2 requires of any registry flip,
/// and it is why this function still exists at all.
///
/// A V1 SourceSpec PDA can be constructed onchain only by [`init_source_spec`],
/// which authenticates the compiled parser and deployment release before
/// atomically creating both SourceSpec and Feed.  A v2 one is constructed by
/// [`super::source_ingest_v2`]'s `InitSourceSpecV2`, which asks the same closed
/// registry before it creates anything.  Either way this re-authenticates
/// owner, address, body, digest, bump, Terms binding, and current registry
/// membership, so neither an old market nor an injected one can bypass the
/// registry at the collateral boundary.
#[inline(never)]
pub(crate) fn require_registered_source_for_market(
    program_id: &Pubkey,
    terms_account: &AccountInfo,
    source_spec_account: &AccountInfo,
    expected_terms: Hash32,
    expected_realm: Hash32,
    expected_feed: Hash32,
) -> Outcome<()> {
    let terms = read_frozen_terms(program_id, terms_account, expected_terms)?;
    require(
        terms.realm == expected_realm && terms.feed == expected_feed,
        ClutchError::MismatchedState,
    )?;
    if source_spec_account.data_len() == SOURCE_SPEC_ACCOUNT_V1_BYTES {
        let verified = verify_spec(program_id, source_spec_account, terms)?;
        return require(
            release_registered(verified.spec()),
            ClutchError::SourceReleaseUnavailable,
        );
    }
    let verified = verify_spec_v2(program_id, source_spec_account, terms)?;
    require(
        crate::source_identity::select_release(verified.spec()).is_some(),
        ClutchError::SourceReleaseUnavailable,
    )
}

/// Authenticate one v2 SourceSpec account against the market's frozen Terms.
///
/// The V1 counterpart is [`verify_spec`]; the two differ only in the codec they
/// hand the bytes to and in the digest domain that codec recomputes.  Both
/// derive the address from Terms rather than accepting it, so a caller cannot
/// present a spec belonging to another feed.
#[inline(never)]
pub(crate) fn verify_spec_v2(
    program_id: &Pubkey,
    account: &AccountInfo,
    terms: FrozenSourceTerms,
) -> Outcome<VerifiedSourceSpecV2> {
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    let (address, bump) = seeds::source_spec_pda(program_id, &terms.feed.bytes());
    expect_pda(account.key, (address, bump), None)?;
    let data = account.data.borrow();
    let verified = source_archive_v2::verify_source_spec_v2_account(
        program_id.to_bytes(),
        address.to_bytes(),
        SourceSpecV2AccountView::new(
            account.key.to_bytes(),
            account.owner.to_bytes(),
            account.executable,
            &data,
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    require(
        verified.feed() == terms.feed && verified.stored_bump() == bump,
        ClutchError::MismatchedState,
    )?;
    bind_spec_v2(verified.spec(), terms)?;
    Ok(verified)
}

/// The v2 counterpart of [`bind_spec`].
///
/// Terms name a feed identity, an adapter, and an adapter version. Because the
/// two generations hash their bodies under disjoint domains, a Terms account
/// naming a V1 feed can never bind a v2 spec and the reverse holds too — the
/// generations fail closed against each other without a version field to
/// compare.
fn bind_spec_v2(spec: SourceSpecV2, terms: FrozenSourceTerms) -> Outcome<()> {
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

pub(crate) fn read_initial_feed(
    program_id: &Pubkey,
    account: &AccountInfo,
    terms: FrozenSourceTerms,
    writable: bool,
) -> Outcome<FeedAccount> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        account.data_len() == account_len::FEED,
        ClutchError::WrongDataLength,
    )?;
    let feed = FeedAccount::decode(&account.data.borrow())?;
    let (address, bump) = seeds::feed_pda(program_id, &terms.feed.bytes());
    expect_pda(account.key, (address, bump), Some(feed.stored_bump))?;
    require(
        feed.feed == terms.feed
            && feed.realm == terms.realm
            && feed.cursor == terms.window.start_bucket()
            && feed.next_boundary == terms.window.start_bucket()
            && feed.archive_pages == 0
            && feed.summary == initial_feed_summary(terms),
        ClutchError::MismatchedState,
    )?;
    Ok(feed)
}

#[inline(never)]
fn init_source_spec(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
    body: &[u8; SOURCE_SPEC_BODY_V1_BYTES],
) -> Outcome<()> {
    require_count(accounts, INIT_SOURCE_SPEC_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_INIT_PAYER])?;
    require(
        accounts[IX_INIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    require_readonly(&accounts[IX_INIT_PROVIDER])?;
    require_readonly(&accounts[IX_INIT_DEPLOYMENT])?;
    require_readonly(&accounts[IX_INIT_SOURCE])?;
    genesis::require_system_program(&accounts[IX_INIT_SYSTEM])?;
    let rent = genesis::read_rent(&accounts[IX_INIT_RENT])?;
    let terms = read_frozen_terms(program_id, &accounts[IX_INIT_TERMS], intent_terms)?;
    let spec = source_archive::decode_source_spec_body_v1(body).map_err(source_refusal)?;
    bind_spec(spec, terms)?;

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

    let mut spec_image = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_registered_spec(
        &mut spec_image,
        spec,
        spec_bump,
        &accounts[IX_INIT_PROVIDER],
        &accounts[IX_INIT_DEPLOYMENT],
        &accounts[IX_INIT_SOURCE],
    )?;

    let spec_bump_seed = [spec_bump];
    genesis::create_pda_account(
        program_id,
        &accounts[IX_INIT_PAYER],
        &accounts[IX_INIT_SPEC_TARGET],
        &accounts[IX_INIT_SYSTEM],
        &rent,
        SOURCE_SPEC_ACCOUNT_V1_BYTES,
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
fn init_source_archive(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT)?;
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
    let verified_spec = verify_spec(program_id, &accounts[IX_ARCHIVE_SPEC], terms)?;
    read_initial_feed(program_id, &accounts[IX_ARCHIVE_FEED], terms, false)?;

    let window_id = source_archive::canonical_window_id(terms.window);
    let (archive_address, archive_bump) =
        seeds::source_archive_pda(program_id, &terms.feed.bytes(), &window_id.bytes());
    expect_pda(
        accounts[IX_ARCHIVE_TARGET].key,
        (archive_address, archive_bump),
        None,
    )?;
    construction::require_absent_target(&accounts[IX_ARCHIVE_TARGET])?;

    let mut archive_image = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_registered_archive(
        &mut archive_image,
        verified_spec,
        terms.window,
        archive_bump,
    )?;
    let archive_bump_seed = [archive_bump];
    genesis::create_pda_account(
        program_id,
        &accounts[IX_ARCHIVE_PAYER],
        &accounts[IX_ARCHIVE_TARGET],
        &accounts[IX_ARCHIVE_SYSTEM],
        &rent,
        SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
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

#[inline(never)]
fn append_source_archive(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, MUTATE_SOURCE_ARCHIVE_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    require_readonly(&accounts[IX_MUTATE_PROVIDER])?;
    require_readonly(&accounts[IX_MUTATE_DEPLOYMENT])?;
    require_readonly(&accounts[IX_MUTATE_SOURCE])?;
    let terms = read_frozen_terms(program_id, &accounts[IX_MUTATE_TERMS], intent_terms)?;
    let verified_spec = verify_spec(program_id, &accounts[IX_MUTATE_SPEC], terms)?;
    read_initial_feed(program_id, &accounts[IX_MUTATE_FEED], terms, false)?;
    let clock = trusted_clock(&accounts[IX_MUTATE_CLOCK])?;
    let window_id = source_archive::canonical_window_id(terms.window);
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
        accounts[IX_MUTATE_ARCHIVE].data_len() == SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    let mut archive = accounts[IX_MUTATE_ARCHIVE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let expected_sequence = registered_open_sequence(&archive, verified_spec, terms.window)?;
    require(sequence == expected_sequence, ClutchError::Replay)?;
    append_registered(
        &mut archive,
        verified_spec,
        terms.window,
        clock,
        &accounts[IX_MUTATE_PROVIDER],
        &accounts[IX_MUTATE_DEPLOYMENT],
        &accounts[IX_MUTATE_SOURCE],
    )
}

#[inline(never)]
fn seal_source_archive(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_terms: Hash32,
) -> Outcome<()> {
    require_count(accounts, MUTATE_SOURCE_ARCHIVE_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    require_readonly(&accounts[IX_MUTATE_PROVIDER])?;
    require_readonly(&accounts[IX_MUTATE_DEPLOYMENT])?;
    require_readonly(&accounts[IX_MUTATE_SOURCE])?;
    let terms = read_frozen_terms(program_id, &accounts[IX_MUTATE_TERMS], intent_terms)?;
    let verified_spec = verify_spec(program_id, &accounts[IX_MUTATE_SPEC], terms)?;
    let mut feed = read_initial_feed(program_id, &accounts[IX_MUTATE_FEED], terms, true)?;
    let clock = trusted_clock(&accounts[IX_MUTATE_CLOCK])?;
    let window_id = source_archive::canonical_window_id(terms.window);
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
        accounts[IX_MUTATE_ARCHIVE].data_len() == SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    {
        let mut archive = accounts[IX_MUTATE_ARCHIVE]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let expected_sequence = registered_open_sequence(&archive, verified_spec, terms.window)?;
        require(sequence == expected_sequence, ClutchError::Replay)?;
        seal_registered(
            &mut archive,
            verified_spec,
            terms.window,
            clock,
            &accounts[IX_MUTATE_PROVIDER],
            &accounts[IX_MUTATE_DEPLOYMENT],
            &accounts[IX_MUTATE_SOURCE],
        )?;
    }
    let archive_data = accounts[IX_MUTATE_ARCHIVE].data.borrow();
    let receipt = source_archive::verify_recorded_sealed_archive(
        program_id.to_bytes(),
        archive_address.to_bytes(),
        ArchiveAccountViewV1::new(
            accounts[IX_MUTATE_ARCHIVE].key.to_bytes(),
            accounts[IX_MUTATE_ARCHIVE].owner.to_bytes(),
            accounts[IX_MUTATE_ARCHIVE].executable,
            &archive_data,
        ),
        verified_spec,
        terms.window,
    )
    .map_err(source_refusal)?;
    require(
        receipt.stored_bump() == archive_bump
            && receipt.feed() == terms.feed
            && receipt.window() == window_id,
        ClutchError::MismatchedState,
    )?;
    apply_sealed_feed(&mut feed, receipt)?;
    feed.encode(
        &mut accounts[IX_MUTATE_FEED]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    Ok(())
}

fn apply_sealed_feed(feed: &mut FeedAccount, receipt: SealedArchiveReceiptV1) -> Outcome<()> {
    feed.cursor = receipt.sealed_feed_cursor();
    feed.next_boundary = receipt.sealed_feed_cursor();
    feed.archive_pages = 1;
    feed.summary = receipt.page_commitment();
    Ok(())
}

fn source_refusal(_: SourceArchiveError) -> Refusal {
    Refusal::Adapter(ClutchError::SourceAdmissionFailed)
}

#[cfg(feature = "non-production-mock-source")]
fn source_view<'data>(account: &AccountInfo<'_>, data: &'data [u8]) -> SourceAccountView<'data> {
    SourceAccountView::new(
        account.key.to_bytes(),
        account.owner.to_bytes(),
        account.executable,
        data,
    )
}

#[cfg(feature = "non-production-mock-source")]
fn runtime_view<'data>(
    account: &AccountInfo<'_>,
    data: &'data [u8],
) -> RuntimeAccountViewV1<'data> {
    RuntimeAccountViewV1::new(
        account.key.to_bytes(),
        account.owner.to_bytes(),
        account.executable,
        data,
    )
}

#[cfg(feature = "non-production-mock-source")]
const MOCK_ADAPTER: [u8; 32] = [0xa1; 32];
#[cfg(feature = "non-production-mock-source")]
const MOCK_PROGRAM: [u8; 32] = [0xb2; 32];
#[cfg(feature = "non-production-mock-source")]
const MOCK_PROGRAM_OWNER: [u8; 32] = [0xb3; 32];
#[cfg(feature = "non-production-mock-source")]
const MOCK_DEPLOYMENT: [u8; 32] = [0xd4; 32];
#[cfg(feature = "non-production-mock-source")]
const MOCK_DEPLOYMENT_OWNER: [u8; 32] = [0xd5; 32];
#[cfg(feature = "non-production-mock-source")]
const MOCK_SOURCE: [u8; 32] = [0xc3; 32];

#[cfg(feature = "non-production-mock-source")]
struct MockDeployment;

#[cfg(feature = "non-production-mock-source")]
impl DeploymentAuthenticatorV1 for MockDeployment {
    const VERIFIER_ID: [u8; 32] = [0xd6; 32];
    const VERIFIER_VERSION: u32 = 1;
    const PROVIDER_PROGRAM: [u8; 32] = MOCK_PROGRAM;
    const PROVIDER_PROGRAM_OWNER: [u8; 32] = MOCK_PROGRAM_OWNER;
    const DEPLOYMENT_ACCOUNT: [u8; 32] = MOCK_DEPLOYMENT;
    const DEPLOYMENT_OWNER: [u8; 32] = MOCK_DEPLOYMENT_OWNER;

    fn deployment_generation(
        provider_program_data: &[u8],
        deployment_account_data: &[u8],
    ) -> Result<u64, SourceArchiveError> {
        if provider_program_data != b"MOCK-PROVIDER-V1"
            || deployment_account_data.len() != 12
            || &deployment_account_data[..4] != b"DEP1"
        {
            return Err(SourceArchiveError::DeploymentAdapterRefused);
        }
        let mut generation = [0_u8; 8];
        generation.copy_from_slice(&deployment_account_data[4..12]);
        Ok(u64::from_le_bytes(generation))
    }
}

#[cfg(feature = "non-production-mock-source")]
struct MockParser;

#[cfg(feature = "non-production-mock-source")]
impl PriceParserV1 for MockParser {
    const SOURCE_ADAPTER_ID: [u8; 32] = MOCK_ADAPTER;
    const SOURCE_ADAPTER_VERSION: u32 = 7;
    const PARSER_ID: u16 = 11;
    const PARSER_VERSION: u16 = 3;

    fn parse(account: SourceAccountView<'_>) -> Result<crate::source::ParsedPriceV1, SourceError> {
        let bytes = account.data();
        if bytes.len() != 77 || &bytes[..4] != b"SRC1" {
            return Err(SourceError::ParserRefused);
        }
        Ok(crate::source::ParsedPriceV1 {
            deployment_generation: u64_at(bytes, 4),
            source_sequence: u64_at(bytes, 12),
            publish_slot: u64_at(bytes, 20),
            publish_time: u64_at(bytes, 28),
            canonical_bucket: u64_at(bytes, 36),
            price_atoms: u128_at(bytes, 44),
            confidence_atoms: u128_at(bytes, 60),
            finalized_bucket: bytes[76] == 1,
        })
    }
}

#[cfg(feature = "non-production-mock-source")]
fn is_mock_release(spec: SourceSpecV1) -> bool {
    spec.source_adapter_id().bytes() == MOCK_ADAPTER
        && spec.source_adapter_version() == 7
        && spec.parser_id() == 11
        && spec.parser_version() == 3
        && spec.source_program() == MOCK_PROGRAM
        && spec.source_account() == MOCK_SOURCE
        && spec.deployment_generation() == 19
}

#[cfg(not(feature = "non-production-mock-source"))]
fn release_registered(_: SourceSpecV1) -> bool {
    false
}

#[cfg(feature = "non-production-mock-source")]
fn release_registered(spec: SourceSpecV1) -> bool {
    is_mock_release(spec)
}

#[allow(clippy::too_many_arguments)]
fn initialize_registered_spec(
    out: &mut [u8],
    spec: SourceSpecV1,
    bump: u8,
    provider: &AccountInfo,
    deployment: &AccountInfo,
    source: &AccountInfo,
) -> Outcome<()> {
    if !release_registered(spec) {
        return Err(ClutchError::SourceReleaseUnavailable.into());
    }
    #[cfg(feature = "non-production-mock-source")]
    {
        let provider_data = provider.data.borrow();
        let deployment_data = deployment.data.borrow();
        let source_data = source.data.borrow();
        source_archive::initialize_authenticated_source_spec_account::<MockParser, MockDeployment>(
            out,
            spec,
            bump,
            runtime_view(provider, &provider_data),
            runtime_view(deployment, &deployment_data),
            source_view(source, &source_data),
        )
        .map_err(source_refusal)
    }
    #[cfg(not(feature = "non-production-mock-source"))]
    {
        let _ = (out, bump, provider, deployment, source);
        Err(ClutchError::SourceReleaseUnavailable.into())
    }
}

fn initialize_registered_archive(
    out: &mut [u8],
    spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    bump: u8,
) -> Outcome<()> {
    if !release_registered(spec.spec()) {
        return Err(ClutchError::SourceReleaseUnavailable.into());
    }
    #[cfg(feature = "non-production-mock-source")]
    {
        source_archive::initialize_genesis_archive::<MockDeployment>(out, spec, window, bump)
            .map_err(source_refusal)
    }
    #[cfg(not(feature = "non-production-mock-source"))]
    {
        let _ = (out, window, bump);
        Err(ClutchError::SourceReleaseUnavailable.into())
    }
}

fn registered_open_sequence(
    archive: &[u8],
    spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
) -> Outcome<u64> {
    if !release_registered(spec.spec()) {
        return Err(ClutchError::SourceReleaseUnavailable.into());
    }
    #[cfg(feature = "non-production-mock-source")]
    {
        source_archive::open_archive_sequence::<MockDeployment>(archive, spec, window)
            .map_err(source_refusal)
    }
    #[cfg(not(feature = "non-production-mock-source"))]
    {
        let _ = (archive, window);
        Err(ClutchError::SourceReleaseUnavailable.into())
    }
}

#[allow(clippy::too_many_arguments)]
fn append_registered(
    archive: &mut [u8],
    spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    clock: TrustedClockV1,
    provider: &AccountInfo,
    deployment: &AccountInfo,
    source: &AccountInfo,
) -> Outcome<()> {
    if !release_registered(spec.spec()) {
        return Err(ClutchError::SourceReleaseUnavailable.into());
    }
    #[cfg(feature = "non-production-mock-source")]
    {
        let provider_data = provider.data.borrow();
        let deployment_data = deployment.data.borrow();
        let source_data = source.data.borrow();
        source_archive::append_authenticated::<MockParser, MockDeployment>(
            archive,
            spec,
            window,
            clock,
            runtime_view(provider, &provider_data),
            runtime_view(deployment, &deployment_data),
            source_view(source, &source_data),
        )
        .map_err(source_refusal)
    }
    #[cfg(not(feature = "non-production-mock-source"))]
    {
        let _ = (archive, window, clock, provider, deployment, source);
        Err(ClutchError::SourceReleaseUnavailable.into())
    }
}

#[allow(clippy::too_many_arguments)]
fn seal_registered(
    archive: &mut [u8],
    spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    clock: TrustedClockV1,
    provider: &AccountInfo,
    deployment: &AccountInfo,
    source: &AccountInfo,
) -> Outcome<()> {
    if !release_registered(spec.spec()) {
        return Err(ClutchError::SourceReleaseUnavailable.into());
    }
    #[cfg(feature = "non-production-mock-source")]
    {
        let provider_data = provider.data.borrow();
        let deployment_data = deployment.data.borrow();
        let source_data = source.data.borrow();
        source_archive::seal_archive_authenticated::<MockParser, MockDeployment>(
            archive,
            spec,
            window,
            clock,
            runtime_view(provider, &provider_data),
            runtime_view(deployment, &deployment_data),
            source_view(source, &source_data),
        )
        .map_err(source_refusal)
    }
    #[cfg(not(feature = "non-production-mock-source"))]
    {
        let _ = (archive, window, clock, provider, deployment, source);
        Err(ClutchError::SourceReleaseUnavailable.into())
    }
}

#[cfg(feature = "non-production-mock-source")]
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    let mut out = [0_u8; 8];
    out.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(out)
}

#[cfg(feature = "non-production-mock-source")]
fn u128_at(bytes: &[u8], offset: usize) -> u128 {
    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes[offset..offset + 16]);
    u128::from_le_bytes(out)
}

#[cfg(all(test, not(feature = "non-production-mock-source")))]
mod tests {
    use super::*;

    #[test]
    fn default_artifact_has_no_registered_source_release() {
        use crate::source::{
            SourceSpecFieldsV1, ORIENTATION_QUOTE_PER_BASE, SELECTION_FINALIZED_BUCKET_RECORD,
        };
        let spec = SourceSpecV1::new(SourceSpecFieldsV1 {
            source_adapter_id: Hash32::from_bytes([0xa1; 32]),
            source_adapter_version: 7,
            parser_id: 11,
            parser_version: 3,
            source_program: [0xb2; 32],
            source_account: [0xc3; 32],
            deployment_generation: 19,
            base_asset_id: Hash32::from_bytes([1; 32]),
            quote_asset_id: Hash32::from_bytes([2; 32]),
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 4,
            grid_version: 2,
            bucket_seconds: 60,
            max_staleness_slots: 20,
            max_staleness_seconds: 90,
            max_future_seconds: 2,
            max_confidence_atoms: 5_000,
            max_confidence_bps: 100,
            confidence_multiplier: 2,
            selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
        })
        .expect("valid mock identity");
        assert!(!release_registered(spec));
    }
}

#[cfg(all(test, feature = "non-production-mock-source"))]
mod mock_registry_tests {
    use super::*;
    use crate::source::{
        SourceSpecFieldsV1, ORIENTATION_QUOTE_PER_BASE, SELECTION_FINALIZED_BUCKET_RECORD,
    };

    fn spec(parser_version: u16, deployment_generation: u64) -> SourceSpecV1 {
        SourceSpecV1::new(SourceSpecFieldsV1 {
            source_adapter_id: Hash32::from_bytes(MOCK_ADAPTER),
            source_adapter_version: 7,
            parser_id: 11,
            parser_version,
            source_program: MOCK_PROGRAM,
            source_account: MOCK_SOURCE,
            deployment_generation,
            base_asset_id: Hash32::from_bytes([1; 32]),
            quote_asset_id: Hash32::from_bytes([2; 32]),
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 4,
            grid_version: 2,
            bucket_seconds: 60,
            max_staleness_slots: 20,
            max_staleness_seconds: 90,
            max_future_seconds: 2,
            max_confidence_atoms: 5_000,
            max_confidence_bps: 100,
            confidence_multiplier: 2,
            selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
        })
        .expect("valid mock-shaped spec")
    }

    #[test]
    fn mock_elf_registers_only_its_exact_compiled_parser_and_deployment_release() {
        assert!(release_registered(spec(3, 19)));
        assert!(!release_registered(spec(4, 19)));
        assert!(!release_registered(spec(3, 20)));
    }
}
