//! Canonical General SettlementReceipt V3 persistence.
//!
//! This successor keeps the frozen tag-15 receipt footprint at 217 bytes, but
//! gives the final byte a real owner: independent buy/sell accounting latches.
//! The preceding `consumed_flags` byte keeps its V2 meaning as delivered-buy,
//! delivered-sell, and slice-exhausted. V2 and V3 have different version bytes
//! and neither decoder accepts the other's body.
//!
//! Transition identities are not stored. Once the adapter has authenticated
//! the receipt PDA derived from [`GENERAL_SETTLEMENT_RECEIPT_SEED_V3`], the two
//! stable transition IDs are hashes of that PDA under disjoint contract
//! domains. The exact prestate data ID separately commits to the PDA and all
//! 217 bytes, including both mutable latch families.
//! Owner/order-set membership is likewise not duplicated here: the action
//! composer must join it from exact authenticated `0x81/2` owner-row bytes.
//! Selected-candidate authority is never lowered through the legacy
//! `CandidateRecord`; the General adapter must join the exact authenticated
//! `0x7c` SelectedCandidate and retained CandidateFeedV2.

use super::{
    account_len, account_version, check_hash, digest, put_header,
    registry::{
        GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
        GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
    },
    CodecError, EpochId, Hash32, MarketId, Reader, Result, Writer, MAX_OUTCOMES, MAX_SLICES,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT, RECEIPT_LEG_MERGE, RECEIPT_LEG_SPLIT,
    SETTLEMENT_RECEIPT_TAG,
};

/// Fresh PDA seed for General SettlementReceipt V3.
pub const GENERAL_SETTLEMENT_RECEIPT_SEED_V3: &[u8] = b"general-receipt:v3";

/// Domain of the stable accounting-only transition identity.
pub const RECEIPT_ACCOUNTING_ID_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-settlement-receipt/accounting/v3\0";
/// Domain of the stable atomic Egg-delivery transition identity.
pub const RECEIPT_DELIVERY_ID_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-settlement-receipt/delivery/v3\0";
/// Domain of the exact mutable receipt-prestate data identity.
pub const RECEIPT_DATA_ID_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-settlement-receipt/data/v3\0";

/// Exact transcript bytes for a receipt prestate: authenticated PDA then body.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES: usize =
    32 + account_len::SETTLEMENT_RECEIPT_V3;

/// Accounting latch for the real buy end.
pub const RECEIPT_ACCOUNTED_BUY_END: u8 = 1;
/// Accounting latch for the real sell end.
pub const RECEIPT_ACCOUNTED_SELL_END: u8 = 2;

const REAL_END_MASK: u8 = RECEIPT_ACCOUNTED_BUY_END | RECEIPT_ACCOUNTED_SELL_END;
const CONSUMED_MASK: u8 =
    RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED;

const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG == SETTLEMENT_RECEIPT_TAG);
const _: () = assert!(
    GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION == account_version::SETTLEMENT_RECEIPT_V3
);
const _: () = assert!(account_len::SETTLEMENT_RECEIPT_V3 == 217);

/// Canonical persisted General settlement receipt successor.
///
/// `accounted_end_mask` replaces V2's reserved-zero final byte. Nothing else
/// moves, so the account remains exactly 217 bytes. The receipt deliberately
/// does not persist the owner-order-set digest or either transition ID; those
/// are authenticated joins and deterministic projections respectively.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptAccountV3 {
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
    /// Zero before delivery and `quantity` after complete atomic delivery.
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
    /// Delivered-buy, delivered-sell, and exhausted flags.
    pub consumed_flags: u8,
    /// Stored receipt PDA bump.
    pub stored_bump: u8,
    /// Independent accounted-buy and accounted-sell latch mask.
    pub accounted_end_mask: u8,
}

