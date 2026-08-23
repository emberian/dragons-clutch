// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    validate_padding_u64, CountedDealerChildV1, DealerChildKindV1, DealerLeaseV1, DealerPhaseV1,
    DealerPolicyV1, DealerStateV1, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DELETABLE_RENT_OWNER_BYTES, MAX_ATOMS, MAX_OUTCOMES, MAX_SETTLEMENT_ROWS,
    SETTLEMENT_POT_CONTENT_DOMAIN_V1,
};

/// Local semantic-body magic; this is not a global account discriminator.
pub const SETTLEMENT_POT_MAGIC_V1: [u8; 8] = *b"DCPOTV01";
/// Exact local semantic-body version.
pub const SETTLEMENT_POT_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `SettlementPotV1` body.
pub const SETTLEMENT_POT_BYTES_V1: usize = HEADER_BYTES
    + (12 * 32)
    + 8
    + 16
    + 8
    + 32
    + (2 * MAX_OUTCOMES * 8)
    + 16
    + (2 * (8 + MAX_OUTCOMES * 8))
    + DELETABLE_RENT_OWNER_BYTES;

/// Transient selected-leg custody phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementPotPhaseV1 {
    /// Begin deposits `D_out` cash and `F_sell` Eggs, then rows collect inputs.
    Collecting = 0,
    /// All inputs are exact; rows may now deliver outputs.
    Delivering = 1,
    /// All row outputs are exact; Finalize must atomically sweep the exact
    /// `D_in`/`F_buy` residue, apply the receipt, and close the pot and lease.
    Finalizing = 2,
}

impl SettlementPotPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Collecting),
            1 => Ok(Self::Delivering),
            2 => Ok(Self::Finalizing),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Pure result of checking one requested half-open cursor slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorRequestV1 {
    /// Process `[start,end)` and publish `cursor = end` only after atomic success.
    Advance {
        /// Exact current cursor.
        start: u16,
        /// Exact successor cursor after atomic success.
        end: u16,
    },
    /// Entire slice was already completed; succeed without mutation.
    IdempotentRetry,
}

/// Classify one collect/deliver request under the strict contiguous cursor rule.
pub const fn classify_cursor_request(
    cursor: u16,
    row_count: u16,
    requested_start: u16,
    requested_end: u16,
) -> Result<CursorRequestV1> {
    if row_count == 0
        || row_count > MAX_SETTLEMENT_ROWS
        || cursor > row_count
        || requested_start > requested_end
        || requested_end > row_count
    {
        return Err(Error::InvalidParameter);
    }
    if requested_end <= cursor {
        return Ok(CursorRequestV1::IdempotentRetry);
    }
    if requested_start == cursor && requested_end > cursor {
        return Ok(CursorRequestV1::Advance {
            start: requested_start,
            end: requested_end,
        });
    }
    Err(Error::MismatchedBinding)
}

/// Custody derived from immutable aggregates plus monotone progress totals.
/// This value is never persisted as a second balance truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPotCustodyV1 {
    /// Exact transient cash atoms currently owned by the pot.
    pub cash_atoms: u64,
    /// Exact transient native Eggs currently owned by the pot.
    pub eggs: [u64; MAX_OUTCOMES],
}

