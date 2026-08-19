//! Provider-neutral authenticated source archive for the transparent runtime.
//!
//! This module freezes the account bytes and the fail-closed admission relation
//! needed to turn one source account into an immutable settlement archive.  It
//! deliberately does **not** contain a Pyth (or any other provider) parser.
//! Provider deployment authentication and price parsing are compile-time
//! adapter traits; a production instruction must select both from a closed
//! registry and construct every account view from `AccountInfo`, never from
//! instruction bytes.
//!
//! The live `Resolve` instruction authenticates the canonical account through
//! [`verify_recorded_sealed_archive_view`] before either projecting point
//! evidence or folding occupation evidence.  The verifier checks the account
//! key and owner, source/spec/grid/window lineage, every record link, seal, and
//! recomputed commitment.  A same-domain caller buffer at another key or under
//! another owner therefore cannot produce its lifetime-bound capability.

pub use clutch_accumulator::{CoveragePolicy, FeedIdentity, Grid, WindowDomain};
use clutch_accumulator::{Observation, WINDOW_DOMAIN_BYTES, WINDOW_DOMAIN_TAG};
use clutch_solana_layout::{canonical_feed_id, Hash32};

use crate::source::{
    admit_price, authenticate_source_account, AuthenticatedArchiveV1, PriceParserV1,
    SourceAccountView, SourceError, SourceHeadV1, SourceSpecFieldsV1, SourceSpecV1, TrustedClockV1,
    SOURCE_SPEC_V1_BYTES,
};

/// Exact bytes of an immutable source-spec account.
pub const SOURCE_SPEC_ACCOUNT_V1_BYTES: usize = 292;
/// Exact bytes before the record array in an archive page.
pub const SOURCE_ARCHIVE_HEADER_V1_BYTES: usize = 512;
/// Exact bytes in one archived observation record.
pub const SOURCE_ARCHIVE_RECORD_V1_BYTES: usize = 64;
/// Maximum records in the single-page V1 settlement window.
pub const SOURCE_ARCHIVE_MAX_RECORDS_V1: usize = 32;
/// Exact bytes of one archive account.
pub const SOURCE_ARCHIVE_ACCOUNT_V1_BYTES: usize =
    SOURCE_ARCHIVE_HEADER_V1_BYTES + SOURCE_ARCHIVE_MAX_RECORDS_V1 * SOURCE_ARCHIVE_RECORD_V1_BYTES;

/// Proposed canonical PDA seed for immutable source-spec accounts.
pub const SOURCE_SPEC_SEED_V1: &[u8] = b"source-spec-v1";
/// Proposed canonical PDA seed for per-window archive accounts.
pub const SOURCE_ARCHIVE_SEED_V1: &[u8] = b"source-archive-v1";

const SOURCE_SPEC_ACCOUNT_TAG: u8 = 0x71;
const SOURCE_SPEC_ACCOUNT_VERSION: u8 = 1;
const SOURCE_ARCHIVE_ACCOUNT_TAG: u8 = 0x72;
const SOURCE_ARCHIVE_ACCOUNT_VERSION: u8 = 1;
const SOURCE_ARCHIVE_FLAG_SEALED: u8 = 1;
const SOURCE_ARCHIVE_COMMITMENT_DOMAIN: &[u8] = b"dragons-clutch/source-archive/v1";

const SPEC_FEED_OFFSET: usize = 2;
const SPEC_BODY_OFFSET: usize = 34;
const SPEC_BUMP_OFFSET: usize = 290;
const SPEC_FLAGS_OFFSET: usize = 291;

const ARCHIVE_FLAGS_OFFSET: usize = 2;
const ARCHIVE_COUNT_OFFSET: usize = 3;
const ARCHIVE_FEED_OFFSET: usize = 4;
const ARCHIVE_ADAPTER_OFFSET: usize = 36;
const ARCHIVE_ADAPTER_VERSION_OFFSET: usize = 68;
const ARCHIVE_PARSER_ID_OFFSET: usize = 72;
const ARCHIVE_PARSER_VERSION_OFFSET: usize = 74;
const ARCHIVE_PROVIDER_PROGRAM_OFFSET: usize = 76;
const ARCHIVE_PROVIDER_PROGRAM_OWNER_OFFSET: usize = 108;
const ARCHIVE_DEPLOYMENT_ACCOUNT_OFFSET: usize = 140;
const ARCHIVE_DEPLOYMENT_OWNER_OFFSET: usize = 172;
const ARCHIVE_DEPLOYMENT_VERIFIER_OFFSET: usize = 204;
const ARCHIVE_DEPLOYMENT_VERIFIER_VERSION_OFFSET: usize = 236;
const ARCHIVE_SOURCE_ACCOUNT_OFFSET: usize = 240;
const ARCHIVE_SOURCE_OWNER_OFFSET: usize = 272;
const ARCHIVE_DEPLOYMENT_GENERATION_OFFSET: usize = 304;
const ARCHIVE_GRID_FAMILY_OFFSET: usize = 312;
const ARCHIVE_GRID_VERSION_OFFSET: usize = 316;
const ARCHIVE_GRID_PADDING_OFFSET: usize = 318;
const ARCHIVE_BUCKET_SECONDS_OFFSET: usize = 320;
const ARCHIVE_WINDOW_OFFSET: usize = 328;
const ARCHIVE_WINDOW_START_OFFSET: usize = 360;
const ARCHIVE_WINDOW_END_OFFSET: usize = 368;
const ARCHIVE_MATURITY_OFFSET: usize = 376;
const ARCHIVE_REPAIR_GENERATION_OFFSET: usize = 384;
const ARCHIVE_PAGE_INDEX_OFFSET: usize = 392;
const ARCHIVE_FIRST_BUCKET_OFFSET: usize = 400;
const ARCHIVE_SEALED_FEED_CURSOR_OFFSET: usize = 408;
const ARCHIVE_PREVIOUS_SEQUENCE_OFFSET: usize = 416;
const ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET: usize = 424;
const ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET: usize = 432;
const ARCHIVE_PREVIOUS_COMMITMENT_OFFSET: usize = 440;
const ARCHIVE_COMMITMENT_OFFSET: usize = 472;
const ARCHIVE_BUMP_OFFSET: usize = 504;
const ARCHIVE_RESERVED_OFFSET: usize = 505;

const _: () = assert!(SOURCE_SPEC_ACCOUNT_V1_BYTES == 292);
const _: () = assert!(SOURCE_ARCHIVE_HEADER_V1_BYTES == 512);
const _: () = assert!(SOURCE_ARCHIVE_RECORD_V1_BYTES == 64);
const _: () = assert!(SOURCE_ARCHIVE_ACCOUNT_V1_BYTES == 2_560);

