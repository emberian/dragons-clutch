//! Fixed-layout candidate and head records for sponsored Pyth push snapshots.
//!
//! Candidates are immutable, sponsor-funded snapshots. One mutable head per
//! Market generation selects the lexicographically greatest valid submitted
//! candidate by `(publish_time, posted_slot, update_digest)`. Admission closes
//! by the Market's own deadline, not by an assumption about upstream history.

use crate::resolution::{Error, Result};
use dclutch_sha256_adapter::digest;

/// Candidate PDA domain.
pub const SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch/sponsor-candidate/v1";
/// Head PDA domain.
pub const SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1: &[u8] = b"dclutch/sponsor-head/v1";
/// Terminal receipt PDA domain.
pub const SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch/sponsor-receipt/v1";
/// Evidence hash domain.
pub const SPONSORED_PUSH_EVIDENCE_DOMAIN_V1: &[u8] = b"dclutch/sponsor-evidence/v1";
/// Candidate wire magic.
pub const SPONSORED_PUSH_CANDIDATE_MAGIC_V1: [u8; 8] = *b"DCLTSPC1";
/// Head wire magic.
pub const SPONSORED_PUSH_HEAD_MAGIC_V1: [u8; 8] = *b"DCLTSPH1";
/// Terminal receipt wire magic.
pub const SPONSORED_PUSH_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTSPR1";
/// Shared wire version.
pub const SPONSORED_PUSH_VERSION_V1: u16 = 1;
/// Exact upstream `PriceUpdateV2` snapshot width.
pub const SPONSORED_PUSH_UPDATE_BYTES_V1: usize = 134;
/// Exact candidate record width.
pub const SPONSORED_PUSH_CANDIDATE_BYTES_V1: usize = 432;
/// Exact head record width.
pub const SPONSORED_PUSH_HEAD_BYTES_V1: usize = 336;
/// Exact terminal receipt width.
pub const SPONSORED_PUSH_RECEIPT_BYTES_V1: usize = 464;
/// Exact instruction width for every sponsored-push transition.
pub const SPONSORED_PUSH_INSTRUCTION_BYTES_V1: usize = 32;
/// Sponsored-push instruction magic.
pub const SPONSORED_PUSH_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTSPI1";
/// Capture account count.
pub const SPONSORED_PUSH_CAPTURE_ACCOUNT_COUNT_V1: usize = 30;
/// Settle account count.
pub const SPONSORED_PUSH_SETTLE_ACCOUNT_COUNT_V1: usize = 32;
/// Candidate-close account count.
pub const SPONSORED_PUSH_CLOSE_CANDIDATE_ACCOUNT_COUNT_V1: usize = 4;
/// Head-close account count.
pub const SPONSORED_PUSH_CLOSE_HEAD_ACCOUNT_COUNT_V1: usize = 4;
/// Head-vacant funded-failure account count.
pub const SPONSORED_PUSH_COMMIT_FAILURE_ACCOUNT_COUNT_V1: usize = 29;

const CANDIDATE_IDENTITIES_OFFSET: usize = 16;
const CANDIDATE_IDENTITY_COUNT: usize = 7;
const CANDIDATE_GENERATION_OFFSET: usize = 240;
const CANDIDATE_SNAPSHOT_SLOT_OFFSET: usize = 248;
const CANDIDATE_SNAPSHOT_TIME_OFFSET: usize = 256;
const CANDIDATE_PUBLISH_TIME_OFFSET: usize = 264;
const CANDIDATE_POSTED_SLOT_OFFSET: usize = 272;
const CANDIDATE_BUMP_OFFSET: usize = 280;
const CANDIDATE_UPDATE_OFFSET: usize = 288;

const HEAD_IDENTITIES_OFFSET: usize = 16;
const HEAD_IDENTITY_COUNT: usize = 9;
const HEAD_GENERATION_OFFSET: usize = 304;
const HEAD_PUBLISH_TIME_OFFSET: usize = 312;
const HEAD_POSTED_SLOT_OFFSET: usize = 320;
const HEAD_BUMP_OFFSET: usize = 328;

