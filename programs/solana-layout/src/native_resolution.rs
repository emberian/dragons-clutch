//! Version-three resolution persistence for native derived payout vectors.
//!
//! This module is intentionally isolated until the shared layout steward
//! replaces the version-two index-only [`super::ResolutionAccount`]. It owns
//! the proposed version-three wire shape, not an additional live account.

use super::{
    CodecError, Hash32, MarketAccount, PayoutVectorBytes, Result, TermsAccount, MAX_OUTCOMES,
    MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, RESOLUTION_TAG,
};

/// Version of the native-vector resolution account wire shape.
pub const NATIVE_RESOLUTION_VERSION: u8 = 3;
/// Exact byte length of [`NativeResolutionAccount`]'s canonical wire shape.
pub const NATIVE_RESOLUTION_LEN: usize = 319;

/// No resolution fact has been admitted.
pub const RESOLUTION_MODE_UNRESOLVED: u8 = 0;
/// The selected vector is owned by the immutable terms payout set.
pub const RESOLUTION_MODE_PRESET: u8 = 1;
/// The exact vector below was natively derived at one evidence point.
pub const RESOLUTION_MODE_DERIVED_POINT: u8 = 2;

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
const OFFSET_STORED_BUMP: usize = OFFSET_WEIGHTS + (MAX_OUTCOMES * 8);
const OFFSET_FLAGS: usize = OFFSET_STORED_BUMP + 1;

const _: () = assert!(OFFSET_FLAGS + 1 == NATIVE_RESOLUTION_LEN);

