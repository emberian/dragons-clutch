//! Receipt-V4 owner-accounting facts for the canonical V3 owner row.
//!
//! The owner row remains `0x81/3`; only the receipt authority moves to the
//! fresh `0x0f/4` family. This module therefore owns V4 receipt identities and
//! projections without duplicating or version-aliasing the row accumulator.

use crate::{
    AuthenticatedReservationHandoffV3, Error, PresentConsiderationV2, Result,
    SettlementSideV1, MAX_ORDERS, OWNER_SETTLEMENT_BODY_V3_BYTES,
};

/// Canonical General SettlementReceipt V4 outer body width.
pub const SETTLEMENT_RECEIPT_BODY_V4_BYTES: usize = 217;
/// Fresh domain for the exact authenticated Receipt V4 prestate.
pub const SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-settlement-receipt/data/v4\0";
/// Exact Receipt V4 prestate transcript: authenticated PDA then 217 bytes.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V4_BYTES: usize =
    32 + SETTLEMENT_RECEIPT_BODY_V4_BYTES;

const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Direct, virtual-split, or virtual-merge route owning one V4 real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementReceiptRouteV4 {
    /// Two real order ends.
    Direct = 0,
    /// A virtual split supplies one real buy end.
    SplitToBuy = 1,
    /// One real sell end supplies a virtual merge.
    SellToMerge = 2,
}

impl SettlementReceiptRouteV4 {
    /// Exact real-end bitmap owned by this route.
    pub const fn expected_end_mask(&self) -> u8 {
        match self {
            Self::Direct => BUY_END_MASK | SELL_END_MASK,
            Self::SplitToBuy => BUY_END_MASK,
            Self::SellToMerge => SELL_END_MASK,
        }
    }
}

/// Typed exact Receipt V4 mutable-prestate data identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct SettlementReceiptDataIdV4([u8; 32]);

impl SettlementReceiptDataIdV4 {
    /// Exact 32-byte data identity.
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// Hash boundary for the exact authenticated Receipt V4 prestate.
pub trait SettlementReceiptDataHashV4 {
    /// Compute SHA-256 over the domain followed by the exact transcript.
    fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32];
}

/// Derive the exact V4 receipt-prestate identity from PDA and all 217 bytes.
pub fn derive_settlement_receipt_data_id_v4<H: SettlementReceiptDataHashV4>(
    authenticated_receipt_pda: [u8; 32],
    exact_receipt_body: &[u8; SETTLEMENT_RECEIPT_BODY_V4_BYTES],
    hash: &H,
) -> Result<SettlementReceiptDataIdV4> {
    if authenticated_receipt_pda == [0; 32] || exact_receipt_body[..2] != [0x0f, 4] {
        return Err(Error::InvalidAccount);
    }
    let mut transcript = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V4_BYTES];
    transcript[..32].copy_from_slice(&authenticated_receipt_pda);
    transcript[32..].copy_from_slice(exact_receipt_body);
    let id = hash.sha256(SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V4, &transcript);
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(SettlementReceiptDataIdV4(id))
}

