// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV1, DealerChildKindV1, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DELETABLE_RENT_OWNER_BYTES, FEE_BUDGET_CONTENT_DOMAIN_V1, LIVENESS_BUDGET_CONTENT_DOMAIN_V1,
};

/// Local fee-budget semantic-body magic; not a global account discriminator.
pub const FEE_BUDGET_MAGIC_V1: [u8; 8] = *b"DCFEEV01";
/// Local liveness-budget semantic-body magic; not a global account discriminator.
pub const LIVENESS_BUDGET_MAGIC_V1: [u8; 8] = *b"DCLIVV01";
/// Exact local fee-budget semantic-body version.
pub const FEE_BUDGET_VERSION_V1: u16 = 1;
/// Exact local liveness-budget semantic-body version.
pub const LIVENESS_BUDGET_VERSION_V1: u16 = 1;
/// Exact bytes in either segregated V1 budget body.
pub const DEALER_BUDGET_BYTES_V1: usize =
    HEADER_BYTES + (6 * 32) + (7 * 8) + 8 + DELETABLE_RENT_OWNER_BYTES;

/// Lifecycle of one strictly prepaid facility budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BudgetPhaseV1 {
    /// New liabilities may be reserved by an enabled successor adapter.
    Open = 0,
    /// Existing liabilities may settle but no new liability may be admitted.
    Frozen = 1,
    /// No available principal or outstanding liability remains.
    Closed = 2,
}

impl BudgetPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Frozen),
            2 => Ok(Self::Closed),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Exact prepaid fee compartment; it contains no projected future revenue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeBudgetV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Exact fee-policy identity.
    pub fee_policy_id: Id,
    /// Economic principal payer.
    pub principal_payer: Id,
    /// Sole recipient of unused economic principal.
    pub principal_refund_recipient: Id,
    /// Immutable sink for donations and non-refund surplus.
    pub neutral_sink: Id,
    /// Parent generation at which this counted budget was admitted.
    pub counted_generation: u64,
    /// Exact prepaid principal; future fees are not included.
    pub principal_atoms: u64,
    /// Unreserved principal currently available.
    pub available_atoms: u64,
    /// Exact amount reserved by outstanding liabilities.
    pub reserved_liability_atoms: u64,
    /// Cumulative amount spent against settled liabilities.
    pub spent_atoms: u64,
    /// Cumulative unused principal returned to the named recipient.
    pub refunded_atoms: u64,
    /// Cumulative donation/surplus amount routed to the neutral sink.
    pub sinked_atoms: u64,
    /// Exact number of outstanding liabilities owning `reserved_liability_atoms`.
    pub liability_count: u32,
    /// Current budget lifecycle phase.
    pub phase: BudgetPhaseV1,
    /// Exact child-account rent owner, separate from economic principal.
    pub rent: DeletableRentOwnerV1,
}

/// Exact prepaid liveness compartment; future fee revenue cannot fund it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivenessBudgetV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Exact liveness-policy identity.
    pub liveness_policy_id: Id,
    /// Economic principal payer.
    pub principal_payer: Id,
    /// Sole recipient of unused economic principal.
    pub principal_refund_recipient: Id,
    /// Immutable sink for donations and non-refund surplus.
    pub neutral_sink: Id,
    /// Parent generation at which this counted budget was admitted.
    pub counted_generation: u64,
    /// Exact prepaid principal; expected fees are not included.
    pub principal_atoms: u64,
    /// Unreserved principal currently available.
    pub available_atoms: u64,
    /// Exact amount reserved by outstanding liveness liabilities.
    pub reserved_liability_atoms: u64,
    /// Cumulative amount spent against settled liveness liabilities.
    pub spent_atoms: u64,
    /// Cumulative unused principal returned to the named recipient.
    pub refunded_atoms: u64,
    /// Cumulative donation/surplus amount routed to the neutral sink.
    pub sinked_atoms: u64,
    /// Exact number of outstanding liabilities owning `reserved_liability_atoms`.
    pub liability_count: u32,
    /// Current budget lifecycle phase.
    pub phase: BudgetPhaseV1,
    /// Exact child-account rent owner, separate from economic principal.
    pub rent: DeletableRentOwnerV1,
}

#[derive(Clone, Copy)]
struct BudgetViewV1 {
    policy_id: Id,
    facility_id: Id,
    semantic_policy_id: Id,
    principal_payer: Id,
    principal_refund_recipient: Id,
    neutral_sink: Id,
    counted_generation: u64,
    principal_atoms: u64,
    available_atoms: u64,
    reserved_liability_atoms: u64,
    spent_atoms: u64,
    refunded_atoms: u64,
    sinked_atoms: u64,
    liability_count: u32,
    phase: BudgetPhaseV1,
    rent: DeletableRentOwnerV1,
}