/// Refusals from source-spec/account codecs, provider authentication, archive
/// mutation, and sealed receipt verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceArchiveError {
    /// An account did not have the one exact V1 length.
    WrongLength,
    /// The account tag was not the V1 source-spec or archive tag.
    WrongTag,
    /// The account version is unsupported.
    WrongVersion,
    /// Reserved bytes, flags, padding, or unused records were noncanonical.
    NonCanonicalPadding,
    /// A required key, digest, or adapter identity was zero.
    ZeroIdentity,
    /// The stored source-spec digest was not the digest of its exact body.
    SourceSpecDigestMismatch,
    /// The runtime source-spec account key is not the canonical expected key.
    SourceSpecAccountMismatch,
    /// The runtime source-spec account is not owned by Dragon's Clutch.
    SourceSpecOwnerMismatch,
    /// A data-bearing source-spec account was executable.
    SourceSpecExecutable,
    /// A source-spec body was structurally invalid or noncanonical.
    InvalidSourceSpec,
    /// The window is not the complete, at-most-32-bucket profile admitted by V1.
    InvalidWindow,
    /// The window's feed, adapter, source version, or grid differs from the spec.
    WindowBindingMismatch,
    /// The stored window identity is not the digest of its exact canonical domain.
    WindowIdentityMismatch,
    /// The compile-time deployment adapter has an invalid release identity.
    InvalidDeploymentAdapter,
    /// The provider program key differed from the compile-time adapter.
    ProviderProgramMismatch,
    /// The provider program owner differed from the compile-time adapter.
    ProviderProgramOwnerMismatch,
    /// The provider program account was not executable.
    ProviderProgramNotExecutable,
    /// The deployment evidence key differed from the compile-time adapter.
    DeploymentAccountMismatch,
    /// The deployment evidence owner differed from the compile-time adapter.
    DeploymentOwnerMismatch,
    /// The deployment evidence account was executable.
    DeploymentAccountExecutable,
    /// The deployment adapter refused its pinned provider bytes.
    DeploymentAdapterRefused,
    /// The authenticated deployment generation differed from the immutable spec.
    DeploymentGenerationMismatch,
    /// The page's frozen parser or deployment adapter release differed.
    AdapterReleaseMismatch,
    /// The archive was already sealed.
    AlreadySealed,
    /// The archive is not sealed.
    NotSealed,
    /// The archive already contains the maximum or full-window record count.
    ArchiveFull,
    /// A record did not continue the exact bucket/sequence/time lineage.
    NonContiguousLineage,
    /// The page commitment did not recompute from the exact canonical bytes.
    CommitmentMismatch,
    /// The page was not complete or the witnessed feed cursor was not mature.
    NotMature,
    /// The runtime archive account key is not the canonical expected key.
    ArchiveAccountMismatch,
    /// The runtime archive account is not owned by the Dragon's Clutch program.
    ArchiveOwnerMismatch,
    /// A data-bearing archive account was executable.
    ArchiveExecutable,
    /// The page's source, grid, window, generation, or predecessor differs.
    ArchiveBindingMismatch,
    /// An admitted record was malformed.
    MalformedRecord,
    /// The source admission kernel refused the candidate.
    Source(SourceError),
}

impl From<SourceError> for SourceArchiveError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

/// Runtime metadata and bytes for a provider program or deployment account.
///
/// A production SBF seam constructs this from `AccountInfo`.  Instruction data
/// is never an acceptable source of these fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccountViewV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> RuntimeAccountViewV1<'a> {
    /// Construct a view at the `AccountInfo` boundary.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }

    /// Account data visible only to the compile-time deployment adapter.
    pub const fn data(self) -> &'a [u8] {
        self.data
    }
}

/// Compile-time provider deployment authenticator.
///
/// A concrete implementation is consensus-critical.  It must parse a pinned
/// official loader/provider layout and return the exact generation represented
/// by the two authenticated accounts.  The program must select the
/// implementation from a closed registry; allowing the caller to choose the
/// generic parameter would void the authentication claim.
pub trait DeploymentAuthenticatorV1 {
    /// Reviewed implementation identity.
    const VERIFIER_ID: [u8; 32];
    /// Reviewed implementation version.
    const VERIFIER_VERSION: u32;
    /// Exact provider program key.
    const PROVIDER_PROGRAM: [u8; 32];
    /// Exact loader/owner of the provider program account.
    const PROVIDER_PROGRAM_OWNER: [u8; 32];
    /// Exact deployment-evidence account key.
    const DEPLOYMENT_ACCOUNT: [u8; 32];
    /// Exact owner of the deployment-evidence account.
    const DEPLOYMENT_OWNER: [u8; 32];

    /// Authenticate pinned account bytes and return their deployment generation.
    fn deployment_generation(
        provider_program_data: &[u8],
        deployment_account_data: &[u8],
    ) -> Result<u64, SourceArchiveError>;
}

/// Exact predecessor facts that begin one archive's source lineage.
///
/// A live initialization instruction must obtain these from the authenticated
/// feed head.  The all-zero value is reserved for the first feed record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivePredecessorV1 {
    /// Last source-native sequence before this archive.
    pub source_sequence: u64,
    /// Last source publish slot before this archive.
    pub publish_slot: u64,
    /// Last source publish time before this archive.
    pub publish_time: u64,
    /// Commitment of the immediately preceding sealed archive, or zero at genesis.
    pub archive_commitment: Hash32,
}

impl ArchivePredecessorV1 {
    /// The unique genesis predecessor.
    pub const GENESIS: Self = Self {
        source_sequence: 0,
        publish_slot: 0,
        publish_time: 0,
        archive_commitment: Hash32::ZERO,
    };
}

/// Opaque predecessor capability admitted for a new archive.
///
/// Callers cannot manufacture this value from sequence/time fields. It is
/// produced only for a generation-zero first page or from a verified sealed
/// predecessor receipt whose feed, deployment, generation, and boundary are
/// contiguous with the new window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedArchivePredecessorV1(ArchivePredecessorV1);

impl VerifiedArchivePredecessorV1 {
    /// Admit the unique no-predecessor state for a generation-zero window.
    pub fn genesis(window: WindowDomain) -> Result<Self, SourceArchiveError> {
        if window.generation() != 0 {
            return Err(SourceArchiveError::NonContiguousLineage);
        }
        Ok(Self(ArchivePredecessorV1::GENESIS))
    }

    /// Derive exact successor lineage from a verified sealed archive receipt.
    pub fn successor(
        receipt: SealedArchiveReceiptV1,
        spec: SourceSpecV1,
        next: WindowDomain,
    ) -> Result<Self, SourceArchiveError> {
        validate_window(spec, next)?;
        if receipt.feed != spec.feed_id()
            || receipt.deployment_generation != spec.deployment_generation()
            || receipt.repair_generation != next.generation()
            || receipt.end_bucket_exclusive != next.start_bucket()
        {
            return Err(SourceArchiveError::NonContiguousLineage);
        }
        Ok(Self(ArchivePredecessorV1 {
            source_sequence: receipt.last_source_sequence,
            publish_slot: receipt.last_publish_slot,
            publish_time: receipt.last_publish_time,
            archive_commitment: receipt.page_commitment,
        }))
    }

    const fn facts(self) -> ArchivePredecessorV1 {
        self.0
    }
}

/// Decoded immutable source-spec account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSourceSpecAccountV1 {
    account_key: [u8; 32],
    spec: SourceSpecV1,
    feed: Hash32,
    stored_bump: u8,
}

impl VerifiedSourceSpecAccountV1 {
    /// Authenticated source-spec account key.
    pub const fn account_key(self) -> [u8; 32] {
        self.account_key
    }

