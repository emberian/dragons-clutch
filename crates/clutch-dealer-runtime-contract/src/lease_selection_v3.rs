// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed admission of General V2 selection, verified Dealer economics, and V3 price evidence.

use clutch_batch::dealer_leg_v2::{
    DealerCashAllocationV2, VerifiedDealerLegV2, MAX_DEALER_ROWS_V2,
};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, complete_candidate_feed_v2,
    encode_score_v2_q_first_admitted_tie_v1, settlement_witness_digest_v1, FirstAdmittedTieV1,
    ScoreV2QComponentsV1, SelectedCandidateV1AccountV1, SettlementCandidateKindV1,
    SettlementSliceLegKindV1, SettlementSliceV1, SCORE_V2_Q_ACTIVE_RANK_BYTES,
    SETTLEMENT_SLICE_BYTES,
};
use clutch_price_measure::VerifiedPriceMeasureV3;
use sha2::{Digest, Sha256};

use crate::{
    validate_padding_u64, DealerEpochBindingV2, DealerGeneralEpochEvidenceV3, DealerLeaseV2,
    DealerPolicyV1, DealerSha256V1, Error, Id, Result, SettlementPotV2, MAX_OUTCOMES,
};

/// Domain for an in-memory authenticated-order projection continuity digest.
pub const DEALER_SETTLEMENT_ORDER_PROJECTION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/dealer-settlement-order-projection/v3\0";

/// User-order side retained from an adapter-authenticated General order set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerSettlementOrderSideV3 {
    /// User buys Eggs from the covered dealer.
    Buy = 1,
    /// User sells Eggs to the covered dealer.
    Sell = 2,
}

/// Exact owner-blind order fields needed to derive one Dealer settlement row.
///
/// This is a forgeable projection, not account authority. The SBF adapter must
/// derive every active row from the authenticated frozen General order set
/// named by SelectedCandidate before constructing selection evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSettlementOrderProjectionV3 {
    /// Canonical order index used by General settlement slices.
    pub order_index: u8,
    /// Canonical RelationV2 order identity.
    pub order_id: Id,
    /// Exact user side.
    pub side: DealerSettlementOrderSideV3,
    /// Native Egg atoms per filled order unit, then canonical zero padding.
    pub coefficients: [u64; MAX_OUTCOMES],
}

/// Canonical inactive order-projection row.
pub const EMPTY_DEALER_SETTLEMENT_ORDER_PROJECTION_V3: DealerSettlementOrderProjectionV3 =
    DealerSettlementOrderProjectionV3 {
        order_index: 0,
        order_id: Id::ZERO,
        side: DealerSettlementOrderSideV3::Buy,
        coefficients: [0; MAX_OUTCOMES],
    };

/// Exact active Dealer-row projection of the authenticated frozen order set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSettlementOrderSetV3 {
    /// Active rows in the same strict order-ID order as the verified Dealer leg.
    pub rows: [DealerSettlementOrderProjectionV3; MAX_DEALER_ROWS_V2],
    /// Active row count.
    pub row_count: u8,
}

