use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer};
use crate::{content_id, ContentId, DrawdownIntervalV3, Error, FixedCodec, Result};

const SOURCE_PLANE_MAGIC: [u8; 8] = *b"DCSPV3\0\0";
const HEAD_MAGIC: [u8; 8] = *b"DCHEADV3";
const OPEN_PAGE_MAGIC: [u8; 8] = *b"DCOPENV3";
const PAGE_MAGIC: [u8; 8] = *b"DCPAGEV3";
const WINDOW_MAGIC: [u8; 8] = *b"DCWINV3\0";
const WORK_MAGIC: [u8; 8] = *b"DCWWORK3";
const CLOSURE_MAGIC: [u8; 8] = *b"DCCLOSE3";
const SEAL_MAGIC: [u8; 8] = *b"DCSEALV3";
const SUMMARY_MAGIC: [u8; 8] = *b"DCSUMV3\0";
const STATISTIC_KEY_MAGIC: [u8; 8] = *b"DCSTKEY3";
const STATISTIC_RESULT_MAGIC: [u8; 8] = *b"DCSTRES3";

const SOURCE_PLANE_DOMAIN: &[u8] = b"dragons-clutch/source-plane-program/v3";
const SOURCE_HEAD_SNAPSHOT_DOMAIN: &[u8] = b"dragons-clutch/source-head-snapshot/v3";
const RAW_PAGE_DOMAIN: &[u8] = b"dragons-clutch/raw-page/v3";
const WINDOW_DOMAIN: &[u8] = b"dragons-clutch/window/v3";
const WINDOW_ROOT_INITIAL_DOMAIN: &[u8] = b"dragons-clutch/window-root-initial/v3";
const WINDOW_ROOT_STEP_DOMAIN: &[u8] = b"dragons-clutch/window-root-step/v3";
const WINDOW_CLOSURE_DOMAIN: &[u8] = b"dragons-clutch/window-closure/v3";
const WINDOW_SEAL_DOMAIN: &[u8] = b"dragons-clutch/window-seal/v3";
const SUMMARY_PROGRAM_DOMAIN: &[u8] = b"dragons-clutch/summary-program/v3";
const STATISTIC_KEY_DOMAIN: &[u8] = b"dragons-clutch/statistic-key/v3";
const STATISTIC_RESULT_DOMAIN: &[u8] = b"dragons-clutch/statistic-result-content/v3";

/// Exact registered SourcePlane generation implemented by this crate.
pub const SOURCE_PLANE_VERSION: u16 = 3;
/// Exact registered raw-page codec implemented by this crate.
pub const RAW_PAGE_CODEC_VERSION: u16 = 1;
/// Exact registered immutable-window codec implemented by this crate.
pub const WINDOW_CODEC_VERSION: u16 = 1;
/// Exact registered statistic-result codec implemented by this crate.
pub const STATISTIC_RESULT_CODEC_VERSION: u16 = 1;
/// Capability bit for a mutable head with no product facts.
pub const CAP_SOURCE_ONLY_HEAD: u32 = 1 << 0;
/// Capability bit for immutable pages shared by overlapping windows.
pub const CAP_REUSABLE_RAW_PAGES: u32 = 1 << 1;
/// Capability bit for a source identity with no collateral Realm.
pub const CAP_REALM_NEUTRAL_FEED: u32 = 1 << 2;
/// Capability bit for multiple derived results per raw window.
pub const CAP_STATISTIC_RESULTS: u32 = 1 << 3;
const REQUIRED_CAPABILITIES: u32 =
    CAP_SOURCE_ONLY_HEAD | CAP_REUSABLE_RAW_PAGES | CAP_REALM_NEUTRAL_FEED | CAP_STATISTIC_RESULTS;

/// Exact SourcePlane-program artifact width.
pub const SOURCE_PLANE_PROGRAM_BYTES: usize = 64;
/// Exact mutable source-only head width.
pub const SOURCE_HEAD_BYTES: usize = 160;
/// Exact dense boundary-record width.
pub const RAW_RECORD_BYTES: usize = 64;
/// Raw-page capacity retained from the current source archive generation.
pub const MAX_RAW_PAGE_RECORDS: usize = 32;
/// Exact mutable open-page work width.
pub const OPEN_RAW_PAGE_BYTES: usize = 2_208;
/// Exact immutable raw-page width.
pub const RAW_PAGE_BYTES: usize = 2_152;
/// Exact semantic window-key body width.
pub const WINDOW_SPEC_BYTES: usize = 120;
/// Exact resumable window-work width.
pub const WINDOW_WORK_BYTES: usize = 200;
/// Exact immutable closure-receipt width.
pub const WINDOW_CLOSURE_RECEIPT_BYTES: usize = 136;
/// Exact immutable WindowSeal width.
pub const WINDOW_SEAL_BYTES: usize = 192;
/// Exact summary-program artifact width.
pub const SUMMARY_PROGRAM_BYTES: usize = 56;
/// Exact predictable statistic-key width.
pub const STATISTIC_KEY_BYTES: usize = 80;
/// Exact statistic-result content width.
pub const STATISTIC_RESULT_BYTES: usize = 112;

/// Largest normalized value admitted by this core.
pub const MAX_SOURCE_VALUE: u128 = 1_000_000_000_000_000_000_000_000;
/// Complete-window coverage registry identity.
pub const COVERAGE_COMPLETE_REQUIRED: u16 = 1;
/// Reserved bounded-gap registry identity. V3 refuses it until the live source
/// adapter provides a non-forgeable authenticated-gap construction route.
pub const COVERAGE_BOUNDED_GAPS: u16 = 2;
/// Evaluator capability for a conservative terminal interval.
pub const FEATURE_TERMINAL_INTERVAL: u64 = 1 << 0;
/// Evaluator capability for conservative ordered maximum drawdown.
pub const FEATURE_DRAWDOWN_INTERVAL: u64 = 1 << 1;
const KNOWN_SUMMARY_FEATURES: u64 = FEATURE_TERMINAL_INTERVAL | FEATURE_DRAWDOWN_INTERVAL;

/// Reviewed SourcePlane release and exact closed codec contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePlaneProgramV3 {
    /// Reviewed evaluator-release binding selected by the Source release.
    pub release_id: ContentId,
    /// Exact SourcePlane schema version.
    pub source_plane_version: u16,
    /// Exact raw-page codec version.
    pub raw_page_codec_version: u16,
    /// Exact window codec version.
    pub window_codec_version: u16,
    /// Exact statistic-result codec version.
    pub statistic_result_codec_version: u16,
    /// Exact closed capability set.
    pub capabilities: u32,
}

impl SourcePlaneProgramV3 {
    /// Refuse both legacy and unknown future versions.
    pub fn validate(&self) -> Result<()> {
        self.release_id.validate()?;
        if self.source_plane_version != SOURCE_PLANE_VERSION
            || self.raw_page_codec_version != RAW_PAGE_CODEC_VERSION
            || self.window_codec_version != WINDOW_CODEC_VERSION
            || self.statistic_result_codec_version != STATISTIC_RESULT_CODEC_VERSION
            || self.capabilities != REQUIRED_CAPABILITIES
        {
            return Err(Error::BadVersion);
        }
        Ok(())
    }

    /// Content identity of this exact reviewed compatibility contract.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; SOURCE_PLANE_PROGRAM_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(SOURCE_PLANE_DOMAIN, &bytes))
    }
}

