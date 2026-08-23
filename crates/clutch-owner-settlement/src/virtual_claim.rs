//! Explicit virtual complete-set inventory and one-real-end receipt contracts.
//!
//! A virtual end is never represented as a synthetic owner, Position,
//! Reservation, or direct-pair end. Split inventory converts explicitly owned
//! FinalPot cash principal into one native claim of every outcome. Merge
//! inventory performs the exact inverse. One-real-end receipt planners then
//! move a selected outcome between the FinalPot and an authenticated real
//! Position while advancing only the real Reservation and owner row.

use crate::{
    direct::{advance_reservation_accounting, advance_reservation_delivery},
    prepare_account_receipt_end_v1, Amount,
    AuthenticatedOrderMembershipV1, AuthenticatedOwnerSettlementAccountV1,
    AuthenticatedPositionV1, AuthenticatedReservationV1, AuthenticatedSettlementReceiptEndV1,
    Error, OrderKindV1, OwnerSettlementAccumulatorV1,
    OwnerSettlementReceiptAccountingPlanV1, ReservationStateV1, Result,
    SettlementCashPotExpectationV1, SettlementCashPotV1, SettlementSideV1,
    VirtualCashDirectionV1, MAX_OUTCOMES,
};

const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Direction of an explicitly verifier-authorized virtual receipt or budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VirtualReceiptKindV1 {
    /// Cash principal becomes one complete native claim set.
    Split = 0,
    /// One complete native claim set becomes cash principal.
    Merge = 1,
}

/// Progress state of an exact selected-candidate inventory budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VirtualInventoryStateV1 {
    /// More selected complete-set atoms remain to process.
    Open = 0,
    /// The exact selected amount has been processed.
    Complete = 1,
}

/// Candidate-owned, default-deny budget for virtual complete-set inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVirtualInventoryBudgetV1 {
    /// Canonical mutable inventory-budget account.
    pub account: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Exact checked-relation witness digest authorizing this amount.
    pub relation_witness_digest: [u8; 32],
    /// Split or merge direction. The two planners never branch across it.
    pub kind: VirtualReceiptKindV1,
    /// Exact complete-set atoms admitted by the selected candidate.
    pub authorized_complete_set_atoms: Amount,
    /// Complete-set atoms already processed.
    pub processed_complete_set_atoms: Amount,
    /// Monotone inventory transition sequence.
    pub transition_sequence: u64,
    /// Open or complete.
    pub state: VirtualInventoryStateV1,
    /// True only after central selected-candidate authentication.
    pub verifier_authorized: bool,
    /// Whether the budget account is writable.
    pub writable: bool,
}

