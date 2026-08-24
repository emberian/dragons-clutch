#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free contract for creating and authenticating large immutable records.
//!
//! The final record account contains only the semantic bytes. Progress,
//! sponsorship, and page geometry live in a separate, temporary
//! [`StagingCursorV1`]. This crate prepares transitions but does not derive an
//! SVM PDA, hash bytes, inspect account ownership, transfer lamports, mutate
//! account data, or close accounts. Those operations are explicit obligations
//! of a composing [`RecordAdapterV1`].

use core::convert::TryFrom;

/// Exact width of an opaque schema/release identity or content digest.
pub const ID_BYTES: usize = 32;
/// Exact width of an SVM-compatible account identity.
pub const ACCOUNT_ID_BYTES: usize = 32;
/// Exact encoded width of the temporary V1 staging cursor.
pub const STAGING_CURSOR_BYTES_V1: usize = 296;
/// Exact encoded width of a Begin request.
pub const BEGIN_RECORD_BYTES_V1: usize = 176;
/// Exact fixed header width before one Append page's semantic bytes.
pub const APPEND_PAGE_HEADER_BYTES_V1: usize = 40;
/// Exact encoded width of a Finalize or Abort request.
pub const UNIT_REQUEST_BYTES_V1: usize = 16;

/// PDA seed domain for the one raw account keyed by schema/release and digest.
pub const RAW_RECORD_PDA_SEED_V1: &[u8] = b"dclutch-raw-record-v1";
/// PDA seed domain for the one temporary cursor paired with a raw account.
pub const STAGING_CURSOR_PDA_SEED_V1: &[u8] = b"dclutch-record-stage-v1";

/// Canonical staging-cursor magic.
pub const STAGING_CURSOR_MAGIC_V1: [u8; 8] = *b"DCLTRCR1";
/// Canonical record-instruction magic.
pub const RECORD_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTRIX1";
/// Implemented cursor and request schema version.
pub const RECORD_SCHEMA_VERSION_V1: u16 = 1;

const HEADER_BYTES: usize = 16;
const CURSOR_STATUS_OFFSET: usize = 10;
const CURSOR_ENVELOPE_KIND_OFFSET: usize = 11;
const CURSOR_SCHEMA_ID_OFFSET: usize = 16;
const CURSOR_DIGEST_OFFSET: usize = 48;
const CURSOR_ENVELOPE_BASIS_OFFSET: usize = 80;
const CURSOR_LIVENESS_POLICY_OFFSET: usize = 112;
const CURSOR_RECORD_ACCOUNT_OFFSET: usize = 144;
const CURSOR_STAGING_ACCOUNT_OFFSET: usize = 176;
const CURSOR_SPONSOR_OFFSET: usize = 208;
const CURSOR_EXACT_LENGTH_OFFSET: usize = 240;
const CURSOR_PAGE_BYTES_OFFSET: usize = 248;
const CURSOR_GEOMETRY_RESERVED_OFFSET: usize = 252;
const CURSOR_PAGE_COUNT_OFFSET: usize = 256;
const CURSOR_NEXT_PAGE_OFFSET: usize = 264;
const CURSOR_NEXT_OFFSET_OFFSET: usize = 272;
const CURSOR_EXPIRY_SLOT_OFFSET: usize = 280;
const CURSOR_CLEANUP_BOUNTY_OFFSET: usize = 288;

const BEGIN_SCHEMA_ID_OFFSET: usize = 16;
const BEGIN_DIGEST_OFFSET: usize = 48;
const BEGIN_EXACT_LENGTH_OFFSET: usize = 80;
const BEGIN_ENVELOPE_KIND_OFFSET: usize = 88;
const BEGIN_ENVELOPE_RESERVED_OFFSET: usize = 89;
const BEGIN_PAGE_BYTES_OFFSET: usize = 92;
const BEGIN_ENVELOPE_BASIS_OFFSET: usize = 96;
const BEGIN_LIVENESS_POLICY_OFFSET: usize = 128;
const BEGIN_EXPIRY_SLOT_OFFSET: usize = 160;
const BEGIN_CLEANUP_BOUNTY_OFFSET: usize = 168;

const APPEND_PAGE_INDEX_OFFSET: usize = 16;
const APPEND_OFFSET_OFFSET: usize = 24;
const APPEND_LENGTH_OFFSET: usize = 32;
const APPEND_RESERVED_OFFSET: usize = 36;

/// Refusal from an exact decoder or pure record transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have its one exact required width.
    InvalidLength,
    /// Magic bytes did not identify this contract.
    InvalidMagic,
    /// The encoded schema version is not implemented.
    UnsupportedSchema,
    /// A request carried an unknown action discriminator.
    UnknownAction,
    /// Reserved bytes were not canonically zero.
    NonCanonicalReservedBytes,
    /// An opaque identity used the reserved all-zero value.
    ZeroIdentity,
    /// Two account roles used the same key.
    AccountAlias,
    /// A record with no semantic bytes was requested.
    ZeroRecordLength,
    /// Page geometry used an unknown kind or zero page width.
    InvalidPageEnvelope,
    /// The adapter refused the selected measured/provisional page envelope.
    PageEnvelopeRefused,
    /// The selected staging liveness policy was not authenticated.
    StagingPolicyRefused,
    /// Expiry was not strictly future or exceeded the authenticated policy.
    InvalidExpiry,
    /// Cleanup bounty was zero, below policy, or absent from staged principal.
    InsufficientCleanupBounty,
    /// Checked length, offset, or page-count arithmetic overflowed.
    ArithmeticOverflow,
    /// Encoded page geometry or cursor progress was not canonical.
    GeometryMismatch,
    /// The adapter refused the canonical raw/cursor PDA derivations.
    AddressDerivationRefused,
    /// Supplied raw or cursor accounts did not match the staged binding.
    CursorBindingMismatch,
    /// An Append repeated an already committed page.
    PageReplay,
    /// An Append skipped or reordered a page.
    PageOutOfOrder,
    /// An Append's byte range overlapped committed bytes.
    PageOverlap,
    /// An Append left a gap before its byte range.
    PageGap,
    /// An Append page did not have its exact committed width.
    PageLengthMismatch,
    /// All committed pages were already appended.
    CursorComplete,
    /// Finalization was requested before every byte was committed.
    CursorIncomplete,
    /// The selected adapter refused exact hashing or schema semantics.
    AdapterValidationRefused,
    /// A non-sponsor requested cleanup before the immutable expiry slot.
    AbortBeforeExpiry,
    /// Exact raw/cursor lamport disposition did not conserve observed balances.
    LamportConservationMismatch,
    /// A cursor did not encode the sole live V1 staging status.
    InvalidCursorStatus,
    /// An output buffer did not have its exact required width.
    OutputLength,
}

/// Result alias for record-contract operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Opaque nonzero identity of one semantic schema and validator release.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SchemaReleaseId([u8; ID_BYTES]);

