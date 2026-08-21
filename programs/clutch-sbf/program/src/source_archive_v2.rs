//! The v2 source-spec account and the v2 sealed archive page.
//!
//! ## What this owns
//!
//! Two account families, both new tags so no V1 account can ever be read as a
//! v2 one or the reverse:
//!
//! | account | tag | bytes | holds |
//! | --- | ---: | ---: | --- |
//! | v2 SourceSpec | `0x73` | 404 | the 368-byte canonical v2 body and its `dragons-clutch/feed/v2` identity |
//! | v2 source archive | `0x74` | 2 560 | one contiguous page of crossing-rule observations |
//!
//! The archive page's **geometry is deliberately identical to the V1 page**:
//! the same 512-byte header, the same thirty-two 64-byte record slots, the same
//! field offsets, the same hole-punched page commitment. Only the tag and the
//! commitment domain differ, and the header's provider slots carry v2 meanings
//! ([`ARCHIVE_SOURCE_ACCOUNT_OFFSET`] is the receiver `Config` key rather than
//! an immutable price account, and the deployment-generation slot is the
//! ProgramData deployment slot rather than a caller-supplied number).
//!
//! Keeping the geometry is what makes a later resolution-plane join cheap: the
//! record decoder, the fold, and the occupation preflight all read the same
//! bytes at the same offsets. What it does **not** do is make the two
//! interchangeable — the tag and domain separation is exactly what stops that,
//! and [`verify_recorded_sealed_archive_v2`] refuses a V1 page byte-for-byte.
//!
//! ## What is not wired yet
//!
//! Nothing in this module is reachable from [`crate::dispatch`]. The remaining
//! step is the instruction family plus a spec-generation dispatch in the
//! resolution plane; see `docs/implementation/R2_PULL_PROMOTION_PLAN.md` P0.5
//! (account planes) and the note in [`crate::source_v2`].

use clutch_accumulator::{WindowDomain, WINDOW_DOMAIN_BYTES, WINDOW_DOMAIN_TAG};
use clutch_solana_layout::Hash32;

use crate::source_archive::{
    SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, SOURCE_ARCHIVE_HEADER_V1_BYTES, SOURCE_ARCHIVE_MAX_RECORDS_V1,
    SOURCE_ARCHIVE_RECORD_V1_BYTES,
};
use crate::source_identity::PullReleaseV2;
use crate::source_v2::auth::{
    authenticate_pull_update_v2, AuthV2Error, AuthenticatedPullUpdateV2, PullAuthenticationV2,
};
use crate::source_v2::crossing::{admit_after, ArchiveRecordV2, ARCHIVE_RECORD_V2_BYTES};
use crate::source_v2::spec::{SourceSpecV2, SpecV2Error, SOURCE_SPEC_V2_BYTES};

/// Exact byte length of a v2 SourceSpec account.
pub const SOURCE_SPEC_ACCOUNT_V2_BYTES: usize = 404;

/// Exact byte length of a v2 source-archive account.
pub const SOURCE_ARCHIVE_ACCOUNT_V2_BYTES: usize = SOURCE_ARCHIVE_ACCOUNT_V1_BYTES;

/// Maximum records one v2 archive page holds.
pub const SOURCE_ARCHIVE_MAX_RECORDS_V2: usize = SOURCE_ARCHIVE_MAX_RECORDS_V1;

const SOURCE_SPEC_ACCOUNT_V2_TAG: u8 = 0x73;
const SOURCE_SPEC_ACCOUNT_V2_VERSION: u8 = 1;
const SOURCE_ARCHIVE_ACCOUNT_V2_TAG: u8 = 0x74;
const SOURCE_ARCHIVE_ACCOUNT_V2_VERSION: u8 = 1;
const SOURCE_ARCHIVE_V2_FLAG_SEALED: u8 = 1;
const SOURCE_ARCHIVE_V2_COMMITMENT_DOMAIN: &[u8] = b"dragons-clutch/source-archive/v2";

const SPEC_FEED_OFFSET: usize = 2;
const SPEC_BODY_OFFSET: usize = 34;
const SPEC_BUMP_OFFSET: usize = 402;
const SPEC_FLAGS_OFFSET: usize = 403;

const _: () = assert!(SPEC_BODY_OFFSET + SOURCE_SPEC_V2_BYTES == SPEC_BUMP_OFFSET);
const _: () = assert!(SPEC_FLAGS_OFFSET + 1 == SOURCE_SPEC_ACCOUNT_V2_BYTES);
const _: () = assert!(ARCHIVE_RECORD_V2_BYTES == SOURCE_ARCHIVE_RECORD_V1_BYTES);
const _: () = assert!(
    SOURCE_ARCHIVE_ACCOUNT_V2_BYTES
        == SOURCE_ARCHIVE_HEADER_V1_BYTES + SOURCE_ARCHIVE_MAX_RECORDS_V2 * ARCHIVE_RECORD_V2_BYTES
);
const _: () = assert!(SOURCE_ARCHIVE_ACCOUNT_V2_TAG != SOURCE_SPEC_ACCOUNT_V2_TAG);

/* The v2 page's header offsets are the V1 page's, field for field.  They are
 * restated rather than imported because they are private to `source_archive`;
 * `the_v2_page_geometry_is_the_v1_page_geometry` pins the totals that are
 * public, and every offset below is inside that pinned 512-byte header. */
const ARCHIVE_FLAGS_OFFSET: usize = 2;
const ARCHIVE_COUNT_OFFSET: usize = 3;
const ARCHIVE_FEED_OFFSET: usize = 4;
const ARCHIVE_ADAPTER_OFFSET: usize = 36;
const ARCHIVE_ADAPTER_VERSION_OFFSET: usize = 68;
const ARCHIVE_PARSER_ID_OFFSET: usize = 72;
const ARCHIVE_PARSER_VERSION_OFFSET: usize = 74;
const ARCHIVE_RECEIVER_PROGRAM_OFFSET: usize = 76;
const ARCHIVE_RECEIVER_PROGRAM_OWNER_OFFSET: usize = 108;
const ARCHIVE_PROGRAMDATA_OFFSET: usize = 140;
const ARCHIVE_PROGRAMDATA_OWNER_OFFSET: usize = 172;
const ARCHIVE_PROVIDER_FEED_OFFSET: usize = 204;
const ARCHIVE_CONFIG_DIGEST_OFFSET: usize = 236;
/// Header slot holding the receiver `Config` key.
///
/// Its V1 counterpart holds the immutable source data-account key. A pull feed
/// has no such account, and the `Config` PDA is the stable identity that
/// replaces it (`spec_v2` delta 1).
pub const ARCHIVE_SOURCE_ACCOUNT_OFFSET: usize = 268;
const ARCHIVE_DEPLOYMENT_SLOT_OFFSET: usize = 300;
const ARCHIVE_GRID_FAMILY_OFFSET: usize = 308;
const ARCHIVE_GRID_VERSION_OFFSET: usize = 312;
const ARCHIVE_BUCKET_SECONDS_OFFSET: usize = 314;
const ARCHIVE_WINDOW_OFFSET: usize = 322;
const ARCHIVE_WINDOW_START_OFFSET: usize = 354;
const ARCHIVE_WINDOW_END_OFFSET: usize = 362;
const ARCHIVE_MATURITY_OFFSET: usize = 370;
const ARCHIVE_REPAIR_GENERATION_OFFSET: usize = 378;
const ARCHIVE_PAGE_INDEX_OFFSET: usize = 386;
const ARCHIVE_FIRST_BUCKET_OFFSET: usize = 394;
const ARCHIVE_SEALED_FEED_CURSOR_OFFSET: usize = 402;
const ARCHIVE_PREVIOUS_SEQUENCE_OFFSET: usize = 410;
const ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET: usize = 418;
const ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET: usize = 426;
const ARCHIVE_PREVIOUS_COMMITMENT_OFFSET: usize = 434;
const ARCHIVE_COMMITMENT_OFFSET: usize = 466;
const ARCHIVE_BUMP_OFFSET: usize = 498;
const ARCHIVE_RESERVED_OFFSET: usize = 499;

const _: () = assert!(ARCHIVE_RESERVED_OFFSET < SOURCE_ARCHIVE_HEADER_V1_BYTES);
const _: () = assert!(ARCHIVE_COMMITMENT_OFFSET + 32 == ARCHIVE_BUMP_OFFSET);

