//! Treasury Position accounting and source classification.

use crate::allocation::RecipientAllocationV1;
use crate::integration::CandidateFeeSettlementV1;
use crate::intent::TreasuryCreditIntentV1;
use crate::projection::SelectedOwnerFeeBookV1;
use crate::retirement::TreasuryDistributionAuthorizationV1;
use crate::selected::{SelectedCompositeFeeAccess, SelectedCompositeFeeV1};
use crate::{add, live, Error, Id, Result};

/// Economic origin of atoms presented to the revenue ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevenueSourceV1 {
    CollectedTradingFee,
    HoardPrincipal,
    RedemptionPrincipal,
    ProjectedFutureFee,
    PrepaidLivenessPrincipal,
}

/// Exact collateral-atom ledger for one ordinary treasury Position.
#[derive(Debug, Eq, PartialEq)]
pub struct TreasuryLedgerV1 {
    fee_record: Id,
    treasury_owner: Id,
    treasury_position: Id,
    credited_atoms: u64,
    withdrawn_atoms: u64,
    available_atoms: u64,
    outstanding_epochs: u64,
    closed: bool,
}

impl TreasuryLedgerV1 {
    pub fn admit<S: SelectedCompositeFeeAccess + ?Sized>(selected: &S) -> Result<Self> {
        Self::restore(selected, 0, 0, 0, 0, false)
    }

    /// Validate and restore the future account adapter's ledger words.
    pub fn restore<S: SelectedCompositeFeeAccess + ?Sized>(
        selected: &S,
        credited_atoms: u64,
        withdrawn_atoms: u64,
        available_atoms: u64,
        outstanding_epochs: u64,
        closed: bool,
    ) -> Result<Self> {
        let fee_record = selected.fee_record();
        let treasury_owner = selected.treasury_owner();
        let treasury_position = selected.treasury_position();
        for identity in [fee_record, treasury_owner, treasury_position] {
            live(identity)?;
        }
        let ledger = Self {
            fee_record,
            treasury_owner,
            treasury_position,
            credited_atoms,
            withdrawn_atoms,
            available_atoms,
            outstanding_epochs,
            closed,
        };
        ledger.validate()?;
        Ok(ledger)
    }

    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn treasury_owner(&self) -> Id {
        self.treasury_owner
    }

    pub const fn treasury_position(&self) -> Id {
        self.treasury_position
    }

    pub const fn credited_atoms(&self) -> u64 {
        self.credited_atoms
    }

    pub const fn withdrawn_atoms(&self) -> u64 {
        self.withdrawn_atoms
    }

    pub const fn available_atoms(&self) -> u64 {
        self.available_atoms
    }

    pub const fn outstanding_epochs(&self) -> u64 {
        self.outstanding_epochs
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    fn open(&self) -> Result<()> {
        if self.closed {
            Err(Error::AlreadyClosed)
        } else {
            Ok(())
        }
    }

    pub fn begin_epoch(mut self, fee_record: Id) -> Result<Self> {
        self.open()?;
        if fee_record != self.fee_record {
            return Err(Error::MismatchedBinding);
        }
        self.outstanding_epochs = add(self.outstanding_epochs, 1)?;
        Ok(self)
    }

    /// Credit the treasury residual from the complete candidate owner-fee
    /// book. No individual payer row can stand in for this aggregate join.
    pub fn credit_candidate(
        mut self,
        intent: &TreasuryCreditIntentV1,
        settlement: &CandidateFeeSettlementV1,
        book: &SelectedOwnerFeeBookV1,
        allocation: &RecipientAllocationV1,
        source: RevenueSourceV1,
    ) -> Result<Self> {
        self.open()?;
        settlement.validate(book, allocation)?;
        if allocation.fee_record() != self.fee_record
            || intent.fee_record().identity() != self.fee_record
            || intent.treasury_position() != self.treasury_position
            || intent.settlement_candidate() != book.settlement_candidate()
            || intent.revenue_policy() != book.revenue_policy()
        {
            return Err(Error::MismatchedBinding);
        }
        if source != RevenueSourceV1::CollectedTradingFee {
            return Err(Error::RevenueSourceForbidden);
        }
        if self.outstanding_epochs == 0 {
            return Err(Error::OutstandingService);
        }
        let atoms = allocation.treasury_atoms();
        self.credited_atoms = add(self.credited_atoms, atoms)?;
        self.available_atoms = add(self.available_atoms, atoms)?;
        Ok(self)
    }

    pub fn settle_epoch(mut self, fee_record: Id) -> Result<Self> {
        self.open()?;
        if fee_record != self.fee_record {
            return Err(Error::MismatchedBinding);
        }
        self.outstanding_epochs = self
            .outstanding_epochs
            .checked_sub(1)
            .ok_or(Error::OutstandingService)?;
        Ok(self)
    }

    /// Consume the one-shot terminal distribution authority after the exact
    /// treasury Position and cash-pot successors have joined the retirement
    /// accumulator. This is the callable large-book path and does not recreate
    /// the deleted owner book from caller summaries.
    pub fn credit_and_settle_retirement(
        mut self,
        authority: TreasuryDistributionAuthorizationV1,
    ) -> Result<Self> {
        self.open()?;
        if authority.fee_record != self.fee_record
            || authority.treasury_owner != self.treasury_owner
            || authority.treasury_position != self.treasury_position
            || authority.settlement_candidate.is_zero()
            || authority.revenue_policy.is_zero()
            || authority.value_disposition_receipt.is_zero()
            || self.outstanding_epochs != 1
            || self.credited_atoms != 0
            || self.withdrawn_atoms != 0
            || self.available_atoms != 0
        {
            return Err(Error::MismatchedBinding);
        }
        self.credited_atoms = authority.credited_atoms;
        self.available_atoms = authority.credited_atoms;
        self.outstanding_epochs = 0;
        self.validate()?;
        Ok(self)
    }

    pub fn withdraw(mut self, signer: Id, atoms: u64) -> Result<Self> {
        self.open()?;
        if signer != self.treasury_owner {
            return Err(Error::UnauthorizedTreasury);
        }
        self.available_atoms = self
            .available_atoms
            .checked_sub(atoms)
            .ok_or(Error::InsufficientTreasuryBalance)?;
        self.withdrawn_atoms = add(self.withdrawn_atoms, atoms)?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        for identity in [self.fee_record, self.treasury_owner, self.treasury_position] {
            live(identity)?;
        }
        if add(self.withdrawn_atoms, self.available_atoms)? != self.credited_atoms {
            return Err(Error::ConservationFailure);
        }
        if self.closed && (self.available_atoms != 0 || self.outstanding_epochs != 0) {
            return Err(Error::ConservationFailure);
        }
        Ok(())
    }

    pub fn close(mut self) -> Result<Self> {
        self.open()?;
        self.validate()?;
        if self.outstanding_epochs != 0 {
            return Err(Error::OutstandingService);
        }
        if self.available_atoms != 0 {
            return Err(Error::InsufficientTreasuryBalance);
        }
        self.closed = true;
        Ok(self)
    }
}