impl SchemaReleaseId {
    /// Construct a nonzero opaque schema/release identity.
    pub fn new(bytes: [u8; ID_BYTES]) -> Result<Self> {
        require_nonzero(&bytes)?;
        Ok(Self(bytes))
    }

    /// Decode one exact nonzero identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact identity bytes.
    pub const fn to_bytes(self) -> [u8; ID_BYTES] {
        self.0
    }

    /// Borrow the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; ID_BYTES] {
        &self.0
    }
}

/// Opaque nonzero digest of the exact final semantic byte sequence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentDigest([u8; ID_BYTES]);

impl ContentDigest {
    /// Construct a nonzero expected content digest.
    pub fn new(bytes: [u8; ID_BYTES]) -> Result<Self> {
        require_nonzero(&bytes)?;
        Ok(Self(bytes))
    }

    /// Decode one exact nonzero digest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact digest bytes.
    pub const fn to_bytes(self) -> [u8; ID_BYTES] {
        self.0
    }

    /// Borrow the exact digest bytes.
    pub const fn as_bytes(&self) -> &[u8; ID_BYTES] {
        &self.0
    }
}

/// Validated nonzero account identity used without importing an SVM SDK.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct AccountId([u8; ACCOUNT_ID_BYTES]);

impl AccountId {
    /// Construct a nonzero account identity.
    pub fn new(bytes: [u8; ACCOUNT_ID_BYTES]) -> Result<Self> {
        require_nonzero(&bytes)?;
        Ok(Self(bytes))
    }

    /// Decode one exact nonzero account identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact account bytes.
    pub const fn to_bytes(self) -> [u8; ACCOUNT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact account bytes.
    pub const fn as_bytes(&self) -> &[u8; ACCOUNT_ID_BYTES] {
        &self.0
    }
}

/// Canonical raw-record identity independent of staging geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordKeyV1 {
    schema_release_id: SchemaReleaseId,
    expected_digest: ContentDigest,
}

impl RecordKeyV1 {
    /// Bind one semantic schema/release to the expected exact content digest.
    pub const fn new(schema_release_id: SchemaReleaseId, expected_digest: ContentDigest) -> Self {
        Self {
            schema_release_id,
            expected_digest,
        }
    }

    /// Return the opaque schema and validator release identity.
    pub const fn schema_release_id(self) -> SchemaReleaseId {
        self.schema_release_id
    }

    /// Return the digest of the exact semantic bytes.
    pub const fn expected_digest(self) -> ContentDigest {
        self.expected_digest
    }

    /// Return the three exact raw-record PDA seed components.
    pub const fn raw_record_pda_seeds(self) -> RecordPdaSeedsV1 {
        RecordPdaSeedsV1 {
            domain: RAW_RECORD_PDA_SEED_V1,
            schema_release_id: self.schema_release_id,
            expected_digest: self.expected_digest,
        }
    }

    /// Return the three exact staging-cursor PDA seed components.
    pub const fn staging_cursor_pda_seeds(self) -> RecordPdaSeedsV1 {
        RecordPdaSeedsV1 {
            domain: STAGING_CURSOR_PDA_SEED_V1,
            schema_release_id: self.schema_release_id,
            expected_digest: self.expected_digest,
        }
    }
}

/// Borrow-free exact PDA seed material exposed to the SVM adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPdaSeedsV1 {
    domain: &'static [u8],
    schema_release_id: SchemaReleaseId,
    expected_digest: ContentDigest,
}

impl RecordPdaSeedsV1 {
    /// Return the domain-separation seed.
    pub const fn domain(self) -> &'static [u8] {
        self.domain
    }

    /// Return the schema/release seed.
    pub const fn schema_release_id(self) -> SchemaReleaseId {
        self.schema_release_id
    }

    /// Return the expected digest seed.
    pub const fn expected_digest(self) -> ContentDigest {
        self.expected_digest
    }
}

/// Evidence class for one bounded transaction page envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PageEnvelopeKindV1 {
    /// `basis_id` identifies a reproducible transaction-envelope measurement.
    Measured = 1,
    /// `basis_id` identifies the required plan for lifting this temporary bound.
    Provisional = 2,
}

impl PageEnvelopeKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Measured),
            2 => Ok(Self::Provisional),
            _ => Err(Error::InvalidPageEnvelope),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// One named per-transaction Append bound and its evidence or lifting plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageEnvelopeV1 {
    kind: PageEnvelopeKindV1,
    page_bytes: u32,
    basis_id: SchemaReleaseId,
}

impl PageEnvelopeV1 {
    /// Construct a nonzero measured or provisional page envelope.
    pub fn new(
        kind: PageEnvelopeKindV1,
        page_bytes: u32,
        basis_id: SchemaReleaseId,
    ) -> Result<Self> {
        if page_bytes == 0 {
            return Err(Error::InvalidPageEnvelope);
        }
        Ok(Self {
            kind,
            page_bytes,
            basis_id,
        })
    }

    /// Return the evidence class.
    pub const fn kind(self) -> PageEnvelopeKindV1 {
        self.kind
    }

    /// Return the exact maximum semantic bytes in one Append request.
    pub const fn page_bytes(self) -> u32 {
        self.page_bytes
    }

    /// Return the measurement-manifest or lifting-plan identity.
    pub const fn basis_id(self) -> SchemaReleaseId {
        self.basis_id
    }

    fn page_count(self, exact_length: u64) -> Result<u64> {
        if exact_length == 0 {
            return Err(Error::ZeroRecordLength);
        }
        let width = u64::from(self.page_bytes);
        let prior = exact_length
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        prior
            .checked_div(width)
            .and_then(|value| value.checked_add(1))
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Authenticated deployment policy bounding how long one canonical PDA can be staged.
///
/// `policy_id` is an opaque release/content identity authenticated by the
/// composing adapter. Keeping limits outside the cursor permits a release to
/// tighten future Begin admissions without changing an in-progress record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagingLivenessPolicyV1 {
    policy_id: SchemaReleaseId,
    maximum_lifetime_slots: u64,
    minimum_cleanup_bounty_lamports: u64,
}

impl StagingLivenessPolicyV1 {
    /// Construct positive anti-squatting limits for one authenticated policy.
    pub fn new(
        policy_id: SchemaReleaseId,
        maximum_lifetime_slots: u64,
        minimum_cleanup_bounty_lamports: u64,
    ) -> Result<Self> {
        if maximum_lifetime_slots == 0 {
            return Err(Error::InvalidExpiry);
        }
        if minimum_cleanup_bounty_lamports == 0 {
            return Err(Error::InsufficientCleanupBounty);
        }
        Ok(Self {
            policy_id,
            maximum_lifetime_slots,
            minimum_cleanup_bounty_lamports,
        })
    }

    /// Return the opaque authenticated policy identity.
    pub const fn policy_id(self) -> SchemaReleaseId {
        self.policy_id
    }