impl DealerSettlementOrderSetV3 {
    fn validate_against(&self, dealer: &VerifiedDealerLegV2, feed_order_count: u8) -> Result<Id> {
        let count = usize::from(self.row_count);
        if self.row_count != dealer.allocation_count() || count == 0 || count > MAX_DEALER_ROWS_V2 {
            return Err(Error::MismatchedBinding);
        }
        let mut facility_sell = [0u64; MAX_OUTCOMES];
        let mut facility_buy = [0u64; MAX_OUTCOMES];
        let mut hasher = Sha256::new();
        hasher.update(DEALER_SETTLEMENT_ORDER_PROJECTION_DOMAIN_V3);
        hasher.update([self.row_count]);
        let mut row = 0usize;
        while row < count {
            let projection = self.rows[row];
            let allocation = dealer.allocations()[row];
            projection.order_id.validate_live()?;
            validate_padding_u64(dealer.outcome_count(), &projection.coefficients)?;
            if projection.order_index >= feed_order_count
                || projection.order_id.bytes() != allocation.order_id
                || (projection.side == DealerSettlementOrderSideV3::Buy
                    && allocation.user_cash_out_atoms != 0)
                || (projection.side == DealerSettlementOrderSideV3::Sell
                    && allocation.user_cash_in_atoms != 0)
            {
                return Err(Error::MismatchedBinding);
            }
            let mut prior = 0usize;
            while prior < row {
                if self.rows[prior].order_index == projection.order_index {
                    return Err(Error::MismatchedBinding);
                }
                prior += 1;
            }
            let mut has_coefficient = false;
            let mut outcome = 0usize;
            while outcome < usize::from(dealer.outcome_count()) {
                let eggs = projection.coefficients[outcome]
                    .checked_mul(allocation.dealer_fill_units)
                    .ok_or(Error::ArithmeticOverflow)?;
                has_coefficient |= projection.coefficients[outcome] != 0;
                let aggregate = match projection.side {
                    DealerSettlementOrderSideV3::Buy => &mut facility_sell[outcome],
                    DealerSettlementOrderSideV3::Sell => &mut facility_buy[outcome],
                };
                *aggregate = aggregate
                    .checked_add(eggs)
                    .ok_or(Error::ArithmeticOverflow)?;
                outcome += 1;
            }
            if !has_coefficient {
                return Err(Error::InvalidParameter);
            }
            hasher.update([projection.order_index, projection.side as u8]);
            hasher.update(projection.order_id.bytes());
            outcome = 0;
            while outcome < MAX_OUTCOMES {
                hasher.update(projection.coefficients[outcome].to_le_bytes());
                outcome += 1;
            }
            row += 1;
        }
        while row < MAX_DEALER_ROWS_V2 {
            if self.rows[row] != EMPTY_DEALER_SETTLEMENT_ORDER_PROJECTION_V3 {
                return Err(Error::InvalidParameter);
            }
            row += 1;
        }
        if facility_sell != dealer.trade().sell_to_users
            || facility_buy != dealer.trade().buy_from_users
        {
            return Err(Error::ConservationFailure);
        }
        let digest = Id::from_bytes(hasher.finalize().into());
        digest.validate_live()?;
        Ok(digest)
    }

    fn row_for_order_index(&self, order_index: u8) -> Option<usize> {
        let mut row = 0usize;
        while row < usize::from(self.row_count) {
            if self.rows[row].order_index == order_index {
                return Some(row);
            }
            row += 1;
        }
        None
    }
}

/// Private-capability result for one exact Dealer settlement row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSettlementRowEvidenceV3 {
    selected_candidate_account_id: Id,
    settlement_candidate_id: Id,
    settlement_witness_digest: Id,
    row_index: u8,
    order: DealerSettlementOrderProjectionV3,
    allocation: DealerCashAllocationV2,
    native_eggs: [u64; MAX_OUTCOMES],
}

impl DealerSettlementRowEvidenceV3 {
    /// Exact selected artifact account.
    pub const fn selected_candidate_account_id(&self) -> Id {
        self.selected_candidate_account_id
    }

    /// Exact final SettlementCandidateId.
    pub const fn settlement_candidate_id(&self) -> Id {
        self.settlement_candidate_id
    }

    /// Exact General settlement-witness digest.
    pub const fn settlement_witness_digest(&self) -> Id {
        self.settlement_witness_digest
    }

    /// Canonical Dealer-row cursor.
    pub const fn row_index(&self) -> u8 {
        self.row_index
    }

    /// Exact owner-blind order projection.
    pub const fn order(&self) -> DealerSettlementOrderProjectionV3 {
        self.order
    }

    /// Exact verified cash and external-fee allocation.
    pub const fn allocation(&self) -> DealerCashAllocationV2 {
        self.allocation
    }

    /// Exact native Egg transfer vector for this row.
    pub const fn native_eggs(&self) -> [u64; MAX_OUTCOMES] {
        self.native_eggs
    }
}

/// In-memory capability joining all upstream selection owners required by one Lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeaseSelectionEvidenceV3 {
    selected_candidate_account_id: Id,
    selected: SelectedCandidateV1AccountV1,
    dealer_leg: VerifiedDealerLegV2,
    price: VerifiedPriceMeasureV3,
    order_projection_digest: Id,
    feed_order_count: u8,
}

