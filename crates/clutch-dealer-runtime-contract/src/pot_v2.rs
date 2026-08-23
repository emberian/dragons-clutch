// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transient settlement custody successor without legacy budget truths.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    classify_cursor_request, validate_padding_u64, CountedDealerChildV2, CursorRequestV1,
    DealerActionLivenessAuthorizationV1, DealerCandidateFeeSettlementBindingV1,
    DealerChildKindV2, DealerFacilityPositionPhaseV1, DealerFacilityPositionV1,
    DealerFundedDependenciesV2, DealerLeaseV2, DealerLivenessScheduleV1, DealerPhaseV1,
    DealerPolicyV1, DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1,
    DealerSelectedFeeAbortBindingV1, DealerSelectedFeeRecordBindingV1, DealerStateV2,
    DeletableRentOwnerV1, Error, FacilityPositionBindingV1, FixedCodec, Id, Result,
    SettlementPotCustodyV1, SettlementPotPhaseV1, DELETABLE_RENT_OWNER_BYTES, MAX_OUTCOMES,
    MAX_SETTLEMENT_ROWS, SETTLEMENT_POT_CONTENT_DOMAIN_V2,
};

/// Local semantic-body magic for the owner-netted Pot successor.
pub const SETTLEMENT_POT_MAGIC_V2: [u8; 8] = *b"DCPOTV02";
/// Exact local semantic-body version.
pub const SETTLEMENT_POT_VERSION_V2: u16 = 2;
/// Exact bytes in one canonical V2 Pot body.
pub const SETTLEMENT_POT_BYTES_V2: usize = HEADER_BYTES
    + (16 * 32)
    + 8
    + 16
    + 8
    + 32
    + (2 * MAX_OUTCOMES * 8)
    + (2 * (8 + MAX_OUTCOMES * 8))
    + DELETABLE_RENT_OWNER_BYTES;

/// Aggregate-only V2 selected-leg custody owner.
///
/// Fee debit/carry/distribution and liveness balance/call facts are absent.
/// Exact immutable references join their respective external semantic owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPotV2 {
    /// Dealer policy identity.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Exact immutable V2 Lease identity.
    pub lease_id: Id,
    /// Exact Epoch identity.
    pub epoch_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Checked aggregate dealer-leg verdict.
    pub aggregate_verdict_id: Id,
    /// Exact quantized curve-price certificate.
    pub curve_price_certificate_id: Id,
    /// Pre-generation Facility Position semantic identity.
    pub facility_position_pre_id: Id,
    /// Current leased Facility Position semantic identity after Begin deposits.
    pub facility_position_leased_id: Id,
    /// Expected post-generation Facility Position semantic identity.
    pub facility_position_post_id: Id,
    /// Immutable canonical settlement-row root.
    pub settlement_rows_root: Id,
    /// Counted funded-dependency semantic identity.
    pub funded_dependencies_id: Id,
    /// Authenticated external seven-account binding digest.
    pub runtime_liveness_binding_digest: Id,
    /// Dealer fine-grained liveness quote schedule identity.
    pub dealer_liveness_schedule_id: Id,
    /// Selected owner-netted fee-record projection digest.
    pub selected_fee_binding_digest: Id,
    /// Selected fee-record account.
    pub selected_fee_record_account_id: Id,
    /// Current transient custody phase.
    pub phase: SettlementPotPhaseV1,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Consumed generation.
    pub pre_generation: u64,
    /// Exact successor generation.
    pub post_generation: u64,
    /// Canonical row count.
    pub row_count: u16,
    /// Next row whose input must be collected.
    pub collect_cursor: u16,
    /// Next row whose output must be delivered.
    pub deliver_cursor: u16,
    /// Aggregate buyer cash paid into transient custody.
    pub user_cash_in_atoms: u64,
    /// Aggregate seller cash delivered from transient custody.
    pub user_cash_out_atoms: u64,
    /// One-directional dealer cash swept to Position.
    pub dealer_net_cash_in_atoms: u64,
    /// One-directional dealer cash deposited at Begin.
    pub dealer_net_cash_out_atoms: u64,
    /// Eggs bought by the facility and swept at Finalize.
    pub facility_buy_eggs: [u64; MAX_OUTCOMES],
    /// Eggs deposited at Begin and delivered to users.
    pub facility_sell_eggs: [u64; MAX_OUTCOMES],
    /// Monotone collected portion of buyer cash.
    pub collected_user_cash_atoms: u64,
    /// Monotone collected portion of facility-buy Eggs.
    pub collected_user_eggs: [u64; MAX_OUTCOMES],
    /// Monotone delivered portion of seller cash.
    pub delivered_user_cash_atoms: u64,
    /// Monotone delivered portion of facility-sell Eggs.
    pub delivered_user_eggs: [u64; MAX_OUTCOMES],
    /// Exact counted-child rent owner.
    pub rent: DeletableRentOwnerV1,
}