    /// Validated source specification.
    pub const fn spec(self) -> SourceSpecV1 {
        self.spec
    }

    /// Canonical digest/FeedId of the exact source-spec body.
    pub const fn feed(self) -> Hash32 {
        self.feed
    }

    /// Canonical PDA bump stored by initialization.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// Initialize one exact immutable source-spec account image.
pub fn initialize_source_spec_account(
    out: &mut [u8],
    spec: SourceSpecV1,
    stored_bump: u8,
) -> Result<(), SourceArchiveError> {
    exact_len(out, SOURCE_SPEC_ACCOUNT_V1_BYTES)?;
    out.fill(0);
    out[0] = SOURCE_SPEC_ACCOUNT_TAG;
    out[1] = SOURCE_SPEC_ACCOUNT_VERSION;
    let body = spec.encode_canonical();
    let feed = spec.feed_id();
    out[SPEC_FEED_OFFSET..SPEC_FEED_OFFSET + 32].copy_from_slice(&feed.bytes());
    out[SPEC_BODY_OFFSET..SPEC_BODY_OFFSET + SOURCE_SPEC_V1_BYTES].copy_from_slice(&body);
    out[SPEC_BUMP_OFFSET] = stored_bump;
    decode_source_spec_account(out).map(|_| ())
}

/// Authenticate one compile-time parser/deployment release and initialize its
/// immutable SourceSpec account image.
///
/// This route authenticates metadata only. It never parses the source account
/// or chooses a price; value admission remains exclusively in
/// [`append_authenticated`].
#[allow(clippy::too_many_arguments)]
pub fn initialize_authenticated_source_spec_account<
    P: PriceParserV1,
    D: DeploymentAuthenticatorV1,
>(
    out: &mut [u8],
    spec: SourceSpecV1,
    stored_bump: u8,
    provider_program: RuntimeAccountViewV1<'_>,
    deployment_account: RuntimeAccountViewV1<'_>,
    source_account: SourceAccountView<'_>,
) -> Result<(), SourceArchiveError> {
    authenticate_deployment::<D>(spec, provider_program, deployment_account)?;
    authenticate_source_account(spec, source_account)?;
    if P::SOURCE_ADAPTER_ID != spec.source_adapter_id().bytes()
        || P::SOURCE_ADAPTER_VERSION != spec.source_adapter_version()
        || P::PARSER_ID != spec.parser_id()
        || P::PARSER_VERSION != spec.parser_version()
    {
        return Err(SourceArchiveError::AdapterReleaseMismatch);
    }
    initialize_source_spec_account(out, spec, stored_bump)
}

/// Decode and validate one exact canonical SourceSpec body supplied by a
/// construction instruction.
pub fn decode_source_spec_body_v1(input: &[u8]) -> Result<SourceSpecV1, SourceArchiveError> {
    exact_len(input, SOURCE_SPEC_V1_BYTES)?;
    if &input[..8] != b"DCSRCV1\0" || u16_at(input, 8) != 1 {
        return Err(SourceArchiveError::InvalidSourceSpec);
    }
    if input[250..].iter().any(|byte| *byte != 0) {
        return Err(SourceArchiveError::NonCanonicalPadding);
    }
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: Hash32::from_bytes(array_32(input, 10)),
        source_adapter_version: u32_at(input, 42),
        parser_id: u16_at(input, 46),
        parser_version: u16_at(input, 48),
        source_program: array_32(input, 50),
        source_account: array_32(input, 82),
        deployment_generation: u64_at(input, 114),
        base_asset_id: Hash32::from_bytes(array_32(input, 122)),
        quote_asset_id: Hash32::from_bytes(array_32(input, 154)),
        orientation: input[186],
        normalized_decimals: input[187],
        grid_family_id: u32_at(input, 188),
        grid_version: u16_at(input, 192),
        bucket_seconds: u64_at(input, 194),
        max_staleness_slots: u64_at(input, 202),
        max_staleness_seconds: u64_at(input, 210),
        max_future_seconds: u64_at(input, 218),
        max_confidence_atoms: u128_at(input, 226),
        max_confidence_bps: u32_at(input, 242),
        confidence_multiplier: u16_at(input, 246),
        selection_rule: u16_at(input, 248),
    })
    .map_err(|_| SourceArchiveError::InvalidSourceSpec)
}

/// Runtime metadata and bytes for an immutable source-spec account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecAccountViewV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> SourceSpecAccountViewV1<'a> {
    /// Construct a source-spec view from `AccountInfo` fields.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }
}

/// Authenticate and decode one immutable source-spec runtime account.
///
/// `expected_source_spec_key` must be the canonical PDA derived from
/// [`SOURCE_SPEC_SEED_V1`] and the feed id by the SBF seam.
pub fn verify_source_spec_account(
    clutch_program: [u8; 32],
    expected_source_spec_key: [u8; 32],
    account: SourceSpecAccountViewV1<'_>,
) -> Result<VerifiedSourceSpecAccountV1, SourceArchiveError> {
    if is_zero(&clutch_program) || is_zero(&expected_source_spec_key) {
        return Err(SourceArchiveError::ZeroIdentity);
    }
    if account.key != expected_source_spec_key {
        return Err(SourceArchiveError::SourceSpecAccountMismatch);
    }
    if account.owner != clutch_program {
        return Err(SourceArchiveError::SourceSpecOwnerMismatch);
    }
    if account.executable {
        return Err(SourceArchiveError::SourceSpecExecutable);
    }
    let mut verified = decode_source_spec_account(account.data)?;
    verified.account_key = account.key;
    Ok(verified)
}

fn decode_source_spec_account(
    input: &[u8],
) -> Result<VerifiedSourceSpecAccountV1, SourceArchiveError> {
    exact_len(input, SOURCE_SPEC_ACCOUNT_V1_BYTES)?;
    if input[0] != SOURCE_SPEC_ACCOUNT_TAG {
        return Err(SourceArchiveError::WrongTag);
    }
    if input[1] != SOURCE_SPEC_ACCOUNT_VERSION {
        return Err(SourceArchiveError::WrongVersion);
    }
    if input[SPEC_FLAGS_OFFSET] != 0 {
        return Err(SourceArchiveError::NonCanonicalPadding);
    }
    let body = &input[SPEC_BODY_OFFSET..SPEC_BODY_OFFSET + SOURCE_SPEC_V1_BYTES];
    let spec = decode_source_spec_body_v1(body)?;
    let feed = Hash32::from_bytes(array_32(input, SPEC_FEED_OFFSET));
    if feed == Hash32::ZERO || feed != canonical_feed_id(body) || feed != spec.feed_id() {
        return Err(SourceArchiveError::SourceSpecDigestMismatch);
    }
    Ok(VerifiedSourceSpecAccountV1 {
        account_key: [0; 32],
        spec,
        feed,
        stored_bump: input[SPEC_BUMP_OFFSET],
    })
}