impl DealerLeaseSelectionEvidenceV3 {
    /// Construct only from canonical checked General, Dealer-leg, and V3 price values.
    pub fn new(
        selected_candidate_account_id: Id,
        selected: SelectedCandidateV1AccountV1,
        dealer_leg: VerifiedDealerLegV2,
        price: VerifiedPriceMeasureV3,
        selected_feed_bytes: &[u8],
        order_set: &DealerSettlementOrderSetV3,
        epoch: &DealerEpochBindingV2,
        general: &DealerGeneralEpochEvidenceV3,
        policy: &DealerPolicyV1,
    ) -> Result<Self> {
        selected_candidate_account_id.validate_live()?;
        selected.validate().map_err(|_| Error::MismatchedBinding)?;
        epoch.validate()?;
        general.validate_epoch(epoch)?;
        general.validate_selected_epoch()?;
        policy.validate()?;
        let (feed_header, feed_tail) = complete_candidate_feed_v2(selected_feed_bytes, true)
            .map_err(|_| Error::MismatchedBinding)?;
        let candidate_bundle =
            candidate_bundle_digest_v1(&DealerSha256V1, selected_feed_bytes, true)
                .map_err(|_| Error::MismatchedBinding)?;
        let settlement_witness = settlement_witness_digest_v1(
            &DealerSha256V1,
            feed_header.base_relation_candidate_id,
            feed_header.slice_count,
            feed_tail.slices_le(),
        )
        .map_err(|_| Error::MismatchedBinding)?;
        let order_projection_digest =
            order_set.validate_against(&dealer_leg, feed_header.order_count)?;
        validate_dealer_settlement_slices_v3(
            feed_tail.slices_le(),
            feed_header.order_count,
            feed_header.outcome_count,
            order_set,
            &dealer_leg,
        )?;
        let price_bindings = price.bindings();
        let final_candidate = Id::from_bytes(*dealer_leg.dealer_economic_candidate_digest());
        let score = dealer_leg.score();
        let expected_rank = encode_score_v2_q_first_admitted_tie_v1(
            ScoreV2QComponentsV1 {
                certified_risk_flow_atoms: score.risk.certified_risk_flow_atoms,
                cash_equivalent_direct_flow_atoms: score.cash_equivalent_direct_flow_atoms,
                virtual_churn_atoms: score.virtual_churn_atoms,
                settlement_candidate_id: selected.settlement_candidate_id,
            },
            FirstAdmittedTieV1 {
                ordinal: selected.ordinal,
            },
        )
        .map_err(|_| Error::MismatchedBinding)?;
        if selected.candidate_kind != SettlementCandidateKindV1::CoveredDealer
            || selected.entitlement_state != 2
            || selected.next_slice_index != selected.slice_count
            || Id::from_bytes(selected.epoch.bytes()) != epoch.epoch_account_id
            || Id::from_bytes(selected.market.bytes()) != general.market_runtime_account_id()
            || feed_header.epoch != selected.epoch
            || feed_header.market != selected.market
            || feed_header.order_set != selected.order_set
            || feed_header.economic_domain_digest != selected.economic_domain_digest
            || feed_header.settlement_candidate_id != selected.settlement_candidate_id
            || feed_header.base_relation_candidate_id != selected.base_relation_candidate_id
            || feed_header.settlement_witness_digest != selected.settlement_witness_digest
            || feed_header.candidate_price_digest != selected.candidate_price_digest
            || feed_header.price_body_digest != selected.price_body_digest
            || feed_header.relation_policy_id != selected.relation_policy_id
            || feed_header.price_measure_policy_v1_id != selected.price_measure_policy_v1_id
            || feed_header.native_claim_basis_id != selected.native_claim_basis_id
            || feed_header.epoch_generation != selected.epoch_generation
            || feed_header.slice_count != selected.slice_count
            || feed_header.candidate_kind != SettlementCandidateKindV1::CoveredDealer
            || candidate_bundle != selected.candidate_bundle_digest
            || settlement_witness != selected.settlement_witness_digest
            || selected.epoch_generation != epoch.general_epoch_generation
            || Id::from_bytes(selected.settlement_candidate_id.bytes()) != final_candidate
            || Id::from_bytes(selected.relation_policy_id.bytes()) != policy.relation_v2_id
            || Id::from_bytes(selected.price_measure_policy_v1_id.bytes())
                != policy.price_measure_policy_id
            || Id::from_bytes(selected.native_claim_basis_id.bytes()) != policy.claim_basis_id
            || Id::from_bytes(selected.economic_domain_digest.bytes()) != epoch.economic_domain_id
            || Id::from_bytes(selected.price_body_digest.bytes())
                != Id::from_bytes(price.body_digest())
            || Id::from_bytes(selected.selected_feed.bytes())
                != Id::from_bytes(price_bindings.candidate_feed)
            || Id::from_bytes(selected.economic_domain_digest.bytes())
                != Id::from_bytes(price_bindings.relation_domain_digest)
            || Id::from_bytes(selected.native_claim_basis_id.bytes())
                != Id::from_bytes(price_bindings.basis_digest)
            || Id::from_bytes(selected.candidate_price_digest.bytes())
                != Id::from_bytes(price_bindings.candidate_price_digest)
            || price.native_outcome_count() != policy.outcome_count
            || dealer_leg.outcome_count() != policy.outcome_count
            || dealer_leg.score().digest != final_candidate.bytes()
            || selected.rank_key != expected_rank
            || usize::from(selected.rank_key_len) != SCORE_V2_Q_ACTIVE_RANK_BYTES
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(Self {
            selected_candidate_account_id,
            selected,
            dealer_leg,
            price,
            order_projection_digest,
            feed_order_count: feed_header.order_count,
        })
    }

