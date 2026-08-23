// SPDX-License-Identifier: AGPL-3.0-or-later

//! One-generation lease successor for funded external liveness and owner-netted fees.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV2, DealerActionLivenessAuthorizationV1, DealerChildKindV2,
    DealerFundedDependenciesV2, DealerLivenessScheduleV1, DealerPhaseV1, DealerPolicyV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DealerSelectedFeeRecordBindingV1,
    DealerStateV2, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DEALER_LEASE_CONTENT_DOMAIN_V2, DELETABLE_RENT_OWNER_BYTES, MAX_OUTCOMES,
    MAX_SETTLEMENT_ROWS,
};

/// Local semantic-body magic for the V2 lease successor.
pub const DEALER_LEASE_MAGIC_V2: [u8; 8] = *b"DCLSEV02";
/// Exact local semantic-body version.
pub const DEALER_LEASE_VERSION_V2: u16 = 2;
/// Exact bytes in one canonical V2 lease body.
pub const DEALER_LEASE_BYTES_V2: usize =
    HEADER_BYTES + (26 * 32) + (5 * 8) + 8 + DELETABLE_RENT_OWNER_BYTES;

/// Immutable authority for one funded `g -> g+1` Dealer settlement.
///
/// There are no legacy FeeBudget/LivenessBudget IDs. The lease instead binds
/// the counted funded-dependency child, exact external-runtime quote/receipt
/// provenance, and the separately authenticated selected owner-netted record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeaseV2 {
    /// Dealer policy identity.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Exact authoritative State V2 account.
    pub dealer_state_account_id: Id,
    /// Pre-generation Facility Position semantic identity.
    pub facility_position_pre_id: Id,
    /// Current leased Facility Position semantic identity after Begin deposits.
    pub facility_position_leased_id: Id,
    /// Derived V2 Lease account recorded by State.
    pub lease_account_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact Epoch identity.
    pub epoch_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Exact upstream economic-candidate identity.
    pub upstream_economic_candidate_id: Id,
    /// Authenticated quote artifact.
    pub quote_id: Id,
    /// Checked dealer-leg verdict.
    pub dealer_leg_verdict_id: Id,
    /// Exact quantized curve-price certificate.
    pub curve_price_certificate_id: Id,
    /// Canonical settlement-row projection root.
    pub settlement_rows_root: Id,
    /// Derived V2 SettlementPot account.
    pub settlement_pot_id: Id,
    /// Immutable counted funded-dependency semantic identity.
    pub funded_dependencies_id: Id,
    /// External runtime-liveness policy identity.
    pub runtime_liveness_policy_id: Id,
    /// Digest of the authenticated external seven-account binding.
    pub runtime_liveness_binding_digest: Id,
    /// Dealer fine-grained liveness quote schedule.
    pub dealer_liveness_schedule_id: Id,
    /// Typed successful SelectLeaseAndBegin receipt account.
    pub select_begin_receipt_account_id: Id,
    /// Semantic identity of that exact successful receipt.
    pub select_begin_receipt_semantic_id: Id,
    /// Program admitted to own the typed receipt.
    pub select_begin_receipt_program_id: Id,
    /// Exact selected fee-record projection digest.
    pub selected_fee_binding_digest: Id,
    /// Selected fee-record account.
    pub selected_fee_record_account_id: Id,
    /// Selected fee-record semantic identity.
    pub selected_fee_record_semantic_id: Id,
    /// Exact owner-netted revenue-policy identity.
    pub fee_revenue_policy_id: Id,
    /// Consumed facility generation.
    pub pre_generation: u64,
    /// Exact successor generation.
    pub post_generation: u64,
    /// Creation slot.
    pub created_slot: u64,
    /// Exclusive collect deadline.
    pub collect_deadline_slot: u64,
    /// Exclusive delivery/finalize deadline.
    pub deliver_deadline_slot: u64,
    /// Native outcome width.
    pub outcome_count: u8,
    /// Canonical settlement-row count.
    pub row_count: u16,
    /// Exact counted-child rent owner.
    pub rent: DeletableRentOwnerV1,
}