impl AuthenticatedVirtualInventoryBudgetV1 {
    fn validate(self) -> Result<()> {
        for key in [
            self.account,
            self.market,
            self.epoch,
            self.candidate,
            self.relation_witness_digest,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.authorized_complete_set_atoms == 0
            || self.processed_complete_set_atoms > self.authorized_complete_set_atoms
            || !self.verifier_authorized
            || !self.writable
        {
            return Err(Error::AuthorityUnavailable);
        }
        let complete = self.processed_complete_set_atoms == self.authorized_complete_set_atoms;
        if complete != (self.state == VirtualInventoryStateV1::Complete) {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

/// FinalPot projection owning virtual cash principal and internal native claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedFinalPotV1 {
    /// Canonical FinalPot account.
    pub account: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the complete owner/order set whose virtual ends this pot owns.
    pub owner_order_set_digest: [u8; 32],
    /// Cash principal attributed to virtual inventory.
    ///
    /// This is neither a fee nor a donation. Physical custody remains in the
    /// Realm-selected collateral adapter.
    pub cash_principal_atoms: Amount,
    /// Native internal claims owned by the pot.
    pub internal_claims: [Amount; MAX_OUTCOMES],
    /// Active outcome width.
    pub outcome_count: u8,
    /// Zero while inventory and receipts remain open.
    pub phase: u8,
    /// Whether the account meta is writable.
    pub writable: bool,
}

impl AuthenticatedFinalPotV1 {
    fn validate(self) -> Result<()> {
        for key in [
            self.account,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.phase != 0
            || !self.writable
            || !zero_padded(&self.internal_claims, self.outcome_count)
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }
}

/// Authenticated base-ledger projection changed by an actual complete-set op.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedMarketClaimLedgerV1 {
    /// Canonical aggregate supply ledger.
    pub ledger: [u8; 32],
    /// Canonical Hoard account whose balance is Realm collateral principal.
    pub hoard: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Hoard collateral principal before the transition.
    pub hoard_collateral_atoms: Amount,
    /// Aggregate internal native-claim supply.
    pub internal_supply: [Amount; MAX_OUTCOMES],
    /// Aggregate native-claim supply across internal and external forms.
    pub total_supply: [Amount; MAX_OUTCOMES],
    /// Active outcome width.
    pub outcome_count: u8,
    /// Zero only while the market admits split and merge.
    pub market_phase: u8,
    /// Whether both ledger and Hoard were presented writable.
    pub writable: bool,
}

impl AuthenticatedMarketClaimLedgerV1 {
    fn validate(self) -> Result<()> {
        if self.ledger == [0; 32]
            || self.hoard == [0; 32]
            || self.market == [0; 32]
            || self.ledger == self.hoard
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.market_phase != 0
            || !self.writable
            || !zero_padded(&self.internal_supply, self.outcome_count)
            || !zero_padded(&self.total_supply, self.outcome_count)
        {
            return Err(Error::InvalidAccount);
        }
        let mut at = 0_usize;
        while at < usize::from(self.outcome_count) {
            if self.internal_supply[at] > self.total_supply[at] {
                return Err(Error::InvariantViolation);
            }
            at += 1;
        }
        Ok(())
    }
}

/// Atomic poststate of one actual complete-set split or merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualInventoryPlanV1 {
    /// Opaque complete transition identity bound by the SBF adapter.
    pub settlement_transition_id: [u8; 32],
    /// Direction whose typed planner produced this value.
    pub kind: VirtualReceiptKindV1,
    /// Exact complete-set quantity processed.
    pub quantity: Amount,
    /// Candidate budget poststate.
    pub budget: AuthenticatedVirtualInventoryBudgetV1,
    /// FinalPot cash/internal-claim poststate.
    pub final_pot: AuthenticatedFinalPotV1,
    /// Hoard and aggregate-supply poststate.
    pub market_ledger: AuthenticatedMarketClaimLedgerV1,
}

/// Convert explicit FinalPot cash principal into complete-set inventory.
pub(crate) fn prepare_virtual_split_inventory_v1(
    settlement_transition_id: [u8; 32],
    budget: AuthenticatedVirtualInventoryBudgetV1,
    final_pot: AuthenticatedFinalPotV1,
    market_ledger: AuthenticatedMarketClaimLedgerV1,
    quantity: Amount,
) -> Result<VirtualInventoryPlanV1> {
    prepare_inventory(
        settlement_transition_id,
        budget,
        final_pot,
        market_ledger,
        quantity,
        VirtualReceiptKindV1::Split,
    )
}

/// Convert complete-set FinalPot inventory back into cash principal.
pub(crate) fn prepare_virtual_merge_inventory_v1(
    settlement_transition_id: [u8; 32],
    budget: AuthenticatedVirtualInventoryBudgetV1,
    final_pot: AuthenticatedFinalPotV1,
    market_ledger: AuthenticatedMarketClaimLedgerV1,
    quantity: Amount,
) -> Result<VirtualInventoryPlanV1> {
    prepare_inventory(
        settlement_transition_id,
        budget,
        final_pot,
        market_ledger,
        quantity,
        VirtualReceiptKindV1::Merge,
    )
}

fn prepare_inventory(
    settlement_transition_id: [u8; 32],
    budget: AuthenticatedVirtualInventoryBudgetV1,
    final_pot: AuthenticatedFinalPotV1,
    market_ledger: AuthenticatedMarketClaimLedgerV1,
    quantity: Amount,
    expected_kind: VirtualReceiptKindV1,
) -> Result<VirtualInventoryPlanV1> {
    budget.validate()?;
    final_pot.validate()?;
    market_ledger.validate()?;
    if settlement_transition_id == [0; 32]
        || quantity == 0
        || budget.kind != expected_kind
        || budget.state != VirtualInventoryStateV1::Open
        || budget.market != final_pot.market
        || budget.epoch != final_pot.epoch
        || budget.candidate != final_pot.candidate
        || market_ledger.market != final_pot.market
        || market_ledger.outcome_count != final_pot.outcome_count
        || budget.account == final_pot.account
        || budget.account == market_ledger.ledger
        || budget.account == market_ledger.hoard
        || final_pot.account == market_ledger.ledger
        || final_pot.account == market_ledger.hoard
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut at = 0_usize;
    while at < usize::from(final_pot.outcome_count) {
        if final_pot.internal_claims[at] > market_ledger.internal_supply[at] {
            return Err(Error::InvariantViolation);
        }
        at += 1;
    }
    let mut next_budget = budget;
    next_budget.processed_complete_set_atoms = next_budget
        .processed_complete_set_atoms
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_budget.processed_complete_set_atoms > next_budget.authorized_complete_set_atoms {
        return Err(Error::TooManyFragments);
    }
    next_budget.transition_sequence = next_budget
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_budget.processed_complete_set_atoms == next_budget.authorized_complete_set_atoms {
        next_budget.state = VirtualInventoryStateV1::Complete;
    }
    let mut next_pot = final_pot;
    let mut next_ledger = market_ledger;
    match expected_kind {
        VirtualReceiptKindV1::Split => {
            next_pot.cash_principal_atoms = next_pot
                .cash_principal_atoms
                .checked_sub(quantity)
                .ok_or(Error::InsufficientCash)?;
            next_ledger.hoard_collateral_atoms = next_ledger
                .hoard_collateral_atoms
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            let mut at = 0_usize;
            while at < usize::from(final_pot.outcome_count) {
                next_pot.internal_claims[at] = next_pot.internal_claims[at]
                    .checked_add(quantity)
                    .ok_or(Error::ArithmeticOverflow)?;
                next_ledger.internal_supply[at] = next_ledger.internal_supply[at]
                    .checked_add(quantity)
                    .ok_or(Error::ArithmeticOverflow)?;
                next_ledger.total_supply[at] = next_ledger.total_supply[at]
                    .checked_add(quantity)
                    .ok_or(Error::ArithmeticOverflow)?;
                at += 1;
            }
        }
        VirtualReceiptKindV1::Merge => {
            next_ledger.hoard_collateral_atoms = next_ledger
                .hoard_collateral_atoms
                .checked_sub(quantity)
                .ok_or(Error::InsufficientCash)?;
            next_pot.cash_principal_atoms = next_pot
                .cash_principal_atoms
                .checked_add(quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            let mut at = 0_usize;
            while at < usize::from(final_pot.outcome_count) {
                next_pot.internal_claims[at] = next_pot.internal_claims[at]
                    .checked_sub(quantity)
                    .ok_or(Error::InsufficientCash)?;
                next_ledger.internal_supply[at] = next_ledger.internal_supply[at]
                    .checked_sub(quantity)
                    .ok_or(Error::InvariantViolation)?;
                next_ledger.total_supply[at] = next_ledger.total_supply[at]
                    .checked_sub(quantity)
                    .ok_or(Error::InvariantViolation)?;
                at += 1;
            }
        }
    }
    next_budget.validate()?;
    next_pot.validate()?;
    next_ledger.validate()?;
    Ok(VirtualInventoryPlanV1 {
        settlement_transition_id,
        kind: expected_kind,
        quantity,
        budget: next_budget,
        final_pot: next_pot,
        market_ledger: next_ledger,
    })
}

/// Default-deny selected-receipt authority for exactly one virtual real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVirtualReceiptAuthorityV1 {
    /// Selected-candidate virtual-authority record.
    pub account: [u8; 32],
    /// Checked relation witness digest.
    pub relation_witness_digest: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Complete owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Receipt account this authority admits.
    pub receipt: [u8; 32],
    /// Earlier action-25 price-accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Complete action-36/37 delivery identity.
    pub delivery_transition_id: [u8; 32],
    /// Sole real order on this virtual receipt.
    pub real_order_id: [u8; 32],
    /// Split or merge direction.
    pub kind: VirtualReceiptKindV1,
    /// Exact outcome.
    pub outcome: u8,
    /// Exact quantity.
    pub quantity: Amount,
    /// Exact consideration in scaled price units.
    pub consideration_price_units: u128,
    /// Exact selected-slice index.
    pub slice_index: u16,
    /// True only after central verifier authentication.
    pub verifier_authorized: bool,
}

impl AuthenticatedVirtualReceiptAuthorityV1 {
    fn validate(self) -> Result<()> {
        for key in [
            self.account,
            self.relation_witness_digest,
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.receipt,
            self.receipt_accounting_id,
            self.delivery_transition_id,
            self.real_order_id,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.receipt_accounting_id == self.delivery_transition_id
            || self.quantity == 0
            || self.consideration_price_units == 0
            || !self.verifier_authorized
        {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(())
    }
}

/// Authenticated one-real-buy-end receipt for a virtual split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVirtualSplitReceiptV1 {
    /// Canonical receipt account.
    pub receipt: [u8; 32],
    /// Earlier price-accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Complete split-and-delivery transition identity.
    pub delivery_transition_id: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Complete owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Sole real buyer order.
    pub buy_order_id: [u8; 32],
    /// Exact outcome.
    pub outcome: u8,
    /// Exact Egg quantity delivered from FinalPot inventory.
    pub quantity: Amount,
    /// Frozen scaled outcome price.
    pub price: Amount,
    /// Must equal `quantity * price`.
    pub consideration_price_units: u128,
    /// Canonical selected-slice index.
    pub slice_index: u16,
    /// Must equal `slice_index + 1`.
    pub sequence: u64,
    /// Quantity already settled; must be zero.
    pub settled_quantity: Amount,
    /// Independent real-end accounting latch.
    pub accounted_end_mask: u8,
    /// Independent real-end delivery latch.
    pub delivered_end_mask: u8,
    /// Must name only the real buy end.
    pub expected_end_mask: u8,
}

/// Authenticated one-real-sell-end receipt for a virtual merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVirtualMergeReceiptV1 {
    /// Canonical receipt account.
    pub receipt: [u8; 32],
    /// Earlier price-accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Complete merge-and-delivery transition identity.
    pub delivery_transition_id: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Complete owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Sole real seller order.
    pub sell_order_id: [u8; 32],
    /// Exact outcome.
    pub outcome: u8,
    /// Exact Egg quantity delivered into FinalPot inventory.
    pub quantity: Amount,
    /// Frozen scaled outcome price.
    pub price: Amount,
    /// Must equal `quantity * price`.
    pub consideration_price_units: u128,
    /// Canonical selected-slice index.
    pub slice_index: u16,
    /// Must equal `slice_index + 1`.
    pub sequence: u64,
    /// Quantity already settled; must be zero.
    pub settled_quantity: Amount,
    /// Independent real-end accounting latch.
    pub accounted_end_mask: u8,
    /// Independent real-end delivery latch.
    pub delivered_end_mask: u8,
    /// Must name only the real sell end.
    pub expected_end_mask: u8,
}

/// Price-accounting prestate for one virtual-split real buyer end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitReceiptAccountingInputV1 {
    /// Default-deny selected-receipt authority.
    pub authority: AuthenticatedVirtualReceiptAuthorityV1,
    /// One-real-buy receipt before its accounting latch is consumed.
    pub receipt: AuthenticatedVirtualSplitReceiptV1,
    /// Frozen real-order membership.
    pub order: AuthenticatedOrderMembershipV1,
    /// Buyer Position projection authenticating owner and generation.
    pub position: AuthenticatedPositionV1,
    /// Buy Reservation accounting prestate.
    pub reservation: AuthenticatedReservationV1,
    /// Buyer owner-row accounting prestate.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
}

/// Price-accounting prestate for one virtual-merge real seller end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeReceiptAccountingInputV1 {
    /// Default-deny selected-receipt authority.
    pub authority: AuthenticatedVirtualReceiptAuthorityV1,
    /// One-real-sell receipt before its accounting latch is consumed.
    pub receipt: AuthenticatedVirtualMergeReceiptV1,
    /// Frozen real-order membership.
    pub order: AuthenticatedOrderMembershipV1,
    /// Seller Position projection authenticating owner and generation.
    pub position: AuthenticatedPositionV1,
    /// Sell Reservation accounting prestate.
    pub reservation: AuthenticatedReservationV1,
    /// Seller owner-row accounting prestate.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
}

