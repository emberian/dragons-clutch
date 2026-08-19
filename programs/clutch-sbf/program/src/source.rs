//! Fail-closed source authentication and price admission for a future feed ABI.
//!
//! This module closes the *semantic* part of the source-admission gap without
//! pretending that a concrete external oracle ABI has been selected.  A
//! [`SourceSpecV1`] commits to one adapter release, parser release, source
//! program, source account, deployment generation, asset orientation, integer
//! scale, observation grid, freshness rule, confidence rule, and canonical
//! bucket-selection rule.  [`admit_price`] then admits exactly one conservative
//! interval from an authenticated runtime account view.
//!
//! The parser is a compile-time implementation of [`PriceParserV1`], not a
//! caller-selected callback or wire object.  A production build must provide a
//! closed parser registry and audit each implementation against a pinned
//! external account layout and deployment.  This repository intentionally has
//! no production parser yet; inventing a Pyth, Switchboard, or DEX layout would
//! turn an unresolved dependency decision into consensus-critical bytes.
//!
//! ## Current runtime boundary
//!
//! `Intent::FeedAdvance` does **not** call this module.  Its current three-
//! account prototype still folds a caller-supplied observation page, and
//! resolution still folds a different caller-supplied evidence buffer.  Those
//! paths are useful SBF/accumulator scaffolding but are not source-authenticated.
//! The exact persisted-account and instruction join required before this
//! module can become load-bearing is specified in
//! `docs/implementation/SOURCE_ADMISSION_V1.md`.

use clutch_accumulator::{Grid, Observation, MAX_VALUE};
use clutch_solana_layout::{canonical_feed_id, Hash32};

/// Exact canonical byte length of [`SourceSpecV1`].
pub const SOURCE_SPEC_V1_BYTES: usize = 256;

/// Canonical source-spec schema version.
pub const SOURCE_SPEC_V1_SCHEMA: u16 = 1;

/// Quote-units-per-one-base-unit orientation.
pub const ORIENTATION_QUOTE_PER_BASE: u8 = 1;

/// The source proves one unique, finalized record for the named bucket.
///
/// A "latest value" account does not satisfy this rule merely because it has
/// a timestamp.  The concrete parser dossier must establish why two callers
/// cannot select different qualifying records for the same bucket.
pub const SELECTION_FINALIZED_BUCKET_RECORD: u16 = 1;

/// Errors from source-spec validation, runtime authentication, and admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceError {
    /// An identity that participates in the feed digest was zero.
    ZeroIdentity,
    /// Base and quote asset identities were equal.
    SameAsset,
    /// A version, generation, or registry identifier was zero.
    Unversioned,
    /// The requested asset orientation is not registered.
    UnknownOrientation,
    /// The requested canonical-record selection rule is not registered.
    UnknownSelectionRule,
    /// The normalized decimal scale is outside the V1 integer envelope.
    InvalidScale,
    /// The source grid is structurally invalid.
    InvalidGrid,
    /// A freshness rule has no finite positive bound.
    InvalidFreshnessPolicy,
    /// The confidence policy is empty, unbounded, or outside its V1 range.
    InvalidConfidencePolicy,
    /// Immutable terms name another canonical feed specification.
    FeedIdentityMismatch,
    /// Immutable terms name another adapter release.
    AdapterMismatch,
    /// The runtime source account key is not the frozen key.
    SourceAccountMismatch,
    /// The runtime account is not owned by the frozen source program.
    SourceOwnerMismatch,
    /// A data-bearing source account was executable.
    SourceExecutable,
    /// The selected compiled parser does not match the frozen parser release.
    ParserMismatch,
    /// The concrete parser refused the pinned source account bytes.
    ParserRefused,
    /// The source deployment generation differs from the frozen generation.
    DeploymentGenerationMismatch,
    /// The feed head belongs to another feed or grid.
    HeadBindingMismatch,
    /// The source-native sequence did not increase strictly.
    SourceSequenceNotMonotone,
    /// The source publish slot moved backwards.
    SourceSlotNotMonotone,
    /// The source publish time moved backwards.
    SourceTimeNotMonotone,
    /// The source claims a publish slot after the trusted cluster clock.
    SourceSlotInFuture,
    /// The source publish slot is older than the frozen slot bound.
    SourceStaleBySlot,
    /// The source time is too far ahead of the trusted cluster clock.
    SourceTimeInFuture,
    /// The source time is older than the frozen time bound.
    SourceStaleByTime,
    /// The parser did not establish a finalized canonical bucket record.
    SourceRecordNotFinal,
    /// The record names another bucket or its source time maps to another one.
    WrongBucket,
    /// The normalized source price is zero or outside the accumulator envelope.
    InvalidPrice,
    /// The confidence interval exceeds the frozen absolute bound.
    ConfidenceTooWideAbsolute,
    /// The confidence interval exceeds the frozen price-relative bound.
    ConfidenceTooWideRelative,
    /// Widening the price by confidence underflowed or overflowed.
    IntervalArithmetic,
    /// Advancing the feed cursor overflowed.
    CursorOverflow,
    /// Resolution evidence came from a caller buffer rather than a canonical
    /// program-owned authenticated archive account.
    UnauthenticatedResolutionEvidence,
    /// A source archive was open, anonymous, empty, or otherwise malformed.
    MalformedArchive,
    /// A source archive belongs to another feed, generation, or window.
    ArchiveBindingMismatch,
}