impl SettlementPotV2 {
    /// Validate identities, cursor partition, conservation, and derived custody.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.lease_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.aggregate_verdict_id,
            self.curve_price_certificate_id,
            self.facility_position_pre_id,
            self.facility_position_leased_id,
            self.facility_position_post_id,
            self.settlement_rows_root,
            self.funded_dependencies_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.selected_fee_binding_digest,
            self.selected_fee_record_account_id,
        ] {
            identity.validate_live()?;
        }
        if self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.post_generation
                != self
                    .pre_generation
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || self.row_count == 0
            || self.row_count > MAX_SETTLEMENT_ROWS
            || self.collect_cursor > self.row_count
            || self.deliver_cursor > self.row_count
        {
            return Err(Error::InvalidParameter);
        }
        validate_padding_u64(self.outcome_count, &self.facility_buy_eggs)?;
        validate_padding_u64(self.outcome_count, &self.facility_sell_eggs)?;
        validate_padding_u64(self.outcome_count, &self.collected_user_eggs)?;
        validate_padding_u64(self.outcome_count, &self.delivered_user_eggs)?;
        if (self.dealer_net_cash_in_atoms != 0 && self.dealer_net_cash_out_atoms != 0)
            || self.collected_user_cash_atoms > self.user_cash_in_atoms
            || self.delivered_user_cash_atoms > self.user_cash_out_atoms
            || (self.collect_cursor == 0
                && (self.collected_user_cash_atoms != 0
                    || self.collected_user_eggs != [0; MAX_OUTCOMES]))
            || (self.deliver_cursor == 0
                && (self.delivered_user_cash_atoms != 0
                    || self.delivered_user_eggs != [0; MAX_OUTCOMES]))
        {
            return Err(Error::ConservationFailure);
        }
        let cash_left = self
            .user_cash_in_atoms
            .checked_add(self.dealer_net_cash_out_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        let cash_right = self
            .user_cash_out_atoms
            .checked_add(self.dealer_net_cash_in_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if cash_left != cash_right {
            return Err(Error::ConservationFailure);
        }
        let mut has_flow = cash_left != 0;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let buy = self.facility_buy_eggs[index];
            let sell = self.facility_sell_eggs[index];
            if (buy != 0 && sell != 0)
                || self.collected_user_eggs[index] > buy
                || self.delivered_user_eggs[index] > sell
            {
                return Err(Error::ConservationFailure);
            }
            has_flow |= buy != 0 || sell != 0;
            index += 1;
        }
        if !has_flow {
            return Err(Error::ConservationFailure);
        }
        let collection_complete = self.collect_cursor == self.row_count
            && self.collected_user_cash_atoms == self.user_cash_in_atoms
            && self.collected_user_eggs == self.facility_buy_eggs;
        let delivery_empty = self.deliver_cursor == 0
            && self.delivered_user_cash_atoms == 0
            && self.delivered_user_eggs == [0; MAX_OUTCOMES];
        let delivery_complete = self.deliver_cursor == self.row_count
            && self.delivered_user_cash_atoms == self.user_cash_out_atoms
            && self.delivered_user_eggs == self.facility_sell_eggs;
        match self.phase {
            SettlementPotPhaseV1::Collecting => {
                if collection_complete || self.collect_cursor == self.row_count || !delivery_empty {
                    return Err(Error::InvalidPhase);
                }
            }
            SettlementPotPhaseV1::Delivering => {
                if !collection_complete
                    || delivery_complete
                    || self.deliver_cursor == self.row_count
                {
                    return Err(Error::InvalidPhase);
                }
            }
            SettlementPotPhaseV1::Finalizing => {
                if !collection_complete || !delivery_complete {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        let custody = self.derived_custody()?;
        if self.phase == SettlementPotPhaseV1::Finalizing
            && (custody.cash_atoms != self.dealer_net_cash_in_atoms
                || custody.eggs != self.facility_buy_eggs)
        {
            return Err(Error::ConservationFailure);
        }
        self.rent.validate()
    }

    /// Derive current transient custody without persisting a second balance owner.
    pub fn derived_custody(&self) -> Result<SettlementPotCustodyV1> {
        let cash_atoms = self
            .dealer_net_cash_out_atoms
            .checked_add(self.collected_user_cash_atoms)
            .and_then(|value| value.checked_sub(self.delivered_user_cash_atoms))
            .ok_or(Error::ConservationFailure)?;
        let mut eggs = [0; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            eggs[index] = self.facility_sell_eggs[index]
                .checked_add(self.collected_user_eggs[index])
                .and_then(|value| value.checked_sub(self.delivered_user_eggs[index]))
                .ok_or(Error::ConservationFailure)?;
            index += 1;
        }
        Ok(SettlementPotCustodyV1 { cash_atoms, eggs })
    }

    /// Join every immutable Pot field to its exact V2 Lease.
    pub fn validate_against_lease(&self, lease: &DealerLeaseV2) -> Result<()> {
        self.validate()?;
        lease.validate()?;
        if self.policy_id != lease.policy_id
            || self.facility_id != lease.facility_id
            || self.lease_id != lease.lease_id()?
            || self.epoch_id != lease.epoch_id
            || self.settlement_candidate_id != lease.settlement_candidate_id
            || self.aggregate_verdict_id != lease.dealer_leg_verdict_id
            || self.curve_price_certificate_id != lease.curve_price_certificate_id
            || self.facility_position_pre_id != lease.facility_position_pre_id
            || self.facility_position_leased_id != lease.facility_position_leased_id
            || self.settlement_rows_root != lease.settlement_rows_root
            || self.funded_dependencies_id != lease.funded_dependencies_id
            || self.runtime_liveness_binding_digest != lease.runtime_liveness_binding_digest
            || self.dealer_liveness_schedule_id != lease.dealer_liveness_schedule_id
            || self.selected_fee_binding_digest != lease.selected_fee_binding_digest
            || self.selected_fee_record_account_id != lease.selected_fee_record_account_id
            || self.outcome_count != lease.outcome_count
            || self.row_count != lease.row_count
            || self.pre_generation != lease.pre_generation
            || self.post_generation != lease.post_generation
            || self.rent.neutral_sink != lease.rent.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Independently recompute `q'` and the one rounded potential difference.
    pub fn validate_transition(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        lease: &DealerLeaseV2,
    ) -> Result<[i64; MAX_OUTCOMES]> {
        self.validate_against_lease(lease)?;
        state.validate_against_policy(policy)?;
        if lease.policy_id != state.policy_id
            || lease.facility_id != state.facility_id
            || lease.facility_position_leased_id != state.facility_position_id
            || lease.epoch_id != state.active_epoch_id
            || lease.lease_account_id != state.active_lease_id
            || lease.pre_generation != state.generation
            || state.children.leases != 1
            || state.children.settlement_pots != 1
        {
            return Err(Error::MismatchedBinding);
        }
        let mut post = [0; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let value = i128::from(state.net_sold[index])
                .checked_add(i128::from(self.facility_sell_eggs[index]))
                .and_then(|amount| amount.checked_sub(i128::from(self.facility_buy_eggs[index])))
                .ok_or(Error::ArithmeticOverflow)?;
            post[index] = i64::try_from(value).map_err(|_| Error::ArithmeticOverflow)?;
            if state.phase == DealerPhaseV1::UnwindOnly {
                let old = state.net_sold[index];
                let new = post[index];
                let reducing = if old > 0 {
                    new >= 0 && new <= old
                } else if old < 0 {
                    new <= 0 && new >= old
                } else {
                    new == 0
                };
                if !reducing {
                    return Err(Error::InvalidPhase);
                }
            }
            index += 1;
        }
        policy.validate_net_sold(&post)?;
        let difference = policy
            .signed_rounded_potential(&post)?
            .checked_sub(policy.signed_rounded_potential(&state.net_sold)?)
            .ok_or(Error::ArithmeticOverflow)?;
        let (expected_in, expected_out) = if difference >= 0 {
            (
                u64::try_from(difference).map_err(|_| Error::ArithmeticOverflow)?,
                0,
            )
        } else {
            (
                0,
                u64::try_from(
                    difference
                        .checked_neg()
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .map_err(|_| Error::ArithmeticOverflow)?,
            )
        };
        if self.dealer_net_cash_in_atoms != expected_in
            || self.dealer_net_cash_out_atoms != expected_out
        {
            return Err(Error::ConservationFailure);
        }
        Ok(post)
    }

    /// Validate a funded Collect/Deliver/Finalize action receipt against this Pot.
    pub fn validate_liveness_authorization(
        &self,
        action: DealerRuntimeActionV1,
        lease: &DealerLeaseV2,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        authorization: &DealerActionLivenessAuthorizationV1,
    ) -> Result<()> {
        self.validate_against_lease(lease)?;
        authorization.validate_against(schedule, runtime)?;
        if !matches!(
            action,
            DealerRuntimeActionV1::Collect
                | DealerRuntimeActionV1::Deliver
                | DealerRuntimeActionV1::FinalizeSettlement
                | DealerRuntimeActionV1::AbortBeforeCollection
        ) || authorization.action != action
            || authorization.owner != lease.dealer_state_account_id
            || authorization.lifecycle_id != self.facility_id
            || authorization.facility_generation != self.pre_generation
            || schedule.schedule_id()?.untyped() != self.dealer_liveness_schedule_id
            || runtime.binding_digest()? != self.runtime_liveness_binding_digest
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Classify and authorize a strict collect slice; retries consume no receipt.
    pub fn authorize_collect(
        &self,
        requested_start: u16,
        requested_end: u16,
        current_slot: u64,
        lease: &DealerLeaseV2,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        authorization: Option<&DealerActionLivenessAuthorizationV1>,
    ) -> Result<CursorRequestV1> {
        if self.phase != SettlementPotPhaseV1::Collecting
            || (current_slot >= lease.collect_deadline_slot && self.collect_cursor == 0)
        {
            return Err(Error::InvalidPhase);
        }
        // Once any exact user input is collected, permissionless collection
        // remains enabled after the service deadline. Starting a new collection
        // after the deadline refuses, but stopping an in-flight one would strand
        // the collected inputs and both counted accounts' rent.
        let request = classify_cursor_request(
            self.collect_cursor,
            self.row_count,
            requested_start,
            requested_end,
        )?;
        match (request, authorization) {
            (CursorRequestV1::IdempotentRetry, None) => Ok(request),
            (CursorRequestV1::Advance { .. }, Some(value)) => {
                self.validate_liveness_authorization(
                    DealerRuntimeActionV1::Collect,
                    lease,
                    schedule,
                    runtime,
                    value,
                )?;
                Ok(request)
            }
            _ => Err(Error::MismatchedBinding),
        }
    }

    /// Classify and authorize a strict delivery slice; retries consume no receipt.
    pub fn authorize_deliver(
        &self,
        requested_start: u16,
        requested_end: u16,
        current_slot: u64,
        lease: &DealerLeaseV2,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        authorization: Option<&DealerActionLivenessAuthorizationV1>,
    ) -> Result<CursorRequestV1> {
        if self.phase != SettlementPotPhaseV1::Delivering {
            return Err(Error::InvalidPhase);
        }
        // Delivery stays permissionless after the service deadline: once all
        // user inputs are collected, refusing late delivery would strand both
        // user outputs and Lease/Pot rent. The deadline is still committed and
        // may drive keeper/failure policy; it is not a deletion veto.
        let _ = current_slot;
        let request = classify_cursor_request(
            self.deliver_cursor,
            self.row_count,
            requested_start,
            requested_end,
        )?;
        match (request, authorization) {
            (CursorRequestV1::IdempotentRetry, None) => Ok(request),
            (CursorRequestV1::Advance { .. }, Some(value)) => {
                self.validate_liveness_authorization(
                    DealerRuntimeActionV1::Deliver,
                    lease,
                    schedule,
                    runtime,
                    value,
                )?;
                Ok(request)
            }
            _ => Err(Error::MismatchedBinding),
        }
    }

    /// Counted V2 child edge.
    pub const fn counted_child(&self) -> CountedDealerChildV2 {
        CountedDealerChildV2 {
            facility_id: self.facility_id,
            kind: DealerChildKindV2::SettlementPot,
            counted_generation: self.pre_generation,
        }
    }

    /// Canonical mutable V2 Pot content identity.
    pub fn pot_content_id(&self) -> Result<Id> {
        self.content_id(SETTLEMENT_POT_CONTENT_DOMAIN_V2)
    }
}

/// Exact authenticated aggregate for one contiguous settlement-row slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSettlementSliceV2 {
    /// Inclusive first row.
    pub start: u16,
    /// Exclusive successor row.
    pub end: u16,
    /// Exact collateral atoms in this slice.
    pub cash_atoms: u64,
    /// Exact native Eggs in this slice.
    pub eggs: [u64; MAX_OUTCOMES],
}

/// Atomically advance one authenticated collection slice.
#[allow(clippy::too_many_arguments)]
pub fn advance_collect_v2(
    pot: &SettlementPotV2,
    lease: &DealerLeaseV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    slice: DealerSettlementSliceV2,
    current_slot: u64,
    authorization: Option<&DealerActionLivenessAuthorizationV1>,
) -> Result<SettlementPotV2> {
    validate_padding_u64(pot.outcome_count, &slice.eggs)?;
    let request = pot.authorize_collect(
        slice.start,
        slice.end,
        current_slot,
        lease,
        schedule,
        runtime,
        authorization,
    )?;
    if request == CursorRequestV1::IdempotentRetry {
        return Ok(*pot);
    }
    let mut next = *pot;
    next.collect_cursor = slice.end;
    next.collected_user_cash_atoms = next
        .collected_user_cash_atoms
        .checked_add(slice.cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut index = 0usize;
    while index < usize::from(next.outcome_count) {
        next.collected_user_eggs[index] = next.collected_user_eggs[index]
            .checked_add(slice.eggs[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if next.collect_cursor == next.row_count {
        if next.collected_user_cash_atoms != next.user_cash_in_atoms
            || next.collected_user_eggs != next.facility_buy_eggs
        {
            return Err(Error::ConservationFailure);
        }
        next.phase = SettlementPotPhaseV1::Delivering;
    }
    next.validate()?;
    Ok(next)
}

/// Atomically advance one authenticated delivery slice.
#[allow(clippy::too_many_arguments)]
pub fn advance_deliver_v2(
    pot: &SettlementPotV2,
    lease: &DealerLeaseV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    slice: DealerSettlementSliceV2,
    current_slot: u64,
    authorization: Option<&DealerActionLivenessAuthorizationV1>,
) -> Result<SettlementPotV2> {
    validate_padding_u64(pot.outcome_count, &slice.eggs)?;
    let request = pot.authorize_deliver(
        slice.start,
        slice.end,
        current_slot,
        lease,
        schedule,
        runtime,
        authorization,
    )?;
    if request == CursorRequestV1::IdempotentRetry {
        return Ok(*pot);
    }
    let mut next = *pot;
    next.deliver_cursor = slice.end;
    next.delivered_user_cash_atoms = next
        .delivered_user_cash_atoms
        .checked_add(slice.cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut index = 0usize;
    while index < usize::from(next.outcome_count) {
        next.delivered_user_eggs[index] = next.delivered_user_eggs[index]
            .checked_add(slice.eggs[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if next.deliver_cursor == next.row_count {
        if next.delivered_user_cash_atoms != next.user_cash_out_atoms
            || next.delivered_user_eggs != next.facility_sell_eggs
        {
            return Err(Error::ConservationFailure);
        }
        next.phase = SettlementPotPhaseV1::Finalizing;
    }
    next.validate()?;
    Ok(next)
}

impl FixedCodec for SettlementPotV2 {
    const ENCODED_LEN: usize = SETTLEMENT_POT_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&SETTLEMENT_POT_MAGIC_V2, SETTLEMENT_POT_VERSION_V2);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.lease_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.aggregate_verdict_id,
            self.curve_price_certificate_id,
            self.facility_position_pre_id,
            self.facility_position_leased_id,
            self.facility_position_post_id,
            self.settlement_rows_root,
            self.funded_dependencies_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.selected_fee_binding_digest,
            self.selected_fee_record_account_id,
        ] {
            writer.id(identity);
        }
        writer.u8(self.phase as u8);
        writer.u8(self.outcome_count);
        writer.reserved(6);
        writer.u64(self.pre_generation);
        writer.u64(self.post_generation);
        writer.u16(self.row_count);
        writer.u16(self.collect_cursor);
        writer.u16(self.deliver_cursor);
        writer.reserved(2);
        writer.u64(self.user_cash_in_atoms);
        writer.u64(self.user_cash_out_atoms);
        writer.u64(self.dealer_net_cash_in_atoms);
        writer.u64(self.dealer_net_cash_out_atoms);
        write_u64_array(&mut writer, &self.facility_buy_eggs);
        write_u64_array(&mut writer, &self.facility_sell_eggs);
        writer.u64(self.collected_user_cash_atoms);
        write_u64_array(&mut writer, &self.collected_user_eggs);
        writer.u64(self.delivered_user_cash_atoms);
        write_u64_array(&mut writer, &self.delivered_user_eggs);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&SETTLEMENT_POT_MAGIC_V2, SETTLEMENT_POT_VERSION_V2)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let lease_id = reader.id();
        let epoch_id = reader.id();
        let settlement_candidate_id = reader.id();
        let aggregate_verdict_id = reader.id();
        let curve_price_certificate_id = reader.id();
        let facility_position_pre_id = reader.id();
        let facility_position_leased_id = reader.id();
        let facility_position_post_id = reader.id();
        let settlement_rows_root = reader.id();
        let funded_dependencies_id = reader.id();
        let runtime_liveness_binding_digest = reader.id();
        let dealer_liveness_schedule_id = reader.id();
        let selected_fee_binding_digest = reader.id();
        let selected_fee_record_account_id = reader.id();
        let phase = SettlementPotPhaseV1::decode(reader.u8())?;
        let outcome_count = reader.u8();
        reader.reserved(6)?;
        let pre_generation = reader.u64();
        let post_generation = reader.u64();
        let row_count = reader.u16();
        let collect_cursor = reader.u16();
        let deliver_cursor = reader.u16();
        reader.reserved(2)?;
        let value = Self {
            policy_id,
            facility_id,
            lease_id,
            epoch_id,
            settlement_candidate_id,
            aggregate_verdict_id,
            curve_price_certificate_id,
            facility_position_pre_id,
            facility_position_leased_id,
            facility_position_post_id,
            settlement_rows_root,
            funded_dependencies_id,
            runtime_liveness_binding_digest,
            dealer_liveness_schedule_id,
            selected_fee_binding_digest,
            selected_fee_record_account_id,
            phase,
            outcome_count,
            pre_generation,
            post_generation,
            row_count,
            collect_cursor,
            deliver_cursor,
            user_cash_in_atoms: reader.u64(),
            user_cash_out_atoms: reader.u64(),
            dealer_net_cash_in_atoms: reader.u64(),
            dealer_net_cash_out_atoms: reader.u64(),
            facility_buy_eggs: read_u64_array(&mut reader),
            facility_sell_eggs: read_u64_array(&mut reader),
            collected_user_cash_atoms: reader.u64(),
            collected_user_eggs: read_u64_array(&mut reader),
            delivered_user_cash_atoms: reader.u64(),
            delivered_user_eggs: read_u64_array(&mut reader),
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Physical rent observations for the atomic V2 Lease/Pot close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeasePotCloseRentV2 {
    /// Lease lamports before close.
    pub lease_lamports_before: u64,
    /// Lease lamports after close; must be zero.
    pub lease_lamports_after: u64,
    /// Pot lamports before close.
    pub pot_lamports_before: u64,
    /// Pot lamports after close; must be zero.
    pub pot_lamports_after: u64,
}

/// Pure result of a successful V2 Lease/Pot close path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeasePotCloseV2 {
    /// Authoritative State after closing exactly two counted children.
    pub state_after: DealerStateV2,
    /// Closed Lease account.
    pub closed_lease_account_id: Id,
    /// Closed Pot account.
    pub closed_pot_account_id: Id,
    /// Exact rent payers in Lease/Pot order.
    pub refund_recipients: [Id; 2],
    /// Exact refundable principals in Lease/Pot order.
    pub refund_lamports: [u64; 2],
    /// Shared policy sink.
    pub neutral_sink: Id,
    /// Combined donation floors and later surplus.
    pub sink_lamports: u64,
    /// External fee-runtime terminal receipt closing this selected record.
    pub fee_terminal_receipt_id: Id,
}

/// Pure result of an atomic V2 Begin transfer and two-child admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerBeginLeaseTransitionV2 {
    /// Authoritative State pointing at the current leased Position and Lease.
    pub state_after: DealerStateV2,
    /// Exact current leased Position semantic identity.
    pub facility_position_leased_id: Id,
    /// Exact initial transient Pot custody after the Begin transfer.
    pub initial_pot_custody: SettlementPotCustodyV1,
}

/// Admit a Lease/Pot pair and atomically move exact Begin deposits from Position to Pot.
#[allow(clippy::too_many_arguments)]
pub fn begin_lease_pot_v2(
    genesis: &crate::DealerFacilityGenesisV1,
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV1,
    state: &DealerStateV2,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    select_begin: &DealerActionLivenessAuthorizationV1,
    selected_fee: &DealerSelectedFeeRecordBindingV1,
    position_before: &DealerFacilityPositionV1,
    position_leased: &DealerFacilityPositionV1,
    current_slot: u64,
) -> Result<DealerBeginLeaseTransitionV2> {
    state.validate_against_policy(policy)?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    dependency.validate_bindings(genesis, binding, policy, schedule, runtime)?;
    select_begin.validate_against(schedule, runtime)?;
    selected_fee.validate()?;
    position_before.validate_live_against(binding, policy)?;
    position_leased.validate_live_against(binding, policy)?;
    pot.validate_against_lease(lease)?;
    if !matches!(state.phase, DealerPhaseV1::Trading | DealerPhaseV1::UnwindOnly)
        || state.children.funded_dependencies != 1
        || state.facility_position_binding_id != binding_id.untyped()
        || state.children.epoch_bindings != 1
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || !state.active_lease_id.is_zero()
        || state.active_epoch_id != lease.epoch_id
        || state.facility_position_id != lease.facility_position_pre_id
        || state.facility_position_id != position_before.position_id()?
        || state.generation != lease.pre_generation
        || state.funded_dependencies_id != dependency.dependency_id()?
        || lease.funded_dependencies_id != state.funded_dependencies_id
        || lease.dealer_state_account_id != binding.dealer_state_account_id
        || lease.lease_account_id == lease.settlement_pot_id
        || lease.facility_position_leased_id != position_leased.position_id()?
        || pot.facility_position_leased_id != lease.facility_position_leased_id
        || position_before.phase != DealerFacilityPositionPhaseV1::Idle
        || position_before.generation != lease.pre_generation
        || position_leased.phase != DealerFacilityPositionPhaseV1::Leased
        || position_leased.generation != lease.pre_generation
        || select_begin.action != DealerRuntimeActionV1::SelectLeaseAndBegin
        || select_begin.owner != lease.dealer_state_account_id
        || select_begin.lifecycle_id != lease.facility_id
        || select_begin.facility_generation != lease.pre_generation
        || lease.select_begin_receipt_account_id != select_begin.receipt_account_id
        || lease.select_begin_receipt_semantic_id != select_begin.receipt_semantic_id
        || lease.select_begin_receipt_program_id != select_begin.receipt_program_id
        || lease.selected_fee_binding_digest != selected_fee.binding_digest()?
        || lease.selected_fee_record_account_id != selected_fee.fee_record_account_id
        || lease.selected_fee_record_semantic_id != selected_fee.fee_record_semantic_id
        || lease.fee_revenue_policy_id != selected_fee.revenue_policy_id
        || lease.created_slot != current_slot
        || current_slot >= lease.collect_deadline_slot
        || pot.phase != SettlementPotPhaseV1::Collecting
        || pot.collect_cursor != 0
        || pot.deliver_cursor != 0
        || pot.collected_user_cash_atoms != 0
        || pot.collected_user_eggs != [0; MAX_OUTCOMES]
        || pot.delivered_user_cash_atoms != 0
        || pot.delivered_user_eggs != [0; MAX_OUTCOMES]
    {
        return Err(Error::MismatchedBinding);
    }
    let expected_cash = position_before
        .cash_atoms
        .checked_sub(pot.dealer_net_cash_out_atoms)
        .ok_or(Error::ConservationFailure)?;
    let mut expected_eggs = position_before.eggs;
    let mut index = 0usize;
    while index < usize::from(pot.outcome_count) {
        expected_eggs[index] = expected_eggs[index]
            .checked_sub(pot.facility_sell_eggs[index])
            .ok_or(Error::ConservationFailure)?;
        index += 1;
    }
    if position_leased.cash_atoms != expected_cash || position_leased.eggs != expected_eggs {
        return Err(Error::ConservationFailure);
    }
    let initial_pot_custody = pot.derived_custody()?;
    if initial_pot_custody.cash_atoms != pot.dealer_net_cash_out_atoms
        || initial_pot_custody.eggs != pot.facility_sell_eggs
    {
        return Err(Error::ConservationFailure);
    }
    let mut state_after = *state;
    state_after.facility_position_id = lease.facility_position_leased_id;
    state_after.active_lease_id = lease.lease_account_id;
    state_after.children.leases = 1;
    state_after.children.settlement_pots = 1;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    lease.validate_bindings(
        policy,
        &state_after,
        dependency,
        schedule,
        runtime,
        select_begin,
        selected_fee,
    )?;
    pot.validate_transition(policy, &state_after, lease)?;
    Ok(DealerBeginLeaseTransitionV2 {
        state_after,
        facility_position_leased_id: lease.facility_position_leased_id,
        initial_pot_custody,
    })
}

/// Finalize exact economic settlement, owner-netted fees, and both counted children.
#[allow(clippy::too_many_arguments)]
pub fn finalize_lease_pot_v2(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV1,
    state: &DealerStateV2,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    select_begin: &DealerActionLivenessAuthorizationV1,
    finalize: &DealerActionLivenessAuthorizationV1,
    selected_fee: &DealerSelectedFeeRecordBindingV1,
    fee_settlement: &DealerCandidateFeeSettlementBindingV1,
    position_leased: &DealerFacilityPositionV1,
    position_after: &DealerFacilityPositionV1,
    rent: DealerLeasePotCloseRentV2,
) -> Result<DealerLeasePotCloseV2> {
    lease.validate_bindings(
        policy,
        state,
        dependency,
        schedule,
        runtime,
        select_begin,
        selected_fee,
    )?;
    pot.validate_liveness_authorization(
        DealerRuntimeActionV1::FinalizeSettlement,
        lease,
        schedule,
        runtime,
        finalize,
    )?;
    let post_net_sold = pot.validate_transition(policy, state, lease)?;
    fee_settlement.validate_against(selected_fee)?;
    position_leased.validate_live_against(binding, policy)?;
    position_after.validate_live_against(binding, policy)?;
    if pot.phase != SettlementPotPhaseV1::Finalizing
        || state.facility_position_binding_id != binding.binding_id()?.untyped()
        || position_leased.position_id()? != pot.facility_position_leased_id
        || position_after.position_id()? != pot.facility_position_post_id
        || position_leased.phase != DealerFacilityPositionPhaseV1::Leased
        || position_after.phase != DealerFacilityPositionPhaseV1::Idle
        || position_leased.generation != pot.pre_generation
        || position_after.generation != pot.post_generation
        || fee_settlement.selected_fee_binding_digest != pot.selected_fee_binding_digest
        || fee_settlement.fee_record_account_id != pot.selected_fee_record_account_id
        || fee_settlement.fee_record_semantic_id != lease.selected_fee_record_semantic_id
        || fee_settlement.settlement_candidate_id != pot.settlement_candidate_id
    {
        return Err(Error::MismatchedBinding);
    }
    let expected_cash = position_leased
        .cash_atoms
        .checked_add(pot.dealer_net_cash_in_atoms)
        .ok_or(Error::ConservationFailure)?;
    let mut expected_eggs = position_leased.eggs;
    let mut index = 0usize;
    while index < usize::from(pot.outcome_count) {
        expected_eggs[index] = expected_eggs[index]
            .checked_add(pot.facility_buy_eggs[index])
            .ok_or(Error::ConservationFailure)?;
        index += 1;
    }
    if position_after.cash_atoms != expected_cash || position_after.eggs != expected_eggs {
        return Err(Error::ConservationFailure);
    }
    close_lease_pot_common(
        policy,
        state,
        lease,
        pot,
        position_after.position_id()?,
        pot.post_generation,
        post_net_sold,
        fee_settlement.fee_terminal_receipt_id,
        rent,
    )
}

/// Abort before any user input, restore exact Begin deposits, and close every selected child.
#[allow(clippy::too_many_arguments)]
pub fn abort_lease_pot_before_collection_v2(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV1,
    state: &DealerStateV2,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    select_begin: &DealerActionLivenessAuthorizationV1,
    abort: &DealerActionLivenessAuthorizationV1,
    selected_fee: &DealerSelectedFeeRecordBindingV1,
    fee_abort: &DealerSelectedFeeAbortBindingV1,
    position_leased: &DealerFacilityPositionV1,
    restored_position: &DealerFacilityPositionV1,
    current_slot: u64,
    rent: DealerLeasePotCloseRentV2,
) -> Result<DealerLeasePotCloseV2> {
    lease.validate_bindings(
        policy,
        state,
        dependency,
        schedule,
        runtime,
        select_begin,
        selected_fee,
    )?;
    pot.validate_liveness_authorization(
        DealerRuntimeActionV1::AbortBeforeCollection,
        lease,
        schedule,
        runtime,
        abort,
    )?;
    fee_abort.validate_against(selected_fee)?;
    position_leased.validate_live_against(binding, policy)?;
    restored_position.validate_live_against(binding, policy)?;
    if pot.phase != SettlementPotPhaseV1::Collecting
        || state.facility_position_binding_id != binding.binding_id()?.untyped()
        || pot.collect_cursor != 0
        || pot.deliver_cursor != 0
        || pot.collected_user_cash_atoms != 0
        || pot.collected_user_eggs != [0; MAX_OUTCOMES]
        || pot.delivered_user_cash_atoms != 0
        || pot.delivered_user_eggs != [0; MAX_OUTCOMES]
        || current_slot < lease.collect_deadline_slot
        || restored_position.phase != DealerFacilityPositionPhaseV1::Idle
        || position_leased.phase != DealerFacilityPositionPhaseV1::Leased
        || position_leased.generation != pot.pre_generation
        || position_leased.position_id()? != pot.facility_position_leased_id
        || restored_position.generation != pot.post_generation
        || fee_abort.selected_fee_binding_digest != pot.selected_fee_binding_digest
        || fee_abort.fee_record_account_id != pot.selected_fee_record_account_id
        || fee_abort.fee_record_semantic_id != lease.selected_fee_record_semantic_id
        || fee_abort.settlement_candidate_id != pot.settlement_candidate_id
    {
        return Err(Error::MismatchedBinding);
    }
    let restored_cash = position_leased
        .cash_atoms
        .checked_add(pot.dealer_net_cash_out_atoms)
        .ok_or(Error::ConservationFailure)?;
    let mut restored_eggs = position_leased.eggs;
    let mut index = 0usize;
    while index < usize::from(pot.outcome_count) {
        restored_eggs[index] = restored_eggs[index]
            .checked_add(pot.facility_sell_eggs[index])
            .ok_or(Error::ConservationFailure)?;
        index += 1;
    }
    if restored_position.cash_atoms != restored_cash || restored_position.eggs != restored_eggs {
        return Err(Error::ConservationFailure);
    }
    close_lease_pot_common(
        policy,
        state,
        lease,
        pot,
        restored_position.position_id()?,
        pot.post_generation,
        state.net_sold,
        fee_abort.fee_abort_receipt_id,
        rent,
    )
}

#[allow(clippy::too_many_arguments)]
fn close_lease_pot_common(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    lease: &DealerLeaseV2,
    pot: &SettlementPotV2,
    position_after_id: Id,
    post_generation: u64,
    post_net_sold: [i64; MAX_OUTCOMES],
    fee_terminal_receipt_id: Id,
    rent: DealerLeasePotCloseRentV2,
) -> Result<DealerLeasePotCloseV2> {
    fee_terminal_receipt_id.validate_live()?;
    if rent.lease_lamports_after != 0
        || rent.pot_lamports_after != 0
        || state.active_lease_id != lease.lease_account_id
        || lease.settlement_pot_id == lease.lease_account_id
        || state.children.leases != 1
        || state.children.settlement_pots != 1
    {
        return Err(Error::InvalidChildGraph);
    }
    let lease_protected = lease
        .rent
        .refundable_principal
        .checked_add(lease.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    let pot_protected = pot
        .rent
        .refundable_principal
        .checked_add(pot.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if rent.lease_lamports_before < lease_protected
        || rent.pot_lamports_before < pot_protected
        || lease.rent.neutral_sink != pot.rent.neutral_sink
        || lease.rent.neutral_sink != policy.neutral_sink
    {
        return Err(Error::ConservationFailure);
    }
    let sink_lamports = rent
        .lease_lamports_before
        .checked_sub(lease.rent.refundable_principal)
        .and_then(|value| {
            value.checked_add(
                rent.pot_lamports_before
                    .checked_sub(pot.rent.refundable_principal)?,
            )
        })
        .ok_or(Error::ArithmeticOverflow)?;
    let mut state_after = *state;
    state_after.facility_position_id = position_after_id;
    state_after.generation = post_generation;
    state_after.net_sold = post_net_sold;
    state_after.active_lease_id = Id::ZERO;
    state_after.children.leases = 0;
    state_after.children.settlement_pots = 0;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    Ok(DealerLeasePotCloseV2 {
        state_after,
        closed_lease_account_id: lease.lease_account_id,
        closed_pot_account_id: lease.settlement_pot_id,
        refund_recipients: [lease.rent.payer, pot.rent.payer],
        refund_lamports: [
            lease.rent.refundable_principal,
            pot.rent.refundable_principal,
        ],
        neutral_sink: policy.neutral_sink,
        sink_lamports,
        fee_terminal_receipt_id,
    })
}

fn write_u64_array(writer: &mut Writer<'_>, values: &[u64; MAX_OUTCOMES]) {
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        writer.u64(values[index]);
        index += 1;
    }
}

fn read_u64_array(reader: &mut Reader<'_>) -> [u64; MAX_OUTCOMES] {
    let mut values = [0; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        values[index] = reader.u64();
        index += 1;
    }
    values
}

const _: () = assert!(SETTLEMENT_POT_BYTES_V2 == 1_196);
const _: () = assert!(SETTLEMENT_POT_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
