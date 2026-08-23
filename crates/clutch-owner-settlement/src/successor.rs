//! Presence-explicit owner settlement successor.
//!
//! V1 treated zero consideration as absence at several independent joins.
//! That excludes legitimate selected slices whose exact grid price is zero.
//! V2 carries presence as a checked bit and permits a present value of zero.
//! Its persisted accumulator remains exactly 288 bytes: one byte of V1's
//! reserved tail is now a canonical presence bitmap and the remaining two
//! bytes stay zero. A V2 outer account version must select this codec; V1 bytes
//! are never reinterpreted.

use crate::{
    owner_credit_atoms, owner_debit_atoms, owner_rounding_residue_price_units, Amount, Error,
    AuthenticatedPositionV3, OwnerSettlementCreateFundingV1, OwnerSettlementDispositionV1,
    PositionSettlementPoststateV3, Result, SelectedOwnerFeeV1, SettlementCashPotV1,
    SettlementSideV1, MAX_ORDERS,
};

/// Exact persisted V2 owner-settlement semantic body width.
pub const OWNER_SETTLEMENT_BODY_V2_BYTES: usize = 288;
/// PDA domain for presence-explicit owner settlement rows.
pub const OWNER_SETTLEMENT_PDA_DOMAIN_V2: &[u8] = b"owner-settlement:v2";
/// Domain prepended before hashing one finalized V2 owner-row body.
pub const OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V2: &[u8] = b"clutch:owner-finalized-row-data:v2";
/// Domain for the exact immutable-and-latch receipt projection transcript.
pub const SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V2: &[u8] = b"clutch:settlement-receipt-data:v2";
/// Exact receipt projection transcript width hashed into its data ID.
pub const SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES: usize = 344;

const EXPECTED_BUY_PRESENT_V2: u8 = 1 << 0;
const EXPECTED_SELL_PRESENT_V2: u8 = 1 << 1;
const CONSUMED_BUY_PRESENT_V2: u8 = 1 << 2;
const CONSUMED_SELL_PRESENT_V2: u8 = 1 << 3;
const OWNER_SETTLEMENT_PRESENCE_MASK_V2: u8 = EXPECTED_BUY_PRESENT_V2
    | EXPECTED_SELL_PRESENT_V2
    | CONSUMED_BUY_PRESENT_V2
    | CONSUMED_SELL_PRESENT_V2;

const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Structural PDA projection supplied by the General V2 adapter.
///
/// This type is not authority. The adapter must independently derive the
/// address from `owner-settlement:v2` before consuming any returned plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementPdaProjectionV2 {
    /// Program owning this seed domain.
    pub program_id: [u8; 32],
    /// Derived V2 row address.
    pub address: [u8; 32],
    /// Parent Epoch PDA seed.
    pub epoch: [u8; 32],
    /// Final selected candidate identity seed.
    pub candidate: [u8; 32],
    /// Semantic Position owner seed.
    pub owner: [u8; 32],
    /// Canonical bump returned by V2 derivation.
    pub bump: u8,
}

impl OwnerSettlementPdaProjectionV2 {
    fn validate(self) -> Result<()> {
        if self.program_id == [0; 32]
            || self.address == [0; 32]
            || self.epoch == [0; 32]
            || self.candidate == [0; 32]
            || self.owner == [0; 32]
        {
            Err(Error::InvalidAccount)
        } else {
            Ok(())
        }
    }
}

/// Structural outer-account facts for a strict V2 body projection.
///
/// This type does not authenticate the tag, version, owner, or PDA. The
/// General adapter must decode `OwnerSettlementV2AccountV1` and derive the V2
/// PDA before treating the returned projection as part of an atomic plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountViewV2<'a> {
    /// Presented V2 row address.
    pub address: [u8; 32],
    /// Presented program owner.
    pub program_owner: [u8; 32],
    /// Whether the account meta is writable.
    pub writable: bool,
    /// Bump stored outside the exact semantic body.
    pub stored_bump: u8,
    /// Current lamport balance.
    pub lamports: u64,
    /// Exact rent minimum for the 292-byte envelope.
    pub rent_minimum: u64,
    /// Exact V2 semantic body bytes.
    pub body: &'a [u8],
}

/// Explicitly present or absent exact integer price.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PresentPriceV2 {
    /// Whether a selected receipt supplied this value.
    pub present: bool,
    /// Exact integer-grid price; zero is valid when `present` is true.
    pub value: Amount,
}

impl PresentPriceV2 {
    /// Canonical absent value.
    pub const ABSENT: Self = Self {
        present: false,
        value: 0,
    };

    /// Construct one explicitly present price, including zero.
    pub const fn new(value: Amount) -> Self {
        Self {
            present: true,
            value,
        }
    }

    /// Validate absence padding.
    pub const fn validate(self) -> Result<()> {
        if !self.present && self.value != 0 {
            Err(Error::InvalidOrder)
        } else {
            Ok(())
        }
    }
}

/// Explicitly present or absent exact consideration in price units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PresentConsiderationV2 {
    /// Whether authenticated selected economics supplied this value.
    pub present: bool,
    /// Exact consideration; zero is valid when `present` is true.
    pub value: u128,
}

impl PresentConsiderationV2 {
    /// Canonical absent value.
    pub const ABSENT: Self = Self {
        present: false,
        value: 0,
    };

    /// Construct one explicitly present consideration, including zero.
    pub const fn new(value: u128) -> Self {
        Self {
            present: true,
            value,
        }
    }

    /// Validate absence padding.
    pub const fn validate(self) -> Result<()> {
        if !self.present && self.value != 0 {
            Err(Error::InvalidOrder)
        } else {
            Ok(())
        }
    }
}

/// Immutable verifier-owned expectation for one participating owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementExpectationV2 {
    /// Canonical General V2 MarketRuntime identity.
    pub market: [u8; 32],
    /// Canonical counted Epoch identity.
    pub epoch: [u8; 32],
    /// Selected candidate identity.
    pub candidate: [u8; 32],
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Digest of exact ordered owner/order/reservation/Position membership.
    pub owner_order_set_digest: [u8; 32],
    /// Exact collateral price scale.
    pub price_scale: Amount,
    /// Filled buy-order indices.
    pub expected_buy_order_mask: u64,
    /// Filled sell-order indices.
    pub expected_sell_order_mask: u64,
    /// Exact real receipt-end count.
    pub expected_slice_count: u16,
    /// Aggregate payer consideration and its explicit presence.
    pub expected_buy_price_units: PresentConsiderationV2,
    /// Aggregate payee consideration and its explicit presence.
    pub expected_sell_price_units: PresentConsiderationV2,
    /// Selected owner fee in whole collateral atoms.
    pub selected_fee_atoms: Amount,
    /// Exact cash reserved across filled buy orders.
    pub reserved_cash_atoms: Amount,
}

impl OwnerSettlementExpectationV2 {
    /// Canonical inactive fixed-capacity row.
    pub const EMPTY: Self = Self {
        market: [0; 32],
        epoch: [0; 32],
        candidate: [0; 32],
        owner: [0; 32],
        owner_order_set_digest: [0; 32],
        price_scale: 0,
        expected_buy_order_mask: 0,
        expected_sell_order_mask: 0,
        expected_slice_count: 0,
        expected_buy_price_units: PresentConsiderationV2::ABSENT,
        expected_sell_price_units: PresentConsiderationV2::ABSENT,
        selected_fee_atoms: 0,
        reserved_cash_atoms: 0,
    };

