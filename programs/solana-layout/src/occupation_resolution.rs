//! Version-four resolution persistence for quantized B-spline occupation.
//!
//! This wire shape is deliberately separate from version-three native point
//! resolution.  It cannot encode a preset or a derived-at-one-point fact:
//! version four admits only a canonical unresolved record or one resolved
//! quantized-occupation vector with its exact archive and finalization
//! provenance.

use super::{
    CodecError, Hash32, MarketAccount, PayoutVectorBytes, Result, TermsAccount, MAX_OUTCOMES,
    PAYOUT_INDEX_UNRESOLVED, RESOLUTION_TAG,
};

/// Version of the quantized-occupation Resolution account wire shape.
pub const OCCUPATION_RESOLUTION_VERSION: u8 = 4;
/// Exact byte length of [`OccupationResolutionAccount`].
pub const OCCUPATION_RESOLUTION_LEN: usize = 383;

/// No resolution fact has been admitted.
pub const RESOLUTION_MODE_UNRESOLVED: u8 = 0;
/// A vector derived by averaging quantized native B-spline basis evaluations.
pub const RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION: u8 = 3;

/// Componentwise exact-only quantized-basis occupation statistic.
pub const STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06: u16 = 6;
/// Canonical largest-remainder quantized-basis occupation statistic.
pub const STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07: u16 = 7;

/// Componentwise exact division is required at the final averaging boundary.
pub const OCCUPATION_FINALIZATION_EXACT_ONLY: u8 = 1;
/// Largest remainder with lowest-index exact ties is used at final averaging.
pub const OCCUPATION_FINALIZATION_LARGEST_REMAINDER_V1: u8 = 2;
/// Frozen native B-spline point-evaluator semantic version.
pub const OCCUPATION_BASIS_EVALUATOR_VERSION: u16 = 1;
/// Frozen bounded occupation-summary semantic version.
pub const OCCUPATION_SUMMARY_VERSION: u16 = 1;

const OFFSET_MARKET: usize = 2;
const OFFSET_TERMS: usize = OFFSET_MARKET + 32;
const OFFSET_FEED: usize = OFFSET_TERMS + 32;
const OFFSET_WINDOW: usize = OFFSET_FEED + 32;
const OFFSET_FEED_CURSOR: usize = OFFSET_WINDOW + 32;
const OFFSET_SEALED_END: usize = OFFSET_FEED_CURSOR + 8;
const OFFSET_REPAIR_GENERATION: usize = OFFSET_SEALED_END + 8;
const OFFSET_RESOLVED_SLOT: usize = OFFSET_REPAIR_GENERATION + 8;
const OFFSET_MODE: usize = OFFSET_RESOLVED_SLOT + 8;
const OFFSET_PAYOUT_INDEX: usize = OFFSET_MODE + 1;
const OFFSET_OUTCOME_COUNT: usize = OFFSET_PAYOUT_INDEX + 1;
const OFFSET_RESOLVED_VALUE: usize = OFFSET_OUTCOME_COUNT + 1;
const OFFSET_DENOMINATOR: usize = OFFSET_RESOLVED_VALUE + 16;
const OFFSET_WEIGHTS: usize = OFFSET_DENOMINATOR + 8;
const OFFSET_ARCHIVE_COMMITMENT: usize = OFFSET_WEIGHTS + (MAX_OUTCOMES * 8);
const OFFSET_STATISTIC: usize = OFFSET_ARCHIVE_COMMITMENT + 32;
const OFFSET_FINALIZATION: usize = OFFSET_STATISTIC + 2;
const OFFSET_BASIS_EVALUATOR_VERSION: usize = OFFSET_FINALIZATION + 1;
const OFFSET_OCCUPATION_SUMMARY_VERSION: usize = OFFSET_BASIS_EVALUATOR_VERSION + 2;
const OFFSET_SAMPLE_COUNT: usize = OFFSET_OCCUPATION_SUMMARY_VERSION + 2;
const OFFSET_COVERAGE_COUNT: usize = OFFSET_SAMPLE_COUNT + 8;
const OFFSET_GAP_COUNT: usize = OFFSET_COVERAGE_COUNT + 8;
const OFFSET_STORED_BUMP: usize = OFFSET_GAP_COUNT + 8;
const OFFSET_FLAGS: usize = OFFSET_STORED_BUMP + 1;
const OFFSET_RESERVED: usize = OFFSET_FLAGS + 1;