/// Atomic accounting-only poststate for a virtual-split receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitReceiptAccountingPlanV1 {
    /// Exact action-25 accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Later action-36 delivery identity committed by the receipt.
    pub delivery_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Real-buy accounting latch poststate.
    pub receipt_accounted_end_mask: u8,
    /// Delivery remains independently unconsumed.
    pub receipt_delivered_end_mask: u8,
    /// Buy Reservation accounting poststate.
    pub reservation: AuthenticatedReservationV1,
    /// Buyer owner-row accounting poststate.
    pub owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Whether this order is now completely accounted.
    pub accounting_complete: bool,
    /// Reservation cash handed to the aggregate owner row exactly once.
    pub reserved_cash_handoff_atoms: Amount,
}

/// Atomic accounting-only poststate for a virtual-merge receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeReceiptAccountingPlanV1 {
    /// Exact action-25 accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Later action-37 delivery identity committed by the receipt.
    pub delivery_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Real-sell accounting latch poststate.
    pub receipt_accounted_end_mask: u8,
    /// Delivery remains independently unconsumed.
    pub receipt_delivered_end_mask: u8,
    /// Sell Reservation accounting poststate.
    pub reservation: AuthenticatedReservationV1,
    /// Seller owner-row accounting poststate.
    pub owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Whether this order is now completely accounted.
    pub accounting_complete: bool,
}

/// Complete virtual-split receipt prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitReceiptInputV1 {
    /// Central verifier authority.
    pub authority: AuthenticatedVirtualReceiptAuthorityV1,
    /// One-real-buy receipt.
    pub receipt: AuthenticatedVirtualSplitReceiptV1,
    /// Frozen real-order membership.
    pub order: AuthenticatedOrderMembershipV1,
    /// Buyer Position.
    pub position: AuthenticatedPositionV1,
    /// Buy Reservation.
    pub reservation: AuthenticatedReservationV1,
    /// Buyer owner row, already finalized by action 38.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
    /// FinalPot holding the Egg inventory.
    pub final_pot: AuthenticatedFinalPotV1,
}

/// Complete virtual-merge receipt prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeReceiptInputV1 {
    /// Central verifier authority.
    pub authority: AuthenticatedVirtualReceiptAuthorityV1,
    /// One-real-sell receipt.
    pub receipt: AuthenticatedVirtualMergeReceiptV1,
    /// Frozen real-order membership.
    pub order: AuthenticatedOrderMembershipV1,
    /// Seller Position.
    pub position: AuthenticatedPositionV1,
    /// Sell Reservation.
    pub reservation: AuthenticatedReservationV1,
    /// Accounting-complete seller row; merge proceeds must exist before action 38.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
    /// FinalPot receiving the Egg inventory.
    pub final_pot: AuthenticatedFinalPotV1,
}