const RECEIPT_IDENTITY_COUNT: usize = 11;
const RECEIPT_IDENTITIES_OFFSET: usize = 16;
const RECEIPT_GENERATION_OFFSET: usize = 368;
const RECEIPT_TERMINAL_SEQUENCE_OFFSET: usize = 376;
const RECEIPT_SNAPSHOT_SLOT_OFFSET: usize = 384;
const RECEIPT_SNAPSHOT_TIME_OFFSET: usize = 392;
const RECEIPT_PUBLISH_TIME_OFFSET: usize = 400;
const RECEIPT_POSTED_SLOT_OFFSET: usize = 408;
const RECEIPT_CONSUMED_SLOT_OFFSET: usize = 416;
const RECEIPT_SELECTOR_OFFSET: usize = 424;
const RECEIPT_OUTCOME_COUNT_OFFSET: usize = 428;
const RECEIPT_RESULT_NUMERATOR_OFFSET: usize = 432;
const RECEIPT_RESULT_DENOMINATOR_OFFSET: usize = 448;
const RECEIPT_BUMP_OFFSET: usize = 456;

const _: () = assert!(SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1.len() <= 32);

/// Sponsored-push state-machine action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SponsoredPushActionV1 {
    /// Snapshot one currently admissible upstream account body and update the head.
    Capture = 1,
    /// Resolve from the post-deadline best valid submitted candidate.
    Settle = 2,
    /// Close one immutable candidate after the Source is terminal.
    CloseCandidate = 3,
    /// Close the mutable head after the Source is terminal.
    CloseHead = 4,
    /// Commit funded failure after the deadline only when the canonical head is vacant.
    CommitFailure = 5,
}

impl SponsoredPushActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Capture),
            2 => Ok(Self::Settle),
            3 => Ok(Self::CloseCandidate),
            4 => Ok(Self::CloseHead),
            5 => Ok(Self::CommitFailure),
            _ => Err(Error::UnknownAction),
        }
    }
}

/// Fixed optimistic coordinates for a sponsored-push transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredPushInstructionV1 {
    /// Exact action.
    pub action: SponsoredPushActionV1,
    /// Immutable Market generation.
    pub generation: u64,
    /// Positive terminal sequence for settle/failure; zero for other actions.
    pub terminal_sequence: u64,
}

impl SponsoredPushInstructionV1 {
    /// Validate action-specific coordinates.
    pub fn validate(self) -> Result<()> {
        if self.generation == 0 {
            return Err(Error::ZeroCoordinate);
        }
        let terminal = matches!(
            self.action,
            SponsoredPushActionV1::Settle | SponsoredPushActionV1::CommitFailure
        );
        if terminal != (self.terminal_sequence != 0) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }

    /// Encode exact canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; SPONSORED_PUSH_INSTRUCTION_BYTES_V1]> {
        self.validate()?;
        let mut out = [0_u8; SPONSORED_PUSH_INSTRUCTION_BYTES_V1];
        out[..8].copy_from_slice(&SPONSORED_PUSH_INSTRUCTION_MAGIC_V1);
        out[8..10].copy_from_slice(&SPONSORED_PUSH_VERSION_V1.to_le_bytes());
        out[10] = self.action as u8;
        put8(&mut out, 16, self.generation)?;
        put8(&mut out, 24, self.terminal_sequence)?;
        Ok(out)
    }

    /// Decode exact canonical bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPONSORED_PUSH_INSTRUCTION_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != SPONSORED_PUSH_INSTRUCTION_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SPONSORED_PUSH_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 11, 5)?;
        let value = Self {
            action: SponsoredPushActionV1::decode(byte(bytes, 10)?)?,
            generation: read_u64(bytes, 16)?,
            terminal_sequence: read_u64(bytes, 24)?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Immutable sponsored update snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredPushCandidateV1 {
    /// Core Market.
    pub market: [u8; 32],
    /// Resolution-owned Source state.
    pub source_state: [u8; 32],
    /// Source-selected `ProviderReleaseV1` identity.
    pub provider_release: [u8; 32],
    /// Exact sponsored release identity.
    pub sponsored_release: [u8; 32],
    /// Exact fixed upstream account.
    pub price_account: [u8; 32],
    /// Candidate sponsor and refund recipient.
    pub refund_recipient: [u8; 32],
    /// SHA-256 of `update_bytes`.
    pub update_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Clock slot at capture.
    pub snapshot_slot: u64,
    /// Clock Unix time at capture.
    pub snapshot_unix_seconds: i64,
    /// Provider publication time parsed at capture.
    pub publish_time: i64,
    /// Receiver posting slot parsed at capture.
    pub posted_slot: u64,
    /// Candidate PDA bump.
    pub bump: u8,
    /// Exact fully verified 134-byte account body.
    pub update_bytes: [u8; SPONSORED_PUSH_UPDATE_BYTES_V1],
}

impl SponsoredPushCandidateV1 {
    /// Validate all fixed-layout relations owned by the record codec.
    pub fn validate(self) -> Result<()> {
        for identity in [
            self.market,
            self.source_state,
            self.provider_release,
            self.sponsored_release,
            self.price_account,
            self.refund_recipient,
            self.update_digest,
        ] {
            if identity == [0; 32] {
                return Err(Error::ZeroCoordinate);
            }
        }
        if self.generation == 0
            || self.snapshot_slot == 0
            || self.snapshot_unix_seconds <= 0
            || self.publish_time <= 0
            || self.posted_slot == 0
            || digest(&self.update_bytes) != self.update_digest
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }

    /// Encode canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; SPONSORED_PUSH_CANDIDATE_BYTES_V1]> {
        self.validate()?;
        let mut out = [0_u8; SPONSORED_PUSH_CANDIDATE_BYTES_V1];
        out[..8].copy_from_slice(&SPONSORED_PUSH_CANDIDATE_MAGIC_V1);
        out[8..10].copy_from_slice(&SPONSORED_PUSH_VERSION_V1.to_le_bytes());
        for (index, value) in self.identities().iter().enumerate() {
            put32(&mut out, CANDIDATE_IDENTITIES_OFFSET + index * 32, value)?;
        }
        put8(&mut out, CANDIDATE_GENERATION_OFFSET, self.generation)?;
        put8(&mut out, CANDIDATE_SNAPSHOT_SLOT_OFFSET, self.snapshot_slot)?;
        put_i64(
            &mut out,
            CANDIDATE_SNAPSHOT_TIME_OFFSET,
            self.snapshot_unix_seconds,
        )?;
        put_i64(&mut out, CANDIDATE_PUBLISH_TIME_OFFSET, self.publish_time)?;
        put8(&mut out, CANDIDATE_POSTED_SLOT_OFFSET, self.posted_slot)?;
        out[CANDIDATE_BUMP_OFFSET] = self.bump;
        out[CANDIDATE_UPDATE_OFFSET..CANDIDATE_UPDATE_OFFSET + SPONSORED_PUSH_UPDATE_BYTES_V1]
            .copy_from_slice(&self.update_bytes);
        Ok(out)
    }

    /// Decode canonical bytes and recheck the full body digest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPONSORED_PUSH_CANDIDATE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != SPONSORED_PUSH_CANDIDATE_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SPONSORED_PUSH_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 281, 7)?;
        require_zero(bytes, 422, 10)?;
        let value = Self {
            market: array(bytes, 16)?,
            source_state: array(bytes, 48)?,
            provider_release: array(bytes, 80)?,
            sponsored_release: array(bytes, 112)?,
            price_account: array(bytes, 144)?,
            refund_recipient: array(bytes, 176)?,
            update_digest: array(bytes, 208)?,
            generation: read_u64(bytes, CANDIDATE_GENERATION_OFFSET)?,
            snapshot_slot: read_u64(bytes, CANDIDATE_SNAPSHOT_SLOT_OFFSET)?,
            snapshot_unix_seconds: read_i64(bytes, CANDIDATE_SNAPSHOT_TIME_OFFSET)?,
            publish_time: read_i64(bytes, CANDIDATE_PUBLISH_TIME_OFFSET)?,
            posted_slot: read_u64(bytes, CANDIDATE_POSTED_SLOT_OFFSET)?,
            bump: byte(bytes, CANDIDATE_BUMP_OFFSET)?,
            update_bytes: array(bytes, CANDIDATE_UPDATE_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Canonical selection tuple.
    pub const fn selection(self) -> SponsoredPushSelectionV1 {
        SponsoredPushSelectionV1 {
            publish_time: self.publish_time,
            posted_slot: self.posted_slot,
            update_digest: self.update_digest,
        }
    }

    fn identities(self) -> [[u8; 32]; CANDIDATE_IDENTITY_COUNT] {
        [
            self.market,
            self.source_state,
            self.provider_release,
            self.sponsored_release,
            self.price_account,
            self.refund_recipient,
            self.update_digest,
        ]
    }
}

/// Ordered best-candidate coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredPushSelectionV1 {
    /// Publication time, primary ordering key.
    pub publish_time: i64,
    /// Posting slot, secondary ordering key.
    pub posted_slot: u64,
    /// Full-body digest, final deterministic tie-break.
    pub update_digest: [u8; 32],
}

impl SponsoredPushSelectionV1 {
    /// Return whether `self` is strictly later in the canonical total order.
    pub fn is_after(self, other: Self) -> bool {
        self.publish_time > other.publish_time
            || (self.publish_time == other.publish_time
                && (self.posted_slot > other.posted_slot
                    || (self.posted_slot == other.posted_slot
                        && self.update_digest > other.update_digest)))
    }
}

/// Mutable canonical best-valid-submitted-candidate head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredPushHeadV1 {
    /// Core Market.
    pub market: [u8; 32],
    /// Resolution-owned Source state.
    pub source_state: [u8; 32],
    /// Source-selected ProviderRelease identity.
    pub provider_release: [u8; 32],
    /// Exact sponsored release identity.
    pub sponsored_release: [u8; 32],
    /// Exact fixed upstream account.
    pub price_account: [u8; 32],
    /// First head funder and immutable head-rent refund recipient.
    pub head_refund_recipient: [u8; 32],
    /// Selected immutable candidate PDA.
    pub best_candidate: [u8; 32],
    /// Selected update body digest.
    pub best_update_digest: [u8; 32],
    /// Digest of the prior complete head bytes, or zero for first selection.
    pub prior_head_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Selected publication time.
    pub best_publish_time: i64,
    /// Selected posted slot.
    pub best_posted_slot: u64,
    /// Head PDA bump.
    pub bump: u8,
}

