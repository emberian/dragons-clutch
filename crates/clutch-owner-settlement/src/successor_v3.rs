//! Reservation-handoff owner settlement successor.
//!
//! V3 removes the immutable reserved-cash summary from an owner expectation.
//! Instead, each canonical terminal buy receipt end atomically hands the exact
//! authenticated Reservation cash into the mutable owner row. The row cannot
//! become accounting-complete until every expected end is consumed, every
//! expected order is complete, and the accumulated handoff funds the one
//! terminal buyer `ceil` plus the already-selected fee. V1 and V2 bodies are
//! never decoded through this module without an authenticated `0x81/3` outer
//! envelope.

use crate::{
    owner_credit_atoms, owner_debit_atoms, owner_rounding_residue_price_units, Amount,
    AuthenticatedPositionV3, Error, OwnerSettlementCreateFundingV1,
    PositionSettlementPoststateV3, PresentConsiderationV2, Result, SelectedOwnerFeeV1,
    SettlementCashPotV1, SettlementSideV1, MAX_ORDERS,
};

/// Canonical General outer tag selecting an owner-settlement row.
pub const OWNER_SETTLEMENT_OUTER_TAG_V3: u8 = 0x81;
/// Fresh General outer version selecting only the V3 semantic codec.
pub const OWNER_SETTLEMENT_OUTER_VERSION_V3: u8 = 3;
/// Exact persisted V3 owner-settlement semantic body width.
pub const OWNER_SETTLEMENT_BODY_V3_BYTES: usize = 288;
/// Fresh PDA domain for Reservation-handoff owner rows.
pub const OWNER_SETTLEMENT_PDA_DOMAIN_V3: &[u8] = b"owner-settlement:v3";
/// Fresh domain for one exact finalized V3 owner-row body.
pub const OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V3: &[u8] =
    b"clutch:owner-finalized-row-data:v3";
/// Canonical General SettlementReceipt V3 outer body width.
pub const SETTLEMENT_RECEIPT_BODY_V3_BYTES: usize = 217;
/// Fresh domain for the exact authenticated Receipt V3 prestate.
pub const SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-settlement-receipt/data/v3\0";
/// Exact Receipt V3 prestate transcript: authenticated PDA then 217 bytes.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES: usize =
    32 + SETTLEMENT_RECEIPT_BODY_V3_BYTES;

const EXPECTED_BUY_PRESENT_V3: u8 = 1 << 0;
const EXPECTED_SELL_PRESENT_V3: u8 = 1 << 1;
const CONSUMED_BUY_PRESENT_V3: u8 = 1 << 2;
const CONSUMED_SELL_PRESENT_V3: u8 = 1 << 3;
const OWNER_SETTLEMENT_PRESENCE_MASK_V3: u8 = EXPECTED_BUY_PRESENT_V3
    | EXPECTED_SELL_PRESENT_V3
    | CONSUMED_BUY_PRESENT_V3
    | CONSUMED_SELL_PRESENT_V3;
const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Direct, virtual-split, or virtual-merge route owning one V3 real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementReceiptRouteV3 {
    /// Two real order ends.
    Direct = 0,
    /// A virtual split supplies one real buy end.
    SplitToBuy = 1,
    /// One real sell end supplies a virtual merge.
    SellToMerge = 2,
}

impl SettlementReceiptRouteV3 {
    /// Exact real-end bitmap owned by this route.
    pub const fn expected_end_mask(&self) -> u8 {
        match self {
            Self::Direct => BUY_END_MASK | SELL_END_MASK,
            Self::SplitToBuy => BUY_END_MASK,
            Self::SellToMerge => SELL_END_MASK,
        }
    }
}

/// Exact V3 accounting/finalization phase encoded in the owner-row body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OwnerSettlementStateV3 {
    /// At least one expected receipt end remains unaccounted.
    Accumulating = 0,
    /// Every exact receipt end and order completion has been accounted.
    AccountingComplete = 1,
    /// The exact handoff has been allocated through Position and FinalPot.
    Finalized = 2,
}

impl OwnerSettlementStateV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Accumulating),
            1 => Ok(Self::AccountingComplete),
            2 => Ok(Self::Finalized),
            _ => Err(Error::InvalidExpectation),
        }
    }
}

/// Immutable verifier-owned owner expectation with no cash summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationV3 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    expected_buy_order_mask: u64,
    expected_sell_order_mask: u64,
    expected_slice_count: u16,
    expected_buy_price_units: PresentConsiderationV2,
    expected_sell_price_units: PresentConsiderationV2,
    selected_fee_atoms: Amount,
}

impl OwnerSettlementExpectationV3 {
    /// General MarketRuntime identity.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Counted Epoch identity.
    pub const fn epoch(&self) -> [u8; 32] {
        self.epoch
    }

    /// Final selected candidate identity.
    pub const fn candidate(&self) -> [u8; 32] {
        self.candidate
    }

    /// Semantic Position owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// Digest of the exhaustive owner/order membership book.
    pub const fn owner_order_set_digest(&self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    /// Exact collateral price scale.
    pub const fn price_scale(&self) -> Amount {
        self.price_scale
    }

    /// Expected filled buy-order mask.
    pub const fn expected_buy_order_mask(&self) -> u64 {
        self.expected_buy_order_mask
    }

    /// Expected filled sell-order mask.
    pub const fn expected_sell_order_mask(&self) -> u64 {
        self.expected_sell_order_mask
    }

    /// Exact expected real receipt-end count.
    pub const fn expected_slice_count(&self) -> u16 {
        self.expected_slice_count
    }

    /// Explicitly present aggregate buy consideration, including zero.
    pub const fn expected_buy_price_units(&self) -> PresentConsiderationV2 {
        self.expected_buy_price_units
    }

    /// Explicitly present aggregate sell consideration, including zero.
    pub const fn expected_sell_price_units(&self) -> PresentConsiderationV2 {
        self.expected_sell_price_units
    }

    /// Already-selected owner fee in whole collateral atoms.
    pub const fn selected_fee_atoms(&self) -> Amount {
        self.selected_fee_atoms
    }

    /// Validate identities, explicit side presence, masks, and fee shape.
    pub fn validate(&self) -> Result<()> {
        self.expected_buy_price_units.validate()?;
        self.expected_sell_price_units.validate()?;
        let identities = [
            self.market,
            self.epoch,
            self.candidate,
            self.owner,
            self.owner_order_set_digest,
        ];
        let mut left = 0usize;
        while left < identities.len() {
            if identities[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        let has_buy = self.expected_buy_order_mask != 0;
        let has_sell = self.expected_sell_order_mask != 0;
        if self.price_scale == 0
            || self.expected_slice_count == 0
            || (self.expected_buy_order_mask & self.expected_sell_order_mask) != 0
            || (!has_buy && !has_sell)
            || self.expected_buy_price_units.present != has_buy
            || self.expected_sell_price_units.present != has_sell
            || (!has_buy && self.selected_fee_atoms != 0)
        {
            return Err(Error::InvalidExpectation);
        }
        Ok(())
    }

    fn expected_presence_bits(&self) -> u8 {
        (if self.expected_buy_price_units.present {
            EXPECTED_BUY_PRESENT_V3
        } else {
            0
        }) | if self.expected_sell_price_units.present {
            EXPECTED_SELL_PRESENT_V3
        } else {
            0
        }
    }
}

/// One filled selected order with no caller-authored cash summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VerifiedSettlementOrderV3 {
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Exact present aggregate consideration, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// Exact real receipt-end count for this order.
    pub slice_count: u16,
}

/// Exact pre-fee owner expectation derived from selected settlement rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisV3 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    expected_buy_order_mask: u64,
    expected_sell_order_mask: u64,
    expected_slice_count: u16,
    expected_buy_price_units: PresentConsiderationV2,
    expected_sell_price_units: PresentConsiderationV2,
}

impl OwnerSettlementExpectationBasisV3 {
    /// General MarketRuntime identity.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Counted Epoch identity.
    pub const fn epoch(&self) -> [u8; 32] {
        self.epoch
    }