/// Aggregate-only three-stage settlement record for one leased generation.
///
/// From successful Begin until atomic Finalize closes it, this body is the
/// semantic owner of transient dealer custody. The Facility Position is the
/// sole long-lived pool asset owner and does not mirror these transient
/// amounts. Per-order allocation remains owned by the authenticated batch
/// projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPotV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Exact immutable `DealerLeaseV1` content identity.
    pub lease_id: Id,
    /// Exact Epoch identity.
    pub epoch_id: Id,
    /// Exact final `SettlementCandidateId`.
    pub settlement_candidate_id: Id,
    /// Exact aggregate checked dealer-leg verdict identity.
    pub aggregate_verdict_id: Id,
    /// Explicit generation-specific curve-price-certificate identity.
    pub curve_price_certificate_id: Id,
    /// Exact pre-generation external Facility Position semantic identity.
    pub facility_position_pre_id: Id,
    /// Exact expected post-generation Facility Position semantic identity.
    pub facility_position_post_id: Id,
    /// Root of the immutable canonical settlement-row projection.
    pub settlement_rows_root: Id,
    /// Exact derived FeeBudget account identity.
    pub fee_budget_id: Id,
    /// Exact derived LivenessBudget account identity.
    pub liveness_budget_id: Id,
    /// Current transient custody phase.
    pub phase: SettlementPotPhaseV1,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Facility generation consumed by this pot.
    pub pre_generation: u64,
    /// Exact successor generation.
    pub post_generation: u64,
    /// Immutable canonical settlement-row count.
    pub row_count: u16,
    /// Next row whose user input must be collected.
    pub collect_cursor: u16,
    /// Next row whose user output must be delivered.
    pub deliver_cursor: u16,
    /// `U_in`: exact aggregate buyer cash paid into transient custody.
    pub user_cash_in_atoms: u64,
    /// `U_out`: exact aggregate seller cash delivered from transient custody.
    pub user_cash_out_atoms: u64,
    /// `D_in`: exact one-directional dealer cash swept to Facility Position.
    pub dealer_net_cash_in_atoms: u64,
    /// `D_out`: exact one-directional dealer cash deposited at Begin.
    pub dealer_net_cash_out_atoms: u64,
    /// `F_buy`: Eggs bought by the facility and swept at Finalize.
    pub facility_buy_eggs: [u64; MAX_OUTCOMES],
    /// `F_sell`: Eggs deposited at Begin and delivered to users.
    pub facility_sell_eggs: [u64; MAX_OUTCOMES],
    /// Exact separately owned FeeBudget liability bound by the verdict.
    pub fee_liability_atoms: u64,
    /// Exact separately owned LivenessBudget liability bound by the verdict.
    pub liveness_liability_atoms: u64,
    /// Monotone collected portion of `U_in`.
    pub collected_user_cash_atoms: u64,
    /// Monotone collected portion of `F_buy`.
    pub collected_user_eggs: [u64; MAX_OUTCOMES],
    /// Monotone delivered portion of `U_out`.
    pub delivered_user_cash_atoms: u64,
    /// Monotone delivered portion of `F_sell`.
    pub delivered_user_eggs: [u64; MAX_OUTCOMES],
    /// Exact counted-child rent owner.
    pub rent: DeletableRentOwnerV1,
}

impl SettlementPotV1 {
    /// Validate bindings, strict cursors, aggregate conservation, persisted
    /// phase canonicality, and pre-Finalize custody. The fourth stage is the
    /// atomic sweep/receipt/close transition and is never serialized.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.lease_id,
            self.epoch_id,
            self.settlement_candidate_id,
            self.aggregate_verdict_id,
            self.curve_price_certificate_id,
            self.facility_position_pre_id,
            self.facility_position_post_id,
            self.settlement_rows_root,
            self.fee_budget_id,
            self.liveness_budget_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
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
        if self.row_count == 0
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