    /// Validate identities, explicit presence, masks, and exact funding.
    pub fn validate(self) -> Result<()> {
        self.expected_buy_price_units.validate()?;
        self.expected_sell_price_units.validate()?;
        let keys = [
            self.market,
            self.epoch,
            self.candidate,
            self.owner,
            self.owner_order_set_digest,
        ];
        let mut left = 0usize;
        while left < keys.len() {
            if keys[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < keys.len() {
                if keys[left] == keys[right] {
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
        {
            return Err(Error::InvalidExpectation);
        }
        let required = owner_debit_atoms(
            self.expected_buy_price_units.value,
            self.price_scale,
            self.selected_fee_atoms,
        )?;
        if self.reserved_cash_atoms < required {
            return Err(Error::InsufficientCash);
        }
        Ok(())
    }

    const fn expected_presence_bits(self) -> u8 {
        (if self.expected_buy_price_units.present {
            EXPECTED_BUY_PRESENT_V2
        } else {
            0
        }) | (if self.expected_sell_price_units.present {
            EXPECTED_SELL_PRESENT_V2
        } else {
            0
        })
    }
}

/// Exact pre-fee owner expectation derived only from selected settlement rows.
///
/// Fields are private so fee code cannot invent side presence, consideration,
/// order masks, or Reservation funding. The full materializer constructs a
/// complete basis book from verified orders; the fee owner consumes one basis
/// and returns [`SelectedOwnerFeeV1`], after which [`Self::with_selected_fee`]
/// mints the final persisted expectation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisV2 {
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
    reserved_cash_atoms: Amount,
}

impl OwnerSettlementExpectationBasisV2 {
    /// Project the pre-fee facts from an already sealed expectation.
    ///
    /// This validates the final expectation and strips only its selected fee.
    /// It confers no account authentication or materialization authority.
    pub fn from_expectation(expectation: OwnerSettlementExpectationV2) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            market: expectation.market,
            epoch: expectation.epoch,
            candidate: expectation.candidate,
            owner: expectation.owner,
            owner_order_set_digest: expectation.owner_order_set_digest,
            price_scale: expectation.price_scale,
            expected_buy_order_mask: expectation.expected_buy_order_mask,
            expected_sell_order_mask: expectation.expected_sell_order_mask,
            expected_slice_count: expectation.expected_slice_count,
            expected_buy_price_units: expectation.expected_buy_price_units,
            expected_sell_price_units: expectation.expected_sell_price_units,
            reserved_cash_atoms: expectation.reserved_cash_atoms,
        })
    }

    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    pub const fn epoch(self) -> [u8; 32] {
        self.epoch
    }

    pub const fn candidate(self) -> [u8; 32] {
        self.candidate
    }

    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    pub const fn owner_order_set_digest(self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    pub const fn price_scale(self) -> Amount {
        self.price_scale
    }

    pub const fn expected_buy_order_mask(self) -> u64 {
        self.expected_buy_order_mask
    }

    pub const fn expected_sell_order_mask(self) -> u64 {
        self.expected_sell_order_mask
    }

    pub const fn expected_slice_count(self) -> u16 {
        self.expected_slice_count
    }

    pub const fn expected_buy_price_units(self) -> PresentConsiderationV2 {
        self.expected_buy_price_units
    }

    pub const fn expected_sell_price_units(self) -> PresentConsiderationV2 {
        self.expected_sell_price_units
    }

    pub const fn reserved_cash_atoms(self) -> Amount {
        self.reserved_cash_atoms
    }

    /// Bind the fee owner's exact row and mint the final persisted expectation.
    pub fn with_selected_fee(
        self,
        selected_fee: SelectedOwnerFeeV1,
    ) -> Result<OwnerSettlementExpectationV2> {
        if selected_fee.owner != self.owner {
            return Err(Error::InvalidIdentity);
        }
        let expectation = OwnerSettlementExpectationV2 {
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
            reserved_cash_atoms: self.reserved_cash_atoms,
        };
        expectation.validate()?;
        Ok(expectation)
    }
}

/// Complete sorted pre-fee owner basis book for one selected candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementExpectationBasisBookV2 {
    rows: [Option<OwnerSettlementExpectationBasisV2>; MAX_ORDERS],
    owner_count: u16,
}

impl OwnerSettlementExpectationBasisBookV2 {
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    pub fn row(&self, ordinal: u16) -> Option<OwnerSettlementExpectationBasisV2> {
        if ordinal < self.owner_count {
            self.rows[usize::from(ordinal)]
        } else {
            None
        }
    }

    pub fn row_for_owner(&self, owner: [u8; 32]) -> Option<OwnerSettlementExpectationBasisV2> {
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

/// Derive the exhaustive pre-fee owner basis from exact selected order rows.
#[allow(clippy::too_many_arguments)]
pub fn build_owner_settlement_expectation_basis_book_v2(
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    orders: &[VerifiedSettlementOrderV2; MAX_ORDERS],
    order_len: u8,
) -> Result<OwnerSettlementExpectationBasisBookV2> {
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
    let mut rows: [Option<OwnerSettlementExpectationBasisV2>; MAX_ORDERS] =
        [None; MAX_ORDERS];
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
            || (order.side == SettlementSideV1::Sell && order.reserved_cash_atoms != 0)
        {
            return Err(Error::InvalidOrder);
        }
        let bit = order_bit(order.order_index)?;
        if seen_order_mask & bit != 0 {
            return Err(Error::InvalidOrder);
        }
        seen_order_mask |= bit;
        let mut slot = 0usize;
        while slot < owner_count {
            if rows[slot].map(|row| row.owner) == Some(order.owner) {
                break;
            }
            slot += 1;
        }
        if slot == owner_count {
            if owner_count >= MAX_ORDERS {
                return Err(Error::ArithmeticOverflow);
            }
            rows[slot] = Some(OwnerSettlementExpectationBasisV2 {
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
                reserved_cash_atoms: 0,
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
                row.reserved_cash_atoms = row
                    .reserved_cash_atoms
                    .checked_add(order.reserved_cash_atoms)
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
            && value.owner
                < rows[insert - 1]
                    .ok_or(Error::InvariantViolation)?
                    .owner
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
        OwnerSettlementExpectationV2 {
            market: basis.market,
            epoch: basis.epoch,
            candidate: basis.candidate,
            owner: basis.owner,
            owner_order_set_digest: basis.owner_order_set_digest,
            price_scale: basis.price_scale,
            expected_buy_order_mask: basis.expected_buy_order_mask,
            expected_sell_order_mask: basis.expected_sell_order_mask,
            expected_slice_count: basis.expected_slice_count,
            expected_buy_price_units: basis.expected_buy_price_units,
            expected_sell_price_units: basis.expected_sell_price_units,
            selected_fee_atoms: 0,
            reserved_cash_atoms: basis.reserved_cash_atoms,
        }
        .validate()?;
        index += 1;
    }
    Ok(OwnerSettlementExpectationBasisBookV2 {
        rows,
        owner_count: u16::try_from(owner_count).map_err(|_| Error::ArithmeticOverflow)?,
    })
}

/// One exact authenticated receipt end for the V2 accumulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOwnerFragmentV2 {
    /// Canonical order-set index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Explicitly present exact consideration.
    pub consideration_price_units: PresentConsiderationV2,
    /// True only for the unique end completing this order.
    pub completes_order: bool,
}

/// Mutable presence-explicit owner accumulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementAccumulatorV2 {
    /// Immutable verifier-owned expectation.
    pub expectation: OwnerSettlementExpectationV2,
    /// Exact consumed buy consideration with explicit observed presence.
    pub consumed_buy_price_units: PresentConsiderationV2,
    /// Exact consumed sell consideration with explicit observed presence.
    pub consumed_sell_price_units: PresentConsiderationV2,
    /// Completed buy-order bitmap.
    pub completed_buy_order_mask: u64,
    /// Completed sell-order bitmap.
    pub completed_sell_order_mask: u64,
    /// Consumed real receipt-end count.
    pub consumed_slice_count: u16,
    /// Zero accumulating, one finalized, two retired.
    pub state: u8,
}

/// Immutable terminal projection from one exact finalized V2 row.
///
/// This value contains no account/PDA authority. Retirement and fee adapters
/// must bind its exact body to the strictly decoded `0x81/2` account and the
/// canonical finalized-row data ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementTerminalProjectionV2 {
    expectation: OwnerSettlementExpectationV2,
    finalized_body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
}

impl OwnerSettlementTerminalProjectionV2 {
    /// Immutable presence-explicit selected expectation.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV2 {
        self.expectation
    }