    /// Final candidate identity.
    pub const fn candidate(&self) -> [u8; 32] {
        self.candidate
    }

    /// Semantic owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// Exhaustive owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    /// Exact collateral price scale.
    pub const fn price_scale(&self) -> Amount {
        self.price_scale
    }

    /// Expected buy-order mask.
    pub const fn expected_buy_order_mask(&self) -> u64 {
        self.expected_buy_order_mask
    }

    /// Expected sell-order mask.
    pub const fn expected_sell_order_mask(&self) -> u64 {
        self.expected_sell_order_mask
    }

    /// Exact expected receipt-end count.
    pub const fn expected_slice_count(&self) -> u16 {
        self.expected_slice_count
    }

    /// Present aggregate buy consideration.
    pub const fn expected_buy_price_units(&self) -> PresentConsiderationV2 {
        self.expected_buy_price_units
    }

    /// Present aggregate sell consideration.
    pub const fn expected_sell_price_units(&self) -> PresentConsiderationV2 {
        self.expected_sell_price_units
    }

    /// Bind the exact fee owner's result after fee selection.
    pub fn with_selected_fee(
        self,
        selected_fee: SelectedOwnerFeeV1,
    ) -> Result<OwnerSettlementExpectationV3> {
        if selected_fee.owner != self.owner {
            return Err(Error::InvalidIdentity);
        }
        let expectation = OwnerSettlementExpectationV3 {
            market: self.market,
            epoch: self.epoch,
            candidate: self.candidate,
            owner: self.owner,
            owner_order_set_digest: self.owner_order_set_digest,
            price_scale: self.price_scale,
            expected_buy_order_mask: self.expected_buy_order_mask,
            expected_sell_order_mask: self.expected_sell_order_mask,
            expected_slice_count: self.expected_slice_count,
            expected_buy_price_units: self.expected_buy_price_units,
            expected_sell_price_units: self.expected_sell_price_units,
            selected_fee_atoms: selected_fee.fee_atoms,
        };
        expectation.validate()?;
        Ok(expectation)
    }
}

/// Complete owner-sorted pre-fee V3 basis book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisBookV3 {
    rows: [Option<OwnerSettlementExpectationBasisV3>; MAX_ORDERS],
    owner_count: u16,
}

impl OwnerSettlementExpectationBasisBookV3 {
    /// Exact participating owner count.
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    /// Return one active sorted row.
    pub fn row(&self, ordinal: u16) -> Option<OwnerSettlementExpectationBasisV3> {
        if ordinal < self.owner_count {
            self.rows[usize::from(ordinal)]
        } else {
            None
        }
    }

    /// Return the unique row for one semantic owner.
    pub fn row_for_owner(&self, owner: [u8; 32]) -> Option<OwnerSettlementExpectationBasisV3> {
        let mut index = 0usize;
        while index < usize::from(self.owner_count) {
            if let Some(row) = self.rows[index] {
                if row.owner == owner {
                    return Some(row);
                }
            }
            index += 1;
        }
        None
    }
}

