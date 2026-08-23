//! Explicit virtual complete-set inventory and one-real-end receipt contracts.
//!
//! A virtual end is never represented as a synthetic owner, Position,
//! Reservation, or direct-pair end. Split inventory converts explicitly owned
//! FinalPot cash principal into one native claim of every outcome. Merge
//! inventory performs the exact inverse. One-real-end receipt planners then
//! move a selected outcome between the FinalPot and an authenticated real
//! Position while advancing only the real Reservation and owner row.

use crate::{
    direct::advance_reservation, prepare_account_receipt_end_v1, Amount,
    AuthenticatedOrderMembershipV1, AuthenticatedOwnerSettlementAccountV1,
    AuthenticatedPositionV1, AuthenticatedReservationV1, AuthenticatedSettlementReceiptEndV1,
    Error, OrderKindV1, OwnerSettlementReceiptAccountingPlanV1, ReservationStateV1, Result,
    SettlementSideV1, MAX_OUTCOMES,
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
pub fn prepare_virtual_split_inventory_v1(
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
pub fn prepare_virtual_merge_inventory_v1(
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
    /// Exact complete transition identity.
    pub settlement_transition_id: [u8; 32],
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
            self.settlement_transition_id,
            self.real_order_id,
        ] {
            if key == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.quantity == 0
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
    /// Exact complete transition identity.
    pub settlement_transition_id: [u8; 32],
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
    /// Independent real-end latch; must be zero.
    pub consumed_end_mask: u8,
    /// Must name only the real buy end.
    pub expected_end_mask: u8,
}

/// Authenticated one-real-sell-end receipt for a virtual merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVirtualMergeReceiptV1 {
    /// Canonical receipt account.
    pub receipt: [u8; 32],
    /// Exact complete transition identity.
    pub settlement_transition_id: [u8; 32],
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
    /// Independent real-end latch; must be zero.
    pub consumed_end_mask: u8,
    /// Must name only the real sell end.
    pub expected_end_mask: u8,
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
    /// Buyer owner-settlement row.
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
    /// Seller owner-settlement row.
    pub owner_row: AuthenticatedOwnerSettlementAccountV1,
    /// FinalPot receiving the Egg inventory.
    pub final_pot: AuthenticatedFinalPotV1,
}

/// Atomic poststate of one virtual-split real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualSplitReceiptPlanV1 {
    /// Exact complete transition identity.
    pub settlement_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Settled receipt quantity.
    pub receipt_settled_quantity: Amount,
    /// Buy-end latch poststate.
    pub receipt_consumed_end_mask: u8,
    /// Buyer Position poststate.
    pub position: AuthenticatedPositionV1,
    /// Buy Reservation poststate.
    pub reservation: AuthenticatedReservationV1,
    /// FinalPot poststate after Egg delivery.
    pub final_pot: AuthenticatedFinalPotV1,
    /// Buyer owner-row poststate.
    pub owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Whether this slice completed the buyer order.
    pub completes_order: bool,
    /// Completed Reservation cash envelope handed to the owner row.
    pub reserved_cash_handoff_atoms: Amount,
}

/// Atomic poststate of one virtual-merge real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VirtualMergeReceiptPlanV1 {
    /// Exact complete transition identity.
    pub settlement_transition_id: [u8; 32],
    /// Receipt account to write.
    pub receipt: [u8; 32],
    /// Settled receipt quantity.
    pub receipt_settled_quantity: Amount,
    /// Sell-end latch poststate.
    pub receipt_consumed_end_mask: u8,
    /// Seller Position poststate.
    pub position: AuthenticatedPositionV1,
    /// Sell Reservation poststate.
    pub reservation: AuthenticatedReservationV1,
    /// FinalPot poststate after Egg delivery.
    pub final_pot: AuthenticatedFinalPotV1,
    /// Seller owner-row poststate.
    pub owner_accounting: OwnerSettlementReceiptAccountingPlanV1,
    /// Whether this slice completed the seller order.
    pub completes_order: bool,
    /// Entire unfilled seller vector returned at completion.
    pub returned_internal: [Amount; MAX_OUTCOMES],
}

