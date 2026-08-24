// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transient, rent-owned owner-fee assessment work.
//!
//! Action 24 cannot name every Reservation belonging to a maximally wide
//! owner under Solana's deployed 64-account ceiling.  This account is the
//! sole temporary semantic owner of the page-by-page assessment transcript.
//! It records no page projection and authorizes no debit by itself: the SBF
//! adapter must authenticate each immutable page and its exact ReservationV9
//! envelopes before advancing the cursor, then reauthenticate the complete
//! retained traversal before sealing and consuming the work.

use clutch_fee_runtime_contract::allocation::{FeeEnvelopeFundingV1, FeeEnvelopeV1};
use clutch_fee_runtime_contract::{Id as FeeId, MAX_FEE_ROWS_V1};

use crate::{CodecError, DeletableRentOwnerV1, Id32, Reader, Sha256BackendV1, Writer};

/// Fresh global discriminator for transient owner-fee assessment work.
pub const OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_TAG: u8 = 0xbe;
/// First and sole current assessment-work version.
pub const OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_VERSION: u8 = 1;
/// Canonical owner-scoped assessment-work PDA seed.
pub const OWNER_FEE_ASSESSMENT_WORK_SEED_DOMAIN_V1: &[u8] = b"owner-fee-assess:v1";
/// Full-data identity domain for one exact work-account body.
pub const OWNER_FEE_ASSESSMENT_WORK_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/owner-fee-assessment-work-data/v1\0";

const AUTHORITY_ID_COUNT: usize = 15;
const AUTHORITY_BYTES: usize = AUTHORITY_ID_COUNT * 32;
const CURSOR_BYTES: usize = 8;
const MASK_BYTES: usize = 16;
const CERTIFIED_ASSESSMENT_BYTES: usize = 24;
const NEUTRAL_SINK_BYTES: usize = 32;
const ENVELOPE_ROW_BYTES: usize = 32 + 1 + 1 + 8;
/// Exact semantic body width, independent of active row count.
pub const OWNER_FEE_ASSESSMENT_WORK_BODY_BYTES_V1: usize = AUTHORITY_BYTES
    + CURSOR_BYTES
    + MASK_BYTES
    + CERTIFIED_ASSESSMENT_BYTES
    + NEUTRAL_SINK_BYTES
    + MAX_FEE_ROWS_V1 * ENVELOPE_ROW_BYTES;
/// Exact rent-owned Solana account width.
pub const OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1: usize =
    2 + OWNER_FEE_ASSESSMENT_WORK_BODY_BYTES_V1 + 48 + 2;

const WORK_PHASE_COLLECTING: u8 = 1;
const WORK_PHASE_READY: u8 = 2;

/// Immutable authority copied from already-authenticated chain state when the
/// transient work account is created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAssessmentAuthorityV1 {
    /// Exact counted SettlementRoot account.
    pub settlement_root_account: Id32,
    /// Current selected-fee record account.
    pub selected_fee_record_account: Id32,
    /// Full current selected-fee account data identity.
    pub selected_fee_record_data_id: Id32,
    /// Immutable Realm identity.
    pub realm: Id32,
    /// RevenuePolicyRecordV2 account.
    pub revenue_policy_record_account: Id32,
    /// Semantic RevenuePolicyRecordV2 identity.
    pub revenue_policy_record_v2_id: Id32,
    /// Exact RevenuePolicyV2 digest.
    pub revenue_policy_v2_digest: Id32,
    /// Retained sealed Feed account.
    pub retained_feed_account: Id32,
    /// Full retained Feed byte identity.
    pub retained_feed_data_id: Id32,
    /// Owner derived from the current root/Feed/index/page chain.
    pub owner: Id32,
    /// Canonical fresh owner-settlement row.
    pub owner_row_account: Id32,
    /// Canonical General MarketRuntime account.
    pub market: Id32,
    /// Exact General Epoch identity.
    pub epoch: Id32,
    /// Frozen order-set identity.
    pub order_set: Id32,
    /// Complete immutable owner/order-set transcript digest.
    pub owner_order_set_digest: Id32,
}