/// Initialize an open, empty per-window archive with an exact source predecessor.
///
/// V1 admits only a complete window of at most 32 buckets and stores it in one
/// page (`page_index = 0`).  The source spec and full canonical window domain
/// determine every mutable record's expected bucket.
pub fn initialize_archive<D: DeploymentAuthenticatorV1>(
    out: &mut [u8],
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    predecessor: ArchivePredecessorV1,
    stored_bump: u8,
) -> Result<(), SourceArchiveError> {
    exact_len(out, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES)?;
    validate_deployment_adapter::<D>()?;
    validate_window(verified_spec.spec, window)?;
    let window_id = canonical_window_id(window);
    let spec_bytes = verified_spec.spec.encode_canonical();
    let parser_id = u16_at(&spec_bytes, 46);
    let parser_version = u16_at(&spec_bytes, 48);

    out.fill(0);
    out[0] = SOURCE_ARCHIVE_ACCOUNT_TAG;
    out[1] = SOURCE_ARCHIVE_ACCOUNT_VERSION;
    put_32(out, ARCHIVE_FEED_OFFSET, verified_spec.feed.bytes());
    put_32(
        out,
        ARCHIVE_ADAPTER_OFFSET,
        verified_spec.spec.source_adapter_id().bytes(),
    );
    put_u32(
        out,
        ARCHIVE_ADAPTER_VERSION_OFFSET,
        verified_spec.spec.source_adapter_version(),
    );
    put_u16(out, ARCHIVE_PARSER_ID_OFFSET, parser_id);
    put_u16(out, ARCHIVE_PARSER_VERSION_OFFSET, parser_version);
    put_32(out, ARCHIVE_PROVIDER_PROGRAM_OFFSET, D::PROVIDER_PROGRAM);
    put_32(
        out,
        ARCHIVE_PROVIDER_PROGRAM_OWNER_OFFSET,
        D::PROVIDER_PROGRAM_OWNER,
    );
    put_32(
        out,
        ARCHIVE_DEPLOYMENT_ACCOUNT_OFFSET,
        D::DEPLOYMENT_ACCOUNT,
    );
    put_32(out, ARCHIVE_DEPLOYMENT_OWNER_OFFSET, D::DEPLOYMENT_OWNER);
    put_32(out, ARCHIVE_DEPLOYMENT_VERIFIER_OFFSET, D::VERIFIER_ID);
    put_u32(
        out,
        ARCHIVE_DEPLOYMENT_VERIFIER_VERSION_OFFSET,
        D::VERIFIER_VERSION,
    );
    put_32(
        out,
        ARCHIVE_SOURCE_ACCOUNT_OFFSET,
        verified_spec.spec.source_account(),
    );
    put_32(
        out,
        ARCHIVE_SOURCE_OWNER_OFFSET,
        verified_spec.spec.source_program(),
    );
    put_u64(
        out,
        ARCHIVE_DEPLOYMENT_GENERATION_OFFSET,
        verified_spec.spec.deployment_generation(),
    );
    let grid = verified_spec.spec.grid();
    put_u32(out, ARCHIVE_GRID_FAMILY_OFFSET, grid.family_id());
    put_u16(out, ARCHIVE_GRID_VERSION_OFFSET, grid.version());
    put_u64(out, ARCHIVE_BUCKET_SECONDS_OFFSET, grid.bucket_seconds());
    put_32(out, ARCHIVE_WINDOW_OFFSET, window_id.bytes());
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
    put_u64(
        out,
        ARCHIVE_PREVIOUS_SEQUENCE_OFFSET,
        predecessor.source_sequence,
    );
    put_u64(
        out,
        ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET,
        predecessor.publish_slot,
    );
    put_u64(
        out,
        ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET,
        predecessor.publish_time,
    );
    put_32(
        out,
        ARCHIVE_PREVIOUS_COMMITMENT_OFFSET,
        predecessor.archive_commitment.bytes(),
    );
    out[ARCHIVE_BUMP_OFFSET] = stored_bump;
    stamp_commitment(out);
    verify_open_archive::<D>(out, verified_spec.spec, window).map(|_| ())
}

/// Initialize a generation-zero archive without caller-authored predecessor
/// facts.
pub fn initialize_genesis_archive<D: DeploymentAuthenticatorV1>(
    out: &mut [u8],
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    stored_bump: u8,
) -> Result<(), SourceArchiveError> {
    let predecessor = VerifiedArchivePredecessorV1::genesis(window)?;
    initialize_archive::<D>(out, verified_spec, window, predecessor.facts(), stored_bump)
}

/// Initialize a successor archive from one already verified sealed receipt.
pub fn initialize_successor_archive<D: DeploymentAuthenticatorV1>(
    out: &mut [u8],
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    predecessor: SealedArchiveReceiptV1,
    stored_bump: u8,
) -> Result<(), SourceArchiveError> {
    let predecessor =
        VerifiedArchivePredecessorV1::successor(predecessor, verified_spec.spec, window)?;
    initialize_archive::<D>(out, verified_spec, window, predecessor.facts(), stored_bump)
}

/// Authenticate one provider record and append it atomically to an open archive.
///
/// No page byte changes until provider metadata, deployment provenance, source
/// parsing, freshness, confidence, bucket selection, and predecessor lineage
/// all pass.  This makes every refusal a byte-for-byte rollback for a caller
/// that holds the account borrow through this call.
pub fn append_authenticated<P: PriceParserV1, D: DeploymentAuthenticatorV1>(
    archive: &mut [u8],
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    clock: TrustedClockV1,
    provider_program: RuntimeAccountViewV1<'_>,
    deployment_account: RuntimeAccountViewV1<'_>,
    source_account: SourceAccountView<'_>,
) -> Result<(), SourceArchiveError> {
    let header = verify_open_archive::<D>(archive, verified_spec.spec, window)?;
    authenticate_deployment::<D>(verified_spec.spec, provider_program, deployment_account)?;
    if P::SOURCE_ADAPTER_ID != verified_spec.spec.source_adapter_id().bytes()
        || P::SOURCE_ADAPTER_VERSION != verified_spec.spec.source_adapter_version()
        || P::PARSER_ID != header.parser_id
        || P::PARSER_VERSION != header.parser_version
    {
        return Err(SourceArchiveError::AdapterReleaseMismatch);
    }
    if usize::from(header.record_count) >= SOURCE_ARCHIVE_MAX_RECORDS_V1
        || u64::from(header.record_count) >= header.window_end - header.window_start
    {
        return Err(SourceArchiveError::ArchiveFull);
    }

    let previous = last_lineage(archive, header)?;
    let head = SourceHeadV1 {
        feed: header.feed,
        grid: header.grid,
        cursor: header
            .first_bucket
            .checked_add(u64::from(header.record_count))
            .ok_or(SourceArchiveError::NonContiguousLineage)?,
        deployment_generation: header.deployment_generation,
        last_source_sequence: previous.source_sequence,
        last_publish_slot: previous.publish_slot,
        last_publish_time: previous.publish_time,
    };
    let admitted = admit_price::<P>(verified_spec.spec, head, clock, source_account)?;
    let (bucket, low, high) = match admitted.observation {
        Observation::Accepted { bucket, value } => (bucket, value.low(), value.high()),
        Observation::Missing { .. } => return Err(SourceArchiveError::MalformedRecord),
    };
    if bucket != head.cursor
        || admitted.source_sequence <= previous.source_sequence
        || admitted.publish_slot < previous.publish_slot
        || admitted.publish_time < previous.publish_time
    {
        return Err(SourceArchiveError::NonContiguousLineage);
    }

    let record_offset = record_offset(usize::from(header.record_count));
    put_u64(archive, record_offset, bucket);
    put_u128(archive, record_offset + 8, low);
    put_u128(archive, record_offset + 24, high);
    put_u64(archive, record_offset + 40, admitted.source_sequence);
    put_u64(archive, record_offset + 48, admitted.publish_slot);
    put_u64(archive, record_offset + 56, admitted.publish_time);
    archive[ARCHIVE_COUNT_OFFSET] = header.record_count + 1;
    stamp_commitment(archive);
    Ok(())
}