    /// Return the maximum admitted `expiry_slot - begin_slot` interval.
    pub const fn maximum_lifetime_slots(self) -> u64 {
        self.maximum_lifetime_slots
    }

    /// Return the minimum separately prepaid cleanup bounty.
    pub const fn minimum_cleanup_bounty_lamports(self) -> u64 {
        self.minimum_cleanup_bounty_lamports
    }
}

/// Exact permissionless Begin request. Account roles carry the sponsor and PDAs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginRecordV1 {
    key: RecordKeyV1,
    exact_length: u64,
    page_envelope: PageEnvelopeV1,
    liveness_policy_id: SchemaReleaseId,
    expiry_slot: u64,
    cleanup_bounty_lamports: u64,
}

impl BeginRecordV1 {
    /// Construct one canonical Begin request.
    pub fn new(
        key: RecordKeyV1,
        exact_length: u64,
        page_envelope: PageEnvelopeV1,
        liveness_policy_id: SchemaReleaseId,
        expiry_slot: u64,
        cleanup_bounty_lamports: u64,
    ) -> Result<Self> {
        page_envelope.page_count(exact_length)?;
        if expiry_slot == 0 {
            return Err(Error::InvalidExpiry);
        }
        if cleanup_bounty_lamports == 0 {
            return Err(Error::InsufficientCleanupBounty);
        }
        Ok(Self {
            key,
            exact_length,
            page_envelope,
            liveness_policy_id,
            expiry_slot,
            cleanup_bounty_lamports,
        })
    }

    /// Decode one exact canonical Begin request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        instruction_header(bytes, BEGIN_RECORD_BYTES_V1, RecordActionV1::Begin)?;
        zero(bytes, 11, 5)?;
        zero(bytes, BEGIN_ENVELOPE_RESERVED_OFFSET, 3)?;
        let key = RecordKeyV1::new(
            SchemaReleaseId::decode(slice(bytes, BEGIN_SCHEMA_ID_OFFSET, ID_BYTES)?)?,
            ContentDigest::decode(slice(bytes, BEGIN_DIGEST_OFFSET, ID_BYTES)?)?,
        );
        let envelope = PageEnvelopeV1::new(
            PageEnvelopeKindV1::decode(read_u8(bytes, BEGIN_ENVELOPE_KIND_OFFSET)?)?,
            read_u32(bytes, BEGIN_PAGE_BYTES_OFFSET)?,
            SchemaReleaseId::decode(slice(bytes, BEGIN_ENVELOPE_BASIS_OFFSET, ID_BYTES)?)?,
        )?;
        Self::new(
            key,
            read_u64(bytes, BEGIN_EXACT_LENGTH_OFFSET)?,
            envelope,
            SchemaReleaseId::decode(slice(bytes, BEGIN_LIVENESS_POLICY_OFFSET, ID_BYTES)?)?,
            read_u64(bytes, BEGIN_EXPIRY_SLOT_OFFSET)?,
            read_u64(bytes, BEGIN_CLEANUP_BOUNTY_OFFSET)?,
        )
    }

    /// Encode the exact canonical Begin request.
    pub fn to_bytes(self) -> [u8; BEGIN_RECORD_BYTES_V1] {
        let mut output = [0; BEGIN_RECORD_BYTES_V1];
        write_instruction_header(&mut output, RecordActionV1::Begin);
        put(
            &mut output,
            BEGIN_SCHEMA_ID_OFFSET,
            self.key.schema_release_id.as_bytes(),
        );
        put(
            &mut output,
            BEGIN_DIGEST_OFFSET,
            self.key.expected_digest.as_bytes(),
        );
        put(
            &mut output,
            BEGIN_EXACT_LENGTH_OFFSET,
            &self.exact_length.to_le_bytes(),
        );
        output[BEGIN_ENVELOPE_KIND_OFFSET] = self.page_envelope.kind.byte();
        put(
            &mut output,
            BEGIN_PAGE_BYTES_OFFSET,
            &self.page_envelope.page_bytes.to_le_bytes(),
        );
        put(
            &mut output,
            BEGIN_ENVELOPE_BASIS_OFFSET,
            self.page_envelope.basis_id.as_bytes(),
        );
        put(
            &mut output,
            BEGIN_LIVENESS_POLICY_OFFSET,
            self.liveness_policy_id.as_bytes(),
        );
        put(
            &mut output,
            BEGIN_EXPIRY_SLOT_OFFSET,
            &self.expiry_slot.to_le_bytes(),
        );
        put(
            &mut output,
            BEGIN_CLEANUP_BOUNTY_OFFSET,
            &self.cleanup_bounty_lamports.to_le_bytes(),
        );
        output
    }

    /// Return the canonical record identity.
    pub const fn key(self) -> RecordKeyV1 {
        self.key
    }

    /// Return the exact raw-account data length.
    pub const fn exact_length(self) -> u64 {
        self.exact_length
    }

    /// Return the committed bounded page envelope.
    pub const fn page_envelope(self) -> PageEnvelopeV1 {
        self.page_envelope
    }

    /// Return the authenticated staging-policy identity.
    pub const fn liveness_policy_id(self) -> SchemaReleaseId {
        self.liveness_policy_id
    }

    /// Return the immutable absolute cleanup-enablement slot.
    pub const fn expiry_slot(self) -> u64 {
        self.expiry_slot
    }

    /// Return the separately prepaid cleanup bounty.
    pub const fn cleanup_bounty_lamports(self) -> u64 {
        self.cleanup_bounty_lamports
    }
}

/// One borrowed Append request whose trailing bytes are written verbatim.
#[derive(Debug, Eq, PartialEq)]
pub struct AppendPageV1<'page> {
    page_index: u64,
    offset: u64,
    page: &'page [u8],
}

impl<'page> AppendPageV1<'page> {
    /// Construct an Append request. Geometry is checked against a cursor later.
    pub fn new(page_index: u64, offset: u64, page: &'page [u8]) -> Result<Self> {
        u32::try_from(page.len()).map_err(|_| Error::InvalidLength)?;
        Ok(Self {
            page_index,
            offset,
            page,
        })
    }