impl OwnerFeeAssessmentAuthorityV1 {
    fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.settlement_root_account,
            self.selected_fee_record_account,
            self.selected_fee_record_data_id,
            self.realm,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.revenue_policy_v2_digest,
            self.retained_feed_account,
            self.retained_feed_data_id,
            self.owner,
            self.owner_row_account,
            self.market,
            self.epoch,
            self.order_set,
            self.owner_order_set_digest,
        ] {
            if id.is_zero() {
                return Err(CodecError::ZeroIdentity);
            }
        }
        Ok(())
    }

    fn encode(&self, writer: &mut Writer<'_>) -> Result<(), CodecError> {
        for id in [
            self.settlement_root_account,
            self.selected_fee_record_account,
            self.selected_fee_record_data_id,
            self.realm,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.revenue_policy_v2_digest,
            self.retained_feed_account,
            self.retained_feed_data_id,
            self.owner,
            self.owner_row_account,
            self.market,
            self.epoch,
            self.order_set,
            self.owner_order_set_digest,
        ] {
            writer.bytes(&id.bytes())?;
        }
        Ok(())
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        let authority = Self {
            settlement_root_account: Id32::new(reader.array()?)?,
            selected_fee_record_account: Id32::new(reader.array()?)?,
            selected_fee_record_data_id: Id32::new(reader.array()?)?,
            realm: Id32::new(reader.array()?)?,
            revenue_policy_record_account: Id32::new(reader.array()?)?,
            revenue_policy_record_v2_id: Id32::new(reader.array()?)?,
            revenue_policy_v2_digest: Id32::new(reader.array()?)?,
            retained_feed_account: Id32::new(reader.array()?)?,
            retained_feed_data_id: Id32::new(reader.array()?)?,
            owner: Id32::new(reader.array()?)?,
            owner_row_account: Id32::new(reader.array()?)?,
            market: Id32::new(reader.array()?)?,
            epoch: Id32::new(reader.array()?)?,
            order_set: Id32::new(reader.array()?)?,
            owner_order_set_digest: Id32::new(reader.array()?)?,
        };
        authority.validate()?;
        Ok(authority)
    }
}

/// One page-authenticated signed Reservation envelope retained until the
/// complete traversal is reauthenticated and the payer allocation is minted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAssessmentEnvelopeV1 {
    order_index: u8,
    funding: FeeEnvelopeFundingV1,
    intent: Id32,
    max_fee_atoms: u64,
}

impl OwnerFeeAssessmentEnvelopeV1 {
    /// Bind one exact Reservation envelope to its dense frozen order index.
    pub fn new(
        expected_owner: Id32,
        order_index: u8,
        envelope: FeeEnvelopeV1,
    ) -> Result<Self, CodecError> {
        if envelope.owner.0 != expected_owner.bytes()
            || envelope.intent.0 == [0; 32]
            || envelope.debited_atoms != 0
            || (envelope.funding == FeeEnvelopeFundingV1::NoCashReservation
                && envelope.max_fee_atoms != 0)
        {
            return Err(CodecError::InvalidState);
        }
        Ok(Self {
            order_index,
            funding: envelope.funding,
            intent: Id32::new(envelope.intent.0)?,
            max_fee_atoms: envelope.max_fee_atoms,
        })
    }

    /// Dense frozen order index.
    pub const fn order_index(&self) -> u8 { self.order_index }
    /// Canonical Reservation identity.
    pub const fn intent(&self) -> Id32 { self.intent }
    /// Signed fee cap from ReservationV9.
    pub const fn max_fee_atoms(&self) -> u64 { self.max_fee_atoms }
    /// Exact payer-funding classification.
    pub const fn funding(&self) -> FeeEnvelopeFundingV1 { self.funding }

    /// Recreate the fee runtime's exact envelope without caller-supplied data.
    pub fn fee_envelope(&self, owner: Id32) -> FeeEnvelopeV1 {
        FeeEnvelopeV1 {
            owner: FeeId(owner.bytes()),
            intent: FeeId(self.intent.bytes()),
            funding: self.funding,
            max_fee_atoms: self.max_fee_atoms,
            debited_atoms: 0,
        }
    }
}

const EMPTY_ENVELOPE: OwnerFeeAssessmentEnvelopeV1 = OwnerFeeAssessmentEnvelopeV1 {
    order_index: 0,
    funding: FeeEnvelopeFundingV1::NoCashReservation,
    intent: Id32::ZERO,
    max_fee_atoms: 0,
};