impl BudgetViewV1 {
    fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.semantic_policy_id,
            self.principal_payer,
            self.principal_refund_recipient,
            self.neutral_sink,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.principal_payer == self.neutral_sink
            || self.principal_refund_recipient == self.neutral_sink
            || self.neutral_sink != self.rent.neutral_sink
            || self.principal_atoms == 0
        {
            return Err(Error::InvalidParameter);
        }
        let accounted = self
            .available_atoms
            .checked_add(self.reserved_liability_atoms)
            .and_then(|value| value.checked_add(self.spent_atoms))
            .and_then(|value| value.checked_add(self.refunded_atoms))
            .and_then(|value| value.checked_add(self.sinked_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        if accounted != self.principal_atoms
            || (self.liability_count == 0) != (self.reserved_liability_atoms == 0)
        {
            return Err(Error::ConservationFailure);
        }
        match self.phase {
            BudgetPhaseV1::Open => {
                if self.refunded_atoms != 0 || self.sinked_atoms != 0 {
                    return Err(Error::InvalidPhase);
                }
            }
            BudgetPhaseV1::Frozen => {
                if self.refunded_atoms != 0 || self.sinked_atoms != 0 {
                    return Err(Error::InvalidPhase);
                }
            }
            BudgetPhaseV1::Closed => {
                if self.available_atoms != 0
                    || self.reserved_liability_atoms != 0
                    || self.liability_count != 0
                {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        self.rent.validate()
    }

    fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.semantic_policy_id);
        writer.id(self.principal_payer);
        writer.id(self.principal_refund_recipient);
        writer.id(self.neutral_sink);
        writer.u64(self.counted_generation);
        writer.u64(self.principal_atoms);
        writer.u64(self.available_atoms);
        writer.u64(self.reserved_liability_atoms);
        writer.u64(self.spent_atoms);
        writer.u64(self.refunded_atoms);
        writer.u64(self.sinked_atoms);
        writer.u32(self.liability_count);
        writer.u8(self.phase as u8);
        writer.reserved(3);
        self.rent.encode_body(writer);
    }

    fn decode_body(reader: &mut Reader<'_>) -> Result<Self> {
        let policy_id = reader.id();
        let facility_id = reader.id();
        let semantic_policy_id = reader.id();
        let principal_payer = reader.id();
        let principal_refund_recipient = reader.id();
        let neutral_sink = reader.id();
        let counted_generation = reader.u64();
        let principal_atoms = reader.u64();
        let available_atoms = reader.u64();
        let reserved_liability_atoms = reader.u64();
        let spent_atoms = reader.u64();
        let refunded_atoms = reader.u64();
        let sinked_atoms = reader.u64();
        let liability_count = reader.u32();
        let phase = BudgetPhaseV1::decode(reader.u8())?;
        reader.reserved(3)?;
        Ok(Self {
            policy_id,
            facility_id,
            semantic_policy_id,
            principal_payer,
            principal_refund_recipient,
            neutral_sink,
            counted_generation,
            principal_atoms,
            available_atoms,
            reserved_liability_atoms,
            spent_atoms,
            refunded_atoms,
            sinked_atoms,
            liability_count,
            phase,
            rent: DeletableRentOwnerV1::decode_body(reader),
        })
    }
}

impl FeeBudgetV1 {
    fn view(&self) -> BudgetViewV1 {
        BudgetViewV1 {
            policy_id: self.policy_id,
            facility_id: self.facility_id,
            semantic_policy_id: self.fee_policy_id,
            principal_payer: self.principal_payer,
            principal_refund_recipient: self.principal_refund_recipient,
            neutral_sink: self.neutral_sink,
            counted_generation: self.counted_generation,
            principal_atoms: self.principal_atoms,
            available_atoms: self.available_atoms,
            reserved_liability_atoms: self.reserved_liability_atoms,
            spent_atoms: self.spent_atoms,
            refunded_atoms: self.refunded_atoms,
            sinked_atoms: self.sinked_atoms,
            liability_count: self.liability_count,
            phase: self.phase,
            rent: self.rent,
        }
    }

    /// Validate exact prepaid conservation and liability ownership.
    pub fn validate(&self) -> Result<()> {
        self.view().validate()
    }

    /// Join the budget to the canonical dealer and fee-policy identities.
    pub fn validate_against_policy(&self, policy: &crate::DealerPolicyV1) -> Result<()> {
        self.validate()?;
        policy.validate()?;
        if self.policy_id != policy.policy_id()?
            || self.fee_policy_id != policy.fee_policy_id
            || self.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Return the exact counted-child edge owned by DealerState.
    pub const fn counted_child(&self) -> CountedDealerChildV1 {
        CountedDealerChildV1 {
            facility_id: self.facility_id,
            kind: DealerChildKindV1::FeeBudget,
            counted_generation: self.counted_generation,
        }
    }

    /// Canonical mutable fee-budget content identity.
    pub fn budget_content_id(&self) -> Result<Id> {
        self.content_id(FEE_BUDGET_CONTENT_DOMAIN_V1)
    }
}

impl LivenessBudgetV1 {
    fn view(&self) -> BudgetViewV1 {
        BudgetViewV1 {
            policy_id: self.policy_id,
            facility_id: self.facility_id,
            semantic_policy_id: self.liveness_policy_id,
            principal_payer: self.principal_payer,
            principal_refund_recipient: self.principal_refund_recipient,
            neutral_sink: self.neutral_sink,
            counted_generation: self.counted_generation,
            principal_atoms: self.principal_atoms,
            available_atoms: self.available_atoms,
            reserved_liability_atoms: self.reserved_liability_atoms,
            spent_atoms: self.spent_atoms,
            refunded_atoms: self.refunded_atoms,
            sinked_atoms: self.sinked_atoms,
            liability_count: self.liability_count,
            phase: self.phase,
            rent: self.rent,
        }
    }

    /// Validate exact prepaid conservation and liability ownership.
    pub fn validate(&self) -> Result<()> {
        self.view().validate()
    }

    /// Join the budget to the canonical dealer and liveness-policy identities.
    pub fn validate_against_policy(&self, policy: &crate::DealerPolicyV1) -> Result<()> {
        self.validate()?;
        policy.validate()?;
        if self.policy_id != policy.policy_id()?
            || self.liveness_policy_id != policy.liveness_policy_id
            || self.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Return the exact counted-child edge owned by DealerState.
    pub const fn counted_child(&self) -> CountedDealerChildV1 {
        CountedDealerChildV1 {
            facility_id: self.facility_id,
            kind: DealerChildKindV1::LivenessBudget,
            counted_generation: self.counted_generation,
        }
    }

    /// Canonical mutable liveness-budget content identity.
    pub fn budget_content_id(&self) -> Result<Id> {
        self.content_id(LIVENESS_BUDGET_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for FeeBudgetV1 {
    const ENCODED_LEN: usize = DEALER_BUDGET_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&FEE_BUDGET_MAGIC_V1, FEE_BUDGET_VERSION_V1);
        self.view().encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&FEE_BUDGET_MAGIC_V1, FEE_BUDGET_VERSION_V1)?;
        let view = BudgetViewV1::decode_body(&mut reader)?;
        reader.finish()?;
        let value = Self {
            policy_id: view.policy_id,
            facility_id: view.facility_id,
            fee_policy_id: view.semantic_policy_id,
            principal_payer: view.principal_payer,
            principal_refund_recipient: view.principal_refund_recipient,
            neutral_sink: view.neutral_sink,
            counted_generation: view.counted_generation,
            principal_atoms: view.principal_atoms,
            available_atoms: view.available_atoms,
            reserved_liability_atoms: view.reserved_liability_atoms,
            spent_atoms: view.spent_atoms,
            refunded_atoms: view.refunded_atoms,
            sinked_atoms: view.sinked_atoms,
            liability_count: view.liability_count,
            phase: view.phase,
            rent: view.rent,
        };
        value.validate()?;
        Ok(value)
    }
}

impl FixedCodec for LivenessBudgetV1 {
    const ENCODED_LEN: usize = DEALER_BUDGET_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&LIVENESS_BUDGET_MAGIC_V1, LIVENESS_BUDGET_VERSION_V1);
        self.view().encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&LIVENESS_BUDGET_MAGIC_V1, LIVENESS_BUDGET_VERSION_V1)?;
        let view = BudgetViewV1::decode_body(&mut reader)?;
        reader.finish()?;
        let value = Self {
            policy_id: view.policy_id,
            facility_id: view.facility_id,
            liveness_policy_id: view.semantic_policy_id,
            principal_payer: view.principal_payer,
            principal_refund_recipient: view.principal_refund_recipient,
            neutral_sink: view.neutral_sink,
            counted_generation: view.counted_generation,
            principal_atoms: view.principal_atoms,
            available_atoms: view.available_atoms,
            reserved_liability_atoms: view.reserved_liability_atoms,
            spent_atoms: view.spent_atoms,
            refunded_atoms: view.refunded_atoms,
            sinked_atoms: view.sinked_atoms,
            liability_count: view.liability_count,
            phase: view.phase,
            rent: view.rent,
        };
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_BUDGET_BYTES_V1 == 348);
const _: () = assert!(DEALER_BUDGET_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