impl SettlementReceiptAccountV3 {
    /// Exact real ends owned by this route.
    pub const fn expected_end_mask(&self) -> u8 {
        match self.leg_kind {
            RECEIPT_LEG_DIRECT => RECEIPT_ACCOUNTED_BUY_END | RECEIPT_ACCOUNTED_SELL_END,
            RECEIPT_LEG_SPLIT => RECEIPT_ACCOUNTED_BUY_END,
            RECEIPT_LEG_MERGE => RECEIPT_ACCOUNTED_SELL_END,
            _ => 0,
        }
    }

    /// Delivered real-end latches, excluding the exhausted bit.
    pub const fn delivered_end_mask(&self) -> u8 {
        self.consumed_flags & REAL_END_MASK
    }

    /// Validate route shape, exact economics, and the two independent latches.
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
        let complete_delivery = delivered == expected
            && exhausted
            && self.settled_quantity == self.quantity
            && self.accounted_end_mask == expected;
        if !fresh_delivery && !complete_delivery {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Encode exactly 217 bytes under tag `0x0f`, version 3.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::SETTLEMENT_RECEIPT_V3 {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
            GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
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
    pub fn encode_exact(&self) -> Result<[u8; account_len::SETTLEMENT_RECEIPT_V3]> {
        let mut body = [0u8; account_len::SETTLEMENT_RECEIPT_V3];
        let written = self.encode(&mut body)?;
        if written != body.len() {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(body)
    }

    /// Decode exactly 217 bytes and refuse V2 at the version byte.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
            GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
            account_len::SETTLEMENT_RECEIPT_V3,
        )?;
        let value = Self {
            epoch: reader.hash()?,
            market: reader.hash()?,
            candidate: reader.hash()?,
            buy_order_id: reader.hash()?,
            sell_order_id: reader.hash()?,
            consideration_price_units: reader.u128()?,
            quantity: reader.u64()?,
            settled_quantity: reader.u64()?,
            price: reader.u64()?,
            sequence: reader.u64()?,
            slice_index: reader.u16()?,
            outcome: reader.u8()?,
            leg_kind: reader.u8()?,
            consumed_flags: reader.u8()?,
            stored_bump: reader.u8()?,
            accounted_end_mask: reader.u8()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }

    /// Derive stable accounting and delivery IDs from an authenticated PDA.
    ///
    /// The adapter must authenticate the exact PDA seed tuple before calling
    /// this projection. No caller-supplied transition identity is admitted.
    pub fn transition_ids(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptTransitionIdsV3> {
        self.validate()?;
        check_hash(authenticated_receipt_pda)?;
        let accounting = Hash32::new(
            digest(RECEIPT_ACCOUNTING_ID_DOMAIN_V3, &[&authenticated_receipt_pda.0]).0,
        )?;
        let delivery = Hash32::new(
            digest(RECEIPT_DELIVERY_ID_DOMAIN_V3, &[&authenticated_receipt_pda.0]).0,
        )?;
        if accounting == delivery {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(SettlementReceiptTransitionIdsV3 {
            receipt_accounting_id: accounting,
            delivery_transition_id: delivery,
        })
    }

    /// Encode the exact mutable prestate transcript: PDA then all 217 bytes.
    pub fn data_id_transcript(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<[u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES]> {
        check_hash(authenticated_receipt_pda)?;
        let body = self.encode_exact()?;
        let mut transcript = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES];
        transcript[..32].copy_from_slice(&authenticated_receipt_pda.0);
        transcript[32..].copy_from_slice(&body);
        Ok(transcript)
    }

    /// Derive the exact mutable receipt-prestate data identity.
    pub fn data_id(&self, authenticated_receipt_pda: Hash32) -> Result<Hash32> {
        let transcript = self.data_id_transcript(authenticated_receipt_pda)?;
        Hash32::new(digest(RECEIPT_DATA_ID_DOMAIN_V3, &[&transcript]).0)
    }

    /// Project exact current bytes and all deterministic IDs for an action.
    pub fn evidence(
        &self,
        authenticated_receipt_pda: Hash32,
    ) -> Result<SettlementReceiptEvidenceV3> {
        let exact_body = self.encode_exact()?;
        let transition_ids = self.transition_ids(authenticated_receipt_pda)?;
        let receipt_data_id = self.data_id(authenticated_receipt_pda)?;
        Ok(SettlementReceiptEvidenceV3 {
            receipt: authenticated_receipt_pda,
            exact_body,
            receipt_data_id,
            transition_ids,
        })
    }
}

/// Stable action identities derived from one authenticated receipt PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptTransitionIdsV3 {
    receipt_accounting_id: Hash32,
    delivery_transition_id: Hash32,
}

impl SettlementReceiptTransitionIdsV3 {
    /// Accounting-only action-25 transition identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 {
        self.receipt_accounting_id
    }

    /// Atomic action-26/36/37 Egg-delivery transition identity.
    pub const fn delivery_transition_id(&self) -> Hash32 {
        self.delivery_transition_id
    }
}

/// Exact decoded receipt evidence projected for an action composer.
///
/// Account ownership, PDA derivation, selected-candidate authentication, and
/// owner-row joins remain adapter responsibilities. This byte projection does
/// not claim any of those external facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptEvidenceV3 {
    receipt: Hash32,
    exact_body: [u8; account_len::SETTLEMENT_RECEIPT_V3],
    receipt_data_id: Hash32,
    transition_ids: SettlementReceiptTransitionIdsV3,
}

impl SettlementReceiptEvidenceV3 {
    /// Authenticated receipt PDA supplied by the adapter.
    pub const fn receipt(&self) -> Hash32 {
        self.receipt
    }

