// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV1, DealerChildKindV1, DealerPolicyV1, DealerStateV1, DeletableRentOwnerV1,
    Error, FixedCodec, Id, Result, DEALER_LEASE_CONTENT_DOMAIN_V1, DELETABLE_RENT_OWNER_BYTES,
    MAX_OUTCOMES, MAX_SETTLEMENT_ROWS,
};

/// Local semantic-body magic; this is not a global account discriminator.
pub const DEALER_LEASE_MAGIC_V1: [u8; 8] = *b"DCLSEV01";
/// Exact local semantic-body version.
pub const DEALER_LEASE_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `DealerLeaseV1` body.
pub const DEALER_LEASE_BYTES_V1: usize =
    HEADER_BYTES + (16 * 32) + (5 * 8) + 8 + DELETABLE_RENT_OWNER_BYTES;

/// Immutable authority for exactly one `pre_generation -> pre_generation + 1` settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeaseV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Exact DealerState account identity, distinct from its content digest.
    pub dealer_state_account_id: Id,
    /// Exact pre-generation external Facility Position semantic identity.
    pub facility_position_pre_id: Id,
    /// Exact derived Lease account identity recorded by DealerState.
    pub lease_account_id: Id,
    /// Full successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact Epoch identity.
    pub epoch_id: Id,
    /// Exact final `SettlementCandidateId`; no projection or provisional candidate.
    pub settlement_candidate_id: Id,
    /// Exact upstream economic-candidate digest bound by the quote.
    pub upstream_economic_candidate_id: Id,
    /// Exact authenticated quote artifact identity.
    pub quote_id: Id,
    /// Exact checked dealer-leg verdict identity.
    pub dealer_leg_verdict_id: Id,
    /// Explicit generation-specific curve-price-certificate identity.
    pub curve_price_certificate_id: Id,
    /// Root of the immutable canonical settlement-row projection.
    pub settlement_rows_root: Id,
    /// Exact derived SettlementPot account identity.
    pub settlement_pot_id: Id,
    /// Exact derived FeeBudget account identity.
    pub fee_budget_id: Id,
    /// Exact derived LivenessBudget account identity.
    pub liveness_budget_id: Id,
    /// Facility generation consumed by this lease.
    pub pre_generation: u64,
    /// Exact successor generation; must equal `pre_generation + 1`.
    pub post_generation: u64,
    /// Slot at which the lease was created.
    pub created_slot: u64,
    /// Exclusive last slot for completing the collect phase.
    pub collect_deadline_slot: u64,
    /// Exclusive last slot for completing delivery and finalization.
    pub deliver_deadline_slot: u64,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Exact canonical settlement-row count.
    pub row_count: u16,
    /// Exact counted-child rent owner.
    pub rent: DeletableRentOwnerV1,
}

impl DealerLeaseV1 {
    /// Validate all full-width bindings, one-generation geometry, and deadlines.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_pre_id,
            self.lease_account_id,
            self.market_instance_v2_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.upstream_economic_candidate_id,
            self.quote_id,
            self.dealer_leg_verdict_id,
            self.curve_price_certificate_id,
            self.settlement_rows_root,
            self.settlement_pot_id,
            self.fee_budget_id,
            self.liveness_budget_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.row_count == 0
            || self.row_count > MAX_SETTLEMENT_ROWS
        {
            return Err(Error::InvalidParameter);
        }
        if self.post_generation
            != self
                .pre_generation
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::MismatchedBinding);
        }
        if self.created_slot == 0
            || self.created_slot >= self.collect_deadline_slot
            || self.collect_deadline_slot >= self.deliver_deadline_slot
        {
            return Err(Error::InvalidSchedule);
        }
        self.rent.validate()
    }

    /// Join the lease to exact immutable policy and pre-generation state bytes.
    pub fn validate_bindings(&self, policy: &DealerPolicyV1, state: &DealerStateV1) -> Result<()> {
        self.validate()?;
        state.validate_against_policy(policy)?;
        if self.policy_id != policy.policy_id()?
            || self.facility_id != state.facility_id
            || self.market_instance_v2_id != policy.market_instance_v2_id
            || self.facility_position_pre_id != state.facility_position_id
            || self.epoch_id != state.active_epoch_id
            || self.lease_account_id != state.active_lease_id
            || self.outcome_count != state.outcome_count
            || self.pre_generation != state.generation
            || state.children.leases != 1
            || state.children.settlement_pots != 1
            || state.children.fee_budgets != 1
            || state.children.liveness_budgets != 1
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Return the exact counted-child edge owned by DealerState.
    pub const fn counted_child(&self) -> CountedDealerChildV1 {
        CountedDealerChildV1 {
            facility_id: self.facility_id,
            kind: DealerChildKindV1::Lease,
            counted_generation: self.pre_generation,
        }
    }

    /// Canonical immutable lease content identity.
    pub fn lease_id(&self) -> Result<Id> {
        self.content_id(DEALER_LEASE_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerLeaseV1 {
    const ENCODED_LEN: usize = DEALER_LEASE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_LEASE_MAGIC_V1, DEALER_LEASE_VERSION_V1);
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.dealer_state_account_id);
        writer.id(self.facility_position_pre_id);
        writer.id(self.lease_account_id);
        writer.id(self.market_instance_v2_id);
        writer.id(self.epoch_id);
        writer.id(self.settlement_candidate_id);
        writer.id(self.upstream_economic_candidate_id);
        writer.id(self.quote_id);
        writer.id(self.dealer_leg_verdict_id);
        writer.id(self.curve_price_certificate_id);
        writer.id(self.settlement_rows_root);
        writer.id(self.settlement_pot_id);
        writer.id(self.fee_budget_id);
        writer.id(self.liveness_budget_id);
        writer.u64(self.pre_generation);
        writer.u64(self.post_generation);
        writer.u64(self.created_slot);
        writer.u64(self.collect_deadline_slot);
        writer.u64(self.deliver_deadline_slot);
        writer.u8(self.outcome_count);
        writer.u16(self.row_count);
        writer.reserved(5);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_LEASE_MAGIC_V1, DEALER_LEASE_VERSION_V1)?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            dealer_state_account_id: reader.id(),
            facility_position_pre_id: reader.id(),
            lease_account_id: reader.id(),
            market_instance_v2_id: reader.id(),
            epoch_id: reader.id(),
            settlement_candidate_id: reader.id(),
            upstream_economic_candidate_id: reader.id(),
            quote_id: reader.id(),
            dealer_leg_verdict_id: reader.id(),
            curve_price_certificate_id: reader.id(),
            settlement_rows_root: reader.id(),
            settlement_pot_id: reader.id(),
            fee_budget_id: reader.id(),
            liveness_budget_id: reader.id(),
            pre_generation: reader.u64(),
            post_generation: reader.u64(),
            created_slot: reader.u64(),
            collect_deadline_slot: reader.u64(),
            deliver_deadline_slot: reader.u64(),
            outcome_count: reader.u8(),
            row_count: reader.u16(),
            rent: {
                reader.reserved(5)?;
                DeletableRentOwnerV1::decode_body(&mut reader)
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_LEASE_BYTES_V1 == 652);
const _: () = assert!(DEALER_LEASE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
