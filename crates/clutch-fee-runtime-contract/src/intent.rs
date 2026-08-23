//! Typed account identities and account-neutral instruction intents.

use core::marker::PhantomData;

use crate::selected::SelectedCompositeFeeV1;
use crate::{independent, live, Error, Id, Result};

/// A live account identity whose semantic kind cannot be interchanged in safe
/// Rust with another fee-runtime account kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct AccountIdV1<Kind> {
    identity: Id,
    kind: PhantomData<Kind>,
}

impl<Kind> AccountIdV1<Kind> {
    pub fn admit(identity: Id) -> Result<Self> {
        live(identity)?;
        Ok(Self {
            identity,
            kind: PhantomData,
        })
    }

    pub const fn identity(&self) -> Id {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FeeRecordAccountKindV1 {}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerFeeCarryAccountKindV1 {}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PayerAllocationAccountKindV1 {}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecipientAllocationAccountKindV1 {}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TreasuryLedgerAccountKindV1 {}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerSettlementAccountKindV1 {}

pub type FeeRecordAccountIdV1 = AccountIdV1<FeeRecordAccountKindV1>;
pub type OwnerFeeCarryAccountIdV1 = AccountIdV1<OwnerFeeCarryAccountKindV1>;
pub type PayerAllocationAccountIdV1 = AccountIdV1<PayerAllocationAccountKindV1>;
pub type RecipientAllocationAccountIdV1 = AccountIdV1<RecipientAllocationAccountKindV1>;
pub type TreasuryLedgerAccountIdV1 = AccountIdV1<TreasuryLedgerAccountKindV1>;
pub type OwnerSettlementAccountIdV1 = AccountIdV1<OwnerSettlementAccountKindV1>;

fn distinct(identities: &[Id]) -> Result<()> {
    independent(identities).map_err(|_| Error::IncompleteAccountGraph)
}

/// Immutable account graph for creating one selected fee record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectFeeRecordIntentV1 {
    fee_record: FeeRecordAccountIdV1,
    realm: Id,
    market: Id,
    epoch: Id,
    settlement_candidate: Id,
    batch_policy: Id,
    revenue_policy: Id,
    treasury_position: Id,
}

impl SelectFeeRecordIntentV1 {
    pub fn bind(
        selected: &SelectedCompositeFeeV1,
        fee_record: FeeRecordAccountIdV1,
    ) -> Result<Self> {
        if fee_record.identity() != selected.fee_record() {
            return Err(Error::MismatchedBinding);
        }
        distinct(&[
            selected.fee_record(),
            selected.realm(),
            selected.market(),
            selected.epoch(),
            selected.selected_candidate(),
            selected.treasury_position(),
        ])?;
        Ok(Self {
            fee_record,
            realm: selected.realm(),
            market: selected.market(),
            epoch: selected.epoch(),
            settlement_candidate: selected.selected_candidate(),
            batch_policy: selected.batch_policy(),
            revenue_policy: selected.revenue_policy(),
            treasury_position: selected.treasury_position(),
        })
    }

    pub const fn fee_record(&self) -> FeeRecordAccountIdV1 {
        self.fee_record
    }

    pub const fn realm(&self) -> Id {
        self.realm
    }

    pub const fn market(&self) -> Id {
        self.market
    }

    pub const fn epoch(&self) -> Id {
        self.epoch
    }

    /// Final `settlement_candidate_id`, never a witness representation.
    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn batch_policy(&self) -> Id {
        self.batch_policy
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn treasury_position(&self) -> Id {
        self.treasury_position
    }
}

/// Ordered identity join for one owner carry assessment and payer allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeTransitionIntentV1 {
    fee_record: FeeRecordAccountIdV1,
    carry: OwnerFeeCarryAccountIdV1,
    payer_allocation: PayerAllocationAccountIdV1,
    owner_settlement: OwnerSettlementAccountIdV1,
    settlement_candidate: Id,
    revenue_policy: Id,
    owner: Id,
}

impl OwnerFeeTransitionIntentV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        selected: &SelectedCompositeFeeV1,
        owner: Id,
        fee_record: FeeRecordAccountIdV1,
        carry: OwnerFeeCarryAccountIdV1,
        payer_allocation: PayerAllocationAccountIdV1,
        owner_settlement: OwnerSettlementAccountIdV1,
    ) -> Result<Self> {
        live(owner)?;
        if fee_record.identity() != selected.fee_record() {
            return Err(Error::MismatchedBinding);
        }
        distinct(&[
            fee_record.identity(),
            carry.identity(),
            payer_allocation.identity(),
            owner_settlement.identity(),
            selected.selected_candidate(),
            owner,
        ])?;
        Ok(Self {
            fee_record,
            carry,
            payer_allocation,
            owner_settlement,
            settlement_candidate: selected.selected_candidate(),
            revenue_policy: selected.revenue_policy(),
            owner,
        })
    }

    pub const fn fee_record(&self) -> FeeRecordAccountIdV1 {
        self.fee_record
    }

    pub const fn carry(&self) -> OwnerFeeCarryAccountIdV1 {
        self.carry
    }

    pub const fn payer_allocation(&self) -> PayerAllocationAccountIdV1 {
        self.payer_allocation
    }

    pub const fn owner_settlement(&self) -> OwnerSettlementAccountIdV1 {
        self.owner_settlement
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn owner(&self) -> Id {
        self.owner
    }
}