/// Public construction fields for an immutable V1 source specification.
///
/// [`SourceSpecV1::new`] validates these fields before they can participate in
/// admission.  The field struct is not itself evidence of a valid spec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecFieldsV1 {
    /// Digest identifying the reviewed adapter implementation.
    pub source_adapter_id: Hash32,
    /// Version of that adapter; immutable terms bind this as `source_version`.
    pub source_adapter_version: u32,
    /// Closed parser-registry identifier.
    pub parser_id: u16,
    /// Version of the selected parser.
    pub parser_version: u16,
    /// Program that must own the source data account.
    pub source_program: [u8; 32],
    /// Exact source data account key.
    pub source_account: [u8; 32],
    /// Pinned source-program deployment generation.
    pub deployment_generation: u64,
    /// Canonical base-asset identity.
    pub base_asset_id: Hash32,
    /// Canonical quote-asset identity.
    pub quote_asset_id: Hash32,
    /// Registered orientation; V1 accepts [`ORIENTATION_QUOTE_PER_BASE`].
    pub orientation: u8,
    /// Decimal places in the normalized positive integer price.
    pub normalized_decimals: u8,
    /// Observation grid family.
    pub grid_family_id: u32,
    /// Observation grid version.
    pub grid_version: u16,
    /// Exact duration of one observation bucket in seconds.
    pub bucket_seconds: u64,
    /// Maximum age of a source record in cluster slots.
    pub max_staleness_slots: u64,
    /// Maximum age of a source record in seconds.
    pub max_staleness_seconds: u64,
    /// Allowed positive source-time skew in seconds.
    pub max_future_seconds: u64,
    /// Maximum admitted half-width after confidence widening.
    pub max_confidence_atoms: u128,
    /// Maximum admitted half-width relative to price, in basis points.
    pub max_confidence_bps: u32,
    /// Integer multiplier applied to the parser's confidence value.
    pub confidence_multiplier: u16,
    /// Canonical record-selection registry id.
    pub selection_rule: u16,
}

/// Validated immutable source specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecV1 {
    fields: SourceSpecFieldsV1,
    grid: Grid,
}

impl SourceSpecV1 {
    /// Validate and construct a source specification.
    pub fn new(fields: SourceSpecFieldsV1) -> Result<Self, SourceError> {
        if fields.source_adapter_id == Hash32::ZERO
            || is_zero(&fields.source_program)
            || is_zero(&fields.source_account)
            || fields.base_asset_id == Hash32::ZERO
            || fields.quote_asset_id == Hash32::ZERO
        {
            return Err(SourceError::ZeroIdentity);
        }
        if fields.base_asset_id == fields.quote_asset_id {
            return Err(SourceError::SameAsset);
        }
        if fields.source_adapter_version == 0
            || fields.parser_id == 0
            || fields.parser_version == 0
            || fields.deployment_generation == 0
        {
            return Err(SourceError::Unversioned);
        }
        if fields.orientation != ORIENTATION_QUOTE_PER_BASE {
            return Err(SourceError::UnknownOrientation);
        }
        if fields.selection_rule != SELECTION_FINALIZED_BUCKET_RECORD {
            return Err(SourceError::UnknownSelectionRule);
        }
        if fields.normalized_decimals > 18 {
            return Err(SourceError::InvalidScale);
        }
        let grid = Grid::new(
            fields.grid_family_id,
            fields.grid_version,
            fields.bucket_seconds,
        )
        .map_err(|_| SourceError::InvalidGrid)?;
        if fields.max_staleness_slots == 0 || fields.max_staleness_seconds == 0 {
            return Err(SourceError::InvalidFreshnessPolicy);
        }
        if fields.max_confidence_atoms == 0
            || fields.max_confidence_atoms > MAX_VALUE
            || fields.max_confidence_bps == 0
            || fields.max_confidence_bps > 10_000
            || fields.confidence_multiplier == 0
            || fields.confidence_multiplier > 32
        {
            return Err(SourceError::InvalidConfidencePolicy);
        }
        Ok(Self { fields, grid })
    }

    /// Immutable source-adapter identity.
    pub const fn source_adapter_id(self) -> Hash32 {
        self.fields.source_adapter_id
    }