/// Refusals from the v2 account family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveV2Error {
    /// The buffer is not the exact account length.
    WrongLength,
    /// The leading tag byte names a different account family, including a V1
    /// SourceSpec or a V1 archive page.
    WrongTag,
    /// The version byte is not the one this codec writes.
    WrongVersion,
    /// A reserved or flag byte carried a non-canonical value.
    NonCanonicalPadding,
    /// A required identity was zero.
    ZeroIdentity,
    /// The stored feed digest is not the body's canonical v2 identity.
    SpecDigestMismatch,
    /// The presented account is not the expected address.
    AccountMismatch,
    /// The account is not owned by this program.
    OwnerMismatch,
    /// A state account was presented as executable.
    ExecutableAccount,
    /// The window does not bind this spec's grid.
    InvalidWindow,
    /// The page header does not bind the presented spec, release, or window.
    BindingMismatch,
    /// The page is already sealed.
    AlreadySealed,
    /// The page is not sealed.
    NotSealed,
    /// The page has no room for another record.
    ArchiveFull,
    /// The page's records are not one contiguous run from its first bucket.
    NonContiguousLineage,
    /// The recomputed page commitment does not match the stored one.
    CommitmentMismatch,
    /// A record index is past the recorded count.
    MalformedRecord,
    /// The page is not complete, so it cannot be sealed.
    WindowIncomplete,
    /// The v2 body failed its own codec.
    Spec(SpecV2Error),
    /// The authentication join refused.
    Auth(AuthV2Error),
    /// The crossing rule refused the record's placement.
    Crossing(crate::source_v2::crossing::CrossingError),
}

/// A metadata-bearing view of one v2 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountViewV2<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> AccountViewV2<'a> {
    /// Wrap one runtime account at the `AccountInfo` boundary.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }
}

/// One authenticated v2 SourceSpec account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSourceSpecV2 {
    account_key: [u8; 32],
    spec: SourceSpecV2,
    feed: Hash32,
    stored_bump: u8,
}

impl VerifiedSourceSpecV2 {
    /// Address the spec was authenticated at.
    pub const fn account_key(self) -> [u8; 32] {
        self.account_key
    }

    /// The decoded immutable spec.
    pub const fn spec(self) -> SourceSpecV2 {
        self.spec
    }

    /// The canonical v2 feed identity.
    pub const fn feed(self) -> Hash32 {
        self.feed
    }

    /// The PDA bump the account records.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// A sealed v2 page's authenticated facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedArchiveReceiptV2 {
    archive_key: [u8; 32],
    archive_owner: [u8; 32],
    feed: Hash32,
    window: Hash32,
    page_commitment: Hash32,
    deployment_slot: u64,
    repair_generation: u64,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    sealed_feed_cursor: u64,
    last_sequence: u64,
    last_publish_slot: u64,
    last_publish_time: u64,
    record_count: u8,
    stored_bump: u8,
}

impl SealedArchiveReceiptV2 {
    /// Address the page was authenticated at.
    pub const fn archive_key(self) -> [u8; 32] {
        self.archive_key
    }
    /// Canonical v2 feed identity the page binds.
    pub const fn feed(self) -> Hash32 {
        self.feed
    }
    /// Canonical window identity the page binds.
    pub const fn window(self) -> Hash32 {
        self.window
    }
    /// Recomputed page commitment.
    pub const fn page_commitment(self) -> Hash32 {
        self.page_commitment
    }
    /// ProgramData deployment slot the page was authored under.
    pub const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }
    /// Repair generation of the bound window.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }
    /// First bucket the page covers.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }
    /// Exclusive last bucket the page covers.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }
    /// Feed cursor the seal established.
    pub const fn sealed_feed_cursor(self) -> u64 {
        self.sealed_feed_cursor
    }
    /// Sequence of the last admitted record.
    pub const fn last_sequence(self) -> u64 {
        self.last_sequence
    }
    /// Receiver-write slot of the last admitted record.
    pub const fn last_publish_slot(self) -> u64 {
        self.last_publish_slot
    }
    /// Publish time of the last admitted record.
    pub const fn last_publish_time(self) -> u64 {
        self.last_publish_time
    }
    /// Number of records the page carries.
    pub const fn record_count(self) -> u8 {
        self.record_count
    }
    /// The PDA bump the account records.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// Canonical window identity, in the same domain the V1 plane uses.
///
/// The window domain is a property of the observation grid and the market's
/// Terms, not of the source generation, so v1 and v2 pages covering one window
/// name it identically. That is intended: it is the *source* identity that is
/// domain-separated, not the window.
pub fn canonical_window_id(window: WindowDomain) -> Hash32 {
    let mut bytes = [0_u8; WINDOW_DOMAIN_BYTES];
    window.encode_canonical(&mut bytes);
    Hash32::from_bytes(solana_sha256_hasher::hashv(&[WINDOW_DOMAIN_TAG, &bytes]).to_bytes())
}

/// Write the canonical image of one v2 SourceSpec account.
pub fn initialize_source_spec_v2_account(
    out: &mut [u8],
    spec: SourceSpecV2,
    stored_bump: u8,
) -> Result<(), ArchiveV2Error> {
    exact_len(out, SOURCE_SPEC_ACCOUNT_V2_BYTES)?;
    out.fill(0);
    out[0] = SOURCE_SPEC_ACCOUNT_V2_TAG;
    out[1] = SOURCE_SPEC_ACCOUNT_V2_VERSION;
    put_32(out, SPEC_FEED_OFFSET, spec.feed_id());
    out[SPEC_BODY_OFFSET..SPEC_BODY_OFFSET + SOURCE_SPEC_V2_BYTES]
        .copy_from_slice(&spec.encode_canonical());
    out[SPEC_BUMP_OFFSET] = stored_bump;
    out[SPEC_FLAGS_OFFSET] = 0;
    Ok(())
}

/// Authenticate one v2 SourceSpec account against its expected address.
///
/// The stored digest is checked against the *recomputed* identity of the stored
/// body, so an account carrying a body and a digest that disagree is refused
/// rather than believed.
pub fn verify_source_spec_v2_account(
    clutch_program: [u8; 32],
    expected_key: [u8; 32],
    account: AccountViewV2<'_>,
) -> Result<VerifiedSourceSpecV2, ArchiveV2Error> {
    if is_zero(&clutch_program) || is_zero(&expected_key) {
        return Err(ArchiveV2Error::ZeroIdentity);
    }
    if account.key != expected_key {
        return Err(ArchiveV2Error::AccountMismatch);
    }
    if account.owner != clutch_program {
        return Err(ArchiveV2Error::OwnerMismatch);
    }
    if account.executable {
        return Err(ArchiveV2Error::ExecutableAccount);
    }
    exact_len(account.data, SOURCE_SPEC_ACCOUNT_V2_BYTES)?;
    if account.data[0] != SOURCE_SPEC_ACCOUNT_V2_TAG {
        return Err(ArchiveV2Error::WrongTag);
    }
    if account.data[1] != SOURCE_SPEC_ACCOUNT_V2_VERSION {
        return Err(ArchiveV2Error::WrongVersion);
    }
    if account.data[SPEC_FLAGS_OFFSET] != 0 {
        return Err(ArchiveV2Error::NonCanonicalPadding);
    }
    let spec = SourceSpecV2::decode_canonical(&account.data[SPEC_BODY_OFFSET..SPEC_BUMP_OFFSET])
        .map_err(ArchiveV2Error::Spec)?;
    let stored = array_32(account.data, SPEC_FEED_OFFSET);
    if is_zero(&stored) {
        return Err(ArchiveV2Error::ZeroIdentity);
    }
    if stored != spec.feed_id() {
        return Err(ArchiveV2Error::SpecDigestMismatch);
    }
    Ok(VerifiedSourceSpecV2 {
        account_key: account.key,
        spec,
        feed: Hash32::from_bytes(stored),
        stored_bump: account.data[SPEC_BUMP_OFFSET],
    })
}

