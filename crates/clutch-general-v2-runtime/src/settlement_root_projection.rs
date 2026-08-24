//! Exhaustive candidate-wide expectations for counted settlement-root creation.
//!
//! The retained Feed and complete frozen V5 page traversal own owner/order
//! membership and consideration. The fee runtime owns the candidate-wide
//! collected fee. This module joins those already-checked facts and derives the
//! exact cash-pot expectation and child counts consumed by action 39. It does
//! not authenticate Solana account ownership, PDA absence, or rent funding.

use clutch_fee_runtime_contract::{
    codec::CertifiedRecipientAllocationSummaryV3,
    projection::{CertifiedRecipientAllocationV2, SelectedOwnerFeeBookV1},
    selected::{SelectedCompositeFeeV1, SelectedCompositeFeeV2},
};
use clutch_owner_settlement::{
    owner_credit_atoms, owner_debit_atoms, owner_rounding_residue_price_units,
    OwnerSettlementExpectationBasisBookV4, SettlementCashPotExpectationV1, VirtualCashDirectionV1,
};

use crate::{SettlementAdapterErrorV1, SettlementTraversalProjectionV5};

/// Historical candidate-wide fee facts retained until all live callsites move
/// atomically to the V3 certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateFeeAggregateProjectionV1<'a> {
    /// Canonical selected fee record is absent and the aggregate fee is zero.
    NoFeeRecord,
    /// Constructor-checked selected record and complete owner fee book.
    CandidateFee {
        /// Immutable selected composite-fee semantics.
        selected: &'a SelectedCompositeFeeV1,
        /// Exhaustive owner-sorted terminal projection.
        book: &'a SelectedOwnerFeeBookV1,
    },
}

/// Private checked action-39 expectation projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRootExpectationProjectionV1 {
    cash: SettlementCashPotExpectationV1,
    expected_reservations: u16,
    expected_filled_reservations: u16,
    expected_merge_payments: u16,
}

impl SettlementRootExpectationProjectionV1 {
    /// Exact candidate-wide cash-pot expectation.
    pub const fn cash(&self) -> SettlementCashPotExpectationV1 {
        self.cash
    }

    /// Every active frozen order Reservation, including unfilled orders.
    pub const fn expected_reservations(&self) -> u16 {
        self.expected_reservations
    }

    /// Distinct filled Reservations later admitted by action 24.
    pub const fn expected_filled_reservations(&self) -> u16 {
        self.expected_filled_reservations
    }

    /// Merge receipts that later require the action-40 payment latch.
    pub const fn expected_merge_payments(&self) -> u16 {
        self.expected_merge_payments
    }
}

/// Historical root join retained for source-compatible decode-only consumers.
/// New writers must use the V3 summary join below.
pub fn derive_settlement_root_expectation_v1(
    traversal: &SettlementTraversalProjectionV5,
    fee: CandidateFeeAggregateProjectionV1<'_>,
) -> Result<SettlementRootExpectationProjectionV1, SettlementAdapterErrorV1> {
    let feed = traversal.feed();
    let (fee_record, selected_fee_atoms) = match fee {
        CandidateFeeAggregateProjectionV1::NoFeeRecord => ([0; 32], 0),
        CandidateFeeAggregateProjectionV1::CandidateFee { selected, book } => {
            let selected_fee_atoms = u64::try_from(book.selected_fee_atoms())
                .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
            if book.fee_record() != selected.fee_record()
                || book.settlement_candidate() != selected.selected_candidate()
                || book.owner_count()
                    != u8::try_from(traversal.expected_owner_count())
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?
                || selected_fee_atoms == 0
                || selected.market().0 != feed.market.bytes()
                || selected.epoch().0 != feed.epoch.bytes()
                || selected.selected_candidate().0 != feed.settlement_candidate_id.bytes()
                || selected.price_scale() != feed.price_scale
                || selected.outcome_count() != feed.outcome_count
            {
                return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
            }
            (selected.fee_record().0, selected_fee_atoms)
        }
    };
    derive_settlement_root_expectation_from_fee_total_v1(
        traversal,
        fee_record,
        selected_fee_atoms,
    )
}

