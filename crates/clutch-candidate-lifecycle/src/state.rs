// SPDX-License-Identifier: AGPL-3.0-or-later

use core::cmp::Ordering;

use crate::codec::{check_header, put_header, CodecError, Reader, Writer};

pub const CANDIDATES_PER_INDEX_PAGE: usize = 16;
pub const MAX_CANDIDATE_INDEX_PAGES: usize = 4;
pub const MAX_BEGUN_CANDIDATES: usize = CANDIDATES_PER_INDEX_PAGE * MAX_CANDIDATE_INDEX_PAGES;
pub const TOP_CANDIDATE_CAPACITY: usize = 3;
pub const RANK_KEY_CAPACITY: usize = 64;
pub const UNINDEXED_ORDINAL: u16 = u16::MAX;

// Kernel-local envelope tags. The SBF adapter must map these semantic families
// to globally reserved Solana account tags; these values are not live tags.
pub const WINDOW_TAG: u8 = 1;
pub const WINDOW_VERSION: u8 = 3;
pub const INDEX_PAGE_TAG: u8 = 2;
pub const INDEX_PAGE_VERSION: u8 = 1;
pub const CANDIDATE_TAG: u8 = 3;
pub const CANDIDATE_VERSION: u8 = 2;
pub const VERDICT_TAG: u8 = 4;
pub const VERDICT_VERSION: u8 = 1;
pub const ESCROW_TAG: u8 = 5;
pub const ESCROW_VERSION: u8 = 2;
pub const EPOCH_BUDGET_TAG: u8 = 6;
pub const EPOCH_BUDGET_VERSION: u8 = 2;
pub const LIFECYCLE_POLICY_TAG: u8 = 7;
pub const LIFECYCLE_POLICY_VERSION: u8 = 2;
pub const LIVENESS_POLICY_TAG: u8 = 8;
pub const LIVENESS_POLICY_VERSION: u8 = 2;

pub const WINDOW_BYTES: usize =
    2 + (6 * 32) + (5 * 8) + 32 + (TOP_CANDIDATE_CAPACITY * 32) + (6 * 2) + 5;
pub const INDEX_PAGE_BYTES: usize = 2 + 32 + 1 + 1 + 2 + (CANDIDATES_PER_INDEX_PAGE * 32) + 2;
pub const CANDIDATE_BYTES: usize = 2 + (12 * 32) + (3 * 8) + 4 + (2 * 2) + 3;
pub const VERDICT_BYTES: usize = 2 + (5 * 32) + 1 + RANK_KEY_CAPACITY + 8 + 2 + 3;
pub const ESCROW_BYTES: usize = 2 + (5 * 32) + (17 * 8) + (2 * 2) + 9;
pub const EPOCH_BUDGET_BYTES: usize = 2 + (5 * 32) + (16 * 8) + 5;
pub const LIFECYCLE_POLICY_BYTES: usize = 2 + 32 + (2 * 8) + 4 + (2 * 2) + 2;
pub const LIVENESS_POLICY_BYTES: usize = 2 + (2 * 32) + (11 * 8) + 2;

const _: () = assert!(WINDOW_BYTES == 379);
const _: () = assert!(INDEX_PAGE_BYTES == 552);
const _: () = assert!(CANDIDATE_BYTES == 421);
const _: () = assert!(VERDICT_BYTES == 240);
const _: () = assert!(ESCROW_BYTES == 311);
const _: () = assert!(EPOCH_BUDGET_BYTES == 295);
const _: () = assert!(LIFECYCLE_POLICY_BYTES == 60);
const _: () = assert!(LIVENESS_POLICY_BYTES == 156);

/// Stable 32-byte identity supplied and authenticated by the adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Id([u8; 32]);