/// Write the canonical image of one open genesis v2 archive page.
pub fn initialize_genesis_archive_v2(
    out: &mut [u8],
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
    stored_bump: u8,
) -> Result<(), ArchiveV2Error> {
    exact_len(out, SOURCE_ARCHIVE_ACCOUNT_V2_BYTES)?;
    validate_window(verified_spec.spec, window)?;
    let fields = verified_spec.spec.fields();
    if release.receiver_program != fields.receiver_program
        || release.parser_id != fields.parser_id
        || release.parser_version != fields.parser_version
        || release.source_adapter_id != fields.source_adapter_id
        || release.source_adapter_version != fields.source_adapter_version
    {
        return Err(ArchiveV2Error::BindingMismatch);
    }

    out.fill(0);
    out[0] = SOURCE_ARCHIVE_ACCOUNT_V2_TAG;
    out[1] = SOURCE_ARCHIVE_ACCOUNT_V2_VERSION;
    put_32(out, ARCHIVE_FEED_OFFSET, verified_spec.feed.bytes());
    put_32(out, ARCHIVE_ADAPTER_OFFSET, fields.source_adapter_id);
    put_u32(
        out,
        ARCHIVE_ADAPTER_VERSION_OFFSET,
        fields.source_adapter_version,
    );
    put_u16(out, ARCHIVE_PARSER_ID_OFFSET, fields.parser_id);
    put_u16(out, ARCHIVE_PARSER_VERSION_OFFSET, fields.parser_version);
    put_32(
        out,
        ARCHIVE_RECEIVER_PROGRAM_OFFSET,
        fields.receiver_program,
    );
    put_32(
        out,
        ARCHIVE_RECEIVER_PROGRAM_OWNER_OFFSET,
        release.upgradeable_loader,
    );
    put_32(out, ARCHIVE_PROGRAMDATA_OFFSET, fields.receiver_programdata);
    put_32(
        out,
        ARCHIVE_PROGRAMDATA_OWNER_OFFSET,
        release.upgradeable_loader,
    );
    put_32(out, ARCHIVE_PROVIDER_FEED_OFFSET, fields.provider_feed_id);
    put_32(out, ARCHIVE_CONFIG_DIGEST_OFFSET, fields.config_digest);
    put_32(out, ARCHIVE_SOURCE_ACCOUNT_OFFSET, fields.receiver_config);
    put_u64(
        out,
        ARCHIVE_DEPLOYMENT_SLOT_OFFSET,
        fields.programdata_deployment_slot,
    );
    let grid = window.grid();
    put_u32(out, ARCHIVE_GRID_FAMILY_OFFSET, grid.family_id());
    put_u16(out, ARCHIVE_GRID_VERSION_OFFSET, grid.version());
    put_u64(out, ARCHIVE_BUCKET_SECONDS_OFFSET, grid.bucket_seconds());
    put_32(
        out,
        ARCHIVE_WINDOW_OFFSET,
        canonical_window_id(window).bytes(),
    );
    put_u64(out, ARCHIVE_WINDOW_START_OFFSET, window.start_bucket());
    put_u64(
        out,
        ARCHIVE_WINDOW_END_OFFSET,
        window.end_bucket_exclusive(),
    );
    put_u64(
        out,
        ARCHIVE_MATURITY_OFFSET,
        window.maturity_bucket_exclusive(),
    );
    put_u64(out, ARCHIVE_REPAIR_GENERATION_OFFSET, window.generation());
    put_u64(out, ARCHIVE_PAGE_INDEX_OFFSET, 0);
    put_u64(out, ARCHIVE_FIRST_BUCKET_OFFSET, window.start_bucket());
    /* A genesis page has no predecessor, so its four lineage fields are
     * explicitly zero rather than absent.  Writing them names the slots a
     * successor page would carry, which is what keeps a later multi-page
     * lineage a codec addition rather than a layout change. */
    put_u64(out, ARCHIVE_PREVIOUS_SEQUENCE_OFFSET, 0);
    put_u64(out, ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET, 0);
    put_u64(out, ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET, 0);
    put_32(out, ARCHIVE_PREVIOUS_COMMITMENT_OFFSET, [0_u8; 32]);
    out[ARCHIVE_BUMP_OFFSET] = stored_bump;
    stamp_commitment(out);
    Ok(())
}

/// The state-owned append nonce: how many records the open page already holds.
///
/// Returning it from authenticated account state rather than accepting it from
/// a caller is what lets an instruction bind a replay sequence to the page
/// itself.
pub fn open_archive_v2_sequence(
    archive: &[u8],
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
) -> Result<u64, ArchiveV2Error> {
    let header = verify_page(archive, verified_spec, release, window, false)?;
    Ok(u64::from(header.record_count))
}

/// Authenticate one pull update and append the record it owns.
///
/// The bucket is taken from the page's own cursor
/// (`first_bucket + record_count`), never from instruction data, so a caller
/// cannot choose which boundary a witness is credited to.
pub fn append_authenticated_v2(
    archive: &mut [u8],
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
    auth: PullAuthenticationV2<'_>,
) -> Result<AuthenticatedPullUpdateV2, ArchiveV2Error> {
    let header = verify_page(archive, verified_spec, release, window, false)?;
    if usize::from(header.record_count) >= SOURCE_ARCHIVE_MAX_RECORDS_V2
        || u64::from(header.record_count) >= header.window_end - header.window_start
    {
        return Err(ArchiveV2Error::ArchiveFull);
    }
    let cursor = header
        .first_bucket
        .checked_add(u64::from(header.record_count))
        .ok_or(ArchiveV2Error::NonContiguousLineage)?;

    /* The caller supplies neither the release, the spec, nor the bucket: all
     * three come from authenticated state, and the join below re-derives the
     * loader state and the adjacent post from the presented accounts. */
    let mut bound = auth;
    bound.release = release;
    bound.spec = verified_spec.spec;
    bound.bucket = cursor;
    let admitted = authenticate_pull_update_v2(bound).map_err(ArchiveV2Error::Auth)?;

    if header.record_count > 0 {
        let previous = record_at(archive, usize::from(header.record_count) - 1);
        admit_after(previous, admitted.record).map_err(ArchiveV2Error::Crossing)?;
    }

    let offset = record_offset(usize::from(header.record_count));
    archive[offset..offset + ARCHIVE_RECORD_V2_BYTES].copy_from_slice(&admitted.record.encode());
    archive[ARCHIVE_COUNT_OFFSET] = header.record_count + 1;
    stamp_commitment(archive);
    Ok(admitted)
}

/// Seal one complete v2 page and fix the feed cursor it establishes.
///
/// A page may be sealed only when it covers its whole window: a short page
/// would silently narrow the observation window a market resolves against.
pub fn seal_archive_v2(
    archive: &mut [u8],
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
) -> Result<(), ArchiveV2Error> {
    let header = verify_page(archive, verified_spec, release, window, false)?;
    if u64::from(header.record_count) != header.window_end - header.window_start {
        return Err(ArchiveV2Error::WindowIncomplete);
    }
    archive[ARCHIVE_FLAGS_OFFSET] = SOURCE_ARCHIVE_V2_FLAG_SEALED;
    put_u64(
        archive,
        ARCHIVE_SEALED_FEED_CURSOR_OFFSET,
        header.window_end,
    );
    stamp_commitment(archive);
    Ok(())
}

/// Authenticate one recorded sealed v2 page.
pub fn verify_recorded_sealed_archive_v2(
    clutch_program: [u8; 32],
    expected_archive_key: [u8; 32],
    account: AccountViewV2<'_>,
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
) -> Result<SealedArchiveReceiptV2, ArchiveV2Error> {
    if is_zero(&clutch_program) || is_zero(&expected_archive_key) {
        return Err(ArchiveV2Error::ZeroIdentity);
    }
    if account.key != expected_archive_key {
        return Err(ArchiveV2Error::AccountMismatch);
    }
    if account.owner != clutch_program {
        return Err(ArchiveV2Error::OwnerMismatch);
    }
    if account.executable {
        return Err(ArchiveV2Error::ExecutableAccount);
    }
    let header = verify_page(account.data, verified_spec, release, window, true)?;
    let last = if header.record_count == 0 {
        ArchiveRecordV2 {
            bucket: 0,
            low: 0,
            high: 0,
            sequence: u64_at(account.data, ARCHIVE_PREVIOUS_SEQUENCE_OFFSET),
            publish_slot: u64_at(account.data, ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET),
            publish_time: u64_at(account.data, ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET),
        }
    } else {
        record_at(account.data, usize::from(header.record_count) - 1)
    };
    Ok(SealedArchiveReceiptV2 {
        archive_key: account.key,
        archive_owner: account.owner,
        feed: header.feed,
        window: header.window,
        page_commitment: header.page_commitment,
        deployment_slot: header.deployment_slot,
        repair_generation: header.repair_generation,
        start_bucket: header.window_start,
        end_bucket_exclusive: header.window_end,
        sealed_feed_cursor: header.sealed_feed_cursor,
        last_sequence: last.sequence,
        last_publish_slot: last.publish_slot,
        last_publish_time: last.publish_time,
        record_count: header.record_count,
        stored_bump: account.data[ARCHIVE_BUMP_OFFSET],
    })
}