/// Historical O(1) V2 certificate join retained until the live SBF callsite
/// cutover. No current writer may create V2 bytes.
pub fn derive_settlement_root_expectation_from_certified_fee_v2(
    traversal: &SettlementTraversalProjectionV5,
    selected: &SelectedCompositeFeeV1,
    certified: &CertifiedRecipientAllocationV2,
) -> Result<SettlementRootExpectationProjectionV1, SettlementAdapterErrorV1> {
    let feed = traversal.feed();
    let allocation = certified.allocation();
    let selected_fee_atoms = allocation.collected_fee_atoms();
    if selected_fee_atoms == 0
        || allocation.fee_record() != selected.fee_record()
        || certified.owner_order_set_digest().0
            != traversal.owner_order_set_digest().bytes()
        || certified.owner_count() != traversal.expected_owner_count()
        || selected.market().0 != feed.market.bytes()
        || selected.epoch().0 != feed.epoch.bytes()
        || selected.selected_candidate().0 != feed.settlement_candidate_id.bytes()
        || selected.price_scale() != feed.price_scale
        || selected.outcome_count() != feed.outcome_count
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    derive_settlement_root_expectation_from_fee_total_v1(
        traversal,
        selected.fee_record().0,
        selected_fee_atoms,
    )
}

/// Derive the canonical absent-selected-fee action-39 expectation.
///
/// The live adapter must authenticate the selected-fee PDA as a zero-data
/// System account before entering this branch.
pub fn derive_zero_fee_settlement_root_expectation_v1(
    traversal: &SettlementTraversalProjectionV5,
) -> Result<SettlementRootExpectationProjectionV1, SettlementAdapterErrorV1> {
    derive_settlement_root_expectation_from_fee_total_v1(traversal, [0; 32], 0)
}

/// Derive action-39 expectations from the current V2 weight-stream-certified
/// recipient allocation.
///
/// The persisted allocation owns the exact collected fee total. The V3
/// certificate equality-binds its traversal digest/count and omits the
/// historical selected-owner fee-book content ID entirely.
pub fn derive_settlement_root_expectation_from_certified_fee_v3(
    traversal: &SettlementTraversalProjectionV5,
    selected: &SelectedCompositeFeeV2,
    certified: CertifiedRecipientAllocationSummaryV3,
) -> Result<SettlementRootExpectationProjectionV1, SettlementAdapterErrorV1> {
    let feed = traversal.feed();
    let selected_fee_atoms = certified.collected_fee_atoms();
    if certified.fee_record() != selected.fee_record()
        || certified.owner_order_set_digest().0
            != traversal.owner_order_set_digest().bytes()
        || certified.traversed_owner_count() != traversal.expected_owner_count()
        || certified.nonzero_weight_row_count() != certified.row_count()
        || selected.market().0 != feed.market.bytes()
        || selected.epoch().0 != feed.epoch.bytes()
        || selected.selected_candidate().0 != feed.settlement_candidate_id.bytes()
        || selected.price_scale() != feed.price_scale
        || selected.outcome_count() != feed.outcome_count
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let cash_fee_record = if selected_fee_atoms == 0 {
        [0; 32]
    } else {
        selected.fee_record().0
    };
    derive_settlement_root_expectation_from_fee_total_v1(
        traversal,
        cash_fee_record,
        selected_fee_atoms,
    )
}

fn derive_settlement_root_expectation_from_fee_total_v1(
    traversal: &SettlementTraversalProjectionV5,
    fee_record: [u8; 32],
    selected_fee_atoms: u64,
) -> Result<SettlementRootExpectationProjectionV1, SettlementAdapterErrorV1> {
    let feed = traversal.feed();
    let cash = SettlementCashPotExpectationV1 {
        market: feed.market.bytes(),
        epoch: feed.epoch.bytes(),
        candidate: feed.settlement_candidate_id.bytes(),
        owner_order_set_digest: traversal.owner_order_set_digest().bytes(),
        fee_record,
        price_scale: feed.price_scale,
        owner_count: traversal.expected_owner_count(),
        consideration_debit_atoms: traversal.consideration_debit_atoms(),
        seller_credit_atoms: traversal.seller_credit_atoms(),
        selected_fee_atoms,
        rounding_pot_price_units: traversal.rounding_pot_price_units(),
        virtual_cash_direction: traversal.virtual_cash_direction(),
        virtual_cash_atoms: traversal.virtual_cash_atoms(),
    };
    cash.validate()?;
    if cash.market != feed.market.bytes()
        || cash.epoch != feed.epoch.bytes()
        || cash.candidate != feed.settlement_candidate_id.bytes()
        || cash.owner_order_set_digest != traversal.owner_order_set_digest().bytes()
        || cash.price_scale != feed.price_scale
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(SettlementRootExpectationProjectionV1 {
        cash,
        expected_reservations: traversal.expected_reservation_count(),
        expected_filled_reservations: traversal.expected_filled_reservation_count(),
        expected_merge_payments: traversal.expected_merge_payment_count(),
    })
}