    /// Exact current V3 body, including both latch families.
    pub const fn exact_body(&self) -> &[u8; account_len::SETTLEMENT_RECEIPT_V3] {
        &self.exact_body
    }

    /// Exact mutable prestate data identity for Replay authority.
    pub const fn receipt_data_id(&self) -> Hash32 {
        self.receipt_data_id
    }

    /// Stable accounting-only action-25 transition identity.
    pub const fn receipt_accounting_id(&self) -> Hash32 {
        self.transition_ids.receipt_accounting_id()
    }

    /// Stable atomic action-26/36/37 delivery transition identity.
    pub const fn delivery_transition_id(&self) -> Hash32 {
        self.transition_ids.delivery_transition_id()
    }
}

/// Decode exact hostile V3 bytes and project deterministic action evidence.
///
/// The `authenticated_receipt_pda` name records the adapter obligation; this
/// pure layout function cannot establish program ownership, writability, or
/// PDA derivation by itself.
pub fn project_settlement_receipt_evidence_v3(
    authenticated_receipt_pda: Hash32,
    exact_body: &[u8],
) -> Result<(SettlementReceiptAccountV3, SettlementReceiptEvidenceV3)> {
    check_hash(authenticated_receipt_pda)?;
    let receipt = SettlementReceiptAccountV3::decode(exact_body)?;
    let evidence = receipt.evidence(authenticated_receipt_pda)?;
    Ok((receipt, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SettlementReceiptAccount;

    fn id(byte: u8) -> Hash32 {
        Hash32([byte; 32])
    }

    fn receipt() -> SettlementReceiptAccountV3 {
        SettlementReceiptAccountV3 {
            epoch: id(1),
            market: id(2),
            candidate: id(3),
            buy_order_id: id(4),
            sell_order_id: id(5),
            consideration_price_units: 0,
            quantity: 7,
            settled_quantity: 0,
            price: 0,
            sequence: 10,
            slice_index: 9,
            outcome: 3,
            leg_kind: RECEIPT_LEG_DIRECT,
            consumed_flags: 0,
            stored_bump: 254,
            accounted_end_mask: 0,
        }
    }

    #[test]
    fn exact_round_trip_and_v2_refusal_are_version_owned() {
        let value = receipt();
        let body = value.encode_exact().unwrap();
        assert_eq!(body.len(), 217);
        assert_eq!(body[..2], [0x0f, 3]);
        assert_eq!(SettlementReceiptAccountV3::decode(&body), Ok(value));
        assert_eq!(SettlementReceiptAccount::decode(&body), Err(CodecError::WrongVersion));

        let legacy = SettlementReceiptAccount {
            epoch: value.epoch,
            market: value.market,
            candidate: value.candidate,
            buy_order_id: value.buy_order_id,
            sell_order_id: value.sell_order_id,
            consideration_price_units: value.consideration_price_units,
            quantity: value.quantity,
            settled_quantity: value.settled_quantity,
            price: value.price,
            sequence: value.sequence,
            slice_index: value.slice_index,
            outcome: value.outcome,
            leg_kind: value.leg_kind,
            consumed_flags: value.consumed_flags,
            stored_bump: value.stored_bump,
            flags: 0,
        };
        let mut legacy_body = [0u8; account_len::SETTLEMENT_RECEIPT];
        legacy.encode(&mut legacy_body).unwrap();
        assert_eq!(SettlementReceiptAccountV3::decode(&legacy_body), Err(CodecError::WrongVersion));
    }

    #[test]
    fn exact_lengths_and_hostile_latches_refuse() {
        let value = receipt();
        let body = value.encode_exact().unwrap();
        assert_eq!(SettlementReceiptAccountV3::decode(&body[..216]), Err(CodecError::Truncated));
        let mut long = [0u8; 218];
        long[..217].copy_from_slice(&body);
        assert_eq!(SettlementReceiptAccountV3::decode(&long), Err(CodecError::TrailingBytes));

        let mut delivered_before_accounting = value;
        delivered_before_accounting.settled_quantity = value.quantity;
        delivered_before_accounting.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED
            | RECEIPT_FLAG_SELL_CONSUMED
            | RECEIPT_FLAG_SLICE_EXHAUSTED;
        assert_eq!(delivered_before_accounting.validate(), Err(CodecError::InvalidEnum));

        let mut partial_delivery = value;
        partial_delivery.accounted_end_mask = REAL_END_MASK;
        partial_delivery.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        assert_eq!(partial_delivery.validate(), Err(CodecError::InvalidEnum));

        let mut split = value;
        split.leg_kind = RECEIPT_LEG_SPLIT;
        split.sell_order_id = Hash32::ZERO;
        split.accounted_end_mask = RECEIPT_ACCOUNTED_SELL_END;
        assert_eq!(split.validate(), Err(CodecError::InvalidEnum));
    }

    #[test]
    fn transition_ids_are_stable_while_data_id_commits_to_latches() {
        let receipt_pda = id(99);
        let mut value = receipt();
        let fresh = value.evidence(receipt_pda).unwrap();
        assert_ne!(fresh.receipt_accounting_id(), fresh.delivery_transition_id());

        value.accounted_end_mask = REAL_END_MASK;
        let accounted = value.evidence(receipt_pda).unwrap();
        assert_eq!(fresh.receipt_accounting_id(), accounted.receipt_accounting_id());
        assert_eq!(fresh.delivery_transition_id(), accounted.delivery_transition_id());
        assert_ne!(fresh.receipt_data_id(), accounted.receipt_data_id());

        value.settled_quantity = value.quantity;
        value.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED
            | RECEIPT_FLAG_SELL_CONSUMED
            | RECEIPT_FLAG_SLICE_EXHAUSTED;
        let delivered = value.evidence(receipt_pda).unwrap();
        assert_eq!(fresh.receipt_accounting_id(), delivered.receipt_accounting_id());
        assert_eq!(fresh.delivery_transition_id(), delivered.delivery_transition_id());
        assert_ne!(accounted.receipt_data_id(), delivered.receipt_data_id());
    }

    #[test]
    fn authenticated_receipt_identity_owns_every_derived_id() {
        let value = receipt();
        let left = value.evidence(id(90)).unwrap();
        let right = value.evidence(id(91)).unwrap();
        assert_ne!(left.receipt_accounting_id(), right.receipt_accounting_id());
        assert_ne!(left.delivery_transition_id(), right.delivery_transition_id());
        assert_ne!(left.receipt_data_id(), right.receipt_data_id());
    }
}