/// Atomic poststate of one virtual-split real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitReceiptPlanV1 {
    /// Exact action-36 delivery identity.
    pub delivery_transition_id: [u8; 32],
    /// Earlier accounting identity whose latch authorizes delivery.
    pub receipt_accounting_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Settled receipt quantity.
    pub receipt_settled_quantity: Amount,
    /// Buy-end delivery latch poststate.
    pub receipt_delivered_end_mask: u8,
    /// Buyer Position poststate.
    pub position: AuthenticatedPositionV1,
    /// Buy Reservation poststate.
    pub reservation: AuthenticatedReservationV1,
    /// FinalPot poststate after Egg delivery.
    pub final_pot: AuthenticatedFinalPotV1,
    /// Whether this slice completed buyer delivery.
    pub completes_order: bool,
}

/// Atomic poststate of one virtual-merge real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeReceiptPlanV1 {
    /// Exact action-37 delivery identity.
    pub delivery_transition_id: [u8; 32],
    /// Earlier accounting identity whose latch authorizes delivery.
    pub receipt_accounting_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Settled receipt quantity.
    pub receipt_settled_quantity: Amount,
    /// Sell-end delivery latch poststate.
    pub receipt_delivered_end_mask: u8,
    /// Seller Position poststate.
    pub position: AuthenticatedPositionV1,
    /// Sell Reservation poststate.
    pub reservation: AuthenticatedReservationV1,
    /// FinalPot poststate after Egg delivery.
    pub final_pot: AuthenticatedFinalPotV1,
    /// Whether this slice completed seller delivery.
    pub completes_order: bool,
    /// Entire unfilled seller vector returned at completion.
    pub returned_internal: [Amount; MAX_OUTCOMES],
}

/// Complete authenticated input for the only public virtual-split composite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitCompositeInputV1 {
    /// One already-accounted and funded buyer delivery prestate.
    pub receipt: VirtualSplitReceiptInputV1,
    /// Exact selected virtual-split inventory budget.
    pub inventory_budget: AuthenticatedVirtualInventoryBudgetV1,
    /// Hoard and aggregate claim-supply prestate.
    pub market_ledger: AuthenticatedMarketClaimLedgerV1,
    /// Fully owner-finalized pot holding the remaining split principal.
    pub cash_pot: SettlementCashPotV1,
}

/// One atomic virtual-split poststate for future General action 36.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitCompositePlanV1 {
    /// Identity shared by inventory, delivery, and outer liveness joins.
    pub delivery_transition_id: [u8; 32],
    /// Real buyer Position/Reservation/receipt and final FinalPot poststate.
    pub receipt: VirtualSplitReceiptPlanV1,
    /// Inventory budget poststate.
    pub inventory_budget: AuthenticatedVirtualInventoryBudgetV1,
    /// Hoard and aggregate supply poststate.
    pub market_ledger: AuthenticatedMarketClaimLedgerV1,
    /// Owner cash-pot poststate after exact principal transfer.
    pub cash_pot: SettlementCashPotV1,
    /// Complete sets minted solely to cover this receipt's inventory deficit.
    pub newly_split_complete_set_atoms: Amount,
}

/// Account one virtual-split buyer end without moving cash or Eggs.
pub fn prepare_virtual_split_receipt_accounting_v1(
    input: VirtualSplitReceiptAccountingInputV1,
) -> Result<VirtualSplitReceiptAccountingPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    validate_split_receipt(
        input.receipt,
        input.position.outcome_count,
        ReceiptPhaseV1::Accounting,
    )?;
    validate_virtual_accounting_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.receipt_accounting_id,
        input.receipt.delivery_transition_id,
        input.receipt.market,
        input.receipt.epoch,
        input.receipt.candidate,
        input.receipt.owner_order_set_digest,
        input.receipt.buy_order_id,
        input.receipt.outcome,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
        input.receipt.slice_index,
        VirtualReceiptKindV1::Split,
        input.order,
        input.position,
        input.reservation,
        input.owner_row,
        0,
    )?;
    if input.order.side != SettlementSideV1::Buy
        || input.reservation.side != SettlementSideV1::Buy
        || input.reservation.state != ReservationStateV1::Entitled
        || input.reservation.remaining_internal != [0; MAX_OUTCOMES]
        || (input.order.order_kind == OrderKindV1::Single
            && input.order.single_outcome != input.receipt.outcome)
        || input.position.reserved_cash_atoms
            < input.owner_row.accumulator.expectation.reserved_cash_atoms
        || input.reservation.initial_cash_atoms
            > input.owner_row.accumulator.expectation.reserved_cash_atoms
    {
        return Err(Error::InvalidOrder);
    }
    let (mut reservation, completes) = advance_reservation_accounting(
        input.reservation,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
    )?;
    let handoff = if completes {
        let value = reservation.remaining_cash_atoms;
        reservation.remaining_cash_atoms = 0;
        value
    } else {
        0
    };
    reservation.validate()?;
    let owner_accounting = prepare_account_receipt_end_v1(
        input.owner_row,
        virtual_receipt_end(
            input.receipt.receipt,
            input.receipt.receipt_accounting_id,
            input.receipt.market,
            input.receipt.epoch,
            input.receipt.candidate,
            input.receipt.owner_order_set_digest,
            input.order,
            input.receipt.consideration_price_units,
            completes,
            input.receipt.slice_index,
            input.receipt.sequence,
            BUY_END_MASK,
        ),
    )?;
    Ok(VirtualSplitReceiptAccountingPlanV1 {
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt: input.receipt.receipt,
        receipt_accounted_end_mask: BUY_END_MASK,
        receipt_delivered_end_mask: 0,
        reservation,
        owner_accounting,
        accounting_complete: completes,
        reserved_cash_handoff_atoms: handoff,
    })
}

