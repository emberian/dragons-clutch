use clutch_source_plane_v3::{ContentId, RawRecordV3, MAX_SOURCE_VALUE};
use clutch_source_profile_v1::spec_v2::{
    SourceSpecV2, SpecV2Error, FEED_DOMAIN_V2, SOURCE_SPEC_V2_BYTES,
};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// Expected V2 SourceSpec account bytes in the model-derived fixture.
pub const SOURCE_SPEC_ACCOUNT_V2_BYTES: usize = 404;
/// Expected V2 archive-record bytes in the model-derived fixture.
pub const ARCHIVE_RECORD_V2_BYTES: usize = 64;

const SOURCE_SPEC_ACCOUNT_V2_TAG: u8 = 0x73;
const SOURCE_SPEC_ACCOUNT_V2_VERSION: u8 = 1;
const SPEC_FEED_OFFSET: usize = 2;
const SPEC_BODY_OFFSET: usize = 34;
const SPEC_BUMP_OFFSET: usize = 402;
const SPEC_FLAGS_OFFSET: usize = 403;

/// Caller-asserted metadata-bearing fixture view of a V2 SourceSpec account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V2AccountView<'a> {
    /// Presented account address.
    pub key: [u8; 32],
    /// Presented account owner.
    pub owner: [u8; 32],
    /// Runtime executable bit.
    pub executable: bool,
    /// Complete account data.
    pub data: &'a [u8],
}

/// Exact refusals for the model-derived V2 host fixture projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V2SourceSpecRefusal {
    /// Program, expected address, or stored feed was zero.
    ZeroIdentity,
    /// Presented key did not match the caller-supplied expected key.
    WrongKey,
    /// Presented owner was not the exact Clutch program.
    WrongOwner,
    /// A state account was executable.
    Executable,
    /// Account data was not exactly 404 bytes.
    WrongLength,
    /// Account-family tag was not `0x73`.
    WrongTag,
    /// Account version was not exactly one.
    WrongVersion,
    /// Flags or reserved bytes were noncanonical.
    NonCanonicalPadding,
    /// Stored PDA bump disagreed with the caller-supplied expected bump.
    WrongBump,
    /// Canonical V2 body refused.
    Body(SpecV2Error),
    /// Stored feed disagreed with `SHA256(domain || body)`.
    DigestMismatch,
}

/// Exact wire projection of one host-supplied V2 SourceSpec fixture.
///
/// This is not runtime authentication. The live adapter must derive and verify
/// owner, key, and bump with the current runtime verifier before promoting the
/// result into [`V2AuthenticatedSourceRoute`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V2SourceSpecBinding {
    account_key: [u8; 32],
    owner: [u8; 32],
    feed_id: ContentId,
    stored_bump: u8,
    spec: SourceSpecV2,
}

impl V2SourceSpecBinding {
    /// Host fixture account address.
    pub const fn account_key(self) -> [u8; 32] {
        self.account_key
    }

    /// Host fixture owner bytes.
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Existing canonical V2 feed identity used as V3 `source_spec_id`.
    pub const fn feed_id(self) -> ContentId {
        self.feed_id
    }

    /// Stored bump checked against the fixture expectation.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }

    /// Full model-derived V2 body. No authentication is implied.
    pub const fn spec(self) -> SourceSpecV2 {
        self.spec
    }
}

/// Project the expected V2 account envelope and full model-derived body.
///
/// The owner, key, bump, and account bytes are all caller-supplied fixture
/// inputs. The body codec is explicitly MODEL-ONLY and this function is not a
/// runtime-produced differential fixture or an authority capability.
pub fn project_v2_source_spec_fixture(
    clutch_program: [u8; 32],
    expected_key: [u8; 32],
    expected_bump: u8,
    account: V2AccountView<'_>,
) -> core::result::Result<V2SourceSpecBinding, V2SourceSpecRefusal> {
    if is_zero(&clutch_program) || is_zero(&expected_key) {
        return Err(V2SourceSpecRefusal::ZeroIdentity);
    }
    if account.key != expected_key {
        return Err(V2SourceSpecRefusal::WrongKey);
    }
    if account.owner != clutch_program {
        return Err(V2SourceSpecRefusal::WrongOwner);
    }
    if account.executable {
        return Err(V2SourceSpecRefusal::Executable);
    }
    if account.data.len() != SOURCE_SPEC_ACCOUNT_V2_BYTES {
        return Err(V2SourceSpecRefusal::WrongLength);
    }
    if account.data[0] != SOURCE_SPEC_ACCOUNT_V2_TAG {
        return Err(V2SourceSpecRefusal::WrongTag);
    }
    if account.data[1] != SOURCE_SPEC_ACCOUNT_V2_VERSION {
        return Err(V2SourceSpecRefusal::WrongVersion);
    }
    if account.data[SPEC_FLAGS_OFFSET] != 0 {
        return Err(V2SourceSpecRefusal::NonCanonicalPadding);
    }
    if account.data[SPEC_BUMP_OFFSET] != expected_bump {
        return Err(V2SourceSpecRefusal::WrongBump);
    }
    let body = &account.data[SPEC_BODY_OFFSET..SPEC_BUMP_OFFSET];
    let spec = SourceSpecV2::decode_canonical(body).map_err(V2SourceSpecRefusal::Body)?;
    let stored = id_at(account.data, SPEC_FEED_OFFSET);
    if stored.is_zero() {
        return Err(V2SourceSpecRefusal::ZeroIdentity);
    }
    let mut hasher = Sha256::new();
    hasher.update(FEED_DOMAIN_V2);
    hasher.update(body);
    let recomputed = ContentId::from_bytes(hasher.finalize().into());
    if stored != recomputed {
        return Err(V2SourceSpecRefusal::DigestMismatch);
    }
    Ok(V2SourceSpecBinding {
        account_key: account.key,
        owner: account.owner,
        feed_id: stored,
        stored_bump: expected_bump,
        spec,
    })
}