    /// Decode an exact header followed by the declared semantic page bytes.
    pub fn decode(bytes: &'page [u8]) -> Result<Self> {
        if bytes.len() < APPEND_PAGE_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        instruction_header_prefix(bytes, RecordActionV1::AppendPage)?;
        zero(bytes, 11, 5)?;
        zero(bytes, APPEND_RESERVED_OFFSET, 4)?;
        let declared = usize::try_from(read_u32(bytes, APPEND_LENGTH_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        let exact = APPEND_PAGE_HEADER_BYTES_V1
            .checked_add(declared)
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != exact {
            return Err(Error::InvalidLength);
        }
        let page = bytes
            .get(APPEND_PAGE_HEADER_BYTES_V1..exact)
            .ok_or(Error::InvalidLength)?;
        Self::new(
            read_u64(bytes, APPEND_PAGE_INDEX_OFFSET)?,
            read_u64(bytes, APPEND_OFFSET_OFFSET)?,
            page,
        )
    }

    /// Encode atomically into an exact caller-owned buffer.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let exact = APPEND_PAGE_HEADER_BYTES_V1
            .checked_add(self.page.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if output.len() != exact {
            return Err(Error::OutputLength);
        }
        let page_length = u32::try_from(self.page.len()).map_err(|_| Error::InvalidLength)?;
        output.fill(0);
        write_instruction_header(output, RecordActionV1::AppendPage);
        put(
            output,
            APPEND_PAGE_INDEX_OFFSET,
            &self.page_index.to_le_bytes(),
        );
        put(output, APPEND_OFFSET_OFFSET, &self.offset.to_le_bytes());
        put(output, APPEND_LENGTH_OFFSET, &page_length.to_le_bytes());
        put(output, APPEND_PAGE_HEADER_BYTES_V1, self.page);
        Ok(())
    }

    /// Return the exact encoded request width.
    pub fn encoded_len(&self) -> Result<usize> {
        APPEND_PAGE_HEADER_BYTES_V1
            .checked_add(self.page.len())
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return the zero-based page index.
    pub const fn page_index(&self) -> u64 {
        self.page_index
    }

    /// Return the exact raw-account byte offset.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Borrow the semantic bytes to write verbatim.
    pub const fn page(&self) -> &'page [u8] {
        self.page
    }
}

/// Finalize request with no caller-authored digest or finality flag.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FinalizeRecordV1;

impl FinalizeRecordV1 {
    /// Decode the exact unit Finalize request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        instruction_header(bytes, UNIT_REQUEST_BYTES_V1, RecordActionV1::Finalize)?;
        zero(bytes, 11, 5)?;
        Ok(Self)
    }

    /// Encode the exact unit Finalize request.
    pub fn to_bytes(self) -> [u8; UNIT_REQUEST_BYTES_V1] {
        let mut output = [0; UNIT_REQUEST_BYTES_V1];
        write_instruction_header(&mut output, RecordActionV1::Finalize);
        output
    }
}

/// Sponsor-authorized Abort request with no caller-selected refund target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AbortRecordV1;

impl AbortRecordV1 {
    /// Decode the exact unit Abort request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        instruction_header(bytes, UNIT_REQUEST_BYTES_V1, RecordActionV1::Abort)?;
        zero(bytes, 11, 5)?;
        Ok(Self)
    }

    /// Encode the exact unit Abort request.
    pub fn to_bytes(self) -> [u8; UNIT_REQUEST_BYTES_V1] {
        let mut output = [0; UNIT_REQUEST_BYTES_V1];
        write_instruction_header(&mut output, RecordActionV1::Abort);
        output
    }
}

/// Sole persisted live status. Finalize and Abort both close this account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StagingStatusV1 {
    /// The raw account is not consumer-authenticatable and accepts one next page.
    Building = 1,
}

impl StagingStatusV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Building),
            _ => Err(Error::InvalidCursorStatus),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Small temporary state for one headerless raw record under construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagingCursorV1 {
    status: StagingStatusV1,
    key: RecordKeyV1,
    page_envelope: PageEnvelopeV1,
    liveness_policy_id: SchemaReleaseId,
    raw_record_account: AccountId,
    staging_account: AccountId,
    sponsor_rent_refund: AccountId,
    exact_length: u64,
    page_count: u64,
    next_page: u64,
    next_offset: u64,
    expiry_slot: u64,
    cleanup_bounty_lamports: u64,
}