    /// Derive one exact private row capability from the same checked order projection.
    pub fn settlement_row(
        &self,
        order_set: &DealerSettlementOrderSetV3,
        row_index: u8,
    ) -> Result<DealerSettlementRowEvidenceV3> {
        if row_index >= self.dealer_leg.allocation_count()
            || order_set.validate_against(&self.dealer_leg, self.feed_order_count)?
                != self.order_projection_digest
        {
            return Err(Error::MismatchedBinding);
        }
        let index = usize::from(row_index);
        let order = order_set.rows[index];
        let allocation = self.dealer_leg.allocations()[index];
        let mut native_eggs = [0u64; MAX_OUTCOMES];
        let mut outcome = 0usize;
        while outcome < usize::from(self.dealer_leg.outcome_count()) {
            native_eggs[outcome] = order.coefficients[outcome]
                .checked_mul(allocation.dealer_fill_units)
                .ok_or(Error::ArithmeticOverflow)?;
            outcome += 1;
        }
        Ok(DealerSettlementRowEvidenceV3 {
            selected_candidate_account_id: self.selected_candidate_account_id,
            settlement_candidate_id: Id::from_bytes(self.selected.settlement_candidate_id.bytes()),
            settlement_witness_digest: Id::from_bytes(
                self.selected.settlement_witness_digest.bytes(),
            ),
            row_index,
            order,
            allocation,
            native_eggs,
        })
    }