    /// Exact canonical state-one row body.
    pub const fn finalized_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V2_BYTES] {
        &self.finalized_body
    }
}

impl OwnerSettlementAccumulatorV2 {
    /// Create an empty V2 accumulator.
    pub fn new(expectation: OwnerSettlementExpectationV2) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            expectation,
            consumed_buy_price_units: PresentConsiderationV2::ABSENT,
            consumed_sell_price_units: PresentConsiderationV2::ABSENT,
            completed_buy_order_mask: 0,
            completed_sell_order_mask: 0,
            consumed_slice_count: 0,
            state: 0,
        })
    }

    /// Project one finalized row for typed fee and retirement joins.
    pub fn terminal_projection(self) -> Result<OwnerSettlementTerminalProjectionV2> {
        self.validate()?;
        if self.state != 1 {
            return Err(Error::Incomplete);
        }
        Ok(OwnerSettlementTerminalProjectionV2 {
            expectation: self.expectation,
            finalized_body: self.encode_body()?,
        })
    }

    /// Consume one present receipt end without applying collateral rounding.
    pub fn consume(&mut self, fragment: AuthenticatedOwnerFragmentV2) -> Result<()> {
        self.validate()?;
        fragment.consideration_price_units.validate()?;
        if self.state != 0 {
            return Err(Error::Terminal);
        }
        if !fragment.consideration_price_units.present {
            return Err(Error::InvalidOrder);
        }
        let bit = order_bit(fragment.order_index)?;
        let mut next = *self;
        match fragment.side {
            SettlementSideV1::Buy => {
                if self.expectation.expected_buy_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_buy_price_units.present = true;
                next.consumed_buy_price_units.value = next
                    .consumed_buy_price_units
                    .value
                    .checked_add(fragment.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_buy_price_units.value
                    > self.expectation.expected_buy_price_units.value
                {
                    return Err(Error::InvariantViolation);
                }
                if fragment.completes_order {
                    if next.completed_buy_order_mask & bit != 0 {
                        return Err(Error::DuplicateCompletion);
                    }
                    next.completed_buy_order_mask |= bit;
                }
            }
            SettlementSideV1::Sell => {
                if self.expectation.expected_sell_order_mask & bit == 0 {
                    return Err(Error::InvalidOrder);
                }
                next.consumed_sell_price_units.present = true;
                next.consumed_sell_price_units.value = next
                    .consumed_sell_price_units
                    .value
                    .checked_add(fragment.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.consumed_sell_price_units.value
                    > self.expectation.expected_sell_price_units.value
                {
                    return Err(Error::InvariantViolation);
                }
                if fragment.completes_order {
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
        if next.consumed_slice_count > self.expectation.expected_slice_count {
            return Err(Error::TooManyFragments);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Convert exact owner totals once at the unchanged Floor/Ceil boundary.
    pub fn finalize(
        &mut self,
        position_cash_atoms: Amount,
        position_reserved_cash_atoms: Amount,
    ) -> Result<OwnerSettlementDispositionV1> {
        self.validate()?;
        if self.state != 0 {
            return Err(Error::Terminal);
        }
        if self.consumed_slice_count != self.expectation.expected_slice_count
            || self.consumed_buy_price_units != self.expectation.expected_buy_price_units
            || self.consumed_sell_price_units != self.expectation.expected_sell_price_units
            || self.completed_buy_order_mask != self.expectation.expected_buy_order_mask
            || self.completed_sell_order_mask != self.expectation.expected_sell_order_mask
        {
            return Err(Error::Incomplete);
        }
        if position_reserved_cash_atoms < self.expectation.reserved_cash_atoms
            || position_cash_atoms < self.expectation.reserved_cash_atoms
        {
            return Err(Error::InsufficientCash);
        }
        let debit_atoms = owner_debit_atoms(
            self.expectation.expected_buy_price_units.value,
            self.expectation.price_scale,
            self.expectation.selected_fee_atoms,
        )?;
        let credit_atoms = owner_credit_atoms(
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let residue_price_units = owner_rounding_residue_price_units(
            self.expectation.expected_buy_price_units.value,
            self.expectation.expected_sell_price_units.value,
            self.expectation.price_scale,
        )?;
        let next_cash = position_cash_atoms
            .checked_sub(debit_atoms)
            .and_then(|value| value.checked_add(credit_atoms))
            .ok_or(Error::ArithmeticUnderflow)?;
        let next_reserved = position_reserved_cash_atoms
            .checked_sub(self.expectation.reserved_cash_atoms)
            .ok_or(Error::ArithmeticUnderflow)?;
        if next_reserved > next_cash {
            return Err(Error::InvariantViolation);
        }
        let released_cash_atoms = self
            .expectation
            .reserved_cash_atoms
            .checked_sub(debit_atoms)
            .ok_or(Error::InsufficientCash)?;
        let mut next = *self;
        next.state = 1;
        next.validate()?;
        *self = next;
        Ok(OwnerSettlementDispositionV1 {
            debit_atoms,
            credit_atoms,
            selected_fee_atoms: self.expectation.selected_fee_atoms,
            released_cash_atoms,
            residue_price_units,
            position_cash_atoms: next_cash,
            position_reserved_cash_atoms: next_reserved,
        })
    }

    /// Validate canonical presence, monotone bounds, and terminal completion.
    pub fn validate(self) -> Result<()> {
        self.expectation.validate()?;
        self.consumed_buy_price_units.validate()?;
        self.consumed_sell_price_units.validate()?;
        if self.state > 2
            || self.completed_buy_order_mask & !self.expectation.expected_buy_order_mask != 0
            || self.completed_sell_order_mask & !self.expectation.expected_sell_order_mask != 0
            || self.consumed_slice_count > self.expectation.expected_slice_count
            || (self.consumed_buy_price_units.present
                && !self.expectation.expected_buy_price_units.present)
            || (self.consumed_sell_price_units.present
                && !self.expectation.expected_sell_price_units.present)
            || self.consumed_buy_price_units.value > self.expectation.expected_buy_price_units.value
            || self.consumed_sell_price_units.value
                > self.expectation.expected_sell_price_units.value
            || (!self.consumed_buy_price_units.present && self.completed_buy_order_mask != 0)
            || (!self.consumed_sell_price_units.present && self.completed_sell_order_mask != 0)
            || (self.consumed_slice_count == 0
                && (self.consumed_buy_price_units.present
                    || self.consumed_sell_price_units.present))
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
        if self.state != 0
            && (self.consumed_slice_count != self.expectation.expected_slice_count
                || self.consumed_buy_price_units != self.expectation.expected_buy_price_units
                || self.consumed_sell_price_units != self.expectation.expected_sell_price_units
                || self.completed_buy_order_mask != self.expectation.expected_buy_order_mask
                || self.completed_sell_order_mask != self.expectation.expected_sell_order_mask)
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    fn presence_bits(self) -> u8 {
        self.expectation.expected_presence_bits()
            | if self.consumed_buy_price_units.present {
                CONSUMED_BUY_PRESENT_V2
            } else {
                0
            }
            | if self.consumed_sell_price_units.present {
                CONSUMED_SELL_PRESENT_V2
            } else {
                0
            }
    }

    /// Encode the exact 288-byte presence-explicit semantic body.
    pub fn encode_body(self) -> Result<[u8; OWNER_SETTLEMENT_BODY_V2_BYTES]> {
        self.validate()?;
        let mut output = [0u8; OWNER_SETTLEMENT_BODY_V2_BYTES];
        let mut cursor = 0usize;
        for key in [
            self.expectation.market,
            self.expectation.epoch,
            self.expectation.candidate,
            self.expectation.owner,
            self.expectation.owner_order_set_digest,
        ] {
            put(&mut output, &mut cursor, &key)?;
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
            &self
                .expectation
                .expected_buy_price_units
                .value
                .to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self
                .expectation
                .expected_sell_price_units
                .value
                .to_le_bytes(),
        )?;
        for value in [
            self.expectation.selected_fee_atoms,
            self.expectation.reserved_cash_atoms,
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
            &[self.state, self.presence_bits()],
        )?;
        put(&mut output, &mut cursor, &[0; 2])?;
        if cursor != OWNER_SETTLEMENT_BODY_V2_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode and validate one exact hostile 288-byte V2 body.
    pub fn decode_body(input: &[u8]) -> Result<Self> {
        if input.len() != OWNER_SETTLEMENT_BODY_V2_BYTES {
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
        let reserved_cash_atoms = read_u64(input, &mut cursor)?;
        let consumed_buy_value = read_u128(input, &mut cursor)?;
        let consumed_sell_value = read_u128(input, &mut cursor)?;
        let completed_buy_order_mask = read_u64(input, &mut cursor)?;
        let completed_sell_order_mask = read_u64(input, &mut cursor)?;
        let consumed_slice_count = read_u16(input, &mut cursor)?;
        let state = read_u8(input, &mut cursor)?;
        let presence = read_u8(input, &mut cursor)?;
        if presence & !OWNER_SETTLEMENT_PRESENCE_MASK_V2 != 0
            || take(input, &mut cursor, 2)? != &[0; 2]
            || cursor != input.len()
        {
            return Err(Error::InvalidExpectation);
        }
        let value = Self {
            expectation: OwnerSettlementExpectationV2 {
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
                    present: presence & EXPECTED_BUY_PRESENT_V2 != 0,
                    value: expected_buy_value,
                },
                expected_sell_price_units: PresentConsiderationV2 {
                    present: presence & EXPECTED_SELL_PRESENT_V2 != 0,
                    value: expected_sell_value,
                },
                selected_fee_atoms,
                reserved_cash_atoms,
            },
            consumed_buy_price_units: PresentConsiderationV2 {
                present: presence & CONSUMED_BUY_PRESENT_V2 != 0,
                value: consumed_buy_value,
            },
            consumed_sell_price_units: PresentConsiderationV2 {
                present: presence & CONSUMED_SELL_PRESENT_V2 != 0,
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
}

/// One filled order with explicitly present aggregate consideration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VerifiedSettlementOrderV2 {
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Present aggregate order consideration, including legitimate zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// Exact receipt ends consuming this order.
    pub slice_count: u16,
    /// Exact buy cash reservation; zero for sells.
    pub reserved_cash_atoms: Amount,
}

/// Frozen order-set membership with presence-explicit selected economics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOrderMembershipV2 {
    /// General MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the complete ordered owner/order set.
    pub owner_order_set_digest: [u8; 32],
    /// Canonical order identity.
    pub order_id: [u8; 32],
    /// Canonical Reservation identity.
    pub reservation: [u8; 32],
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Canonical order-set index.
    pub order_index: u8,
    /// Order generation frozen in the page row.
    pub order_generation: u64,
    /// Position generation frozen at placement.
    pub position_generation: u64,
    /// Buy or sell side.
    pub side: SettlementSideV1,
    /// Scalar or portfolio order family.
    pub order_kind: crate::OrderKindV1,
    /// Active outcome width.
    pub outcome_count: u8,
    /// Scalar outcome, or `u8::MAX` for a portfolio.
    pub single_outcome: u8,
    /// Exact entitled Egg units across every selected slice.
    pub entitled_units: Amount,
    /// Explicit selected consideration, including legitimate zero.
    pub entitled_consideration_price_units: PresentConsiderationV2,
}

impl AuthenticatedOrderMembershipV2 {
    /// Validate immutable membership and explicit selected economics.
    pub fn validate(self) -> Result<()> {
        self.entitled_consideration_price_units.validate()?;
        for key in [
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.order_id,
            self.reservation,
            self.owner,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if usize::from(self.order_index) >= MAX_ORDERS
            || self.order_generation == 0
            || self.position_generation == 0
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > crate::MAX_OUTCOMES
            || self.entitled_units == 0
            || !self.entitled_consideration_price_units.present
        {
            return Err(Error::InvalidOrder);
        }
        match self.order_kind {
            crate::OrderKindV1::Single if self.single_outcome >= self.outcome_count => {
                return Err(Error::InvalidOrder);
            }
            crate::OrderKindV1::Portfolio if self.single_outcome != u8::MAX => {
                return Err(Error::InvalidOrder);
            }
            crate::OrderKindV1::Single | crate::OrderKindV1::Portfolio => {}
        }
        Ok(())
    }
}

/// Presence-explicit candidate totals recomputed from exact order rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CandidateSettlementTotalsV2 {
    /// Exact participating owner count.
    pub owner_count: u16,
    /// Aggregate buy consideration and explicit side presence.
    pub buy_price_units: PresentConsiderationV2,
    /// Aggregate sell consideration and explicit side presence.
    pub sell_price_units: PresentConsiderationV2,
    /// Exact selected fee total.
    pub selected_fee_atoms: u128,
    /// Exact TerminalOwnerFloor/Ceil rounding residue.
    pub rounding_pot_price_units: u128,
    /// Exact real receipt-end count.
    pub owner_slice_end_count: u16,
}

/// Canonically owner-sorted V2 settlement expectations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementBookV2 {
    /// Active sorted owner prefix.
    pub rows: [OwnerSettlementExpectationV2; MAX_ORDERS],
    /// Active row count.
    pub owner_count: u16,
    /// Whole owner debits including selected fees.
    pub debit_atoms: u128,
    /// Whole owner credits.
    pub credit_atoms: u128,
    /// Exact non-fee rounding residue.
    pub rounding_pot_price_units: u128,
}

/// Recompute every presence-explicit owner expectation from exact selected rows.
#[allow(clippy::too_many_arguments)]
pub fn build_owner_settlement_book_v2(
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    price_scale: Amount,
    orders: &[VerifiedSettlementOrderV2; MAX_ORDERS],
    order_len: u8,
    fees: &[SelectedOwnerFeeV1; MAX_ORDERS],
    fee_len: u8,
    expected: CandidateSettlementTotalsV2,
) -> Result<OwnerSettlementBookV2> {
    if market == [0; 32]
        || epoch == [0; 32]
        || candidate == [0; 32]
        || owner_order_set_digest == [0; 32]
        || price_scale == 0
        || order_len == 0
        || usize::from(order_len) > MAX_ORDERS
        || usize::from(fee_len) > MAX_ORDERS
    {
        return Err(Error::InvalidExpectation);
    }
    let mut rows = [OwnerSettlementExpectationV2::EMPTY; MAX_ORDERS];
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
            || (order.side == SettlementSideV1::Sell && order.reserved_cash_atoms != 0)
        {
            return Err(Error::InvalidOrder);
        }
        let bit = order_bit(order.order_index)?;
        if seen_order_mask & bit != 0 {
            return Err(Error::InvalidOrder);
        }
        seen_order_mask |= bit;
        let mut slot = 0usize;
        while slot < owner_count && rows[slot].owner != order.owner {
            slot += 1;
        }
        if slot == owner_count {
            if owner_count >= MAX_ORDERS {
                return Err(Error::ArithmeticOverflow);
            }
            rows[slot] = OwnerSettlementExpectationV2 {
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
                selected_fee_atoms: 0,
                reserved_cash_atoms: 0,
            };
            owner_count += 1;
        }
        rows[slot].expected_slice_count = rows[slot]
            .expected_slice_count
            .checked_add(order.slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        match order.side {
            SettlementSideV1::Buy => {
                rows[slot].expected_buy_order_mask |= bit;
                rows[slot].expected_buy_price_units.present = true;
                rows[slot].expected_buy_price_units.value = rows[slot]
                    .expected_buy_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
                rows[slot].reserved_cash_atoms = rows[slot]
                    .reserved_cash_atoms
                    .checked_add(order.reserved_cash_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            SettlementSideV1::Sell => {
                rows[slot].expected_sell_order_mask |= bit;
                rows[slot].expected_sell_price_units.present = true;
                rows[slot].expected_sell_price_units.value = rows[slot]
                    .expected_sell_price_units
                    .value
                    .checked_add(order.consideration_price_units.value)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        order_at += 1;
    }
    if usize::from(fee_len) != owner_count {
        return Err(Error::InvalidExpectation);
    }
    let mut fee_seen_mask = 0u64;
    let mut fee_at = 0usize;
    while fee_at < usize::from(fee_len) {
        let fee = fees[fee_at];
        if fee.owner == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut slot = 0usize;
        while slot < owner_count && rows[slot].owner != fee.owner {
            slot += 1;
        }
        if slot == owner_count {
            return Err(Error::InvalidIdentity);
        }
        let bit = 1u64
            .checked_shl(u32::try_from(slot).map_err(|_| Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        if fee_seen_mask & bit != 0 {
            return Err(Error::InvalidIdentity);
        }
        fee_seen_mask |= bit;
        rows[slot].selected_fee_atoms = fee.fee_atoms;
        fee_at += 1;
    }
    let mut at = 1usize;
    while at < owner_count {
        let value = rows[at];
        let mut insert = at;
        while insert > 0 && value.owner < rows[insert - 1].owner {
            rows[insert] = rows[insert - 1];
            insert -= 1;
        }
        rows[insert] = value;
        at += 1;
    }

    let mut buy_price_units = PresentConsiderationV2::ABSENT;
    let mut sell_price_units = PresentConsiderationV2::ABSENT;
    let mut selected_fee_atoms = 0u128;
    let mut rounding_pot_price_units = 0u128;
    let mut owner_slice_end_count = 0u16;
    let mut debit_atoms = 0u128;
    let mut credit_atoms = 0u128;
    let mut owner_at = 0usize;
    while owner_at < owner_count {
        let row = rows[owner_at];
        row.validate()?;
        if row.expected_buy_price_units.present {
            buy_price_units.present = true;
            buy_price_units.value = buy_price_units
                .value
                .checked_add(row.expected_buy_price_units.value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if row.expected_sell_price_units.present {
            sell_price_units.present = true;
            sell_price_units.value = sell_price_units
                .value
                .checked_add(row.expected_sell_price_units.value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        selected_fee_atoms = selected_fee_atoms
            .checked_add(u128::from(row.selected_fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        owner_slice_end_count = owner_slice_end_count
            .checked_add(row.expected_slice_count)
            .ok_or(Error::ArithmeticOverflow)?;
        rounding_pot_price_units = rounding_pot_price_units
            .checked_add(owner_rounding_residue_price_units(
                row.expected_buy_price_units.value,
                row.expected_sell_price_units.value,
                price_scale,
            )?)
            .ok_or(Error::ArithmeticOverflow)?;
        debit_atoms = debit_atoms
            .checked_add(u128::from(owner_debit_atoms(
                row.expected_buy_price_units.value,
                price_scale,
                row.selected_fee_atoms,
            )?))
            .ok_or(Error::ArithmeticOverflow)?;
        credit_atoms = credit_atoms
            .checked_add(u128::from(owner_credit_atoms(
                row.expected_sell_price_units.value,
                price_scale,
            )?))
            .ok_or(Error::ArithmeticOverflow)?;
        owner_at += 1;
    }
    let actual = CandidateSettlementTotalsV2 {
        owner_count: u16::try_from(owner_count).map_err(|_| Error::ArithmeticOverflow)?,
        buy_price_units,
        sell_price_units,
        selected_fee_atoms,
        rounding_pot_price_units,
        owner_slice_end_count,
    };
    if actual != expected {
        return Err(Error::InvariantViolation);
    }
    Ok(OwnerSettlementBookV2 {
        rows,
        owner_count: actual.owner_count,
        debit_atoms,
        credit_atoms,
        rounding_pot_price_units,
    })
}

/// Direct, virtual-split, or virtual-merge route owning one real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementReceiptRouteV2 {
    /// Two real order ends.
    Direct = 0,
    /// A virtual split supplies one real buy end.
    SplitToBuy = 1,
    /// One real sell end supplies a virtual merge.
    SellToMerge = 2,
}

impl SettlementReceiptRouteV2 {
    /// Exact real-end bitmap owned by this route.
    pub const fn expected_end_mask(self) -> u8 {
        match self {
            Self::Direct => BUY_END_MASK | SELL_END_MASK,
            Self::SplitToBuy => BUY_END_MASK,
            Self::SellToMerge => SELL_END_MASK,
        }
    }
}

/// Complete immutable receipt projection for direct, split, and merge routes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedSettlementReceiptV2 {
    /// Canonical receipt account.
    pub receipt: [u8; 32],
    /// Contract-derived ID of the exact canonical receipt transcript.
    pub receipt_data_id: [u8; 32],
    /// Earlier price-accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Complete Egg-delivery transition identity.
    pub delivery_transition_id: [u8; 32],
    /// General MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the complete ordered owner/order set.
    pub owner_order_set_digest: [u8; 32],
    /// Exact buyer order, or canonical zero for a virtual merge.
    pub buy_order_id: [u8; 32],
    /// Exact seller order, or canonical zero for a virtual split.
    pub sell_order_id: [u8; 32],
    /// Direct, split-to-buy, or sell-to-merge classification.
    pub route: SettlementReceiptRouteV2,
    /// Exact native Egg outcome.
    pub outcome: u8,
    /// Exact native Egg quantity moved by this slice.
    pub quantity: Amount,
    /// Explicit frozen scaled outcome price, including zero.
    pub price: PresentPriceV2,
    /// Explicit exact `quantity * price`, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// Canonical zero-based selected-slice index.
    pub slice_index: u16,
    /// Exactly `slice_index + 1`.
    pub sequence: u64,
    /// Quantity already delivered on this one-shot receipt.
    pub settled_quantity: Amount,
    /// Independent price-accounting latches.
    pub accounted_end_mask: u8,
    /// Independent Egg-delivery latches.
    pub delivered_end_mask: u8,
}

impl AuthenticatedSettlementReceiptV2 {
    /// Canonical receipt account identity.
    pub const fn receipt(&self) -> [u8; 32] {
        self.receipt
    }

    /// Contract-derived exact prestate transcript identity.
    pub const fn receipt_data_id(&self) -> [u8; 32] {
        self.receipt_data_id
    }

    /// Accounting-only Replay transition identity.
    pub const fn receipt_accounting_id(&self) -> [u8; 32] {
        self.receipt_accounting_id
    }

    /// Later atomic Egg-delivery Replay transition identity.
    pub const fn delivery_transition_id(&self) -> [u8; 32] {
        self.delivery_transition_id
    }

    fn validate_shape(self, outcome_count: u8) -> Result<()> {
        self.price.validate()?;
        self.consideration_price_units.validate()?;
        for key in [
            self.receipt,
            self.receipt_accounting_id,
            self.delivery_transition_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        let expected_end_mask = self.route.expected_end_mask();
        let delivery_state_valid = (self.settled_quantity == 0 && self.delivered_end_mask == 0)
            || (self.settled_quantity == self.quantity
                && self.delivered_end_mask == expected_end_mask
                && self.accounted_end_mask == expected_end_mask);
        let order_shape_valid = match self.route {
            SettlementReceiptRouteV2::Direct => {
                self.buy_order_id != [0; 32]
                    && self.sell_order_id != [0; 32]
                    && self.buy_order_id != self.sell_order_id
            }
            SettlementReceiptRouteV2::SplitToBuy => {
                self.buy_order_id != [0; 32] && self.sell_order_id == [0; 32]
            }
            SettlementReceiptRouteV2::SellToMerge => {
                self.buy_order_id == [0; 32] && self.sell_order_id != [0; 32]
            }
        };
        if self.receipt_accounting_id == self.delivery_transition_id
            || !order_shape_valid
            || outcome_count < 2
            || usize::from(outcome_count) > crate::MAX_OUTCOMES
            || self.outcome >= outcome_count
            || self.quantity == 0
            || !self.price.present
            || !self.consideration_price_units.present
            || self.consideration_price_units.value
                != u128::from(self.quantity) * u128::from(self.price.value)
            || self.sequence != u64::from(self.slice_index) + 1
            || self.accounted_end_mask & !expected_end_mask != 0
            || self.delivered_end_mask & !expected_end_mask != 0
            || !delivery_state_valid
        {
            return Err(Error::InvalidOrder);
        }
        Ok(())
    }

    /// Validate exact identities, route-owned legs, presence, and latches.
    pub fn validate(self, outcome_count: u8) -> Result<()> {
        self.validate_shape(outcome_count)?;
        if self.receipt_data_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }

    /// Encode the sole canonical transcript from which `receipt_data_id` is
    /// derived. The ID itself is deliberately excluded to avoid recursion.
    pub fn encode_data_id_transcript(
        self,
        outcome_count: u8,
    ) -> Result<[u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES]> {
        self.validate_shape(outcome_count)?;
        let mut output = [0u8; SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES];
        let mut cursor = 0usize;
        for id in [
            self.receipt,
            self.receipt_accounting_id,
            self.delivery_transition_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.buy_order_id,
            self.sell_order_id,
        ] {
            put(&mut output, &mut cursor, &id)?;
        }
        let route = match self.route {
            SettlementReceiptRouteV2::Direct => 0,
            SettlementReceiptRouteV2::SplitToBuy => 1,
            SettlementReceiptRouteV2::SellToMerge => 2,
        };
        put(&mut output, &mut cursor, &[route, self.outcome])?;
        put(&mut output, &mut cursor, &self.quantity.to_le_bytes())?;
        put(
            &mut output,
            &mut cursor,
            &[if self.price.present { 1 } else { 0 }],
        )?;
        put(&mut output, &mut cursor, &self.price.value.to_le_bytes())?;
        put(
            &mut output,
            &mut cursor,
            &[if self.consideration_price_units.present {
                1
            } else {
                0
            }],
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.consideration_price_units.value.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.slice_index.to_le_bytes())?;
        put(&mut output, &mut cursor, &self.sequence.to_le_bytes())?;
        put(
            &mut output,
            &mut cursor,
            &self.settled_quantity.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &[self.accounted_end_mask, self.delivered_end_mask],
        )?;
        if cursor != SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }
}

/// Hash boundary for the canonical V2 receipt-prestate transcript.
pub trait SettlementReceiptDataHashV2 {
    /// Compute SHA-256 over the exact domain followed by the exact transcript.
    fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32];
}

/// Derive the sole receipt-prestate data ID without duplicating its transcript.
pub fn derive_settlement_receipt_data_id_v2<H: SettlementReceiptDataHashV2>(
    receipt: AuthenticatedSettlementReceiptV2,
    outcome_count: u8,
    hash: &H,
) -> Result<[u8; 32]> {
    let transcript = receipt.encode_data_id_transcript(outcome_count)?;
    let id = hash.sha256(SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V2, &transcript);
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(id)
}

/// One authenticated V2 receipt end with explicit consideration presence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedSettlementReceiptEndV2 {
    /// Canonical receipt PDA.
    pub receipt: [u8; 32],
    /// Contract-recomputed ID of the exact receipt prestate transcript.
    pub receipt_data_id: [u8; 32],
    /// Accounting-only receipt/reservation/owner-row transition identity.
    pub receipt_accounting_id: [u8; 32],
    /// General V2 MarketRuntime identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final candidate identity.
    pub candidate: [u8; 32],
    /// Exact owner/order/reservation/Position digest.
    pub owner_order_set_digest: [u8; 32],
    /// Semantic owner of this real end.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Payer or payee side.
    pub side: SettlementSideV1,
    /// Direct, split-to-buy, or sell-to-merge classification.
    pub route: SettlementReceiptRouteV2,
    /// Explicitly present consideration, including zero.
    pub consideration_price_units: PresentConsiderationV2,
    /// Whether this end completes its order.
    pub completes_order: bool,
    /// Canonical zero-based slice index.
    pub slice_index: u16,
    /// Exactly `slice_index + 1`.
    pub sequence: u64,
    /// Already accounted real-end mask.
    pub accounted_end_mask: u8,
    /// Exact real ends present on this receipt.
    pub expected_end_mask: u8,
}

impl AuthenticatedSettlementReceiptEndV2 {
    fn side_mask(self) -> u8 {
        match self.side {
            SettlementSideV1::Buy => BUY_END_MASK,
            SettlementSideV1::Sell => SELL_END_MASK,
        }
    }

    /// Validate identity, explicit presence, route, sequence, and once-only latch.
    pub fn validate(self) -> Result<()> {
        self.consideration_price_units.validate()?;
        for id in [
            self.receipt,
            self.receipt_data_id,
            self.receipt_accounting_id,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.owner,
        ] {
            if id == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if !self.consideration_price_units.present
            || self.expected_end_mask == 0
            || self.expected_end_mask & !(BUY_END_MASK | SELL_END_MASK) != 0
            || self.accounted_end_mask & !self.expected_end_mask != 0
            || self.sequence != u64::from(self.slice_index) + 1
        {
            return Err(Error::InvalidOrder);
        }
        match (self.route, self.side, self.expected_end_mask) {
            (SettlementReceiptRouteV2::Direct, _, 3)
            | (SettlementReceiptRouteV2::SplitToBuy, SettlementSideV1::Buy, 1)
            | (SettlementReceiptRouteV2::SellToMerge, SettlementSideV1::Sell, 2) => {}
            _ => return Err(Error::InvalidOrder),
        }
        let side = self.side_mask();
        if self.expected_end_mask & side == 0 || self.accounted_end_mask & side != 0 {
            return Err(Error::DuplicateCompletion);
        }
        Ok(())
    }
}

/// Structurally checked V2 owner-row projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementAccountProjectionV2 {
    /// Canonical row PDA.
    pub address: [u8; 32],
    /// Dragon's Clutch program identity.
    pub program_id: [u8; 32],
    /// Current lamports.
    pub lamports: u64,
    /// Current exact rent minimum.
    pub rent_minimum: u64,
    /// Decoded canonical V2 accumulator.
    pub accumulator: OwnerSettlementAccumulatorV2,
}

/// Project an existing V2 row without claiming adapter authentication.
pub fn project_owner_settlement_account_v2(
    view: OwnerSettlementAccountViewV2<'_>,
    derived: OwnerSettlementPdaProjectionV2,
) -> Result<OwnerSettlementAccountProjectionV2> {
    validate_derived(derived)?;
    if !view.writable
        || view.address != derived.address
        || view.program_owner != derived.program_id
        || view.stored_bump != derived.bump
        || view.lamports < view.rent_minimum
        || view.body.len() != OWNER_SETTLEMENT_BODY_V2_BYTES
    {
        return Err(Error::InvalidAccount);
    }
    let accumulator = OwnerSettlementAccumulatorV2::decode_body(view.body)?;
    if accumulator.expectation.epoch != derived.epoch
        || accumulator.expectation.candidate != derived.candidate
        || accumulator.expectation.owner != derived.owner
    {
        return Err(Error::InvalidAccount);
    }
    Ok(OwnerSettlementAccountProjectionV2 {
        address: view.address,
        program_id: derived.program_id,
        lamports: view.lamports,
        rent_minimum: view.rent_minimum,
        accumulator,
    })
}

/// Selected-candidate authority for one V2 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SelectedOwnerRowAuthorityV2 {
    /// SelectedCandidate account PDA.
    pub selected_candidate_account: [u8; 32],
    /// Complete V2 expectation derived by the owner builder.
    pub expectation: OwnerSettlementExpectationV2,
    /// Zero-based sorted owner ordinal.
    pub row_ordinal: u16,
    /// Exact active owner count.
    pub owner_count: u16,
    /// Present rent payer.
    pub rent_payer: [u8; 32],
    /// Sole eventual refund recipient.
    pub rent_refund_recipient: [u8; 32],
    /// Persisted rent ledger.
    pub rent_ledger: [u8; 32],
    /// Canonical donation sink.
    pub donation_sink: [u8; 32],
}

impl SelectedOwnerRowAuthorityV2 {
    fn validate(self) -> Result<()> {
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

/// Atomic V2 owner-row creation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementCreatePlanV2 {
    /// Account to allocate.
    pub address: [u8; 32],
    /// Program owner after assignment.
    pub program_id: [u8; 32],
    /// Canonical stored bump.
    pub bump: u8,
    /// Present payer debit.
    pub payer_debit_lamports: u64,
    /// Final target balance.
    pub target_lamports_after: u64,
    /// Sole refund recipient.
    pub refund_recipient: [u8; 32],
    /// Persisted rent ledger.
    pub rent_ledger: [u8; 32],
    /// Maximum refundable payer principal.
    pub payer_rent_principal_lamports: u64,
    /// Unsolicited prefunding.
    pub prefunded_donation_lamports: u64,
    /// Canonical prefund sink.
    pub donation_sink: [u8; 32],
    /// Canonical initial V2 body.
    pub body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
}

/// Prepare rent-safe creation of a V2 owner row.
pub fn prepare_create_owner_settlement_account_v2(
    authority: SelectedOwnerRowAuthorityV2,
    derived: OwnerSettlementPdaProjectionV2,
    funding: OwnerSettlementCreateFundingV1,
) -> Result<OwnerSettlementCreatePlanV2> {
    authority.validate()?;
    validate_derived(derived)?;
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
    let accumulator = OwnerSettlementAccumulatorV2::new(authority.expectation)?;
    Ok(OwnerSettlementCreatePlanV2 {
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
        body: accumulator.encode_body()?,
    })
}

/// Non-authorizing V2 row-plus-receipt accounting projection.
///
/// A live action 25 must additionally advance the canonical Reservation
/// accounting state and terminal reserved-cash handoff in one atomic plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementReceiptAccountingProjectionV2 {
    /// Owner row to write.
    pub owner_settlement_account: [u8; 32],
    /// Canonical next V2 body.
    pub owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
    /// Receipt account to latch.
    pub receipt: [u8; 32],
    /// Exact authenticated receipt prestate data ID.
    pub receipt_data_id: [u8; 32],
    /// Required accounting-only receipt/reservation/owner-row transition.
    pub receipt_accounting_id: [u8; 32],
    /// Next independent end mask.
    pub receipt_accounted_end_mask: u8,
}

/// Project one present V2 end, including zero, without authorizing a write.
pub fn project_owner_receipt_end_v2(
    account: OwnerSettlementAccountProjectionV2,
    receipt: AuthenticatedSettlementReceiptEndV2,
) -> Result<OwnerSettlementReceiptAccountingProjectionV2> {
    receipt.validate()?;
    let expected = account.accumulator.expectation;
    if receipt.market != expected.market
        || receipt.epoch != expected.epoch
        || receipt.candidate != expected.candidate
        || receipt.owner_order_set_digest != expected.owner_order_set_digest
        || receipt.owner != expected.owner
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next = account.accumulator;
    next.consume(AuthenticatedOwnerFragmentV2 {
        order_index: receipt.order_index,
        side: receipt.side,
        consideration_price_units: receipt.consideration_price_units,
        completes_order: receipt.completes_order,
    })?;
    Ok(OwnerSettlementReceiptAccountingProjectionV2 {
        owner_settlement_account: account.address,
        owner_settlement_body: next.encode_body()?,
        receipt: receipt.receipt,
        receipt_data_id: receipt.receipt_data_id,
        receipt_accounting_id: receipt.receipt_accounting_id,
        receipt_accounted_end_mask: receipt.accounted_end_mask | receipt.side_mask(),
    })
}

/// Hash boundary for the canonical finalized V2 owner-row data ID.
pub trait OwnerFinalizedRowDataHashV2 {
    /// Compute SHA-256 over the exact domain followed by the exact row body.
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32];
}

/// Derive the action-38 transition identity from one exact terminal row body.
pub fn derive_owner_finalized_row_data_id_v2<H: OwnerFinalizedRowDataHashV2>(
    finalized_body: &[u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
    hash: &H,
) -> Result<[u8; 32]> {
    let row = OwnerSettlementAccumulatorV2::decode_body(finalized_body)?;
    if row.state != 1 {
        return Err(Error::Incomplete);
    }
    let data_id = hash.sha256(OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V2, finalized_body);
    if data_id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(data_id)
}

/// Structural V2 row, Position, and cash-pot realization.
///
/// This plan is not execution authority. The live General composer must
/// rederive it from strictly decoded accounts, join the fee runtime's typed
/// payer-allocation deletion, and bind purpose Replay V3 before any write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerCashRealizationPlanV2 {
    owner_settlement_account: [u8; 32],
    expectation: OwnerSettlementExpectationV2,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
    finalized_row_data_id: [u8; 32],
    position: PositionSettlementPoststateV3,
    settlement_cash_pot: SettlementCashPotV1,
    disposition: OwnerSettlementDispositionV1,
}

impl OwnerCashRealizationPlanV2 {
    /// V2 owner row to compare-and-write.
    pub const fn owner_settlement_account(&self) -> [u8; 32] {
        self.owner_settlement_account
    }

    /// Exact immutable expectation selected before accounting began.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV2 {
        self.expectation
    }

    /// Exact finalized 288-byte V2 row body.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V2_BYTES] {
        &self.owner_settlement_body
    }

    /// Canonical action-38 transition identity derived from the final row.
    pub const fn finalized_row_data_id(&self) -> [u8; 32] {
        self.finalized_row_data_id
    }

    /// Exact canonical Position V3 successor.
    pub const fn position(&self) -> PositionSettlementPoststateV3 {
        self.position
    }

    /// Exact candidate-wide cash-pot successor.
    pub const fn settlement_cash_pot(&self) -> SettlementCashPotV1 {
        self.settlement_cash_pot
    }

    /// Exact Floor/Ceil owner disposition used for every poststate.
    pub const fn disposition(&self) -> OwnerSettlementDispositionV1 {
        self.disposition
    }
}

/// Structurally realize one complete presence-explicit owner row.
///
/// The row's immutable expectation is the only fee amount consumed here. A
/// sell-heavy owner refuses without consuming state until prior buyer debits
/// or opening merge proceeds make its exact credit executable. Finalization
/// never persists another opaque ID in the 288-byte row.
pub fn prepare_realize_owner_cash_v2<H: OwnerFinalizedRowDataHashV2>(
    account: OwnerSettlementAccountProjectionV2,
    position: AuthenticatedPositionV3,
    pot: SettlementCashPotV1,
    hash: &H,
) -> Result<OwnerCashRealizationPlanV2> {
    pot.validate()?;
    position.validate_writable()?;
    let position_prestate = position;
    let expected = account.accumulator.expectation;
    expected.validate()?;
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
    if disposition.selected_fee_atoms != expected.selected_fee_atoms {
        return Err(Error::InvariantViolation);
    }
    let consideration_debit = disposition
        .debit_atoms
        .checked_sub(disposition.selected_fee_atoms)
        .ok_or(Error::InvariantViolation)?;
    let available_consideration_atoms = pot
        .available_consideration_atoms
        .checked_add(consideration_debit)
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
        derive_owner_finalized_row_data_id_v2(&owner_settlement_body, hash)?;
    Ok(OwnerCashRealizationPlanV2 {
        owner_settlement_account: account.address,
        expectation: expected,
        owner_settlement_body,
        finalized_row_data_id,
        position,
        settlement_cash_pot: next_pot,
        disposition,
    })
}

fn validate_derived(value: OwnerSettlementPdaProjectionV2) -> Result<()> {
    value.validate()
}

fn order_bit(order_index: u8) -> Result<u64> {
    if usize::from(order_index) >= MAX_ORDERS {
        return Err(Error::InvalidOrder);
    }
    1u64.checked_shl(u32::from(order_index))
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

const _: () = assert!(OWNER_SETTLEMENT_BODY_V2_BYTES == 288);
const _: () = assert!(SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES == 344);

#[cfg(test)]
mod expectation_basis_tests {
    use super::*;

    #[test]
    fn zero_price_buy_basis_binds_fee_only_after_selection() {
        let mut orders = [VerifiedSettlementOrderV2 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            reserved_cash_atoms: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV2 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 2,
            reserved_cash_atoms: 5,
        };
        let book = build_owner_settlement_expectation_basis_book_v2(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap();
        let basis = book.row(0).unwrap();
        assert_eq!(basis.owner(), [6; 32]);
        assert_eq!(basis.expected_buy_price_units(), PresentConsiderationV2::new(0));
        assert_eq!(basis.reserved_cash_atoms(), 5);
        assert_eq!(
            basis
                .with_selected_fee(SelectedOwnerFeeV1 {
                    owner: [6; 32],
                    fee_atoms: 5,
                })
                .unwrap()
                .selected_fee_atoms,
            5
        );
        assert_eq!(
            basis.with_selected_fee(SelectedOwnerFeeV1 {
                owner: [6; 32],
                fee_atoms: 6,
            }),
            Err(Error::InsufficientCash)
        );
    }

    #[test]
    fn selected_fee_cannot_be_rebound_to_another_owner() {
        let mut orders = [VerifiedSettlementOrderV2 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            reserved_cash_atoms: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV2 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 1,
            reserved_cash_atoms: 0,
        };
        let basis = build_owner_settlement_expectation_basis_book_v2(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap();
        assert_eq!(
            basis.with_selected_fee(SelectedOwnerFeeV1 {
                owner: [7; 32],
                fee_atoms: 0,
            }),
            Err(Error::InvalidIdentity)
        );
    }
}