    /// Immutable source-adapter version.
    pub const fn source_adapter_version(self) -> u32 {
        self.fields.source_adapter_version
    }

    /// Closed parser-registry identifier.
    pub const fn parser_id(self) -> u16 {
        self.fields.parser_id
    }

    /// Reviewed parser release version.
    pub const fn parser_version(self) -> u16 {
        self.fields.parser_version
    }

    /// Frozen observation grid.
    pub const fn grid(self) -> Grid {
        self.grid
    }

    /// Frozen source data account key.
    pub const fn source_account(self) -> [u8; 32] {
        self.fields.source_account
    }

    /// Frozen source owner-program key.
    pub const fn source_program(self) -> [u8; 32] {
        self.fields.source_program
    }

    /// Pinned source deployment generation.
    pub const fn deployment_generation(self) -> u64 {
        self.fields.deployment_generation
    }

    /// Derive the feed identity committed to by immutable terms and feed state.
    pub fn feed_id(self) -> Hash32 {
        canonical_feed_id(&self.encode_canonical())
    }

    /// Encode the exact 256-byte feed-spec preimage.
    pub fn encode_canonical(self) -> [u8; SOURCE_SPEC_V1_BYTES] {
        let mut out = [0_u8; SOURCE_SPEC_V1_BYTES];
        let mut at = 0_usize;
        put(&mut out, &mut at, b"DCSRCV1\0");
        put(&mut out, &mut at, &SOURCE_SPEC_V1_SCHEMA.to_le_bytes());
        put(&mut out, &mut at, &self.fields.source_adapter_id.bytes());
        put(
            &mut out,
            &mut at,
            &self.fields.source_adapter_version.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.parser_id.to_le_bytes());
        put(&mut out, &mut at, &self.fields.parser_version.to_le_bytes());
        put(&mut out, &mut at, &self.fields.source_program);
        put(&mut out, &mut at, &self.fields.source_account);
        put(
            &mut out,
            &mut at,
            &self.fields.deployment_generation.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.base_asset_id.bytes());
        put(&mut out, &mut at, &self.fields.quote_asset_id.bytes());
        put(&mut out, &mut at, &[self.fields.orientation]);
        put(&mut out, &mut at, &[self.fields.normalized_decimals]);
        put(&mut out, &mut at, &self.fields.grid_family_id.to_le_bytes());
        put(&mut out, &mut at, &self.fields.grid_version.to_le_bytes());
        put(&mut out, &mut at, &self.fields.bucket_seconds.to_le_bytes());
        put(
            &mut out,
            &mut at,
            &self.fields.max_staleness_slots.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_staleness_seconds.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_future_seconds.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_confidence_atoms.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_confidence_bps.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.confidence_multiplier.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.selection_rule.to_le_bytes());
        debug_assert_eq!(at, SOURCE_SPEC_V1_BYTES - 6);
        out
    }
}

/// Authenticate the immutable source-account metadata named by a specification.
///
/// This is the construction-time subset of [`admit_price`]. It deliberately
/// does not parse or admit a value: source bytes have value authority only in
/// an append transaction carrying an authenticated clock and expected bucket.
pub fn authenticate_source_account(
    spec: SourceSpecV1,
    account: SourceAccountView<'_>,
) -> Result<(), SourceError> {
    if account.key != spec.source_account() {
        return Err(SourceError::SourceAccountMismatch);
    }
    if account.owner != spec.source_program() {
        return Err(SourceError::SourceOwnerMismatch);
    }
    if account.executable {
        return Err(SourceError::SourceExecutable);
    }
    Ok(())
}

/// Immutable source facts already bound by a market's terms artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermsSourceBindingV1 {
    /// Canonical feed identity.
    pub feed: Hash32,
    /// Frozen source-adapter identity.
    pub source_adapter_id: Hash32,
    /// Frozen source-adapter version.
    pub source_version: u32,
}

/// Authenticate a source specification against immutable market terms.
pub fn check_terms_binding(
    spec: SourceSpecV1,
    terms: TermsSourceBindingV1,
) -> Result<(), SourceError> {
    if terms.feed != spec.feed_id() {
        return Err(SourceError::FeedIdentityMismatch);
    }
    if terms.source_adapter_id != spec.source_adapter_id()
        || terms.source_version != spec.source_adapter_version()
    {
        return Err(SourceError::AdapterMismatch);
    }
    Ok(())
}

/// Runtime metadata and bytes of the source data account.
///
/// The eventual SBF seam must construct this from `AccountInfo`; these fields
/// must never be decoded from instruction data or a caller-owned wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountView<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> SourceAccountView<'a> {
    /// Construct a runtime view at the adapter boundary.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }

    /// Authenticated source account bytes presented to the selected parser.
    pub const fn data(self) -> &'a [u8] {
        self.data
    }
}