    /// Require a Lease and Pot to be the exact executable projection of this evidence.
    pub fn validate_lease_pot(
        &self,
        lease: &DealerLeaseV2,
        pot: &SettlementPotV2,
        epoch: &DealerEpochBindingV2,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        lease.validate()?;
        pot.validate_against_lease(lease)?;
        epoch.validate()?;
        policy.validate()?;
        let verdict = self.dealer_leg.verdict();
        let final_candidate = Id::from_bytes(*self.dealer_leg.dealer_economic_candidate_digest());
        let quote = Id::from_bytes(*self.dealer_leg.dealer_quote_semantics_digest());
        let price_body = Id::from_bytes(self.price.body_digest());
        let mut user_cash_in_atoms = 0u64;
        let mut user_cash_out_atoms = 0u64;
        let mut row = 0usize;
        while row < usize::from(verdict.allocation_count) {
            user_cash_in_atoms = user_cash_in_atoms
                .checked_add(verdict.allocations[row].user_cash_in_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            user_cash_out_atoms = user_cash_out_atoms
                .checked_add(verdict.allocations[row].user_cash_out_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            row += 1;
        }
        let (dealer_net_cash_in_atoms, dealer_net_cash_out_atoms) =
            if user_cash_in_atoms >= user_cash_out_atoms {
                (
                    user_cash_in_atoms
                        .checked_sub(user_cash_out_atoms)
                        .ok_or(Error::ArithmeticOverflow)?,
                    0,
                )
            } else {
                (
                    0,
                    user_cash_out_atoms
                        .checked_sub(user_cash_in_atoms)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
            };
        if lease.selected_candidate_account_id != self.selected_candidate_account_id
            || lease.epoch_id != epoch.epoch_id
            || lease.epoch_binding_account_id != epoch.epoch_binding_account_id
            || lease.settlement_candidate_id != final_candidate
            || lease.settlement_candidate_id
                != Id::from_bytes(self.selected.settlement_candidate_id.bytes())
            || lease.upstream_economic_candidate_id
                != Id::from_bytes(self.selected.base_relation_candidate_id.bytes())
            || lease.quote_id != quote
            || lease.dealer_leg_verdict_id != final_candidate
            || lease.curve_price_certificate_id != price_body
            || lease.settlement_rows_root
                != Id::from_bytes(self.selected.settlement_witness_digest.bytes())
            || lease.row_count != u16::from(verdict.allocation_count)
            || lease.outcome_count != policy.outcome_count
            || pot.user_cash_in_atoms != user_cash_in_atoms
            || pot.user_cash_out_atoms != user_cash_out_atoms
            || pot.dealer_net_cash_in_atoms != dealer_net_cash_in_atoms
            || pot.dealer_net_cash_out_atoms != dealer_net_cash_out_atoms
            || pot.facility_buy_eggs != verdict.trade.buy_from_users
            || pot.facility_sell_eggs != verdict.trade.sell_to_users
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

fn validate_dealer_settlement_slices_v3(
    encoded_slices: &[u8],
    feed_order_count: u8,
    outcome_count: u8,
    order_set: &DealerSettlementOrderSetV3,
    dealer: &VerifiedDealerLegV2,
) -> Result<()> {
    if !encoded_slices.len().is_multiple_of(SETTLEMENT_SLICE_BYTES) {
        return Err(Error::InvalidParameter);
    }
    let mut slice_at = 0usize;
    while slice_at < encoded_slices.len() {
        let slice = SettlementSliceV1::decode(
            &encoded_slices[slice_at..slice_at + SETTLEMENT_SLICE_BYTES],
            feed_order_count,
            outcome_count,
        )
        .map_err(|_| Error::MismatchedBinding)?;
        let facility_order = match (slice.buy_kind, slice.sell_kind) {
            (SettlementSliceLegKindV1::Order, SettlementSliceLegKindV1::CoveredDealerSell) => {
                Some((slice.buy_index, DealerSettlementOrderSideV3::Buy))
            }
            (SettlementSliceLegKindV1::CoveredDealerBuy, SettlementSliceLegKindV1::Order) => {
                Some((slice.sell_index, DealerSettlementOrderSideV3::Sell))
            }
            (SettlementSliceLegKindV1::Order, SettlementSliceLegKindV1::Order) => None,
            _ => return Err(Error::MismatchedBinding),
        };
        if let Some((order_index, side)) = facility_order {
            let row = order_set
                .row_for_order_index(order_index)
                .ok_or(Error::MismatchedBinding)?;
            if order_set.rows[row].side != side {
                return Err(Error::MismatchedBinding);
            }
        }
        slice_at += SETTLEMENT_SLICE_BYTES;
    }

    let mut row = 0usize;
    while row < usize::from(order_set.row_count) {
        let projection = order_set.rows[row];
        let allocation = dealer.allocations()[row];
        let mut observed = [0u64; MAX_OUTCOMES];
        slice_at = 0;
        while slice_at < encoded_slices.len() {
            let slice = SettlementSliceV1::decode(
                &encoded_slices[slice_at..slice_at + SETTLEMENT_SLICE_BYTES],
                feed_order_count,
                outcome_count,
            )
            .map_err(|_| Error::MismatchedBinding)?;
            let matches = match projection.side {
                DealerSettlementOrderSideV3::Buy => {
                    slice.buy_kind == SettlementSliceLegKindV1::Order
                        && slice.buy_index == projection.order_index
                        && slice.sell_kind == SettlementSliceLegKindV1::CoveredDealerSell
                }
                DealerSettlementOrderSideV3::Sell => {
                    slice.buy_kind == SettlementSliceLegKindV1::CoveredDealerBuy
                        && slice.sell_kind == SettlementSliceLegKindV1::Order
                        && slice.sell_index == projection.order_index
                }
            };
            if matches {
                let outcome = usize::from(slice.outcome);
                observed[outcome] = observed[outcome]
                    .checked_add(slice.quantity)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            slice_at += SETTLEMENT_SLICE_BYTES;
        }
        let mut outcome = 0usize;
        while outcome < usize::from(outcome_count) {
            if observed[outcome]
                != projection.coefficients[outcome]
                    .checked_mul(allocation.dealer_fill_units)
                    .ok_or(Error::ArithmeticOverflow)?
            {
                return Err(Error::ConservationFailure);
            }
            outcome += 1;
        }
        row += 1;
    }
    Ok(())
}