/// Lifetime-bound read capability for one fully verified sealed v2 page.
///
/// The v2 twin of [`crate::source_archive::VerifiedSealedArchiveViewV1`], and
/// it exists for the same reason: [`verify_recorded_sealed_archive_v2`] has
/// already run the key, owner, executable, spec, release, window, lineage,
/// seal, and page-commitment checks over these exact bytes, and the immutable
/// borrow prevents the page from changing under a fold.  Indexed reads
/// therefore check only their bounded index instead of rehashing the
/// 2,560-byte page once per bucket.
///
/// No raw slice accessor exists.  A consumer can read checked records and the
/// authenticated receipt, and nothing else.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSealedArchiveViewV2<'a> {
    receipt: SealedArchiveReceiptV2,
    data: &'a [u8],
}

impl VerifiedSealedArchiveViewV2<'_> {
    /// Return the authenticated archive/window provenance.
    pub const fn receipt(self) -> SealedArchiveReceiptV2 {
        self.receipt
    }

    /// Read one bounded record from the immutable page verified at
    /// construction.
    ///
    /// The index remains hostile and is checked against the committed record
    /// count; page bytes and metadata are not caller-selectable once this
    /// capability exists.
    pub fn archived_record(self, index: usize) -> Result<ArchiveRecordV2, ArchiveV2Error> {
        if index >= usize::from(self.receipt.record_count) {
            return Err(ArchiveV2Error::MalformedRecord);
        }
        Ok(record_at(self.data, index))
    }
}

/// Authenticate one recorded sealed v2 page and retain its immutable bytes.
///
/// The live-fold form of [`verify_recorded_sealed_archive_v2`].  It does not
/// weaken that receipt API: it runs it first on the exact same account view,
/// then binds the resulting receipt to the lifetime of those verified bytes.
pub fn verify_recorded_sealed_archive_v2_view(
    clutch_program: [u8; 32],
    expected_archive_key: [u8; 32],
    account: AccountViewV2<'_>,
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
) -> Result<VerifiedSealedArchiveViewV2<'_>, ArchiveV2Error> {
    let receipt = verify_recorded_sealed_archive_v2(
        clutch_program,
        expected_archive_key,
        account,
        verified_spec,
        release,
        window,
    )?;
    Ok(VerifiedSealedArchiveViewV2 {
        receipt,
        data: account.data,
    })
}

/// Read one record back from the exact page that produced `receipt`.
///
/// The key, owner, executability, length, and recomputed commitment are all
/// repeated, so a receipt can never be paired with another page's bytes.
pub fn archived_record_v2(
    receipt: SealedArchiveReceiptV2,
    account: AccountViewV2<'_>,
    index: usize,
) -> Result<ArchiveRecordV2, ArchiveV2Error> {
    if account.key != receipt.archive_key {
        return Err(ArchiveV2Error::AccountMismatch);
    }
    if account.owner != receipt.archive_owner {
        return Err(ArchiveV2Error::OwnerMismatch);
    }
    if account.executable {
        return Err(ArchiveV2Error::ExecutableAccount);
    }
    exact_len(account.data, SOURCE_ARCHIVE_ACCOUNT_V2_BYTES)?;
    if page_commitment(account.data) != receipt.page_commitment {
        return Err(ArchiveV2Error::CommitmentMismatch);
    }
    if index >= usize::from(receipt.record_count) {
        return Err(ArchiveV2Error::MalformedRecord);
    }
    Ok(record_at(account.data, index))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PageHeaderV2 {
    feed: Hash32,
    window: Hash32,
    page_commitment: Hash32,
    deployment_slot: u64,
    repair_generation: u64,
    window_start: u64,
    window_end: u64,
    first_bucket: u64,
    sealed_feed_cursor: u64,
    record_count: u8,
}

/// Authenticate one page against a spec, a release, and a window.
fn verify_page(
    archive: &[u8],
    verified_spec: VerifiedSourceSpecV2,
    release: PullReleaseV2,
    window: WindowDomain,
    require_sealed: bool,
) -> Result<PageHeaderV2, ArchiveV2Error> {
    exact_len(archive, SOURCE_ARCHIVE_ACCOUNT_V2_BYTES)?;
    if archive[0] != SOURCE_ARCHIVE_ACCOUNT_V2_TAG {
        return Err(ArchiveV2Error::WrongTag);
    }
    if archive[1] != SOURCE_ARCHIVE_ACCOUNT_V2_VERSION {
        return Err(ArchiveV2Error::WrongVersion);
    }
    let flags = archive[ARCHIVE_FLAGS_OFFSET];
    if flags > SOURCE_ARCHIVE_V2_FLAG_SEALED {
        return Err(ArchiveV2Error::NonCanonicalPadding);
    }
    let sealed = flags == SOURCE_ARCHIVE_V2_FLAG_SEALED;
    if require_sealed && !sealed {
        return Err(ArchiveV2Error::NotSealed);
    }
    if !require_sealed && sealed {
        return Err(ArchiveV2Error::AlreadySealed);
    }
    if archive[ARCHIVE_RESERVED_OFFSET..SOURCE_ARCHIVE_HEADER_V1_BYTES]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ArchiveV2Error::NonCanonicalPadding);
    }

    validate_window(verified_spec.spec, window)?;
    let fields = verified_spec.spec.fields();
    let grid = window.grid();
    let window_id = canonical_window_id(window);

    /* Every identity the page recorded at creation is re-compared against the
     * spec and the compiled release presented now.  A page authored under a
     * different config generation, deployment slot, feed, or parser release is
     * a different source and is refused rather than re-labelled. */
    let matches = array_32(archive, ARCHIVE_FEED_OFFSET) == verified_spec.feed.bytes()
        && array_32(archive, ARCHIVE_ADAPTER_OFFSET) == fields.source_adapter_id
        && u32_at(archive, ARCHIVE_ADAPTER_VERSION_OFFSET) == fields.source_adapter_version
        && u16_at(archive, ARCHIVE_PARSER_ID_OFFSET) == fields.parser_id
        && u16_at(archive, ARCHIVE_PARSER_VERSION_OFFSET) == fields.parser_version
        && array_32(archive, ARCHIVE_RECEIVER_PROGRAM_OFFSET) == fields.receiver_program
        && array_32(archive, ARCHIVE_RECEIVER_PROGRAM_OWNER_OFFSET) == release.upgradeable_loader
        && array_32(archive, ARCHIVE_PROGRAMDATA_OFFSET) == fields.receiver_programdata
        && array_32(archive, ARCHIVE_PROGRAMDATA_OWNER_OFFSET) == release.upgradeable_loader
        && array_32(archive, ARCHIVE_PROVIDER_FEED_OFFSET) == fields.provider_feed_id
        && array_32(archive, ARCHIVE_CONFIG_DIGEST_OFFSET) == fields.config_digest
        && array_32(archive, ARCHIVE_SOURCE_ACCOUNT_OFFSET) == fields.receiver_config
        && u64_at(archive, ARCHIVE_DEPLOYMENT_SLOT_OFFSET) == fields.programdata_deployment_slot
        && u32_at(archive, ARCHIVE_GRID_FAMILY_OFFSET) == grid.family_id()
        && u16_at(archive, ARCHIVE_GRID_VERSION_OFFSET) == grid.version()
        && u64_at(archive, ARCHIVE_BUCKET_SECONDS_OFFSET) == grid.bucket_seconds()
        && array_32(archive, ARCHIVE_WINDOW_OFFSET) == window_id.bytes()
        && u64_at(archive, ARCHIVE_WINDOW_START_OFFSET) == window.start_bucket()
        && u64_at(archive, ARCHIVE_WINDOW_END_OFFSET) == window.end_bucket_exclusive()
        && u64_at(archive, ARCHIVE_MATURITY_OFFSET) == window.maturity_bucket_exclusive()
        && u64_at(archive, ARCHIVE_REPAIR_GENERATION_OFFSET) == window.generation()
        && u64_at(archive, ARCHIVE_PAGE_INDEX_OFFSET) == 0
        && u64_at(archive, ARCHIVE_FIRST_BUCKET_OFFSET) == window.start_bucket();
    if !matches {
        return Err(ArchiveV2Error::BindingMismatch);
    }

    let record_count = archive[ARCHIVE_COUNT_OFFSET];
    if usize::from(record_count) > SOURCE_ARCHIVE_MAX_RECORDS_V2 {
        return Err(ArchiveV2Error::MalformedRecord);
    }
    let sealed_feed_cursor = u64_at(archive, ARCHIVE_SEALED_FEED_CURSOR_OFFSET);
    if sealed {
        if sealed_feed_cursor != window.end_bucket_exclusive()
            || u64::from(record_count) != window.end_bucket_exclusive() - window.start_bucket()
        {
            return Err(ArchiveV2Error::BindingMismatch);
        }
    } else if sealed_feed_cursor != 0 {
        return Err(ArchiveV2Error::BindingMismatch);
    }

    verify_records(archive, record_count, window.start_bucket())?;

    let stored_commitment = array_32(archive, ARCHIVE_COMMITMENT_OFFSET);
    let recomputed = page_commitment(archive);
    if stored_commitment != recomputed.bytes() {
        return Err(ArchiveV2Error::CommitmentMismatch);
    }

    Ok(PageHeaderV2 {
        feed: verified_spec.feed,
        window: window_id,
        page_commitment: recomputed,
        deployment_slot: fields.programdata_deployment_slot,
        repair_generation: window.generation(),
        window_start: window.start_bucket(),
        window_end: window.end_bucket_exclusive(),
        first_bucket: window.start_bucket(),
        sealed_feed_cursor,
        record_count,
    })
}

