//! Receipt-V5 owner-accounting facts for the canonical V4 semantic row.
//!
//! The rent-owned `0x0f/5` outer and its exact 298-byte data identity are
//! authenticated by the General adapter. This dependency-lower module owns
//! only the fresh typed end presented to the existing V4 arithmetic state
//! machine; it never accepts a V4 data-ID wrapper or claims outer authority.

use crate::{
    AuthenticatedReservationHandoffV3, Error, PresentConsiderationV2, Result,
    SettlementReceiptRouteV4, SettlementSideV1, MAX_ORDERS,
};

const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Typed exact Receipt V5 mutable-prestate data identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct SettlementReceiptDataIdV5([u8; 32]);

impl SettlementReceiptDataIdV5 {
    /// Promote one adapter-derived nonzero V5 account-data identity.
    ///
    /// This is a structural newtype, not an execution capability. The live
    /// composer must still rederive the exact V5 outer/PDA evidence.
    pub fn new(bytes: [u8; 32]) -> Result<Self> {
        if bytes == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self(bytes))
    }

    /// Exact 32-byte data identity.
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// One exact authenticated V5 receipt end presented to V4 row semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSettlementReceiptEndV5 {
    /// Canonical rent-owned receipt PDA.
    pub receipt: [u8; 32],
    /// Exact typed 298-byte V5 receipt-prestate identity.
    pub receipt_data_id: SettlementReceiptDataIdV5,
    /// Stable accounting-only transition identity derived from the V5 PDA.
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
    /// Explicit exact consideration, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// True only for the canonical end exhausting this order.
    pub completes_order: bool,
    /// Zero-based selected slice index.
    pub slice_index: u16,
    /// Exactly `slice_index + 1`.
    pub sequence: u64,
    /// Already-accounted real-end mask.
    pub accounted_end_mask: u8,
    /// Exact real ends owned by the route.
    pub expected_end_mask: u8,
    /// Present exactly on a completing buy end, including zero cash.
    pub reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl AuthenticatedSettlementReceiptEndV5 {
    pub(crate) const fn side_mask(&self) -> u8 {
        match self.side {
            SettlementSideV1::Buy => BUY_END_MASK,
            SettlementSideV1::Sell => SELL_END_MASK,
        }
    }

    /// Validate exact V5 end shape and Reservation handoff ownership.
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
                return Err(Error::InvariantViolation)
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v5_data_id_refuses_zero() {
        assert_eq!(
            SettlementReceiptDataIdV5::new([0; 32]),
            Err(Error::InvalidIdentity)
        );
    }
}