/// Mutable semantic body of one transient per-owner assessment.
#[derive(Debug, Eq, PartialEq)]
pub struct OwnerFeeAssessmentWorkV1 {
    authority: OwnerFeeAssessmentAuthorityV1,
    next_page: u8,
    page_count: u8,
    order_count: u8,
    envelope_count: u8,
    phase: u8,
    processed_buy_mask: u64,
    processed_sell_mask: u64,
    exact_weight_numerator: u128,
    charged_atoms: u64,
    neutral_sink: Id32,
    envelopes: [OwnerFeeAssessmentEnvelopeV1; MAX_FEE_ROWS_V1],
}

impl OwnerFeeAssessmentWorkV1 {
    /// Start the canonical page-zero cursor. No caller fee, weight, row count,
    /// or envelope is accepted here.
    pub fn begin(
        authority: OwnerFeeAssessmentAuthorityV1,
        page_count: u8,
        order_count: u8,
        neutral_sink: Id32,
    ) -> Result<Self, CodecError> {
        authority.validate()?;
        if page_count == 0 || page_count > 4 || order_count == 0 || order_count > 64 {
            return Err(CodecError::InvalidCount);
        }
        if neutral_sink.is_zero()
            || neutral_sink == authority.owner
            || neutral_sink == authority.owner_row_account
            || neutral_sink == authority.settlement_root_account
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            authority,
            next_page: 0,
            page_count,
            order_count,
            envelope_count: 0,
            phase: WORK_PHASE_COLLECTING,
            processed_buy_mask: 0,
            processed_sell_mask: 0,
            exact_weight_numerator: 0,
            charged_atoms: 0,
            neutral_sink,
            envelopes: [EMPTY_ENVELOPE; MAX_FEE_ROWS_V1],
        })
    }

    /// Immutable source binding.
    pub const fn authority(&self) -> OwnerFeeAssessmentAuthorityV1 { self.authority }
    /// Only page index admitted by the next continuation.
    pub const fn next_page(&self) -> u8 { self.next_page }
    /// Canonical frozen page count.
    pub const fn page_count(&self) -> u8 { self.page_count }
    /// Dense live-order count.
    pub const fn order_count(&self) -> u8 { self.order_count }
    /// Number of retained signed envelopes.
    pub const fn envelope_count(&self) -> u8 { self.envelope_count }
    /// Exact processed owner buy-order mask.
    pub const fn processed_buy_mask(&self) -> u64 { self.processed_buy_mask }
    /// Exact processed owner sell-order mask.
    pub const fn processed_sell_mask(&self) -> u64 { self.processed_sell_mask }
    /// Certified owner-netted composite numerator, zero until sealed.
    pub const fn exact_weight_numerator(&self) -> u128 { self.exact_weight_numerator }
    /// Certified terminal-ceil charge, zero until sealed.
    pub const fn charged_atoms(&self) -> u64 { self.charged_atoms }
    /// Donation recipient fixed at work creation.
    pub const fn neutral_sink(&self) -> Id32 { self.neutral_sink }
    /// Whether the complete traversal has certified and sealed this work.
    pub const fn is_ready(&self) -> bool { self.phase == WORK_PHASE_READY }
    /// One retained envelope by canonical Reservation identity.
    pub fn envelope(&self, index: u8) -> Option<OwnerFeeAssessmentEnvelopeV1> {
        self.envelopes.get(usize::from(index)).copied()
            .filter(|_| index < self.envelope_count)
    }

    /// Append one page-authenticated envelope. The adapter may call this only
    /// while processing `next_page` and must not persist until `finish_page`.
    pub fn record_page_envelope(
        &mut self,
        page_index: u8,
        envelope: OwnerFeeAssessmentEnvelopeV1,
    ) -> Result<(), CodecError> {
        if self.phase != WORK_PHASE_COLLECTING
            || page_index != self.next_page
            || envelope.order_index >= self.order_count
        {
            return Err(CodecError::InvalidState);
        }
        let bit = 1u64
            .checked_shl(u32::from(envelope.order_index))
            .ok_or(CodecError::ArithmeticOverflow)?;
        let (target_mask, other_mask) = match envelope.funding {
            FeeEnvelopeFundingV1::BuyCashReservation => {
                (&mut self.processed_buy_mask, self.processed_sell_mask)
            }
            FeeEnvelopeFundingV1::NoCashReservation => {
                (&mut self.processed_sell_mask, self.processed_buy_mask)
            }
        };
        if (*target_mask | other_mask) & bit != 0 {
            return Err(CodecError::InvalidState);
        }
        let len = usize::from(self.envelope_count);
        if len >= MAX_FEE_ROWS_V1 {
            return Err(CodecError::InvalidCount);
        }
        let mut insert = len;
        while insert > 0 && self.envelopes[insert - 1].intent > envelope.intent {
            self.envelopes[insert] = self.envelopes[insert - 1];
            insert -= 1;
        }
        if insert > 0 && self.envelopes[insert - 1].intent == envelope.intent {
            return Err(CodecError::MismatchedBinding);
        }
        if insert < len && self.envelopes[insert].intent == envelope.intent {
            return Err(CodecError::MismatchedBinding);
        }
        self.envelopes[insert] = envelope;
        self.envelope_count = self.envelope_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        *target_mask |= bit;
        Ok(())
    }

    /// Commit one completely processed canonical page. Replaying or skipping
    /// a page is impossible because the cursor advances exactly once.
    pub fn finish_page(&mut self, page_index: u8) -> Result<(), CodecError> {
        if self.phase != WORK_PHASE_COLLECTING || page_index != self.next_page {
            return Err(CodecError::InvalidState);
        }
        self.next_page = self.next_page
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        if self.next_page > self.page_count {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Seal only after the complete traversal reproduces the exact owner masks
    /// and exact owner-netted CompositeDispersionFloor assessment.
    pub fn seal(
        &mut self,
        expected_buy_mask: u64,
        expected_sell_mask: u64,
        exact_weight_numerator: u128,
        charged_atoms: u64,
    ) -> Result<(), CodecError> {
        if self.phase != WORK_PHASE_COLLECTING
            || self.next_page != self.page_count
            || expected_buy_mask != self.processed_buy_mask
            || expected_sell_mask != self.processed_sell_mask
            || expected_buy_mask & expected_sell_mask != 0
            || (expected_buy_mask | expected_sell_mask).count_ones()
                != u32::from(self.envelope_count)
            || self.envelope_count == 0
            || (charged_atoms != 0 && expected_buy_mask == 0)
        {
            return Err(CodecError::MismatchedBinding);
        }
        self.exact_weight_numerator = exact_weight_numerator;
        self.charged_atoms = charged_atoms;
        self.phase = WORK_PHASE_READY;
        Ok(())
    }

    fn validate(&self) -> Result<(), CodecError> {
        self.authority.validate()?;
        if self.page_count == 0
            || self.page_count > 4
            || self.next_page > self.page_count
            || self.order_count == 0
            || self.order_count > 64
            || (self.envelope_count == 0 && self.next_page == self.page_count)
            || self.processed_buy_mask & self.processed_sell_mask != 0
            || (self.processed_buy_mask | self.processed_sell_mask).count_ones()
                != u32::from(self.envelope_count)
            || self.neutral_sink.is_zero()
        {
            return Err(CodecError::InvalidState);
        }
        match self.phase {
            WORK_PHASE_COLLECTING => {
                if self.exact_weight_numerator != 0 || self.charged_atoms != 0 {
                    return Err(CodecError::InvalidState);
                }
            }
            WORK_PHASE_READY => {
                if self.next_page != self.page_count
                    || (self.charged_atoms != 0 && self.processed_buy_mask == 0)
                {
                    return Err(CodecError::InvalidState);
                }
            }
            _ => return Err(CodecError::InvalidState),
        }
        let mut prior = None;
        let mut index = 0usize;
        while index < MAX_FEE_ROWS_V1 {
            let row = self.envelopes[index];
            if index < usize::from(self.envelope_count) {
                if row.intent.is_zero() || row.order_index >= self.order_count {
                    return Err(CodecError::InvalidState);
                }
                if prior.is_some_and(|value| value >= row.intent) {
                    return Err(CodecError::MismatchedBinding);
                }
                let bit = 1u64 << row.order_index;
                match row.funding {
                    FeeEnvelopeFundingV1::BuyCashReservation
                        if self.processed_buy_mask & bit != 0 => {}
                    FeeEnvelopeFundingV1::NoCashReservation
                        if self.processed_sell_mask & bit != 0 && row.max_fee_atoms == 0 => {}
                    _ => return Err(CodecError::MismatchedBinding),
                }
                prior = Some(row.intent);
            } else if row != EMPTY_ENVELOPE {
                return Err(CodecError::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    fn encode(&self, writer: &mut Writer<'_>) -> Result<(), CodecError> {
        self.validate()?;
        self.authority.encode(writer)?;
        writer.u8(self.next_page)?;
        writer.u8(self.page_count)?;
        writer.u8(self.order_count)?;
        writer.u8(self.envelope_count)?;
        writer.u8(self.phase)?;
        writer.bytes(&[0; 3])?;
        writer.u64(self.processed_buy_mask)?;
        writer.u64(self.processed_sell_mask)?;
        writer.u128(self.exact_weight_numerator)?;
        writer.u64(self.charged_atoms)?;
        writer.bytes(&self.neutral_sink.bytes())?;
        for row in self.envelopes {
            writer.bytes(&row.intent.bytes())?;
            writer.u8(row.order_index)?;
            writer.u8(row.funding as u8)?;
            writer.u64(row.max_fee_atoms)?;
        }
        Ok(())
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, CodecError> {
        let authority = OwnerFeeAssessmentAuthorityV1::decode(reader)?;
        let next_page = reader.u8()?;
        let page_count = reader.u8()?;
        let order_count = reader.u8()?;
        let envelope_count = reader.u8()?;
        let phase = reader.u8()?;
        if reader.array::<3>()? != [0; 3] {
            return Err(CodecError::NonCanonicalPadding);
        }
        let processed_buy_mask = reader.u64()?;
        let processed_sell_mask = reader.u64()?;
        let exact_weight_numerator = reader.u128()?;
        let charged_atoms = reader.u64()?;
        let neutral_sink = Id32::new(reader.array()?)?;
        let mut envelopes = [EMPTY_ENVELOPE; MAX_FEE_ROWS_V1];
        let mut index = 0usize;
        while index < MAX_FEE_ROWS_V1 {
            let intent_bytes: [u8; 32] = reader.array()?;
            let order_index = reader.u8()?;
            let funding = match reader.u8()? {
                0 => FeeEnvelopeFundingV1::NoCashReservation,
                1 => FeeEnvelopeFundingV1::BuyCashReservation,
                _ => return Err(CodecError::InvalidState),
            };
            envelopes[index] = OwnerFeeAssessmentEnvelopeV1 {
                order_index,
                funding,
                intent: if intent_bytes == [0; 32] {
                    Id32::ZERO
                } else {
                    Id32::new(intent_bytes)?
                },
                max_fee_atoms: reader.u64()?,
            };
            index += 1;
        }
        let work = Self {
            authority,
            next_page,
            page_count,
            order_count,
            envelope_count,
            phase,
            processed_buy_mask,
            processed_sell_mask,
            exact_weight_numerator,
            charged_atoms,
            neutral_sink,
            envelopes,
        };
        work.validate()?;
        Ok(work)
    }
}

/// Exact rent-owned transient account. It is consumed, never upgraded.
#[derive(Debug, Eq, PartialEq)]
pub struct OwnerFeeAssessmentWorkV1AccountV1 {
    /// Sole transient semantic body.
    pub semantic: OwnerFeeAssessmentWorkV1,
    /// Exact separately funded principal and observed donation floor.
    pub rent: DeletableRentOwnerV1,
    /// Canonical PDA bump.
    pub stored_bump: u8,
}

impl OwnerFeeAssessmentWorkV1AccountV1 {
    fn validate_rent_bindings(&self) -> Result<(), CodecError> {
        self.rent.validate()?;
        if self.rent.payer == self.semantic.authority.settlement_root_account
            || self.rent.payer == self.semantic.authority.selected_fee_record_account
            || self.rent.payer == self.semantic.authority.retained_feed_account
            || self.rent.payer == self.semantic.neutral_sink
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode the exact fixed-width work account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate_rent_bindings()?;
        let mut writer = Writer::exact(output, OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1)?;
        writer.u8(OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_TAG)?;
        writer.u8(OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_VERSION)?;
        self.semantic.encode(&mut writer)?;
        writer.bytes(&self.rent.payer.bytes())?;
        writer.u64(self.rent.refundable_principal)?;
        writer.u64(self.rent.donation_floor)?;
        writer.u8(self.stored_bump)?;
        writer.u8(0)?;
        writer.finish()
    }

    /// Hostile decode with all semantic and padding invariants restored.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1)?;
        if reader.u8()? != OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let semantic = OwnerFeeAssessmentWorkV1::decode(&mut reader)?;
        let rent = DeletableRentOwnerV1 {
            payer: Id32::new(reader.array()?)?,
            refundable_principal: reader.u64()?,
            donation_floor: reader.u64()?,
        };
        rent.validate()?;
        let stored_bump = reader.u8()?;
        if reader.u8()? != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        reader.finish()?;
        let account = Self { semantic, rent, stored_bump };
        account.validate_rent_bindings()?;
        Ok(account)
    }

    /// Bind the exact account bytes and address for continuation replay checks.
    pub fn data_id<B: Sha256BackendV1>(
        &self,
        backend: &B,
        account: Id32,
    ) -> Result<Id32, CodecError> {
        if account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        let mut bytes = [0u8; OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1];
        self.encode(&mut bytes)?;
        Id32::new(backend.sha256(&[
            OWNER_FEE_ASSESSMENT_WORK_DATA_ID_DOMAIN_V1,
            &account.bytes(),
            &bytes,
        ]))
    }
}

const _: () = assert!(OWNER_FEE_ASSESSMENT_WORK_SEED_DOMAIN_V1.len() <= 32);
const _: () = assert!(OWNER_FEE_ASSESSMENT_WORK_BODY_BYTES_V1 == 3_248);
const _: () = assert!(OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1 == 3_300);
const _: () = assert!(OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1 <= 4_096);
const _: () = assert!(OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1 <= 10_240);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 { Id32::new([byte; 32]).unwrap() }

    fn authority() -> OwnerFeeAssessmentAuthorityV1 {
        OwnerFeeAssessmentAuthorityV1 {
            settlement_root_account: id(1), selected_fee_record_account: id(2),
            selected_fee_record_data_id: id(3), realm: id(4),
            revenue_policy_record_account: id(5), revenue_policy_record_v2_id: id(6),
            revenue_policy_v2_digest: id(7), retained_feed_account: id(8),
            retained_feed_data_id: id(9), owner: id(10), owner_row_account: id(11),
            market: id(12), epoch: id(13), order_set: id(14),
            owner_order_set_digest: id(15),
        }
    }

    fn envelope(order: u8, intent: u8, buy: bool) -> OwnerFeeAssessmentEnvelopeV1 {
        OwnerFeeAssessmentEnvelopeV1::new(id(10), order, FeeEnvelopeV1 {
            owner: FeeId(id(10).bytes()), intent: FeeId([intent; 32]),
            funding: if buy { FeeEnvelopeFundingV1::BuyCashReservation }
                else { FeeEnvelopeFundingV1::NoCashReservation },
            max_fee_atoms: if buy { 20 } else { 0 }, debited_atoms: 0,
        }).unwrap()
    }

    #[test]
    fn page_cursor_replay_and_skip_refuse() {
        let mut work = OwnerFeeAssessmentWorkV1::begin(authority(), 2, 4, id(16)).unwrap();
        assert!(work.record_page_envelope(1, envelope(0, 21, true)).is_err());
        work.record_page_envelope(0, envelope(0, 21, true)).unwrap();
        work.finish_page(0).unwrap();
        assert!(work.finish_page(0).is_err());
        assert!(work.record_page_envelope(0, envelope(1, 22, false)).is_err());
    }

    #[test]
    fn canonical_envelopes_sort_and_seal_exact_masks() {
        let mut work = OwnerFeeAssessmentWorkV1::begin(authority(), 1, 4, id(16)).unwrap();
        work.record_page_envelope(0, envelope(2, 23, false)).unwrap();
        work.record_page_envelope(0, envelope(0, 21, true)).unwrap();
        assert_eq!(work.envelope(0).unwrap().intent(), id(21));
        assert_eq!(work.envelope(1).unwrap().intent(), id(23));
        assert!(work.record_page_envelope(0, envelope(1, 21, true)).is_err());
        work.finish_page(0).unwrap();
        assert!(work.seal(1, 0, 99, 1).is_err());
        work.seal(1, 4, 99, 1).unwrap();
        assert!(work.is_ready());
    }
}