impl StagingCursorV1 {
    /// Decode one exact hostile cursor and recheck all derived geometry.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != STAGING_CURSOR_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != STAGING_CURSOR_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != RECORD_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        zero(bytes, 12, 4)?;
        zero(bytes, CURSOR_GEOMETRY_RESERVED_OFFSET, 4)?;
        let status = StagingStatusV1::decode(read_u8(bytes, CURSOR_STATUS_OFFSET)?)?;
        let key = RecordKeyV1::new(
            SchemaReleaseId::decode(slice(bytes, CURSOR_SCHEMA_ID_OFFSET, ID_BYTES)?)?,
            ContentDigest::decode(slice(bytes, CURSOR_DIGEST_OFFSET, ID_BYTES)?)?,
        );
        let page_envelope = PageEnvelopeV1::new(
            PageEnvelopeKindV1::decode(read_u8(bytes, CURSOR_ENVELOPE_KIND_OFFSET)?)?,
            read_u32(bytes, CURSOR_PAGE_BYTES_OFFSET)?,
            SchemaReleaseId::decode(slice(bytes, CURSOR_ENVELOPE_BASIS_OFFSET, ID_BYTES)?)?,
        )?;
        let liveness_policy_id =
            SchemaReleaseId::decode(slice(bytes, CURSOR_LIVENESS_POLICY_OFFSET, ID_BYTES)?)?;
        let raw_record_account = AccountId::decode(slice(
            bytes,
            CURSOR_RECORD_ACCOUNT_OFFSET,
            ACCOUNT_ID_BYTES,
        )?)?;
        let staging_account = AccountId::decode(slice(
            bytes,
            CURSOR_STAGING_ACCOUNT_OFFSET,
            ACCOUNT_ID_BYTES,
        )?)?;
        let sponsor_rent_refund =
            AccountId::decode(slice(bytes, CURSOR_SPONSOR_OFFSET, ACCOUNT_ID_BYTES)?)?;
        require_distinct(&[raw_record_account, staging_account, sponsor_rent_refund])?;
        let exact_length = read_u64(bytes, CURSOR_EXACT_LENGTH_OFFSET)?;
        let page_count = read_u64(bytes, CURSOR_PAGE_COUNT_OFFSET)?;
        let next_page = read_u64(bytes, CURSOR_NEXT_PAGE_OFFSET)?;
        let next_offset = read_u64(bytes, CURSOR_NEXT_OFFSET_OFFSET)?;
        let expiry_slot = read_u64(bytes, CURSOR_EXPIRY_SLOT_OFFSET)?;
        let cleanup_bounty_lamports = read_u64(bytes, CURSOR_CLEANUP_BOUNTY_OFFSET)?;
        if expiry_slot == 0 {
            return Err(Error::InvalidExpiry);
        }
        if cleanup_bounty_lamports == 0 {
            return Err(Error::InsufficientCleanupBounty);
        }
        if page_count != page_envelope.page_count(exact_length)? || next_page > page_count {
            return Err(Error::GeometryMismatch);
        }
        let expected_offset = offset_after_pages(page_envelope, exact_length, next_page)?;
        if next_offset != expected_offset {
            return Err(Error::GeometryMismatch);
        }
        Ok(Self {
            status,
            key,
            page_envelope,
            liveness_policy_id,
            raw_record_account,
            staging_account,
            sponsor_rent_refund,
            exact_length,
            page_count,
            next_page,
            next_offset,
            expiry_slot,
            cleanup_bounty_lamports,
        })
    }

    /// Encode exact canonical cursor bytes.
    pub fn to_bytes(self) -> [u8; STAGING_CURSOR_BYTES_V1] {
        let mut output = [0; STAGING_CURSOR_BYTES_V1];
        put(&mut output, 0, &STAGING_CURSOR_MAGIC_V1);
        put(&mut output, 8, &RECORD_SCHEMA_VERSION_V1.to_le_bytes());
        output[CURSOR_STATUS_OFFSET] = self.status.byte();
        output[CURSOR_ENVELOPE_KIND_OFFSET] = self.page_envelope.kind.byte();
        put(
            &mut output,
            CURSOR_SCHEMA_ID_OFFSET,
            self.key.schema_release_id.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_DIGEST_OFFSET,
            self.key.expected_digest.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_ENVELOPE_BASIS_OFFSET,
            self.page_envelope.basis_id.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_LIVENESS_POLICY_OFFSET,
            self.liveness_policy_id.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_RECORD_ACCOUNT_OFFSET,
            self.raw_record_account.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_STAGING_ACCOUNT_OFFSET,
            self.staging_account.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_SPONSOR_OFFSET,
            self.sponsor_rent_refund.as_bytes(),
        );
        put(
            &mut output,
            CURSOR_EXACT_LENGTH_OFFSET,
            &self.exact_length.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_PAGE_BYTES_OFFSET,
            &self.page_envelope.page_bytes.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_PAGE_COUNT_OFFSET,
            &self.page_count.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_NEXT_PAGE_OFFSET,
            &self.next_page.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_NEXT_OFFSET_OFFSET,
            &self.next_offset.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_EXPIRY_SLOT_OFFSET,
            &self.expiry_slot.to_le_bytes(),
        );
        put(
            &mut output,
            CURSOR_CLEANUP_BOUNTY_OFFSET,
            &self.cleanup_bounty_lamports.to_le_bytes(),
        );
        output
    }

    /// Return the sole live cursor status.
    pub const fn status(self) -> StagingStatusV1 {
        self.status
    }

    /// Return the canonical content key.
    pub const fn key(self) -> RecordKeyV1 {
        self.key
    }

    /// Return the bounded per-transaction page envelope.
    pub const fn page_envelope(self) -> PageEnvelopeV1 {
        self.page_envelope
    }

    /// Return the authenticated anti-squatting policy identity.
    pub const fn liveness_policy_id(self) -> SchemaReleaseId {
        self.liveness_policy_id
    }

    /// Return the raw content account.
    pub const fn raw_record_account(self) -> AccountId {
        self.raw_record_account
    }

    /// Return the temporary staging account.
    pub const fn staging_account(self) -> AccountId {
        self.staging_account
    }

    /// Return the immutable sponsor and sole rent-refund destination.
    pub const fn sponsor_rent_refund(self) -> AccountId {
        self.sponsor_rent_refund
    }

    /// Return the exact final semantic byte length.
    pub const fn exact_length(self) -> u64 {
        self.exact_length
    }

    /// Return the checked total page count.
    pub const fn page_count(self) -> u64 {
        self.page_count
    }

    /// Return the sole replay cursor: the next required page index.
    pub const fn next_page(self) -> u64 {
        self.next_page
    }

    /// Return the next required raw-account byte offset.
    pub const fn next_offset(self) -> u64 {
        self.next_offset
    }

    /// Return the absolute slot enabling permissionless cleanup.
    pub const fn expiry_slot(self) -> u64 {
        self.expiry_slot
    }

    /// Return the exact bounty paid only for expired permissionless cleanup.
    pub const fn cleanup_bounty_lamports(self) -> u64 {
        self.cleanup_bounty_lamports
    }

    /// Report whether every committed byte has been appended.
    pub const fn is_complete(self) -> bool {
        self.next_page == self.page_count && self.next_offset == self.exact_length
    }

    fn expected_next_page_length(self) -> Result<u32> {
        if self.is_complete() {
            return Err(Error::CursorComplete);
        }
        let remaining = self
            .exact_length
            .checked_sub(self.next_offset)
            .ok_or(Error::GeometryMismatch)?;
        let length = core::cmp::min(remaining, u64::from(self.page_envelope.page_bytes));
        u32::try_from(length).map_err(|_| Error::ArithmeticOverflow)
    }
}

/// Exact address-derivation check requested from a composing adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressDerivationObligationV1 {
    key: RecordKeyV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
}

impl AddressDerivationObligationV1 {
    /// Return the record key that supplies both PDA identity seeds.
    pub const fn key(self) -> RecordKeyV1 {
        self.key
    }

    /// Return the claimed canonical raw-record account.
    pub const fn raw_record_account(self) -> AccountId {
        self.raw_record_account
    }

    /// Return the claimed canonical staging-cursor account.
    pub const fn staging_account(self) -> AccountId {
        self.staging_account
    }
}

/// Context in which the adapter validates the exact raw semantic bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawRecordValidationModeV1 {
    /// The complete live cursor is closed only after the adapter returns true.
    Finalization,
    /// The adapter must additionally attest that the canonical cursor is absent.
    ConsumerAuthentication,
}

/// Exact hashing and semantic-validation obligation passed to the adapter.
#[derive(Debug, Eq, PartialEq)]
pub struct RawRecordValidationObligationV1<'content> {
    mode: RawRecordValidationModeV1,
    key: RecordKeyV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
    exact_content: &'content [u8],
}

impl<'content> RawRecordValidationObligationV1<'content> {
    /// Return the finalization or later-consumer validation mode.
    pub const fn mode(&self) -> RawRecordValidationModeV1 {
        self.mode
    }

    /// Return the schema/release and expected-digest identity.
    pub const fn key(&self) -> RecordKeyV1 {
        self.key
    }

    /// Return the raw record account whose complete data is supplied.
    pub const fn raw_record_account(&self) -> AccountId {
        self.raw_record_account
    }

    /// Return the paired canonical staging account.
    pub const fn staging_account(&self) -> AccountId {
        self.staging_account
    }

    /// Borrow the entire exact raw account data, including every zero byte.
    pub const fn exact_content(&self) -> &'content [u8] {
        self.exact_content
    }
}

/// Explicit trust boundary implemented by the SVM account/hash/schema adapter.
///
/// Neither `true` result is instruction data. The adapter must derive both
/// PDAs from this crate's exact seeds. Content validation must hash the entire
/// raw account data with the selected digest policy and invoke the validator
/// selected by `schema_release_id`. Consumer mode must also prove that the
/// canonical staging account is vacant, so a valid zero suffix cannot make an
/// incomplete staged record authenticate.
pub trait RecordAdapterV1 {
    /// Authenticate the selected page width, evidence class, and basis identity.
    fn validate_page_envelope(&self, envelope: &PageEnvelopeV1) -> bool;