const _: () = assert!(OFFSET_ARCHIVE_COMMITMENT == 317);
const _: () = assert!(OFFSET_STORED_BUMP == 380);
const _: () = assert!(OFFSET_RESERVED + 1 == OCCUPATION_RESOLUTION_LEN);

/// Sole persisted owner of one quantized B-spline occupation payout vector.
///
/// `resolved_value` is retained only to keep the common v3 prefix byte-for-byte
/// legible.  It is inactive and must be zero in this distinct mode.  The
/// archive commitment, statistic, finalizer, semantic versions, and coverage
/// counts make an exact retry comparable without inferring meaning from the
/// vector contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccupationResolutionAccount {
    /// Market identity.
    pub market: Hash32,
    /// Immutable Terms digest.
    pub terms: Hash32,
    /// Feed identity selected by Terms.
    pub feed: Hash32,
    /// Canonical immutable window-domain identity.
    pub window: Hash32,
    /// Authenticated feed cursor witnessed at archive seal.
    pub feed_cursor: u64,
    /// Exclusive final bucket of the sealed archive.
    pub sealed_end_bucket_exclusive: u64,
    /// Repair generation of the sealed archive.
    pub repair_generation: u64,
    /// Slot at which the immutable resolution fact was first recorded.
    pub resolved_slot: u64,
    /// Zero while unresolved; otherwise
    /// [`RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION`].
    pub mode: u8,
    /// Always [`PAYOUT_INDEX_UNRESOLVED`]; occupation never searches presets.
    pub payout_index: u8,
    /// Active vector prefix when resolved; zero while unresolved.
    pub outcome_count: u8,
    /// Inactive common-prefix field; canonically zero in every v4 state.
    pub resolved_value: u128,
    /// Derived vector when resolved; canonical all-zero padding otherwise.
    pub vector: PayoutVectorBytes,
    /// Commitment of the exact sealed archive page that was folded.
    pub archive_commitment: Hash32,
    /// One of the two registered quantized-occupation statistic ids.
    pub statistic: u16,
    /// Exact-only or canonical largest-remainder finalization.
    pub finalization: u8,
    /// Frozen point-evaluator semantic version.
    pub basis_evaluator_version: u16,
    /// Frozen occupation-summary semantic version.
    pub occupation_summary_version: u16,
    /// Number of canonical buckets folded, including explicit gaps.
    pub sample_count: u64,
    /// Number of admitted exact observations.
    pub coverage_count: u64,
    /// Number of explicit gaps; successful v1 finalization requires zero.
    pub gap_count: u64,
    /// Stored Resolution PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
    /// Reserved byte; currently zero.
    pub reserved: u8,
}

impl OccupationResolutionAccount {
    /// All-zero decode target; not a valid account.
    pub const ZEROED: Self = Self {
        market: Hash32::ZERO,
        terms: Hash32::ZERO,
        feed: Hash32::ZERO,
        window: Hash32::ZERO,
        feed_cursor: 0,
        sealed_end_bucket_exclusive: 0,
        repair_generation: 0,
        resolved_slot: 0,
        mode: RESOLUTION_MODE_UNRESOLVED,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: 0,
        resolved_value: 0,
        vector: PayoutVectorBytes::ZERO,
        archive_commitment: Hash32::ZERO,
        statistic: 0,
        finalization: 0,
        basis_evaluator_version: 0,
        occupation_summary_version: 0,
        sample_count: 0,
        coverage_count: 0,
        gap_count: 0,
        stored_bump: 0,
        flags: 0,
        reserved: 0,
    };

    /// Canonical unresolved v4 record for a newly founded occupation market.
    pub const fn unresolved(market: Hash32, terms: Hash32, feed: Hash32, stored_bump: u8) -> Self {
        Self {
            market,
            terms,
            feed,
            stored_bump,
            ..Self::ZEROED
        }
    }

    /// Whether this account carries an immutable occupation resolution fact.
    pub const fn is_resolved(&self) -> bool {
        self.mode == RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION
    }