        if self.fee_liability_atoms > MAX_ATOMS
            || self.liveness_liability_atoms > MAX_ATOMS
            || (self.dealer_net_cash_in_atoms != 0 && self.dealer_net_cash_out_atoms != 0)
            || self.collected_user_cash_atoms > self.user_cash_in_atoms
            || self.delivered_user_cash_atoms > self.user_cash_out_atoms
        {
            return Err(Error::ConservationFailure);
        }
        if (self.collect_cursor == 0
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
        index = 0;
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

    /// Derive the sole transient custody facts from Begin deposits and exact
    /// monotone collect/deliver totals before atomic Finalize.
    pub fn derived_custody(&self) -> Result<SettlementPotCustodyV1> {
        let cash_atoms = self
            .dealer_net_cash_out_atoms
            .checked_add(self.collected_user_cash_atoms)
            .and_then(|value| value.checked_sub(self.delivered_user_cash_atoms))
            .ok_or(Error::ConservationFailure)?;
        let mut eggs = [0u64; MAX_OUTCOMES];
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

    /// Join immutable pot facts to the exact one-generation lease.
    pub fn validate_against_lease(&self, lease: &DealerLeaseV1) -> Result<()> {
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
            || self.settlement_rows_root != lease.settlement_rows_root
            || self.fee_budget_id != lease.fee_budget_id
            || self.liveness_budget_id != lease.liveness_budget_id
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

    /// Join the exact pot aggregates to the canonical signed dealer endpoint
    /// transition. This independently recomputes `q'` and the one-directional
    /// `ceil(C(q')) - ceil(C(q))` cash receipt; a verdict digest alone is never
    /// accepted as economic authority.
    pub fn validate_transition(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV1,
        lease: &DealerLeaseV1,
    ) -> Result<[i64; MAX_OUTCOMES]> {
        self.validate_against_lease(lease)?;
        lease.validate_bindings(policy, state)?;

        let mut post = [0i64; MAX_OUTCOMES];
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

        let old_potential = policy.signed_rounded_potential(&state.net_sold)?;
        let new_potential = policy.signed_rounded_potential(&post)?;
        let difference = new_potential
            .checked_sub(old_potential)
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
                    i128::from(difference)
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

    /// Return the exact counted-child edge owned by DealerState.
    pub const fn counted_child(&self) -> CountedDealerChildV1 {
        CountedDealerChildV1 {
            facility_id: self.facility_id,
            kind: DealerChildKindV1::SettlementPot,
            counted_generation: self.pre_generation,
        }
    }

    /// Canonical mutable-pot content identity.
    pub fn pot_content_id(&self) -> Result<Id> {
        self.content_id(SETTLEMENT_POT_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for SettlementPotV1 {
    const ENCODED_LEN: usize = SETTLEMENT_POT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&SETTLEMENT_POT_MAGIC_V1, SETTLEMENT_POT_VERSION_V1);
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.lease_id);
        writer.id(self.epoch_id);
        writer.id(self.settlement_candidate_id);
        writer.id(self.aggregate_verdict_id);
        writer.id(self.curve_price_certificate_id);
        writer.id(self.facility_position_pre_id);
        writer.id(self.facility_position_post_id);
        writer.id(self.settlement_rows_root);
        writer.id(self.fee_budget_id);
        writer.id(self.liveness_budget_id);
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
        writer.u64(self.fee_liability_atoms);
        writer.u64(self.liveness_liability_atoms);
        writer.u64(self.collected_user_cash_atoms);
        write_u64_array(&mut writer, &self.collected_user_eggs);
        writer.u64(self.delivered_user_cash_atoms);
        write_u64_array(&mut writer, &self.delivered_user_eggs);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&SETTLEMENT_POT_MAGIC_V1, SETTLEMENT_POT_VERSION_V1)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let lease_id = reader.id();
        let epoch_id = reader.id();
        let settlement_candidate_id = reader.id();
        let aggregate_verdict_id = reader.id();
        let curve_price_certificate_id = reader.id();
        let facility_position_pre_id = reader.id();
        let facility_position_post_id = reader.id();
        let settlement_rows_root = reader.id();
        let fee_budget_id = reader.id();
        let liveness_budget_id = reader.id();
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
            facility_position_post_id,
            settlement_rows_root,
            fee_budget_id,
            liveness_budget_id,
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
            fee_liability_atoms: reader.u64(),
            liveness_liability_atoms: reader.u64(),
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

fn write_u64_array(writer: &mut Writer<'_>, values: &[u64; MAX_OUTCOMES]) {
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        writer.u64(values[index]);
        index += 1;
    }
}

fn read_u64_array(reader: &mut Reader<'_>) -> [u64; MAX_OUTCOMES] {
    let mut values = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        values[index] = reader.u64();
        index += 1;
    }
    values
}

const _: () = assert!(SETTLEMENT_POT_BYTES_V1 == 1_084);
const _: () = assert!(SETTLEMENT_POT_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