/// Seal a complete archive after witnessing an authenticated feed cursor at or
/// beyond the immutable maturity boundary.
pub fn seal_archive<D: DeploymentAuthenticatorV1>(
    archive: &mut [u8],
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
    authenticated_feed_cursor: u64,
) -> Result<(), SourceArchiveError> {
    let header = verify_open_archive::<D>(archive, verified_spec.spec, window)?;
    if u64::from(header.record_count) != header.window_end - header.window_start
        || authenticated_feed_cursor < header.maturity_bucket_exclusive
    {
        return Err(SourceArchiveError::NotMature);
    }
    archive[ARCHIVE_FLAGS_OFFSET] = SOURCE_ARCHIVE_FLAG_SEALED;
    put_u64(
        archive,
        ARCHIVE_SEALED_FEED_CURSOR_OFFSET,
        authenticated_feed_cursor,
    );
    stamp_commitment(archive);
    Ok(())
}

/// Runtime metadata and bytes for a sealed archive account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveAccountViewV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> ArchiveAccountViewV1<'a> {
    /// Construct a runtime archive view from `AccountInfo` fields.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }
}

/// Receipt returned only after a sealed archive account and every lineage fact
/// have been authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedArchiveReceiptV1 {
    archive_key: [u8; 32],
    archive_owner: [u8; 32],
    feed: Hash32,
    window: Hash32,
    page_commitment: Hash32,
    deployment_generation: u64,
    repair_generation: u64,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    sealed_feed_cursor: u64,
    last_source_sequence: u64,
    last_publish_slot: u64,
    last_publish_time: u64,
    stored_bump: u8,
}

impl SealedArchiveReceiptV1 {
    /// Authenticated archive account key.
    pub const fn archive_key(self) -> [u8; 32] {
        self.archive_key
    }

    /// Canonical source-spec digest/feed identity.
    pub const fn feed(self) -> Hash32 {
        self.feed
    }

    /// Canonical immutable window-domain identity.
    pub const fn window(self) -> Hash32 {
        self.window
    }

    /// Commitment recomputed from the exact sealed page bytes.
    pub const fn page_commitment(self) -> Hash32 {
        self.page_commitment
    }

    /// Pinned provider deployment generation.
    pub const fn deployment_generation(self) -> u64 {
        self.deployment_generation
    }

    /// Immutable repair generation of the archived window.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// Inclusive first archived bucket.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Exclusive last archived bucket.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }

    /// Authenticated feed cursor witnessed at sealing.
    pub const fn sealed_feed_cursor(self) -> u64 {
        self.sealed_feed_cursor
    }

    /// Last source-native sequence in the page.
    pub const fn last_source_sequence(self) -> u64 {
        self.last_source_sequence
    }

    /// Last source publish slot in the page.
    pub const fn last_publish_slot(self) -> u64 {
        self.last_publish_slot
    }

    /// Last source publish time in the page.
    pub const fn last_publish_time(self) -> u64 {
        self.last_publish_time
    }

    /// Canonical archive PDA bump stored in the sealed page.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }

    /// Project into the older typed resolution-archive relation only after the
    /// stronger key/owner/window verifier has constructed this receipt.
    pub const fn authenticated_archive(self) -> AuthenticatedArchiveV1 {
        AuthenticatedArchiveV1 {
            feed: self.feed,
            deployment_generation: self.deployment_generation,
            start_bucket: self.start_bucket,
            end_bucket_exclusive: self.end_bucket_exclusive,
            summary_digest: self.page_commitment,
            sealed: true,
        }
    }
}

/// Lifetime-bound read capability for one fully verified sealed archive page.
///
/// Construction runs the same key, owner, executable, source-spec, release,
/// window, lineage, padding, and page-commitment checks as
/// [`verify_recorded_sealed_archive`].  The borrowed bytes cannot be mutated
/// while this value is live, so indexed reads need not hash the entire
/// 2,560-byte page again.  No raw slice accessor exists: consumers can read
/// only checked [`ArchivedObservationV1`] records and the authenticated
/// receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSealedArchiveViewV1<'a> {
    receipt: SealedArchiveReceiptV1,
    data: &'a [u8],
}

impl VerifiedSealedArchiveViewV1<'_> {
    /// Return the authenticated archive/window provenance.
    pub const fn receipt(self) -> SealedArchiveReceiptV1 {
        self.receipt
    }

    /// Read one bounded record from the immutable page verified at construction.
    ///
    /// The index remains hostile and is checked against both the committed
    /// record count and the compile-time page bound.  Page bytes and metadata
    /// are not caller-selectable after this capability has been constructed.
    pub fn archived_observation(
        self,
        index: usize,
    ) -> Result<ArchivedObservationV1, SourceArchiveError> {
        archived_observation_at(self.data, index)
    }
}

/// Authenticate a sealed archive for a later `Resolve` join.
///
/// `expected_archive_key` must be the canonical PDA derived by the SBF seam
/// from [`SOURCE_ARCHIVE_SEED_V1`], the source-spec digest, and the window id.
/// This function additionally checks runtime owner/executable metadata, so a
/// caller buffer with identical records and domain bytes is unusable.
pub fn verify_sealed_archive<D: DeploymentAuthenticatorV1>(
    clutch_program: [u8; 32],
    expected_archive_key: [u8; 32],
    account: ArchiveAccountViewV1<'_>,
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
) -> Result<SealedArchiveReceiptV1, SourceArchiveError> {
    if is_zero(&clutch_program) || is_zero(&expected_archive_key) {
        return Err(SourceArchiveError::ZeroIdentity);
    }
    if account.key != expected_archive_key {
        return Err(SourceArchiveError::ArchiveAccountMismatch);
    }
    if account.owner != clutch_program {
        return Err(SourceArchiveError::ArchiveOwnerMismatch);
    }
    if account.executable {
        return Err(SourceArchiveError::ArchiveExecutable);
    }
    let header = verify_archive_release(
        account.data,
        verified_spec.spec,
        window,
        true,
        deployment_release::<D>()?,
    )?;
    let last = last_lineage(account.data, header)?;
    Ok(SealedArchiveReceiptV1 {
        archive_key: account.key,
        archive_owner: account.owner,
        feed: header.feed,
        window: header.window,
        page_commitment: header.page_commitment,
        deployment_generation: header.deployment_generation,
        repair_generation: header.repair_generation,
        start_bucket: header.window_start,
        end_bucket_exclusive: header.window_end,
        sealed_feed_cursor: header.sealed_feed_cursor,
        last_source_sequence: last.source_sequence,
        last_publish_slot: last.publish_slot,
        last_publish_time: last.publish_time,
        stored_bump: account.data[ARCHIVE_BUMP_OFFSET],
    })
}