    /// Validate identity, mode, vector, provenance, version, and count shape.
    pub fn validate(&self) -> Result<()> {
        check_required_hash(self.market)?;
        check_required_hash(self.terms)?;
        check_required_hash(self.feed)?;
        if self.flags != 0 || self.reserved != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        match self.mode {
            RESOLUTION_MODE_UNRESOLVED => self.validate_unresolved(),
            RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION => self.validate_resolved(),
            _ => Err(CodecError::InvalidEnum),
        }
    }

    fn validate_unresolved(&self) -> Result<()> {
        if self.window != Hash32::ZERO
            || self.feed_cursor != 0
            || self.sealed_end_bucket_exclusive != 0
            || self.repair_generation != 0
            || self.resolved_slot != 0
            || self.payout_index != PAYOUT_INDEX_UNRESOLVED
            || self.outcome_count != 0
            || self.resolved_value != 0
            || self.vector != PayoutVectorBytes::ZERO
            || self.archive_commitment != Hash32::ZERO
            || self.statistic != 0
            || self.finalization != 0
            || self.basis_evaluator_version != 0
            || self.occupation_summary_version != 0
            || self.sample_count != 0
            || self.coverage_count != 0
            || self.gap_count != 0
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(())
    }

    fn validate_resolved(&self) -> Result<()> {
        check_required_hash(self.window)?;
        check_required_hash(self.archive_commitment)?;
        if self.sealed_end_bucket_exclusive == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.feed_cursor < self.sealed_end_bucket_exclusive {
            return Err(CodecError::MismatchedBinding);
        }
        if self.payout_index != PAYOUT_INDEX_UNRESOLVED || self.resolved_value != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        if !(2..=MAX_OUTCOMES as u8).contains(&self.outcome_count) {
            return Err(CodecError::InvalidCount);
        }
        self.vector
            .validate_active(self.outcome_count, self.vector.denominator)?;
        let expected_finalization = finalization_for_statistic(self.statistic)?;
        if self.finalization != expected_finalization {
            return Err(CodecError::MismatchedBinding);
        }
        if self.basis_evaluator_version != OCCUPATION_BASIS_EVALUATOR_VERSION
            || self.occupation_summary_version != OCCUPATION_SUMMARY_VERSION
        {
            return Err(CodecError::WrongVersion);
        }
        if self.sample_count == 0 || self.coverage_count != self.sample_count || self.gap_count != 0
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// Bind this record to already validated immutable Terms fields.
    pub fn binds_terms_fields(&self, terms: &TermsAccount) -> Result<()> {
        if self.terms != terms.terms || self.feed != terms.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if !(1..=3).contains(&terms.basis_degree) || !is_occupation_statistic(terms.statistic_id) {
            return Err(CodecError::MismatchedBinding);
        }
        if self.is_resolved() {
            let maturity = terms
                .expected_start_bucket
                .checked_add(terms.maturity_horizon_buckets)
                .ok_or(CodecError::ArithmeticOverflow)?;
            let span = terms.expected_span()?;
            if self.sealed_end_bucket_exclusive != terms.expected_end_bucket_exclusive
                || self.repair_generation != terms.repair_generation
                || self.feed_cursor < maturity
                || self.outcome_count != terms.outcome_count
                || self.vector.denominator != terms.payouts[0].denominator
                || self.statistic != terms.statistic_id
                || u32::from(self.basis_evaluator_version) != terms.evaluator_version
                || self.sample_count != span
            {
                return Err(CodecError::MismatchedBinding);
            }
        }
        Ok(())
    }

    /// Validate and bind this record to immutable Terms.
    pub fn binds_terms(&self, terms: &TermsAccount) -> Result<()> {
        self.validate()?;
        terms.validate()?;
        self.binds_terms_fields(terms)
    }

    /// Bind this record to already validated Market fields.
    pub fn binds_market_fields(&self, market: &MarketAccount) -> Result<()> {
        if self.market != market.market || self.terms != market.terms || self.feed != market.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if (self.is_resolved() && market.lifecycle != 1)
            || (!self.is_resolved() && market.lifecycle != 0)
        {
            return Err(CodecError::MismatchedBinding);
        }
        if self.is_resolved() && self.outcome_count != market.outcome_count {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Validate the complete Market/Terms/Resolution join.
    pub fn binds_market_and_terms(
        &self,
        market: &MarketAccount,
        terms: &TermsAccount,
    ) -> Result<()> {
        self.validate()?;
        terms.binds_market(market)?;
        self.binds_market_fields(market)?;
        self.binds_terms_fields(terms)
    }

    /// Return the record-owned vector, or `None` while unresolved.
    pub fn effective_vector(&self, terms: &TermsAccount) -> Result<Option<PayoutVectorBytes>> {
        self.binds_terms(terms)?;
        if self.is_resolved() {
            Ok(Some(self.vector))
        } else {
            Ok(None)
        }
    }

    /// Encode exactly [`OCCUPATION_RESOLUTION_LEN`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < OCCUPATION_RESOLUTION_LEN {
            return Err(CodecError::OutputTooSmall);
        }
        out[0] = RESOLUTION_TAG;
        out[1] = OCCUPATION_RESOLUTION_VERSION;
        put_hash(out, OFFSET_MARKET, self.market);
        put_hash(out, OFFSET_TERMS, self.terms);
        put_hash(out, OFFSET_FEED, self.feed);
        put_hash(out, OFFSET_WINDOW, self.window);
        put_u64(out, OFFSET_FEED_CURSOR, self.feed_cursor);
        put_u64(out, OFFSET_SEALED_END, self.sealed_end_bucket_exclusive);
        put_u64(out, OFFSET_REPAIR_GENERATION, self.repair_generation);
        put_u64(out, OFFSET_RESOLVED_SLOT, self.resolved_slot);
        out[OFFSET_MODE] = self.mode;
        out[OFFSET_PAYOUT_INDEX] = self.payout_index;
        out[OFFSET_OUTCOME_COUNT] = self.outcome_count;
        put_u128(out, OFFSET_RESOLVED_VALUE, self.resolved_value);
        put_u64(out, OFFSET_DENOMINATOR, self.vector.denominator);
        let mut index = 0;
        while index < MAX_OUTCOMES {
            put_u64(
                out,
                OFFSET_WEIGHTS + (index * 8),
                self.vector.weights[index],
            );
            index += 1;
        }
        put_hash(out, OFFSET_ARCHIVE_COMMITMENT, self.archive_commitment);
        put_u16(out, OFFSET_STATISTIC, self.statistic);
        out[OFFSET_FINALIZATION] = self.finalization;
        put_u16(
            out,
            OFFSET_BASIS_EVALUATOR_VERSION,
            self.basis_evaluator_version,
        );
        put_u16(
            out,
            OFFSET_OCCUPATION_SUMMARY_VERSION,
            self.occupation_summary_version,
        );
        put_u64(out, OFFSET_SAMPLE_COUNT, self.sample_count);
        put_u64(out, OFFSET_COVERAGE_COUNT, self.coverage_count);
        put_u64(out, OFFSET_GAP_COUNT, self.gap_count);
        out[OFFSET_STORED_BUMP] = self.stored_bump;
        out[OFFSET_FLAGS] = self.flags;
        out[OFFSET_RESERVED] = self.reserved;
        Ok(OCCUPATION_RESOLUTION_LEN)
    }

    /// Parse exactly [`OCCUPATION_RESOLUTION_LEN`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::ZEROED;
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }

    /// Parse hostile bytes directly into caller-owned storage.
    ///
    /// On refusal, `out` may contain a partial value and must not be read.
    pub fn decode_into(input: &[u8], out: &mut Self) -> Result<()> {
        if input.len() < OCCUPATION_RESOLUTION_LEN {
            return Err(CodecError::Truncated);
        }
        if input.len() > OCCUPATION_RESOLUTION_LEN {
            return Err(CodecError::TrailingBytes);
        }
        if input[0] != RESOLUTION_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != OCCUPATION_RESOLUTION_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let mut weights = [0_u64; MAX_OUTCOMES];
        let mut index = 0;
        while index < MAX_OUTCOMES {
            weights[index] = get_u64(input, OFFSET_WEIGHTS + (index * 8));
            index += 1;
        }
        out.market = get_hash(input, OFFSET_MARKET);
        out.terms = get_hash(input, OFFSET_TERMS);
        out.feed = get_hash(input, OFFSET_FEED);
        out.window = get_hash(input, OFFSET_WINDOW);
        out.feed_cursor = get_u64(input, OFFSET_FEED_CURSOR);
        out.sealed_end_bucket_exclusive = get_u64(input, OFFSET_SEALED_END);
        out.repair_generation = get_u64(input, OFFSET_REPAIR_GENERATION);
        out.resolved_slot = get_u64(input, OFFSET_RESOLVED_SLOT);
        out.mode = input[OFFSET_MODE];
        out.payout_index = input[OFFSET_PAYOUT_INDEX];
        out.outcome_count = input[OFFSET_OUTCOME_COUNT];
        out.resolved_value = get_u128(input, OFFSET_RESOLVED_VALUE);
        out.vector = PayoutVectorBytes {
            denominator: get_u64(input, OFFSET_DENOMINATOR),
            weights,
        };
        out.archive_commitment = get_hash(input, OFFSET_ARCHIVE_COMMITMENT);
        out.statistic = get_u16(input, OFFSET_STATISTIC);
        out.finalization = input[OFFSET_FINALIZATION];
        out.basis_evaluator_version = get_u16(input, OFFSET_BASIS_EVALUATOR_VERSION);
        out.occupation_summary_version = get_u16(input, OFFSET_OCCUPATION_SUMMARY_VERSION);
        out.sample_count = get_u64(input, OFFSET_SAMPLE_COUNT);
        out.coverage_count = get_u64(input, OFFSET_COVERAGE_COUNT);
        out.gap_count = get_u64(input, OFFSET_GAP_COUNT);
        out.stored_bump = input[OFFSET_STORED_BUMP];
        out.flags = input[OFFSET_FLAGS];
        out.reserved = input[OFFSET_RESERVED];
        out.validate()
    }
}