/// Ordered identity join for the candidate-wide recipient allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientAllocationIntentV1 {
    fee_record: FeeRecordAccountIdV1,
    recipient_allocation: RecipientAllocationAccountIdV1,
    treasury_ledger: TreasuryLedgerAccountIdV1,
    settlement_candidate: Id,
    revenue_policy: Id,
    treasury_position: Id,
}

impl RecipientAllocationIntentV1 {
    pub fn bind(
        selected: &SelectedCompositeFeeV1,
        fee_record: FeeRecordAccountIdV1,
        recipient_allocation: RecipientAllocationAccountIdV1,
        treasury_ledger: TreasuryLedgerAccountIdV1,
    ) -> Result<Self> {
        if fee_record.identity() != selected.fee_record() {
            return Err(Error::MismatchedBinding);
        }
        distinct(&[
            fee_record.identity(),
            recipient_allocation.identity(),
            treasury_ledger.identity(),
            selected.selected_candidate(),
            selected.treasury_position(),
        ])?;
        Ok(Self {
            fee_record,
            recipient_allocation,
            treasury_ledger,
            settlement_candidate: selected.selected_candidate(),
            revenue_policy: selected.revenue_policy(),
            treasury_position: selected.treasury_position(),
        })
    }

    pub const fn fee_record(&self) -> FeeRecordAccountIdV1 {
        self.fee_record
    }

    pub const fn recipient_allocation(&self) -> RecipientAllocationAccountIdV1 {
        self.recipient_allocation
    }

    pub const fn treasury_ledger(&self) -> TreasuryLedgerAccountIdV1 {
        self.treasury_ledger
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn treasury_position(&self) -> Id {
        self.treasury_position
    }
}

/// Complete mutation join for crediting the ordinary treasury Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreasuryCreditIntentV1 {
    fee_record: FeeRecordAccountIdV1,
    recipient_allocation: RecipientAllocationAccountIdV1,
    treasury_ledger: TreasuryLedgerAccountIdV1,
    owner_settlement: OwnerSettlementAccountIdV1,
    settlement_candidate: Id,
    revenue_policy: Id,
    treasury_position: Id,
}

impl TreasuryCreditIntentV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        selected: &SelectedCompositeFeeV1,
        fee_record: FeeRecordAccountIdV1,
        recipient_allocation: RecipientAllocationAccountIdV1,
        treasury_ledger: TreasuryLedgerAccountIdV1,
        owner_settlement: OwnerSettlementAccountIdV1,
    ) -> Result<Self> {
        if fee_record.identity() != selected.fee_record() {
            return Err(Error::MismatchedBinding);
        }
        distinct(&[
            fee_record.identity(),
            recipient_allocation.identity(),
            treasury_ledger.identity(),
            owner_settlement.identity(),
            selected.selected_candidate(),
            selected.treasury_position(),
        ])?;
        Ok(Self {
            fee_record,
            recipient_allocation,
            treasury_ledger,
            owner_settlement,
            settlement_candidate: selected.selected_candidate(),
            revenue_policy: selected.revenue_policy(),
            treasury_position: selected.treasury_position(),
        })
    }

    pub const fn fee_record(&self) -> FeeRecordAccountIdV1 {
        self.fee_record
    }

    pub const fn recipient_allocation(&self) -> RecipientAllocationAccountIdV1 {
        self.recipient_allocation
    }

    pub const fn treasury_ledger(&self) -> TreasuryLedgerAccountIdV1 {
        self.treasury_ledger
    }

    pub const fn owner_settlement(&self) -> OwnerSettlementAccountIdV1 {
        self.owner_settlement
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn treasury_position(&self) -> Id {
        self.treasury_position
    }
}
