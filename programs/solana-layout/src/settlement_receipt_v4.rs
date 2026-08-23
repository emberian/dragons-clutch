//! Canonical General SettlementReceipt V4 persistence.
//!
//! V4 keeps the 217-byte V3 footprint and the same independent accounting and
//! delivery latches. It adds no persisted field. Instead, it gives virtual
//! merge delivery an explicit canonical payment-pending state and derives a
//! third, disjoint payment-transition identity from the authenticated receipt
//! PDA. V3 remains a historical, withdrawn schema and is never reinterpreted.

use super::{
    account_len, account_version, check_hash, digest, put_header,
    registry::{
        GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG,
        GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION,
    },
    CodecError, EpochId, Hash32, MarketId, Reader, Result, Writer, MAX_OUTCOMES, MAX_SLICES,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT, RECEIPT_LEG_MERGE, RECEIPT_LEG_SPLIT,
    SETTLEMENT_RECEIPT_TAG,
};

/// Fresh PDA seed for General SettlementReceipt V4.
pub const GENERAL_SETTLEMENT_RECEIPT_SEED_V4: &[u8] = b"general-receipt:v4";
/// Domain of the stable accounting-only transition identity.
pub const RECEIPT_ACCOUNTING_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-settlement-receipt/accounting/v4\0";
/// Domain of the stable atomic Egg-delivery transition identity.
pub const RECEIPT_DELIVERY_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-settlement-receipt/delivery/v4\0";
/// Domain of the stable merge-payment transition identity.
pub const RECEIPT_PAYMENT_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-settlement-receipt/payment/v4\0";
/// Domain of the exact mutable receipt-prestate data identity.
pub const RECEIPT_DATA_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-settlement-receipt/data/v4\0";

/// Exact transcript bytes for a receipt prestate: authenticated PDA then body.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V4_BYTES: usize =
    32 + account_len::SETTLEMENT_RECEIPT_V4;

/// Accounting latch for the real buy end.
pub const RECEIPT_ACCOUNTED_BUY_END: u8 = 1;
/// Accounting latch for the real sell end.
pub const RECEIPT_ACCOUNTED_SELL_END: u8 = 2;

const REAL_END_MASK: u8 = RECEIPT_ACCOUNTED_BUY_END | RECEIPT_ACCOUNTED_SELL_END;
const CONSUMED_MASK: u8 =
    RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED;

const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG == SETTLEMENT_RECEIPT_TAG);
const _: () = assert!(
    GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION == account_version::SETTLEMENT_RECEIPT_V4
);
const _: () = assert!(account_len::SETTLEMENT_RECEIPT_V4 == 217);

/// Canonical same-width receipt with an explicit merge payment window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptAccountV4 {
    /// Counted Epoch identity.
    pub epoch: EpochId,
    /// General MarketRuntime identity.
    pub market: MarketId,
    /// Final selected SettlementCandidate identity.
    pub candidate: Hash32,
    /// Buy order identity; zero exactly for virtual merge.
    pub buy_order_id: Hash32,
    /// Sell order identity; zero exactly for virtual split.
    pub sell_order_id: Hash32,
    /// Exact `quantity * price` in scaled price units, including zero.
    pub consideration_price_units: u128,
    /// Slice quantity in native Egg atoms.
    pub quantity: u64,
    /// Zero before delivery and `quantity` after complete Egg delivery.
    pub settled_quantity: u64,
    /// Frozen selected price for `outcome`, including zero.
    pub price: u64,
    /// Canonical selected-slice ordinal, exactly `slice_index + 1`.
    pub sequence: u64,
    /// Zero-based selected slice index.
    pub slice_index: u16,
    /// Native Egg outcome delivered by the slice.
    pub outcome: u8,
    /// Direct, split-to-buy, or sell-to-merge route.
    pub leg_kind: u8,
    /// Delivered-buy, delivered-sell, and payment-complete/exhausted flags.
    pub consumed_flags: u8,
    /// Stored receipt PDA bump.
    pub stored_bump: u8,
    /// Independent accounted-buy and accounted-sell latch mask.
    pub accounted_end_mask: u8,
}

impl SettlementReceiptAccountV4 {
    /// Exact real ends owned by this route.
    pub const fn expected_end_mask(&self) -> u8 {
        match self.leg_kind {
            RECEIPT_LEG_DIRECT => REAL_END_MASK,
            RECEIPT_LEG_SPLIT => RECEIPT_ACCOUNTED_BUY_END,
            RECEIPT_LEG_MERGE => RECEIPT_ACCOUNTED_SELL_END,
            _ => 0,
        }
    }