impl FixedCodec for SourcePlaneProgramV3 {
    const ENCODED_LEN: usize = SOURCE_PLANE_PROGRAM_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SOURCE_PLANE_MAGIC);
        writer.id(self.release_id);
        writer.u16(self.source_plane_version);
        writer.u16(self.raw_page_codec_version);
        writer.u16(self.window_codec_version);
        writer.u16(self.statistic_result_codec_version);
        writer.u32(self.capabilities);
        writer.reserved(12);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SOURCE_PLANE_MAGIC)?;
        let value = Self {
            release_id: reader.id(),
            source_plane_version: reader.u16(),
            raw_page_codec_version: reader.u16(),
            window_codec_version: reader.u16(),
            statistic_result_codec_version: reader.u16(),
            capabilities: reader.u32(),
        };
        reader.reserved(12)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Mutable source-only state. `source_spec_id` is the existing SourceSpec truth;
/// no second adapter/grid description is persisted here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceHeadV3 {
    /// Existing externally authenticated SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Latest immutable page, or zero before page zero.
    pub latest_page_id: ContentId,
    /// Next canonical boundary bucket admitted by a new page.
    pub next_boundary_bucket: u64,
    /// Number of immutable pages committed.
    pub page_count: u64,
    /// Exact source repair generation.
    pub repair_generation: u64,
    /// Last authenticated source-native sequence; not assumed consecutive.
    pub last_source_sequence: u64,
    /// Last authenticated publication slot.
    pub last_publish_slot: u64,
    /// Last authenticated publication time encoded by the source adapter.
    pub last_publish_time: u64,
    /// Digest of the last exact source body used for equal-sequence checks.
    pub last_record_body_digest: ContentId,
}

impl SourceHeadV3 {
    /// Construct an empty generation at an authenticated starting boundary.
    pub fn new(
        source_spec_id: ContentId,
        next_boundary_bucket: u64,
        repair_generation: u64,
    ) -> Result<Self> {
        let value = Self {
            source_spec_id,
            latest_page_id: ContentId::ZERO,
            next_boundary_bucket,
            page_count: 0,
            repair_generation,
            last_source_sequence: 0,
            last_publish_slot: 0,
            last_publish_time: 0,
            last_record_body_digest: ContentId::ZERO,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate genesis/live equivalence without interpreting source semantics.
    pub fn validate(&self) -> Result<()> {
        self.source_spec_id.validate()?;
        let empty = self.page_count == 0;
        if empty != self.latest_page_id.is_zero() || empty != self.last_record_body_digest.is_zero()
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Immutable digest of the exact authenticated head snapshot.
    pub fn snapshot_id(&self) -> Result<ContentId> {
        let mut bytes = [0; SOURCE_HEAD_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(SOURCE_HEAD_SNAPSHOT_DOMAIN, &bytes))
    }

    /// Begin the one-boundary-per-transaction ingestion route.
    pub fn open_page(&self) -> Result<OpenRawPageV3> {
        self.validate()?;
        let page = OpenRawPageV3 {
            source_spec_id: self.source_spec_id,
            repair_generation: self.repair_generation,
            page_index: self.page_count,
            start_bucket: self.next_boundary_bucket,
            record_count: 0,
            previous_page_id: self.latest_page_id,
            baseline_source_sequence: self.last_source_sequence,
            baseline_publish_slot: self.last_publish_slot,
            baseline_publish_time: self.last_publish_time,
            baseline_record_body_digest: self.last_record_body_digest,
            records: [RawRecordV3::PADDING; MAX_RAW_PAGE_RECORDS],
        };
        page.validate()?;
        Ok(page)
    }

    /// Atomically commit a sealed page that was opened from this exact head.
    pub fn commit_page(self, page: &RawPageV3) -> Result<Self> {
        self.validate()?;
        page.validate()?;
        if page.source_spec_id != self.source_spec_id
            || page.repair_generation != self.repair_generation
            || page.page_index != self.page_count
            || page.start_bucket != self.next_boundary_bucket
            || page.previous_page_id != self.latest_page_id
        {
            return Err(Error::DiscontinuousPage);
        }
        let first = page.records[0];
        if self.page_count > 0
            && (first.source_sequence < self.last_source_sequence
                || first.publish_slot < self.last_publish_slot
                || first.publish_time < self.last_publish_time
                || (first.source_sequence == self.last_source_sequence
                    && first.body_digest()? != self.last_record_body_digest))
        {
            return Err(Error::DiscontinuousPage);
        }
        let last = page.records[usize::from(page.record_count) - 1];
        let next = Self {
            source_spec_id: self.source_spec_id,
            latest_page_id: page.id()?,
            next_boundary_bucket: page.end_bucket_exclusive()?,
            page_count: self
                .page_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            repair_generation: self.repair_generation,
            last_source_sequence: last.source_sequence,
            last_publish_slot: last.publish_slot,
            last_publish_time: last.publish_time,
            last_record_body_digest: last.body_digest()?,
        };
        next.validate()?;
        Ok(next)
    }
}

impl FixedCodec for SourceHeadV3 {
    const ENCODED_LEN: usize = SOURCE_HEAD_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&HEAD_MAGIC);
        writer.id(self.source_spec_id);
        writer.id(self.latest_page_id);
        writer.u64(self.next_boundary_bucket);
        writer.u64(self.page_count);
        writer.u64(self.repair_generation);
        writer.u64(self.last_source_sequence);
        writer.u64(self.last_publish_slot);
        writer.u64(self.last_publish_time);
        writer.id(self.last_record_body_digest);
        writer.reserved(8);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&HEAD_MAGIC)?;
        let value = Self {
            source_spec_id: reader.id(),
            latest_page_id: reader.id(),
            next_boundary_bucket: reader.u64(),
            page_count: reader.u64(),
            repair_generation: reader.u64(),
            last_source_sequence: reader.u64(),
            last_publish_slot: reader.u64(),
            last_publish_time: reader.u64(),
            last_record_body_digest: reader.id(),
        };
        reader.reserved(8)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Raw boundary-record kind. Gap construction is intentionally not public;
/// the future authenticated source adapter must own that capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RawRecordKindV3 {
    /// Exact inactive fixed-array padding.
    Padding = 0,
    /// One authenticated conservative source interval.
    Observation = 1,
    /// One authenticated explicit absence, constructible only inside this crate.
    Gap = 2,
}

/// One 64-byte source record. Bucket is derived from page start plus slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawRecordV3 {
    kind: RawRecordKindV3,
    low: u128,
    high: u128,
    source_sequence: u64,
    publish_slot: u64,
    publish_time: u64,
}

impl RawRecordV3 {
    /// Exact inactive padding.
    pub const PADDING: Self = Self {
        kind: RawRecordKindV3::Padding,
        low: 0,
        high: 0,
        source_sequence: 0,
        publish_slot: 0,
        publish_time: 0,
    };

    /// Construct an adapter-authenticated observation projection.
    ///
    /// This function does not authenticate the source. The live adapter must
    /// create it only after its exact owner/PDA/release/source checks succeed.
    pub const fn observation(
        low: u128,
        high: u128,
        source_sequence: u64,
        publish_slot: u64,
        publish_time: u64,
    ) -> Self {
        Self {
            kind: RawRecordKindV3::Observation,
            low,
            high,
            source_sequence,
            publish_slot,
            publish_time,
        }
    }

