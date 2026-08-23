// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed projections of the separately owned owner-netted fee runtime.
//!
//! These values own no fee rate, carry, debit, rebate, or treasury balance.
//! The future adapter must construct them from authenticated fee-runtime bodies
//! and keep that runtime as the only mutable fee truth.

use crate::{Error, Id, Result, MAX_OUTCOMES};
use sha2::{Digest, Sha256};

/// Exact selected fee-record facts required by one Dealer lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSelectedFeeRecordBindingV1 {
    /// Program admitted to own all selected fee artifacts.
    pub fee_program_id: Id,
    /// Physical selected fee-record account.
    pub fee_record_account_id: Id,
    /// Semantic identity of the authenticated selected fee-record body.
    pub fee_record_semantic_id: Id,
    /// Exact Realm.
    pub realm_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact Epoch.
    pub epoch_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Exact batch-policy digest.
    pub batch_policy_id: Id,
    /// Exact nonzero revenue-policy digest.
    pub revenue_policy_id: Id,
    /// Treasury owner admitted by that policy.
    pub treasury_owner_id: Id,
    /// Ordinary treasury Position used for collateral-atom custody.
    pub treasury_position_id: Id,
    /// Exact selected integer price scale.
    pub price_scale: u64,
    /// Selected native outcome width.
    pub outcome_count: u8,
    /// Composite dispersion fee rate in basis points.
    pub dispersion_bps: u32,
    /// Composite floor/range fee rate in basis points.
    pub floor_range_bps: u32,
    /// Exact u128 carry denominator owned by the fee record.
    pub carry_denominator: u128,
}

