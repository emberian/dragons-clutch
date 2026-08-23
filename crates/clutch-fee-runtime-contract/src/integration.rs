//! Settlement, redemption, and liveness integration contracts.

use crate::allocation::{PayerAllocationV1, RecipientAllocationV1};
use crate::{add, Error, Id, Result};

/// Exact atom conservation for one fee-bearing settlement group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeBearingSettlementV1 {
    pub fee_record: Id,
    pub hoard_collateral_before: u64,
    pub hoard_collateral_after: u64,
    pub consideration_debit_atoms: u64,
    pub seller_credit_atoms: u64,
    pub rounding_residue_atoms: u64,
    pub fee_debit_atoms: u64,
    pub maker_rebate_atoms: u64,
    pub executor_atoms: u64,
    pub treasury_credit_atoms: u64,
}

impl FeeBearingSettlementV1 {
    pub fn validate(
        &self,
        payer: &PayerAllocationV1,
        recipients: &RecipientAllocationV1,
    ) -> Result<()> {
        if self.fee_record != payer.fee_record()
            || self.fee_record != recipients.fee_record()
            || self.hoard_collateral_before != self.hoard_collateral_after
            || add(self.seller_credit_atoms, self.rounding_residue_atoms)?
                != self.consideration_debit_atoms
            || add(
                add(self.maker_rebate_atoms, self.executor_atoms)?,
                self.treasury_credit_atoms,
            )? != self.fee_debit_atoms
            || self.fee_debit_atoms != payer.total_debit_atoms()
            || self.fee_debit_atoms != recipients.collected_fee_atoms()
            || self.maker_rebate_atoms != recipients.maker_rebate_total()
            || self.executor_atoms != recipients.executor_atoms()
            || self.treasury_credit_atoms != recipients.treasury_atoms()
        {
            return Err(Error::ConservationFailure);
        }
        Ok(())
    }
}

/// Redemption is a principal disposition, never a new fee event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedemptionNoRakeV1 {
    pub principal_debit_atoms: u64,
    pub claimant_credit_atoms: u64,
    pub rounding_residue_atoms: u64,
    pub fee_atoms: u64,
    pub treasury_credit_atoms: u64,
}

impl RedemptionNoRakeV1 {
    pub fn validate(&self) -> Result<()> {
        if self.fee_atoms != 0 || self.treasury_credit_atoms != 0 {
            return Err(Error::RedemptionRakeForbidden);
        }
        if add(self.claimant_credit_atoms, self.rounding_residue_atoms)?
            != self.principal_debit_atoms
        {
            return Err(Error::ConservationFailure);
        }
        Ok(())
    }
}

/// Lamport capitalization for mandatory work. Only present, independently
/// prepaid liveness principal may cover the quoted requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivenessCapitalizationV1 {
    pub required_lamports: u64,
    pub present_prepaid_lamports: u64,
    /// Future collateral-atom revenue, which has neither present existence nor
    /// a lamport conversion inside this contract.
    pub projected_future_fee_atoms: u64,
    /// Collateral backing the Hoard's liabilities, never a work budget.
    pub hoard_principal_atoms: u64,
    /// Collateral owed to redeeming claimants, never a work budget.
    pub redemption_principal_atoms: u64,
}

impl LivenessCapitalizationV1 {
    pub fn validate(&self) -> Result<()> {
        if self.projected_future_fee_atoms != 0
            || self.hoard_principal_atoms != 0
            || self.redemption_principal_atoms != 0
        {
            return Err(Error::LivenessCapitalizationForbidden);
        }
        if self.present_prepaid_lamports < self.required_lamports {
            return Err(Error::LivenessCapitalizationForbidden);
        }
        Ok(())
    }
}