    /// Delivered real-end latches, excluding the exhausted bit.
    pub const fn delivered_end_mask(&self) -> u8 {
        self.consumed_flags & REAL_END_MASK
    }

    /// Whether merge delivery has completed but seller payment remains open.
    pub const fn merge_payment_pending(&self) -> bool {
        self.leg_kind == RECEIPT_LEG_MERGE
            && self.delivered_end_mask() == RECEIPT_ACCOUNTED_SELL_END
            && self.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED == 0
            && self.settled_quantity == self.quantity
            && self.accounted_end_mask == RECEIPT_ACCOUNTED_SELL_END
    }

    /// Whether the receipt is in its terminal delivered-and-paid state.
    pub const fn payment_complete(&self) -> bool {
        self.delivered_end_mask() == self.expected_end_mask()
            && self.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED != 0
            && self.settled_quantity == self.quantity
            && self.accounted_end_mask == self.expected_end_mask()
    }

    /// Validate route shape, exact economics, and all canonical lifecycle states.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.epoch)?;
        check_hash(self.market)?;
        check_hash(self.candidate)?;
        if self.leg_kind > RECEIPT_LEG_MERGE
            || self.consumed_flags & !CONSUMED_MASK != 0
            || self.accounted_end_mask & !REAL_END_MASK != 0
        {
            return Err(CodecError::InvalidEnum);
        }
        match self.leg_kind {
            RECEIPT_LEG_DIRECT => {
                check_hash(self.buy_order_id)?;
                check_hash(self.sell_order_id)?;
                if self.buy_order_id == self.sell_order_id {
                    return Err(CodecError::NonCanonicalIdentity);
                }
            }
            RECEIPT_LEG_SPLIT => {
                check_hash(self.buy_order_id)?;
                if self.sell_order_id != Hash32::ZERO {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
            RECEIPT_LEG_MERGE => {
                check_hash(self.sell_order_id)?;
                if self.buy_order_id != Hash32::ZERO {
                    return Err(CodecError::NonCanonicalPadding);
                }
            }
            _ => return Err(CodecError::InvalidEnum),
        }
        if usize::from(self.outcome) >= MAX_OUTCOMES
            || usize::from(self.slice_index) >= MAX_SLICES
            || self.quantity == 0
            || self.sequence != u64::from(self.slice_index) + 1
        {
            return Err(CodecError::InvalidCount);
        }
        if self.consideration_price_units
            != u128::from(self.quantity) * u128::from(self.price)
        {
            return Err(CodecError::InvalidConsideration);
        }

        let expected = self.expected_end_mask();
        let delivered = self.delivered_end_mask();
        if self.accounted_end_mask & !expected != 0 || delivered & !expected != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let exhausted = self.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED != 0;
        let fresh_delivery = delivered == 0 && !exhausted && self.settled_quantity == 0;
        let terminal_direct_or_split = self.leg_kind != RECEIPT_LEG_MERGE
            && self.payment_complete();
        let terminal_merge = self.leg_kind == RECEIPT_LEG_MERGE
            && (self.merge_payment_pending() || self.payment_complete());
        if !fresh_delivery && !terminal_direct_or_split && !terminal_merge {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Encode exactly 217 bytes under tag `0x0f`, version 4.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::SETTLEMENT_RECEIPT_V4 {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG,
            GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION,
        )?;
        writer.hash(self.epoch)?;
        writer.hash(self.market)?;
        writer.hash(self.candidate)?;
        writer.hash(self.buy_order_id)?;
        writer.hash(self.sell_order_id)?;
        writer.u128(self.consideration_price_units)?;
        writer.u64(self.quantity)?;
        writer.u64(self.settled_quantity)?;
        writer.u64(self.price)?;
        writer.u64(self.sequence)?;
        writer.u16(self.slice_index)?;
        writer.u8(self.outcome)?;
        writer.u8(self.leg_kind)?;
        writer.u8(self.consumed_flags)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.accounted_end_mask)?;
        Ok(writer.at)
    }

    /// Return the exact canonical current account body.
    pub fn encode_exact(&self) -> Result<[u8; account_len::SETTLEMENT_RECEIPT_V4]> {
        let mut body = [0u8; account_len::SETTLEMENT_RECEIPT_V4];
        let written = self.encode(&mut body)?;
        if written != body.len() {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(body)
    }

    /// Decode exactly 217 bytes and refuse V2/V3 at the version byte.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG,
            GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION,
            account_len::SETTLEMENT_RECEIPT_V4,
        )?;
        let value = Self {
            epoch: reader.hash()?, market: reader.hash()?, candidate: reader.hash()?,
            buy_order_id: reader.hash()?, sell_order_id: reader.hash()?,
            consideration_price_units: reader.u128()?, quantity: reader.u64()?,
            settled_quantity: reader.u64()?, price: reader.u64()?, sequence: reader.u64()?,
            slice_index: reader.u16()?, outcome: reader.u8()?, leg_kind: reader.u8()?,
            consumed_flags: reader.u8()?, stored_bump: reader.u8()?,
            accounted_end_mask: reader.u8()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }

    /// Derive all three disjoint action identities from an authenticated PDA.
    pub fn transition_ids(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptTransitionIdsV4> {
        self.validate()?;
        check_hash(authenticated_receipt_pda)?;
        let accounting = Hash32::new(digest(
            RECEIPT_ACCOUNTING_ID_DOMAIN_V4,
            &[&authenticated_receipt_pda.0],
        ).0)?;
        let delivery = Hash32::new(digest(
            RECEIPT_DELIVERY_ID_DOMAIN_V4,
            &[&authenticated_receipt_pda.0],
        ).0)?;
        let payment = Hash32::new(digest(
            RECEIPT_PAYMENT_ID_DOMAIN_V4,
            &[&authenticated_receipt_pda.0],
        ).0)?;
        if accounting == delivery || accounting == payment || delivery == payment {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(SettlementReceiptTransitionIdsV4 {
            receipt_accounting_id: accounting,
            delivery_transition_id: delivery,
            payment_transition_id: payment,
        })
    }

    /// Encode the exact mutable prestate transcript: PDA then all 217 bytes.
    pub fn data_id_transcript(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<[u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V4_BYTES]> {
        check_hash(authenticated_receipt_pda)?;
        let body = self.encode_exact()?;
        let mut transcript = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V4_BYTES];
        transcript[..32].copy_from_slice(&authenticated_receipt_pda.0);
        transcript[32..].copy_from_slice(&body);
        Ok(transcript)
    }

    /// Derive the exact mutable receipt-prestate data identity.
    pub fn data_id(&self, authenticated_receipt_pda: Hash32) -> Result<Hash32> {
        let transcript = self.data_id_transcript(authenticated_receipt_pda)?;
        Hash32::new(digest(RECEIPT_DATA_ID_DOMAIN_V4, &[&transcript]).0)
    }

    /// Project exact current bytes and all deterministic IDs for an action.
    pub fn evidence(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptEvidenceV4> {
        Ok(SettlementReceiptEvidenceV4 {
            receipt: authenticated_receipt_pda,
            exact_body: self.encode_exact()?,
            receipt_data_id: self.data_id(authenticated_receipt_pda)?,
            transition_ids: self.transition_ids(authenticated_receipt_pda)?,
        })
    }
}

/// Stable action identities derived from one authenticated V4 receipt PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptTransitionIdsV4 {
    receipt_accounting_id: Hash32,
    delivery_transition_id: Hash32,
    payment_transition_id: Hash32,
}

impl SettlementReceiptTransitionIdsV4 {
    /// Accounting-only action-25 transition identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 { self.receipt_accounting_id }
    /// Atomic action-26/36/37 Egg-delivery transition identity.
    pub const fn delivery_transition_id(&self) -> Hash32 { self.delivery_transition_id }
    /// Action-40 merge-payment transition identity.
    pub const fn payment_transition_id(&self) -> Hash32 { self.payment_transition_id }
}

/// Exact decoded receipt evidence projected for an action composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptEvidenceV4 {
    receipt: Hash32,
    exact_body: [u8; account_len::SETTLEMENT_RECEIPT_V4],
    receipt_data_id: Hash32,
    transition_ids: SettlementReceiptTransitionIdsV4,
}

impl SettlementReceiptEvidenceV4 {
    /// Authenticated receipt PDA supplied by the adapter.
    pub const fn receipt(&self) -> Hash32 { self.receipt }
    /// Exact current V4 body, including both latch families.
    pub const fn exact_body(&self) -> &[u8; account_len::SETTLEMENT_RECEIPT_V4] {
        &self.exact_body
    }
    /// Exact mutable prestate data identity for Replay authority.
    pub const fn receipt_data_id(&self) -> Hash32 { self.receipt_data_id }
    /// Stable accounting-only action-25 transition identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 {
        self.transition_ids.receipt_accounting_id()
    }
    /// Stable atomic action-26/36/37 delivery transition identity.
    pub const fn delivery_transition_id(&self) -> Hash32 {
        self.transition_ids.delivery_transition_id()
    }
    /// Stable action-40 merge-payment transition identity.
    pub const fn payment_transition_id(&self) -> Hash32 {
        self.transition_ids.payment_transition_id()
    }
}

/// Decode hostile V4 bytes and project deterministic action evidence.
pub fn project_settlement_receipt_evidence_v4(
    authenticated_receipt_pda: Hash32,
    exact_body: &[u8],
) -> Result<(SettlementReceiptAccountV4, SettlementReceiptEvidenceV4)> {
    check_hash(authenticated_receipt_pda)?;
    let receipt = SettlementReceiptAccountV4::decode(exact_body)?;
    let evidence = receipt.evidence(authenticated_receipt_pda)?;
    Ok((receipt, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement_receipt_v3::SettlementReceiptAccountV3;

    fn id(byte: u8) -> Hash32 { Hash32([byte; 32]) }

    fn merge() -> SettlementReceiptAccountV4 {
        SettlementReceiptAccountV4 {
            epoch: id(1), market: id(2), candidate: id(3), buy_order_id: Hash32::ZERO,
            sell_order_id: id(5), consideration_price_units: 0, quantity: 7,
            settled_quantity: 0, price: 0, sequence: 10, slice_index: 9, outcome: 3,
            leg_kind: RECEIPT_LEG_MERGE, consumed_flags: 0, stored_bump: 254,
            accounted_end_mask: 0,
        }
    }

    #[test]
    fn exact_round_trip_and_v3_refusal_are_version_owned() {
        let value = merge();
        let body = value.encode_exact().unwrap();
        assert_eq!(body.len(), 217);
        assert_eq!(body[..2], [0x0f, 4]);
        assert_eq!(SettlementReceiptAccountV4::decode(&body), Ok(value));
        assert_eq!(SettlementReceiptAccountV3::decode(&body), Err(CodecError::WrongVersion));
    }

    #[test]
    fn merge_delivery_and_payment_are_disjoint_canonical_states() {
        let mut value = merge();
        value.accounted_end_mask = RECEIPT_ACCOUNTED_SELL_END;
        value.settled_quantity = value.quantity;
        value.consumed_flags = RECEIPT_FLAG_SELL_CONSUMED;
        assert!(value.merge_payment_pending());
        assert!(!value.payment_complete());
        assert_eq!(value.validate(), Ok(()));

        value.consumed_flags |= RECEIPT_FLAG_SLICE_EXHAUSTED;
        assert!(!value.merge_payment_pending());
        assert!(value.payment_complete());
        assert_eq!(value.validate(), Ok(()));
    }

    #[test]
    fn direct_and_split_cannot_enter_merge_payment_window() {
        let mut value = merge();
        value.leg_kind = RECEIPT_LEG_SPLIT;
        value.buy_order_id = id(4);
        value.sell_order_id = Hash32::ZERO;
        value.accounted_end_mask = RECEIPT_ACCOUNTED_BUY_END;
        value.settled_quantity = value.quantity;
        value.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        assert_eq!(value.validate(), Err(CodecError::InvalidEnum));
    }

    #[test]
    fn three_transition_ids_are_stable_disjoint_and_pda_owned() {
        let mut value = merge();
        let fresh = value.evidence(id(90)).unwrap();
        assert_ne!(fresh.receipt_accounting_id(), fresh.delivery_transition_id());
        assert_ne!(fresh.receipt_accounting_id(), fresh.payment_transition_id());
        assert_ne!(fresh.delivery_transition_id(), fresh.payment_transition_id());

        value.accounted_end_mask = RECEIPT_ACCOUNTED_SELL_END;
        let accounted = value.evidence(id(90)).unwrap();
        assert_eq!(fresh.receipt_accounting_id(), accounted.receipt_accounting_id());
        assert_eq!(fresh.delivery_transition_id(), accounted.delivery_transition_id());
        assert_eq!(fresh.payment_transition_id(), accounted.payment_transition_id());
        assert_ne!(fresh.receipt_data_id(), accounted.receipt_data_id());

        let other_pda = value.evidence(id(91)).unwrap();
        assert_ne!(accounted.payment_transition_id(), other_pda.payment_transition_id());
    }
}