impl DealerLeaseV2 {
    /// Validate full identities, generation geometry, deadlines, and rent.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_pre_id,
            self.facility_position_leased_id,
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
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.select_begin_receipt_account_id,
            self.select_begin_receipt_semantic_id,
            self.select_begin_receipt_program_id,
            self.selected_fee_binding_digest,
            self.selected_fee_record_account_id,
            self.selected_fee_record_semantic_id,
            self.fee_revenue_policy_id,
        ] {
            identity.validate_live()?;
        }
        if self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.row_count == 0
            || self.row_count > MAX_SETTLEMENT_ROWS
            || self.post_generation
                != self
                    .pre_generation
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || self.created_slot == 0
            || self.created_slot >= self.collect_deadline_slot
            || self.collect_deadline_slot >= self.deliver_deadline_slot
            || self.dealer_state_account_id == self.lease_account_id
            || self.dealer_state_account_id == self.settlement_pot_id
            || self.lease_account_id == self.settlement_pot_id
            || self.select_begin_receipt_account_id == self.dealer_state_account_id
            || self.select_begin_receipt_account_id == self.lease_account_id
            || self.select_begin_receipt_account_id == self.settlement_pot_id
            || self.selected_fee_record_account_id == self.dealer_state_account_id
            || self.selected_fee_record_account_id == self.lease_account_id
            || self.selected_fee_record_account_id == self.settlement_pot_id
            || self.facility_position_pre_id == self.facility_position_leased_id
        {
            return Err(Error::InvalidParameter);
        }
        self.rent.validate()
    }

    /// Join the lease to V2 State, its counted dependency, external receipt, and fee record.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_bindings(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        dependency: &DealerFundedDependenciesV2,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        select_begin: &DealerActionLivenessAuthorizationV1,
        selected_fee: &DealerSelectedFeeRecordBindingV1,
    ) -> Result<()> {
        self.validate()?;
        state.validate_against_policy(policy)?;
        dependency.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        select_begin.validate_against(schedule, runtime)?;
        selected_fee.validate()?;
        if !matches!(state.phase, DealerPhaseV1::Trading | DealerPhaseV1::UnwindOnly)
            || self.policy_id != policy.policy_id()?
            || self.facility_id != state.facility_id
            || self.dealer_state_account_id != dependency.bindings.asset_vault_authority_account_id
            || self.market_instance_v2_id != policy.market_instance_v2_id
            || self.facility_position_leased_id != state.facility_position_id
            || self.epoch_id != state.active_epoch_id
            || self.lease_account_id != state.active_lease_id
            || self.outcome_count != state.outcome_count
            || self.pre_generation != state.generation
            || state.children.funded_dependencies != 1
            || state.children.epoch_bindings != 1
            || state.children.leases != 1
            || state.children.settlement_pots != 1
            || self.funded_dependencies_id != state.funded_dependencies_id
            || self.funded_dependencies_id != dependency.dependency_id()?
            || self.runtime_liveness_policy_id != dependency.bindings.runtime_liveness_policy_id
            || self.runtime_liveness_policy_id != runtime.runtime_policy_id
            || self.runtime_liveness_binding_digest
                != dependency.bindings.runtime_liveness_binding_digest
            || self.runtime_liveness_binding_digest != runtime.binding_digest()?
            || self.dealer_liveness_schedule_id != dependency.bindings.liveness_schedule_id
            || self.dealer_liveness_schedule_id != schedule.schedule_id()?.untyped()
            || select_begin.action != DealerRuntimeActionV1::SelectLeaseAndBegin
            || select_begin.owner != self.dealer_state_account_id
            || select_begin.lifecycle_id != self.facility_id
            || select_begin.facility_generation != self.pre_generation
            || self.select_begin_receipt_account_id != select_begin.receipt_account_id
            || self.select_begin_receipt_semantic_id != select_begin.receipt_semantic_id
            || self.select_begin_receipt_program_id != select_begin.receipt_program_id
            || self.selected_fee_binding_digest != selected_fee.binding_digest()?
            || self.selected_fee_record_account_id != selected_fee.fee_record_account_id
            || self.selected_fee_record_semantic_id != selected_fee.fee_record_semantic_id
            || self.fee_revenue_policy_id != selected_fee.revenue_policy_id
            || self.fee_revenue_policy_id != dependency.bindings.fee_policy_id
            || self.fee_revenue_policy_id != policy.fee_policy_id
            || selected_fee.realm_id != policy.realm_id
            || selected_fee.market_instance_v2_id != self.market_instance_v2_id
            || selected_fee.epoch_id != self.epoch_id
            || selected_fee.settlement_candidate_id != self.settlement_candidate_id
            || selected_fee.outcome_count != self.outcome_count
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Counted V2 child edge.
    pub const fn counted_child(&self) -> CountedDealerChildV2 {
        CountedDealerChildV2 {
            facility_id: self.facility_id,
            kind: DealerChildKindV2::Lease,
            counted_generation: self.pre_generation,
        }
    }

    /// Canonical immutable V2 lease identity.
    pub fn lease_id(&self) -> Result<Id> {
        self.content_id(DEALER_LEASE_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for DealerLeaseV2 {
    const ENCODED_LEN: usize = DEALER_LEASE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_LEASE_MAGIC_V2, DEALER_LEASE_VERSION_V2);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_pre_id,
            self.facility_position_leased_id,
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
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.select_begin_receipt_account_id,
            self.select_begin_receipt_semantic_id,
            self.select_begin_receipt_program_id,
            self.selected_fee_binding_digest,
            self.selected_fee_record_account_id,
            self.selected_fee_record_semantic_id,
            self.fee_revenue_policy_id,
        ] {
            writer.id(identity);
        }
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
        reader.header(&DEALER_LEASE_MAGIC_V2, DEALER_LEASE_VERSION_V2)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let dealer_state_account_id = reader.id();
        let facility_position_pre_id = reader.id();
        let facility_position_leased_id = reader.id();
        let lease_account_id = reader.id();
        let market_instance_v2_id = reader.id();
        let epoch_id = reader.id();
        let settlement_candidate_id = reader.id();
        let upstream_economic_candidate_id = reader.id();
        let quote_id = reader.id();
        let dealer_leg_verdict_id = reader.id();
        let curve_price_certificate_id = reader.id();
        let settlement_rows_root = reader.id();
        let settlement_pot_id = reader.id();
        let funded_dependencies_id = reader.id();
        let runtime_liveness_policy_id = reader.id();
        let runtime_liveness_binding_digest = reader.id();
        let dealer_liveness_schedule_id = reader.id();
        let select_begin_receipt_account_id = reader.id();
        let select_begin_receipt_semantic_id = reader.id();
        let select_begin_receipt_program_id = reader.id();
        let selected_fee_binding_digest = reader.id();
        let selected_fee_record_account_id = reader.id();
        let selected_fee_record_semantic_id = reader.id();
        let fee_revenue_policy_id = reader.id();
        let value = Self {
            policy_id,
            facility_id,
            dealer_state_account_id,
            facility_position_pre_id,
            facility_position_leased_id,
            lease_account_id,
            market_instance_v2_id,
            epoch_id,
            settlement_candidate_id,
            upstream_economic_candidate_id,
            quote_id,
            dealer_leg_verdict_id,
            curve_price_certificate_id,
            settlement_rows_root,
            settlement_pot_id,
            funded_dependencies_id,
            runtime_liveness_policy_id,
            runtime_liveness_binding_digest,
            dealer_liveness_schedule_id,
            select_begin_receipt_account_id,
            select_begin_receipt_semantic_id,
            select_begin_receipt_program_id,
            selected_fee_binding_digest,
            selected_fee_record_account_id,
            selected_fee_record_semantic_id,
            fee_revenue_policy_id,
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

const _: () = assert!(DEALER_LEASE_BYTES_V2 == 972);
const _: () = assert!(DEALER_LEASE_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