    /// Authenticate the selected anti-squatting policy and its exact limits.
    fn validate_staging_liveness_policy(&self, policy: &StagingLivenessPolicyV1) -> bool;

    /// Confirm exact canonical PDA derivations under the adapter's program ID.
    fn validate_canonical_addresses(&self, obligation: &AddressDerivationObligationV1) -> bool;

    /// Confirm exact hashing, schema semantics, account ownership, and lifecycle.
    fn validate_raw_record(&self, obligation: &RawRecordValidationObligationV1<'_>) -> bool;
}

/// Allocation obligations returned after a successful Begin preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordAllocationV1 {
    raw_record_account: AccountId,
    raw_data_length: u64,
    staging_account: AccountId,
    staging_data_length: u64,
    sponsor_rent_refund: AccountId,
    cleanup_bounty_lamports: u64,
}

impl RecordAllocationV1 {
    /// Return the canonical raw account to allocate without a header.
    pub const fn raw_record_account(self) -> AccountId {
        self.raw_record_account
    }

    /// Return the exact semantic-data allocation length.
    pub const fn raw_data_length(self) -> u64 {
        self.raw_data_length
    }

    /// Return the canonical staging account to allocate.
    pub const fn staging_account(self) -> AccountId {
        self.staging_account
    }

    /// Return the exact V1 cursor allocation length.
    pub const fn staging_data_length(self) -> u64 {
        self.staging_data_length
    }

    /// Return the immutable signer/refund account funding both rents.
    pub const fn sponsor_rent_refund(self) -> AccountId {
        self.sponsor_rent_refund
    }

    /// Return the additional principal that must be held above staging rent.
    pub const fn cleanup_bounty_lamports(self) -> u64 {
        self.cleanup_bounty_lamports
    }
}

/// Move-only successful Begin transition.
#[derive(Debug, Eq, PartialEq)]
pub struct BeginTransitionV1 {
    cursor: StagingCursorV1,
    allocation: RecordAllocationV1,
}

impl BeginTransitionV1 {
    /// Return the exact cursor to write only after all allocation preflights.
    pub const fn cursor(&self) -> StagingCursorV1 {
        self.cursor
    }

    /// Return the raw/cursor allocation and rent ownership obligation.
    pub const fn allocation(&self) -> RecordAllocationV1 {
        self.allocation
    }
}

/// Prepare a permissionless Begin without performing account operations.
pub fn prepare_begin_v1<A: RecordAdapterV1>(
    adapter: &A,
    request: BeginRecordV1,
    liveness_policy: StagingLivenessPolicyV1,
    observed_current_slot: u64,
    raw_record_account: AccountId,
    staging_account: AccountId,
    sponsor_rent_refund: AccountId,
) -> Result<BeginTransitionV1> {
    require_distinct(&[raw_record_account, staging_account, sponsor_rent_refund])?;
    let addresses = AddressDerivationObligationV1 {
        key: request.key,
        raw_record_account,
        staging_account,
    };
    if !adapter.validate_canonical_addresses(&addresses) {
        return Err(Error::AddressDerivationRefused);
    }
    if !adapter.validate_page_envelope(&request.page_envelope) {
        return Err(Error::PageEnvelopeRefused);
    }
    if request.liveness_policy_id != liveness_policy.policy_id
        || !adapter.validate_staging_liveness_policy(&liveness_policy)
    {
        return Err(Error::StagingPolicyRefused);
    }
    let lifetime = request
        .expiry_slot
        .checked_sub(observed_current_slot)
        .ok_or(Error::InvalidExpiry)?;
    if lifetime == 0 || lifetime > liveness_policy.maximum_lifetime_slots {
        return Err(Error::InvalidExpiry);
    }
    if request.cleanup_bounty_lamports < liveness_policy.minimum_cleanup_bounty_lamports {
        return Err(Error::InsufficientCleanupBounty);
    }
    let page_count = request.page_envelope.page_count(request.exact_length)?;
    let cursor = StagingCursorV1 {
        status: StagingStatusV1::Building,
        key: request.key,
        page_envelope: request.page_envelope,
        liveness_policy_id: request.liveness_policy_id,
        raw_record_account,
        staging_account,
        sponsor_rent_refund,
        exact_length: request.exact_length,
        page_count,
        next_page: 0,
        next_offset: 0,
        expiry_slot: request.expiry_slot,
        cleanup_bounty_lamports: request.cleanup_bounty_lamports,
    };
    let staging_data_length =
        u64::try_from(STAGING_CURSOR_BYTES_V1).map_err(|_| Error::ArithmeticOverflow)?;
    Ok(BeginTransitionV1 {
        cursor,
        allocation: RecordAllocationV1 {
            raw_record_account,
            raw_data_length: request.exact_length,
            staging_account,
            staging_data_length,
            sponsor_rent_refund,
            cleanup_bounty_lamports: request.cleanup_bounty_lamports,
        },
    })
}

/// Move-only exact raw-account write prepared by one Append.
#[derive(Debug, Eq, PartialEq)]
pub struct RawPageWriteV1<'page> {
    raw_record_account: AccountId,
    offset: u64,
    page: &'page [u8],
}

impl<'page> RawPageWriteV1<'page> {
    /// Return the exact raw account to mutate.
    pub const fn raw_record_account(&self) -> AccountId {
        self.raw_record_account
    }

    /// Return the exact starting offset.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Borrow the bytes to copy verbatim.
    pub const fn page(&self) -> &'page [u8] {
        self.page
    }
}

/// Move-only Append transition joining one raw write to one next cursor.
#[derive(Debug, Eq, PartialEq)]
pub struct AppendTransitionV1<'page> {
    prior_cursor: StagingCursorV1,
    next_cursor: StagingCursorV1,
    write: RawPageWriteV1<'page>,
}

impl<'page> AppendTransitionV1<'page> {
    /// Return the exact prewrite cursor identity for adapter reauthentication.
    pub const fn prior_cursor(&self) -> StagingCursorV1 {
        self.prior_cursor
    }

    /// Return the next cursor to encode atomically with the raw write.
    pub const fn next_cursor(&self) -> StagingCursorV1 {
        self.next_cursor
    }

    /// Return the exact raw-account page write.
    pub const fn write(&self) -> &RawPageWriteV1<'page> {
        &self.write
    }
}