/// Derive the exhaustive owner-sorted V3 pre-fee basis.
#[allow(clippy::too_many_arguments)]
pub fn build_owner_settlement_expectation_basis_book_v3(
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    orders: &[VerifiedSettlementOrderV3; MAX_ORDERS],
    order_len: u8,
) -> Result<OwnerSettlementExpectationBasisBookV3> {
    if market == [0; 32]
        || epoch == [0; 32]
        || candidate == [0; 32]
        || owner_order_set_digest == [0; 32]
        || price_scale == 0
        || order_len == 0
        || usize::from(order_len) > MAX_ORDERS
    {
        return Err(Error::InvalidExpectation);
    }
    let mut rows: [Option<OwnerSettlementExpectationBasisV3>; MAX_ORDERS] = [None; MAX_ORDERS];
    let mut owner_count = 0usize;
    let mut seen_order_mask = 0u64;
    let mut order_at = 0usize;
    while order_at < usize::from(order_len) {
        let order = orders[order_at];
        order.consideration_price_units.validate()?;
        if order.owner == [0; 32]
            || !order.consideration_price_units.present
            || order.slice_count == 0
            || usize::from(order.order_index) >= MAX_ORDERS
        {
            return Err(Error::InvalidOrder);
        }
        let bit = order_bit(order.order_index)?;
        if seen_order_mask & bit != 0 {
            return Err(Error::InvalidOrder);
        }
        seen_order_mask |= bit;
        let mut slot = 0usize;
        while slot < owner_count && rows[slot].map(|row| row.owner) != Some(order.owner) {
            slot += 1;
        }
        if slot == owner_count {
            if owner_count >= MAX_ORDERS {
                return Err(Error::ArithmeticOverflow);
            }
            rows[slot] = Some(OwnerSettlementExpectationBasisV3 {
                market,
                epoch,
                candidate,
                owner: order.owner,
                owner_order_set_digest,
                price_scale,
                expected_buy_order_mask: 0,
                expected_sell_order_mask: 0,
                expected_slice_count: 0,
                expected_buy_price_units: PresentConsiderationV2::ABSENT,
                expected_sell_price_units: PresentConsiderationV2::ABSENT,
            });
            owner_count += 1;
        }
        let mut row = rows[slot].ok_or(Error::InvariantViolation)?;
        row.expected_slice_count = row
            .expected_slice_count
            .checked_add(order.slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        match order.side {
            SettlementSideV1::Buy => {
                row.expected_buy_order_mask |= bit;
                row.expected_buy_price_units.present = true;
                row.expected_buy_price_units.value = row
                    .expected_buy_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            SettlementSideV1::Sell => {
                row.expected_sell_order_mask |= bit;
                row.expected_sell_price_units.present = true;
                row.expected_sell_price_units.value = row
                    .expected_sell_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        rows[slot] = Some(row);
        order_at += 1;
    }
    let mut at = 1usize;
    while at < owner_count {
        let value = rows[at].ok_or(Error::InvariantViolation)?;
        let mut insert = at;
        while insert > 0
            && value.owner < rows[insert - 1].ok_or(Error::InvariantViolation)?.owner
        {
            rows[insert] = rows[insert - 1];
            insert -= 1;
        }
        rows[insert] = Some(value);
        at += 1;
    }
    let mut index = 0usize;
    while index < owner_count {
        let basis = rows[index].ok_or(Error::InvariantViolation)?;
        basis
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: basis.owner,
                fee_atoms: 0,
            })?
            .validate()?;
        index += 1;
    }
    Ok(OwnerSettlementExpectationBasisBookV3 {
        rows,
        owner_count: u16::try_from(owner_count).map_err(|_| Error::ArithmeticOverflow)?,
    })
}

/// Exact authenticated Reservation cash handoff on a terminal buy end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedReservationHandoffV3 {
    reservation_account: [u8; 32],
    reservation_semantic_id: [u8; 32],
    order_id: [u8; 32],
    owner: [u8; 32],
    cash_atoms: Amount,
}

impl AuthenticatedReservationHandoffV3 {
    /// Construct an adapter-authenticated exact Reservation transition.
    pub fn new(
        reservation_account: [u8; 32],
        reservation_semantic_id: [u8; 32],
        order_id: [u8; 32],
        owner: [u8; 32],
        cash_atoms: Amount,
    ) -> Result<Self> {
        let value = Self {
            reservation_account,
            reservation_semantic_id,
            order_id,
            owner,
            cash_atoms,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for identity in [
            self.reservation_account,
            self.reservation_semantic_id,
            self.order_id,
            self.owner,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.reservation_account == self.reservation_semantic_id
            || self.reservation_account == self.order_id
            || self.reservation_account == self.owner
            || self.reservation_semantic_id == self.order_id
            || self.reservation_semantic_id == self.owner
            || self.order_id == self.owner
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }

    /// Canonical Reservation account.
    pub const fn reservation_account(&self) -> [u8; 32] {
        self.reservation_account
    }

    /// Canonical content-derived Reservation identity, distinct from its PDA.
    pub const fn reservation_semantic_id(&self) -> [u8; 32] {
        self.reservation_semantic_id
    }

    /// Canonical buy order identity.
    pub const fn order_id(&self) -> [u8; 32] {
        self.order_id
    }

    /// Semantic Reservation owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// Exact cash ownership handed from the Reservation into the row.
    pub const fn cash_atoms(&self) -> Amount {
        self.cash_atoms
    }
}

/// Typed exact Receipt V3 mutable-prestate data identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct SettlementReceiptDataIdV3([u8; 32]);

impl SettlementReceiptDataIdV3 {
    /// Exact 32-byte data identity.
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// Hash boundary for the exact authenticated Receipt V3 prestate.
pub trait SettlementReceiptDataHashV3 {
    /// Compute SHA-256 over the domain followed by the exact transcript.
    fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32];
}

/// Derive the exact V3 receipt-prestate identity from PDA and all 217 bytes.
pub fn derive_settlement_receipt_data_id_v3<H: SettlementReceiptDataHashV3>(
    authenticated_receipt_pda: [u8; 32],
    exact_receipt_body: &[u8; SETTLEMENT_RECEIPT_BODY_V3_BYTES],
    hash: &H,
) -> Result<SettlementReceiptDataIdV3> {
    if authenticated_receipt_pda == [0; 32] || exact_receipt_body[..2] != [0x0f, 3] {
        return Err(Error::InvalidAccount);
    }
    let mut transcript = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES];
    transcript[..32].copy_from_slice(&authenticated_receipt_pda);
    transcript[32..].copy_from_slice(exact_receipt_body);
    let id = hash.sha256(SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V3, &transcript);
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(SettlementReceiptDataIdV3(id))
}

/// One exact authenticated V3 receipt end presented for owner accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSettlementReceiptEndV3 {
    /// Canonical receipt PDA.
    pub receipt: [u8; 32],
    /// Exact typed receipt prestate identity.
    pub receipt_data_id: SettlementReceiptDataIdV3,
    /// Stable accounting-only transition identity derived from the receipt PDA.
    pub receipt_accounting_id: [u8; 32],
    /// General MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final candidate identity.
    pub candidate: [u8; 32],
    /// Exact owner/order-set digest.
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
    pub route: SettlementReceiptRouteV3,
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
    /// Present exactly on a completing buy end, including a zero-cash handoff.
    pub reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl AuthenticatedSettlementReceiptEndV3 {
    fn side_mask(&self) -> u8 {
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
            (SettlementReceiptRouteV3::Direct, _, 3)
            | (SettlementReceiptRouteV3::SplitToBuy, SettlementSideV1::Buy, 1)
            | (SettlementReceiptRouteV3::SellToMerge, SettlementSideV1::Sell, 2) => {}
            _ => return Err(Error::InvalidOrder),
        }
        let side = self.side_mask();
        if self.expected_end_mask & side == 0 || self.accounted_end_mask & side != 0 {
            return Err(Error::DuplicateCompletion);
        }
        match (self.side, self.completes_order, self.reservation_handoff) {
            (SettlementSideV1::Buy, true, Some(handoff)) => {
                handoff.validate()?;
                if handoff.owner != self.owner || handoff.order_id != self.order_id {
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

/// Mutable V3 row with exact Reservation cash ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccumulatorV3 {
    expectation: OwnerSettlementExpectationV3,
    buy_cash_handoff_atoms: Amount,
    consumed_buy_price_units: PresentConsiderationV2,
    consumed_sell_price_units: PresentConsiderationV2,
    completed_buy_order_mask: u64,
    completed_sell_order_mask: u64,
    consumed_slice_count: u16,
    state: OwnerSettlementStateV3,
}

impl OwnerSettlementAccumulatorV3 {
    /// Create a pristine row. Reservation cash is always initially zero.
    pub fn new(expectation: OwnerSettlementExpectationV3) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            expectation,
            buy_cash_handoff_atoms: 0,
            consumed_buy_price_units: PresentConsiderationV2::ABSENT,
            consumed_sell_price_units: PresentConsiderationV2::ABSENT,
            completed_buy_order_mask: 0,
            completed_sell_order_mask: 0,
            consumed_slice_count: 0,
            state: OwnerSettlementStateV3::Accumulating,
        })
    }

    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV3 {
        self.expectation
    }

    /// Exact cash handed off by terminal buy Reservations so far.
    pub const fn buy_cash_handoff_atoms(&self) -> Amount {
        self.buy_cash_handoff_atoms
    }

    /// Exact consumed buy consideration.
    pub const fn consumed_buy_price_units(&self) -> PresentConsiderationV2 {
        self.consumed_buy_price_units
    }

    /// Exact consumed sell consideration.
    pub const fn consumed_sell_price_units(&self) -> PresentConsiderationV2 {
        self.consumed_sell_price_units
    }

    /// Completed buy-order mask.
    pub const fn completed_buy_order_mask(&self) -> u64 {
        self.completed_buy_order_mask
    }

    /// Completed sell-order mask.
    pub const fn completed_sell_order_mask(&self) -> u64 {
        self.completed_sell_order_mask
    }

    /// Exact consumed real receipt-end count.
    pub const fn consumed_slice_count(&self) -> u16 {
        self.consumed_slice_count
    }

    /// Exact accounting/finalization phase.
    pub const fn state(&self) -> OwnerSettlementStateV3 {
        self.state
    }

    /// Consume one receipt end and its inseparable terminal-buy cash handoff.
    pub fn consume(&mut self, receipt: &AuthenticatedSettlementReceiptEndV3) -> Result<()> {
        self.validate()?;
        receipt.validate()?;
        if self.state != OwnerSettlementStateV3::Accumulating {
            return Err(Error::Terminal);
        }
        let expected = self.expectation;
        if receipt.market != expected.market
            || receipt.epoch != expected.epoch
            || receipt.candidate != expected.candidate
            || receipt.owner_order_set_digest != expected.owner_order_set_digest
            || receipt.owner != expected.owner
        {
            return Err(Error::AuthorityUnavailable);
        }
        let bit = order_bit(receipt.order_index)?;
        let mut next = *self;
        match receipt.side {
            SettlementSideV1::Buy => {
                if expected.expected_buy_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_buy_price_units.present = true;
                next.consumed_buy_price_units.value = next
                    .consumed_buy_price_units
                    .value
                    .checked_add(receipt.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_buy_price_units.value > expected.expected_buy_price_units.value {
                    return Err(Error::InvariantViolation);
                }
                if receipt.completes_order {
                    if next.completed_buy_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    let handoff = receipt
                        .reservation_handoff
                        .ok_or(Error::AuthorityUnavailable)?;
                    next.buy_cash_handoff_atoms = next
                        .buy_cash_handoff_atoms
                        .checked_add(handoff.cash_atoms)
                        .ok_or(Error::ArithmeticOverflow)?;
                    next.completed_buy_order_mask |= bit;
                }
            }
            SettlementSideV1::Sell => {
                if expected.expected_sell_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_sell_price_units.present = true;
                next.consumed_sell_price_units.value = next
                    .consumed_sell_price_units
                    .value
                    .checked_add(receipt.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_sell_price_units.value > expected.expected_sell_price_units.value {
                    return Err(Error::InvariantViolation);
                }
                if receipt.completes_order {
                    if next.completed_sell_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    next.completed_sell_order_mask |= bit;
                }
            }
        }
        next.consumed_slice_count = next
            .consumed_slice_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next.consumed_slice_count > expected.expected_slice_count {
            return Err(Error::TooManyFragments);
        }
        if next.accounting_fields_complete() {
            let required = owner_debit_atoms(
                expected.expected_buy_price_units.value,
                expected.price_scale,
                expected.selected_fee_atoms,
            )?;
            if next.buy_cash_handoff_atoms < required {
                return Err(Error::InsufficientCash);
            }
            next.state = OwnerSettlementStateV3::AccountingComplete;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn accounting_fields_complete(&self) -> bool {
        self.consumed_slice_count == self.expectation.expected_slice_count
            && self.consumed_buy_price_units == self.expectation.expected_buy_price_units
            && self.consumed_sell_price_units == self.expectation.expected_sell_price_units
            && self.completed_buy_order_mask == self.expectation.expected_buy_order_mask
            && self.completed_sell_order_mask == self.expectation.expected_sell_order_mask
    }

    /// Convert the exact handoff once at the named Floor/Ceil boundary.
    pub fn finalize(
        &mut self,
        position_cash_atoms: Amount,
        position_reserved_cash_atoms: Amount,
    ) -> Result<OwnerSettlementDispositionV3> {
        self.validate()?;
        if self.state != OwnerSettlementStateV3::AccountingComplete {
            return Err(if self.state == OwnerSettlementStateV3::Finalized {
                Error::Terminal
            } else {
                Error::Incomplete
            });
        }
        let consideration_debit_atoms = owner_debit_atoms(
            self.expectation.expected_buy_price_units.value,
            self.expectation.price_scale,
            0,
        )?;
        let total_debit_atoms = consideration_debit_atoms
            .checked_add(self.expectation.selected_fee_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.buy_cash_handoff_atoms < total_debit_atoms
            || position_reserved_cash_atoms < self.buy_cash_handoff_atoms
            || position_cash_atoms < self.buy_cash_handoff_atoms
        {
            return Err(Error::InsufficientCash);
        }
        let credit_atoms = owner_credit_atoms(
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let residue_price_units = owner_rounding_residue_price_units(
            self.expectation.expected_buy_price_units.value,
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let released_cash_atoms = self
            .buy_cash_handoff_atoms
            .checked_sub(total_debit_atoms)
            .ok_or(Error::InsufficientCash)?;
        let position_cash_without_handoff = position_cash_atoms
            .checked_sub(self.buy_cash_handoff_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        let position_cash_atoms = position_cash_without_handoff
            .checked_add(released_cash_atoms)
            .and_then(|value| value.checked_add(credit_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        let position_reserved_cash_atoms = position_reserved_cash_atoms
            .checked_sub(self.buy_cash_handoff_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        if position_reserved_cash_atoms > position_cash_atoms {
            return Err(Error::InvariantViolation);
        }
        let disposition = OwnerSettlementDispositionV3 {
            buy_cash_handoff_atoms: self.buy_cash_handoff_atoms,
            consideration_debit_atoms,
            selected_fee_atoms: self.expectation.selected_fee_atoms,
            total_debit_atoms,
            credit_atoms,
            released_cash_atoms,
            residue_price_units,
            position_cash_atoms,
            position_reserved_cash_atoms,
        };
        self.state = OwnerSettlementStateV3::Finalized;
        self.validate()?;
        Ok(disposition)
    }

    /// Validate canonical presence, exact phase, and monotone progress.
    pub fn validate(&self) -> Result<()> {
        self.expectation.validate()?;
        self.consumed_buy_price_units.validate()?;
        self.consumed_sell_price_units.validate()?;
        if self.completed_buy_order_mask & !self.expectation.expected_buy_order_mask != 0
            || self.completed_sell_order_mask & !self.expectation.expected_sell_order_mask != 0
            || self.consumed_slice_count > self.expectation.expected_slice_count
            || (self.consumed_buy_price_units.present
                && !self.expectation.expected_buy_price_units.present)
            || (self.consumed_sell_price_units.present
                && !self.expectation.expected_sell_price_units.present)
            || self.consumed_buy_price_units.value
                > self.expectation.expected_buy_price_units.value
            || self.consumed_sell_price_units.value
                > self.expectation.expected_sell_price_units.value
            || (!self.consumed_buy_price_units.present && self.completed_buy_order_mask != 0)
            || (!self.consumed_sell_price_units.present && self.completed_sell_order_mask != 0)
            || (self.consumed_slice_count == 0
                && (self.consumed_buy_price_units.present
                    || self.consumed_sell_price_units.present))
            || (self.completed_buy_order_mask == 0 && self.buy_cash_handoff_atoms != 0)
            || (self.expectation.expected_buy_order_mask == 0 && self.buy_cash_handoff_atoms != 0)
        {
            return Err(Error::InvariantViolation);
        }
        let observed_side_count = (if self.consumed_buy_price_units.present {
            1u16
        } else {
            0
        })
            .checked_add(if self.consumed_sell_price_units.present {
                1u16
            } else {
                0
            })
            .ok_or(Error::ArithmeticOverflow)?;
        if observed_side_count > self.consumed_slice_count {
            return Err(Error::InvariantViolation);
        }
        let complete = self.accounting_fields_complete();
        if (self.state == OwnerSettlementStateV3::Accumulating) == complete {
            return Err(Error::InvariantViolation);
        }
        if complete {
            let required = owner_debit_atoms(
                self.expectation.expected_buy_price_units.value,
                self.expectation.price_scale,
                self.expectation.selected_fee_atoms,
            )?;
            if self.buy_cash_handoff_atoms < required {
                return Err(Error::InsufficientCash);
            }
        }
        Ok(())
    }

    fn presence_bits(&self) -> u8 {
        self.expectation.expected_presence_bits()
            | if self.consumed_buy_price_units.present {
                CONSUMED_BUY_PRESENT_V3
            } else {
                0
            }
            | if self.consumed_sell_price_units.present {
                CONSUMED_SELL_PRESENT_V3
            } else {
                0
            }
    }

    /// Encode the exact canonical 288-byte V3 body.
    pub fn encode_body(&self) -> Result<[u8; OWNER_SETTLEMENT_BODY_V3_BYTES]> {
        self.validate()?;
        let mut output = [0u8; OWNER_SETTLEMENT_BODY_V3_BYTES];
        let mut cursor = 0usize;
        for identity in [
            self.expectation.market,
            self.expectation.epoch,
            self.expectation.candidate,
            self.expectation.owner,
            self.expectation.owner_order_set_digest,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        for value in [
            self.expectation.price_scale,
            self.expectation.expected_buy_order_mask,
            self.expectation.expected_sell_order_mask,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_slice_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_buy_price_units.value.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.expected_sell_price_units.value.to_le_bytes(),
        )?;
        for value in [
            self.expectation.selected_fee_atoms,
            self.buy_cash_handoff_atoms,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.consumed_buy_price_units.value.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consumed_sell_price_units.value.to_le_bytes(),
        )?;
        for value in [
            self.completed_buy_order_mask,
            self.completed_sell_order_mask,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.consumed_slice_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &[self.state as u8, self.presence_bits(), 0, 0],
        )?;
        if cursor != OWNER_SETTLEMENT_BODY_V3_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode hostile bytes only after authenticating the exact `0x81/3` outer.
    pub fn decode_body(outer_tag: u8, outer_version: u8, input: &[u8]) -> Result<Self> {
        if outer_tag != OWNER_SETTLEMENT_OUTER_TAG_V3
            || outer_version != OWNER_SETTLEMENT_OUTER_VERSION_V3
            || input.len() != OWNER_SETTLEMENT_BODY_V3_BYTES
        {
            return Err(Error::InvalidAccount);
        }
        decode_semantic_body(input)
    }

    /// Project one exact finalized row for typed terminal joins.
    pub fn terminal_projection(&self) -> Result<OwnerSettlementTerminalProjectionV3> {
        self.validate()?;
        if self.state != OwnerSettlementStateV3::Finalized {
            return Err(Error::Incomplete);
        }
        Ok(OwnerSettlementTerminalProjectionV3 {
            expectation: self.expectation,
            finalized_body: self.encode_body()?,
        })
    }
}

fn decode_semantic_body(input: &[u8]) -> Result<OwnerSettlementAccumulatorV3> {
    if input.len() != OWNER_SETTLEMENT_BODY_V3_BYTES {
        return Err(Error::InvalidExpectation);
    }
    let mut cursor = 0usize;
    let market = read_key(input, &mut cursor)?;
    let epoch = read_key(input, &mut cursor)?;
    let candidate = read_key(input, &mut cursor)?;
    let owner = read_key(input, &mut cursor)?;
    let owner_order_set_digest = read_key(input, &mut cursor)?;
    let price_scale = read_u64(input, &mut cursor)?;
    let expected_buy_order_mask = read_u64(input, &mut cursor)?;
    let expected_sell_order_mask = read_u64(input, &mut cursor)?;
    let expected_slice_count = read_u16(input, &mut cursor)?;
    let expected_buy_value = read_u128(input, &mut cursor)?;
    let expected_sell_value = read_u128(input, &mut cursor)?;
    let selected_fee_atoms = read_u64(input, &mut cursor)?;
    let buy_cash_handoff_atoms = read_u64(input, &mut cursor)?;
    let consumed_buy_value = read_u128(input, &mut cursor)?;
    let consumed_sell_value = read_u128(input, &mut cursor)?;
    let completed_buy_order_mask = read_u64(input, &mut cursor)?;
    let completed_sell_order_mask = read_u64(input, &mut cursor)?;
    let consumed_slice_count = read_u16(input, &mut cursor)?;
    let state = OwnerSettlementStateV3::decode(read_u8(input, &mut cursor)?)?;
    let presence = read_u8(input, &mut cursor)?;
    if presence & !OWNER_SETTLEMENT_PRESENCE_MASK_V3 != 0
        || take(input, &mut cursor, 2)? != &[0; 2]
        || cursor != input.len()
    {
        return Err(Error::InvalidExpectation);
    }
    let value = OwnerSettlementAccumulatorV3 {
        expectation: OwnerSettlementExpectationV3 {
            market,
            epoch,
            candidate,
            owner,
            owner_order_set_digest,
            price_scale,
            expected_buy_order_mask,
            expected_sell_order_mask,
            expected_slice_count,
            expected_buy_price_units: PresentConsiderationV2 {
                present: presence & EXPECTED_BUY_PRESENT_V3 != 0,
                value: expected_buy_value,
            },
            expected_sell_price_units: PresentConsiderationV2 {
                present: presence & EXPECTED_SELL_PRESENT_V3 != 0,
                value: expected_sell_value,
            },
            selected_fee_atoms,
        },
        buy_cash_handoff_atoms,
        consumed_buy_price_units: PresentConsiderationV2 {
            present: presence & CONSUMED_BUY_PRESENT_V3 != 0,
            value: consumed_buy_value,
        },
        consumed_sell_price_units: PresentConsiderationV2 {
            present: presence & CONSUMED_SELL_PRESENT_V3 != 0,
            value: consumed_sell_value,
        },
        completed_buy_order_mask,
        completed_sell_order_mask,
        consumed_slice_count,
        state,
    };
    value.validate()?;
    if value.presence_bits() != presence {
        return Err(Error::InvalidExpectation);
    }
    Ok(value)
}

/// Exact owner cash disposition derived from the mutable V3 handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementDispositionV3 {
    buy_cash_handoff_atoms: Amount,
    consideration_debit_atoms: Amount,
    selected_fee_atoms: Amount,
    total_debit_atoms: Amount,
    credit_atoms: Amount,
    released_cash_atoms: Amount,
    residue_price_units: u128,
    position_cash_atoms: Amount,
    position_reserved_cash_atoms: Amount,
}

impl OwnerSettlementDispositionV3 {
    /// Exact Reservation cash ownership removed from Position reserved cash.
    pub const fn buy_cash_handoff_atoms(&self) -> Amount {
        self.buy_cash_handoff_atoms
    }

    /// Aggregate buyer consideration converted once with ceiling.
    pub const fn consideration_debit_atoms(&self) -> Amount {
        self.consideration_debit_atoms
    }

    /// Selected owner fee included in the exact handoff debit.
    pub const fn selected_fee_atoms(&self) -> Amount {
        self.selected_fee_atoms
    }

    /// Buyer consideration plus selected fee.
    pub const fn total_debit_atoms(&self) -> Amount {
        self.total_debit_atoms
    }

    /// Aggregate seller credit converted once with floor.
    pub const fn credit_atoms(&self) -> Amount {
        self.credit_atoms
    }

    /// Excess handed-off buyer cash returned to free Position cash.
    pub const fn released_cash_atoms(&self) -> Amount {
        self.released_cash_atoms
    }

    /// Exact non-fee terminal rounding residue in price units.
    pub const fn residue_price_units(&self) -> u128 {
        self.residue_price_units
    }

    /// Exact prospective total Position cash.
    pub const fn position_cash_atoms(&self) -> Amount {
        self.position_cash_atoms
    }

    /// Exact prospective Position reserved cash.
    pub const fn position_reserved_cash_atoms(&self) -> Amount {
        self.position_reserved_cash_atoms
    }
}

/// Typed data identity of one exact finalized V3 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct OwnerFinalizedRowDataIdV3([u8; 32]);

impl OwnerFinalizedRowDataIdV3 {
    /// Exact 32-byte finalized-row identity.
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// Hash boundary for one exact finalized V3 owner row.
pub trait OwnerFinalizedRowDataHashV3 {
    /// Compute SHA-256 over the domain followed by all 288 body bytes.
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32];
}

/// Derive the typed finalized-row identity from an exact state-two body.
pub fn derive_owner_finalized_row_data_id_v3<H: OwnerFinalizedRowDataHashV3>(
    finalized_body: &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
    hash: &H,
) -> Result<OwnerFinalizedRowDataIdV3> {
    let row = decode_semantic_body(finalized_body)?;
    if row.state != OwnerSettlementStateV3::Finalized {
        return Err(Error::Incomplete);
    }
    let id = hash.sha256(OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V3, finalized_body);
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(OwnerFinalizedRowDataIdV3(id))
}

/// Immutable terminal projection from one finalized V3 row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementTerminalProjectionV3 {
    expectation: OwnerSettlementExpectationV3,
    finalized_body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
}

impl OwnerSettlementTerminalProjectionV3 {
    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV3 {
        self.expectation
    }

    /// Exact canonical finalized V3 body.
    pub const fn finalized_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES] {
        &self.finalized_body
    }
}

/// Structural V3 PDA projection supplied by the General adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementPdaProjectionV3 {
    /// Program owning the V3 seed domain.
    pub program_id: [u8; 32],
    /// Derived owner-row address.
    pub address: [u8; 32],
    /// Parent Epoch PDA seed.
    pub epoch: [u8; 32],
    /// Final selected candidate seed.
    pub candidate: [u8; 32],
    /// Semantic owner seed.
    pub owner: [u8; 32],
    /// Canonical V3 PDA bump.
    pub bump: u8,
}

impl OwnerSettlementPdaProjectionV3 {
    fn validate(&self) -> Result<()> {
        if self.program_id == [0; 32]
            || self.address == [0; 32]
            || self.epoch == [0; 32]
            || self.candidate == [0; 32]
            || self.owner == [0; 32]
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }
}

/// Strict outer-account facts for one V3 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountViewV3<'a> {
    /// Presented V3 row address.
    pub address: [u8; 32],
    /// Presented program owner.
    pub program_owner: [u8; 32],
    /// Whether the account meta is writable.
    pub writable: bool,
    /// Authenticated General outer tag.
    pub outer_tag: u8,
    /// Authenticated General outer version.
    pub outer_version: u8,
    /// Stored V3 row PDA bump.
    pub stored_bump: u8,
    /// Current lamport balance.
    pub lamports: u64,
    /// Exact current rent minimum for the 292-byte envelope.
    pub rent_minimum: u64,
    /// Exact 288-byte semantic body.
    pub body: &'a [u8],
}

/// Structurally checked V3 row projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountProjectionV3 {
    address: [u8; 32],
    program_id: [u8; 32],
    lamports: u64,
    rent_minimum: u64,
    accumulator: OwnerSettlementAccumulatorV3,
}

impl OwnerSettlementAccountProjectionV3 {
    /// Canonical owner-row PDA.
    pub const fn address(&self) -> [u8; 32] {
        self.address
    }

    /// Dragon's Clutch program identity.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Current lamports.
    pub const fn lamports(&self) -> u64 {
        self.lamports
    }

    /// Current exact rent minimum.
    pub const fn rent_minimum(&self) -> u64 {
        self.rent_minimum
    }

    /// Exact decoded semantic accumulator.
    pub const fn accumulator(&self) -> OwnerSettlementAccumulatorV3 {
        self.accumulator
    }
}

/// Project an existing exact `0x81/3` owner row.
pub fn project_owner_settlement_account_v3(
    view: OwnerSettlementAccountViewV3<'_>,
    derived: OwnerSettlementPdaProjectionV3,
) -> Result<OwnerSettlementAccountProjectionV3> {
    derived.validate()?;
    if !view.writable
        || view.address != derived.address
        || view.program_owner != derived.program_id
        || view.stored_bump != derived.bump
        || view.lamports < view.rent_minimum
        || view.body.len() != OWNER_SETTLEMENT_BODY_V3_BYTES
    {
        return Err(Error::InvalidAccount);
    }
    let accumulator = OwnerSettlementAccumulatorV3::decode_body(
        view.outer_tag,
        view.outer_version,
        view.body,
    )?;
    let expectation = accumulator.expectation;
    if expectation.epoch != derived.epoch
        || expectation.candidate != derived.candidate
        || expectation.owner != derived.owner
    {
        return Err(Error::InvalidAccount);
    }
    Ok(OwnerSettlementAccountProjectionV3 {
        address: view.address,
        program_id: derived.program_id,
        lamports: view.lamports,
        rent_minimum: view.rent_minimum,
        accumulator,
    })
}

/// Selected-candidate authority for one V3 owner-row creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SelectedOwnerRowAuthorityV3 {
    /// SelectedCandidate account PDA.
    pub selected_candidate_account: [u8; 32],
    /// Complete V3 expectation.
    pub expectation: OwnerSettlementExpectationV3,
    /// Zero-based owner-sorted row ordinal.
    pub row_ordinal: u16,
    /// Exact owner count.
    pub owner_count: u16,
    /// Present rent payer.
    pub rent_payer: [u8; 32],
    /// Sole eventual refund recipient.
    pub rent_refund_recipient: [u8; 32],
    /// Persisted rent ledger.
    pub rent_ledger: [u8; 32],
    /// Canonical prefund donation sink.
    pub donation_sink: [u8; 32],
}

impl SelectedOwnerRowAuthorityV3 {
    fn validate(&self) -> Result<()> {
        self.expectation.validate()?;
        if self.selected_candidate_account == [0; 32]
            || self.owner_count == 0
            || self.row_ordinal >= self.owner_count
            || self.rent_payer == [0; 32]
            || self.rent_refund_recipient == [0; 32]
            || self.rent_ledger == [0; 32]
            || self.donation_sink == [0; 32]
            || self.rent_ledger == self.donation_sink
            || self.rent_ledger == self.selected_candidate_account
            || self.rent_ledger == self.rent_payer
            || self.rent_ledger == self.rent_refund_recipient
            || self.donation_sink == self.selected_candidate_account
            || self.donation_sink == self.rent_payer
            || self.donation_sink == self.rent_refund_recipient
        {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(())
    }
}

/// Atomic rent-safe V3 owner-row creation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementCreatePlanV3 {
    address: [u8; 32],
    program_id: [u8; 32],
    bump: u8,
    payer_debit_lamports: u64,
    target_lamports_after: u64,
    refund_recipient: [u8; 32],
    rent_ledger: [u8; 32],
    payer_rent_principal_lamports: u64,
    prefunded_donation_lamports: u64,
    donation_sink: [u8; 32],
    body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
}

impl OwnerSettlementCreatePlanV3 {
    /// Account to allocate and assign.
    pub const fn address(&self) -> [u8; 32] {
        self.address
    }

    /// Program owner after assignment.
    pub const fn program_id(&self) -> [u8; 32] {
        self.program_id
    }

    /// Canonical stored bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    /// Present payer debit.
    pub const fn payer_debit_lamports(&self) -> u64 {
        self.payer_debit_lamports
    }

    /// Final target lamport balance.
    pub const fn target_lamports_after(&self) -> u64 {
        self.target_lamports_after
    }

    /// Sole eventual refund recipient.
    pub const fn refund_recipient(&self) -> [u8; 32] {
        self.refund_recipient
    }

    /// Persisted rent ledger.
    pub const fn rent_ledger(&self) -> [u8; 32] {
        self.rent_ledger
    }

    /// Maximum refundable payer principal.
    pub const fn payer_rent_principal_lamports(&self) -> u64 {
        self.payer_rent_principal_lamports
    }

    /// Unsolicited prefunding routed to the donation sink at closure.
    pub const fn prefunded_donation_lamports(&self) -> u64 {
        self.prefunded_donation_lamports
    }

    /// Canonical prefund donation sink.
    pub const fn donation_sink(&self) -> [u8; 32] {
        self.donation_sink
    }

    /// Exact pristine V3 semantic body.
    pub const fn body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES] {
        &self.body
    }
}

/// Prepare rent-safe creation of a pristine V3 owner row.
pub fn prepare_create_owner_settlement_account_v3(
    authority: SelectedOwnerRowAuthorityV3,
    derived: OwnerSettlementPdaProjectionV3,
    funding: OwnerSettlementCreateFundingV1,
) -> Result<OwnerSettlementCreatePlanV3> {
    authority.validate()?;
    derived.validate()?;
    if derived.epoch != authority.expectation.epoch
        || derived.candidate != authority.expectation.candidate
        || derived.owner != authority.expectation.owner
        || funding.payer == [0; 32]
        || funding.refund_recipient == [0; 32]
        || funding.payer != authority.rent_payer
        || funding.refund_recipient != authority.rent_refund_recipient
        || funding.payer == derived.address
        || funding.refund_recipient == derived.address
        || authority.rent_ledger == derived.address
        || authority.donation_sink == derived.address
        || funding.system_program_id == [0; 32]
        || funding.system_program_id == derived.program_id
        || funding.target_owner_before != funding.system_program_id
        || funding.target_data_len_before != 0
        || !funding.target_writable
        || funding.target_executable
        || funding.rent_minimum == 0
    {
        return Err(Error::InvalidAccount);
    }
    let payer_debit_lamports = funding
        .rent_minimum
        .saturating_sub(funding.target_lamports_before);
    if funding.payer_lamports < payer_debit_lamports {
        return Err(Error::InsufficientCash);
    }
    let row = OwnerSettlementAccumulatorV3::new(authority.expectation)?;
    Ok(OwnerSettlementCreatePlanV3 {
        address: derived.address,
        program_id: derived.program_id,
        bump: derived.bump,
        payer_debit_lamports,
        target_lamports_after: funding
            .target_lamports_before
            .checked_add(payer_debit_lamports)
            .ok_or(Error::ArithmeticOverflow)?,
        refund_recipient: funding.refund_recipient,
        rent_ledger: authority.rent_ledger,
        payer_rent_principal_lamports: payer_debit_lamports,
        prefunded_donation_lamports: funding.target_lamports_before,
        donation_sink: authority.donation_sink,
        body: row.encode_body()?,
    })
}

/// Non-authorizing exact row/receipt/Reservation accounting projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementReceiptAccountingProjectionV3 {
    owner_settlement_account: [u8; 32],
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
    receipt: [u8; 32],
    receipt_data_id: SettlementReceiptDataIdV3,
    receipt_accounting_id: [u8; 32],
    receipt_accounted_end_mask: u8,
    reservation_handoff: Option<AuthenticatedReservationHandoffV3>,
}

impl OwnerSettlementReceiptAccountingProjectionV3 {
    /// Owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Exact canonical V3 row successor body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES] {
        &self.owner_settlement_body
    }

    /// Receipt account to latch.
    pub const fn receipt(&self) -> [u8; 32] {
        self.receipt
    }

    /// Exact typed receipt prestate identity.
    pub const fn receipt_data_id(&self) -> SettlementReceiptDataIdV3 {
        self.receipt_data_id
    }

    /// Stable accounting-only transition identity.
    pub const fn receipt_accounting_id(&self) -> [u8; 32] {
        self.receipt_accounting_id
    }

    /// Exact next independent accounting mask.
    pub const fn receipt_accounted_end_mask(&self) -> u8 {
        self.receipt_accounted_end_mask
    }

    /// Exact authenticated Reservation handoff, present only for terminal buy.
    pub const fn reservation_handoff(&self) -> Option<AuthenticatedReservationHandoffV3> {
        self.reservation_handoff
    }
}

/// Project one exact V3 receipt end without authorizing an outer write.
pub fn project_owner_receipt_end_v3(
    account: OwnerSettlementAccountProjectionV3,
    receipt: AuthenticatedSettlementReceiptEndV3,
) -> Result<OwnerSettlementReceiptAccountingProjectionV3> {
    receipt.validate()?;
    let mut next = account.accumulator;
    next.consume(&receipt)?;
    Ok(OwnerSettlementReceiptAccountingProjectionV3 {
        owner_settlement_account: account.address,
        owner_settlement_body: next.encode_body()?,
        receipt: receipt.receipt,
        receipt_data_id: receipt.receipt_data_id,
        receipt_accounting_id: receipt.receipt_accounting_id,
        receipt_accounted_end_mask: receipt.accounted_end_mask | receipt.side_mask(),
        reservation_handoff: receipt.reservation_handoff,
    })
}

/// Structural V3 row, Position, and settlement-pot realization plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerCashRealizationPlanV3 {
    owner_settlement_account: [u8; 32],
    expectation: OwnerSettlementExpectationV3,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES],
    finalized_row_data_id: OwnerFinalizedRowDataIdV3,
    position: PositionSettlementPoststateV3,
    settlement_cash_pot: SettlementCashPotV1,
    disposition: OwnerSettlementDispositionV3,
}

impl OwnerCashRealizationPlanV3 {
    /// V3 owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Immutable selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV3 {
        self.expectation
    }

    /// Exact finalized V3 owner-row body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V3_BYTES] {
        &self.owner_settlement_body
    }

    /// Typed exact finalized-row data identity.
    pub const fn finalized_row_data_id(&self) -> OwnerFinalizedRowDataIdV3 {
        self.finalized_row_data_id
    }

    /// Exact canonical Position V3 successor.
    pub const fn position(&self) -> PositionSettlementPoststateV3 {
        self.position
    }

    /// Exact candidate-wide settlement-pot successor.
    pub const fn settlement_cash_pot(&self) -> SettlementCashPotV1 {
        self.settlement_cash_pot
    }

    /// Exact handoff-derived Floor/Ceil disposition.
    pub const fn disposition(&self) -> OwnerSettlementDispositionV3 {
        self.disposition
    }
}

/// Realize one accounting-complete V3 row without external funding summaries.
pub fn prepare_realize_owner_cash_v3<H: OwnerFinalizedRowDataHashV3>(
    account: OwnerSettlementAccountProjectionV3,
    position: AuthenticatedPositionV3,
    pot: SettlementCashPotV1,
    hash: &H,
) -> Result<OwnerCashRealizationPlanV3> {
    pot.validate()?;
    position.validate_writable()?;
    let position_prestate = position;
    let expected = account.accumulator.expectation;
    let position_fields = position.semantic.fields();
    if pot.state != 0
        || position.general_market_runtime != expected.market
        || position_fields.owner.bytes() != expected.owner
        || pot.expectation.market != expected.market
        || pot.expectation.epoch != expected.epoch
        || pot.expectation.candidate != expected.candidate
        || pot.expectation.owner_order_set_digest != expected.owner_order_set_digest
        || account.address == position.account
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next_row = account.accumulator;
    let disposition = next_row.finalize(
        position_fields.cash_atoms,
        position_fields.reserved_cash_atoms,
    )?;
    let available_consideration_atoms = pot
        .available_consideration_atoms
        .checked_add(disposition.consideration_debit_atoms)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(disposition.credit_atoms)
        .ok_or(Error::SettlementLiquidityUnavailable)?;
    let mut next_pot = pot;
    next_pot.available_consideration_atoms = available_consideration_atoms;
    next_pot.collected_fee_atoms = next_pot
        .collected_fee_atoms
        .checked_add(disposition.selected_fee_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.realized_rounding_price_units = next_pot
        .realized_rounding_price_units
        .checked_add(disposition.residue_price_units)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.finalized_owner_count = next_pot
        .finalized_owner_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_pot.finalized_owner_count == next_pot.expectation.owner_count {
        next_pot.state = 1;
    }
    next_pot.validate()?;
    let position = position_prestate.settlement_poststate(
        disposition.position_cash_atoms,
        disposition.position_reserved_cash_atoms,
        position_fields.native_eggs,
    )?;
    position.validate_successor_of(
        position_prestate,
        disposition.position_cash_atoms,
        disposition.position_reserved_cash_atoms,
        position_fields.native_eggs,
    )?;
    let owner_settlement_body = next_row.encode_body()?;
    let finalized_row_data_id =
        derive_owner_finalized_row_data_id_v3(&owner_settlement_body, hash)?;
    Ok(OwnerCashRealizationPlanV3 {
        owner_settlement_account: account.address,
        expectation: expected,
        owner_settlement_body,
        finalized_row_data_id,
        position,
        settlement_cash_pot: next_pot,
        disposition,
    })
}

fn order_bit(order_index: u8) -> Result<u64> {
    if usize::from(order_index) >= MAX_ORDERS {
        return Err(Error::InvalidOrder);
    }
    1u64
        .checked_shl(u32::from(order_index))
        .ok_or(Error::InvalidOrder)
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::InvalidExpectation)?
        .copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidExpectation)?;
    *cursor = end;
    Ok(value)
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}

