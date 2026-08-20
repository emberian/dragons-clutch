//! Hostile-byte ownership for the bounded direct-selection authority chain.
//!
//! Version two Epoch bytes remain owned by [`super::EpochAccount`] and are
//! never accepted here.  This module allocates version three of the same Epoch
//! discriminator, adding immutable submission boundaries, and wraps the
//! policy crate's exact fixed bodies with account tag/version envelopes.  It
//! does not reinterpret, truncate, or duplicate any policy or candidate field.

use clutch_batch_policy_identity::direct_window_v1::{
    DirectCandidateV2, DirectCandidateWindowV1, DirectWindowErrorV1,
    DIRECT_CANDIDATE_ACCOUNT_BYTES, DIRECT_CANDIDATE_BODY_BYTES, DIRECT_WINDOW_ACCOUNT_BYTES,
    DIRECT_WINDOW_BODY_BYTES,
};

use super::{
    account_len, canonical_epoch_id, digest, put_header, CodecError, EpochAccount, Hash32, Reader,
    Result, Writer, EPOCH_PHASE_OPEN, EPOCH_TAG,
};

/// Version-three Epoch schema carrying immutable candidate-window boundaries.
pub const DIRECT_EPOCH_VERSION: u8 = 3;
/// Exact byte length of one [`DirectEpochV3Account`].
pub const DIRECT_EPOCH_BYTES: usize = account_len::EPOCH + 16;
/// Account tag of one full-width verified direct candidate.
pub const DIRECT_CANDIDATE_TAG: u8 = 20;
/// Account version of one full-width verified direct candidate.
pub const DIRECT_CANDIDATE_VERSION: u8 = 1;
/// Account tag of the direct candidate-set/window owner.
pub const DIRECT_WINDOW_TAG: u8 = 21;
/// Account version of the direct candidate-set/window owner.
pub const DIRECT_WINDOW_VERSION: u8 = 1;

/// Derive the one book identity admitted by the bounded direct profile.
pub fn canonical_direct_book_id(epoch: Hash32) -> Hash32 {
    digest(b"dragons-clutch:direct-book:v2", &[&epoch.0])
}

/// Derive the deterministic relation remainder seed for the direct profile.
pub const fn canonical_direct_remainder_seed(epoch: Hash32) -> u64 {
    u64::from_le_bytes([
        epoch.0[0], epoch.0[1], epoch.0[2], epoch.0[3], epoch.0[4], epoch.0[5], epoch.0[6],
        epoch.0[7],
    ])
}

/// Version-three Epoch with immutable half-open candidate-submission window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEpochV3Account {
    /// Every version-two field, in its exact original order and meaning.
    pub common: EpochAccount,
    /// First slot at which a candidate may be admitted.
    pub submission_opens_slot: u64,
    /// First slot at which submissions refuse and selection may occur.
    pub submission_closes_slot: u64,
}

impl DirectEpochV3Account {
    /// Validate the common Epoch relation plus the immutable direct schedule.
    pub fn validate(&self) -> Result<()> {
        self.common.validate()?;
        if self.common.book != canonical_direct_book_id(self.common.epoch)
            || self.common.remainder_seed != canonical_direct_remainder_seed(self.common.epoch)
            || self.submission_opens_slot >= self.submission_closes_slot
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly [`DIRECT_EPOCH_BYTES`] bytes under Epoch tag/version 3.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < DIRECT_EPOCH_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let e = &self.common;
        let mut w = Writer::new(out);
        put_header(&mut w, EPOCH_TAG, DIRECT_EPOCH_VERSION)?;
        w.hash(e.epoch)?;
        w.hash(e.market)?;
        w.hash(e.book)?;
        w.hash(e.terms)?;
        w.hash(e.price_grid)?;
        w.hash(e.policy)?;
        w.hash(e.order_set)?;
        w.hash(e.first_order_id)?;
        w.hash(e.last_order_id)?;
        w.u64(e.epoch_index)?;
        w.u32(e.relation_version)?;
        w.u64(e.price_scale)?;
        w.u64(e.remainder_seed)?;
        w.u16(e.owner_count)?;
        w.u16(e.page_count)?;
        w.u16(e.order_count)?;
        w.u8(e.outcome_count)?;
        w.u8(e.phase)?;
        w.u64(self.submission_opens_slot)?;
        w.u64(self.submission_closes_slot)?;
        w.u8(e.stored_bump)?;
        w.u8(e.flags)?;
        Ok(w.at)
    }