impl Id {
    pub const ZERO: Self = Self([0; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

/// Explicit refusal from state validation or a pure transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ArithmeticOverflow,
    ZeroIdentity,
    DuplicateIdentity,
    InvalidPolicy,
    InvalidSchedule,
    InvalidCount,
    InvalidState,
    MismatchedBinding,
    NotActive,
    Replay,
    CapacityReached,
    Underfunded,
    RankCollision,
    UnresolvedCandidates,
}

pub(crate) fn live(id: Id) -> Result<(), Error> {
    if id.is_zero() {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn add(left: u64, right: u64) -> Result<u64, Error> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

pub(crate) fn mul(left: u64, right: u64) -> Result<u64, Error> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

fn map_validation(error: Error) -> CodecError {
    match error {
        Error::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        Error::ZeroIdentity => CodecError::ZeroIdentity,
        Error::InvalidCount | Error::CapacityReached => CodecError::InvalidCount,
        Error::InvalidState | Error::Replay | Error::NotActive => CodecError::InvalidEnum,
        Error::DuplicateIdentity
        | Error::InvalidPolicy
        | Error::InvalidSchedule
        | Error::MismatchedBinding
        | Error::Underfunded
        | Error::RankCollision
        | Error::UnresolvedCandidates => CodecError::MismatchedBinding,
    }
}

fn write_id(writer: &mut Writer<'_>, id: Id) -> Result<(), CodecError> {
    writer.bytes(&id.bytes())
}

fn read_id(reader: &mut Reader<'_>) -> Result<Id, CodecError> {
    Ok(Id::from_bytes(reader.array()?))
}

/// Canonical score-policy output. Greater lexicographic bytes rank first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankKey {
    len: u8,
    bytes: [u8; RANK_KEY_CAPACITY],
}

impl RankKey {
    pub const EMPTY: Self = Self {
        len: 0,
        bytes: [0; RANK_KEY_CAPACITY],
    };

    pub fn new(len: u8, bytes: [u8; RANK_KEY_CAPACITY]) -> Result<Self, Error> {
        let value = Self { len, bytes };
        value.validate_live()?;
        Ok(value)
    }

    pub const fn len(self) -> u8 {
        self.len
    }

    pub const fn bytes(self) -> [u8; RANK_KEY_CAPACITY] {
        self.bytes
    }

    pub fn is_empty(self) -> bool {
        self.len == 0 && self.bytes == [0; RANK_KEY_CAPACITY]
    }

    pub fn validate_live(self) -> Result<(), Error> {
        let len = usize::from(self.len);
        if len == 0 || len > RANK_KEY_CAPACITY {
            return Err(Error::InvalidCount);
        }
        if self.bytes[len..].iter().any(|byte| *byte != 0) {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    pub fn compare(self, other: Self) -> Result<Ordering, Error> {
        self.validate_live()?;
        other.validate_live()?;
        if self.len != other.len {
            return Err(Error::MismatchedBinding);
        }
        Ok(self.bytes[..usize::from(self.len)].cmp(&other.bytes[..usize::from(other.len)]))
    }

    /// Bind the fixed final 32 active bytes to the bitwise complement of the
    /// candidate identity. The lifecycle comparator is descending, so this
    /// makes a smaller candidate identity win an otherwise exact score tie
    /// without any global duplicate-key scan.
    pub fn validate_for_candidate(self, candidate: Id) -> Result<(), Error> {
        self.validate_live()?;
        live(candidate)?;
        let len = usize::from(self.len);
        if len < 32 {
            return Err(Error::InvalidCount);
        }
        let candidate_bytes = candidate.bytes();
        let suffix = &self.bytes[len - 32..len];
        let mut index = 0usize;
        while index < 32 {
            if suffix[index] != !candidate_bytes[index] {
                return Err(Error::MismatchedBinding);
            }
            index += 1;
        }
        Ok(())
    }

    fn encode_body(self, writer: &mut Writer<'_>) -> Result<(), CodecError> {
        self.validate_live().map_err(map_validation)?;
        writer.u8(self.len)?;
        writer.bytes(&self.bytes)
    }
}

/// Clock interval under the half-open S/V convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interval {
    Submission,
    Verification,
    Terminal,
}

/// Immutable schedule stamped by the successful freeze.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Schedule {
    pub frozen_slot: u64,
    pub submission_closes_slot: u64,
    pub verification_closes_slot: u64,
}

impl Schedule {
    pub fn stamp(
        frozen_slot: u64,
        submission_span_slots: u64,
        verification_span_slots: u64,
    ) -> Result<Self, Error> {
        if frozen_slot == 0 || submission_span_slots == 0 || verification_span_slots == 0 {
            return Err(Error::InvalidSchedule);
        }
        let submission_closes_slot = add(frozen_slot, submission_span_slots)?;
        let verification_closes_slot = add(submission_closes_slot, verification_span_slots)?;
        Ok(Self {
            frozen_slot,
            submission_closes_slot,
            verification_closes_slot,
        })
    }

    pub const fn interval(self, slot: u64) -> Result<Interval, Error> {
        if slot < self.frozen_slot {
            return Err(Error::NotActive);
        }
        if slot < self.submission_closes_slot {
            Ok(Interval::Submission)
        } else if slot < self.verification_closes_slot {
            Ok(Interval::Verification)
        } else {
            Ok(Interval::Terminal)
        }
    }
}

/// Immutable timing/capacity policy. Its identity is authenticated externally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateLifecyclePolicyV2 {
    pub policy_id: Id,
    pub submission_span_slots: u64,
    pub verification_span_slots: u64,
    pub max_feed_bytes: u32,
    /// Maximum begun records. Staging and sealed children share this bound.
    pub max_begun_candidates: u16,
    pub max_verification_units: u16,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateLifecyclePolicyV2 {
    pub fn validate(self) -> Result<(), Error> {
        live(self.policy_id)?;
        if self.submission_span_slots == 0
            || self.verification_span_slots == 0
            || self.max_feed_bytes == 0
            || self.max_begun_candidates == 0
            || usize::from(self.max_begun_candidates) > MAX_BEGUN_CANDIDATES
            || self.max_verification_units == 0
            || self.flags != 0
        {
            return Err(Error::InvalidPolicy);
        }
        Schedule::stamp(1, self.submission_span_slots, self.verification_span_slots)?;
        Ok(())
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, LIFECYCLE_POLICY_BYTES)?;
        put_header(&mut writer, LIFECYCLE_POLICY_TAG, LIFECYCLE_POLICY_VERSION)?;
        write_id(&mut writer, self.policy_id)?;
        writer.u64(self.submission_span_slots)?;
        writer.u64(self.verification_span_slots)?;
        writer.bytes(&self.max_feed_bytes.to_le_bytes())?;
        writer.u16(self.max_begun_candidates)?;
        writer.u16(self.max_verification_units)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, LIFECYCLE_POLICY_BYTES)?;
        check_header(&mut reader, LIFECYCLE_POLICY_TAG, LIFECYCLE_POLICY_VERSION)?;
        let value = Self {
            policy_id: read_id(&mut reader)?,
            submission_span_slots: reader.u64()?,
            verification_span_slots: reader.u64()?,
            max_feed_bytes: u32::from_le_bytes(reader.array()?),
            max_begun_candidates: reader.u16()?,
            max_verification_units: reader.u16()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

/// Immutable present-funding policy. No fee or collateral input exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateLivenessPolicyV2 {
    pub policy_id: Id,
    pub neutral_sink: Id,
    pub progress_reward_per_unit: u64,
    pub completion_reward: u64,
    pub expiry_reward: u64,
    pub candidate_close_reward: u64,
    pub freeze_reward: u64,
    pub finalizer_reward: u64,
    pub index_page_close_reward: u64,
    pub bond_lamports: u64,
    pub invalidity_penalty: u64,
    pub abandonment_penalty: u64,
    pub solver_prize: u64,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateLivenessPolicyV2 {
    pub fn validate(self) -> Result<(), Error> {
        live(self.policy_id)?;
        live(self.neutral_sink)?;
        if self.progress_reward_per_unit == 0
            || self.completion_reward == 0
            || self.expiry_reward == 0
            || self.candidate_close_reward == 0
            || self.freeze_reward == 0
            || self.finalizer_reward == 0
            || self.index_page_close_reward == 0
            || self.bond_lamports == 0
            || self.invalidity_penalty == 0
            || self.invalidity_penalty > self.bond_lamports
            || self.abandonment_penalty == 0
            || self.abandonment_penalty > self.bond_lamports
            || self.flags != 0
        {
            return Err(Error::InvalidPolicy);
        }
        add(
            mul(self.progress_reward_per_unit, u64::from(u16::MAX))?,
            self.completion_reward,
        )?;
        add(self.expiry_reward, self.candidate_close_reward)?;
        mul(
            self.index_page_close_reward,
            u64::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?,
        )?;
        Ok(())
    }

    pub fn work_reserve(self, verification_units: u16) -> Result<u64, Error> {
        self.validate()?;
        if verification_units == 0 {
            return Err(Error::InvalidCount);
        }
        add(
            mul(self.progress_reward_per_unit, u64::from(verification_units))?,
            self.completion_reward,
        )
    }

    pub fn candidate_cleanup_reserve(self) -> Result<u64, Error> {
        self.validate()?;
        add(self.expiry_reward, self.candidate_close_reward)
    }

    pub fn index_cleanup_reserve(self) -> Result<u64, Error> {
        self.validate()?;
        mul(
            self.index_page_close_reward,
            u64::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?,
        )
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, LIVENESS_POLICY_BYTES)?;
        put_header(&mut writer, LIVENESS_POLICY_TAG, LIVENESS_POLICY_VERSION)?;
        write_id(&mut writer, self.policy_id)?;
        write_id(&mut writer, self.neutral_sink)?;
        for value in [
            self.progress_reward_per_unit,
            self.completion_reward,
            self.expiry_reward,
            self.candidate_close_reward,
            self.freeze_reward,
            self.finalizer_reward,
            self.index_page_close_reward,
            self.bond_lamports,
            self.invalidity_penalty,
            self.abandonment_penalty,
            self.solver_prize,
        ] {
            writer.u64(value)?;
        }
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, LIVENESS_POLICY_BYTES)?;
        check_header(&mut reader, LIVENESS_POLICY_TAG, LIVENESS_POLICY_VERSION)?;
        let value = Self {
            policy_id: read_id(&mut reader)?,
            neutral_sink: read_id(&mut reader)?,
            progress_reward_per_unit: reader.u64()?,
            completion_reward: reader.u64()?,
            expiry_reward: reader.u64()?,
            candidate_close_reward: reader.u64()?,
            freeze_reward: reader.u64()?,
            finalizer_reward: reader.u64()?,
            index_page_close_reward: reader.u64()?,
            bond_lamports: reader.u64()?,
            invalidity_penalty: reader.u64()?,
            abandonment_penalty: reader.u64()?,
            solver_prize: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

/// Immutable score-policy binding. Score computation lives outside this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScorePolicyBindingV1 {
    pub policy_id: Id,
    pub rank_key_len: u8,
}

impl ScorePolicyBindingV1 {
    pub fn validate(self) -> Result<(), Error> {
        live(self.policy_id)?;
        if self.rank_key_len < 32 || usize::from(self.rank_key_len) > RANK_KEY_CAPACITY {
            return Err(Error::InvalidPolicy);
        }
        Ok(())
    }
}

/// Version-three Window state for the two exclusive deadlines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateWindowV3 {
    pub epoch: Id,
    pub market: Id,
    pub relation_policy_id: Id,
    pub lifecycle_policy_id: Id,
    pub score_policy_id: Id,
    pub liveness_policy_id: Id,
    pub freeze_deadline_slot: u64,
    pub frozen_slot: u64,
    pub submission_closes_slot: u64,
    pub verification_closes_slot: u64,
    pub finalized_slot: u64,
    pub selected_candidate: Id,
    /// Candidate identities retained in rank order. Verdict accounts own ranks.
    pub top_candidates: [Id; TOP_CANDIDATE_CAPACITY],
    pub begun_candidate_count: u16,
    pub sealed_candidate_count: u16,
    pub verdict_count: u16,
    pub valid_verdict_count: u16,
    pub expired_staging_count: u16,
    pub expired_unverified_count: u16,
    pub candidate_page_count: u8,
    pub top_count: u8,
    pub rank_key_len: u8,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateWindowV3 {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        epoch: Id,
        market: Id,
        relation_policy_id: Id,
        lifecycle: CandidateLifecyclePolicyV2,
        score: ScorePolicyBindingV1,
        liveness: CandidateLivenessPolicyV2,
        freeze_deadline_slot: u64,
        stored_bump: u8,
    ) -> Result<Self, Error> {
        for id in [epoch, market, relation_policy_id] {
            live(id)?;
        }
        lifecycle.validate()?;
        score.validate()?;
        liveness.validate()?;
        if freeze_deadline_slot == 0 {
            return Err(Error::InvalidSchedule);
        }
        let value = Self {
            epoch,
            market,
            relation_policy_id,
            lifecycle_policy_id: lifecycle.policy_id,
            score_policy_id: score.policy_id,
            liveness_policy_id: liveness.policy_id,
            freeze_deadline_slot,
            frozen_slot: 0,
            submission_closes_slot: 0,
            verification_closes_slot: 0,
            finalized_slot: 0,
            selected_candidate: Id::ZERO,
            top_candidates: [Id::ZERO; TOP_CANDIDATE_CAPACITY],
            begun_candidate_count: 0,
            sealed_candidate_count: 0,
            verdict_count: 0,
            valid_verdict_count: 0,
            expired_staging_count: 0,
            expired_unverified_count: 0,
            candidate_page_count: 0,
            top_count: 0,
            rank_key_len: score.rank_key_len,
            stored_bump,
            flags: 0,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), Error> {
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.lifecycle_policy_id,
            self.score_policy_id,
            self.liveness_policy_id,
        ] {
            live(id)?;
        }
        if self.freeze_deadline_slot == 0
            || self.rank_key_len < 32
            || usize::from(self.rank_key_len) > RANK_KEY_CAPACITY
            || self.flags != 0
            || usize::from(self.begun_candidate_count) > MAX_BEGUN_CANDIDATES
            || usize::from(self.candidate_page_count) > MAX_CANDIDATE_INDEX_PAGES
            || usize::from(self.top_count) > TOP_CANDIDATE_CAPACITY
            || self.sealed_candidate_count > self.begun_candidate_count
            || self.verdict_count > self.sealed_candidate_count
            || self.valid_verdict_count > self.verdict_count
            || self.expired_staging_count > self.begun_candidate_count
            || self.expired_unverified_count > self.sealed_candidate_count
            || self
                .sealed_candidate_count
                .checked_add(self.expired_staging_count)
                .ok_or(Error::ArithmeticOverflow)?
                > self.begun_candidate_count
            || self
                .verdict_count
                .checked_add(self.expired_unverified_count)
                .ok_or(Error::ArithmeticOverflow)?
                > self.sealed_candidate_count
        {
            return Err(Error::InvalidCount);
        }
        let expected_top = core::cmp::min(
            usize::from(self.valid_verdict_count),
            TOP_CANDIDATE_CAPACITY,
        );
        if usize::from(self.top_count) != expected_top {
            return Err(Error::MismatchedBinding);
        }
        let expected_pages = if self.begun_candidate_count == 0 {
            0
        } else {
            u8::try_from(
                (usize::from(self.begun_candidate_count) - 1) / CANDIDATES_PER_INDEX_PAGE + 1,
            )
            .map_err(|_| Error::ArithmeticOverflow)?
        };
        if self.candidate_page_count != expected_pages {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < TOP_CANDIDATE_CAPACITY {
            if index < usize::from(self.top_count) {
                live(self.top_candidates[index])?;
                let mut other = index + 1;
                while other < usize::from(self.top_count) {
                    if self.top_candidates[index] == self.top_candidates[other] {
                        return Err(Error::DuplicateIdentity);
                    }
                    other += 1;
                }
            } else if !self.top_candidates[index].is_zero() {
                return Err(Error::InvalidState);
            }
            index += 1;
        }
        if self.frozen_slot == 0 {
            if self.submission_closes_slot != 0
                || self.verification_closes_slot != 0
                || self.finalized_slot != 0
                || !self.selected_candidate.is_zero()
                || self.begun_candidate_count != 0
                || self.sealed_candidate_count != 0
                || self.verdict_count != 0
                || self.valid_verdict_count != 0
                || self.expired_staging_count != 0
                || self.expired_unverified_count != 0
                || self.top_count != 0
            {
                return Err(Error::InvalidState);
            }
        } else if self.frozen_slot < self.freeze_deadline_slot
            || self.frozen_slot >= self.submission_closes_slot
            || self.submission_closes_slot >= self.verification_closes_slot
        {
            return Err(Error::InvalidSchedule);
        }
        if self.finalized_slot == 0 {
            if !self.selected_candidate.is_zero() {
                return Err(Error::InvalidState);
            }
        } else if self.frozen_slot == 0 || self.finalized_slot < self.submission_closes_slot {
            return Err(Error::InvalidSchedule);
        } else {
            if self.selected_candidate.is_zero() != (self.top_count == 0)
                || (!self.selected_candidate.is_zero()
                    && self.selected_candidate != self.top_candidates[0])
            {
                return Err(Error::MismatchedBinding);
            }
            if self.finalized_slot < self.verification_closes_slot
                && self.verdict_count != self.sealed_candidate_count
            {
                return Err(Error::UnresolvedCandidates);
            }
        }
        Ok(())
    }

    pub fn schedule(self) -> Result<Schedule, Error> {
        self.validate()?;
        if self.frozen_slot == 0 {
            return Err(Error::NotActive);
        }
        Ok(Schedule {
            frozen_slot: self.frozen_slot,
            submission_closes_slot: self.submission_closes_slot,
            verification_closes_slot: self.verification_closes_slot,
        })
    }

    pub const fn is_finalized(self) -> bool {
        self.finalized_slot != 0
    }

    pub fn bind_policies(
        self,
        lifecycle: CandidateLifecyclePolicyV2,
        score: ScorePolicyBindingV1,
        liveness: CandidateLivenessPolicyV2,
    ) -> Result<(), Error> {
        self.validate()?;
        lifecycle.validate()?;
        score.validate()?;
        liveness.validate()?;
        if self.lifecycle_policy_id != lifecycle.policy_id
            || self.score_policy_id != score.policy_id
            || self.rank_key_len != score.rank_key_len
            || self.liveness_policy_id != liveness.policy_id
        {
            return Err(Error::MismatchedBinding);
        }
        if self.begun_candidate_count > lifecycle.max_begun_candidates {
            return Err(Error::InvalidCount);
        }
        if self.frozen_slot != 0 {
            let expected = Schedule::stamp(
                self.frozen_slot,
                lifecycle.submission_span_slots,
                lifecycle.verification_span_slots,
            )?;
            if self.submission_closes_slot != expected.submission_closes_slot
                || self.verification_closes_slot != expected.verification_closes_slot
            {
                return Err(Error::InvalidSchedule);
            }
        }
        Ok(())
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, WINDOW_BYTES)?;
        put_header(&mut writer, WINDOW_TAG, WINDOW_VERSION)?;
        for id in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.lifecycle_policy_id,
            self.score_policy_id,
            self.liveness_policy_id,
        ] {
            write_id(&mut writer, id)?;
        }
        for slot in [
            self.freeze_deadline_slot,
            self.frozen_slot,
            self.submission_closes_slot,
            self.verification_closes_slot,
            self.finalized_slot,
        ] {
            writer.u64(slot)?;
        }
        write_id(&mut writer, self.selected_candidate)?;
        for id in self.top_candidates {
            write_id(&mut writer, id)?;
        }
        writer.u16(self.begun_candidate_count)?;
        writer.u16(self.sealed_candidate_count)?;
        writer.u16(self.verdict_count)?;
        writer.u16(self.valid_verdict_count)?;
        writer.u16(self.expired_staging_count)?;
        writer.u16(self.expired_unverified_count)?;
        writer.u8(self.candidate_page_count)?;
        writer.u8(self.top_count)?;
        writer.u8(self.rank_key_len)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, WINDOW_BYTES)?;
        check_header(&mut reader, WINDOW_TAG, WINDOW_VERSION)?;
        let value = Self {
            epoch: read_id(&mut reader)?,
            market: read_id(&mut reader)?,
            relation_policy_id: read_id(&mut reader)?,
            lifecycle_policy_id: read_id(&mut reader)?,
            score_policy_id: read_id(&mut reader)?,
            liveness_policy_id: read_id(&mut reader)?,
            freeze_deadline_slot: reader.u64()?,
            frozen_slot: reader.u64()?,
            submission_closes_slot: reader.u64()?,
            verification_closes_slot: reader.u64()?,
            finalized_slot: reader.u64()?,
            selected_candidate: read_id(&mut reader)?,
            top_candidates: [
                read_id(&mut reader)?,
                read_id(&mut reader)?,
                read_id(&mut reader)?,
            ],
            begun_candidate_count: reader.u16()?,
            sealed_candidate_count: reader.u16()?,
            verdict_count: reader.u16()?,
            valid_verdict_count: reader.u16()?,
            expired_staging_count: reader.u16()?,
            expired_unverified_count: reader.u16()?,
            candidate_page_count: reader.u8()?,
            top_count: reader.u8()?,
            rank_key_len: reader.u8()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

/// One fixed page of the exhaustive begun-candidate enumeration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateIndexPageV1 {
    pub epoch: Id,
    pub page_index: u8,
    pub count: u8,
    /// One bit per enumerated candidate; set only by terminal cleanup.
    pub closed_mask: u16,
    pub candidates: [Id; CANDIDATES_PER_INDEX_PAGE],
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateIndexPageV1 {
    pub fn empty(epoch: Id, page_index: u8, stored_bump: u8) -> Result<Self, Error> {
        let value = Self {
            epoch,
            page_index,
            count: 0,
            closed_mask: 0,
            candidates: [Id::ZERO; CANDIDATES_PER_INDEX_PAGE],
            stored_bump,
            flags: 0,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), Error> {
        live(self.epoch)?;
        if usize::from(self.page_index) >= MAX_CANDIDATE_INDEX_PAGES
            || usize::from(self.count) > CANDIDATES_PER_INDEX_PAGE
            || self.flags != 0
        {
            return Err(Error::InvalidCount);
        }
        let active_mask = if usize::from(self.count) == CANDIDATES_PER_INDEX_PAGE {
            u16::MAX
        } else {
            (1u16 << self.count) - 1
        };
        if self.closed_mask & !active_mask != 0 {
            return Err(Error::InvalidState);
        }
        let mut index = 0usize;
        while index < CANDIDATES_PER_INDEX_PAGE {
            if index < usize::from(self.count) {
                live(self.candidates[index])?;
                let mut other = index + 1;
                while other < usize::from(self.count) {
                    if self.candidates[index] == self.candidates[other] {
                        return Err(Error::DuplicateIdentity);
                    }
                    other += 1;
                }
            } else if !self.candidates[index].is_zero() {
                return Err(Error::InvalidState);
            }
            index += 1;
        }
        Ok(())
    }

    pub fn bind_candidate(self, candidate: CandidateRecordV2) -> Result<(), Error> {
        self.validate()?;
        candidate.validate()?;
        let ordinal = usize::from(candidate.index_ordinal);
        let expected_page = ordinal / CANDIDATES_PER_INDEX_PAGE;
        let expected_offset = ordinal % CANDIDATES_PER_INDEX_PAGE;
        if self.epoch != candidate.epoch
            || usize::from(self.page_index) != expected_page
            || expected_offset >= usize::from(self.count)
            || self.candidates[expected_offset] != candidate.candidate
            || self.closed_mask & (1u16 << expected_offset) != 0
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    pub fn mark_candidate_closed(self, candidate: CandidateRecordV2) -> Result<Self, Error> {
        self.bind_candidate(candidate)?;
        let offset = usize::from(candidate.index_ordinal) % CANDIDATES_PER_INDEX_PAGE;
        let mut next = self;
        next.closed_mask |= 1u16 << offset;
        next.validate()?;
        Ok(next)
    }

    pub fn all_candidates_closed(self) -> Result<bool, Error> {
        self.validate()?;
        let active_mask = if usize::from(self.count) == CANDIDATES_PER_INDEX_PAGE {
            u16::MAX
        } else {
            (1u16 << self.count) - 1
        };
        Ok(self.closed_mask == active_mask)
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, INDEX_PAGE_BYTES)?;
        put_header(&mut writer, INDEX_PAGE_TAG, INDEX_PAGE_VERSION)?;
        write_id(&mut writer, self.epoch)?;
        writer.u8(self.page_index)?;
        writer.u8(self.count)?;
        writer.u16(self.closed_mask)?;
        for candidate in self.candidates {
            write_id(&mut writer, candidate)?;
        }
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, INDEX_PAGE_BYTES)?;
        check_header(&mut reader, INDEX_PAGE_TAG, INDEX_PAGE_VERSION)?;
        let mut candidates = [Id::ZERO; CANDIDATES_PER_INDEX_PAGE];
        let epoch = read_id(&mut reader)?;
        let page_index = reader.u8()?;
        let count = reader.u8()?;
        let closed_mask = reader.u16()?;
        let mut index = 0usize;
        while index < CANDIDATES_PER_INDEX_PAGE {
            candidates[index] = read_id(&mut reader)?;
            index += 1;
        }
        let value = Self {
            epoch,
            page_index,
            count,
            closed_mask,
            candidates,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidateStatus {
    Staging = 0,
    Sealed = 1,
    Verdicted = 2,
    ExpiredStaging = 3,
    ExpiredUnverified = 4,
}

impl CandidateStatus {
    const fn wire(self) -> u8 {
        match self {
            Self::Staging => 0,
            Self::Sealed => 1,
            Self::Verdicted => 2,
            Self::ExpiredStaging => 3,
            Self::ExpiredUnverified => 4,
        }
    }

    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Staging),
            1 => Ok(Self::Sealed),
            2 => Ok(Self::Verdicted),
            3 => Ok(Self::ExpiredStaging),
            4 => Ok(Self::ExpiredUnverified),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Candidate lifecycle record. Score components do not live here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateRecordV2 {
    pub candidate: Id,
    pub epoch: Id,
    pub market: Id,
    pub relation_policy_id: Id,
    pub lifecycle_policy_id: Id,
    pub score_policy_id: Id,
    pub liveness_policy_id: Id,
    /// Authenticated solver identity. Copy/front-run resistance is an adapter
    /// admission obligation; this field makes the resulting owner immutable.
    pub solver: Id,
    pub solver_reward_destination: Id,
    pub feed: Id,
    /// Digest of the exact sealed feed bytes; zero while staging.
    pub feed_content_digest: Id,
    /// Immutable Verdict identity; zero until a checked verdict exists.
    pub verdict: Id,
    pub begun_slot: u64,
    pub sealed_slot: u64,
    pub terminal_slot: u64,
    pub expected_feed_bytes: u32,
    pub verification_units: u16,
    pub index_ordinal: u16,
    pub status: CandidateStatus,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateRecordV2 {
    pub fn validate(self) -> Result<(), Error> {
        for id in [
            self.candidate,
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.lifecycle_policy_id,
            self.score_policy_id,
            self.liveness_policy_id,
            self.solver,
            self.solver_reward_destination,
            self.feed,
        ] {
            live(id)?;
        }
        if self.begun_slot == 0
            || self.expected_feed_bytes == 0
            || self.verification_units == 0
            || self.flags != 0
        {
            return Err(Error::InvalidState);
        }
        let indexed = self.index_ordinal != UNINDEXED_ORDINAL;
        if indexed && usize::from(self.index_ordinal) >= MAX_BEGUN_CANDIDATES {
            return Err(Error::InvalidCount);
        }
        match self.status {
            CandidateStatus::Staging => {
                if self.sealed_slot != 0
                    || self.terminal_slot != 0
                    || !indexed
                    || !self.feed_content_digest.is_zero()
                    || !self.verdict.is_zero()
                {
                    return Err(Error::InvalidState);
                }
            }
            CandidateStatus::Sealed => {
                if self.sealed_slot < self.begun_slot
                    || self.terminal_slot != 0
                    || !indexed
                    || self.feed_content_digest.is_zero()
                    || !self.verdict.is_zero()
                {
                    return Err(Error::InvalidState);
                }
            }
            CandidateStatus::Verdicted => {
                if self.sealed_slot < self.begun_slot
                    || self.terminal_slot < self.sealed_slot
                    || !indexed
                    || self.feed_content_digest.is_zero()
                    || self.verdict.is_zero()
                {
                    return Err(Error::InvalidState);
                }
            }
            CandidateStatus::ExpiredUnverified => {
                if self.sealed_slot < self.begun_slot
                    || self.terminal_slot < self.sealed_slot
                    || !indexed
                    || self.feed_content_digest.is_zero()
                    || !self.verdict.is_zero()
                {
                    return Err(Error::InvalidState);
                }
            }
            CandidateStatus::ExpiredStaging => {
                if self.sealed_slot != 0
                    || self.terminal_slot < self.begun_slot
                    || !indexed
                    || !self.feed_content_digest.is_zero()
                    || !self.verdict.is_zero()
                {
                    return Err(Error::InvalidState);
                }
            }
        }
        Ok(())
    }

    pub fn bind_window(self, window: CandidateWindowV3) -> Result<(), Error> {
        self.validate()?;
        window.validate()?;
        if self.epoch != window.epoch
            || self.market != window.market
            || self.relation_policy_id != window.relation_policy_id
            || self.lifecycle_policy_id != window.lifecycle_policy_id
            || self.score_policy_id != window.score_policy_id
            || self.liveness_policy_id != window.liveness_policy_id
            || self.verification_units == 0
        {
            return Err(Error::MismatchedBinding);
        }
        let schedule = window.schedule()?;
        if self.begun_slot < schedule.frozen_slot
            || self.begun_slot >= schedule.submission_closes_slot
        {
            return Err(Error::InvalidSchedule);
        }
        match self.status {
            CandidateStatus::Staging => {}
            CandidateStatus::Sealed => {
                if self.sealed_slot >= schedule.submission_closes_slot {
                    return Err(Error::InvalidSchedule);
                }
            }
            CandidateStatus::Verdicted => {
                if self.sealed_slot >= schedule.submission_closes_slot
                    || self.terminal_slot < schedule.submission_closes_slot
                    || self.terminal_slot >= schedule.verification_closes_slot
                {
                    return Err(Error::InvalidSchedule);
                }
            }
            CandidateStatus::ExpiredStaging => {
                if self.terminal_slot < schedule.submission_closes_slot {
                    return Err(Error::InvalidSchedule);
                }
            }
            CandidateStatus::ExpiredUnverified => {
                if self.sealed_slot >= schedule.submission_closes_slot
                    || self.terminal_slot < schedule.verification_closes_slot
                {
                    return Err(Error::InvalidSchedule);
                }
            }
        }
        Ok(())
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, CANDIDATE_BYTES)?;
        put_header(&mut writer, CANDIDATE_TAG, CANDIDATE_VERSION)?;
        for id in [
            self.candidate,
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.lifecycle_policy_id,
            self.score_policy_id,
            self.liveness_policy_id,
            self.solver,
            self.solver_reward_destination,
            self.feed,
            self.feed_content_digest,
            self.verdict,
        ] {
            write_id(&mut writer, id)?;
        }
        writer.u64(self.begun_slot)?;
        writer.u64(self.sealed_slot)?;
        writer.u64(self.terminal_slot)?;
        writer.bytes(&self.expected_feed_bytes.to_le_bytes())?;
        writer.u16(self.verification_units)?;
        writer.u16(self.index_ordinal)?;
        writer.u8(self.status.wire())?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, CANDIDATE_BYTES)?;
        check_header(&mut reader, CANDIDATE_TAG, CANDIDATE_VERSION)?;
        let value = Self {
            candidate: read_id(&mut reader)?,
            epoch: read_id(&mut reader)?,
            market: read_id(&mut reader)?,
            relation_policy_id: read_id(&mut reader)?,
            lifecycle_policy_id: read_id(&mut reader)?,
            score_policy_id: read_id(&mut reader)?,
            liveness_policy_id: read_id(&mut reader)?,
            solver: read_id(&mut reader)?,
            solver_reward_destination: read_id(&mut reader)?,
            feed: read_id(&mut reader)?,
            feed_content_digest: read_id(&mut reader)?,
            verdict: read_id(&mut reader)?,
            begun_slot: reader.u64()?,
            sealed_slot: reader.u64()?,
            terminal_slot: reader.u64()?,
            expected_feed_bytes: u32::from_le_bytes(reader.array()?),
            verification_units: reader.u16()?,
            index_ordinal: reader.u16()?,
            status: CandidateStatus::decode(reader.u8()?)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VerdictKind {
    Valid = 1,
    Refused = 2,
}

impl VerdictKind {
    const fn wire(self) -> u8 {
        match self {
            Self::Valid => 1,
            Self::Refused => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Valid),
            2 => Ok(Self::Refused),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Immutable checked relation verdict and generic score rank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateVerdictV1 {
    pub verdict: Id,
    pub candidate: Id,
    pub epoch: Id,
    pub relation_digest: Id,
    pub score_policy_id: Id,
    pub rank_key: RankKey,
    pub verified_slot: u64,
    pub refusal_code: u16,
    pub kind: VerdictKind,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateVerdictV1 {
    pub const EMPTY: Self = Self {
        verdict: Id::ZERO,
        candidate: Id::ZERO,
        epoch: Id::ZERO,
        relation_digest: Id::ZERO,
        score_policy_id: Id::ZERO,
        rank_key: RankKey::EMPTY,
        verified_slot: 0,
        refusal_code: 0,
        kind: VerdictKind::Valid,
        stored_bump: 0,
        flags: 0,
    };

    pub fn is_empty(self) -> bool {
        self.verdict.is_zero()
            && self.candidate.is_zero()
            && self.epoch.is_zero()
            && self.relation_digest.is_zero()
            && self.score_policy_id.is_zero()
            && self.rank_key.is_empty()
            && self.verified_slot == 0
            && self.refusal_code == 0
            && self.stored_bump == 0
            && self.flags == 0
    }

    pub fn validate(self) -> Result<(), Error> {
        for id in [
            self.verdict,
            self.candidate,
            self.epoch,
            self.relation_digest,
            self.score_policy_id,
        ] {
            live(id)?;
        }
        if self.verified_slot == 0 || self.flags != 0 {
            return Err(Error::InvalidState);
        }
        match self.kind {
            VerdictKind::Valid => {
                self.rank_key.validate_for_candidate(self.candidate)?;
                if self.refusal_code != 0 {
                    return Err(Error::InvalidState);
                }
            }
            VerdictKind::Refused => {
                if !self.rank_key.is_empty() || self.refusal_code == 0 {
                    return Err(Error::InvalidState);
                }
            }
        }
        Ok(())
    }

    pub fn bind_candidate(
        self,
        candidate: CandidateRecordV2,
        window: CandidateWindowV3,
    ) -> Result<(), Error> {
        self.validate()?;
        candidate.bind_window(window)?;
        if self.candidate != candidate.candidate
            || self.epoch != candidate.epoch
            || self.verdict != candidate.verdict
            || self.score_policy_id != window.score_policy_id
            || self.verified_slot != candidate.terminal_slot
            || candidate.status != CandidateStatus::Verdicted
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, VERDICT_BYTES)?;
        put_header(&mut writer, VERDICT_TAG, VERDICT_VERSION)?;
        for id in [
            self.verdict,
            self.candidate,
            self.epoch,
            self.relation_digest,
            self.score_policy_id,
        ] {
            write_id(&mut writer, id)?;
        }
        if self.kind == VerdictKind::Valid {
            self.rank_key.encode_body(&mut writer)?;
        } else {
            writer.u8(0)?;
            writer.bytes(&[0; RANK_KEY_CAPACITY])?;
        }
        writer.u64(self.verified_slot)?;
        writer.u16(self.refusal_code)?;
        writer.u8(self.kind.wire())?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, VERDICT_BYTES)?;
        check_header(&mut reader, VERDICT_TAG, VERDICT_VERSION)?;
        let verdict = read_id(&mut reader)?;
        let candidate = read_id(&mut reader)?;
        let epoch = read_id(&mut reader)?;
        let relation_digest = read_id(&mut reader)?;
        let score_policy_id = read_id(&mut reader)?;
        let rank_len = reader.u8()?;
        let rank_bytes = reader.array()?;
        let value = Self {
            verdict,
            candidate,
            epoch,
            relation_digest,
            score_policy_id,
            rank_key: if rank_len == 0 {
                if rank_bytes != [0; RANK_KEY_CAPACITY] {
                    return Err(CodecError::NonCanonicalPadding);
                }
                RankKey::EMPTY
            } else {
                RankKey::new(rank_len, rank_bytes).map_err(map_validation)?
            },
            verified_slot: reader.u64()?,
            refusal_code: reader.u16()?,
            kind: VerdictKind::decode(reader.u8()?)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EscrowFundingState {
    Staging = 0,
    Sealed = 1,
}

impl EscrowFundingState {
    const fn wire(self) -> u8 {
        match self {
            Self::Staging => 0,
            Self::Sealed => 1,
        }
    }

    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Staging),
            1 => Ok(Self::Sealed),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Exact prepaid candidate compartments. Rent is never spendable as reward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateEscrowV2 {
    pub candidate: Id,
    pub payer: Id,
    pub refund_destination: Id,
    pub neutral_sink: Id,
    pub liveness_policy_id: Id,
    pub staging_rent_principal: u64,
    pub verification_rent_principal: u64,
    pub work_initial: u64,
    pub work_remaining: u64,
    pub work_paid: u64,
    pub work_refunded: u64,
    pub bond_initial: u64,
    pub bond_remaining: u64,
    pub bond_slashed: u64,
    pub bond_refunded: u64,
    pub cleanup_initial: u64,
    pub cleanup_remaining: u64,
    pub cleanup_paid: u64,
    pub cleanup_refunded: u64,
    pub solver_credited: u64,
    pub solver_remaining: u64,
    pub solver_paid: u64,
    pub paid_units: u16,
    pub total_units: u16,
    pub funding_state: EscrowFundingState,
    pub bond_refund_claimed: u8,
    pub work_refund_claimed: u8,
    pub cleanup_finalized: u8,
    pub solver_credit_claimed: u8,
    pub work_closed: u8,
    pub candidate_closed: u8,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateEscrowV2 {
    pub fn validate(self) -> Result<(), Error> {
        for id in [
            self.candidate,
            self.payer,
            self.refund_destination,
            self.neutral_sink,
            self.liveness_policy_id,
        ] {
            live(id)?;
        }
        if self.payer == self.neutral_sink
            || self.refund_destination == self.neutral_sink
            || self.staging_rent_principal == 0
            || self.bond_initial == 0
            || self.cleanup_initial == 0
            || self.bond_refund_claimed > 1
            || self.work_refund_claimed > 1
            || self.cleanup_finalized > 1
            || self.solver_credit_claimed > 1
            || self.work_closed > 1
            || self.candidate_closed > 1
            || self.flags != 0
            || self.paid_units > self.total_units
        {
            return Err(Error::InvalidState);
        }
        if add(
            add(self.work_remaining, self.work_paid)?,
            self.work_refunded,
        )? != self.work_initial
            || add(
                add(self.bond_remaining, self.bond_slashed)?,
                self.bond_refunded,
            )? != self.bond_initial
            || add(
                add(self.cleanup_remaining, self.cleanup_paid)?,
                self.cleanup_refunded,
            )? != self.cleanup_initial
            || add(self.solver_remaining, self.solver_paid)? != self.solver_credited
        {
            return Err(Error::MismatchedBinding);
        }
        match self.funding_state {
            EscrowFundingState::Staging => {
                if self.verification_rent_principal != 0
                    || self.work_initial != 0
                    || self.work_remaining != 0
                    || self.work_paid != 0
                    || self.work_refunded != 0
                    || self.paid_units != 0
                    || self.total_units != 0
                {
                    return Err(Error::InvalidState);
                }
            }
            EscrowFundingState::Sealed => {
                if self.verification_rent_principal == 0
                    || self.work_initial == 0
                    || self.total_units == 0
                {
                    return Err(Error::InvalidState);
                }
            }
        }
        if self.bond_refund_claimed == 1 && self.bond_remaining != 0 {
            return Err(Error::InvalidState);
        }
        if self.work_refund_claimed == 1 && (self.work_remaining != 0 || self.work_closed != 1) {
            return Err(Error::InvalidState);
        }
        if self.cleanup_finalized != self.candidate_closed
            || (self.cleanup_finalized == 1 && self.cleanup_remaining != 0)
        {
            return Err(Error::InvalidState);
        }
        if self.candidate_closed == 1
            && (self.bond_refund_claimed != 1
                || (self.funding_state == EscrowFundingState::Sealed
                    && (self.work_closed != 1 || self.work_refund_claimed != 1))
                || (self.solver_credited != 0 && self.solver_credit_claimed != 1))
        {
            return Err(Error::InvalidState);
        }
        if self.solver_credit_claimed == 1
            && (self.solver_remaining != 0 || self.solver_credited == 0)
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    pub fn accounted_lamports(self) -> Result<u64, Error> {
        self.validate()?;
        let rent = if self.candidate_closed == 1 {
            0
        } else {
            add(
                self.staging_rent_principal,
                self.verification_rent_principal,
            )?
        };
        let live = add(
            add(self.work_remaining, self.bond_remaining)?,
            self.cleanup_remaining,
        )?;
        add(rent, add(live, self.solver_remaining)?)
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, ESCROW_BYTES)?;
        put_header(&mut writer, ESCROW_TAG, ESCROW_VERSION)?;
        for id in [
            self.candidate,
            self.payer,
            self.refund_destination,
            self.neutral_sink,
            self.liveness_policy_id,
        ] {
            write_id(&mut writer, id)?;
        }
        for value in [
            self.staging_rent_principal,
            self.verification_rent_principal,
            self.work_initial,
            self.work_remaining,
            self.work_paid,
            self.work_refunded,
            self.bond_initial,
            self.bond_remaining,
            self.bond_slashed,
            self.bond_refunded,
            self.cleanup_initial,
            self.cleanup_remaining,
            self.cleanup_paid,
            self.cleanup_refunded,
            self.solver_credited,
            self.solver_remaining,
            self.solver_paid,
        ] {
            writer.u64(value)?;
        }
        writer.u16(self.paid_units)?;
        writer.u16(self.total_units)?;
        writer.u8(self.funding_state.wire())?;
        writer.u8(self.bond_refund_claimed)?;
        writer.u8(self.work_refund_claimed)?;
        writer.u8(self.cleanup_finalized)?;
        writer.u8(self.solver_credit_claimed)?;
        writer.u8(self.work_closed)?;
        writer.u8(self.candidate_closed)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, ESCROW_BYTES)?;
        check_header(&mut reader, ESCROW_TAG, ESCROW_VERSION)?;
        let value = Self {
            candidate: read_id(&mut reader)?,
            payer: read_id(&mut reader)?,
            refund_destination: read_id(&mut reader)?,
            neutral_sink: read_id(&mut reader)?,
            liveness_policy_id: read_id(&mut reader)?,
            staging_rent_principal: reader.u64()?,
            verification_rent_principal: reader.u64()?,
            work_initial: reader.u64()?,
            work_remaining: reader.u64()?,
            work_paid: reader.u64()?,
            work_refunded: reader.u64()?,
            bond_initial: reader.u64()?,
            bond_remaining: reader.u64()?,
            bond_slashed: reader.u64()?,
            bond_refunded: reader.u64()?,
            cleanup_initial: reader.u64()?,
            cleanup_remaining: reader.u64()?,
            cleanup_paid: reader.u64()?,
            cleanup_refunded: reader.u64()?,
            solver_credited: reader.u64()?,
            solver_remaining: reader.u64()?,
            solver_paid: reader.u64()?,
            paid_units: reader.u16()?,
            total_units: reader.u16()?,
            funding_state: EscrowFundingState::decode(reader.u8()?)?,
            bond_refund_claimed: reader.u8()?,
            work_refund_claimed: reader.u8()?,
            cleanup_finalized: reader.u8()?,
            solver_credit_claimed: reader.u8()?,
            work_closed: reader.u8()?,
            candidate_closed: reader.u8()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}

/// Prepaid epoch-level finalization and solver-prize budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochCandidateBudgetV2 {
    pub epoch: Id,
    pub sponsor: Id,
    pub refund_destination: Id,
    pub neutral_sink: Id,
    pub liveness_policy_id: Id,
    pub account_rent_principal: u64,
    pub index_page_rent_principal: u64,
    pub freeze_initial: u64,
    pub freeze_remaining: u64,
    pub freeze_paid: u64,
    pub finalizer_initial: u64,
    pub finalizer_remaining: u64,
    pub finalizer_paid: u64,
    pub index_cleanup_initial: u64,
    pub index_cleanup_remaining: u64,
    pub index_cleanup_paid: u64,
    pub index_cleanup_refunded: u64,
    pub solver_initial: u64,
    pub solver_remaining: u64,
    pub solver_credited: u64,
    pub solver_refunded: u64,
    pub index_pages_owed: u8,
    pub terminalized: u8,
    pub refund_claimed: u8,
    pub stored_bump: u8,
    pub flags: u8,
}

impl EpochCandidateBudgetV2 {
    pub fn validate(self) -> Result<(), Error> {
        for id in [
            self.epoch,
            self.sponsor,
            self.refund_destination,
            self.neutral_sink,
            self.liveness_policy_id,
        ] {
            live(id)?;
        }
        if self.sponsor == self.neutral_sink
            || self.refund_destination == self.neutral_sink
            || self.account_rent_principal == 0
            || self.index_page_rent_principal == 0
            || !self.index_page_rent_principal.is_multiple_of(
                u64::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?,
            )
            || self.freeze_initial == 0
            || self.finalizer_initial == 0
            || self.index_cleanup_initial == 0
            || usize::from(self.index_pages_owed) > MAX_CANDIDATE_INDEX_PAGES
            || self.terminalized > 1
            || self.refund_claimed > 1
            || self.flags != 0
            || add(self.freeze_remaining, self.freeze_paid)? != self.freeze_initial
            || add(self.finalizer_remaining, self.finalizer_paid)? != self.finalizer_initial
            || add(
                add(self.index_cleanup_remaining, self.index_cleanup_paid)?,
                self.index_cleanup_refunded,
            )? != self.index_cleanup_initial
            || add(
                add(self.solver_remaining, self.solver_credited)?,
                self.solver_refunded,
            )? != self.solver_initial
        {
            return Err(Error::MismatchedBinding);
        }
        if self.terminalized == 0 && (self.index_pages_owed != 0 || self.refund_claimed != 0) {
            return Err(Error::InvalidState);
        }
        if self.refund_claimed == 1
            && (self.solver_remaining != 0
                || self.index_cleanup_remaining != 0
                || self.index_pages_owed != 0)
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    pub fn accounted_lamports(self) -> Result<u64, Error> {
        self.validate()?;
        let index_rent = if self.terminalized == 0 {
            self.index_page_rent_principal
        } else {
            mul(
                self.index_page_rent_principal
                    / u64::try_from(MAX_CANDIDATE_INDEX_PAGES)
                        .map_err(|_| Error::ArithmeticOverflow)?,
                u64::from(self.index_pages_owed),
            )?
        };
        add(
            add(self.account_rent_principal, index_rent)?,
            add(
                add(self.freeze_remaining, self.finalizer_remaining)?,
                add(self.index_cleanup_remaining, self.solver_remaining)?,
            )?,
        )
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_validation)?;
        let mut writer = Writer::exact(out, EPOCH_BUDGET_BYTES)?;
        put_header(&mut writer, EPOCH_BUDGET_TAG, EPOCH_BUDGET_VERSION)?;
        for id in [
            self.epoch,
            self.sponsor,
            self.refund_destination,
            self.neutral_sink,
            self.liveness_policy_id,
        ] {
            write_id(&mut writer, id)?;
        }
        for value in [
            self.account_rent_principal,
            self.index_page_rent_principal,
            self.freeze_initial,
            self.freeze_remaining,
            self.freeze_paid,
            self.finalizer_initial,
            self.finalizer_remaining,
            self.finalizer_paid,
            self.index_cleanup_initial,
            self.index_cleanup_remaining,
            self.index_cleanup_paid,
            self.index_cleanup_refunded,
            self.solver_initial,
            self.solver_remaining,
            self.solver_credited,
            self.solver_refunded,
        ] {
            writer.u64(value)?;
        }
        writer.u8(self.index_pages_owed)?;
        writer.u8(self.terminalized)?;
        writer.u8(self.refund_claimed)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, EPOCH_BUDGET_BYTES)?;
        check_header(&mut reader, EPOCH_BUDGET_TAG, EPOCH_BUDGET_VERSION)?;
        let value = Self {
            epoch: read_id(&mut reader)?,
            sponsor: read_id(&mut reader)?,
            refund_destination: read_id(&mut reader)?,
            neutral_sink: read_id(&mut reader)?,
            liveness_policy_id: read_id(&mut reader)?,
            account_rent_principal: reader.u64()?,
            index_page_rent_principal: reader.u64()?,
            freeze_initial: reader.u64()?,
            freeze_remaining: reader.u64()?,
            freeze_paid: reader.u64()?,
            finalizer_initial: reader.u64()?,
            finalizer_remaining: reader.u64()?,
            finalizer_paid: reader.u64()?,
            index_cleanup_initial: reader.u64()?,
            index_cleanup_remaining: reader.u64()?,
            index_cleanup_paid: reader.u64()?,
            index_cleanup_refunded: reader.u64()?,
            solver_initial: reader.u64()?,
            solver_remaining: reader.u64()?,
            solver_credited: reader.u64()?,
            solver_refunded: reader.u64()?,
            index_pages_owed: reader.u8()?,
            terminalized: reader.u8()?,
            refund_claimed: reader.u8()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate().map_err(map_validation)?;
        Ok(value)
    }
}