impl SponsoredPushHeadV1 {
    /// Construct the first head from one authenticated candidate.
    pub fn first(
        candidate_key: [u8; 32],
        candidate: SponsoredPushCandidateV1,
        head_refund_recipient: [u8; 32],
        bump: u8,
    ) -> Result<Self> {
        candidate.validate()?;
        if candidate_key == [0; 32] || head_refund_recipient == [0; 32] {
            return Err(Error::ZeroCoordinate);
        }
        let value = Self {
            market: candidate.market,
            source_state: candidate.source_state,
            provider_release: candidate.provider_release,
            sponsored_release: candidate.sponsored_release,
            price_account: candidate.price_account,
            head_refund_recipient,
            best_candidate: candidate_key,
            best_update_digest: candidate.update_digest,
            prior_head_digest: [0; 32],
            generation: candidate.generation,
            best_publish_time: candidate.publish_time,
            best_posted_slot: candidate.posted_slot,
            bump,
        };
        value.validate()?;
        Ok(value)
    }

    /// Advance only to a strictly greater candidate, retaining prior-head digest.
    pub fn select(
        self,
        candidate_key: [u8; 32],
        candidate: SponsoredPushCandidateV1,
    ) -> Result<Self> {
        self.validate()?;
        candidate.validate()?;
        if candidate_key == [0; 32]
            || self.market != candidate.market
            || self.source_state != candidate.source_state
            || self.provider_release != candidate.provider_release
            || self.sponsored_release != candidate.sponsored_release
            || self.price_account != candidate.price_account
            || self.generation != candidate.generation
            || !candidate.selection().is_after(self.selection())
        {
            return Err(Error::DuplicateCoordinate);
        }
        let prior_head_digest = digest(&self.to_bytes()?);
        let next = Self {
            best_candidate: candidate_key,
            best_update_digest: candidate.update_digest,
            prior_head_digest,
            best_publish_time: candidate.publish_time,
            best_posted_slot: candidate.posted_slot,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Canonical selection tuple.
    pub const fn selection(self) -> SponsoredPushSelectionV1 {
        SponsoredPushSelectionV1 {
            publish_time: self.best_publish_time,
            posted_slot: self.best_posted_slot,
            update_digest: self.best_update_digest,
        }
    }

    /// Validate head shape.
    pub fn validate(self) -> Result<()> {
        for identity in [
            self.market,
            self.source_state,
            self.provider_release,
            self.sponsored_release,
            self.price_account,
            self.head_refund_recipient,
            self.best_candidate,
            self.best_update_digest,
        ] {
            if identity == [0; 32] {
                return Err(Error::ZeroCoordinate);
            }
        }
        if self.generation == 0 || self.best_publish_time <= 0 || self.best_posted_slot == 0 {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }

    /// Encode canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; SPONSORED_PUSH_HEAD_BYTES_V1]> {
        self.validate()?;
        let mut out = [0_u8; SPONSORED_PUSH_HEAD_BYTES_V1];
        out[..8].copy_from_slice(&SPONSORED_PUSH_HEAD_MAGIC_V1);
        out[8..10].copy_from_slice(&SPONSORED_PUSH_VERSION_V1.to_le_bytes());
        for (index, value) in self.identities().iter().enumerate() {
            put32(&mut out, HEAD_IDENTITIES_OFFSET + index * 32, value)?;
        }
        put8(&mut out, HEAD_GENERATION_OFFSET, self.generation)?;
        put_i64(&mut out, HEAD_PUBLISH_TIME_OFFSET, self.best_publish_time)?;
        put8(&mut out, HEAD_POSTED_SLOT_OFFSET, self.best_posted_slot)?;
        out[HEAD_BUMP_OFFSET] = self.bump;
        Ok(out)
    }

    /// Decode canonical bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPONSORED_PUSH_HEAD_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != SPONSORED_PUSH_HEAD_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SPONSORED_PUSH_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 329, 7)?;
        let value = Self {
            market: array(bytes, 16)?,
            source_state: array(bytes, 48)?,
            provider_release: array(bytes, 80)?,
            sponsored_release: array(bytes, 112)?,
            price_account: array(bytes, 144)?,
            head_refund_recipient: array(bytes, 176)?,
            best_candidate: array(bytes, 208)?,
            best_update_digest: array(bytes, 240)?,
            prior_head_digest: array(bytes, 272)?,
            generation: read_u64(bytes, HEAD_GENERATION_OFFSET)?,
            best_publish_time: read_i64(bytes, HEAD_PUBLISH_TIME_OFFSET)?,
            best_posted_slot: read_u64(bytes, HEAD_POSTED_SLOT_OFFSET)?,
            bump: byte(bytes, HEAD_BUMP_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    fn identities(self) -> [[u8; 32]; HEAD_IDENTITY_COUNT] {
        [
            self.market,
            self.source_state,
            self.provider_release,
            self.sponsored_release,
            self.price_account,
            self.head_refund_recipient,
            self.best_candidate,
            self.best_update_digest,
            self.prior_head_digest,
        ]
    }
}

/// Durable terminal evidence for one sponsored candidate consumption.
///
/// The candidate remains immutable and independently closable. This receipt is
/// the permanent join between its capture-time provider facts and the slot at
/// which Resolution terminalized the Source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredPushReceiptV1 {
    /// Core Market.
    pub market: [u8; 32],
    /// Resolution-owned Source state.
    pub source_state: [u8; 32],
    /// Source-selected ProviderRelease identity.
    pub provider_release: [u8; 32],
    /// Exact sponsored deployment release identity.
    pub sponsored_release: [u8; 32],
    /// Exact sponsored upstream account.
    pub price_account: [u8; 32],
    /// Canonical mutable head account.
    pub head: [u8; 32],
    /// Selected immutable candidate account.
    pub candidate: [u8; 32],
    /// SHA-256 of the complete selected candidate bytes.
    pub candidate_digest: [u8; 32],
    /// Domain-separated evidence identity committed by Source and certificate.
    pub provider_evidence: [u8; 32],
    /// Canonical terminal certificate account.
    pub certificate: [u8; 32],
    /// Permissionless resolver that submitted the terminal transaction.
    pub resolver: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Positive Source terminal sequence.
    pub terminal_sequence: u64,
    /// Clock slot recorded by candidate admission.
    pub snapshot_slot: u64,
    /// Clock Unix time recorded by candidate admission.
    pub snapshot_unix_seconds: i64,
    /// Provider publication time.
    pub publish_time: i64,
    /// Provider posting slot.
    pub posted_slot: u64,
    /// Clock slot at terminal consumption.
    pub consumed_slot: u64,
    /// Selected ordinary Product outcome.
    pub selector: u32,
    /// Authenticated Product outcome count.
    pub outcome_count: u32,
    /// Exact normalized result numerator.
    pub result_numerator: i128,
    /// Positive exact result denominator.
    pub result_denominator: u64,
    /// Receipt PDA bump.
    pub bump: u8,
}

impl SponsoredPushReceiptV1 {
    /// Validate all canonical receipt relations owned by the codec.
    pub fn validate(self) -> Result<()> {
        for identity in self.identities() {
            if identity == [0; 32] {
                return Err(Error::ZeroCoordinate);
            }
        }
        if self.generation == 0
            || self.terminal_sequence == 0
            || self.snapshot_slot == 0
            || self.snapshot_unix_seconds <= 0
            || self.publish_time <= 0
            || self.posted_slot == 0
            || self.consumed_slot < self.snapshot_slot
            || self.outcome_count < 2
            || self.selector >= self.outcome_count - 1
            || self.result_denominator == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }

    /// Encode exact canonical receipt bytes.
    pub fn to_bytes(self) -> Result<[u8; SPONSORED_PUSH_RECEIPT_BYTES_V1]> {
        self.validate()?;
        let mut out = [0_u8; SPONSORED_PUSH_RECEIPT_BYTES_V1];
        out[..8].copy_from_slice(&SPONSORED_PUSH_RECEIPT_MAGIC_V1);
        out[8..10].copy_from_slice(&SPONSORED_PUSH_VERSION_V1.to_le_bytes());
        for (index, value) in self.identities().iter().enumerate() {
            put32(&mut out, RECEIPT_IDENTITIES_OFFSET + index * 32, value)?;
        }
        put8(&mut out, RECEIPT_GENERATION_OFFSET, self.generation)?;
        put8(
            &mut out,
            RECEIPT_TERMINAL_SEQUENCE_OFFSET,
            self.terminal_sequence,
        )?;
        put8(&mut out, RECEIPT_SNAPSHOT_SLOT_OFFSET, self.snapshot_slot)?;
        put_i64(
            &mut out,
            RECEIPT_SNAPSHOT_TIME_OFFSET,
            self.snapshot_unix_seconds,
        )?;
        put_i64(&mut out, RECEIPT_PUBLISH_TIME_OFFSET, self.publish_time)?;
        put8(&mut out, RECEIPT_POSTED_SLOT_OFFSET, self.posted_slot)?;
        put8(&mut out, RECEIPT_CONSUMED_SLOT_OFFSET, self.consumed_slot)?;
        put_u32(&mut out, RECEIPT_SELECTOR_OFFSET, self.selector)?;
        put_u32(&mut out, RECEIPT_OUTCOME_COUNT_OFFSET, self.outcome_count)?;
        put_i128(
            &mut out,
            RECEIPT_RESULT_NUMERATOR_OFFSET,
            self.result_numerator,
        )?;
        put8(
            &mut out,
            RECEIPT_RESULT_DENOMINATOR_OFFSET,
            self.result_denominator,
        )?;
        out[RECEIPT_BUMP_OFFSET] = self.bump;
        Ok(out)
    }

    /// Decode exact canonical receipt bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPONSORED_PUSH_RECEIPT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != SPONSORED_PUSH_RECEIPT_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SPONSORED_PUSH_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 457, 7)?;
        let value = Self {
            market: array(bytes, 16)?,
            source_state: array(bytes, 48)?,
            provider_release: array(bytes, 80)?,
            sponsored_release: array(bytes, 112)?,
            price_account: array(bytes, 144)?,
            head: array(bytes, 176)?,
            candidate: array(bytes, 208)?,
            candidate_digest: array(bytes, 240)?,
            provider_evidence: array(bytes, 272)?,
            certificate: array(bytes, 304)?,
            resolver: array(bytes, 336)?,
            generation: read_u64(bytes, RECEIPT_GENERATION_OFFSET)?,
            terminal_sequence: read_u64(bytes, RECEIPT_TERMINAL_SEQUENCE_OFFSET)?,
            snapshot_slot: read_u64(bytes, RECEIPT_SNAPSHOT_SLOT_OFFSET)?,
            snapshot_unix_seconds: read_i64(bytes, RECEIPT_SNAPSHOT_TIME_OFFSET)?,
            publish_time: read_i64(bytes, RECEIPT_PUBLISH_TIME_OFFSET)?,
            posted_slot: read_u64(bytes, RECEIPT_POSTED_SLOT_OFFSET)?,
            consumed_slot: read_u64(bytes, RECEIPT_CONSUMED_SLOT_OFFSET)?,
            selector: read_u32(bytes, RECEIPT_SELECTOR_OFFSET)?,
            outcome_count: read_u32(bytes, RECEIPT_OUTCOME_COUNT_OFFSET)?,
            result_numerator: read_i128(bytes, RECEIPT_RESULT_NUMERATOR_OFFSET)?,
            result_denominator: read_u64(bytes, RECEIPT_RESULT_DENOMINATOR_OFFSET)?,
            bump: byte(bytes, RECEIPT_BUMP_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    fn identities(self) -> [[u8; 32]; RECEIPT_IDENTITY_COUNT] {
        [
            self.market,
            self.source_state,
            self.provider_release,
            self.sponsored_release,
            self.price_account,
            self.head,
            self.candidate,
            self.candidate_digest,
            self.provider_evidence,
            self.certificate,
            self.resolver,
        ]
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    if bytes
        .get(offset..offset.checked_add(len).ok_or(Error::InvalidLength)?)
        .is_some_and(|value| value.iter().all(|byte| *byte == 0))
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
}

fn put32(output: &mut [u8], offset: usize, value: &[u8; 32]) -> Result<()> {
    output
        .get_mut(offset..offset.checked_add(32).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put8(output: &mut [u8], offset: usize, value: u64) -> Result<()> {
    output
        .get_mut(offset..offset.checked_add(8).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_i64(output: &mut [u8], offset: usize, value: i64) -> Result<()> {
    output
        .get_mut(offset..offset.checked_add(8).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    output
        .get_mut(offset..offset.checked_add(4).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_i128(output: &mut [u8], offset: usize, value: i128) -> Result<()> {
    output
        .get_mut(offset..offset.checked_add(16).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(tag: u8, publish_time: i64, posted_slot: u64) -> SponsoredPushCandidateV1 {
        let mut update_bytes = [tag; SPONSORED_PUSH_UPDATE_BYTES_V1];
        update_bytes[0] = 0x22;
        SponsoredPushCandidateV1 {
            market: [1; 32],
            source_state: [2; 32],
            provider_release: [3; 32],
            sponsored_release: [4; 32],
            price_account: [5; 32],
            refund_recipient: [6; 32],
            update_digest: digest(&update_bytes),
            generation: 7,
            snapshot_slot: 8,
            snapshot_unix_seconds: 9,
            publish_time,
            posted_slot,
            bump: 10,
            update_bytes,
        }
    }

    #[test]
    fn candidate_round_trip_is_exact_and_hostile_to_body_substitution() {
        let value = candidate(11, 12, 13);
        let bytes = value.to_bytes().expect("candidate");
        assert_eq!(SponsoredPushCandidateV1::decode(&bytes), Ok(value));
        let mut hostile = bytes;
        hostile[CANDIDATE_UPDATE_OFFSET + 1] ^= 1;
        assert_eq!(
            SponsoredPushCandidateV1::decode(&hostile),
            Err(Error::ZeroCoordinate)
        );
    }

    #[test]
    fn head_is_monotone_and_commits_prior_head_digest() {
        let first_candidate = candidate(11, 100, 20);
        let first =
            SponsoredPushHeadV1::first([20; 32], first_candidate, [21; 32], 7).expect("first");
        let first_bytes = first.to_bytes().expect("head bytes");
        assert_eq!(SponsoredPushHeadV1::decode(&first_bytes), Ok(first));

        let later = candidate(12, 101, 19);
        let next = first.select([21; 32], later).expect("later");
        assert_eq!(next.prior_head_digest, digest(&first_bytes));
        assert_eq!(next.best_publish_time, 101);
        assert_eq!(
            SponsoredPushHeadV1::decode(&next.to_bytes().expect("next")),
            Ok(next)
        );

        let older = candidate(13, 99, 99);
        assert_eq!(
            next.select([22; 32], older),
            Err(Error::DuplicateCoordinate)
        );
    }

    #[test]
    fn same_time_uses_slot_then_digest_as_total_order() {
        let first_candidate = candidate(11, 100, 20);
        let first =
            SponsoredPushHeadV1::first([20; 32], first_candidate, [21; 32], 7).expect("first");
        let later_slot = candidate(12, 100, 21);
        let second = first.select([21; 32], later_slot).expect("slot");
        let mut tie = candidate(13, 100, 21);
        while tie.update_digest <= second.best_update_digest {
            tie.update_bytes[1] = tie.update_bytes[1].wrapping_add(1);
            tie.update_digest = digest(&tie.update_bytes);
        }
        assert!(second.select([22; 32], tie).is_ok());
    }

    #[test]
    fn terminal_receipt_round_trips_both_provider_and_consumed_slots() {
        let receipt = SponsoredPushReceiptV1 {
            market: [1; 32],
            source_state: [2; 32],
            provider_release: [3; 32],
            sponsored_release: [4; 32],
            price_account: [5; 32],
            head: [6; 32],
            candidate: [7; 32],
            candidate_digest: [8; 32],
            provider_evidence: [9; 32],
            certificate: [10; 32],
            resolver: [11; 32],
            generation: 12,
            terminal_sequence: 13,
            snapshot_slot: 14,
            snapshot_unix_seconds: 15,
            publish_time: 16,
            posted_slot: 17,
            consumed_slot: 18,
            selector: 0,
            outcome_count: 2,
            result_numerator: -19,
            result_denominator: 1,
            bump: 20,
        };
        let bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(SponsoredPushReceiptV1::decode(&bytes), Ok(receipt));
        let mut hostile = bytes;
        hostile[457] = 1;
        assert_eq!(
            SponsoredPushReceiptV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }

    #[test]
    fn instruction_partition_is_exact() {
        for action in [
            SponsoredPushActionV1::Capture,
            SponsoredPushActionV1::Settle,
            SponsoredPushActionV1::CloseCandidate,
            SponsoredPushActionV1::CloseHead,
            SponsoredPushActionV1::CommitFailure,
        ] {
            let terminal_sequence = if matches!(
                action,
                SponsoredPushActionV1::Settle | SponsoredPushActionV1::CommitFailure
            ) {
                2
            } else {
                0
            };
            let value = SponsoredPushInstructionV1 {
                action,
                generation: 1,
                terminal_sequence,
            };
            let bytes = value.to_bytes().expect("instruction");
            assert_eq!(SponsoredPushInstructionV1::decode(&bytes), Ok(value));
        }
        let mut hostile = SponsoredPushInstructionV1 {
            action: SponsoredPushActionV1::Capture,
            generation: 1,
            terminal_sequence: 0,
        }
        .to_bytes()
        .expect("capture");
        hostile[11] = 1;
        assert_eq!(
            SponsoredPushInstructionV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }
}