/// Every recorded slot is one contiguous run, and every unused slot is zero.
fn verify_records(archive: &[u8], count: u8, first_bucket: u64) -> Result<(), ArchiveV2Error> {
    let mut previous: Option<ArchiveRecordV2> = None;
    for index in 0..usize::from(count) {
        let record = record_at(archive, index);
        let expected = first_bucket
            .checked_add(index as u64)
            .ok_or(ArchiveV2Error::NonContiguousLineage)?;
        if record.bucket != expected || record.low > record.high {
            return Err(ArchiveV2Error::NonContiguousLineage);
        }
        if let Some(prev) = previous {
            admit_after(prev, record).map_err(ArchiveV2Error::Crossing)?;
        }
        previous = Some(record);
    }
    for index in usize::from(count)..SOURCE_ARCHIVE_MAX_RECORDS_V2 {
        let offset = record_offset(index);
        if archive[offset..offset + ARCHIVE_RECORD_V2_BYTES]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ArchiveV2Error::NonCanonicalPadding);
        }
    }
    Ok(())
}

fn validate_window(spec: SourceSpecV2, window: WindowDomain) -> Result<(), ArchiveV2Error> {
    let fields = spec.fields();
    let grid = window.grid();
    if grid.family_id() != fields.grid_family_id
        || grid.version() != fields.grid_version
        || grid.bucket_seconds() != fields.bucket_seconds
    {
        return Err(ArchiveV2Error::InvalidWindow);
    }
    if window.end_bucket_exclusive() <= window.start_bucket() {
        return Err(ArchiveV2Error::InvalidWindow);
    }
    if usize::try_from(window.end_bucket_exclusive() - window.start_bucket())
        .map(|span| span > SOURCE_ARCHIVE_MAX_RECORDS_V2)
        .unwrap_or(true)
    {
        return Err(ArchiveV2Error::InvalidWindow);
    }
    Ok(())
}

/// The page commitment, computed over everything except its own slot.
fn page_commitment(archive: &[u8]) -> Hash32 {
    Hash32::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_ARCHIVE_V2_COMMITMENT_DOMAIN,
            &archive[..ARCHIVE_COMMITMENT_OFFSET],
            &archive[ARCHIVE_COMMITMENT_OFFSET + 32..],
        ])
        .to_bytes(),
    )
}

fn stamp_commitment(archive: &mut [u8]) {
    archive[ARCHIVE_COMMITMENT_OFFSET..ARCHIVE_COMMITMENT_OFFSET + 32].fill(0);
    let commitment = page_commitment(archive);
    put_32(archive, ARCHIVE_COMMITMENT_OFFSET, commitment.bytes());
}

fn record_offset(index: usize) -> usize {
    SOURCE_ARCHIVE_HEADER_V1_BYTES + index * ARCHIVE_RECORD_V2_BYTES
}

fn record_at(archive: &[u8], index: usize) -> ArchiveRecordV2 {
    let at = record_offset(index);
    ArchiveRecordV2 {
        bucket: u64_at(archive, at),
        low: u128_at(archive, at + 8),
        high: u128_at(archive, at + 24),
        sequence: u64_at(archive, at + 40),
        publish_slot: u64_at(archive, at + 48),
        publish_time: u64_at(archive, at + 56),
    }
}

fn exact_len(bytes: &[u8], expected: usize) -> Result<(), ArchiveV2Error> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(ArchiveV2Error::WrongLength)
    }
}

fn is_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn array_32(bytes: &[u8], offset: usize) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes[offset..offset + 32]);
    out
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    let mut out = [0_u8; 2];
    out.copy_from_slice(&bytes[offset..offset + 2]);
    u16::from_le_bytes(out)
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    let mut out = [0_u8; 4];
    out.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(out)
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    let mut out = [0_u8; 8];
    out.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(out)
}

fn u128_at(bytes: &[u8], offset: usize) -> u128 {
    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes[offset..offset + 16]);
    u128::from_le_bytes(out)
}