/// Authenticate a sealed archive by its canonical program-owned receipt.
///
/// Unlike [`verify_sealed_archive`], this function does not re-run or select a
/// provider deployment adapter.  It is the resolution-time verifier: the exact
/// program-owned key, sealed flag, source-spec binding, recorded nonzero
/// adapter/parser/deployment release, record lineage, and page commitment are
/// the historical admission capability.  A production archive can reach this
/// state only through the program's closed-registry append/seal route.  This
/// function therefore makes no claim that such a route or a production oracle
/// adapter exists today.
pub fn verify_recorded_sealed_archive(
    clutch_program: [u8; 32],
    expected_archive_key: [u8; 32],
    account: ArchiveAccountViewV1<'_>,
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
) -> Result<SealedArchiveReceiptV1, SourceArchiveError> {
    if is_zero(&clutch_program) || is_zero(&expected_archive_key) {
        return Err(SourceArchiveError::ZeroIdentity);
    }
    if account.key != expected_archive_key {
        return Err(SourceArchiveError::ArchiveAccountMismatch);
    }
    if account.owner != clutch_program {
        return Err(SourceArchiveError::ArchiveOwnerMismatch);
    }
    if account.executable {
        return Err(SourceArchiveError::ArchiveExecutable);
    }
    let release = recorded_deployment_release(account.data, verified_spec.spec)?;
    let header = verify_archive_release(account.data, verified_spec.spec, window, true, release)?;
    let last = last_lineage(account.data, header)?;
    Ok(SealedArchiveReceiptV1 {
        archive_key: account.key,
        archive_owner: account.owner,
        feed: header.feed,
        window: header.window,
        page_commitment: header.page_commitment,
        deployment_generation: header.deployment_generation,
        repair_generation: header.repair_generation,
        start_bucket: header.window_start,
        end_bucket_exclusive: header.window_end,
        sealed_feed_cursor: header.sealed_feed_cursor,
        last_source_sequence: last.source_sequence,
        last_publish_slot: last.publish_slot,
        last_publish_time: last.publish_time,
        stored_bump: account.data[ARCHIVE_BUMP_OFFSET],
    })
}

/// Authenticate one recorded sealed archive and retain its immutable bytes for
/// bounded record reads without repeated whole-page hashing.
///
/// This is the live-fold form of [`verify_recorded_sealed_archive`].  It does
/// not weaken or replace that receipt API; it first runs it on the exact same
/// account view, then binds the resulting private receipt to the lifetime of
/// those already verified bytes.
pub fn verify_recorded_sealed_archive_view(
    clutch_program: [u8; 32],
    expected_archive_key: [u8; 32],
    account: ArchiveAccountViewV1<'_>,
    verified_spec: VerifiedSourceSpecAccountV1,
    window: WindowDomain,
) -> Result<VerifiedSealedArchiveViewV1<'_>, SourceArchiveError> {
    let receipt = verify_recorded_sealed_archive(
        clutch_program,
        expected_archive_key,
        account,
        verified_spec,
        window,
    )?;
    Ok(VerifiedSealedArchiveViewV1 {
        receipt,
        data: account.data,
    })
}

/// One exact value record read back from a verified sealed archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivedObservationV1 {
    /// Canonical bucket.
    pub bucket: u64,
    /// Conservative low endpoint.
    pub low: u128,
    /// Conservative high endpoint.
    pub high: u128,
    /// Source-native sequence.
    pub source_sequence: u64,
    /// Source publish slot.
    pub publish_slot: u64,
    /// Source publish time.
    pub publish_time: u64,
}

/// Read one observation from the exact account that produced `receipt`.
///
/// The account key, owner, executable bit, length, and recomputed commitment
/// are repeated so a receipt cannot be paired with another page's bytes.
pub fn archived_observation(
    receipt: SealedArchiveReceiptV1,
    account: ArchiveAccountViewV1<'_>,
    index: usize,
) -> Result<ArchivedObservationV1, SourceArchiveError> {
    if account.key != receipt.archive_key {
        return Err(SourceArchiveError::ArchiveAccountMismatch);
    }
    if account.owner != receipt.archive_owner {
        return Err(SourceArchiveError::ArchiveOwnerMismatch);
    }
    if account.executable {
        return Err(SourceArchiveError::ArchiveExecutable);
    }
    exact_len(account.data, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES)?;
    if page_commitment(account.data) != receipt.page_commitment {
        return Err(SourceArchiveError::CommitmentMismatch);
    }
    archived_observation_at(account.data, index)
}