/// Account one virtual-merge seller end without moving cash or Eggs.
pub fn prepare_virtual_merge_receipt_accounting_v1(
    input: VirtualMergeReceiptAccountingInputV1,
) -> Result<VirtualMergeReceiptAccountingPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    validate_merge_receipt(
        input.receipt,
        input.position.outcome_count,
        ReceiptPhaseV1::Accounting,
    )?;
    validate_virtual_accounting_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.receipt_accounting_id,
        input.receipt.delivery_transition_id,
        input.receipt.market,
        input.receipt.epoch,
        input.receipt.candidate,
        input.receipt.owner_order_set_digest,
        input.receipt.sell_order_id,
        input.receipt.outcome,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
        input.receipt.slice_index,
        VirtualReceiptKindV1::Merge,
        input.order,
        input.position,
        input.reservation,
        input.owner_row,
        0,
    )?;
    if input.order.side != SettlementSideV1::Sell
        || input.reservation.side != SettlementSideV1::Sell
        || input.reservation.state != ReservationStateV1::Entitled
        || input.reservation.remaining_cash_atoms != 0
        || input.reservation.remaining_internal[usize::from(input.receipt.outcome)]
            < input.receipt.quantity
        || (input.order.order_kind == OrderKindV1::Single
            && input.order.single_outcome != input.receipt.outcome)
    {
        return Err(Error::InvalidOrder);
    }
    let (reservation, completes) = advance_reservation_accounting(
        input.reservation,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
    )?;
    reservation.validate()?;
    let owner_accounting = prepare_account_receipt_end_v1(
        input.owner_row,
        virtual_receipt_end(
            input.receipt.receipt,
            input.receipt.receipt_accounting_id,
            input.receipt.market,
            input.receipt.epoch,
            input.receipt.candidate,
            input.receipt.owner_order_set_digest,
            input.order,
            input.receipt.consideration_price_units,
            completes,
            input.receipt.slice_index,
            input.receipt.sequence,
            SELL_END_MASK,
        ),
    )?;
    Ok(VirtualMergeReceiptAccountingPlanV1 {
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt: input.receipt.receipt,
        receipt_accounted_end_mask: SELL_END_MASK,
        receipt_delivered_end_mask: 0,
        reservation,
        owner_accounting,
        accounting_complete: completes,
    })
}

/// Stage the sole public virtual-split composition.
///
/// The fully finalized owner pot is the only source of split principal. The
/// planner transfers exactly the selected receipt's inventory deficit into
/// FinalPot, creates that many complete sets, and delivers the requested real
/// Egg in one poststate bundle. No callable intermediate can prefinance Hoard
/// or strand FinalPot cash.
pub fn prepare_virtual_split_composite_v1(
    input: VirtualSplitCompositeInputV1,
) -> Result<VirtualSplitCompositePlanV1> {
    input.inventory_budget.validate()?;
    input.market_ledger.validate()?;
    input.cash_pot.validate()?;
    let authority = input.receipt.authority;
    let mut funded_final_pot = input.receipt.final_pot;
    authority.validate()?;
    let transition = authority.delivery_transition_id;
    if authority.kind != VirtualReceiptKindV1::Split
        || input.inventory_budget.kind != VirtualReceiptKindV1::Split
        || input.inventory_budget.relation_witness_digest != authority.relation_witness_digest
        || input.inventory_budget.market != authority.market
        || input.inventory_budget.epoch != authority.epoch
        || input.inventory_budget.candidate != authority.candidate
        || input.cash_pot.expectation.virtual_cash_direction != VirtualCashDirectionV1::Split
        || input.cash_pot.expectation.virtual_cash_atoms
            != input.inventory_budget.authorized_complete_set_atoms
        || input.cash_pot.expectation.market != authority.market
        || input.cash_pot.expectation.epoch != authority.epoch
        || input.cash_pot.expectation.candidate != authority.candidate
        || input.cash_pot.expectation.owner_order_set_digest != authority.owner_order_set_digest
        || input.market_ledger.market != authority.market
        || input.market_ledger.outcome_count != funded_final_pot.outcome_count
        || funded_final_pot.cash_principal_atoms != 0
    {
        return Err(Error::AuthorityUnavailable);
    }
    let remaining = input
        .inventory_budget
        .authorized_complete_set_atoms
        .checked_sub(input.inventory_budget.processed_complete_set_atoms)
        .ok_or(Error::InvariantViolation)?;
    let expected_cash_state = if input.inventory_budget.state == VirtualInventoryStateV1::Complete {
        2
    } else {
        1
    };
    if input.cash_pot.state != expected_cash_state {
        return Err(Error::AuthorityUnavailable);
    }
    let rounding_atoms = Amount::try_from(
        input.cash_pot.expectation.rounding_pot_price_units
            / u128::from(input.cash_pot.expectation.price_scale),
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    if input.cash_pot.available_consideration_atoms
        != rounding_atoms
            .checked_add(remaining)
            .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::InvariantViolation);
    }
    let outcome = usize::from(input.receipt.receipt.outcome);
    if outcome >= usize::from(funded_final_pot.outcome_count) {
        return Err(Error::InvalidOrder);
    }
    let available = funded_final_pot.internal_claims[outcome];
    let deficit = if available < input.receipt.receipt.quantity {
        input.receipt.receipt.quantity - available
    } else {
        0
    };
    if deficit > remaining {
        return Err(Error::InsufficientCash);
    }

    let mut cash_pot = input.cash_pot;
    let (inventory_budget, final_pot, market_ledger) = if deficit == 0 {
        (
            input.inventory_budget,
            funded_final_pot,
            input.market_ledger,
        )
    } else {
        cash_pot.available_consideration_atoms = cash_pot
            .available_consideration_atoms
            .checked_sub(deficit)
            .ok_or(Error::InsufficientCash)?;
        funded_final_pot.cash_principal_atoms = funded_final_pot
            .cash_principal_atoms
            .checked_add(deficit)
            .ok_or(Error::ArithmeticOverflow)?;
        let inventory = prepare_virtual_split_inventory_v1(
            transition,
            input.inventory_budget,
            funded_final_pot,
            input.market_ledger,
            deficit,
        )?;
        (inventory.budget, inventory.final_pot, inventory.market_ledger)
    };
    if inventory_budget.state == VirtualInventoryStateV1::Complete {
        cash_pot.state = 2;
    }
    cash_pot.validate()?;
    let mut receipt_input = input.receipt;
    receipt_input.final_pot = final_pot;
    let receipt = prepare_virtual_split_receipt_v1(receipt_input)?;
    Ok(VirtualSplitCompositePlanV1 {
        delivery_transition_id: transition,
        receipt,
        inventory_budget,
        market_ledger,
        cash_pot,
        newly_split_complete_set_atoms: deficit,
    })
}

/// Owner-cash-pot result of one merge composite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualMergeCashPotPostV1 {
    /// More outcome inventory is required before any merge cash may be spent.
    AwaitingCompleteSet,
    /// The exact full merge budget funded the owner cash pot atomically.
    Funded(SettlementCashPotV1),
}

/// Complete authenticated input for the only public virtual-merge composite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeCompositeInputV1 {
    /// One real seller end and its complete settlement prestate.
    pub receipt: VirtualMergeReceiptInputV1,
    /// Exact selected virtual-merge inventory budget.
    pub inventory_budget: AuthenticatedVirtualInventoryBudgetV1,
    /// Hoard and aggregate claim-supply prestate.
    pub market_ledger: AuthenticatedMarketClaimLedgerV1,
    /// Exact owner-cash expectation funded only when the merge completes.
    pub cash_expectation: SettlementCashPotExpectationV1,
}