/// Prepare the sole next page. The returned write and cursor must commit atomically.
pub fn prepare_append_page_v1<'page>(
    cursor: StagingCursorV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
    observed_raw_data_length: u64,
    request: AppendPageV1<'page>,
) -> Result<AppendTransitionV1<'page>> {
    require_cursor_accounts(
        cursor,
        raw_record_account,
        staging_account,
        observed_raw_data_length,
    )?;
    if cursor.is_complete() {
        return Err(Error::CursorComplete);
    }
    if request.page_index < cursor.next_page {
        return Err(Error::PageReplay);
    }
    if request.page_index > cursor.next_page {
        return Err(Error::PageOutOfOrder);
    }
    if request.offset < cursor.next_offset {
        return Err(Error::PageOverlap);
    }
    if request.offset > cursor.next_offset {
        return Err(Error::PageGap);
    }
    let expected_length = cursor.expected_next_page_length()?;
    let actual_length = u32::try_from(request.page.len()).map_err(|_| Error::InvalidLength)?;
    if actual_length != expected_length {
        return Err(Error::PageLengthMismatch);
    }
    let next_page = cursor
        .next_page
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_offset = cursor
        .next_offset
        .checked_add(u64::from(actual_length))
        .ok_or(Error::ArithmeticOverflow)?;
    let next_cursor = StagingCursorV1 {
        next_page,
        next_offset,
        ..cursor
    };
    Ok(AppendTransitionV1 {
        prior_cursor: cursor,
        next_cursor,
        write: RawPageWriteV1 {
            raw_record_account,
            offset: request.offset,
            page: request.page,
        },
    })
}

/// One exact account-close and full-lamport-refund obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCloseV1 {
    account: AccountId,
    full_lamport_refund: AccountId,
    observed_lamports: u64,
}

impl AccountCloseV1 {
    /// Return the account whose complete lamport balance must be returned.
    pub const fn account(self) -> AccountId {
        self.account
    }

    /// Return the immutable refund destination.
    pub const fn full_lamport_refund(self) -> AccountId {
        self.full_lamport_refund
    }

    /// Return the complete preclose balance covered by this obligation.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }
}

/// Exact split of a staging balance during expired permissionless cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagingLamportCloseV1 {
    account: AccountId,
    cleanup_recipient: AccountId,
    cleanup_bounty_lamports: u64,
    sponsor_recipient: AccountId,
    sponsor_refund_lamports: u64,
    observed_lamports: u64,
}

impl StagingLamportCloseV1 {
    /// Return the staging account to close.
    pub const fn account(self) -> AccountId {
        self.account
    }

    /// Return the expired-cleanup bounty recipient.
    pub const fn cleanup_recipient(self) -> AccountId {
        self.cleanup_recipient
    }

    /// Return the exact cleanup bounty, or zero for a sponsor's early Abort.
    pub const fn cleanup_bounty_lamports(self) -> u64 {
        self.cleanup_bounty_lamports
    }

    /// Return the immutable sponsor recipient for every remaining lamport.
    pub const fn sponsor_recipient(self) -> AccountId {
        self.sponsor_recipient
    }

    /// Return the exact remaining balance refunded to sponsor.
    pub const fn sponsor_refund_lamports(self) -> u64 {
        self.sponsor_refund_lamports
    }

    /// Return the complete preclose staging balance.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }

    /// Recheck exact balance conservation without account mutation.
    pub fn validate_conservation(self) -> Result<()> {
        let total = self
            .cleanup_bounty_lamports
            .checked_add(self.sponsor_refund_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if total != self.observed_lamports {
            return Err(Error::LamportConservationMismatch);
        }
        Ok(())
    }
}

/// Move-only authority that a consumer may use only in the current instruction.
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedRawRecordV1<'content> {
    key: RecordKeyV1,
    raw_record_account: AccountId,
    exact_content: &'content [u8],
}

impl<'content> AuthenticatedRawRecordV1<'content> {
    /// Return the exact schema/release and digest identity.
    pub const fn key(&self) -> RecordKeyV1 {
        self.key
    }

    /// Return the canonical raw account.
    pub const fn raw_record_account(&self) -> AccountId {
        self.raw_record_account
    }

    /// Borrow the adapter-validated exact semantic bytes.
    pub const fn exact_content(&self) -> &'content [u8] {
        self.exact_content
    }
}

/// Move-only Finalize transition: retain raw bytes and close only the cursor.
#[derive(Debug, Eq, PartialEq)]
pub struct FinalizeTransitionV1<'content> {
    authenticated_record: AuthenticatedRawRecordV1<'content>,
    staging_close: AccountCloseV1,
}

impl<'content> FinalizeTransitionV1<'content> {
    /// Borrow the same-instruction authenticated raw-record authority.
    pub const fn authenticated_record(&self) -> &AuthenticatedRawRecordV1<'content> {
        &self.authenticated_record
    }

    /// Return the cursor-close and sponsor-refund obligation.
    pub const fn staging_close(&self) -> AccountCloseV1 {
        self.staging_close
    }
}

/// Prepare Finalize only after complete geometry and adapter validation.
pub fn prepare_finalize_v1<'content, A: RecordAdapterV1>(
    adapter: &A,
    cursor: StagingCursorV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
    observed_staging_lamports: u64,
    exact_content: &'content [u8],
) -> Result<FinalizeTransitionV1<'content>> {
    let observed_length = u64::try_from(exact_content.len()).map_err(|_| Error::InvalidLength)?;
    require_cursor_accounts(cursor, raw_record_account, staging_account, observed_length)?;
    if !cursor.is_complete() {
        return Err(Error::CursorIncomplete);
    }
    require_adapter_addresses(adapter, cursor.key, raw_record_account, staging_account)?;
    let validation = RawRecordValidationObligationV1 {
        mode: RawRecordValidationModeV1::Finalization,
        key: cursor.key,
        raw_record_account,
        staging_account,
        exact_content,
    };
    if !adapter.validate_raw_record(&validation) {
        return Err(Error::AdapterValidationRefused);
    }
    Ok(FinalizeTransitionV1 {
        authenticated_record: AuthenticatedRawRecordV1 {
            key: cursor.key,
            raw_record_account,
            exact_content,
        },
        staging_close: AccountCloseV1 {
            account: staging_account,
            full_lamport_refund: cursor.sponsor_rent_refund,
            observed_lamports: observed_staging_lamports,
        },
    })
}

/// Authenticate a finalized headerless raw record for a later consumer.
///
/// The adapter must prove the canonical staging PDA is absent in addition to
/// validating the full byte hash and schema semantics. Thus the raw account's
/// PDA and apparent payload alone never assert finality.
pub fn authenticate_finalized_raw_record_v1<'content, A: RecordAdapterV1>(
    adapter: &A,
    key: RecordKeyV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
    exact_content: &'content [u8],
) -> Result<AuthenticatedRawRecordV1<'content>> {
    require_distinct(&[raw_record_account, staging_account])?;
    u64::try_from(exact_content.len()).map_err(|_| Error::InvalidLength)?;
    require_adapter_addresses(adapter, key, raw_record_account, staging_account)?;
    let validation = RawRecordValidationObligationV1 {
        mode: RawRecordValidationModeV1::ConsumerAuthentication,
        key,
        raw_record_account,
        staging_account,
        exact_content,
    };
    if !adapter.validate_raw_record(&validation) {
        return Err(Error::AdapterValidationRefused);
    }
    Ok(AuthenticatedRawRecordV1 {
        key,
        raw_record_account,
        exact_content,
    })
}