const _: () = assert!(OWNER_SETTLEMENT_BODY_V3_BYTES == 288);
const _: () = assert!(SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V3_BYTES == 249);

#[cfg(test)]
mod tests {
    use super::*;

    struct PrefixHash;

    impl SettlementReceiptDataHashV3 for PrefixHash {
        fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32] {
            let mut value = [0u8; 32];
            value[0] = domain[domain.len() - 2];
            value[1] = transcript[0];
            value[2] = transcript[32];
            value
        }
    }

    impl OwnerFinalizedRowDataHashV3 for PrefixHash {
        fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
            let mut value = [0u8; 32];
            value[0] = domain[domain.len() - 1];
            value[1] = body[0];
            value
        }
    }

    fn basis(side: SettlementSideV1, consideration: u128) -> OwnerSettlementExpectationBasisV3 {
        let mut orders = [VerifiedSettlementOrderV3 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV3 {
            owner: [6; 32],
            order_index: 4,
            side,
            consideration_price_units: PresentConsiderationV2::new(consideration),
            slice_count: 1,
        };
        build_owner_settlement_expectation_basis_book_v3(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap()
    }

    fn receipt(
        side: SettlementSideV1,
        consideration: u128,
        handoff: Option<AuthenticatedReservationHandoffV3>,
    ) -> AuthenticatedSettlementReceiptEndV3 {
        AuthenticatedSettlementReceiptEndV3 {
            receipt: [11; 32],
            receipt_data_id: SettlementReceiptDataIdV3([12; 32]),
            receipt_accounting_id: [13; 32],
            market: [1; 32],
            epoch: [2; 32],
            candidate: [3; 32],
            owner_order_set_digest: [4; 32],
            owner: [6; 32],
            order_id: [7; 32],
            order_index: 4,
            side,
            route: match side {
                SettlementSideV1::Buy => SettlementReceiptRouteV3::SplitToBuy,
                SettlementSideV1::Sell => SettlementReceiptRouteV3::SellToMerge,
            },
            consideration_price_units: PresentConsiderationV2::new(consideration),
            completes_order: true,
            slice_index: 0,
            sequence: 1,
            accounted_end_mask: 0,
            expected_end_mask: match side {
                SettlementSideV1::Buy => BUY_END_MASK,
                SettlementSideV1::Sell => SELL_END_MASK,
            },
            reservation_handoff: handoff,
        }
    }

    fn handoff(cash_atoms: u64) -> AuthenticatedReservationHandoffV3 {
        AuthenticatedReservationHandoffV3::new(
            [8; 32], [9; 32], [7; 32], [6; 32], cash_atoms,
        )
        .unwrap()
    }

    #[test]
    fn zero_price_buy_is_present_and_zero_cash_handoff_is_real() {
        let expectation = basis(SettlementSideV1::Buy, 0)
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 0,
            })
            .unwrap();
        let mut row = OwnerSettlementAccumulatorV3::new(expectation).unwrap();
        row.consume(&receipt(SettlementSideV1::Buy, 0, Some(handoff(0))))
            .unwrap();
        assert_eq!(row.state(), OwnerSettlementStateV3::AccountingComplete);
        assert_eq!(row.buy_cash_handoff_atoms(), 0);
        assert_eq!(
            OwnerSettlementAccumulatorV3::decode_body(
                OWNER_SETTLEMENT_OUTER_TAG_V3,
                OWNER_SETTLEMENT_OUTER_VERSION_V3,
                &row.encode_body().unwrap(),
            ),
            Ok(row)
        );
    }

    #[test]
    fn terminal_buy_requires_authenticated_handoff_even_when_amount_is_zero() {
        let expectation = basis(SettlementSideV1::Buy, 0)
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 0,
            })
            .unwrap();
        let mut row = OwnerSettlementAccumulatorV3::new(expectation).unwrap();
        assert_eq!(
            row.consume(&receipt(SettlementSideV1::Buy, 0, None)),
            Err(Error::AuthorityUnavailable)
        );
    }

    #[test]
    fn handoff_is_forbidden_on_sell_and_nonterminal_buy() {
        let sell = receipt(SettlementSideV1::Sell, 100, Some(handoff(2)));
        assert_eq!(sell.validate(), Err(Error::InvariantViolation));
        let mut buy = receipt(SettlementSideV1::Buy, 50, Some(handoff(2)));
        buy.completes_order = false;
        assert_eq!(buy.validate(), Err(Error::InvariantViolation));
    }

    #[test]
    fn accounting_complete_refuses_underfunded_handoff() {
        let expectation = basis(SettlementSideV1::Buy, 101)
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 2,
            })
            .unwrap();
        let mut row = OwnerSettlementAccumulatorV3::new(expectation).unwrap();
        assert_eq!(
            row.consume(&receipt(SettlementSideV1::Buy, 101, Some(handoff(3)))),
            Err(Error::InsufficientCash)
        );
        assert_eq!(row.state(), OwnerSettlementStateV3::Accumulating);
    }

    #[test]
    fn finalization_removes_exact_handoff_then_releases_excess() {
        let expectation = basis(SettlementSideV1::Buy, 101)
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 2,
            })
            .unwrap();
        let mut row = OwnerSettlementAccumulatorV3::new(expectation).unwrap();
        row.consume(&receipt(SettlementSideV1::Buy, 101, Some(handoff(10))))
            .unwrap();
        let disposition = row.finalize(30, 12).unwrap();
        assert_eq!(disposition.buy_cash_handoff_atoms(), 10);
        assert_eq!(disposition.consideration_debit_atoms(), 2);
        assert_eq!(disposition.total_debit_atoms(), 4);
        assert_eq!(disposition.released_cash_atoms(), 6);
        assert_eq!(disposition.position_cash_atoms(), 26);
        assert_eq!(disposition.position_reserved_cash_atoms(), 2);
        assert_eq!(row.state(), OwnerSettlementStateV3::Finalized);
    }

    #[test]
    fn hostile_outer_versions_and_padding_are_refused() {
        let expectation = basis(SettlementSideV1::Buy, 0)
            .with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 0,
            })
            .unwrap();
        let row = OwnerSettlementAccumulatorV3::new(expectation).unwrap();
        let mut body = row.encode_body().unwrap();
        assert_eq!(
            OwnerSettlementAccumulatorV3::decode_body(0x81, 2, &body),
            Err(Error::InvalidAccount)
        );
        body[287] = 1;
        assert_eq!(
            OwnerSettlementAccumulatorV3::decode_body(0x81, 3, &body),
            Err(Error::InvalidExpectation)
        );
    }

    #[test]
    fn receipt_data_id_refuses_non_v3_outer_bytes() {
        let mut body = [0u8; SETTLEMENT_RECEIPT_BODY_V3_BYTES];
        body[..2].copy_from_slice(&[0x0f, 2]);
        assert_eq!(
            derive_settlement_receipt_data_id_v3([1; 32], &body, &PrefixHash),
            Err(Error::InvalidAccount)
        );
        body[1] = 3;
        assert!(derive_settlement_receipt_data_id_v3([1; 32], &body, &PrefixHash).is_ok());
    }
}