/// One atomic virtual-merge poststate for future General action 37.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeCompositePlanV1 {
    /// One identity shared by receipt, inventory, cash, and liveness joins.
    pub delivery_transition_id: [u8; 32],
    /// Real seller Position/Reservation/row/receipt and final FinalPot poststate.
    pub receipt: VirtualMergeReceiptPlanV1,
    /// Inventory budget poststate.
    pub inventory_budget: AuthenticatedVirtualInventoryBudgetV1,
    /// Hoard and aggregate supply poststate.
    pub market_ledger: AuthenticatedMarketClaimLedgerV1,
    /// Complete sets burned after accepting this real seller end.
    pub newly_merged_complete_set_atoms: Amount,
    /// Cash pot is absent until the exact merge budget completes.
    pub cash_pot: VirtualMergeCashPotPostV1,
}

/// Stage the sole public virtual-merge composition.
///
/// The real seller Egg enters FinalPot inventory first. The planner then burns
/// the canonical newly available complete-set floor, never a caller-chosen
/// subset. Merge cash remains attributed to FinalPot and unavailable to seller
/// realization until every selected complete set exists and the budget becomes
/// complete; that last call atomically transfers the exact full proceeds into
/// a fresh owner cash pot. No inventory-only or cash-only executable plan is
/// exported.
pub fn prepare_virtual_merge_composite_v1(
    input: VirtualMergeCompositeInputV1,
) -> Result<VirtualMergeCompositePlanV1> {
    input.inventory_budget.validate()?;
    input.market_ledger.validate()?;
    input.cash_expectation.validate()?;
    let authority = input.receipt.authority;
    let pot_before = input.receipt.final_pot;
    let transition = authority.delivery_transition_id;
    if authority.kind != VirtualReceiptKindV1::Merge
        || input.inventory_budget.kind != VirtualReceiptKindV1::Merge
        || input.inventory_budget.relation_witness_digest != authority.relation_witness_digest
        || input.inventory_budget.market != authority.market
        || input.inventory_budget.epoch != authority.epoch
        || input.inventory_budget.candidate != authority.candidate
        || input.inventory_budget.authorized_complete_set_atoms
            != input.cash_expectation.virtual_cash_atoms
        || input.cash_expectation.virtual_cash_direction != VirtualCashDirectionV1::Merge
        || input.cash_expectation.market != authority.market
        || input.cash_expectation.epoch != authority.epoch
        || input.cash_expectation.candidate != authority.candidate
        || input.cash_expectation.owner_order_set_digest != authority.owner_order_set_digest
        || input.market_ledger.market != authority.market
        || input.market_ledger.outcome_count != pot_before.outcome_count
        || pot_before.cash_principal_atoms
            != input.inventory_budget.processed_complete_set_atoms
    {
        return Err(Error::AuthorityUnavailable);
    }

    let mut receipt = prepare_virtual_merge_receipt_v1(input.receipt)?;
    let remaining = input
        .inventory_budget
        .authorized_complete_set_atoms
        .checked_sub(input.inventory_budget.processed_complete_set_atoms)
        .ok_or(Error::InvariantViolation)?;
    let complete_inventory = minimum_internal(
        &receipt.final_pot.internal_claims,
        receipt.final_pot.outcome_count,
    )?;
    if complete_inventory > remaining {
        return Err(Error::InvariantViolation);
    }
    let (next_budget, next_pot, next_ledger, newly_merged) = if complete_inventory == 0 {
        (
            input.inventory_budget,
            receipt.final_pot,
            input.market_ledger,
            0,
        )
    } else {
        let inventory = prepare_virtual_merge_inventory_v1(
            transition,
            input.inventory_budget,
            receipt.final_pot,
            input.market_ledger,
            complete_inventory,
        )?;
        (
            inventory.budget,
            inventory.final_pot,
            inventory.market_ledger,
            inventory.quantity,
        )
    };
    let (final_pot, cash_pot) = if next_budget.state == VirtualInventoryStateV1::Complete {
        if next_pot.cash_principal_atoms != next_budget.authorized_complete_set_atoms
            || !active_internal_is_zero(&next_pot.internal_claims, next_pot.outcome_count)
        {
            return Err(Error::InvariantViolation);
        }
        let mut funded_pot = next_pot;
        funded_pot.cash_principal_atoms = 0;
        funded_pot.validate()?;
        (
            funded_pot,
            VirtualMergeCashPotPostV1::Funded(SettlementCashPotV1::new(
                input.cash_expectation,
            )?),
        )
    } else {
        (
            next_pot,
            VirtualMergeCashPotPostV1::AwaitingCompleteSet,
        )
    };
    receipt.final_pot = final_pot;
    Ok(VirtualMergeCompositePlanV1 {
        delivery_transition_id: transition,
        receipt,
        inventory_budget: next_budget,
        market_ledger: next_ledger,
        newly_merged_complete_set_atoms: newly_merged,
        cash_pot,
    })
}

/// Deliver one selected virtual-split Egg from FinalPot to its funded buyer.
pub(crate) fn prepare_virtual_split_receipt_v1(
    input: VirtualSplitReceiptInputV1,
) -> Result<VirtualSplitReceiptPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    input.final_pot.validate()?;
    validate_split_receipt(
        input.receipt,
        input.position.outcome_count,
        ReceiptPhaseV1::Delivery,
    )?;
    validate_virtual_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.receipt_accounting_id,
        input.receipt.delivery_transition_id,
        input.receipt.market,
        input.receipt.epoch,
        input.receipt.candidate,
        input.receipt.owner_order_set_digest,
        input.receipt.buy_order_id,
        input.receipt.outcome,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
        input.receipt.slice_index,
        VirtualReceiptKindV1::Split,
        input.order,
        input.position,
        input.reservation,
        input.owner_row,
        input.final_pot,
        1,
    )?;
    if input.order.side != SettlementSideV1::Buy
        || input.reservation.side != SettlementSideV1::Buy
        || input.reservation.state != ReservationStateV1::Entitled
        || input.reservation.remaining_internal != [0; MAX_OUTCOMES]
        || (input.order.order_kind == OrderKindV1::Single
            && input.order.single_outcome != input.receipt.outcome)
    {
        return Err(Error::InvalidOrder);
    }
    let outcome = usize::from(input.receipt.outcome);
    let mut next_pot = input.final_pot;
    next_pot.internal_claims[outcome] = next_pot.internal_claims[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Error::InsufficientCash)?;
    let mut next_position = input.position;
    next_position.internal[outcome] = next_position.internal[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let (next_reservation, completes) =
        advance_reservation_delivery(input.reservation, input.receipt.quantity)?;
    next_pot.validate()?;
    next_position.validate()?;
    next_reservation.validate()?;
    Ok(VirtualSplitReceiptPlanV1 {
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_delivered_end_mask: BUY_END_MASK,
        position: next_position,
        reservation: next_reservation,
        final_pot: next_pot,
        completes_order: completes,
    })
}