fn put_32(bytes: &mut [u8], offset: usize, value: [u8; 32]) {
    bytes[offset..offset + 32].copy_from_slice(&value);
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions_sysvar::{INSTRUCTIONS_SYSVAR_ID, SYSVAR_OWNER_ID};
    use crate::loader_state::{LoaderAccountViewV1, UPGRADEABLE_LOADER_ID};
    use crate::pyth_receiver::{config_byte_digest, PriceUpdateAccountViewV1};
    use crate::source_identity::{fixture, CLOCK_SYSVAR_ID};
    use crate::source_v2::auth::{
        decode_clock_view, AccountViewV2 as AuthAccountView, PullAuthenticationV2,
    };
    use crate::source_v2::crossing::SELECTION_CROSSING_V1;
    use crate::source_v2::fixtures as fx;
    use crate::source_v2::spec::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
        SOURCE_SPEC_V2_BYTES,
    };
    use clutch_accumulator::{CoveragePolicy, FeedIdentity, Grid};

    const CLUTCH_PROGRAM: [u8; 32] = [0x51; 32];
    const SPEC_KEY: [u8; 32] = [0x52; 32];
    const ARCHIVE_KEY: [u8; 32] = [0x53; 32];
    const SPEC_BUMP: u8 = 254;
    const ARCHIVE_BUMP: u8 = 253;
    const WRITE_AUTHORITY: [u8; 32] = [0x11; 32];

    /// Ten-second buckets, so the window's four boundaries are
    /// T(k) = (k+1)*10 for k in START..START+4.
    const BUCKET_SECONDS: u64 = 10;
    /// T(START) = 1_700_000_000.
    const START_BUCKET: u64 = 169_999_999;
    const WINDOW_BUCKETS: u64 = 4;

    fn config_bytes() -> Vec<u8> {
        fx::config_body(
            [0x9f, 0x1e, 0x2b, 0x33, 0x44, 0x55, 0x66, 0x77],
            [0x31; 32],
            None,
            [0x32; 32],
            &[(1, [0x33; 32]), (26, [0x34; 32])],
            1,
            3,
        )
    }

    fn spec_fields() -> SourceSpecFieldsV2 {
        SourceSpecFieldsV2 {
            source_adapter_id: fixture::SOURCE_ADAPTER_ID,
            source_adapter_version: fixture::SOURCE_ADAPTER_VERSION,
            parser_id: fixture::PARSER_ID,
            parser_version: fixture::PARSER_VERSION,
            receiver_program: fixture::RECEIVER_PROGRAM,
            receiver_programdata: fixture::RECEIVER_PROGRAMDATA,
            receiver_config: fixture::RECEIVER_CONFIG,
            config_digest: config_byte_digest(&config_bytes()),
            provider_feed_id: fixture::PROVIDER_FEED_ID,
            programdata_deployment_slot: fixture::PROGRAMDATA_DEPLOYMENT_SLOT,
            base_asset_id: fixture::BASE_ASSET_ID,
            quote_asset_id: fixture::QUOTE_ASSET_ID,
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 4,
            grid_version: 9,
            grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
            bucket_seconds: BUCKET_SECONDS,
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

    fn spec() -> SourceSpecV2 {
        SourceSpecV2::new(spec_fields()).expect("valid fixture spec")
    }

    fn window() -> WindowDomain {
        let feed = FeedIdentity::new(
            fixture::SOURCE_ADAPTER_ID,
            spec().feed_id(),
            fixture::SOURCE_ADAPTER_VERSION,
            1,
        )
        .expect("valid feed identity");
        let grid = Grid::new(4, 9, BUCKET_SECONDS).expect("valid grid");
        WindowDomain::new(
            feed,
            grid,
            START_BUCKET,
            START_BUCKET + WINDOW_BUCKETS,
            START_BUCKET + WINDOW_BUCKETS + 1,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid window")
    }

    fn verified_spec_account() -> (Vec<u8>, VerifiedSourceSpecV2) {
        let mut image = vec![0_u8; SOURCE_SPEC_ACCOUNT_V2_BYTES];
        initialize_source_spec_v2_account(&mut image, spec(), SPEC_BUMP).expect("spec image");
        let verified = verify_source_spec_v2_account(
            CLUTCH_PROGRAM,
            SPEC_KEY,
            AccountViewV2::new(SPEC_KEY, CLUTCH_PROGRAM, false, &image),
        )
        .expect("spec authenticates");
        (image, verified)
    }

    /// The four fabricated provider accounts plus the sysvar image, for one
    /// append at `bucket`.
    struct Presented {
        program: Vec<u8>,
        programdata: Vec<u8>,
        config: Vec<u8>,
        update: Vec<u8>,
        sysvar: Vec<u8>,
        clock: Vec<u8>,
        update_key: [u8; 32],
    }

    /// The witness for bucket `k`: `prev < T(k) <= publish`.
    ///
    /// Each bucket gets its own distinct witness, one second past its
    /// boundary, so the archive's sequence is strictly increasing and the
    /// non-strict clause is not what carries the walk.
    fn witness_for(bucket: u64) -> (i64, i64) {
        let boundary = ((bucket + 1) * BUCKET_SECONDS) as i64;
        (boundary + 1, boundary - 3)
    }

    fn presented(bucket: u64, update_key: [u8; 32]) -> Presented {
        let (publish, prev) = witness_for(bucket);
        let posted_slot = 250_000_000 + bucket - START_BUCKET;
        let clock_unix = publish + 10;
        Presented {
            program: fx::receiver_program_body(fixture::RECEIVER_PROGRAMDATA),
            programdata: fx::programdata_body(
                fixture::PROGRAMDATA_DEPLOYMENT_SLOT,
                None,
                [0xde; 32],
                b"fabricated-receiver-elf-bytes",
            ),
            config: config_bytes(),
            update: fx::price_update_body(fx::PriceUpdateFixture::new(
                WRITE_AUTHORITY,
                fixture::PROVIDER_FEED_ID,
                publish,
                prev,
                posted_slot,
            )),
            sysvar: fx::instructions_sysvar_body(
                &[
                    fx::post_instruction(
                        fixture::RECEIVER_PROGRAM,
                        fixture::RECEIVER_CONFIG,
                        update_key,
                        WRITE_AUTHORITY,
                    ),
                    fx::consuming_instruction(CLUTCH_PROGRAM),
                ],
                1,
            ),
            clock: fx::clock_body(posted_slot + 5, clock_unix),
            update_key,
        }
    }

    fn authentication(p: &Presented) -> PullAuthenticationV2<'_> {
        let clock = decode_clock_view(AuthAccountView::new(
            CLOCK_SYSVAR_ID,
            [0; 32],
            false,
            &p.clock,
        ))
        .expect("canonical clock");
        PullAuthenticationV2 {
            release: fixture::RELEASE,
            spec: spec(),
            receiver_program: LoaderAccountViewV1::new(
                fixture::RECEIVER_PROGRAM,
                UPGRADEABLE_LOADER_ID,
                true,
                &p.program,
            ),
            receiver_programdata: LoaderAccountViewV1::new(
                fixture::RECEIVER_PROGRAMDATA,
                UPGRADEABLE_LOADER_ID,
                false,
                &p.programdata,
            ),
            receiver_config: AuthAccountView::new(
                fixture::RECEIVER_CONFIG,
                fixture::RECEIVER_PROGRAM,
                false,
                &p.config,
            ),
            update: PriceUpdateAccountViewV1::new(
                p.update_key,
                fixture::RECEIVER_PROGRAM,
                false,
                &p.update,
            ),
            instructions_sysvar: AuthAccountView::new(
                INSTRUCTIONS_SYSVAR_ID,
                SYSVAR_OWNER_ID,
                false,
                &p.sysvar,
            ),
            clock,
            bucket: 0,
        }
    }

    /// Drive the whole page: genesis, four authenticated appends through four
    /// distinct ephemeral accounts, then the seal.
    fn walk_the_window() -> (Vec<u8>, VerifiedSourceSpecV2) {
        let (_, verified) = verified_spec_account();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        initialize_genesis_archive_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            ARCHIVE_BUMP,
        )
        .expect("genesis page");

        for step in 0..WINDOW_BUCKETS {
            let bucket = START_BUCKET + step;
            let sequence = open_archive_v2_sequence(&archive, verified, fixture::RELEASE, window())
                .expect("open page reports its own cursor");
            assert_eq!(sequence, step, "the append nonce is state-owned");

            let mut update_key = [0x60; 32];
            update_key[31] = step as u8;
            let p = presented(bucket, update_key);
            let admitted = append_authenticated_v2(
                &mut archive,
                verified,
                fixture::RELEASE,
                window(),
                authentication(&p),
            )
            .expect("well-formed witness appends");
            assert_eq!(admitted.record.bucket, bucket);
            assert_eq!(admitted.update_account, update_key);
        }

        seal_archive_v2(&mut archive, verified, fixture::RELEASE, window()).expect("page seals");
        (archive, verified)
    }

    #[test]
    fn the_whole_window_ingests_seals_and_reads_back() {
        let (archive, verified) = walk_the_window();
        let receipt = verify_recorded_sealed_archive_v2(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
            verified,
            fixture::RELEASE,
            window(),
        )
        .expect("sealed page authenticates");

        assert_eq!(receipt.feed().bytes(), spec().feed_id());
        assert_eq!(receipt.window(), canonical_window_id(window()));
        assert_eq!(receipt.record_count(), WINDOW_BUCKETS as u8);
        assert_eq!(receipt.start_bucket(), START_BUCKET);
        assert_eq!(
            receipt.end_bucket_exclusive(),
            START_BUCKET + WINDOW_BUCKETS
        );
        assert_eq!(receipt.sealed_feed_cursor(), START_BUCKET + WINDOW_BUCKETS);
        assert_eq!(
            receipt.deployment_slot(),
            fixture::PROGRAMDATA_DEPLOYMENT_SLOT
        );
        assert_eq!(receipt.stored_bump(), ARCHIVE_BUMP);

        let view = AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive);
        let mut previous_sequence = 0_u64;
        for step in 0..WINDOW_BUCKETS {
            let record =
                archived_record_v2(receipt, view, step as usize).expect("record reads back");
            let bucket = START_BUCKET + step;
            let (publish, _) = witness_for(bucket);
            assert_eq!(record.bucket, bucket);
            assert_eq!(record.sequence, publish as u64);
            assert_eq!(record.publish_time, publish as u64);
            assert!(record.sequence > previous_sequence);
            assert!(record.low < record.high);
            previous_sequence = record.sequence;
        }
        assert_eq!(
            archived_record_v2(receipt, view, WINDOW_BUCKETS as usize),
            Err(ArchiveV2Error::MalformedRecord)
        );
    }

    #[test]
    fn a_sealed_page_refuses_a_further_append_and_a_second_seal() {
        let (mut archive, verified) = walk_the_window();
        let p = presented(START_BUCKET + WINDOW_BUCKETS, [0x70; 32]);
        assert_eq!(
            append_authenticated_v2(
                &mut archive,
                verified,
                fixture::RELEASE,
                window(),
                authentication(&p),
            )
            .err(),
            Some(ArchiveV2Error::AlreadySealed)
        );
        assert_eq!(
            seal_archive_v2(&mut archive, verified, fixture::RELEASE, window()),
            Err(ArchiveV2Error::AlreadySealed)
        );
    }

    #[test]
    fn an_incomplete_page_cannot_be_sealed() {
        let (_, verified) = verified_spec_account();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        initialize_genesis_archive_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            ARCHIVE_BUMP,
        )
        .unwrap();
        let p = presented(START_BUCKET, [0x60; 32]);
        append_authenticated_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            authentication(&p),
        )
        .unwrap();
        // One of four buckets covered: sealing here would silently narrow the
        // window a market resolves against.
        assert_eq!(
            seal_archive_v2(&mut archive, verified, fixture::RELEASE, window()),
            Err(ArchiveV2Error::WindowIncomplete)
        );
        assert_eq!(
            verify_recorded_sealed_archive_v2(
                CLUTCH_PROGRAM,
                ARCHIVE_KEY,
                AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
                verified,
                fixture::RELEASE,
                window(),
            ),
            Err(ArchiveV2Error::NotSealed)
        );
    }

    /// Every hostile presentation the battery covers, against a page that is
    /// otherwise ready to accept its first record.
    fn refuse_first_append(mutate: impl FnOnce(&mut Presented)) -> ArchiveV2Error {
        let (_, verified) = verified_spec_account();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        initialize_genesis_archive_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            ARCHIVE_BUMP,
        )
        .unwrap();
        let before = archive.clone();
        let mut p = presented(START_BUCKET, [0x60; 32]);
        mutate(&mut p);
        let error = append_authenticated_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            authentication(&p),
        )
        .expect_err("hostile presentation must refuse");
        assert_eq!(archive, before, "a refused append must write no byte");
        error
    }

    #[test]
    fn wrong_magic_refuses_with_a_typed_error() {
        let error = refuse_first_append(|p| {
            let (publish, prev) = witness_for(START_BUCKET);
            p.update = fx::price_update_body_with_discriminator(
                fx::PriceUpdateFixture::new(
                    WRITE_AUTHORITY,
                    fixture::PROVIDER_FEED_ID,
                    publish,
                    prev,
                    250_000_000,
                ),
                [0; 8],
            );
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::Parser(
                crate::pyth_receiver::PythReceiverError::WrongDiscriminator
            ))
        );
    }

    #[test]
    fn a_stale_receiver_write_slot_refuses() {
        let error = refuse_first_append(|p| {
            let (publish, _) = witness_for(START_BUCKET);
            // The Clock has advanced far past the slot the receiver recorded.
            p.clock = fx::clock_body(250_000_000 + 501, publish + 10);
        });
        assert_eq!(error, ArchiveV2Error::Auth(AuthV2Error::StalePostedSlot));
    }

    #[test]
    fn a_future_receiver_write_slot_refuses() {
        let error = refuse_first_append(|p| {
            let (publish, _) = witness_for(START_BUCKET);
            p.clock = fx::clock_body(249_999_999, publish + 10);
        });
        assert_eq!(error, ArchiveV2Error::Auth(AuthV2Error::FuturePostedSlot));
    }

    #[test]
    fn the_wrong_feed_id_refuses() {
        let error = refuse_first_append(|p| {
            let (publish, prev) = witness_for(START_BUCKET);
            let mut other = fixture::PROVIDER_FEED_ID;
            other[0] ^= 1;
            p.update = fx::price_update_body(fx::PriceUpdateFixture::new(
                WRITE_AUTHORITY,
                other,
                publish,
                prev,
                250_000_000,
            ));
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::Parser(
                crate::pyth_receiver::PythReceiverError::WrongFeed
            ))
        );
    }

    #[test]
    fn a_tampered_price_refuses_at_the_confidence_cap_or_the_parser() {
        // Raising the confidence past the relative cap is the tamper that
        // survives every structural check and still fails closed.
        let error = refuse_first_append(|p| {
            let (publish, prev) = witness_for(START_BUCKET);
            let mut update = fx::PriceUpdateFixture::new(
                WRITE_AUTHORITY,
                fixture::PROVIDER_FEED_ID,
                publish,
                prev,
                250_000_000,
            );
            update.confidence = 90_000_000;
            p.update = fx::price_update_body(update);
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::ConfidenceCapExceeded)
        );

        // A non-positive price is not adapted into a zero interval.
        let error = refuse_first_append(|p| {
            let (publish, prev) = witness_for(START_BUCKET);
            let mut update = fx::PriceUpdateFixture::new(
                WRITE_AUTHORITY,
                fixture::PROVIDER_FEED_ID,
                publish,
                prev,
                250_000_000,
            );
            update.price = 0;
            p.update = fx::price_update_body(update);
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::Parser(
                crate::pyth_receiver::PythReceiverError::InvalidPrice
            ))
        );
    }

    #[test]
    fn a_wrong_owner_update_account_refuses() {
        let (_, verified) = verified_spec_account();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        initialize_genesis_archive_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            ARCHIVE_BUMP,
        )
        .unwrap();
        let p = presented(START_BUCKET, [0x60; 32]);
        let mut auth = authentication(&p);
        auth.update = PriceUpdateAccountViewV1::new(p.update_key, [0xbe; 32], false, &p.update);
        assert_eq!(
            append_authenticated_v2(&mut archive, verified, fixture::RELEASE, window(), auth),
            Err(ArchiveV2Error::Auth(AuthV2Error::Parser(
                crate::pyth_receiver::PythReceiverError::WrongOwner
            )))
        );
    }

    #[test]
    fn a_partially_verified_update_refuses() {
        let error = refuse_first_append(|p| {
            let (publish, prev) = witness_for(START_BUCKET);
            let mut update = fx::PriceUpdateFixture::new(
                WRITE_AUTHORITY,
                fixture::PROVIDER_FEED_ID,
                publish,
                prev,
                250_000_000,
            );
            update.verification_level = 0;
            p.update = fx::price_update_body(update);
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::Parser(
                crate::pyth_receiver::PythReceiverError::NotFullyVerified
            ))
        );
    }

    #[test]
    fn a_mutated_config_generation_refuses() {
        let error = refuse_first_append(|p| p.config[0] ^= 1);
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::ConfigDigestMismatch)
        );
    }

    #[test]
    fn an_upgraded_programdata_refuses_as_a_new_generation() {
        let error = refuse_first_append(|p| {
            p.programdata = fx::programdata_body(
                fixture::PROGRAMDATA_DEPLOYMENT_SLOT + 1,
                Some([0x77; 32]),
                [0; 32],
                b"upgraded-receiver-elf",
            );
        });
        assert_eq!(
            error,
            ArchiveV2Error::Auth(AuthV2Error::DeploymentSlotMismatch)
        );
    }

    #[test]
    fn a_non_adjacent_post_refuses() {
        let error = refuse_first_append(|p| {
            // set / post / restore: the post is at index 0, but the consuming
            // instruction is at index 2, so `current - 1` is some other
            // instruction entirely.
            /* The intervening instruction is deliberately post-*shaped* -- same
             * seven metas, same positions -- so the refusal is about identity
             * rather than about the ABI running off the end of a short
             * instruction. */
            p.sysvar = fx::instructions_sysvar_body(
                &[
                    fx::post_instruction(
                        fixture::RECEIVER_PROGRAM,
                        fixture::RECEIVER_CONFIG,
                        p.update_key,
                        WRITE_AUTHORITY,
                    ),
                    fx::post_instruction(
                        [0x88; 32],
                        fixture::RECEIVER_CONFIG,
                        p.update_key,
                        WRITE_AUTHORITY,
                    ),
                    fx::consuming_instruction(CLUTCH_PROGRAM),
                ],
                2,
            );
        });
        assert_eq!(error, ArchiveV2Error::Auth(AuthV2Error::WrongPostProgram));
    }

    #[test]
    fn a_post_naming_another_update_account_refuses() {
        let error = refuse_first_append(|p| {
            p.sysvar = fx::instructions_sysvar_body(
                &[
                    fx::post_instruction(
                        fixture::RECEIVER_PROGRAM,
                        fixture::RECEIVER_CONFIG,
                        [0xaa; 32],
                        WRITE_AUTHORITY,
                    ),
                    fx::consuming_instruction(CLUTCH_PROGRAM),
                ],
                1,
            );
        });
        assert_eq!(error, ArchiveV2Error::Auth(AuthV2Error::WrongPostUpdate));
    }

    #[test]
    fn a_reused_stale_update_refuses_at_the_crossing_rule() {
        // Append bucket 0 legitimately, then present the same update again for
        // bucket 1.  It cannot witness the next boundary, so it is refused
        // rather than credited twice.
        let (_, verified) = verified_spec_account();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        initialize_genesis_archive_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            ARCHIVE_BUMP,
        )
        .unwrap();
        let first = presented(START_BUCKET, [0x60; 32]);
        append_authenticated_v2(
            &mut archive,
            verified,
            fixture::RELEASE,
            window(),
            authentication(&first),
        )
        .unwrap();

        /* Advance the Clock past the next boundary's grace so the maturity gate
         * is satisfied and the refusal is unambiguously the crossing rule's:
         * the update's window closed before T(k+1), so it witnesses nothing
         * there no matter how much time passes. */
        let mut replayed = presented(START_BUCKET, [0x60; 32]);
        replayed.clock = fx::clock_body(
            250_000_005,
            ((START_BUCKET + 2) * BUCKET_SECONDS + 10) as i64,
        );

        let before = archive.clone();
        assert_eq!(
            append_authenticated_v2(
                &mut archive,
                verified,
                fixture::RELEASE,
                window(),
                authentication(&replayed),
            )
            .err(),
            Some(ArchiveV2Error::Auth(AuthV2Error::Crossing(
                crate::source_v2::crossing::CrossingError::NotBoundaryWitness
            )))
        );
        assert_eq!(archive, before);
    }

    #[test]
    fn a_page_authored_under_another_spec_refuses() {
        let (archive, verified) = walk_the_window();
        let mut other_fields = spec_fields();
        other_fields.receiver_config[0] ^= 1;
        let other = SourceSpecV2::new(other_fields).unwrap();
        let mut other_image = vec![0_u8; SOURCE_SPEC_ACCOUNT_V2_BYTES];
        initialize_source_spec_v2_account(&mut other_image, other, SPEC_BUMP).unwrap();
        let other_verified = verify_source_spec_v2_account(
            CLUTCH_PROGRAM,
            SPEC_KEY,
            AccountViewV2::new(SPEC_KEY, CLUTCH_PROGRAM, false, &other_image),
        )
        .unwrap();
        assert_ne!(other_verified.feed(), verified.feed());
        assert_eq!(
            verify_recorded_sealed_archive_v2(
                CLUTCH_PROGRAM,
                ARCHIVE_KEY,
                AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
                other_verified,
                fixture::RELEASE,
                window(),
            ),
            Err(ArchiveV2Error::BindingMismatch)
        );
    }

    #[test]
    fn every_page_byte_is_under_the_commitment() {
        let (archive, verified) = walk_the_window();
        fn view(bytes: &[u8]) -> AccountViewV2<'_> {
            AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, bytes)
        }
        verify_recorded_sealed_archive_v2(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            view(&archive),
            verified,
            fixture::RELEASE,
            window(),
        )
        .expect("baseline authenticates");

        // Sample the header, the commitment slot's neighbours, the bump, the
        // reserved tail, every live record, and an unused slot.
        let probes = [
            0_usize,
            ARCHIVE_COUNT_OFFSET,
            ARCHIVE_FEED_OFFSET,
            ARCHIVE_CONFIG_DIGEST_OFFSET,
            ARCHIVE_SOURCE_ACCOUNT_OFFSET,
            ARCHIVE_COMMITMENT_OFFSET - 1,
            ARCHIVE_BUMP_OFFSET,
            ARCHIVE_RESERVED_OFFSET,
            record_offset(0),
            record_offset(0) + 8,
            record_offset(3) + 56,
            record_offset(WINDOW_BUCKETS as usize),
            SOURCE_ARCHIVE_ACCOUNT_V2_BYTES - 1,
        ];
        for at in probes {
            let mut hostile = archive.clone();
            hostile[at] ^= 1;
            assert!(
                verify_recorded_sealed_archive_v2(
                    CLUTCH_PROGRAM,
                    ARCHIVE_KEY,
                    view(&hostile),
                    verified,
                    fixture::RELEASE,
                    window(),
                )
                .is_err(),
                "byte {at} was not load-bearing"
            );
        }

        /* Restamping is not forgery-proof and is not claimed to be: only this
         * program can write a program-owned account, so anyone able to restamp
         * has already written the page.  What the commitment buys is that a
         * *receipt* is bound to the exact bytes that produced it -- a tampered
         * page that restamps cleanly gets a different commitment, and every
         * consumer holding the earlier receipt refuses it. */
        let baseline = verify_recorded_sealed_archive_v2(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            view(&archive),
            verified,
            fixture::RELEASE,
            window(),
        )
        .unwrap();
        let mut restamped = archive.clone();
        restamped[record_offset(0) + 8] ^= 1;
        stamp_commitment(&mut restamped);
        let forged = verify_recorded_sealed_archive_v2(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            view(&restamped),
            verified,
            fixture::RELEASE,
            window(),
        )
        .expect("a restamped page is internally consistent");
        assert_ne!(forged.page_commitment(), baseline.page_commitment());
        assert_eq!(
            archived_record_v2(baseline, view(&restamped), 0),
            Err(ArchiveV2Error::CommitmentMismatch)
        );
    }

    #[test]
    fn a_v1_page_or_spec_account_is_never_read_as_a_v2_one() {
        let (mut image, _) = verified_spec_account();
        image[0] = 0x71; // the V1 SourceSpec tag
        assert_eq!(
            verify_source_spec_v2_account(
                CLUTCH_PROGRAM,
                SPEC_KEY,
                AccountViewV2::new(SPEC_KEY, CLUTCH_PROGRAM, false, &image),
            ),
            Err(ArchiveV2Error::WrongTag)
        );

        let (mut archive, verified) = walk_the_window();
        archive[0] = 0x72; // the V1 archive tag
        assert_eq!(
            verify_recorded_sealed_archive_v2(
                CLUTCH_PROGRAM,
                ARCHIVE_KEY,
                AccountViewV2::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
                verified,
                fixture::RELEASE,
                window(),
            ),
            Err(ArchiveV2Error::WrongTag)
        );
    }

    #[test]
    fn spec_account_metadata_is_authenticated_before_its_body() {
        let (image, _) = verified_spec_account();
        for (view, expected) in [
            (
                AccountViewV2::new([0x99; 32], CLUTCH_PROGRAM, false, &image),
                ArchiveV2Error::AccountMismatch,
            ),
            (
                AccountViewV2::new(SPEC_KEY, [0x98; 32], false, &image),
                ArchiveV2Error::OwnerMismatch,
            ),
            (
                AccountViewV2::new(SPEC_KEY, CLUTCH_PROGRAM, true, &image),
                ArchiveV2Error::ExecutableAccount,
            ),
        ] {
            assert_eq!(
                verify_source_spec_v2_account(CLUTCH_PROGRAM, SPEC_KEY, view),
                Err(expected)
            );
        }

        // A digest that disagrees with the body it frames is refused rather
        // than believed: the identity is recomputed, never trusted.
        let mut tampered = image.clone();
        tampered[SPEC_FEED_OFFSET] ^= 1;
        assert_eq!(
            verify_source_spec_v2_account(
                CLUTCH_PROGRAM,
                SPEC_KEY,
                AccountViewV2::new(SPEC_KEY, CLUTCH_PROGRAM, false, &tampered),
            ),
            Err(ArchiveV2Error::SpecDigestMismatch)
        );
    }

    #[test]
    fn a_window_that_does_not_bind_the_spec_grid_refuses() {
        let (_, verified) = verified_spec_account();
        let feed = FeedIdentity::new(
            fixture::SOURCE_ADAPTER_ID,
            spec().feed_id(),
            fixture::SOURCE_ADAPTER_VERSION,
            1,
        )
        .unwrap();
        let wrong_grid = Grid::new(4, 9, BUCKET_SECONDS * 2).unwrap();
        let hostile = WindowDomain::new(
            feed,
            wrong_grid,
            START_BUCKET,
            START_BUCKET + WINDOW_BUCKETS,
            START_BUCKET + WINDOW_BUCKETS + 1,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .unwrap();
        let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V2_BYTES];
        assert_eq!(
            initialize_genesis_archive_v2(
                &mut archive,
                verified,
                fixture::RELEASE,
                hostile,
                ARCHIVE_BUMP,
            ),
            Err(ArchiveV2Error::InvalidWindow)
        );
    }

    #[test]
    fn the_v2_page_geometry_is_the_v1_page_geometry() {
        // Restated offsets are only safe while the totals agree; this is the
        // check that keeps the restatement honest.
        assert_eq!(SOURCE_ARCHIVE_ACCOUNT_V2_BYTES, 2_560);
        assert_eq!(SOURCE_ARCHIVE_HEADER_V1_BYTES, 512);
        assert_eq!(ARCHIVE_RECORD_V2_BYTES, 64);
        assert_eq!(SOURCE_ARCHIVE_MAX_RECORDS_V2, 32);
        assert_eq!(record_offset(0), 512);
        assert_eq!(record_offset(31), 512 + 31 * 64);
        assert_eq!(
            record_offset(SOURCE_ARCHIVE_MAX_RECORDS_V2 - 1) + ARCHIVE_RECORD_V2_BYTES,
            SOURCE_ARCHIVE_ACCOUNT_V2_BYTES
        );
    }

    #[test]
    fn the_spec_account_is_the_body_plus_its_frame() {
        assert_eq!(
            SOURCE_SPEC_ACCOUNT_V2_BYTES,
            2 + 32 + SOURCE_SPEC_V2_BYTES + 1 + 1
        );
        // A V1 SourceSpec account is 292 bytes; the two families can never be
        // confused by length alone, and the tags differ regardless.
        assert_ne!(
            SOURCE_SPEC_ACCOUNT_V2_BYTES,
            crate::source_archive::SOURCE_SPEC_ACCOUNT_V1_BYTES
        );
    }

    #[test]
    fn the_two_commitment_domains_are_distinct() {
        assert_eq!(
            SOURCE_ARCHIVE_V2_COMMITMENT_DOMAIN,
            b"dragons-clutch/source-archive/v2"
        );
        assert_ne!(
            SOURCE_ARCHIVE_V2_COMMITMENT_DOMAIN,
            b"dragons-clutch/source-archive/v1"
        );
    }
}