/// Whether a statistic id selects this v4 occupation wire shape.
pub const fn is_occupation_statistic(statistic: u16) -> bool {
    statistic == STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06
        || statistic == STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07
}

/// Return the sole finalization byte registered to an occupation statistic.
pub const fn finalization_for_statistic(statistic: u16) -> Result<u8> {
    match statistic {
        STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06 => Ok(OCCUPATION_FINALIZATION_EXACT_ONLY),
        STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07 => {
            Ok(OCCUPATION_FINALIZATION_LARGEST_REMAINDER_V1)
        }
        _ => Err(CodecError::InvalidEnum),
    }
}

fn check_required_hash(value: Hash32) -> Result<()> {
    Hash32::new(value.bytes()).map(|_| ())
}

fn put_hash(out: &mut [u8], offset: usize, value: Hash32) {
    out[offset..offset + 32].copy_from_slice(&value.bytes());
}

fn get_hash(input: &[u8], offset: usize) -> Hash32 {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&input[offset..offset + 32]);
    Hash32::from_bytes(bytes)
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(&input[offset..offset + 2]);
    u16::from_le_bytes(bytes)
}

fn put_u64(out: &mut [u8], offset: usize, value: u64) {
    out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u64(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn put_u128(out: &mut [u8], offset: usize, value: u128) {
    out[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
}

fn get_u128(input: &[u8], offset: usize) -> u128 {
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&input[offset..offset + 16]);
    u128::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn resolved(statistic: u16) -> OccupationResolutionAccount {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[..4].copy_from_slice(&[16, 40, 8, 0]);
        OccupationResolutionAccount {
            market: hash(1),
            terms: hash(2),
            feed: hash(3),
            window: hash(4),
            feed_cursor: 140,
            sealed_end_bucket_exclusive: 130,
            repair_generation: 7,
            resolved_slot: 900,
            mode: RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            outcome_count: 4,
            resolved_value: 0,
            vector: PayoutVectorBytes {
                denominator: 64,
                weights,
            },
            archive_commitment: hash(5),
            statistic,
            finalization: finalization_for_statistic(statistic).unwrap(),
            basis_evaluator_version: OCCUPATION_BASIS_EVALUATOR_VERSION,
            occupation_summary_version: OCCUPATION_SUMMARY_VERSION,
            sample_count: 8,
            coverage_count: 8,
            gap_count: 0,
            stored_bump: 6,
            flags: 0,
            reserved: 0,
        }
    }

    #[test]
    fn unresolved_and_both_finalizers_round_trip_exactly() {
        let cases = [
            OccupationResolutionAccount::unresolved(hash(1), hash(2), hash(3), 6),
            resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06),
            resolved(STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07),
        ];
        for value in cases {
            value.validate().unwrap();
            let mut bytes = [0_u8; OCCUPATION_RESOLUTION_LEN];
            assert_eq!(value.encode(&mut bytes), Ok(OCCUPATION_RESOLUTION_LEN));
            assert_eq!(OccupationResolutionAccount::decode(&bytes), Ok(value));
        }
    }

    #[test]
    fn hostile_header_version_and_lengths_refuse() {
        let mut bytes = [0_u8; OCCUPATION_RESOLUTION_LEN + 1];
        resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06)
            .encode(&mut bytes)
            .unwrap();
        assert_eq!(
            OccupationResolutionAccount::decode(&bytes[..OCCUPATION_RESOLUTION_LEN - 1]),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            OccupationResolutionAccount::decode(&bytes),
            Err(CodecError::TrailingBytes)
        );
        let exact = &mut bytes[..OCCUPATION_RESOLUTION_LEN];
        exact[0] ^= 0x80;
        assert_eq!(
            OccupationResolutionAccount::decode(exact),
            Err(CodecError::WrongTag)
        );
        exact[0] = RESOLUTION_TAG;
        exact[1] = 3;
        assert_eq!(
            OccupationResolutionAccount::decode(exact),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06)
                .encode(&mut [0_u8; OCCUPATION_RESOLUTION_LEN - 1]),
            Err(CodecError::OutputTooSmall)
        );
    }

    #[test]
    fn v4_never_overloads_preset_or_point_modes() {
        let mut value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.mode = 1;
        assert_eq!(value.validate(), Err(CodecError::InvalidEnum));
        value.mode = 2;
        assert_eq!(value.validate(), Err(CodecError::InvalidEnum));
        value.mode = RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION;
        value.resolved_value = 1;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value.resolved_value = 0;
        value.payout_index = 0;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
    }

    #[test]
    fn statistic_and_finalizer_are_one_to_one() {
        let mut value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.finalization = OCCUPATION_FINALIZATION_LARGEST_REMAINDER_V1;
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07);
        value.finalization = OCCUPATION_FINALIZATION_EXACT_ONLY;
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.statistic = 8;
        assert_eq!(value.validate(), Err(CodecError::InvalidEnum));
    }

    #[test]
    fn archive_versions_and_counts_are_not_advisory() {
        let mut value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.archive_commitment = Hash32::ZERO;
        assert_eq!(value.validate(), Err(CodecError::ZeroIdentity));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.basis_evaluator_version += 1;
        assert_eq!(value.validate(), Err(CodecError::WrongVersion));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.occupation_summary_version += 1;
        assert_eq!(value.validate(), Err(CodecError::WrongVersion));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.coverage_count -= 1;
        value.gap_count = 1;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
        value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        value.sample_count = 0;
        value.coverage_count = 0;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
    }

    #[test]
    fn unresolved_bytes_cannot_smuggle_resolution_facts() {
        let base = OccupationResolutionAccount::unresolved(hash(1), hash(2), hash(3), 6);
        let mut cases = [base; 8];
        cases[0].window = hash(4);
        cases[1].archive_commitment = hash(5);
        cases[2].statistic = STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06;
        cases[3].finalization = OCCUPATION_FINALIZATION_EXACT_ONLY;
        cases[4].basis_evaluator_version = OCCUPATION_BASIS_EVALUATOR_VERSION;
        cases[5].sample_count = 1;
        cases[6].outcome_count = 2;
        cases[7].vector.denominator = 1;
        for value in cases {
            assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        }
    }

    #[test]
    fn exact_offsets_and_reserved_tail_are_pinned() {
        let value = resolved(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        let mut bytes = [0_u8; OCCUPATION_RESOLUTION_LEN];
        value.encode(&mut bytes).unwrap();
        assert_eq!(&bytes[317..349], &hash(5).bytes());
        assert_eq!(&bytes[349..351], &6_u16.to_le_bytes());
        assert_eq!(bytes[351], OCCUPATION_FINALIZATION_EXACT_ONLY);
        assert_eq!(&bytes[352..354], &1_u16.to_le_bytes());
        assert_eq!(&bytes[354..356], &1_u16.to_le_bytes());
        assert_eq!(&bytes[356..364], &8_u64.to_le_bytes());
        assert_eq!(&bytes[364..372], &8_u64.to_le_bytes());
        assert_eq!(&bytes[372..380], &0_u64.to_le_bytes());
        assert_eq!(bytes[380], 6);
        bytes[382] = 1;
        assert_eq!(
            OccupationResolutionAccount::decode(&bytes),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