/// Deliver one selected real seller Egg into virtual-merge FinalPot inventory.
pub(crate) fn prepare_virtual_merge_receipt_v1(
    input: VirtualMergeReceiptInputV1,
) -> Result<VirtualMergeReceiptPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    input.final_pot.validate()?;
    validate_merge_receipt(
        input.receipt,
        input.position.outcome_count,
        ReceiptPhaseV1::Delivery,
    )?;
    validate_virtual_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.receipt_accounting_id,
        input.receipt.delivery_transition_id,
        input.receipt.market,
        input.receipt.epoch,
        input.receipt.candidate,
        input.receipt.owner_order_set_digest,
        input.receipt.sell_order_id,
        input.receipt.outcome,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
        input.receipt.slice_index,
        VirtualReceiptKindV1::Merge,
        input.order,
        input.position,
        input.reservation,
        input.owner_row,
        input.final_pot,
        0,
    )?;
    if input.order.side != SettlementSideV1::Sell
        || input.reservation.side != SettlementSideV1::Sell
        || input.reservation.state != ReservationStateV1::Entitled
        || input.reservation.remaining_cash_atoms != 0
        || input.reservation.remaining_internal[usize::from(input.receipt.outcome)]
            < input.receipt.quantity
        || (input.order.order_kind == OrderKindV1::Single
            && input.order.single_outcome != input.receipt.outcome)
    {
        return Err(Error::InvalidOrder);
    }
    if !owner_accounting_complete(input.owner_row.accumulator) {
        return Err(Error::Incomplete);
    }
    let (mut next_reservation, completes) =
        advance_reservation_delivery(input.reservation, input.receipt.quantity)?;
    let outcome = usize::from(input.receipt.outcome);
    next_reservation.remaining_internal[outcome] = next_reservation.remaining_internal[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;
    let mut next_pot = input.final_pot;
    next_pot.internal_claims[outcome] = next_pot.internal_claims[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut next_position = input.position;
    let mut returned = [0_u64; MAX_OUTCOMES];
    if completes {
        returned = next_reservation.remaining_internal;
        let mut at = 0_usize;
        while at < MAX_OUTCOMES {
            next_position.internal[at] = next_position.internal[at]
                .checked_add(returned[at])
                .ok_or(Error::ArithmeticOverflow)?;
            next_reservation.remaining_internal[at] = 0;
            at += 1;
        }
    }
    next_pot.validate()?;
    next_position.validate()?;
    next_reservation.validate()?;
    Ok(VirtualMergeReceiptPlanV1 {
        delivery_transition_id: input.receipt.delivery_transition_id,
        receipt_accounting_id: input.receipt.receipt_accounting_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_delivered_end_mask: SELL_END_MASK,
        position: next_position,
        reservation: next_reservation,
        final_pot: next_pot,
        completes_order: completes,
        returned_internal: returned,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_virtual_binding(
    authority: AuthenticatedVirtualReceiptAuthorityV1,
    receipt: [u8; 32],
    receipt_accounting_id: [u8; 32],
    delivery_transition_id: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    order_id: [u8; 32],
    outcome: u8,
    quantity: Amount,
    consideration: u128,
    slice_index: u16,
    expected_kind: VirtualReceiptKindV1,
    order: AuthenticatedOrderMembershipV1,
    position: AuthenticatedPositionV1,
    reservation: AuthenticatedReservationV1,
    owner_row: AuthenticatedOwnerSettlementAccountV1,
    final_pot: AuthenticatedFinalPotV1,
    required_owner_state: u8,
) -> Result<()> {
    validate_virtual_accounting_binding(
        authority,
        receipt,
        receipt_accounting_id,
        delivery_transition_id,
        market,
        epoch,
        candidate,
        owner_order_set_digest,
        order_id,
        outcome,
        quantity,
        consideration,
        slice_index,
        expected_kind,
        order,
        position,
        reservation,
        owner_row,
        required_owner_state,
    )?;
    if final_pot.market != market
        || final_pot.epoch != epoch
        || final_pot.candidate != candidate
        || final_pot.owner_order_set_digest != owner_order_set_digest
        || final_pot.outcome_count != position.outcome_count
        || receipt == final_pot.account
        || authority.account == final_pot.account
        || final_pot.account == position.position
        || final_pot.account == reservation.account
        || final_pot.account == owner_row.address
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_virtual_accounting_binding(
    authority: AuthenticatedVirtualReceiptAuthorityV1,
    receipt: [u8; 32],
    receipt_accounting_id: [u8; 32],
    delivery_transition_id: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    order_id: [u8; 32],
    outcome: u8,
    quantity: Amount,
    consideration: u128,
    slice_index: u16,
    expected_kind: VirtualReceiptKindV1,
    order: AuthenticatedOrderMembershipV1,
    position: AuthenticatedPositionV1,
    reservation: AuthenticatedReservationV1,
    owner_row: AuthenticatedOwnerSettlementAccountV1,
    required_owner_state: u8,
) -> Result<()> {
    owner_row.accumulator.validate()?;
    let expectation = owner_row.accumulator.expectation;
    if owner_row.address == [0; 32]
        || owner_row.accumulator.state != required_owner_state
        || authority.kind != expected_kind
        || authority.receipt != receipt
        || authority.receipt_accounting_id != receipt_accounting_id
        || authority.delivery_transition_id != delivery_transition_id
        || authority.market != market
        || authority.epoch != epoch
        || authority.candidate != candidate
        || authority.owner_order_set_digest != owner_order_set_digest
        || authority.real_order_id != order_id
        || authority.outcome != outcome
        || authority.quantity != quantity
        || authority.consideration_price_units != consideration
        || authority.slice_index != slice_index
        || order.market != market
        || order.epoch != epoch
        || order.candidate != candidate
        || order.owner_order_set_digest != owner_order_set_digest
        || order.order_id != order_id
        || order.owner != position.owner
        || order.reservation != reservation.reservation
        || order.position_generation != position.generation
        || order.position_generation != reservation.position_generation
        || order.order_generation != reservation.order_generation
        || order.order_kind != reservation.order_kind
        || order.side != reservation.side
        || order.entitled_units != reservation.entitled_units
        || order.entitled_consideration_price_units
            != reservation.entitled_consideration_price_units
        || position.market != market
        || reservation.market != market
        || reservation.epoch != epoch
        || reservation.owner != position.owner
        || reservation.position != position.position
        || expectation.market != market
        || expectation.epoch != epoch
        || expectation.candidate != candidate
        || expectation.owner_order_set_digest != owner_order_set_digest
        || expectation.owner != position.owner
        || order.outcome_count != position.outcome_count
        || reservation.outcome_count != position.outcome_count
        || receipt == position.position
        || receipt == reservation.account
        || receipt == owner_row.address
        || authority.account == receipt
        || authority.account == position.position
        || authority.account == reservation.account
        || authority.account == owner_row.address
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptPhaseV1 {
    Accounting,
    Delivery,
}

fn validate_split_receipt(
    receipt: AuthenticatedVirtualSplitReceiptV1,
    outcome_count: u8,
    phase: ReceiptPhaseV1,
) -> Result<()> {
    validate_receipt_common(
        receipt.receipt,
        receipt.receipt_accounting_id,
        receipt.delivery_transition_id,
        receipt.market,
        receipt.epoch,
        receipt.candidate,
        receipt.owner_order_set_digest,
        receipt.buy_order_id,
        receipt.outcome,
        receipt.quantity,
        receipt.price,
        receipt.consideration_price_units,
        receipt.slice_index,
        receipt.sequence,
        receipt.settled_quantity,
        receipt.accounted_end_mask,
        receipt.delivered_end_mask,
        receipt.expected_end_mask,
        BUY_END_MASK,
        outcome_count,
        phase,
    )
}

fn validate_merge_receipt(
    receipt: AuthenticatedVirtualMergeReceiptV1,
    outcome_count: u8,
    phase: ReceiptPhaseV1,
) -> Result<()> {
    validate_receipt_common(
        receipt.receipt,
        receipt.receipt_accounting_id,
        receipt.delivery_transition_id,
        receipt.market,
        receipt.epoch,
        receipt.candidate,
        receipt.owner_order_set_digest,
        receipt.sell_order_id,
        receipt.outcome,
        receipt.quantity,
        receipt.price,
        receipt.consideration_price_units,
        receipt.slice_index,
        receipt.sequence,
        receipt.settled_quantity,
        receipt.accounted_end_mask,
        receipt.delivered_end_mask,
        receipt.expected_end_mask,
        SELL_END_MASK,
        outcome_count,
        phase,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_receipt_common(
    receipt: [u8; 32],
    receipt_accounting_id: [u8; 32],
    delivery_transition_id: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    order_id: [u8; 32],
    outcome: u8,
    quantity: Amount,
    price: Amount,
    consideration: u128,
    slice_index: u16,
    sequence: u64,
    settled_quantity: Amount,
    accounted_end_mask: u8,
    delivered_end_mask: u8,
    expected_end_mask: u8,
    required_end_mask: u8,
    outcome_count: u8,
    phase: ReceiptPhaseV1,
) -> Result<()> {
    for key in [
        receipt,
        receipt_accounting_id,
        delivery_transition_id,
        market,
        epoch,
        candidate,
        owner_order_set_digest,
        order_id,
    ] {
        if key == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
    }
    if receipt_accounting_id == delivery_transition_id
        || outcome >= outcome_count
        || quantity == 0
        || price == 0
        || consideration != u128::from(quantity) * u128::from(price)
        || sequence != u64::from(slice_index) + 1
        || settled_quantity != 0
        || expected_end_mask != required_end_mask
        || delivered_end_mask != 0
    {
        return Err(Error::InvalidOrder);
    }
    match phase {
        ReceiptPhaseV1::Accounting if accounted_end_mask != 0 => return Err(Error::Terminal),
        ReceiptPhaseV1::Delivery if accounted_end_mask != required_end_mask => {
            return Err(Error::AuthorityUnavailable);
        }
        ReceiptPhaseV1::Accounting | ReceiptPhaseV1::Delivery => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn virtual_receipt_end(
    receipt: [u8; 32],
    transition: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    owner_order_set_digest: [u8; 32],
    order: AuthenticatedOrderMembershipV1,
    consideration: u128,
    completes: bool,
    slice_index: u16,
    sequence: u64,
    expected_end_mask: u8,
) -> AuthenticatedSettlementReceiptEndV1 {
    AuthenticatedSettlementReceiptEndV1 {
        receipt,
        settlement_transition_id: transition,
        market,
        epoch,
        candidate,
        owner_order_set_digest,
        owner: order.owner,
        order_index: order.order_index,
        side: order.side,
        consideration_price_units: consideration,
        completes_order: completes,
        slice_index,
        sequence,
        consumed_end_mask: 0,
        expected_end_mask,
    }
}

fn owner_accounting_complete(accumulator: OwnerSettlementAccumulatorV1) -> bool {
    accumulator.state == 0
        && accumulator.consumed_slice_count == accumulator.expectation.expected_slice_count
        && accumulator.consumed_buy_price_units
            == accumulator.expectation.expected_buy_price_units
        && accumulator.consumed_sell_price_units
            == accumulator.expectation.expected_sell_price_units
        && accumulator.completed_buy_order_mask
            == accumulator.expectation.expected_buy_order_mask
        && accumulator.completed_sell_order_mask
            == accumulator.expectation.expected_sell_order_mask
}

fn minimum_internal(values: &[Amount; MAX_OUTCOMES], outcome_count: u8) -> Result<Amount> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidAccount);
    }
    let mut minimum = values[0];
    let mut at = 1_usize;
    while at < usize::from(outcome_count) {
        if values[at] < minimum {
            minimum = values[at];
        }
        at += 1;
    }
    Ok(minimum)
}

fn active_internal_is_zero(values: &[Amount; MAX_OUTCOMES], outcome_count: u8) -> bool {
    let mut at = 0_usize;
    while at < usize::from(outcome_count) {
        if values[at] != 0 {
            return false;
        }
        at += 1;
    }
    true
}

fn zero_padded(values: &[Amount; MAX_OUTCOMES], outcome_count: u8) -> bool {
    let mut at = usize::from(outcome_count);
    while at < MAX_OUTCOMES {
        if values[at] != 0 {
            return false;
        }
        at += 1;
    }
    true
}