fn archived_observation_at(
    data: &[u8],
    index: usize,
) -> Result<ArchivedObservationV1, SourceArchiveError> {
    let count = usize::from(data[ARCHIVE_COUNT_OFFSET]);
    if index >= count || index >= SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        return Err(SourceArchiveError::MalformedRecord);
    }
    let offset = record_offset(index);
    Ok(ArchivedObservationV1 {
        bucket: u64_at(data, offset),
        low: u128_at(data, offset + 8),
        high: u128_at(data, offset + 24),
        source_sequence: u64_at(data, offset + 40),
        publish_slot: u64_at(data, offset + 48),
        publish_time: u64_at(data, offset + 56),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LineageV1 {
    source_sequence: u64,
    publish_slot: u64,
    publish_time: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArchiveHeaderV1 {
    feed: Hash32,
    window: Hash32,
    page_commitment: Hash32,
    grid: Grid,
    parser_id: u16,
    parser_version: u16,
    deployment_generation: u64,
    repair_generation: u64,
    window_start: u64,
    window_end: u64,
    maturity_bucket_exclusive: u64,
    first_bucket: u64,
    record_count: u8,
    sealed_feed_cursor: u64,
    previous: LineageV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeploymentReleaseV1 {
    provider_program: [u8; 32],
    provider_program_owner: [u8; 32],
    deployment_account: [u8; 32],
    deployment_owner: [u8; 32],
    verifier_id: [u8; 32],
    verifier_version: u32,
}

fn verify_open_archive<D: DeploymentAuthenticatorV1>(
    archive: &[u8],
    spec: SourceSpecV1,
    window: WindowDomain,
) -> Result<ArchiveHeaderV1, SourceArchiveError> {
    verify_archive::<D>(archive, spec, window, false)
}

fn verify_archive<D: DeploymentAuthenticatorV1>(
    archive: &[u8],
    spec: SourceSpecV1,
    window: WindowDomain,
    require_sealed: bool,
) -> Result<ArchiveHeaderV1, SourceArchiveError> {
    verify_archive_release(
        archive,
        spec,
        window,
        require_sealed,
        deployment_release::<D>()?,
    )
}

fn verify_archive_release(
    archive: &[u8],
    spec: SourceSpecV1,
    window: WindowDomain,
    require_sealed: bool,
    release: DeploymentReleaseV1,
) -> Result<ArchiveHeaderV1, SourceArchiveError> {
    exact_len(archive, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES)?;
    validate_window(spec, window)?;
    if archive[0] != SOURCE_ARCHIVE_ACCOUNT_TAG {
        return Err(SourceArchiveError::WrongTag);
    }
    if archive[1] != SOURCE_ARCHIVE_ACCOUNT_VERSION {
        return Err(SourceArchiveError::WrongVersion);
    }
    let flags = archive[ARCHIVE_FLAGS_OFFSET];
    if flags > SOURCE_ARCHIVE_FLAG_SEALED {
        return Err(SourceArchiveError::NonCanonicalPadding);
    }
    if require_sealed && flags != SOURCE_ARCHIVE_FLAG_SEALED {
        return Err(SourceArchiveError::NotSealed);
    }
    if !require_sealed && flags == SOURCE_ARCHIVE_FLAG_SEALED {
        return Err(SourceArchiveError::AlreadySealed);
    }
    if archive[ARCHIVE_GRID_PADDING_OFFSET..ARCHIVE_BUCKET_SECONDS_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
        || archive[ARCHIVE_RESERVED_OFFSET..SOURCE_ARCHIVE_HEADER_V1_BYTES]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(SourceArchiveError::NonCanonicalPadding);
    }
    let count = archive[ARCHIVE_COUNT_OFFSET];
    if usize::from(count) > SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        return Err(SourceArchiveError::ArchiveFull);
    }
    let expected_feed = spec.feed_id();
    let expected_window = canonical_window_id(window);
    let feed = Hash32::from_bytes(array_32(archive, ARCHIVE_FEED_OFFSET));
    let stored_window = Hash32::from_bytes(array_32(archive, ARCHIVE_WINDOW_OFFSET));
    if stored_window != expected_window {
        return Err(SourceArchiveError::WindowIdentityMismatch);
    }
    if feed != expected_feed
        || array_32(archive, ARCHIVE_ADAPTER_OFFSET) != spec.source_adapter_id().bytes()
        || u32_at(archive, ARCHIVE_ADAPTER_VERSION_OFFSET) != spec.source_adapter_version()
        || array_32(archive, ARCHIVE_SOURCE_ACCOUNT_OFFSET) != spec.source_account()
        || array_32(archive, ARCHIVE_SOURCE_OWNER_OFFSET) != spec.source_program()
        || u64_at(archive, ARCHIVE_DEPLOYMENT_GENERATION_OFFSET) != spec.deployment_generation()
    {
        return Err(SourceArchiveError::ArchiveBindingMismatch);
    }
    let spec_bytes = spec.encode_canonical();
    if u16_at(archive, ARCHIVE_PARSER_ID_OFFSET) != u16_at(&spec_bytes, 46)
        || u16_at(archive, ARCHIVE_PARSER_VERSION_OFFSET) != u16_at(&spec_bytes, 48)
        || array_32(archive, ARCHIVE_PROVIDER_PROGRAM_OFFSET) != release.provider_program
        || array_32(archive, ARCHIVE_PROVIDER_PROGRAM_OWNER_OFFSET)
            != release.provider_program_owner
        || array_32(archive, ARCHIVE_DEPLOYMENT_ACCOUNT_OFFSET) != release.deployment_account
        || array_32(archive, ARCHIVE_DEPLOYMENT_OWNER_OFFSET) != release.deployment_owner
        || array_32(archive, ARCHIVE_DEPLOYMENT_VERIFIER_OFFSET) != release.verifier_id
        || u32_at(archive, ARCHIVE_DEPLOYMENT_VERIFIER_VERSION_OFFSET) != release.verifier_version
    {
        return Err(SourceArchiveError::AdapterReleaseMismatch);
    }
    let grid = spec.grid();
    if u32_at(archive, ARCHIVE_GRID_FAMILY_OFFSET) != grid.family_id()
        || u16_at(archive, ARCHIVE_GRID_VERSION_OFFSET) != grid.version()
        || u64_at(archive, ARCHIVE_BUCKET_SECONDS_OFFSET) != grid.bucket_seconds()
        || u64_at(archive, ARCHIVE_WINDOW_START_OFFSET) != window.start_bucket()
        || u64_at(archive, ARCHIVE_WINDOW_END_OFFSET) != window.end_bucket_exclusive()
        || u64_at(archive, ARCHIVE_MATURITY_OFFSET) != window.maturity_bucket_exclusive()
        || u64_at(archive, ARCHIVE_REPAIR_GENERATION_OFFSET) != window.generation()
        || u64_at(archive, ARCHIVE_PAGE_INDEX_OFFSET) != 0
        || u64_at(archive, ARCHIVE_FIRST_BUCKET_OFFSET) != window.start_bucket()
    {
        return Err(SourceArchiveError::ArchiveBindingMismatch);
    }
    let window_len = window.end_bucket_exclusive() - window.start_bucket();
    if u64::from(count) > window_len {
        return Err(SourceArchiveError::ArchiveFull);
    }
    let sealed_cursor = u64_at(archive, ARCHIVE_SEALED_FEED_CURSOR_OFFSET);
    if flags == 0 && sealed_cursor != 0 {
        return Err(SourceArchiveError::NonCanonicalPadding);
    }
    if flags == SOURCE_ARCHIVE_FLAG_SEALED
        && (u64::from(count) != window_len || sealed_cursor < window.maturity_bucket_exclusive())
    {
        return Err(SourceArchiveError::NotMature);
    }

    let stored_commitment = Hash32::from_bytes(array_32(archive, ARCHIVE_COMMITMENT_OFFSET));
    if stored_commitment == Hash32::ZERO || stored_commitment != page_commitment(archive) {
        return Err(SourceArchiveError::CommitmentMismatch);
    }
    verify_records(archive, count, window.start_bucket())?;
    Ok(ArchiveHeaderV1 {
        feed,
        window: stored_window,
        page_commitment: stored_commitment,
        grid,
        parser_id: u16_at(archive, ARCHIVE_PARSER_ID_OFFSET),
        parser_version: u16_at(archive, ARCHIVE_PARSER_VERSION_OFFSET),
        deployment_generation: spec.deployment_generation(),
        repair_generation: window.generation(),
        window_start: window.start_bucket(),
        window_end: window.end_bucket_exclusive(),
        maturity_bucket_exclusive: window.maturity_bucket_exclusive(),
        first_bucket: window.start_bucket(),
        record_count: count,
        sealed_feed_cursor: sealed_cursor,
        previous: LineageV1 {
            source_sequence: u64_at(archive, ARCHIVE_PREVIOUS_SEQUENCE_OFFSET),
            publish_slot: u64_at(archive, ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET),
            publish_time: u64_at(archive, ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET),
        },
    })
}

fn deployment_release<D: DeploymentAuthenticatorV1>(
) -> Result<DeploymentReleaseV1, SourceArchiveError> {
    validate_deployment_adapter::<D>()?;
    Ok(DeploymentReleaseV1 {
        provider_program: D::PROVIDER_PROGRAM,
        provider_program_owner: D::PROVIDER_PROGRAM_OWNER,
        deployment_account: D::DEPLOYMENT_ACCOUNT,
        deployment_owner: D::DEPLOYMENT_OWNER,
        verifier_id: D::VERIFIER_ID,
        verifier_version: D::VERIFIER_VERSION,
    })
}

fn recorded_deployment_release(
    archive: &[u8],
    spec: SourceSpecV1,
) -> Result<DeploymentReleaseV1, SourceArchiveError> {
    exact_len(archive, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES)?;
    let release = DeploymentReleaseV1 {
        provider_program: array_32(archive, ARCHIVE_PROVIDER_PROGRAM_OFFSET),
        provider_program_owner: array_32(archive, ARCHIVE_PROVIDER_PROGRAM_OWNER_OFFSET),
        deployment_account: array_32(archive, ARCHIVE_DEPLOYMENT_ACCOUNT_OFFSET),
        deployment_owner: array_32(archive, ARCHIVE_DEPLOYMENT_OWNER_OFFSET),
        verifier_id: array_32(archive, ARCHIVE_DEPLOYMENT_VERIFIER_OFFSET),
        verifier_version: u32_at(archive, ARCHIVE_DEPLOYMENT_VERIFIER_VERSION_OFFSET),
    };
    if release.provider_program != spec.source_program()
        || is_zero(&release.provider_program_owner)
        || is_zero(&release.deployment_account)
        || is_zero(&release.deployment_owner)
        || is_zero(&release.verifier_id)
        || release.verifier_version == 0
    {
        return Err(SourceArchiveError::AdapterReleaseMismatch);
    }
    Ok(release)
}

fn verify_records(archive: &[u8], count: u8, first_bucket: u64) -> Result<(), SourceArchiveError> {
    let mut previous = LineageV1 {
        source_sequence: u64_at(archive, ARCHIVE_PREVIOUS_SEQUENCE_OFFSET),
        publish_slot: u64_at(archive, ARCHIVE_PREVIOUS_PUBLISH_SLOT_OFFSET),
        publish_time: u64_at(archive, ARCHIVE_PREVIOUS_PUBLISH_TIME_OFFSET),
    };
    let mut index = 0_usize;
    while index < SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        let offset = record_offset(index);
        if index >= usize::from(count) {
            if archive[offset..offset + SOURCE_ARCHIVE_RECORD_V1_BYTES]
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(SourceArchiveError::NonCanonicalPadding);
            }
        } else {
            let bucket = u64_at(archive, offset);
            let low = u128_at(archive, offset + 8);
            let high = u128_at(archive, offset + 24);
            let current = LineageV1 {
                source_sequence: u64_at(archive, offset + 40),
                publish_slot: u64_at(archive, offset + 48),
                publish_time: u64_at(archive, offset + 56),
            };
            if bucket != first_bucket + index as u64
                || low > high
                || current.source_sequence <= previous.source_sequence
                || current.publish_slot < previous.publish_slot
                || current.publish_time < previous.publish_time
            {
                return Err(SourceArchiveError::NonContiguousLineage);
            }
            previous = current;
        }
        index += 1;
    }
    Ok(())
}

fn last_lineage(archive: &[u8], header: ArchiveHeaderV1) -> Result<LineageV1, SourceArchiveError> {
    if header.record_count == 0 {
        return Ok(header.previous);
    }
    let offset = record_offset(usize::from(header.record_count - 1));
    let lineage = LineageV1 {
        source_sequence: u64_at(archive, offset + 40),
        publish_slot: u64_at(archive, offset + 48),
        publish_time: u64_at(archive, offset + 56),
    };
    if lineage.source_sequence <= header.previous.source_sequence {
        return Err(SourceArchiveError::NonContiguousLineage);
    }
    Ok(lineage)
}

fn authenticate_deployment<D: DeploymentAuthenticatorV1>(
    spec: SourceSpecV1,
    provider: RuntimeAccountViewV1<'_>,
    deployment: RuntimeAccountViewV1<'_>,
) -> Result<(), SourceArchiveError> {
    validate_deployment_adapter::<D>()?;
    if provider.key != D::PROVIDER_PROGRAM || provider.key != spec.source_program() {
        return Err(SourceArchiveError::ProviderProgramMismatch);
    }
    if provider.owner != D::PROVIDER_PROGRAM_OWNER {
        return Err(SourceArchiveError::ProviderProgramOwnerMismatch);
    }
    if !provider.executable {
        return Err(SourceArchiveError::ProviderProgramNotExecutable);
    }
    if deployment.key != D::DEPLOYMENT_ACCOUNT {
        return Err(SourceArchiveError::DeploymentAccountMismatch);
    }
    if deployment.owner != D::DEPLOYMENT_OWNER {
        return Err(SourceArchiveError::DeploymentOwnerMismatch);
    }
    if deployment.executable {
        return Err(SourceArchiveError::DeploymentAccountExecutable);
    }
    let generation = D::deployment_generation(provider.data(), deployment.data())?;
    if generation != spec.deployment_generation() {
        return Err(SourceArchiveError::DeploymentGenerationMismatch);
    }
    Ok(())
}

fn validate_deployment_adapter<D: DeploymentAuthenticatorV1>() -> Result<(), SourceArchiveError> {
    if is_zero(&D::VERIFIER_ID)
        || is_zero(&D::PROVIDER_PROGRAM)
        || is_zero(&D::PROVIDER_PROGRAM_OWNER)
        || is_zero(&D::DEPLOYMENT_ACCOUNT)
        || is_zero(&D::DEPLOYMENT_OWNER)
        || D::VERIFIER_VERSION == 0
    {
        return Err(SourceArchiveError::InvalidDeploymentAdapter);
    }
    Ok(())
}

fn validate_window(spec: SourceSpecV1, window: WindowDomain) -> Result<(), SourceArchiveError> {
    if window.coverage_policy() != CoveragePolicy::COMPLETE_REQUIRED
        || window.range_len() == 0
        || window.range_len() > SOURCE_ARCHIVE_MAX_RECORDS_V1 as u64
    {
        return Err(SourceArchiveError::InvalidWindow);
    }
    let identity = window.feed();
    if identity.source_adapter_id() != spec.source_adapter_id().bytes()
        || identity.feed_spec_id() != spec.feed_id().bytes()
        || identity.source_version() != spec.source_adapter_version()
        || window.grid() != spec.grid()
    {
        return Err(SourceArchiveError::WindowBindingMismatch);
    }
    Ok(())
}

/// Derive the canonical identity of one immutable window domain.
pub fn canonical_window_id(window: WindowDomain) -> Hash32 {
    let mut bytes = [0_u8; WINDOW_DOMAIN_BYTES];
    window.encode_canonical(&mut bytes);
    Hash32::from_bytes(solana_sha256_hasher::hashv(&[WINDOW_DOMAIN_TAG, &bytes]).to_bytes())
}

fn page_commitment(archive: &[u8]) -> Hash32 {
    Hash32::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_ARCHIVE_COMMITMENT_DOMAIN,
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

fn exact_len(bytes: &[u8], expected: usize) -> Result<(), SourceArchiveError> {
    if bytes.len() != expected {
        return Err(SourceArchiveError::WrongLength);
    }
    Ok(())
}

fn record_offset(index: usize) -> usize {
    SOURCE_ARCHIVE_HEADER_V1_BYTES + index * SOURCE_ARCHIVE_RECORD_V1_BYTES
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
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
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

fn put_u128(bytes: &mut [u8], offset: usize, value: u128) {
    bytes[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
}