/// Opaque output reserved for the future live V2 spec/Terms/release join.
///
/// There is deliberately no public constructor. The current host SourceSpec
/// projection is not enough to create this capability; promotion must move or
/// reuse the complete runtime verifier inside this crate's trust boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V2AuthenticatedSourceRoute {
    binding: V2SourceSpecBinding,
    source_plane_contract_id: ContentId,
    terms_digest: ContentId,
    compiled_release_digest: ContentId,
}

impl V2AuthenticatedSourceRoute {
    /// Existing canonical V2 feed identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.binding.feed_id
    }

    /// Exact reviewed V3 SourcePlane release selected by this route.
    pub const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    /// Immutable Terms digest authenticated by the reserved live constructor.
    pub const fn terms_digest(self) -> ContentId {
        self.terms_digest
    }

    /// Compiled adapter/parser/receiver release digest.
    pub const fn compiled_release_digest(self) -> ContentId {
        self.compiled_release_digest
    }
}

/// Exact field projection of one expected V2 archive-record fixture.
///
/// Its layout is not a V3 layout: V2 stores `bucket` in bytes 0..8, while V3
/// stores kind/reserved there and derives bucket from page position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V2ArchiveRecord {
    /// Explicit V2 bucket.
    pub bucket: u64,
    /// Conservative low endpoint.
    pub low: u128,
    /// Conservative high endpoint.
    pub high: u128,
    /// V2 source sequence, constructed as publish time by live admission.
    pub sequence: u64,
    /// Receiver-write slot; V2 does not require it to be monotone.
    pub publish_slot: u64,
    /// Source publish time.
    pub publish_time: u64,
}

impl V2ArchiveRecord {
    /// Decode the exact 64-byte V2 field layout.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != ARCHIVE_RECORD_V2_BYTES {
            return Err(Error::WrongLength);
        }
        Ok(Self {
            bucket: u64_at(input, 0),
            low: u128_at(input, 8),
            high: u128_at(input, 24),
            sequence: u64_at(input, 40),
            publish_slot: u64_at(input, 48),
            publish_time: u64_at(input, 56),
        })
    }

    /// Encode the model-derived expected V2 fixture layout.
    pub fn encode(self) -> [u8; ARCHIVE_RECORD_V2_BYTES] {
        let mut output = [0; ARCHIVE_RECORD_V2_BYTES];
        output[0..8].copy_from_slice(&self.bucket.to_le_bytes());
        output[8..24].copy_from_slice(&self.low.to_le_bytes());
        output[24..40].copy_from_slice(&self.high.to_le_bytes());
        output[40..48].copy_from_slice(&self.sequence.to_le_bytes());
        output[48..56].copy_from_slice(&self.publish_slot.to_le_bytes());
        output[56..64].copy_from_slice(&self.publish_time.to_le_bytes());
        output
    }

    /// Reproject wire fields into a V3 record candidate at a state-owned
    /// bucket. This proves representation compatibility only; it does not
    /// authenticate a V2 update, SourceSpec, Terms, release, Clock, or adjacent
    /// receiver instruction and cannot construct [`V2AuthenticatedRecord`].
    pub fn project_v3_candidate(self, expected_bucket: u64) -> Result<RawRecordV3> {
        if self.bucket != expected_bucket
            || self.low > self.high
            || self.high > MAX_SOURCE_VALUE
            || self.sequence != self.publish_time
        {
            return Err(Error::V2ProjectionUnavailable);
        }
        Ok(RawRecordV3::observation(
            self.low,
            self.high,
            self.sequence,
            self.publish_slot,
            self.publish_time,
        ))
    }
}

/// One record emitted by the complete live V2 authentication join.
///
/// `authentication_digest` is an adapter-authenticated transcript commitment,
/// not proof by itself. A live constructor must remain behind the existing V2
/// owner/PDA/release/config/adjacent-post/Clock/CROSSING verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V2AuthenticatedRecord {
    source_spec_id: ContentId,
    source_plane_contract_id: ContentId,
    authentication_digest: ContentId,
    record: V2ArchiveRecord,
}

impl V2AuthenticatedRecord {
    /// Exact V2 feed identity this evidence was authenticated against.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Exact reviewed V3 release selected by the adapter route.
    pub const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    /// Full authentication transcript commitment for intent binding.
    pub const fn authentication_digest(self) -> ContentId {
        self.authentication_digest
    }

    /// Original authenticated V2 record fields.
    pub const fn record(self) -> V2ArchiveRecord {
        self.record
    }

    /// Transcode only at the state-owned expected bucket and exact release.
    pub fn project_v3(
        self,
        source_spec_id: ContentId,
        source_plane_contract_id: ContentId,
        expected_bucket: u64,
    ) -> Result<RawRecordV3> {
        if self.source_spec_id != source_spec_id
            || self.source_plane_contract_id != source_plane_contract_id
        {
            return Err(Error::MismatchedState);
        }
        self.record.project_v3_candidate(expected_bucket)
    }
}

const _: () = assert!(SPEC_BODY_OFFSET + SOURCE_SPEC_V2_BYTES == SPEC_BUMP_OFFSET);

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn id_at(input: &[u8], offset: usize) -> ContentId {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&input[offset..offset + 32]);
    ContentId::from_bytes(bytes)
}

fn u64_at(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn u128_at(input: &[u8], offset: usize) -> u128 {
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&input[offset..offset + 16]);
    u128::from_le_bytes(bytes)
}