/// Trusted cluster clock facts used for freshness checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustedClockV1 {
    /// Current cluster slot.
    pub slot: u64,
    /// Current non-negative Unix time in seconds.
    pub unix_seconds: u64,
}

/// Persisted source progress required to admit the next bucket.
///
/// The existing 124-byte `FeedAccount` does not carry all of these facts.  This
/// type is therefore a required future layout, not a projection callers may
/// fill from instruction bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceHeadV1 {
    /// Feed identity derived from the immutable source spec.
    pub feed: Hash32,
    /// Grid frozen into the source spec.
    pub grid: Grid,
    /// Next bucket that may be admitted.
    pub cursor: u64,
    /// Pinned source deployment generation.
    pub deployment_generation: u64,
    /// Last accepted source-native sequence.
    pub last_source_sequence: u64,
    /// Last accepted source publish slot.
    pub last_publish_slot: u64,
    /// Last accepted source publish time.
    pub last_publish_time: u64,
}

/// Normalized result of one audited source-specific parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParsedPriceV1 {
    /// Source program deployment generation observed by the parser seam.
    pub deployment_generation: u64,
    /// Strictly increasing source-native sequence.
    pub source_sequence: u64,
    /// Source publish slot.
    pub publish_slot: u64,
    /// Source publish time as a non-negative Unix timestamp.
    pub publish_time: u64,
    /// Unique bucket named by the source's canonical-record rule.
    pub canonical_bucket: u64,
    /// Whether the source makes the bucket record terminal and unique.
    pub finalized_bucket: bool,
    /// Positive normalized price in the spec's decimal scale.
    pub price_atoms: u128,
    /// Non-negative normalized source confidence half-width.
    pub confidence_atoms: u128,
}

/// Compile-time parser selected by the program's closed registry.
///
/// Implementations are consensus-critical adapter code.  They must parse the
/// exact pinned external layout, establish the deployment generation and the
/// uniqueness/finality of `canonical_bucket`, normalize signed/exponent values
/// with checked integer arithmetic, and return [`SourceError::ParserRefused`]
/// for every unsupported state.
pub trait PriceParserV1 {
    /// Adapter identity of this compiled parser release.
    const SOURCE_ADAPTER_ID: [u8; 32];
    /// Adapter version of this compiled parser release.
    const SOURCE_ADAPTER_VERSION: u32;
    /// Closed parser registry id.
    const PARSER_ID: u16;
    /// Parser implementation version.
    const PARSER_VERSION: u16;

    /// Parse already metadata-authenticated source-account bytes.
    fn parse(account: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError>;
}

/// One source-authenticated interval and the progress facts to persist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedObservationV1 {
    /// Conservative accepted interval for exactly one bucket.
    pub observation: Observation,
    /// Next feed cursor after accepting the record.
    pub next_cursor: u64,
    /// Source-native sequence to persist.
    pub source_sequence: u64,
    /// Publish slot to persist.
    pub publish_slot: u64,
    /// Publish time to persist.
    pub publish_time: u64,
}

/// Exact authenticated archive facts a resolving market must bind.
///
/// A future account decoder constructs this only after checking program
/// ownership, the canonical archive PDA, the frozen codec, and the archive's
/// sealed state.  It is deliberately distinct from the untrusted bytes that
/// today's prototype resolution instruction accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedArchiveV1 {
    /// Canonical feed identity.
    pub feed: Hash32,
    /// Pinned source deployment generation used for every contained record.
    pub deployment_generation: u64,
    /// First archived bucket, inclusive.
    pub start_bucket: u64,
    /// Last archived bucket, exclusive.
    pub end_bucket_exclusive: u64,
    /// Digest recomputed from the canonical archived record bytes.
    pub summary_digest: Hash32,
    /// Whether the archive page is terminal and immutable.
    pub sealed: bool,
}

/// Source lineage immutable market terms require at resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionArchiveExpectationV1 {
    /// Canonical feed identity.
    pub feed: Hash32,
    /// Pinned source deployment generation.
    pub deployment_generation: u64,
    /// Exact first required bucket, inclusive.
    pub start_bucket: u64,
    /// Exact last required bucket, exclusive.
    pub end_bucket_exclusive: u64,
}

/// Provenance class of evidence presented to resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionEvidenceV1 {
    /// A canonical, program-owned archive account decoded by the adapter.
    AuthenticatedArchive(AuthenticatedArchiveV1),
    /// The current prototype's independent caller-supplied evidence buffer.
    ///
    /// Its domain may equal immutable terms and its claimed cursor may equal
    /// the feed head, but neither fact links its observation records to an
    /// authenticated source transition.
    CallerSuppliedBuffer,
}