    /// Decode exactly Epoch tag/version 3. Version two fails closed.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(input, EPOCH_TAG, DIRECT_EPOCH_VERSION, DIRECT_EPOCH_BYTES)?;
        let epoch = r.hash()?;
        let common = EpochAccount {
            epoch,
            market: r.hash()?,
            book: r.hash()?,
            terms: r.hash()?,
            price_grid: r.hash()?,
            policy: r.hash()?,
            order_set: r.hash()?,
            first_order_id: r.hash()?,
            last_order_id: r.hash()?,
            epoch_index: r.u64()?,
            relation_version: r.u32()?,
            price_scale: r.u64()?,
            remainder_seed: r.u64()?,
            owner_count: r.u16()?,
            page_count: r.u16()?,
            order_count: r.u16()?,
            outcome_count: r.u8()?,
            phase: r.u8()?,
            stored_bump: 0,
            flags: 0,
        };
        let value = Self {
            common,
            submission_opens_slot: r.u64()?,
            submission_closes_slot: r.u64()?,
        };
        let mut value = value;
        value.common.stored_bump = r.u8()?;
        value.common.flags = r.u8()?;
        r.done()?;
        value.validate()?;
        Ok(value)
    }

    /// Construct the canonical empty/open direct Epoch.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        market: Hash32,
        terms: Hash32,
        price_grid: Hash32,
        policy: Hash32,
        epoch_index: u64,
        price_scale: u64,
        opens_slot: u64,
        closes_slot: u64,
        stored_bump: u8,
    ) -> Result<Self> {
        let epoch = canonical_epoch_id(market, epoch_index);
        let value = Self {
            common: EpochAccount {
                epoch,
                market,
                book: canonical_direct_book_id(epoch),
                terms,
                price_grid,
                policy,
                order_set: Hash32::ZERO,
                first_order_id: Hash32::ZERO,
                last_order_id: Hash32::ZERO,
                epoch_index,
                relation_version: super::RELATION_VERSION,
                price_scale,
                remainder_seed: canonical_direct_remainder_seed(epoch),
                owner_count: 2,
                page_count: 0,
                order_count: 0,
                outcome_count: 2,
                phase: EPOCH_PHASE_OPEN,
                stored_bump,
                flags: 0,
            },
            submission_opens_slot: opens_slot,
            submission_closes_slot: closes_slot,
        };
        value.validate()?;
        Ok(value)
    }
}

fn map_direct_error(error: DirectWindowErrorV1) -> CodecError {
    match error {
        DirectWindowErrorV1::WrongLength => CodecError::Truncated,
        DirectWindowErrorV1::ZeroIdentity => CodecError::ZeroIdentity,
        DirectWindowErrorV1::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        DirectWindowErrorV1::MismatchedBinding
        | DirectWindowErrorV1::NotDirect
        | DirectWindowErrorV1::Relation(_) => CodecError::MismatchedBinding,
        DirectWindowErrorV1::NonCanonical => CodecError::NonCanonicalPadding,
        DirectWindowErrorV1::BeforeOpen
        | DirectWindowErrorV1::SubmissionClosed
        | DirectWindowErrorV1::SelectionEarly
        | DirectWindowErrorV1::AlreadySelected
        | DirectWindowErrorV1::Replay => CodecError::InvalidEnum,
    }
}

/// Encode one exact full-width candidate account.
pub fn encode_direct_candidate(value: &DirectCandidateV2, out: &mut [u8]) -> Result<usize> {
    value.validate().map_err(map_direct_error)?;
    if out.len() < DIRECT_CANDIDATE_ACCOUNT_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    out[0] = DIRECT_CANDIDATE_TAG;
    out[1] = DIRECT_CANDIDATE_VERSION;
    value
        .encode_body(&mut out[2..DIRECT_CANDIDATE_ACCOUNT_BYTES])
        .map_err(map_direct_error)?;
    Ok(DIRECT_CANDIDATE_ACCOUNT_BYTES)
}

/// Decode exactly one full-width candidate account.
pub fn decode_direct_candidate(input: &[u8]) -> Result<DirectCandidateV2> {
    if input.len() < DIRECT_CANDIDATE_ACCOUNT_BYTES {
        return Err(CodecError::Truncated);
    }
    if input.len() > DIRECT_CANDIDATE_ACCOUNT_BYTES {
        return Err(CodecError::TrailingBytes);
    }
    if input[0] != DIRECT_CANDIDATE_TAG {
        return Err(CodecError::WrongTag);
    }
    if input[1] != DIRECT_CANDIDATE_VERSION {
        return Err(CodecError::WrongVersion);
    }
    DirectCandidateV2::decode_body(&input[2..2 + DIRECT_CANDIDATE_BODY_BYTES])
        .map_err(map_direct_error)
}

/// Encode one exact direct window account.
pub fn encode_direct_window(value: &DirectCandidateWindowV1, out: &mut [u8]) -> Result<usize> {
    value.validate().map_err(map_direct_error)?;
    if out.len() < DIRECT_WINDOW_ACCOUNT_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    out[0] = DIRECT_WINDOW_TAG;
    out[1] = DIRECT_WINDOW_VERSION;
    value
        .encode_body(&mut out[2..DIRECT_WINDOW_ACCOUNT_BYTES])
        .map_err(map_direct_error)?;
    Ok(DIRECT_WINDOW_ACCOUNT_BYTES)
}