impl DealerSelectedFeeRecordBindingV1 {
    /// Validate the exact selected record projection without inventing a fee vault.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.fee_program_id,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.realm_id,
            self.market_instance_v2_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.batch_policy_id,
            self.revenue_policy_id,
            self.treasury_owner_id,
            self.treasury_position_id,
        ] {
            identity.validate_live()?;
        }
        if self.fee_record_account_id == self.fee_program_id
            || self.fee_record_account_id == self.treasury_position_id
            || self.treasury_owner_id == self.treasury_position_id
            || self.price_scale == 0
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || (self.dispersion_bps == 0 && self.floor_range_bps == 0)
            || self.carry_denominator == 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Frozen digest of the exact authenticated selected-record projection.
    pub fn binding_digest(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(crate::DEALER_SELECTED_FEE_BINDING_CONTENT_DOMAIN_V1);
        for identity in [
            self.fee_program_id,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.realm_id,
            self.market_instance_v2_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.batch_policy_id,
            self.revenue_policy_id,
            self.treasury_owner_id,
            self.treasury_position_id,
        ] {
            hasher.update(identity.bytes());
        }
        hasher.update(self.price_scale.to_le_bytes());
        hasher.update([self.outcome_count]);
        hasher.update(self.dispersion_bps.to_le_bytes());
        hasher.update(self.floor_range_bps.to_le_bytes());
        hasher.update(self.carry_denominator.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Candidate-wide completed owner-netted fee settlement projection.
///
/// `owner_settlement_book_id` and both allocation IDs must come from the fee
/// runtime's complete owner/intent folds. A single payer snapshot or caller-
/// asserted raw fee atom is not an admissible construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCandidateFeeSettlementBindingV1 {
    /// Selected record projection used for every owner carry and allocation.
    pub selected_fee_binding_digest: Id,
    /// Physical selected fee-record account.
    pub fee_record_account_id: Id,
    /// Exact semantic selected fee-record identity.
    pub fee_record_semantic_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Complete owner settlement-book identity.
    pub owner_settlement_book_id: Id,
    /// Canonical strict intent-ordered payer allocation identity.
    pub payer_allocation_id: Id,
    /// Complete candidate recipient allocation identity.
    pub recipient_allocation_id: Id,
    /// Fee-bearing candidate settlement identity.
    pub candidate_fee_settlement_id: Id,
    /// Typed terminal receipt proving all selected-record fee work is closed.
    pub fee_terminal_receipt_id: Id,
    /// Treasury ledger semantic identity.
    pub treasury_ledger_id: Id,
    /// Ordinary treasury Position receiving only the residual.
    pub treasury_position_id: Id,
    /// Authenticated Hoard collateral account, observed but never debited for fees.
    pub hoard_collateral_account_id: Id,
    /// Number of owners in the complete settlement book.
    pub owner_count: u32,
    /// Number of strict intent-ID-ordered payer envelopes.
    pub intent_count: u32,
    /// Exact owner-netted selected fee debited from ordinary owner Positions.
    pub selected_fee_debit_atoms: u128,
    /// Candidate-verified standing-liquidity maker rebates.
    pub maker_rebate_atoms: u64,
    /// Executor allocation; V1 requires zero because no executor identity exists.
    pub executor_atoms: u64,
    /// Residual credit to the ordinary treasury Position.
    pub treasury_atoms: u64,
    /// Hoard collateral before fee settlement.
    pub hoard_collateral_before_atoms: u64,
    /// Hoard collateral after fee settlement; must be unchanged.
    pub hoard_collateral_after_atoms: u64,
}

impl DealerCandidateFeeSettlementBindingV1 {
    /// Validate fee conservation, complete fold witnesses, and Hoard exclusion.
    pub fn validate_against(&self, selected: &DealerSelectedFeeRecordBindingV1) -> Result<()> {
        selected.validate()?;
        for identity in [
            self.selected_fee_binding_digest,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.settlement_candidate_id,
            self.owner_settlement_book_id,
            self.payer_allocation_id,
            self.recipient_allocation_id,
            self.candidate_fee_settlement_id,
            self.fee_terminal_receipt_id,
            self.treasury_ledger_id,
            self.treasury_position_id,
            self.hoard_collateral_account_id,
        ] {
            identity.validate_live()?;
        }
        let distributed = u128::from(self.maker_rebate_atoms)
            .checked_add(u128::from(self.executor_atoms))
            .and_then(|value| value.checked_add(u128::from(self.treasury_atoms)))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.selected_fee_binding_digest != selected.binding_digest()?
            || self.fee_record_account_id != selected.fee_record_account_id
            || self.fee_record_semantic_id != selected.fee_record_semantic_id
            || self.settlement_candidate_id != selected.settlement_candidate_id
            || self.treasury_position_id != selected.treasury_position_id
            || self.owner_count == 0
            || self.intent_count == 0
            || self.executor_atoms != 0
            || distributed != self.selected_fee_debit_atoms
            || self.hoard_collateral_before_atoms != self.hoard_collateral_after_atoms
            || self.hoard_collateral_account_id == self.treasury_position_id
        {
            return Err(Error::ConservationFailure);
        }
        Ok(())
    }

    /// Frozen digest of the candidate-wide completed fee evidence.
    pub fn binding_digest(&self) -> Result<Id> {
        for identity in [
            self.selected_fee_binding_digest,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.settlement_candidate_id,
            self.owner_settlement_book_id,
            self.payer_allocation_id,
            self.recipient_allocation_id,
            self.candidate_fee_settlement_id,
            self.fee_terminal_receipt_id,
            self.treasury_ledger_id,
            self.treasury_position_id,
            self.hoard_collateral_account_id,
        ] {
            identity.validate_live()?;
        }
        let mut hasher = Sha256::new();
        hasher.update(crate::DEALER_CANDIDATE_FEE_SETTLEMENT_CONTENT_DOMAIN_V1);
        for identity in [
            self.selected_fee_binding_digest,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.settlement_candidate_id,
            self.owner_settlement_book_id,
            self.payer_allocation_id,
            self.recipient_allocation_id,
            self.candidate_fee_settlement_id,
            self.fee_terminal_receipt_id,
            self.treasury_ledger_id,
            self.treasury_position_id,
            self.hoard_collateral_account_id,
        ] {
            hasher.update(identity.bytes());
        }
        hasher.update(self.owner_count.to_le_bytes());
        hasher.update(self.intent_count.to_le_bytes());
        hasher.update(self.selected_fee_debit_atoms.to_le_bytes());
        hasher.update(self.maker_rebate_atoms.to_le_bytes());
        hasher.update(self.executor_atoms.to_le_bytes());
        hasher.update(self.treasury_atoms.to_le_bytes());
        hasher.update(self.hoard_collateral_before_atoms.to_le_bytes());
        hasher.update(self.hoard_collateral_after_atoms.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Typed zero-debit abort/close projection for a selected fee record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSelectedFeeAbortBindingV1 {
    /// Exact selected fee projection.
    pub selected_fee_binding_digest: Id,
    /// Selected fee-record account.
    pub fee_record_account_id: Id,
    /// Selected fee-record semantic identity.
    pub fee_record_semantic_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Typed external fee-runtime abort/terminal receipt.
    pub fee_abort_receipt_id: Id,
    /// Selected fee-record account lamports after external atomic close.
    pub fee_record_lamports_after: u64,
}

impl DealerSelectedFeeAbortBindingV1 {
    /// Require exact record identity, zero post-close balance, and no fee debit.
    pub fn validate_against(&self, selected: &DealerSelectedFeeRecordBindingV1) -> Result<()> {
        for identity in [
            self.selected_fee_binding_digest,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.settlement_candidate_id,
            self.fee_abort_receipt_id,
        ] {
            identity.validate_live()?;
        }
        if self.selected_fee_binding_digest != selected.binding_digest()?
            || self.fee_record_account_id != selected.fee_record_account_id
            || self.fee_record_semantic_id != selected.fee_record_semantic_id
            || self.settlement_candidate_id != selected.settlement_candidate_id
            || self.fee_record_lamports_after != 0
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Frozen typed abort/terminal receipt projection digest.
    pub fn binding_digest(&self) -> Result<Id> {
        let mut hasher = Sha256::new();
        hasher.update(crate::DEALER_SELECTED_FEE_ABORT_CONTENT_DOMAIN_V1);
        for identity in [
            self.selected_fee_binding_digest,
            self.fee_record_account_id,
            self.fee_record_semantic_id,
            self.settlement_candidate_id,
            self.fee_abort_receipt_id,
        ] {
            identity.validate_live()?;
            hasher.update(identity.bytes());
        }
        hasher.update(self.fee_record_lamports_after.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}