    /// Record kind.
    pub const fn kind(self) -> RawRecordKindV3 {
        self.kind
    }

    /// Conservative low endpoint.
    pub const fn low(self) -> u128 {
        self.low
    }

    /// Conservative high endpoint.
    pub const fn high(self) -> u128 {
        self.high
    }

    /// Source-native sequence, which may repeat or jump.
    pub const fn source_sequence(self) -> u64 {
        self.source_sequence
    }

    /// Authenticated publication slot.
    pub const fn publish_slot(self) -> u64 {
        self.publish_slot
    }

    /// Authenticated publication time in the source adapter's exact encoding.
    pub const fn publish_time(self) -> u64 {
        self.publish_time
    }

    fn validate_active(self) -> Result<()> {
        match self.kind {
            RawRecordKindV3::Padding => Err(Error::NonCanonicalPadding),
            RawRecordKindV3::Observation => {
                if self.low > self.high || self.high > MAX_SOURCE_VALUE {
                    Err(Error::InvalidParameter)
                } else {
                    Ok(())
                }
            }
            RawRecordKindV3::Gap => {
                if self.low != 0
                    || self.high != 0
                    || self.source_sequence != 0
                    || self.publish_slot != 0
                    || self.publish_time != 0
                {
                    Err(Error::NonCanonicalPadding)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn is_padding(self) -> bool {
        self == Self::PADDING
    }

    fn body_digest(self) -> Result<ContentId> {
        self.validate_active()?;
        let mut bytes = [0; RAW_RECORD_BYTES];
        self.encode_exact(&mut bytes)?;
        Ok(content_id(b"dragons-clutch/source-record-body/v3", &bytes))
    }

    fn encode_exact(self, output: &mut [u8]) -> Result<()> {
        let mut writer = Writer::new(output, RAW_RECORD_BYTES)?;
        writer.u8(self.kind as u8);
        writer.reserved(7);
        writer.u128(self.low);
        writer.u128(self.high);
        writer.u64(self.source_sequence);
        writer.u64(self.publish_slot);
        writer.u64(self.publish_time);
        writer.finish()
    }

    fn decode_exact(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, RAW_RECORD_BYTES)?;
        let kind = match reader.u8() {
            0 => RawRecordKindV3::Padding,
            1 => RawRecordKindV3::Observation,
            2 => RawRecordKindV3::Gap,
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(7)?;
        let value = Self {
            kind,
            low: reader.u128(),
            high: reader.u128(),
            source_sequence: reader.u64(),
            publish_slot: reader.u64(),
            publish_time: reader.u64(),
        };
        reader.finish()?;
        if kind == RawRecordKindV3::Padding {
            if !value.is_padding() {
                return Err(Error::NonCanonicalPadding);
            }
        } else {
            value.validate_active()?;
        }
        Ok(value)
    }
}

/// Persistable open-page state for real one-boundary-per-transaction ingestion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRawPageV3 {
    /// Existing SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Exact source repair generation.
    pub repair_generation: u64,
    /// State-assigned page index.
    pub page_index: u64,
    /// Bucket derived for record slot zero.
    pub start_bucket: u64,
    /// Active leading record count.
    pub record_count: u8,
    /// Previous immutable page, zero only at page zero.
    pub previous_page_id: ContentId,
    baseline_source_sequence: u64,
    baseline_publish_slot: u64,
    baseline_publish_time: u64,
    baseline_record_body_digest: ContentId,
    /// Active observations followed by exact padding.
    pub records: [RawRecordV3; MAX_RAW_PAGE_RECORDS],
}

impl OpenRawPageV3 {
    /// Authenticate the immutable page prefix against the exact current head.
    ///
    /// Adapters must call this before every one-boundary mutation. Validating
    /// the open page alone proves internal shape, but only this join prevents a
    /// caller from replaying a well-formed page opened from another head
    /// snapshot, repair generation, page index, or predecessor.
    pub fn validate_against_head(&self, head: &SourceHeadV3) -> Result<()> {
        self.validate()?;
        head.validate()?;
        if self.source_spec_id != head.source_spec_id
            || self.repair_generation != head.repair_generation
            || self.page_index != head.page_count
            || self.start_bucket != head.next_boundary_bucket
            || self.previous_page_id != head.latest_page_id
            || self.baseline_source_sequence != head.last_source_sequence
            || self.baseline_publish_slot != head.last_publish_slot
            || self.baseline_publish_time != head.last_publish_time
            || self.baseline_record_body_digest != head.last_record_body_digest
        {
            return Err(Error::DiscontinuousPage);
        }
        Ok(())
    }

    /// Append one authenticated observation while preserving real V2 semantics:
    /// sequences may jump or repeat, but never regress; a repeated sequence must
    /// repeat the exact source body.
    pub fn append_observation(&self, record: RawRecordV3) -> Result<Self> {
        self.validate()?;
        record.validate_active()?;
        if record.kind != RawRecordKindV3::Observation
            || usize::from(self.record_count) >= MAX_RAW_PAGE_RECORDS
        {
            return Err(Error::InvalidParameter);
        }
        let (sequence, slot, time, prior_digest) = if self.record_count == 0 {
            (
                self.baseline_source_sequence,
                self.baseline_publish_slot,
                self.baseline_publish_time,
                self.baseline_record_body_digest,
            )
        } else {
            let prior = self.records[usize::from(self.record_count) - 1];
            (
                prior.source_sequence,
                prior.publish_slot,
                prior.publish_time,
                prior.body_digest()?,
            )
        };
        if record.source_sequence < sequence
            || record.publish_slot < slot
            || record.publish_time < time
        {
            return Err(Error::DiscontinuousPage);
        }
        if record.source_sequence == sequence && !prior_digest.is_zero() {
            let digest = record.body_digest()?;
            if digest != prior_digest {
                return Err(Error::DiscontinuousPage);
            }
        }
        let mut next = *self;
        next.records[usize::from(next.record_count)] = record;
        next.record_count = next
            .record_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        Ok(next)
    }

    /// Seal the current immutable prefix. Later records begin a new page and
    /// therefore cannot change this page or any overlapping WindowSeal.
    pub fn seal(self) -> Result<RawPageV3> {
        self.validate()?;
        if self.record_count == 0 {
            return Err(Error::InvalidParameter);
        }
        let page = RawPageV3 {
            source_spec_id: self.source_spec_id,
            repair_generation: self.repair_generation,
            page_index: self.page_index,
            start_bucket: self.start_bucket,
            record_count: self.record_count,
            previous_page_id: self.previous_page_id,
            records: self.records,
        };
        page.validate()?;
        Ok(page)
    }

    /// Validate origin snapshot, lineage, and padding.
    pub fn validate(&self) -> Result<()> {
        self.source_spec_id.validate()?;
        if self.page_index == 0 {
            if !self.previous_page_id.is_zero()
                || !self.baseline_record_body_digest.is_zero()
                || self.baseline_source_sequence != 0
                || self.baseline_publish_slot != 0
                || self.baseline_publish_time != 0
            {
                return Err(Error::DiscontinuousPage);
            }
        } else if self.previous_page_id.is_zero() || self.baseline_record_body_digest.is_zero() {
            return Err(Error::DiscontinuousPage);
        }
        if usize::from(self.record_count) > MAX_RAW_PAGE_RECORDS {
            return Err(Error::InvalidParameter);
        }
        self.start_bucket
            .checked_add(u64::from(self.record_count))
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0_usize;
        let mut prior_sequence = self.baseline_source_sequence;
        let mut prior_slot = self.baseline_publish_slot;
        let mut prior_time = self.baseline_publish_time;
        let mut prior_digest = self.baseline_record_body_digest;
        while index < MAX_RAW_PAGE_RECORDS {
            let record = self.records[index];
            if index < usize::from(self.record_count) {
                record.validate_active()?;
                if record.kind != RawRecordKindV3::Observation
                    || record.source_sequence < prior_sequence
                    || record.publish_slot < prior_slot
                    || record.publish_time < prior_time
                {
                    return Err(Error::DiscontinuousPage);
                }
                let digest = record.body_digest()?;
                if record.source_sequence == prior_sequence
                    && !prior_digest.is_zero()
                    && digest != prior_digest
                {
                    return Err(Error::DiscontinuousPage);
                }
                prior_sequence = record.source_sequence;
                prior_slot = record.publish_slot;
                prior_time = record.publish_time;
                prior_digest = digest;
            } else if !record.is_padding() {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }
}

impl FixedCodec for OpenRawPageV3 {
    const ENCODED_LEN: usize = OPEN_RAW_PAGE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        encode_page_body(
            &OPEN_PAGE_MAGIC,
            true,
            self.source_spec_id,
            self.repair_generation,
            self.page_index,
            self.start_bucket,
            self.record_count,
            self.previous_page_id,
            self.baseline_source_sequence,
            self.baseline_publish_slot,
            self.baseline_publish_time,
            self.baseline_record_body_digest,
            &self.records,
            output,
        )
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let body = decode_page_body(&OPEN_PAGE_MAGIC, true, input)?;
        let value = Self {
            source_spec_id: body.source_spec_id,
            repair_generation: body.repair_generation,
            page_index: body.page_index,
            start_bucket: body.start_bucket,
            record_count: body.record_count,
            previous_page_id: body.previous_page_id,
            baseline_source_sequence: body.baseline_source_sequence,
            baseline_publish_slot: body.baseline_publish_slot,
            baseline_publish_time: body.baseline_publish_time,
            baseline_record_body_digest: body.baseline_record_body_digest,
            records: body.records,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Immutable, content-addressed raw page reusable by overlapping windows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawPageV3 {
    /// Existing SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Exact repair generation.
    pub repair_generation: u64,
    /// State-assigned page index.
    pub page_index: u64,
    /// Bucket derived for record slot zero.
    pub start_bucket: u64,
    /// Active leading record count.
    pub record_count: u8,
    /// Previous immutable page, zero only at page zero.
    pub previous_page_id: ContentId,
    /// Active records followed by exact padding.
    pub records: [RawRecordV3; MAX_RAW_PAGE_RECORDS],
}

impl RawPageV3 {
    /// Validate immutable page geometry and padding.
    pub fn validate(&self) -> Result<()> {
        self.source_spec_id.validate()?;
        if self.record_count == 0 || usize::from(self.record_count) > MAX_RAW_PAGE_RECORDS {
            return Err(Error::InvalidParameter);
        }
        if (self.page_index == 0) != self.previous_page_id.is_zero() {
            return Err(Error::DiscontinuousPage);
        }
        self.end_bucket_exclusive()?;
        let mut index = 0;
        let mut previous: Option<RawRecordV3> = None;
        while index < MAX_RAW_PAGE_RECORDS {
            if index < usize::from(self.record_count) {
                let record = self.records[index];
                record.validate_active()?;
                if record.kind != RawRecordKindV3::Observation {
                    return Err(Error::UnsupportedPolicy);
                }
                if let Some(prior) = previous {
                    if record.source_sequence < prior.source_sequence
                        || record.publish_slot < prior.publish_slot
                        || record.publish_time < prior.publish_time
                        || (record.source_sequence == prior.source_sequence
                            && record.body_digest()? != prior.body_digest()?)
                    {
                        return Err(Error::DiscontinuousPage);
                    }
                }
                previous = Some(record);
            } else if !self.records[index].is_padding() {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    /// Exclusive page boundary derived without duplicating it in bytes.
    pub fn end_bucket_exclusive(&self) -> Result<u64> {
        self.start_bucket
            .checked_add(u64::from(self.record_count))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Content identity of the exact immutable page and predecessor.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; RAW_PAGE_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(RAW_PAGE_DOMAIN, &bytes))
    }
}

impl FixedCodec for RawPageV3 {
    const ENCODED_LEN: usize = RAW_PAGE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        encode_page_body(
            &PAGE_MAGIC,
            false,
            self.source_spec_id,
            self.repair_generation,
            self.page_index,
            self.start_bucket,
            self.record_count,
            self.previous_page_id,
            0,
            0,
            0,
            ContentId::ZERO,
            &self.records,
            output,
        )
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let body = decode_page_body(&PAGE_MAGIC, false, input)?;
        let value = Self {
            source_spec_id: body.source_spec_id,
            repair_generation: body.repair_generation,
            page_index: body.page_index,
            start_bucket: body.start_bucket,
            record_count: body.record_count,
            previous_page_id: body.previous_page_id,
            records: body.records,
        };
        value.validate()?;
        Ok(value)
    }
}

struct PageBody {
    source_spec_id: ContentId,
    repair_generation: u64,
    page_index: u64,
    start_bucket: u64,
    record_count: u8,
    previous_page_id: ContentId,
    baseline_source_sequence: u64,
    baseline_publish_slot: u64,
    baseline_publish_time: u64,
    baseline_record_body_digest: ContentId,
    records: [RawRecordV3; MAX_RAW_PAGE_RECORDS],
}

#[allow(clippy::too_many_arguments)]
fn encode_page_body(
    magic: &[u8; 8],
    include_baseline: bool,
    source_spec_id: ContentId,
    repair_generation: u64,
    page_index: u64,
    start_bucket: u64,
    record_count: u8,
    previous_page_id: ContentId,
    baseline_source_sequence: u64,
    baseline_publish_slot: u64,
    baseline_publish_time: u64,
    baseline_record_body_digest: ContentId,
    records: &[RawRecordV3; MAX_RAW_PAGE_RECORDS],
    output: &mut [u8],
) -> Result<()> {
    let expected = if include_baseline {
        OPEN_RAW_PAGE_BYTES
    } else {
        RAW_PAGE_BYTES
    };
    let mut writer = Writer::new(output, expected)?;
    writer.bytes(magic);
    writer.id(source_spec_id);
    writer.u64(repair_generation);
    writer.u64(page_index);
    writer.u64(start_bucket);
    writer.u8(record_count);
    writer.reserved(7);
    writer.id(previous_page_id);
    if include_baseline {
        writer.u64(baseline_source_sequence);
        writer.u64(baseline_publish_slot);
        writer.u64(baseline_publish_time);
        writer.id(baseline_record_body_digest);
    }
    let mut index = 0;
    while index < MAX_RAW_PAGE_RECORDS {
        let mut record = [0; RAW_RECORD_BYTES];
        records[index].encode_exact(&mut record)?;
        writer.bytes(&record);
        index += 1;
    }
    writer.finish()
}

fn decode_page_body(magic: &[u8; 8], include_baseline: bool, input: &[u8]) -> Result<PageBody> {
    let expected = if include_baseline {
        OPEN_RAW_PAGE_BYTES
    } else {
        RAW_PAGE_BYTES
    };
    let mut reader = Reader::new(input, expected)?;
    reader.magic(magic)?;
    let source_spec_id = reader.id();
    let repair_generation = reader.u64();
    let page_index = reader.u64();
    let start_bucket = reader.u64();
    let record_count = reader.u8();
    reader.reserved(7)?;
    let previous_page_id = reader.id();
    let (
        baseline_source_sequence,
        baseline_publish_slot,
        baseline_publish_time,
        baseline_record_body_digest,
    ) = if include_baseline {
        (reader.u64(), reader.u64(), reader.u64(), reader.id())
    } else {
        (0, 0, 0, ContentId::ZERO)
    };
    let mut records = [RawRecordV3::PADDING; MAX_RAW_PAGE_RECORDS];
    let mut index = 0;
    while index < MAX_RAW_PAGE_RECORDS {
        let record = reader.bytes::<RAW_RECORD_BYTES>();
        records[index] = RawRecordV3::decode_exact(&record)?;
        index += 1;
    }
    reader.finish()?;
    Ok(PageBody {
        source_spec_id,
        repair_generation,
        page_index,
        start_bucket,
        record_count,
        previous_page_id,
        baseline_source_sequence,
        baseline_publish_slot,
        baseline_publish_time,
        baseline_record_body_digest,
        records,
    })
}

/// Semantic, content-addressed raw window independent of Realm and evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowSpecV3 {
    /// Existing SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Exact reviewed SourcePlane program contract.
    pub source_plane_program_id: ContentId,
    /// Inclusive first bucket.
    pub start_bucket: u64,
    /// Exclusive observation end.
    pub end_bucket_exclusive: u64,
    /// Exact maturity boundary.
    pub maturity_bucket_exclusive: u64,
    /// Exact repair generation selected by immutable Template semantics.
    pub repair_generation: u64,
    /// Closed coverage policy.
    pub coverage_policy_id: u16,
    /// Exact coverage parameter.
    pub coverage_policy_parameter: u64,
}

impl WindowSpecV3 {
    /// Validate range and closed coverage registry.
    pub fn validate(&self) -> Result<()> {
        self.source_spec_id.validate()?;
        self.source_plane_program_id.validate()?;
        if self.start_bucket >= self.end_bucket_exclusive
            || self.end_bucket_exclusive > self.maturity_bucket_exclusive
        {
            return Err(Error::InvalidParameter);
        }
        match self.coverage_policy_id {
            COVERAGE_COMPLETE_REQUIRED if self.coverage_policy_parameter == 0 => Ok(()),
            COVERAGE_BOUNDED_GAPS => Err(Error::UnsupportedPolicy),
            COVERAGE_COMPLETE_REQUIRED => Err(Error::InvalidParameter),
            _ => Err(Error::UnsupportedPolicy),
        }
    }

    /// Predictable semantic WindowKey, independent of page packing and results.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; WINDOW_SPEC_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(WINDOW_DOMAIN, &bytes))
    }
}

impl FixedCodec for WindowSpecV3 {
    const ENCODED_LEN: usize = WINDOW_SPEC_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&WINDOW_MAGIC);
        writer.id(self.source_spec_id);
        writer.id(self.source_plane_program_id);
        writer.u64(self.start_bucket);
        writer.u64(self.end_bucket_exclusive);
        writer.u64(self.maturity_bucket_exclusive);
        writer.u64(self.repair_generation);
        writer.u16(self.coverage_policy_id);
        writer.reserved(6);
        writer.u64(self.coverage_policy_parameter);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&WINDOW_MAGIC)?;
        let value = Self {
            source_spec_id: reader.id(),
            source_plane_program_id: reader.id(),
            start_bucket: reader.u64(),
            end_bucket_exclusive: reader.u64(),
            maturity_bucket_exclusive: reader.u64(),
            repair_generation: reader.u64(),
            coverage_policy_id: reader.u16(),
            coverage_policy_parameter: {
                reader.reserved(6)?;
                reader.u64()
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Persistable rolling window fold; no opaque SHA implementation state appears.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowWorkV3 {
    window_id: ContentId,
    next_window_bucket: u64,
    first_page_id: ContentId,
    last_page_id: ContentId,
    last_page_index: u64,
    last_page_end_bucket: u64,
    accepted_count: u32,
    gap_count: u32,
    evidence_page_count: u32,
    record_stream_root: ContentId,
    has_page: bool,
    mature: bool,
}

impl WindowWorkV3 {
    /// Start the canonical rolling root from one exact WindowKey.
    pub fn new(window: &WindowSpecV3) -> Result<Self> {
        window.validate()?;
        let window_id = window.id()?;
        Ok(Self {
            window_id,
            next_window_bucket: window.start_bucket,
            first_page_id: ContentId::ZERO,
            last_page_id: ContentId::ZERO,
            last_page_index: 0,
            last_page_end_bucket: 0,
            accepted_count: 0,
            gap_count: 0,
            evidence_page_count: 0,
            record_stream_root: content_id(WINDOW_ROOT_INITIAL_DOMAIN, &window_id.bytes()),
            has_page: false,
            mature: false,
        })
    }

    /// Stage one immutable page. Refusals leave the original work byte-identical.
    pub fn push_page(self, window: &WindowSpecV3, page: &RawPageV3) -> Result<Self> {
        self.validate_against(window)?;
        page.validate()?;
        if self.mature {
            return Err(Error::WindowAlreadyMature);
        }
        if page.source_spec_id != window.source_spec_id
            || page.repair_generation != window.repair_generation
        {
            return Err(Error::MismatchedArtifact);
        }
        let page_end = page.end_bucket_exclusive()?;
        if !self.has_page {
            if page.start_bucket > window.start_bucket || page_end <= window.start_bucket {
                return Err(Error::IncompleteWindow);
            }
        } else if page.previous_page_id != self.last_page_id
            || page.page_index
                != self
                    .last_page_index
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || page.start_bucket != self.last_page_end_bucket
        {
            return Err(Error::DiscontinuousPage);
        }
        let page_id = page.id()?;
        let mut next = self;
        if !next.has_page {
            next.first_page_id = page_id;
        }
        next.last_page_id = page_id;
        next.last_page_index = page.page_index;
        next.last_page_end_bucket = page_end;
        next.evidence_page_count = next
            .evidence_page_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.has_page = true;
        let mut index = 0_usize;
        while index < usize::from(page.record_count) {
            let bucket = page
                .start_bucket
                .checked_add(u64::try_from(index).map_err(|_| Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if bucket >= window.start_bucket && bucket < window.end_bucket_exclusive {
                if bucket != next.next_window_bucket {
                    return Err(Error::IncompleteWindow);
                }
                let record = page.records[index];
                let mut record_bytes = [0; RAW_RECORD_BYTES];
                record.encode_exact(&mut record_bytes)?;
                let mut hasher = Sha256::new();
                hasher.update(WINDOW_ROOT_STEP_DOMAIN);
                hasher.update(next.record_stream_root.bytes());
                hasher.update(bucket.to_le_bytes());
                hasher.update(record_bytes);
                next.record_stream_root = ContentId::from_bytes(hasher.finalize().into());
                match record.kind {
                    RawRecordKindV3::Observation => {
                        next.accepted_count = next
                            .accepted_count
                            .checked_add(1)
                            .ok_or(Error::ArithmeticOverflow)?;
                    }
                    RawRecordKindV3::Gap => {
                        next.gap_count = next
                            .gap_count
                            .checked_add(1)
                            .ok_or(Error::ArithmeticOverflow)?;
                    }
                    RawRecordKindV3::Padding => return Err(Error::NonCanonicalPadding),
                }
                next.next_window_bucket = next
                    .next_window_bucket
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        next.mature = page_end >= window.maturity_bucket_exclusive;
        next.validate_against(window)?;
        Ok(next)
    }

    /// Finish with a closure receipt derived from this exact maturity page.
    pub fn finish(
        self,
        window: &WindowSpecV3,
        closure: &WindowClosureReceiptV3,
    ) -> Result<WindowSealV3> {
        self.validate_against(window)?;
        closure.validate_against(window, self.last_page_id, self.last_page_end_bucket)?;
        if !self.has_page || !self.mature || self.next_window_bucket != window.end_bucket_exclusive
        {
            return Err(Error::IncompleteWindow);
        }
        let seal = WindowSealV3 {
            window_id: self.window_id,
            first_page_id: self.first_page_id,
            last_page_id: self.last_page_id,
            record_stream_root: self.record_stream_root,
            closure_receipt_id: closure.id()?,
            sealed_boundary_bucket: closure.sealed_boundary_bucket,
            accepted_count: self.accepted_count,
            gap_count: self.gap_count,
            evidence_page_count: self.evidence_page_count,
        };
        seal.validate_against(window)?;
        Ok(seal)
    }

    /// Validate resumable work against its immutable WindowKey.
    pub fn validate_against(&self, window: &WindowSpecV3) -> Result<()> {
        window.validate()?;
        self.validate_shape()?;
        if self.window_id != window.id()?
            || self.next_window_bucket < window.start_bucket
            || self.next_window_bucket > window.end_bucket_exclusive
            || (self.has_page != (self.evidence_page_count > 0))
            || (self.has_page == self.first_page_id.is_zero())
            || (self.has_page == self.last_page_id.is_zero())
        {
            return Err(Error::MismatchedArtifact);
        }
        if self.mature
            != (self.has_page && self.last_page_end_bucket >= window.maturity_bucket_exclusive)
        {
            return Err(Error::MismatchedArtifact);
        }
        let total = self
            .accepted_count
            .checked_add(self.gap_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let processed = self
            .next_window_bucket
            .checked_sub(window.start_bucket)
            .ok_or(Error::ArithmeticOverflow)?;
        if u64::from(total) != processed {
            return Err(Error::IncompleteWindow);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<()> {
        self.window_id.validate()?;
        self.record_stream_root.validate()?;
        let has_pages = self.evidence_page_count > 0;
        if self.has_page != has_pages
            || self.has_page == self.first_page_id.is_zero()
            || self.has_page == self.last_page_id.is_zero()
            || (!self.has_page
                && (self.mature
                    || self.last_page_index != 0
                    || self.last_page_end_bucket != 0
                    || self.accepted_count != 0
                    || self.gap_count != 0))
        {
            return Err(Error::InvalidParameter);
        }
        self.accepted_count
            .checked_add(self.gap_count)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

impl FixedCodec for WindowWorkV3 {
    const ENCODED_LEN: usize = WINDOW_WORK_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&WORK_MAGIC);
        writer.id(self.window_id);
        writer.u64(self.next_window_bucket);
        writer.id(self.first_page_id);
        writer.id(self.last_page_id);
        writer.u64(self.last_page_index);
        writer.u64(self.last_page_end_bucket);
        writer.u32(self.accepted_count);
        writer.u32(self.gap_count);
        writer.u32(self.evidence_page_count);
        writer.id(self.record_stream_root);
        writer.u8(u8::from(self.has_page));
        writer.u8(u8::from(self.mature));
        writer.reserved(26);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&WORK_MAGIC)?;
        let window_id = reader.id();
        let next_window_bucket = reader.u64();
        let first_page_id = reader.id();
        let last_page_id = reader.id();
        let last_page_index = reader.u64();
        let last_page_end_bucket = reader.u64();
        let accepted_count = reader.u32();
        let gap_count = reader.u32();
        let evidence_page_count = reader.u32();
        let record_stream_root = reader.id();
        let has_page = match reader.u8() {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidParameter),
        };
        let mature = match reader.u8() {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(26)?;
        reader.finish()?;
        let value = Self {
            window_id,
            next_window_bucket,
            first_page_id,
            last_page_id,
            last_page_index,
            last_page_end_bucket,
            accepted_count,
            gap_count,
            evidence_page_count,
            record_stream_root,
            has_page,
            mature,
        };
        value.validate_shape()?;
        Ok(value)
    }
}

/// Immutable proof that one exact page reached a window's maturity boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowClosureReceiptV3 {
    /// Exact reviewed SourcePlane program.
    pub source_plane_program_id: ContentId,
    /// Existing SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Maturity page identity.
    pub maturity_page_id: ContentId,
    /// Page-exclusive boundary authenticated by page construction.
    pub sealed_boundary_bucket: u64,
    /// Exact source repair generation.
    pub repair_generation: u64,
}

impl WindowClosureReceiptV3 {
    /// Derive a deterministic receipt from an immutable maturity page.
    pub fn from_page(
        source_plane: &SourcePlaneProgramV3,
        window: &WindowSpecV3,
        page: &RawPageV3,
    ) -> Result<Self> {
        source_plane.validate()?;
        window.validate()?;
        page.validate()?;
        let receipt = Self {
            source_plane_program_id: source_plane.id()?,
            source_spec_id: page.source_spec_id,
            maturity_page_id: page.id()?,
            sealed_boundary_bucket: page.end_bucket_exclusive()?,
            repair_generation: page.repair_generation,
        };
        receipt.validate_against(window, page.id()?, page.end_bucket_exclusive()?)?;
        Ok(receipt)
    }

    /// Validate the exact release/source/generation/maturity join.
    pub fn validate_against(
        &self,
        window: &WindowSpecV3,
        last_page_id: ContentId,
        last_page_end: u64,
    ) -> Result<()> {
        window.validate()?;
        self.source_plane_program_id.validate()?;
        self.source_spec_id.validate()?;
        self.maturity_page_id.validate()?;
        if self.source_plane_program_id != window.source_plane_program_id
            || self.source_spec_id != window.source_spec_id
            || self.maturity_page_id != last_page_id
            || self.sealed_boundary_bucket != last_page_end
            || self.sealed_boundary_bucket < window.maturity_bucket_exclusive
            || self.repair_generation != window.repair_generation
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Content identity of exact closure evidence.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; WINDOW_CLOSURE_RECEIPT_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(WINDOW_CLOSURE_DOMAIN, &bytes))
    }
}

impl FixedCodec for WindowClosureReceiptV3 {
    const ENCODED_LEN: usize = WINDOW_CLOSURE_RECEIPT_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.source_plane_program_id.validate()?;
        self.source_spec_id.validate()?;
        self.maturity_page_id.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&CLOSURE_MAGIC);
        writer.id(self.source_plane_program_id);
        writer.id(self.source_spec_id);
        writer.id(self.maturity_page_id);
        writer.u64(self.sealed_boundary_bucket);
        writer.u64(self.repair_generation);
        writer.reserved(16);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&CLOSURE_MAGIC)?;
        let value = Self {
            source_plane_program_id: reader.id(),
            source_spec_id: reader.id(),
            maturity_page_id: reader.id(),
            sealed_boundary_bucket: reader.u64(),
            repair_generation: reader.u64(),
        };
        reader.reserved(16)?;
        reader.finish()?;
        value.source_plane_program_id.validate()?;
        value.source_spec_id.validate()?;
        value.maturity_page_id.validate()?;
        Ok(value)
    }
}

/// Final evidence content for one predictable semantic WindowKey.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowSealV3 {
    /// Predictable semantic WindowKey.
    pub window_id: ContentId,
    /// First reusable page inspected.
    pub first_page_id: ContentId,
    /// Exact maturity page.
    pub last_page_id: ContentId,
    /// Page-packing-independent rolling root of in-window records.
    pub record_stream_root: ContentId,
    /// Exact immutable closure-receipt content.
    pub closure_receipt_id: ContentId,
    /// Authenticated maturity boundary.
    pub sealed_boundary_bucket: u64,
    /// Accepted observations in the window.
    pub accepted_count: u32,
    /// Authenticated explicit gaps in the window.
    pub gap_count: u32,
    /// Reusable pages inspected through maturity.
    pub evidence_page_count: u32,
}

impl WindowSealV3 {
    /// Validate counts and coverage against immutable Window semantics.
    pub fn validate_against(&self, window: &WindowSpecV3) -> Result<()> {
        window.validate()?;
        self.window_id.validate()?;
        self.first_page_id.validate()?;
        self.last_page_id.validate()?;
        self.record_stream_root.validate()?;
        self.closure_receipt_id.validate()?;
        if self.window_id != window.id()?
            || self.sealed_boundary_bucket < window.maturity_bucket_exclusive
            || self.evidence_page_count == 0
        {
            return Err(Error::MismatchedArtifact);
        }
        let total = self
            .accepted_count
            .checked_add(self.gap_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let span = window.end_bucket_exclusive - window.start_bucket;
        if u64::from(total) != span {
            return Err(Error::IncompleteWindow);
        }
        match window.coverage_policy_id {
            COVERAGE_COMPLETE_REQUIRED if self.gap_count == 0 => Ok(()),
            COVERAGE_BOUNDED_GAPS => Err(Error::UnsupportedPolicy),
            COVERAGE_COMPLETE_REQUIRED => Err(Error::IncompleteWindow),
            _ => Err(Error::UnsupportedPolicy),
        }
    }

    /// Content identity of actual evidence, distinct from WindowKey.
    pub fn id(&self) -> Result<ContentId> {
        self.validate_shape()?;
        let mut bytes = [0; WINDOW_SEAL_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(WINDOW_SEAL_DOMAIN, &bytes))
    }

    fn validate_shape(&self) -> Result<()> {
        self.window_id.validate()?;
        self.first_page_id.validate()?;
        self.last_page_id.validate()?;
        self.record_stream_root.validate()?;
        self.closure_receipt_id.validate()?;
        if self.evidence_page_count == 0 {
            return Err(Error::InvalidParameter);
        }
        self.accepted_count
            .checked_add(self.gap_count)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

impl FixedCodec for WindowSealV3 {
    const ENCODED_LEN: usize = WINDOW_SEAL_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SEAL_MAGIC);
        writer.id(self.window_id);
        writer.id(self.first_page_id);
        writer.id(self.last_page_id);
        writer.id(self.record_stream_root);
        writer.id(self.closure_receipt_id);
        writer.u64(self.sealed_boundary_bucket);
        writer.u32(self.accepted_count);
        writer.u32(self.gap_count);
        writer.u32(self.evidence_page_count);
        writer.reserved(4);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SEAL_MAGIC)?;
        let value = Self {
            window_id: reader.id(),
            first_page_id: reader.id(),
            last_page_id: reader.id(),
            record_stream_root: reader.id(),
            closure_receipt_id: reader.id(),
            sealed_boundary_bucket: reader.u64(),
            accepted_count: reader.u32(),
            gap_count: reader.u32(),
            evidence_page_count: reader.u32(),
        };
        reader.reserved(4)?;
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Closed statistic registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum StatisticKindV3 {
    /// Conservative terminal interval.
    TerminalInterval = 1,
    /// Ordered maximum-drawdown interval in integer ppm.
    MaximumDrawdownInterval = 2,
}

impl StatisticKindV3 {
    pub(crate) const fn feature(self) -> u64 {
        match self {
            Self::TerminalInterval => FEATURE_TERMINAL_INTERVAL,
            Self::MaximumDrawdownInterval => FEATURE_DRAWDOWN_INTERVAL,
        }
    }

    fn decode(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::TerminalInterval),
            2 => Ok(Self::MaximumDrawdownInterval),
            _ => Err(Error::UnsupportedStatistic),
        }
    }
}

/// Reviewed source-neutral evaluator implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummaryProgramV3 {
    /// Reviewed evaluator release digest.
    pub evaluator_release_id: ContentId,
    /// Exact evaluator semantic version.
    pub evaluator_version: u32,
    /// Closed implemented feature set.
    pub feature_mask: u64,
}

impl SummaryProgramV3 {
    /// Validate closed evaluator identity and features.
    pub fn validate(&self) -> Result<()> {
        self.evaluator_release_id.validate()?;
        if self.evaluator_version == 0
            || self.feature_mask == 0
            || self.feature_mask & !KNOWN_SUMMARY_FEATURES != 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Whether the reviewed evaluator implements this statistic.
    pub fn supports(&self, statistic: StatisticKindV3) -> bool {
        self.feature_mask & statistic.feature() != 0
    }

    /// Content identity of exact evaluator semantics.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; SUMMARY_PROGRAM_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(SUMMARY_PROGRAM_DOMAIN, &bytes))
    }
}

impl FixedCodec for SummaryProgramV3 {
    const ENCODED_LEN: usize = SUMMARY_PROGRAM_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SUMMARY_MAGIC);
        writer.id(self.evaluator_release_id);
        writer.u32(self.evaluator_version);
        writer.reserved(4);
        writer.u64(self.feature_mask);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SUMMARY_MAGIC)?;
        let value = Self {
            evaluator_release_id: reader.id(),
            evaluator_version: reader.u32(),
            feature_mask: {
                reader.reserved(4)?;
                reader.u64()
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Predictable evaluator request, not a result-content digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticKeyV3 {
    /// Predictable semantic WindowKey.
    pub window_id: ContentId,
    /// Exact SummaryProgram identity.
    pub summary_program_id: ContentId,
    /// Closed statistic.
    pub statistic: StatisticKindV3,
}

impl StatisticKeyV3 {
    /// Validate required content references.
    pub fn validate(&self) -> Result<()> {
        self.window_id.validate()?;
        self.summary_program_id.validate()?;
        Ok(())
    }

    /// Predictable key computable before evidence or result value exists.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; STATISTIC_KEY_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(STATISTIC_KEY_DOMAIN, &bytes))
    }
}

impl FixedCodec for StatisticKeyV3 {
    const ENCODED_LEN: usize = STATISTIC_KEY_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&STATISTIC_KEY_MAGIC);
        writer.id(self.window_id);
        writer.id(self.summary_program_id);
        writer.u16(self.statistic as u16);
        writer.reserved(6);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&STATISTIC_KEY_MAGIC)?;
        let value = Self {
            window_id: reader.id(),
            summary_program_id: reader.id(),
            statistic: StatisticKindV3::decode(reader.u16())?,
        };
        reader.reserved(6)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Statistic result state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StatisticResultStatusV3 {
    /// Evaluator produced a canonical value payload.
    Success = 1,
    /// Evaluator produced a stable nonzero refusal code and no value.
    Refused = 2,
}

/// Immutable result content with closed payload constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticResultV3 {
    statistic_key_id: ContentId,
    window_seal_id: ContentId,
    statistic: StatisticKindV3,
    status: StatisticResultStatusV3,
    refusal_code: u32,
    payload: [u8; 32],
}

impl StatisticResultV3 {
    /// Revalidate decoded result content against its exact evaluator request
    /// and final raw evidence. Account adapters must call this after hostile
    /// decoding; shape validation alone does not authenticate references.
    pub fn validate_against(
        &self,
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
    ) -> Result<()> {
        self.validate_shape()?;
        validate_result_inputs(key, summary, seal, window, self.statistic)?;
        if self.statistic_key_id != key.id()? || self.window_seal_id != seal.id()? {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Construct a successful terminal interval result.
    pub fn terminal(
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
        low: u128,
        high: u128,
    ) -> Result<Self> {
        validate_result_inputs(
            key,
            summary,
            seal,
            window,
            StatisticKindV3::TerminalInterval,
        )?;
        if low > high || high > MAX_SOURCE_VALUE {
            return Err(Error::InvalidParameter);
        }
        let mut payload = [0; 32];
        payload[..16].copy_from_slice(&low.to_le_bytes());
        payload[16..].copy_from_slice(&high.to_le_bytes());
        Ok(Self {
            statistic_key_id: key.id()?,
            window_seal_id: seal.id()?,
            statistic: key.statistic,
            status: StatisticResultStatusV3::Success,
            refusal_code: 0,
            payload,
        })
    }

    /// Construct a successful maximum-drawdown result.
    pub fn drawdown(
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
        interval: DrawdownIntervalV3,
    ) -> Result<Self> {
        validate_result_inputs(
            key,
            summary,
            seal,
            window,
            StatisticKindV3::MaximumDrawdownInterval,
        )?;
        if seal.gap_count != 0
            || interval.low_ppm > interval.high_ppm
            || interval.high_ppm > crate::DRAWDOWN_PPM_SCALE
        {
            return Err(Error::InvalidParameter);
        }
        let mut payload = [0; 32];
        payload[..8].copy_from_slice(&interval.low_ppm.to_le_bytes());
        payload[8..16].copy_from_slice(&interval.high_ppm.to_le_bytes());
        Ok(Self {
            statistic_key_id: key.id()?,
            window_seal_id: seal.id()?,
            statistic: key.statistic,
            status: StatisticResultStatusV3::Success,
            refusal_code: 0,
            payload,
        })
    }

    /// Construct a stable refused evaluation with exact zero value payload.
    pub fn refused(
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
        refusal_code: u32,
    ) -> Result<Self> {
        validate_result_inputs(key, summary, seal, window, key.statistic)?;
        if refusal_code == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            statistic_key_id: key.id()?,
            window_seal_id: seal.id()?,
            statistic: key.statistic,
            status: StatisticResultStatusV3::Refused,
            refusal_code,
            payload: [0; 32],
        })
    }

    /// Predictable request/key identity.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Final WindowSeal content used by evaluation.
    pub const fn window_seal_id(self) -> ContentId {
        self.window_seal_id
    }

    /// Result status.
    pub const fn status(self) -> StatisticResultStatusV3 {
        self.status
    }

    /// Stable refusal code, zero only for success.
    pub const fn refusal_code(self) -> u32 {
        self.refusal_code
    }

    /// Decode the terminal payload only under exact successful terminal status.
    pub fn terminal_interval(self) -> Result<(u128, u128)> {
        if self.statistic != StatisticKindV3::TerminalInterval
            || self.status != StatisticResultStatusV3::Success
        {
            return Err(Error::UnsupportedStatistic);
        }
        let mut low = [0; 16];
        let mut high = [0; 16];
        low.copy_from_slice(&self.payload[..16]);
        high.copy_from_slice(&self.payload[16..]);
        Ok((u128::from_le_bytes(low), u128::from_le_bytes(high)))
    }

    /// Decode the drawdown payload only under exact successful drawdown status.
    pub fn drawdown_interval(self) -> Result<DrawdownIntervalV3> {
        if self.statistic != StatisticKindV3::MaximumDrawdownInterval
            || self.status != StatisticResultStatusV3::Success
            || self.payload[16..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::UnsupportedStatistic);
        }
        let mut low = [0; 8];
        let mut high = [0; 8];
        low.copy_from_slice(&self.payload[..8]);
        high.copy_from_slice(&self.payload[8..16]);
        Ok(DrawdownIntervalV3 {
            low_ppm: u64::from_le_bytes(low),
            high_ppm: u64::from_le_bytes(high),
        })
    }

    /// Content identity that commits exact seal, status, and payload.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; STATISTIC_RESULT_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(STATISTIC_RESULT_DOMAIN, &bytes))
    }

    fn validate_shape(&self) -> Result<()> {
        self.statistic_key_id.validate()?;
        self.window_seal_id.validate()?;
        match self.status {
            StatisticResultStatusV3::Success => {
                if self.refusal_code != 0 {
                    return Err(Error::InvalidParameter);
                }
                match self.statistic {
                    StatisticKindV3::TerminalInterval => {
                        let (low, high) = self.terminal_interval()?;
                        if low > high || high > MAX_SOURCE_VALUE {
                            return Err(Error::InvalidParameter);
                        }
                    }
                    StatisticKindV3::MaximumDrawdownInterval => {
                        let interval = self.drawdown_interval()?;
                        if interval.low_ppm > interval.high_ppm
                            || interval.high_ppm > crate::DRAWDOWN_PPM_SCALE
                        {
                            return Err(Error::InvalidParameter);
                        }
                    }
                }
            }
            StatisticResultStatusV3::Refused => {
                if self.refusal_code == 0 || self.payload.iter().any(|byte| *byte != 0) {
                    return Err(Error::InvalidParameter);
                }
            }
        }
        Ok(())
    }
}

fn validate_result_inputs(
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    seal: &WindowSealV3,
    window: &WindowSpecV3,
    expected: StatisticKindV3,
) -> Result<()> {
    key.validate()?;
    summary.validate()?;
    seal.validate_against(window)?;
    if key.window_id != window.id()?
        || key.summary_program_id != summary.id()?
        || key.statistic != expected
        || !summary.supports(expected)
    {
        return Err(Error::MismatchedArtifact);
    }
    Ok(())
}

impl FixedCodec for StatisticResultV3 {
    const ENCODED_LEN: usize = STATISTIC_RESULT_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&STATISTIC_RESULT_MAGIC);
        writer.id(self.statistic_key_id);
        writer.id(self.window_seal_id);
        writer.u16(self.statistic as u16);
        writer.u8(self.status as u8);
        writer.reserved(1);
        writer.u32(self.refusal_code);
        writer.bytes(&self.payload);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&STATISTIC_RESULT_MAGIC)?;
        let statistic_key_id = reader.id();
        let window_seal_id = reader.id();
        let statistic = StatisticKindV3::decode(reader.u16())?;
        let status = match reader.u8() {
            1 => StatisticResultStatusV3::Success,
            2 => StatisticResultStatusV3::Refused,
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(1)?;
        let value = Self {
            statistic_key_id,
            window_seal_id,
            statistic,
            status,
            refusal_code: reader.u32(),
            payload: reader.bytes(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}