/// Move-only Abort transition closing both staging and raw accounts to sponsor.
#[derive(Debug, Eq, PartialEq)]
pub struct AbortTransitionV1 {
    sponsor_signature_required: bool,
    raw_record_close: AccountCloseV1,
    staging_close: StagingLamportCloseV1,
}

impl AbortTransitionV1 {
    /// Return the signer the SVM adapter must authenticate.
    pub const fn sponsor_signature_required(&self) -> bool {
        self.sponsor_signature_required
    }

    /// Return the raw-account full-rent refund obligation.
    pub const fn raw_record_close(&self) -> AccountCloseV1 {
        self.raw_record_close
    }

    /// Return the cursor-account full-rent refund obligation.
    pub const fn staging_close(&self) -> StagingLamportCloseV1 {
        self.staging_close
    }
}

/// Exact live-account and clock observation used to preflight one Abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbortObservationV1 {
    raw_record_account: AccountId,
    staging_account: AccountId,
    raw_data_length: u64,
    raw_lamports: u64,
    staging_lamports: u64,
    current_slot: u64,
    abort_actor: AccountId,
}

impl AbortObservationV1 {
    /// Construct the complete SVM observation required by Abort.
    pub const fn new(
        raw_record_account: AccountId,
        staging_account: AccountId,
        raw_data_length: u64,
        raw_lamports: u64,
        staging_lamports: u64,
        current_slot: u64,
        abort_actor: AccountId,
    ) -> Self {
        Self {
            raw_record_account,
            staging_account,
            raw_data_length,
            raw_lamports,
            staging_lamports,
            current_slot,
            abort_actor,
        }
    }

    /// Return the observed raw-record account.
    pub const fn raw_record_account(self) -> AccountId {
        self.raw_record_account
    }

    /// Return the observed staging account.
    pub const fn staging_account(self) -> AccountId {
        self.staging_account
    }

    /// Return the observed raw data length.
    pub const fn raw_data_length(self) -> u64 {
        self.raw_data_length
    }

    /// Return the complete observed raw lamport balance.
    pub const fn raw_lamports(self) -> u64 {
        self.raw_lamports
    }

    /// Return the complete observed staging lamport balance.
    pub const fn staging_lamports(self) -> u64 {
        self.staging_lamports
    }

    /// Return the authenticated current slot.
    pub const fn current_slot(self) -> u64 {
        self.current_slot
    }

    /// Return the sponsor signer or expired-cleanup recipient.
    pub const fn abort_actor(self) -> AccountId {
        self.abort_actor
    }
}

/// Prepare Abort from a live cursor; no finalized-record state is accepted.
pub fn prepare_abort_v1(
    cursor: StagingCursorV1,
    observation: AbortObservationV1,
) -> Result<AbortTransitionV1> {
    require_cursor_accounts(
        cursor,
        observation.raw_record_account,
        observation.staging_account,
        observation.raw_data_length,
    )?;
    require_distinct(&[
        observation.raw_record_account,
        observation.staging_account,
        observation.abort_actor,
    ])?;
    let expired = observation.current_slot >= cursor.expiry_slot;
    if !expired && observation.abort_actor != cursor.sponsor_rent_refund {
        return Err(Error::AbortBeforeExpiry);
    }
    let cleanup_bounty_lamports = if expired {
        cursor.cleanup_bounty_lamports
    } else {
        0
    };
    let sponsor_refund_lamports = observation
        .staging_lamports
        .checked_sub(cleanup_bounty_lamports)
        .ok_or(Error::InsufficientCleanupBounty)?;
    let staging_close = StagingLamportCloseV1 {
        account: observation.staging_account,
        cleanup_recipient: observation.abort_actor,
        cleanup_bounty_lamports,
        sponsor_recipient: cursor.sponsor_rent_refund,
        sponsor_refund_lamports,
        observed_lamports: observation.staging_lamports,
    };
    staging_close.validate_conservation()?;
    Ok(AbortTransitionV1 {
        sponsor_signature_required: !expired,
        raw_record_close: AccountCloseV1 {
            account: observation.raw_record_account,
            full_lamport_refund: cursor.sponsor_rent_refund,
            observed_lamports: observation.raw_lamports,
        },
        staging_close,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum RecordActionV1 {
    Begin = 1,
    AppendPage = 2,
    Finalize = 3,
    Abort = 4,
}

impl RecordActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Begin),
            2 => Ok(Self::AppendPage),
            3 => Ok(Self::Finalize),
            4 => Ok(Self::Abort),
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

fn require_adapter_addresses<A: RecordAdapterV1>(
    adapter: &A,
    key: RecordKeyV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
) -> Result<()> {
    let obligation = AddressDerivationObligationV1 {
        key,
        raw_record_account,
        staging_account,
    };
    if !adapter.validate_canonical_addresses(&obligation) {
        return Err(Error::AddressDerivationRefused);
    }
    Ok(())
}

fn require_cursor_accounts(
    cursor: StagingCursorV1,
    raw_record_account: AccountId,
    staging_account: AccountId,
    observed_raw_data_length: u64,
) -> Result<()> {
    if cursor.status != StagingStatusV1::Building
        || raw_record_account != cursor.raw_record_account
        || staging_account != cursor.staging_account
        || observed_raw_data_length != cursor.exact_length
    {
        return Err(Error::CursorBindingMismatch);
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountId]) -> Result<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        while right < accounts.len() {
            if accounts.get(left) == accounts.get(right) {
                return Err(Error::AccountAlias);
            }
            right = right.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        left = left.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn offset_after_pages(envelope: PageEnvelopeV1, exact_length: u64, pages: u64) -> Result<u64> {
    let unbounded = pages
        .checked_mul(u64::from(envelope.page_bytes))
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(core::cmp::min(unbounded, exact_length))
}

fn instruction_header(bytes: &[u8], exact: usize, action: RecordActionV1) -> Result<()> {
    if bytes.len() != exact {
        return Err(Error::InvalidLength);
    }
    instruction_header_prefix(bytes, action)
}

fn instruction_header_prefix(bytes: &[u8], action: RecordActionV1) -> Result<()> {
    if bytes.len() < HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != RECORD_INSTRUCTION_MAGIC_V1 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != RECORD_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if RecordActionV1::decode(read_u8(bytes, 10)?)? != action {
        return Err(Error::UnknownAction);
    }
    Ok(())
}

fn write_instruction_header(output: &mut [u8], action: RecordActionV1) {
    put(output, 0, &RECORD_INSTRUCTION_MAGIC_V1);
    put(output, 8, &RECORD_SCHEMA_VERSION_V1.to_le_bytes());
    if let Some(destination) = output.get_mut(10) {
        *destination = action.byte();
    }
}

fn require_nonzero(bytes: &[u8; ID_BYTES]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentity);
    }
    Ok(())
}

fn zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    if slice(bytes, offset, length)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests;