/// Require resolution evidence to be the exact authenticated source archive.
///
/// This is the named refusal for the current substitution attack: advancing a
/// feed with one set of observations and resolving with an unrelated caller
/// buffer that declares the same domain and cursor.
pub fn check_resolution_archive(
    expected: ResolutionArchiveExpectationV1,
    evidence: ResolutionEvidenceV1,
) -> Result<AuthenticatedArchiveV1, SourceError> {
    let archive = match evidence {
        ResolutionEvidenceV1::AuthenticatedArchive(archive) => archive,
        ResolutionEvidenceV1::CallerSuppliedBuffer => {
            return Err(SourceError::UnauthenticatedResolutionEvidence);
        }
    };
    if !archive.sealed
        || archive.feed == Hash32::ZERO
        || archive.summary_digest == Hash32::ZERO
        || archive.start_bucket >= archive.end_bucket_exclusive
    {
        return Err(SourceError::MalformedArchive);
    }
    if archive.feed != expected.feed
        || archive.deployment_generation != expected.deployment_generation
        || archive.start_bucket != expected.start_bucket
        || archive.end_bucket_exclusive != expected.end_bucket_exclusive
    {
        return Err(SourceError::ArchiveBindingMismatch);
    }
    Ok(archive)
}

/// Authenticate and admit one canonical finalized source record.
///
/// This function never manufactures a `Missing` observation.  Absence cannot
/// be proven from a caller's failure to present data; a future missing/repair
/// policy needs source-native non-existence evidence or must fail closed.
pub fn admit_price<P: PriceParserV1>(
    spec: SourceSpecV1,
    head: SourceHeadV1,
    clock: TrustedClockV1,
    account: SourceAccountView<'_>,
) -> Result<AuthenticatedObservationV1, SourceError> {
    if account.key != spec.fields.source_account {
        return Err(SourceError::SourceAccountMismatch);
    }
    if account.owner != spec.fields.source_program {
        return Err(SourceError::SourceOwnerMismatch);
    }
    if account.executable {
        return Err(SourceError::SourceExecutable);
    }
    if P::SOURCE_ADAPTER_ID != spec.fields.source_adapter_id.bytes()
        || P::SOURCE_ADAPTER_VERSION != spec.fields.source_adapter_version
        || P::PARSER_ID != spec.fields.parser_id
        || P::PARSER_VERSION != spec.fields.parser_version
    {
        return Err(SourceError::ParserMismatch);
    }
    if head.feed != spec.feed_id()
        || head.grid != spec.grid
        || head.deployment_generation != spec.fields.deployment_generation
    {
        return Err(SourceError::HeadBindingMismatch);
    }

    let parsed = P::parse(account)?;
    if parsed.deployment_generation != spec.fields.deployment_generation {
        return Err(SourceError::DeploymentGenerationMismatch);
    }
    if parsed.source_sequence <= head.last_source_sequence {
        return Err(SourceError::SourceSequenceNotMonotone);
    }
    if parsed.publish_slot < head.last_publish_slot {
        return Err(SourceError::SourceSlotNotMonotone);
    }
    if parsed.publish_time < head.last_publish_time {
        return Err(SourceError::SourceTimeNotMonotone);
    }
    if parsed.publish_slot > clock.slot {
        return Err(SourceError::SourceSlotInFuture);
    }
    if clock.slot - parsed.publish_slot > spec.fields.max_staleness_slots {
        return Err(SourceError::SourceStaleBySlot);
    }
    let latest_allowed_time = clock
        .unix_seconds
        .checked_add(spec.fields.max_future_seconds)
        .ok_or(SourceError::SourceTimeInFuture)?;
    if parsed.publish_time > latest_allowed_time {
        return Err(SourceError::SourceTimeInFuture);
    }
    if parsed.publish_time <= clock.unix_seconds
        && clock.unix_seconds - parsed.publish_time > spec.fields.max_staleness_seconds
    {
        return Err(SourceError::SourceStaleByTime);
    }
    if !parsed.finalized_bucket {
        return Err(SourceError::SourceRecordNotFinal);
    }
    let time_bucket = parsed.publish_time / spec.fields.bucket_seconds;
    if parsed.canonical_bucket != head.cursor || time_bucket != head.cursor {
        return Err(SourceError::WrongBucket);
    }
    if parsed.price_atoms == 0 || parsed.price_atoms > MAX_VALUE {
        return Err(SourceError::InvalidPrice);
    }

    let radius = parsed
        .confidence_atoms
        .checked_mul(u128::from(spec.fields.confidence_multiplier))
        .ok_or(SourceError::IntervalArithmetic)?;
    if radius > spec.fields.max_confidence_atoms {
        return Err(SourceError::ConfidenceTooWideAbsolute);
    }
    let scaled_radius = radius
        .checked_mul(10_000)
        .ok_or(SourceError::IntervalArithmetic)?;
    let relative_limit = parsed
        .price_atoms
        .checked_mul(u128::from(spec.fields.max_confidence_bps))
        .ok_or(SourceError::IntervalArithmetic)?;
    if scaled_radius > relative_limit {
        return Err(SourceError::ConfidenceTooWideRelative);
    }
    let low = parsed
        .price_atoms
        .checked_sub(radius)
        .ok_or(SourceError::IntervalArithmetic)?;
    let high = parsed
        .price_atoms
        .checked_add(radius)
        .filter(|value| *value <= MAX_VALUE)
        .ok_or(SourceError::IntervalArithmetic)?;
    let next_cursor = head
        .cursor
        .checked_add(1)
        .ok_or(SourceError::CursorOverflow)?;

    Ok(AuthenticatedObservationV1 {
        observation: Observation::accepted(head.cursor, low, high),
        next_cursor,
        source_sequence: parsed.source_sequence,
        publish_slot: parsed.publish_slot,
        publish_time: parsed.publish_time,
    })
}

