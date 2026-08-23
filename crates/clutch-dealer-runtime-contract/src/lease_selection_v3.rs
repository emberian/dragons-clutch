// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed admission of General V2 selection, verified Dealer economics, and V3 price evidence.

use clutch_batch::dealer_leg_v2::VerifiedDealerLegV2;
use clutch_general_v2_contract::{
    encode_score_v2_q_first_admitted_tie_v1, FirstAdmittedTieV1, ScoreV2QComponentsV1,
    SelectedCandidateV1AccountV1, SettlementCandidateKindV1, SCORE_V2_Q_ACTIVE_RANK_BYTES,
};
use clutch_price_measure::VerifiedPriceMeasureV3;

use crate::{
    DealerEpochBindingV2, DealerGeneralEpochEvidenceV3, DealerLeaseV2, DealerPolicyV1, Error, Id,
    Result, SettlementPotV2,
};

/// In-memory capability joining all upstream selection owners required by one Lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeaseSelectionEvidenceV3 {
    selected_candidate_account_id: Id,
    selected: SelectedCandidateV1AccountV1,
    dealer_leg: VerifiedDealerLegV2,
    price: VerifiedPriceMeasureV3,
}

impl DealerLeaseSelectionEvidenceV3 {
    /// Construct only from canonical checked General, Dealer-leg, and V3 price values.
    pub fn new(
        selected_candidate_account_id: Id,
        selected: SelectedCandidateV1AccountV1,
        dealer_leg: VerifiedDealerLegV2,
        price: VerifiedPriceMeasureV3,
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