/// Decode exactly one direct window account.
pub fn decode_direct_window(input: &[u8]) -> Result<DirectCandidateWindowV1> {
    if input.len() < DIRECT_WINDOW_ACCOUNT_BYTES {
        return Err(CodecError::Truncated);
    }
    if input.len() > DIRECT_WINDOW_ACCOUNT_BYTES {
        return Err(CodecError::TrailingBytes);
    }
    if input[0] != DIRECT_WINDOW_TAG {
        return Err(CodecError::WrongTag);
    }
    if input[1] != DIRECT_WINDOW_VERSION {
        return Err(CodecError::WrongVersion);
    }
    DirectCandidateWindowV1::decode_body(&input[2..2 + DIRECT_WINDOW_BODY_BYTES])
        .map_err(map_direct_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateRecord, CANDIDATE_STATUS_SUBMITTED, MAX_OUTCOMES};
    use clutch_batch_policy_identity::{
        direct_window_v1::{
            canonical_account_candidate_id, DirectCandidateEntryV1, DirectCandidateV2,
            DirectCandidateWindowV1, DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_WINDOW_PHASE_OPEN,
            MAX_DIRECT_CANDIDATES,
        },
        Identity32V1,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    #[test]
    fn direct_epoch_v3_roundtrips_and_v2_refuses() {
        let value =
            DirectEpochV3Account::open(h(1), h(2), h(3), h(4), 7, 10_000, 100, 120, 9).unwrap();
        let mut bytes = [0u8; DIRECT_EPOCH_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(DIRECT_EPOCH_BYTES));
        assert_eq!(DirectEpochV3Account::decode(&bytes), Ok(value));
        assert_eq!(EpochAccount::decode(&bytes), Err(CodecError::TrailingBytes));
        bytes[1] = super::super::account_version::EPOCH;
        assert_eq!(
            DirectEpochV3Account::decode(&bytes),
            Err(CodecError::WrongVersion)
        );
    }

    #[test]
    fn schedule_and_direct_identity_are_not_caller_slack() {
        let mut value =
            DirectEpochV3Account::open(h(1), h(2), h(3), h(4), 7, 10_000, 100, 120, 9).unwrap();
        value.submission_opens_slot = 120;
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
        value.submission_opens_slot = 100;
        value.common.book = h(8);
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn policy_bridge_and_layout_candidate_identity_are_byte_exact() {
        let epoch = h(3);
        let market = h(1);
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 2_500;
        prices[1] = 7_500;
        let mut layout = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch,
            market,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 0,
            limit_surplus_price_units: 0,
            score_digest: Hash32::ZERO,
            churn: 0,
            submitted_slot: 100,
            distinct_owners: 0,
            order_len: 2,
            outcome_count: 2,
            status: CANDIDATE_STATUS_SUBMITTED,
            stored_bump: 9,
            flags: 0,
        };
        layout.candidate = layout.recomputed_candidate_digest().unwrap();
        let bridged = canonical_account_candidate_id(
            Identity32V1(epoch.bytes()),
            Identity32V1(market.bytes()),
            &prices,
        );
        assert_eq!(layout.candidate.bytes(), bridged.0);
    }

    #[test]
    fn refused_candidate_and_window_encodes_do_not_touch_output() {
        let invalid_candidate = DirectCandidateV2 {
            candidate_id: Identity32V1::ZERO,
            epoch_id: Identity32V1::ZERO,
            market_id: Identity32V1::ZERO,
            order_set_id: Identity32V1::ZERO,
            policy_id: Identity32V1::ZERO,
            relation_domain_digest: Identity32V1::ZERO,
            relation_candidate_digest: Identity32V1::ZERO,
            prices: [0; MAX_OUTCOMES],
            fills: [0; 2],
            weighted_direct_volume: 0,
            limit_surplus_price_units: 0,
            submitted_slot: 0,
            quantity: 0,
            buy_index: 0,
            sell_index: 0,
            outcome: 0,
            distinct_owners: 0,
            order_len: 0,
            outcome_count: 0,
            status: DIRECT_CANDIDATE_STATUS_VERIFIED,
            stored_bump: 0,
            flags: 0,
            reserved: [0; 12],
        };
        let mut candidate_bytes = [0xa5; DIRECT_CANDIDATE_ACCOUNT_BYTES];
        assert!(encode_direct_candidate(&invalid_candidate, &mut candidate_bytes).is_err());
        assert_eq!(candidate_bytes, [0xa5; DIRECT_CANDIDATE_ACCOUNT_BYTES]);

        let invalid_window = DirectCandidateWindowV1 {
            epoch_id: Identity32V1::ZERO,
            market_id: Identity32V1::ZERO,
            order_set_id: Identity32V1::ZERO,
            policy_id: Identity32V1::ZERO,
            relation_domain_digest: Identity32V1::ZERO,
            admission_transcript: Identity32V1::ZERO,
            selected_candidate: Identity32V1::ZERO,
            top: [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES],
            opens_slot: 0,
            closes_slot: 0,
            selected_slot: 0,
            admitted_count: 0,
            top_count: 0,
            phase: DIRECT_WINDOW_PHASE_OPEN,
            stored_bump: 0,
            flags: 0,
            reserved: [0; 2],
        };
        let mut window_bytes = [0x5a; DIRECT_WINDOW_ACCOUNT_BYTES];
        assert!(encode_direct_window(&invalid_window, &mut window_bytes).is_err());
        assert_eq!(window_bytes, [0x5a; DIRECT_WINDOW_ACCOUNT_BYTES]);
    }
}