/// One exact authenticated V4 receipt end presented for V3 owner accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSettlementReceiptEndV4 {
    /// Canonical receipt PDA.
    pub receipt: [u8; 32],
    /// Exact typed receipt prestate identity.
    pub receipt_data_id: SettlementReceiptDataIdV4,
    /// Stable accounting-only transition identity derived from the V4 PDA.
    pub receipt_accounting_id: [u8; 32],
    /// General MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final candidate identity.
    pub candidate: [u8; 32],
    /// Exact immutable owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Semantic owner of this real end.
    pub owner: [u8; 32],
    /// Canonical selected order identity.
    pub order_id: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Direct, split-to-buy, or sell-to-merge route.
    pub route: SettlementReceiptRouteV4,
    /// Explicitly present exact consideration, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// True only for the canonical end exhausting this order.
    pub completes_order: bool,
    /// Zero-based selected slice index.
    pub slice_index: u16,
    /// Exactly `slice_index + 1`.
    pub sequence: u64,
    /// Already-accounted real-end mask.
    pub accounted_end_mask: u8,
    /// Exact real ends owned by the receipt route.
    pub expected_end_mask: u8,
    /// Present exactly on a completing buy end, including zero cash.
    pub reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl AuthenticatedSettlementReceiptEndV4 {
    pub(crate) const fn side_mask(&self) -> u8 {
        match self.side {
            SettlementSideV1::Buy => BUY_END_MASK,
            SettlementSideV1::Sell => SELL_END_MASK,
        }
    }

    /// Validate exact receipt shape and the canonical Reservation handoff join.
    pub fn validate(&self) -> Result<()> {
        self.consideration_price_units.validate()?;
        for identity in [
            self.receipt,
            self.receipt_data_id.bytes(),
            self.receipt_accounting_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.owner,
            self.order_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if !self.consideration_price_units.present
            || usize::from(self.order_index) >= MAX_ORDERS
            || self.expected_end_mask == 0
            || self.expected_end_mask & !(BUY_END_MASK | SELL_END_MASK) != 0
            || self.accounted_end_mask & !self.expected_end_mask != 0
            || self.sequence != u64::from(self.slice_index) + 1
        {
            return Err(Error::InvalidOrder);
        }
        match (self.route, self.side, self.expected_end_mask) {
            (SettlementReceiptRouteV4::Direct, _, 3)
            | (SettlementReceiptRouteV4::SplitToBuy, SettlementSideV1::Buy, 1)
            | (SettlementReceiptRouteV4::SellToMerge, SettlementSideV1::Sell, 2) => {}
            _ => return Err(Error::InvalidOrder),
        }
        let side = self.side_mask();
        if self.expected_end_mask & side == 0 || self.accounted_end_mask & side != 0 {
            return Err(Error::DuplicateCompletion);
        }
        match (self.side, self.completes_order, self.reservation_handoff) {
            (SettlementSideV1::Buy, true, Some(handoff)) => {
                handoff.validate()?;
                if handoff.owner() != self.owner || handoff.order_id() != self.order_id {
                    return Err(Error::AuthorityUnavailable);
                }
            }
            (SettlementSideV1::Buy, true, None) => return Err(Error::AuthorityUnavailable),
            (SettlementSideV1::Buy, false, None) | (SettlementSideV1::Sell, _, None) => {}
            (SettlementSideV1::Buy, false, Some(_)) | (SettlementSideV1::Sell, _, Some(_)) => {
                return Err(Error::InvariantViolation);
            }
        }
        Ok(())
    }
}

/// Non-authorizing V4 receipt projection onto the canonical V3 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementReceiptAccountingProjectionV4 {
    pub(crate) owner_settlement_account: [u8; 32],
    pub(crate) owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
    pub(crate) receipt: [u8; 32],
    pub(crate) receipt_data_id: SettlementReceiptDataIdV4,
    pub(crate) receipt_accounting_id: [u8; 32],
    pub(crate) receipt_accounted_end_mask: u8,
    pub(crate) reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl OwnerSettlementReceiptAccountingProjectionV4 {
    /// Owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] { self.owner_settlement_account }
    /// Exact canonical V3 row successor body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES] {
        &self.owner_settlement_body
    }
    /// Receipt account to latch.
    pub const fn receipt(&self) -> [u8; 32] { self.receipt }
    /// Exact typed V4 receipt prestate identity.
    pub const fn receipt_data_id(&self) -> SettlementReceiptDataIdV4 { self.receipt_data_id }
    /// Stable V4 accounting-only transition identity.
    pub const fn receipt_accounting_id(&self) -> [u8; 32] { self.receipt_accounting_id }
    /// Exact next independent accounting mask.
    pub const fn receipt_accounted_end_mask(&self) -> u8 { self.receipt_accounted_end_mask }
    /// Exact Reservation handoff, present only for a terminal buy.
    pub const fn reservation_handoff(&self) -> Option<AuthenticatedReservationHandoffV3> {
        self.reservation_handoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct PrefixHash;

    impl SettlementReceiptDataHashV4 for PrefixHash {
        fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[..4].copy_from_slice(&domain[..4]);
            out[4..].copy_from_slice(&transcript[..28]);
            out
        }
    }

    #[test]
    fn receipt_identity_refuses_every_non_v4_version() {
        let mut body = [0u8; SETTLEMENT_RECEIPT_BODY_V4_BYTES];
        body[..2].copy_from_slice(&[0x0f, 3]);
        assert_eq!(
            derive_settlement_receipt_data_id_v4([1; 32], &body, &PrefixHash),
            Err(Error::InvalidAccount)
        );
        body[1] = 4;
        assert!(derive_settlement_receipt_data_id_v4([1; 32], &body, &PrefixHash).is_ok());
    }
}