fn is_zero(bytes: &[u8; 32]) -> bool {
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn put<const N: usize>(out: &mut [u8; N], at: &mut usize, bytes: &[u8]) {
    let end = *at + bytes.len();
    out[*at..end].copy_from_slice(bytes);
    *at = end;
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADAPTER: [u8; 32] = [0xa1; 32];
    const PROGRAM: [u8; 32] = [0xb2; 32];
    const ACCOUNT: [u8; 32] = [0xc3; 32];
    const RECORD_BYTES: usize = 77;

    struct FixtureParser;

    impl PriceParserV1 for FixtureParser {
        const SOURCE_ADAPTER_ID: [u8; 32] = ADAPTER;
        const SOURCE_ADAPTER_VERSION: u32 = 7;
        const PARSER_ID: u16 = 11;
        const PARSER_VERSION: u16 = 3;

        fn parse(account: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError> {
            let bytes = account.data();
            if bytes.len() != RECORD_BYTES || &bytes[..4] != b"SRC1" {
                return Err(SourceError::ParserRefused);
            }
            Ok(ParsedPriceV1 {
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

    struct WrongParser;

    impl PriceParserV1 for WrongParser {
        const SOURCE_ADAPTER_ID: [u8; 32] = [0xdd; 32];
        const SOURCE_ADAPTER_VERSION: u32 = 7;
        const PARSER_ID: u16 = 11;
        const PARSER_VERSION: u16 = 3;

        fn parse(_: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError> {
            panic!("a mismatched parser must be refused before parsing")
        }
    }

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn fields() -> SourceSpecFieldsV1 {
        SourceSpecFieldsV1 {
            source_adapter_id: Hash32::from_bytes(ADAPTER),
            source_adapter_version: 7,
            parser_id: 11,
            parser_version: 3,
            source_program: PROGRAM,
            source_account: ACCOUNT,
            deployment_generation: 19,
            base_asset_id: h(1),
            quote_asset_id: h(2),
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
        }
    }

    fn spec() -> SourceSpecV1 {
        SourceSpecV1::new(fields()).expect("valid source spec")
    }

    fn record(
        generation: u64,
        sequence: u64,
        slot: u64,
        time: u64,
        bucket: u64,
        price_and_confidence: (u128, u128),
        finalized: bool,
    ) -> [u8; RECORD_BYTES] {
        let mut out = [0_u8; RECORD_BYTES];
        out[..4].copy_from_slice(b"SRC1");
        out[4..12].copy_from_slice(&generation.to_le_bytes());
        out[12..20].copy_from_slice(&sequence.to_le_bytes());
        out[20..28].copy_from_slice(&slot.to_le_bytes());
        out[28..36].copy_from_slice(&time.to_le_bytes());
        out[36..44].copy_from_slice(&bucket.to_le_bytes());
        out[44..60].copy_from_slice(&price_and_confidence.0.to_le_bytes());
        out[60..76].copy_from_slice(&price_and_confidence.1.to_le_bytes());
        out[76] = u8::from(finalized);
        out
    }

    fn account(data: &[u8]) -> SourceAccountView<'_> {
        SourceAccountView::new(ACCOUNT, PROGRAM, false, data)
    }

    fn head(spec: SourceSpecV1) -> SourceHeadV1 {
        SourceHeadV1 {
            feed: spec.feed_id(),
            grid: spec.grid(),
            cursor: 100,
            deployment_generation: 19,
            last_source_sequence: 40,
            last_publish_slot: 990,
            last_publish_time: 5_999,
        }
    }

    fn clock() -> TrustedClockV1 {
        TrustedClockV1 {
            slot: 1_000,
            unix_seconds: 6_010,
        }
    }

    fn good_record() -> [u8; RECORD_BYTES] {
        record(19, 41, 995, 6_000, 100, (1_000_000, 2_000), true)
    }

    fn u64_at(bytes: &[u8], at: usize) -> u64 {
        let mut out = [0_u8; 8];
        out.copy_from_slice(&bytes[at..at + 8]);
        u64::from_le_bytes(out)
    }

    fn u128_at(bytes: &[u8], at: usize) -> u128 {
        let mut out = [0_u8; 16];
        out.copy_from_slice(&bytes[at..at + 16]);
        u128::from_le_bytes(out)
    }

    #[test]
    fn canonical_spec_binds_every_admission_policy_field() {
        let base = spec();
        let bytes = base.encode_canonical();
        assert_eq!(bytes.len(), SOURCE_SPEC_V1_BYTES);
        assert_eq!(&bytes[..8], b"DCSRCV1\0");
        assert_eq!(&bytes[250..], &[0; 6]);

        let original = base.feed_id();
        let mut cases = [fields(); 8];
        cases[0].source_account[0] ^= 1;
        cases[1].deployment_generation += 1;
        cases[2].quote_asset_id = h(3);
        cases[3].bucket_seconds += 1;
        cases[4].max_staleness_slots += 1;
        cases[5].max_confidence_atoms += 1;
        cases[6].max_confidence_bps += 1;
        cases[7].confidence_multiplier += 1;
        for changed in cases {
            assert_ne!(
                SourceSpecV1::new(changed).expect("still valid").feed_id(),
                original
            );
        }
    }

    #[test]
    fn invalid_specs_fail_before_they_can_name_a_feed() {
        let mut case = fields();
        case.source_account = [0; 32];
        assert_eq!(SourceSpecV1::new(case), Err(SourceError::ZeroIdentity));

        let mut case = fields();
        case.quote_asset_id = case.base_asset_id;
        assert_eq!(SourceSpecV1::new(case), Err(SourceError::SameAsset));

        let mut case = fields();
        case.source_adapter_version = 0;
        assert_eq!(SourceSpecV1::new(case), Err(SourceError::Unversioned));

        let mut case = fields();
        case.selection_rule = 9;
        assert_eq!(
            SourceSpecV1::new(case),
            Err(SourceError::UnknownSelectionRule)
        );

        let mut case = fields();
        case.max_staleness_slots = 0;
        assert_eq!(
            SourceSpecV1::new(case),
            Err(SourceError::InvalidFreshnessPolicy)
        );

        let mut case = fields();
        case.max_confidence_bps = 10_001;
        assert_eq!(
            SourceSpecV1::new(case),
            Err(SourceError::InvalidConfidencePolicy)
        );
    }

    #[test]
    fn terms_bind_the_canonical_feed_and_adapter_release() {
        let spec = spec();
        let good = TermsSourceBindingV1 {
            feed: spec.feed_id(),
            source_adapter_id: spec.source_adapter_id(),
            source_version: spec.source_adapter_version(),
        };
        assert_eq!(check_terms_binding(spec, good), Ok(()));

        let mut wrong = good;
        wrong.feed = h(99);
        assert_eq!(
            check_terms_binding(spec, wrong),
            Err(SourceError::FeedIdentityMismatch)
        );
        let mut wrong = good;
        wrong.source_version += 1;
        assert_eq!(
            check_terms_binding(spec, wrong),
            Err(SourceError::AdapterMismatch)
        );
    }

    #[test]
    fn one_finalized_fresh_record_becomes_one_conservative_interval() {
        let spec = spec();
        let bytes = good_record();
        let admitted = admit_price::<FixtureParser>(spec, head(spec), clock(), account(&bytes))
            .expect("authenticated record");
        assert_eq!(
            admitted.observation,
            Observation::accepted(100, 996_000, 1_004_000)
        );
        assert_eq!(admitted.next_cursor, 101);
        assert_eq!(admitted.source_sequence, 41);
        assert_eq!(admitted.publish_slot, 995);
        assert_eq!(admitted.publish_time, 6_000);
    }

    #[test]
    fn runtime_metadata_and_parser_release_are_not_caller_substitutable() {
        let spec = spec();
        let bytes = good_record();
        assert_eq!(
            admit_price::<FixtureParser>(
                spec,
                head(spec),
                clock(),
                SourceAccountView::new([1; 32], PROGRAM, false, &bytes),
            ),
            Err(SourceError::SourceAccountMismatch)
        );
        assert_eq!(
            admit_price::<FixtureParser>(
                spec,
                head(spec),
                clock(),
                SourceAccountView::new(ACCOUNT, [2; 32], false, &bytes),
            ),
            Err(SourceError::SourceOwnerMismatch)
        );
        assert_eq!(
            admit_price::<FixtureParser>(
                spec,
                head(spec),
                clock(),
                SourceAccountView::new(ACCOUNT, PROGRAM, true, &bytes),
            ),
            Err(SourceError::SourceExecutable)
        );
        assert_eq!(
            admit_price::<WrongParser>(spec, head(spec), clock(), account(&bytes)),
            Err(SourceError::ParserMismatch)
        );
        assert_eq!(
            admit_price::<FixtureParser>(spec, head(spec), clock(), account(b"not-a-record")),
            Err(SourceError::ParserRefused)
        );
    }

    #[test]
    fn generation_sequence_and_clocks_fail_closed() {
        let spec = spec();
        let base_head = head(spec);

        let bytes = record(20, 41, 995, 6_000, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::DeploymentGenerationMismatch)
        );
        let bytes = record(19, 40, 995, 6_000, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceSequenceNotMonotone)
        );
        let bytes = record(19, 41, 989, 6_000, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceSlotNotMonotone)
        );
        let bytes = record(19, 41, 1_001, 6_000, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceSlotInFuture)
        );
        let bytes = record(19, 41, 990, 6_000, 100, (1_000_000, 2_000), true);
        let stale_slot_clock = TrustedClockV1 {
            slot: 1_011,
            unix_seconds: 6_010,
        };
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, stale_slot_clock, account(&bytes)),
            Err(SourceError::SourceStaleBySlot)
        );
        let bytes = record(19, 41, 995, 5_998, 99, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceTimeNotMonotone)
        );
        let bytes = record(19, 41, 995, 6_013, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceTimeInFuture)
        );
        let bytes = record(19, 41, 995, 5_999, 99, (1_000_000, 2_000), true);
        let stale_time_clock = TrustedClockV1 {
            slot: 1_000,
            unix_seconds: 6_090,
        };
        /* Staleness is decided before bucket mapping: a record outside the
         * time policy is not allowed to become a semantically different
         * `WrongBucket` diagnostic. */
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, stale_time_clock, account(&bytes)),
            Err(SourceError::SourceStaleByTime)
        );
    }

    #[test]
    fn bucket_finality_and_confidence_are_consensus_checks() {
        let spec = spec();
        let base_head = head(spec);

        let bytes = record(19, 41, 995, 6_000, 100, (1_000_000, 2_000), false);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::SourceRecordNotFinal)
        );
        let bytes = record(19, 41, 995, 6_000, 101, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::WrongBucket)
        );
        let bytes = record(19, 41, 995, 5_999, 100, (1_000_000, 2_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::WrongBucket)
        );
        let bytes = record(19, 41, 995, 6_000, 100, (1_000_000, 3_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::ConfidenceTooWideAbsolute)
        );
        let bytes = record(19, 41, 995, 6_000, 100, (100_000, 1_000), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::ConfidenceTooWideRelative)
        );
        let bytes = record(19, 41, 995, 6_000, 100, (1, 2), true);
        /* Relative confidence is checked before interval subtraction.  Since
         * V1 caps the relative width at 100%, every would-underflow interval
         * is already a more precise relative-policy refusal. */
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::ConfidenceTooWideRelative)
        );
        let bytes = record(19, 41, 995, 6_000, 100, (MAX_VALUE, 1), true);
        assert_eq!(
            admit_price::<FixtureParser>(spec, base_head, clock(), account(&bytes)),
            Err(SourceError::IntervalArithmetic)
        );
    }

    #[test]
    fn an_unrelated_caller_buffer_cannot_substitute_for_the_feed_archive() {
        let spec = spec();
        let expected = ResolutionArchiveExpectationV1 {
            feed: spec.feed_id(),
            deployment_generation: spec.deployment_generation(),
            start_bucket: 100,
            end_bucket_exclusive: 103,
        };

        /* This is the exact shape the live prototype still accepts: a caller
         * can declare the expected domain and cursor while supplying unrelated
         * observation records.  The authenticated path has no conversion from
         * those bytes into an archive and refuses the provenance class. */
        assert_eq!(
            check_resolution_archive(expected, ResolutionEvidenceV1::CallerSuppliedBuffer),
            Err(SourceError::UnauthenticatedResolutionEvidence)
        );

        let archive = AuthenticatedArchiveV1 {
            feed: expected.feed,
            deployment_generation: expected.deployment_generation,
            start_bucket: expected.start_bucket,
            end_bucket_exclusive: expected.end_bucket_exclusive,
            summary_digest: h(0x55),
            sealed: true,
        };
        assert_eq!(
            check_resolution_archive(
                expected,
                ResolutionEvidenceV1::AuthenticatedArchive(archive)
            ),
            Ok(archive)
        );

        let mut substituted = archive;
        substituted.summary_digest = h(0x56);
        /* A different nonzero digest is not rejected by this binding alone;
         * the future archive codec must recompute it from authenticated record
         * bytes.  Changing the window lineage, however, is a direct refusal. */
        substituted.start_bucket = 99;
        assert_eq!(
            check_resolution_archive(
                expected,
                ResolutionEvidenceV1::AuthenticatedArchive(substituted)
            ),
            Err(SourceError::ArchiveBindingMismatch)
        );
    }
}