/// Deliver one selected virtual-split Egg from FinalPot to its real buyer.
pub fn prepare_virtual_split_receipt_v1(
    input: VirtualSplitReceiptInputV1,
) -> Result<VirtualSplitReceiptPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    input.final_pot.validate()?;
    validate_split_receipt(input.receipt, input.position.outcome_count)?;
    validate_virtual_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.settlement_transition_id,
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
    let outcome = usize::from(input.receipt.outcome);
    let mut next_pot = input.final_pot;
    next_pot.internal_claims[outcome] = next_pot.internal_claims[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Error::InsufficientCash)?;
    let mut next_position = input.position;
    next_position.internal[outcome] = next_position.internal[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let (mut next_reservation, completes) = advance_reservation(
        input.reservation,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
    )?;
    let handoff = if completes {
        let value = next_reservation.remaining_cash_atoms;
        next_reservation.remaining_cash_atoms = 0;
        value
    } else {
        0
    };
    next_pot.validate()?;
    next_position.validate()?;
    next_reservation.validate()?;
    let owner_accounting = prepare_account_receipt_end_v1(
        input.owner_row,
        virtual_receipt_end(
            input.receipt.receipt,
            input.receipt.settlement_transition_id,
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
    Ok(VirtualSplitReceiptPlanV1 {
        settlement_transition_id: input.receipt.settlement_transition_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_consumed_end_mask: BUY_END_MASK,
        position: next_position,
        reservation: next_reservation,
        final_pot: next_pot,
        owner_accounting,
        completes_order: completes,
        reserved_cash_handoff_atoms: handoff,
    })
}

/// Deliver one selected real seller Egg into virtual-merge FinalPot inventory.
pub fn prepare_virtual_merge_receipt_v1(
    input: VirtualMergeReceiptInputV1,
) -> Result<VirtualMergeReceiptPlanV1> {
    input.authority.validate()?;
    input.order.validate()?;
    input.position.validate()?;
    input.reservation.validate()?;
    input.final_pot.validate()?;
    validate_merge_receipt(input.receipt, input.position.outcome_count)?;
    validate_virtual_binding(
        input.authority,
        input.receipt.receipt,
        input.receipt.settlement_transition_id,
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
    let (mut next_reservation, completes) = advance_reservation(
        input.reservation,
        input.receipt.quantity,
        input.receipt.consideration_price_units,
    )?;
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
    let owner_accounting = prepare_account_receipt_end_v1(
        input.owner_row,
        virtual_receipt_end(
            input.receipt.receipt,
            input.receipt.settlement_transition_id,
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
    Ok(VirtualMergeReceiptPlanV1 {
        settlement_transition_id: input.receipt.settlement_transition_id,
        receipt: input.receipt.receipt,
        receipt_settled_quantity: input.receipt.quantity,
        receipt_consumed_end_mask: SELL_END_MASK,
        position: next_position,
        reservation: next_reservation,
        final_pot: next_pot,
        owner_accounting,
        completes_order: completes,
        returned_internal: returned,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_virtual_binding(
    authority: AuthenticatedVirtualReceiptAuthorityV1,
    receipt: [u8; 32],
    transition: [u8; 32],
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
) -> Result<()> {
    let expectation = owner_row.accumulator.expectation;
    if authority.kind != expected_kind
        || authority.receipt != receipt
        || authority.settlement_transition_id != transition
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
        || final_pot.market != market
        || final_pot.epoch != epoch
        || final_pot.candidate != candidate
        || final_pot.owner_order_set_digest != owner_order_set_digest
        || final_pot.outcome_count != position.outcome_count
        || order.outcome_count != position.outcome_count
        || reservation.outcome_count != position.outcome_count
        || receipt == final_pot.account
        || receipt == position.position
        || receipt == reservation.account
        || receipt == owner_row.address
        || authority.account == receipt
        || authority.account == final_pot.account
        || authority.account == position.position
        || authority.account == reservation.account
        || authority.account == owner_row.address
        || final_pot.account == position.position
        || final_pot.account == reservation.account
        || final_pot.account == owner_row.address
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(())
}

fn validate_split_receipt(
    receipt: AuthenticatedVirtualSplitReceiptV1,
    outcome_count: u8,
) -> Result<()> {
    validate_receipt_common(
        receipt.receipt,
        receipt.settlement_transition_id,
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
        receipt.consumed_end_mask,
        receipt.expected_end_mask,
        BUY_END_MASK,
        outcome_count,
    )
}

fn validate_merge_receipt(
    receipt: AuthenticatedVirtualMergeReceiptV1,
    outcome_count: u8,
) -> Result<()> {
    validate_receipt_common(
        receipt.receipt,
        receipt.settlement_transition_id,
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
        receipt.consumed_end_mask,
        receipt.expected_end_mask,
        SELL_END_MASK,
        outcome_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_receipt_common(
    receipt: [u8; 32],
    transition: [u8; 32],
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
    consumed_end_mask: u8,
    expected_end_mask: u8,
    required_end_mask: u8,
    outcome_count: u8,
) -> Result<()> {
    for key in [
        receipt,
        transition,
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
    if outcome >= outcome_count
        || quantity == 0
        || price == 0
        || consideration != u128::from(quantity) * u128::from(price)
        || sequence != u64::from(slice_index) + 1
        || settled_quantity != 0
        || consumed_end_mask != 0
        || expected_end_mask != required_end_mask
    {
        return Err(Error::InvalidOrder);
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