/// Immutable resolution fact with one persisted owner for a native vector.
///
/// In preset mode, immutable terms own the vector and this account stores only
/// its index. In derived-point mode, this account owns the denominator and
/// weights; no persisted kernel account may copy them. `resolved_value` is the
/// exact pre-edge-handling integer statistic point authenticated by the bound
/// window. The resolution instruction must recompute the vector from that
/// point and the bound terms before writing this account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeResolutionAccount {
    /// Market identity.
    pub market: Hash32,
    /// Immutable terms digest.
    pub terms: Hash32,
    /// Feed identity selected by the terms.
    pub feed: Hash32,
    /// Digest of the exact sealed window result.
    pub window: Hash32,
    /// Authenticated feed cursor witnessed at seal.
    pub feed_cursor: u64,
    /// Exclusive end bucket of the exact sealed window.
    pub sealed_end_bucket_exclusive: u64,
    /// Repair generation of the sealed window.
    pub repair_generation: u64,
    /// Slot at which this immutable fact was recorded.
    pub resolved_slot: u64,
    /// One of the registered `RESOLUTION_MODE_*` values.
    pub mode: u8,
    /// Terms payout index in preset mode; unresolved sentinel otherwise.
    pub payout_index: u8,
    /// Active vector prefix in derived mode; zero otherwise.
    pub outcome_count: u8,
    /// Exact integer statistic point in derived mode; zero otherwise.
    pub resolved_value: u128,
    /// Native vector in derived mode; canonical all-zero padding otherwise.
    pub vector: PayoutVectorBytes,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl NativeResolutionAccount {
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
        stored_bump: 0,
        flags: 0,
    };

    /// Canonical unresolved state for a freshly founded market.
    pub const fn unresolved(market: Hash32, terms: Hash32, feed: Hash32, stored_bump: u8) -> Self {
        Self {
            market,
            terms,
            feed,
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
            stored_bump,
            flags: 0,
        }
    }

    /// Whether this account carries an immutable resolution fact.
    pub const fn is_resolved(&self) -> bool {
        self.mode != RESOLUTION_MODE_UNRESOLVED
    }

    /// Validate identities, mode discipline, vector shape, and padding.
    pub fn validate(&self) -> Result<()> {
        check_required_hash(self.market)?;
        check_required_hash(self.terms)?;
        check_required_hash(self.feed)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        match self.mode {
            RESOLUTION_MODE_UNRESOLVED => self.validate_unresolved(),
            RESOLUTION_MODE_PRESET => self.validate_preset(),
            RESOLUTION_MODE_DERIVED_POINT => self.validate_derived(),
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
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(())
    }

    fn validate_resolved_header(&self) -> Result<()> {
        check_required_hash(self.window)?;
        if self.sealed_end_bucket_exclusive == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.feed_cursor < self.sealed_end_bucket_exclusive {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_preset(&self) -> Result<()> {
        self.validate_resolved_header()?;
        if usize::from(self.payout_index) >= MAX_PAYOUTS {
            return Err(CodecError::InvalidCount);
        }
        if self.outcome_count != 0
            || self.resolved_value != 0
            || self.vector != PayoutVectorBytes::ZERO
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(())
    }

    fn validate_derived(&self) -> Result<()> {
        self.validate_resolved_header()?;
        if self.payout_index != PAYOUT_INDEX_UNRESOLVED {
            return Err(CodecError::NonCanonicalPadding);
        }
        if !(2..=MAX_OUTCOMES as u8).contains(&self.outcome_count) {
            return Err(CodecError::InvalidCount);
        }
        self.vector
            .validate_active(self.outcome_count, self.vector.denominator)
    }

    /// Binding comparisons against already-validated immutable terms.
    pub fn binds_terms_fields(&self, terms: &TermsAccount) -> Result<()> {
        if self.terms != terms.terms || self.feed != terms.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if self.is_resolved() {
            let maturity = terms
                .expected_start_bucket
                .checked_add(terms.maturity_horizon_buckets)
                .ok_or(CodecError::ArithmeticOverflow)?;
            if self.sealed_end_bucket_exclusive != terms.expected_end_bucket_exclusive
                || self.repair_generation != terms.repair_generation
                || self.feed_cursor < maturity
            {
                return Err(CodecError::MismatchedBinding);
            }
        }
        match self.mode {
            RESOLUTION_MODE_UNRESOLVED => Ok(()),
            RESOLUTION_MODE_PRESET => {
                if terms.basis_degree != 0 || self.payout_index >= terms.payout_count {
                    return Err(CodecError::MismatchedBinding);
                }
                Ok(())
            }
            RESOLUTION_MODE_DERIVED_POINT => {
                if !(1..=3).contains(&terms.basis_degree)
                    || self.outcome_count != terms.outcome_count
                    || self.vector.denominator != terms.payouts[0].denominator
                {
                    return Err(CodecError::MismatchedBinding);
                }
                Ok(())
            }
            _ => Err(CodecError::InvalidEnum),
        }
    }

    /// Validate and bind this record to immutable terms.
    pub fn binds_terms(&self, terms: &TermsAccount) -> Result<()> {
        self.validate()?;
        terms.validate()?;
        self.binds_terms_fields(terms)
    }

    /// Binding comparisons against an already-validated market account.
    pub fn binds_market_fields(&self, market: &MarketAccount) -> Result<()> {
        if self.market != market.market || self.terms != market.terms || self.feed != market.feed {
            return Err(CodecError::MismatchedBinding);
        }
        if (self.is_resolved() && market.lifecycle != 1)
            || (!self.is_resolved() && market.lifecycle != 0)
        {
            return Err(CodecError::MismatchedBinding);
        }
        if self.mode == RESOLUTION_MODE_DERIVED_POINT && self.outcome_count != market.outcome_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Validate the complete market/terms/account join.
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

    /// Reconstruct the exact effective payout vector from one semantic owner.
    ///
    /// Unresolved returns `None`. Preset mode reads the terms-owned member;
    /// derived mode returns the record-owned vector. The returned value is an
    /// ephemeral kernel input and must not be persisted into another account.
    pub fn effective_vector(&self, terms: &TermsAccount) -> Result<Option<PayoutVectorBytes>> {
        self.binds_terms(terms)?;
        match self.mode {
            RESOLUTION_MODE_UNRESOLVED => Ok(None),
            RESOLUTION_MODE_PRESET => Ok(Some(terms.payouts[usize::from(self.payout_index)])),
            RESOLUTION_MODE_DERIVED_POINT => Ok(Some(self.vector)),
            _ => Err(CodecError::InvalidEnum),
        }
    }

    /// Encode exactly [`NATIVE_RESOLUTION_LEN`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < NATIVE_RESOLUTION_LEN {
            return Err(CodecError::OutputTooSmall);
        }
        out[0] = RESOLUTION_TAG;
        out[1] = NATIVE_RESOLUTION_VERSION;
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
        out[OFFSET_STORED_BUMP] = self.stored_bump;
        out[OFFSET_FLAGS] = self.flags;
        Ok(NATIVE_RESOLUTION_LEN)
    }

    /// Parse exactly [`NATIVE_RESOLUTION_LEN`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::ZEROED;
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }

    /// Parse hostile bytes directly into a caller-owned slot.
    ///
    /// On refusal, `out` may contain a partial value and must not be read.
    /// This is the SBF-facing seam: it avoids a second account-sized return
    /// temporary when the caller already owns storage for the decoded record.
    pub fn decode_into(input: &[u8], out: &mut Self) -> Result<()> {
        if input.len() < NATIVE_RESOLUTION_LEN {
            return Err(CodecError::Truncated);
        }
        if input.len() > NATIVE_RESOLUTION_LEN {
            return Err(CodecError::TrailingBytes);
        }
        if input[0] != RESOLUTION_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != NATIVE_RESOLUTION_VERSION {
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
        out.stored_bump = input[OFFSET_STORED_BUMP];
        out.flags = input[OFFSET_FLAGS];
        out.validate()
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

    fn preset() -> NativeResolutionAccount {
        NativeResolutionAccount {
            market: hash(1),
            terms: hash(2),
            feed: hash(3),
            window: hash(4),
            feed_cursor: 140,
            sealed_end_bucket_exclusive: 130,
            repair_generation: 7,
            resolved_slot: 900,
            mode: RESOLUTION_MODE_PRESET,
            payout_index: 1,
            outcome_count: 0,
            resolved_value: 0,
            vector: PayoutVectorBytes::ZERO,
            stored_bump: 5,
            flags: 0,
        }
    }

    fn derived() -> NativeResolutionAccount {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[..4].copy_from_slice(&[16, 40, 8, 0]);
        NativeResolutionAccount {
            mode: RESOLUTION_MODE_DERIVED_POINT,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            outcome_count: 4,
            resolved_value: 2,
            vector: PayoutVectorBytes {
                denominator: 64,
                weights,
            },
            ..preset()
        }
    }

    #[test]
    fn all_three_modes_round_trip_exactly() {
        let cases = [
            NativeResolutionAccount::unresolved(hash(1), hash(2), hash(3), 5),
            preset(),
            derived(),
        ];
        for value in cases {
            value.validate().unwrap();
            let mut bytes = [0_u8; NATIVE_RESOLUTION_LEN];
            assert_eq!(value.encode(&mut bytes), Ok(NATIVE_RESOLUTION_LEN));
            assert_eq!(NativeResolutionAccount::decode(&bytes), Ok(value));
        }
    }

    #[test]
    fn hostile_header_and_length_refuse() {
        let mut bytes = [0_u8; NATIVE_RESOLUTION_LEN + 1];
        preset().encode(&mut bytes).unwrap();
        assert_eq!(
            NativeResolutionAccount::decode(&bytes[..NATIVE_RESOLUTION_LEN - 1]),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            NativeResolutionAccount::decode(&bytes),
            Err(CodecError::TrailingBytes)
        );
        let mut exact = [0_u8; NATIVE_RESOLUTION_LEN];
        preset().encode(&mut exact).unwrap();
        exact[0] ^= 0x80;
        assert_eq!(
            NativeResolutionAccount::decode(&exact),
            Err(CodecError::WrongTag)
        );
        exact[0] = RESOLUTION_TAG;
        exact[1] = 2;
        assert_eq!(
            NativeResolutionAccount::decode(&exact),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            preset().encode(&mut [0_u8; NATIVE_RESOLUTION_LEN - 1]),
            Err(CodecError::OutputTooSmall)
        );
    }

    #[test]
    fn unresolved_and_preset_padding_is_canonical() {
        let mut unresolved = NativeResolutionAccount::unresolved(hash(1), hash(2), hash(3), 5);
        unresolved.resolved_value = 1;
        assert_eq!(unresolved.validate(), Err(CodecError::NonCanonicalPadding));

        let mut value = preset();
        value.outcome_count = 2;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value = preset();
        value.vector.denominator = 1;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value = preset();
        value.payout_index = MAX_PAYOUTS as u8;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
    }

    #[test]
    fn derived_vector_shape_and_padding_refuse_hostile_values() {
        let mut value = derived();
        value.payout_index = 0;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value = derived();
        value.outcome_count = 1;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
        value = derived();
        value.vector.weights[4] = 1;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));
        value = derived();
        value.vector.weights[0] -= 1;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
        value = derived();
        value.vector.denominator = 0;
        assert_eq!(value.validate(), Err(CodecError::ZeroValue));
    }

    #[test]
    fn resolved_window_header_is_not_a_bare_assertion() {
        let mut value = derived();
        value.window = Hash32::ZERO;
        assert_eq!(value.validate(), Err(CodecError::ZeroIdentity));
        value = derived();
        value.sealed_end_bucket_exclusive = 0;
        assert_eq!(value.validate(), Err(CodecError::ZeroValue));
        value = derived();
        value.feed_cursor = value.sealed_end_bucket_exclusive - 1;
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn zero_is_a_valid_native_resolution_point() {
        let mut value = derived();
        value.resolved_value = 0;
        assert_eq!(value.validate(), Ok(()));
    }
}