fn derive_cash_expectation_from_basis_v1(
    basis: &OwnerSettlementExpectationBasisBookV4,
    fee_record: [u8; 32],
    selected_fee_atoms: u64,
    virtual_cash_direction: VirtualCashDirectionV1,
    virtual_cash_atoms: u64,
) -> Result<SettlementCashPotExpectationV1, SettlementAdapterErrorV1> {
    let first = basis
        .row(0)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let mut consideration_debit_atoms = 0u64;
    let mut seller_credit_atoms = 0u64;
    let mut rounding_pot_price_units = 0u128;
    let mut ordinal = 0u16;
    while ordinal < basis.owner_count() {
        let owner = basis
            .row(ordinal)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        if owner.market() != first.market()
            || owner.epoch() != first.epoch()
            || owner.candidate() != first.candidate()
            || owner.owner_order_set_digest() != first.owner_order_set_digest()
            || owner.price_scale() != first.price_scale()
        {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        let buy = owner.expected_buy_price_units();
        let sell = owner.expected_sell_price_units();
        consideration_debit_atoms = consideration_debit_atoms
            .checked_add(owner_debit_atoms(buy.value, owner.price_scale(), 0)?)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        seller_credit_atoms = seller_credit_atoms
            .checked_add(owner_credit_atoms(sell.value, owner.price_scale())?)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        rounding_pot_price_units = rounding_pot_price_units
            .checked_add(owner_rounding_residue_price_units(
                buy.value,
                sell.value,
                owner.price_scale(),
            )?)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        ordinal = ordinal
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    let expectation = SettlementCashPotExpectationV1 {
        market: first.market(),
        epoch: first.epoch(),
        candidate: first.candidate(),
        owner_order_set_digest: first.owner_order_set_digest(),
        fee_record,
        price_scale: first.price_scale(),
        owner_count: basis.owner_count(),
        consideration_debit_atoms,
        seller_credit_atoms,
        selected_fee_atoms,
        rounding_pot_price_units,
        virtual_cash_direction,
        virtual_cash_atoms,
    };
    expectation.validate()?;
    Ok(expectation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_owner_settlement::{
        build_owner_settlement_expectation_basis_book_v4, PresentConsiderationV2, SettlementSideV1,
        VerifiedSettlementOrderV4, MAX_ORDERS,
    };

    #[test]
    fn explicitly_present_zero_price_is_not_treated_as_absent() {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [5; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        orders[1] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 1,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        let basis = build_owner_settlement_expectation_basis_book_v4(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 2,
        )
        .unwrap();
        let value = derive_cash_expectation_from_basis_v1(
            &basis,
            [0; 32],
            0,
            VirtualCashDirectionV1::None,
            0,
        )
        .unwrap();
        assert_eq!(value.owner_count, 2);
        assert_eq!(value.consideration_debit_atoms, 0);
        assert_eq!(value.seller_credit_atoms, 0);
    }

    #[test]
    fn fee_presence_and_identity_are_not_decoupled() {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [5; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::new(100),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        orders[1] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 1,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(100),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        let basis = build_owner_settlement_expectation_basis_book_v4(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 2,
        )
        .unwrap();
        assert!(derive_cash_expectation_from_basis_v1(
            &basis,
            [0; 32],
            1,
            VirtualCashDirectionV1::None,
            0,
        )
        .is_err());
    }
}
