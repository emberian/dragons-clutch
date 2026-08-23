//! External terminal and abort receipts for one selected fee record.
//!
//! The existing owner carry PDA is the only per-owner semantic owner. It is
//! resized and transitioned once from carry state into the immutable
//! [`OwnerFeeFinalizationReceiptV1`]. The payer-allocation account is deleted
//! in that same atomic action; its complete data identity and the authenticated
//! rent/surplus disposition are committed here, so it has no independent
//! receipt lifetime. Candidate-wide terminal construction may later close the
//! temporary owner receipts after consuming them.
//!
//! These contracts never accept Hoard or redemption principal, collateral as
//! rent, projected future fees, Dealer budgets, or liveness capitalization.

use clutch_owner_settlement::{
    AuthenticatedPositionCashV1, OwnerCashRealizationPlanV1,
    OwnerSettlementAccumulatorV1, SettlementCashPotV1,
};

use crate::allocation::{FeeEnvelopeFundingV1, FeeEnvelopeV1, RecipientAllocationV1};
use crate::integration::CandidateFeeSettlementV1;
use crate::intent::{OwnerFeeTransitionIntentV1, RecipientAllocationIntentV1};
use crate::projection::{AuthenticatedSelectedOwnerFeeV1, SelectedOwnerFeeBookV1};
use crate::selected::{OwnerFeeCarryV1, SelectedCompositeFeeV1};
use crate::treasury::TreasuryLedgerV1;
use crate::{add, independent, live, Error, Id, Result, MAX_FEE_ROWS_V1};

/// Inner semantic width of the terminal owner state.
pub const OWNER_FEE_FINALIZATION_BODY_V2_BYTES: usize = 496;
/// Inner magic for the existing carry PDA's terminal successor body.
pub const OWNER_FEE_FINALIZATION_MAGIC_V2: [u8; 8] = *b"DCFEEFIN";
/// Inner version matching the same-tag outer account successor.
pub const OWNER_FEE_FINALIZATION_VERSION_V2: u16 = 2;
/// Exact canonical terminal-receipt width.
pub const FEE_TERMINAL_RECEIPT_V1_BYTES: usize = 544;
/// Exact canonical closure-manifest receipt width.
pub const FEE_CLOSURE_MANIFEST_V1_BYTES: usize = 224;
/// Terminal receipt magic.
pub const FEE_TERMINAL_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCFEEEND";
/// Closure-manifest receipt magic.
pub const FEE_CLOSURE_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCFEECLS";
/// Shared terminal receipt version.
pub const FEE_TERMINAL_RECEIPT_VERSION_V1: u16 = 1;

/// Per-owner disposition committed by the in-place carry successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OwnerFeeFinalizationOutcomeV2 {
    /// Position and pot realization collected the selected owner fee.
    Settled = 1,
    /// No Position debit occurred; assessed envelope authorization was released.
    Aborted = 2,
}

impl OwnerFeeFinalizationOutcomeV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Settled),
            2 => Ok(Self::Aborted),
            _ => Err(Error::InvalidTerminalDisposition),
        }
    }
}

/// Exact present-funding realloc and temporary payer-account close owned by the
/// existing authenticated rent ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeRentDispositionV2 {
    /// Digest of the canonical authenticated pre/post rent-ledger transition.
    /// This is data identity, not a separately allocated receipt account.
    pub data_id: Id,
    pub carry_account: Id,
    pub payer_allocation_account: Id,
    /// Existing carry principal owner and sole permitted realloc top-up payer.
    pub carry_rent_refund_owner: Id,
    /// Adapter-authenticated signer whose lamports fund the exact realloc delta.
    pub carry_top_up_payer: Id,
    pub payer_rent_refund_owner: Id,
    pub neutral_sink: Id,
    pub carry_balance_before_lamports: u64,
    pub carry_rent_principal_before_lamports: u64,
    pub carry_donation_before_lamports: u64,
    pub carry_v2_rent_minimum_lamports: u64,
    pub carry_top_up_lamports: u64,
    pub carry_balance_after_lamports: u64,
    pub carry_rent_principal_after_lamports: u64,
    pub carry_donation_after_lamports: u64,
    pub payer_balance_before_lamports: u64,
    pub payer_rent_principal_lamports: u64,
    pub payer_donation_lamports: u64,
}

impl OwnerFeeRentDispositionV2 {
    /// Validate present funding without allowing fee, donation, keeper, or
    /// liveness value to masquerade as refundable realloc principal.
    pub fn validate(&self) -> Result<()> {
        independent(&[
            self.data_id,
            self.carry_account,
            self.payer_allocation_account,
            self.neutral_sink,
        ])?;
        live(self.carry_rent_refund_owner)?;
        live(self.carry_top_up_payer)?;
        live(self.payer_rent_refund_owner)?;
        if self.carry_top_up_payer != self.carry_rent_refund_owner
            || self.carry_rent_refund_owner == self.neutral_sink
            || self.payer_rent_refund_owner == self.neutral_sink
            || self.carry_rent_refund_owner == self.carry_account
            || self.carry_rent_refund_owner == self.payer_allocation_account
            || self.carry_rent_refund_owner == self.data_id
            || self.payer_rent_refund_owner == self.carry_account
            || self.payer_rent_refund_owner == self.payer_allocation_account
            || self.payer_rent_refund_owner == self.data_id
        {
            return Err(Error::IdentityAlias);
        }
        let exact_top_up = self
            .carry_v2_rent_minimum_lamports
            .saturating_sub(self.carry_balance_before_lamports);
        if add(
            self.carry_rent_principal_before_lamports,
            self.carry_donation_before_lamports,
        )? != self.carry_balance_before_lamports
            || self.carry_top_up_lamports != exact_top_up
            || self.carry_rent_principal_after_lamports
                != add(
                    self.carry_rent_principal_before_lamports,
                    self.carry_top_up_lamports,
                )?
            || self.carry_donation_after_lamports != self.carry_donation_before_lamports
            || self.carry_balance_after_lamports
                != add(
                    self.carry_balance_before_lamports,
                    self.carry_top_up_lamports,
                )?
            || add(
                self.carry_rent_principal_after_lamports,
                self.carry_donation_after_lamports,
            )? != self.carry_balance_after_lamports
            || self.carry_balance_after_lamports < self.carry_v2_rent_minimum_lamports
            || add(
                self.payer_rent_principal_lamports,
                self.payer_donation_lamports,
            )? != self.payer_balance_before_lamports
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        Ok(())
    }
}

/// Adapter-authenticated identities which are recomputed from exact account
/// bytes or authenticated PDA/rent-ledger state, never caller summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeFinalizationBindingsV2 {
    /// Exact reviewed fee-runtime release.
    pub runtime_release: Id,
    /// Digest of the complete payer-allocation bytes deleted atomically.
    pub payer_allocation_data_id: Id,
    /// Final owner-settlement account.
    pub owner_settlement_account: Id,
    /// Digest of the exact finalized 288-byte owner-settlement body.
    pub owner_settlement_final_data_id: Id,
    /// Candidate-wide settlement cash pot account.
    pub settlement_cash_pot: Id,
    /// Existing authenticated rent-ledger transition.
    pub rent_disposition: OwnerFeeRentDispositionV2,
}

impl OwnerFeeFinalizationBindingsV2 {
    fn validate(&self) -> Result<()> {
        independent(&[
            self.runtime_release,
            self.payer_allocation_data_id,
            self.owner_settlement_account,
            self.owner_settlement_final_data_id,
            self.settlement_cash_pot,
            self.rent_disposition.data_id,
        ])?;
        self.rent_disposition.validate()
    }
}

/// Exact terminal successor body persisted at the existing owner carry PDA.
///
/// The account address, bump, and program owner remain outer adapter facts.
/// There is deliberately no copied `finalization_id`: the one-way account
/// version transition plus finalized row data ID already own replay finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeFinalizationReceiptV1 {
    runtime_release: Id,
    fee_record: Id,
    settlement_candidate: Id,
    owner: Id,
    payer_allocation_data_id: Id,
    owner_settlement_account: Id,
    owner_settlement_final_data_id: Id,
    position: Id,
    settlement_cash_pot: Id,
    rent_disposition_data_id: Id,
    outcome: OwnerFeeFinalizationOutcomeV2,
    authorized_fee_atoms: u64,
    position_debit_atoms: u64,
    position_credit_atoms: u64,
    released_cash_atoms: u64,
    position_cash_before: u64,
    position_cash_after: u64,
    position_reserved_before: u64,
    position_reserved_after: u64,
    pot_available_before: u64,
    pot_available_after: u64,
    pot_collected_fee_before: u64,
    pot_collected_fee_after: u64,
    owner_rounding_residue_price_units: u128,
    pot_rounding_before_price_units: u128,
    pot_rounding_after_price_units: u128,
    pot_finalized_owner_count_before: u16,
    pot_finalized_owner_count_after: u16,
    pot_state_before: u8,
    pot_state_after: u8,
}

impl OwnerFeeFinalizationReceiptV1 {
    /// Construct the settled successor from the pure owner-realization plan,
    /// not a caller-provided debit summary.
    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        selected: &SelectedCompositeFeeV1,
        projection: &AuthenticatedSelectedOwnerFeeV1,
        carry: &OwnerFeeCarryV1,
        bindings: OwnerFeeFinalizationBindingsV2,
        position_before: AuthenticatedPositionCashV1,
        pot_before: SettlementCashPotV1,
        plan: OwnerCashRealizationPlanV1,
    ) -> Result<Self> {
        bindings.validate()?;
        let owner = Id(projection.row().owner);
        let final_row = OwnerSettlementAccumulatorV1::decode_body(&plan.owner_settlement_body)
            .map_err(|_| Error::InvalidAccountData)?;
        if projection.fee_record() != selected.fee_record()
            || projection.settlement_candidate() != selected.selected_candidate()
            || projection.revenue_policy() != selected.revenue_policy()
            || carry.fee_record() != selected.fee_record()
            || carry.owner() != owner
            || carry.denominator() != selected.carry_denominator()
            || !carry.is_closed()
            || carry.remainder() != 0
            || carry.paid_atoms() != projection.row().fee_atoms
            || bindings.owner_settlement_account.0 != plan.owner_settlement_account
            || position_before.position != plan.position
            || position_before.owner != owner.0
            || !position_before.writable
            || final_row.expectation.owner != owner.0
            || final_row.expectation.candidate != selected.selected_candidate().0
            || final_row.expectation.selected_fee_atoms != carry.paid_atoms()
            || final_row.state != 1
            || pot_before.expectation.candidate != selected.selected_candidate().0
            || pot_before.expectation.fee_record != selected.fee_record().0
            || plan.settlement_cash_pot.expectation != pot_before.expectation
            || plan.disposition.selected_fee_atoms != carry.paid_atoms()
            || plan.disposition.position_cash_atoms != plan.position_cash_atoms
            || plan.disposition.position_reserved_cash_atoms != plan.position_reserved_cash_atoms
            || bindings.rent_disposition.carry_account != projection.carry_account()
            || bindings.rent_disposition.payer_allocation_account
                != projection.payer_allocation_account()
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            runtime_release: bindings.runtime_release,
            fee_record: selected.fee_record(),
            settlement_candidate: selected.selected_candidate(),
            owner,
            payer_allocation_data_id: bindings.payer_allocation_data_id,
            owner_settlement_account: bindings.owner_settlement_account,
            owner_settlement_final_data_id: bindings.owner_settlement_final_data_id,
            position: Id(plan.position),
            settlement_cash_pot: bindings.settlement_cash_pot,
            rent_disposition_data_id: bindings.rent_disposition.data_id,
            outcome: OwnerFeeFinalizationOutcomeV2::Settled,
            authorized_fee_atoms: carry.paid_atoms(),
            position_debit_atoms: plan.disposition.debit_atoms,
            position_credit_atoms: plan.disposition.credit_atoms,
            released_cash_atoms: plan.disposition.released_cash_atoms,
            position_cash_before: position_before.cash_atoms,
            position_cash_after: plan.position_cash_atoms,
            position_reserved_before: position_before.reserved_cash_atoms,
            position_reserved_after: plan.position_reserved_cash_atoms,
            pot_available_before: pot_before.available_consideration_atoms,
            pot_available_after: plan.settlement_cash_pot.available_consideration_atoms,
            pot_collected_fee_before: pot_before.collected_fee_atoms,
            pot_collected_fee_after: plan.settlement_cash_pot.collected_fee_atoms,
            owner_rounding_residue_price_units: plan.disposition.residue_price_units,
            pot_rounding_before_price_units: pot_before.realized_rounding_price_units,
            pot_rounding_after_price_units: plan
                .settlement_cash_pot
                .realized_rounding_price_units,
            pot_finalized_owner_count_before: pot_before.finalized_owner_count,
            pot_finalized_owner_count_after: plan.settlement_cash_pot.finalized_owner_count,
            pot_state_before: pot_before.state,
            pot_state_after: plan.settlement_cash_pot.state,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct the abort successor. Envelope authorization is released; no
    /// collateral debit/credit or pot mutation may occur through this route.
    #[allow(clippy::too_many_arguments)]
    pub fn abort(
        selected: &SelectedCompositeFeeV1,
        transition: &OwnerFeeTransitionIntentV1,
        carry: &OwnerFeeCarryV1,
        bindings: OwnerFeeFinalizationBindingsV2,
        position: AuthenticatedPositionCashV1,
        pot: SettlementCashPotV1,
        envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
        envelope_len: u8,
    ) -> Result<Self> {
        bindings.validate()?;
        let owner = carry.owner();
        if transition.fee_record().identity() != selected.fee_record()
            || transition.settlement_candidate() != selected.selected_candidate()
            || transition.revenue_policy() != selected.revenue_policy()
            || transition.owner() != owner
            || transition.carry().identity() != bindings.rent_disposition.carry_account
            || transition.payer_allocation().identity()
                != bindings.rent_disposition.payer_allocation_account
            || transition.owner_settlement().identity() != bindings.owner_settlement_account
            || carry.fee_record() != selected.fee_record()
            || carry.denominator() != selected.carry_denominator()
            || !carry.is_closed()
            || carry.remainder() != 0
            || position.owner != owner.0
            || !position.writable
            || pot.expectation.candidate != selected.selected_candidate().0
            || pot.expectation.fee_record != selected.fee_record().0
            || envelope_len == 0
            || usize::from(envelope_len) > MAX_FEE_ROWS_V1
        {
            return Err(Error::MismatchedBinding);
        }
        let mut authorized = 0u64;
        let mut prior = None;
        let mut index = 0usize;
        while index < usize::from(envelope_len) {
            let envelope = envelopes[index];
            if envelope.owner != owner || envelope.debited_atoms > envelope.max_fee_atoms {
                return Err(Error::MismatchedBinding);
            }
            live(envelope.intent)?;
            if let Some(intent) = prior {
                if envelope.intent <= intent {
                    return Err(Error::NonCanonicalOrder);
                }
            }
            if envelope.funding == FeeEnvelopeFundingV1::NoCashReservation
                && (envelope.max_fee_atoms != 0 || envelope.debited_atoms != 0)
            {
                return Err(Error::SellerFeeForbidden);
            }
            authorized = add(authorized, envelope.debited_atoms)?;
            prior = Some(envelope.intent);
            index += 1;
        }
        if authorized != carry.paid_atoms() {
            return Err(Error::ConservationFailure);
        }
        let value = Self {
            runtime_release: bindings.runtime_release,
            fee_record: selected.fee_record(),
            settlement_candidate: selected.selected_candidate(),
            owner,
            payer_allocation_data_id: bindings.payer_allocation_data_id,
            owner_settlement_account: bindings.owner_settlement_account,
            owner_settlement_final_data_id: bindings.owner_settlement_final_data_id,
            position: Id(position.position),
            settlement_cash_pot: bindings.settlement_cash_pot,
            rent_disposition_data_id: bindings.rent_disposition.data_id,
            outcome: OwnerFeeFinalizationOutcomeV2::Aborted,
            authorized_fee_atoms: authorized,
            position_debit_atoms: 0,
            position_credit_atoms: 0,
            released_cash_atoms: 0,
            position_cash_before: position.cash_atoms,
            position_cash_after: position.cash_atoms,
            position_reserved_before: position.reserved_cash_atoms,
            position_reserved_after: position.reserved_cash_atoms,
            pot_available_before: pot.available_consideration_atoms,
            pot_available_after: pot.available_consideration_atoms,
            pot_collected_fee_before: pot.collected_fee_atoms,
            pot_collected_fee_after: pot.collected_fee_atoms,
            owner_rounding_residue_price_units: 0,
            pot_rounding_before_price_units: pot.realized_rounding_price_units,
            pot_rounding_after_price_units: pot.realized_rounding_price_units,
            pot_finalized_owner_count_before: pot.finalized_owner_count,
            pot_finalized_owner_count_after: pot.finalized_owner_count,
            pot_state_before: pot.state,
            pot_state_after: pot.state,
        };
        value.validate()?;
        Ok(value)
    }

    pub const fn runtime_release(&self) -> Id {
        self.runtime_release
    }
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }
    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }
    pub const fn owner(&self) -> Id {
        self.owner
    }
    pub const fn owner_settlement_account(&self) -> Id {
        self.owner_settlement_account
    }
    pub const fn owner_settlement_final_data_id(&self) -> Id {
        self.owner_settlement_final_data_id
    }
    pub const fn payer_allocation_data_id(&self) -> Id {
        self.payer_allocation_data_id
    }
    pub const fn position(&self) -> Id {
        self.position
    }
    pub const fn settlement_cash_pot(&self) -> Id {
        self.settlement_cash_pot
    }
    pub const fn rent_disposition_data_id(&self) -> Id {
        self.rent_disposition_data_id
    }
    pub const fn outcome(&self) -> OwnerFeeFinalizationOutcomeV2 {
        self.outcome
    }
    pub const fn authorized_fee_atoms(&self) -> u64 {
        self.authorized_fee_atoms
    }

    /// Encode the exact 496-byte inner successor body.
    pub fn encode(&self) -> Result<[u8; OWNER_FEE_FINALIZATION_BODY_V2_BYTES]> {
        self.validate()?;
        let mut output = [0u8; OWNER_FEE_FINALIZATION_BODY_V2_BYTES];
        let mut at = 0usize;
        put(&mut output, &mut at, &OWNER_FEE_FINALIZATION_MAGIC_V2)?;
        put(&mut output, &mut at, &OWNER_FEE_FINALIZATION_VERSION_V2.to_le_bytes())?;
        put(&mut output, &mut at, &[self.outcome as u8, 0])?;
        put(&mut output, &mut at, &[0; 4])?;
        for identity in [
            self.runtime_release,
            self.fee_record,
            self.settlement_candidate,
            self.owner,
            self.payer_allocation_data_id,
            self.owner_settlement_account,
            self.owner_settlement_final_data_id,
            self.position,
            self.settlement_cash_pot,
            self.rent_disposition_data_id,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        for amount in [
            self.authorized_fee_atoms,
            self.position_debit_atoms,
            self.position_credit_atoms,
            self.released_cash_atoms,
            self.position_cash_before,
            self.position_cash_after,
            self.position_reserved_before,
            self.position_reserved_after,
            self.pot_available_before,
            self.pot_available_after,
            self.pot_collected_fee_before,
            self.pot_collected_fee_after,
        ] {
            put(&mut output, &mut at, &amount.to_le_bytes())?;
        }
        for amount in [
            self.owner_rounding_residue_price_units,
            self.pot_rounding_before_price_units,
            self.pot_rounding_after_price_units,
        ] {
            put(&mut output, &mut at, &amount.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut at,
            &self.pot_finalized_owner_count_before.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut at,
            &self.pot_finalized_owner_count_after.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut at,
            &[self.pot_state_before, self.pot_state_after],
        )?;
        put(&mut output, &mut at, &[0; 10])?;
        if at != output.len() {
            return Err(Error::InvalidWidth);
        }
        Ok(output)
    }

    /// Decode only the exact terminal successor body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != OWNER_FEE_FINALIZATION_BODY_V2_BYTES
            || input[..8] != OWNER_FEE_FINALIZATION_MAGIC_V2
            || u16::from_le_bytes([input[8], input[9]]) != OWNER_FEE_FINALIZATION_VERSION_V2
            || input[11] != 0
            || input[12..16] != [0; 4]
            || input[486..496] != [0; 10]
        {
            return Err(Error::InvalidAccountData);
        }
        let outcome = OwnerFeeFinalizationOutcomeV2::decode(input[10])?;
        let mut at = 16usize;
        let runtime_release = take_id(input, &mut at)?;
        let fee_record = take_id(input, &mut at)?;
        let settlement_candidate = take_id(input, &mut at)?;
        let owner = take_id(input, &mut at)?;
        let payer_allocation_data_id = take_id(input, &mut at)?;
        let owner_settlement_account = take_id(input, &mut at)?;
        let owner_settlement_final_data_id = take_id(input, &mut at)?;
        let position = take_id(input, &mut at)?;
        let settlement_cash_pot = take_id(input, &mut at)?;
        let rent_disposition_data_id = take_id(input, &mut at)?;
        let authorized_fee_atoms = take_u64(input, &mut at)?;
        let position_debit_atoms = take_u64(input, &mut at)?;
        let position_credit_atoms = take_u64(input, &mut at)?;
        let released_cash_atoms = take_u64(input, &mut at)?;
        let position_cash_before = take_u64(input, &mut at)?;
        let position_cash_after = take_u64(input, &mut at)?;
        let position_reserved_before = take_u64(input, &mut at)?;
        let position_reserved_after = take_u64(input, &mut at)?;
        let pot_available_before = take_u64(input, &mut at)?;
        let pot_available_after = take_u64(input, &mut at)?;
        let pot_collected_fee_before = take_u64(input, &mut at)?;
        let pot_collected_fee_after = take_u64(input, &mut at)?;
        let owner_rounding_residue_price_units = take_u128(input, &mut at)?;
        let pot_rounding_before_price_units = take_u128(input, &mut at)?;
        let pot_rounding_after_price_units = take_u128(input, &mut at)?;
        let pot_finalized_owner_count_before = take_u16(input, &mut at)?;
        let pot_finalized_owner_count_after = take_u16(input, &mut at)?;
        let pot_state_before = take_u8(input, &mut at)?;
        let pot_state_after = take_u8(input, &mut at)?;
        at += 10;
        if at != input.len() {
            return Err(Error::InvalidWidth);
        }
        let value = Self {
            runtime_release,
            fee_record,
            settlement_candidate,
            owner,
            payer_allocation_data_id,
            owner_settlement_account,
            owner_settlement_final_data_id,
            position,
            settlement_cash_pot,
            rent_disposition_data_id,
            outcome,
            authorized_fee_atoms,
            position_debit_atoms,
            position_credit_atoms,
            released_cash_atoms,
            position_cash_before,
            position_cash_after,
            position_reserved_before,
            position_reserved_after,
            pot_available_before,
            pot_available_after,
            pot_collected_fee_before,
            pot_collected_fee_after,
            owner_rounding_residue_price_units,
            pot_rounding_before_price_units,
            pot_rounding_after_price_units,
            pot_finalized_owner_count_before,
            pot_finalized_owner_count_after,
            pot_state_before,
            pot_state_after,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        independent(&[
            self.runtime_release,
            self.fee_record,
            self.settlement_candidate,
            self.owner,
            self.payer_allocation_data_id,
            self.owner_settlement_account,
            self.owner_settlement_final_data_id,
            self.position,
            self.settlement_cash_pot,
            self.rent_disposition_data_id,
        ])?;
        match self.outcome {
            OwnerFeeFinalizationOutcomeV2::Settled => {
                let consideration = self
                    .position_debit_atoms
                    .checked_sub(self.authorized_fee_atoms)
                    .ok_or(Error::ConservationFailure)?;
                let expected_cash = self
                    .position_cash_before
                    .checked_sub(self.position_debit_atoms)
                    .and_then(|value| value.checked_add(self.position_credit_atoms))
                    .ok_or(Error::ConservationFailure)?;
                let consumed_reservation = self
                    .position_debit_atoms
                    .checked_add(self.released_cash_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
                let expected_reserved = self
                    .position_reserved_before
                    .checked_sub(consumed_reservation)
                    .ok_or(Error::ConservationFailure)?;
                let expected_available = self
                    .pot_available_before
                    .checked_add(consideration)
                    .and_then(|value| value.checked_sub(self.position_credit_atoms))
                    .ok_or(Error::ConservationFailure)?;
                if self.position_cash_after != expected_cash
                    || self.position_reserved_after != expected_reserved
                    || self.position_reserved_after > self.position_cash_after
                    || self.pot_available_after != expected_available
                    || self.pot_collected_fee_after
                        != add(self.pot_collected_fee_before, self.authorized_fee_atoms)?
                    || self.pot_rounding_after_price_units
                        != self
                            .pot_rounding_before_price_units
                            .checked_add(self.owner_rounding_residue_price_units)
                            .ok_or(Error::ArithmeticOverflow)?
                    || self.pot_finalized_owner_count_after
                        != self
                            .pot_finalized_owner_count_before
                            .checked_add(1)
                            .ok_or(Error::ArithmeticOverflow)?
                    || self.pot_state_before != 0
                    || self.pot_state_after > 1
                {
                    return Err(Error::ConservationFailure);
                }
            }
            OwnerFeeFinalizationOutcomeV2::Aborted => {
                if self.position_debit_atoms != 0
                    || self.position_credit_atoms != 0
                    || self.released_cash_atoms != 0
                    || self.position_cash_before != self.position_cash_after
                    || self.position_reserved_before != self.position_reserved_after
                    || self.pot_available_before != self.pot_available_after
                    || self.pot_collected_fee_before != self.pot_collected_fee_after
                    || self.owner_rounding_residue_price_units != 0
                    || self.pot_rounding_before_price_units != self.pot_rounding_after_price_units
                    || self.pot_finalized_owner_count_before
                        != self.pot_finalized_owner_count_after
                    || self.pot_state_before != self.pot_state_after
                {
                    return Err(Error::InvalidTerminalDisposition);
                }
            }
        }
        Ok(())
    }
}

/// Adapter-authenticated existing carry-PDA view after its in-place terminal
/// transition. This type does not derive its own PDA or authenticate bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedOwnerFeeFinalizationV1 {
    pub carry_account: Id,
    pub receipt: OwnerFeeFinalizationReceiptV1,
}

impl AuthenticatedOwnerFeeFinalizationV1 {
    pub const EMPTY: Self = Self {
        carry_account: Id([0; 32]),
        receipt: EMPTY_OWNER_FINALIZATION,
    };

    /// Typed General dependency after the adapter authenticates the unchanged
    /// carry PDA and exact v2 body bytes.
    pub fn project_general(&self) -> Result<GeneralOwnerFeeFinalizationProjectionV2> {
        live(self.carry_account)?;
        if self.carry_account == self.receipt.fee_record
            || self.carry_account == self.receipt.payer_allocation_data_id
            || self.carry_account == self.receipt.owner_settlement_account
        {
            return Err(Error::IdentityAlias);
        }
        self.receipt.validate()?;
        Ok(GeneralOwnerFeeFinalizationProjectionV2 {
            carry_account: self.carry_account,
            runtime_release: self.receipt.runtime_release,
            fee_record: self.receipt.fee_record,
            settlement_candidate: self.receipt.settlement_candidate,
            owner: self.receipt.owner,
            payer_allocation_data_id: self.receipt.payer_allocation_data_id,
            owner_settlement_account: self.receipt.owner_settlement_account,
            owner_settlement_final_data_id: self.receipt.owner_settlement_final_data_id,
            position: self.receipt.position,
            settlement_cash_pot: self.receipt.settlement_cash_pot,
            rent_disposition_data_id: self.receipt.rent_disposition_data_id,
            outcome: self.receipt.outcome,
            authorized_fee_atoms: self.receipt.authorized_fee_atoms,
            position_debit_atoms: self.receipt.position_debit_atoms,
            position_credit_atoms: self.receipt.position_credit_atoms,
            released_cash_atoms: self.receipt.released_cash_atoms,
        })
    }
}

/// General V2's exact per-owner terminal dependency. The carry account is the
/// original `(fee record, owner)` PDA, now decoded only as outer version 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOwnerFeeFinalizationProjectionV2 {
    pub carry_account: Id,
    pub runtime_release: Id,
    pub fee_record: Id,
    pub settlement_candidate: Id,
    pub owner: Id,
    pub payer_allocation_data_id: Id,
    pub owner_settlement_account: Id,
    pub owner_settlement_final_data_id: Id,
    pub position: Id,
    pub settlement_cash_pot: Id,
    pub rent_disposition_data_id: Id,
    pub outcome: OwnerFeeFinalizationOutcomeV2,
    pub authorized_fee_atoms: u64,
    pub position_debit_atoms: u64,
    pub position_credit_atoms: u64,
    pub released_cash_atoms: u64,
}

/// Candidate terminal outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FeeTerminalOutcomeV1 {
    Settled = 1,
    Aborted = 2,
}

impl FeeTerminalOutcomeV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Settled),
            2 => Ok(Self::Aborted),
            _ => Err(Error::InvalidTerminalDisposition),
        }
    }
}

/// Candidate-wide fee account closed after all owner receipts are consumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidateFeeAccountRoleV1 {
    SelectedFeeRecord = 1,
    RecipientAllocation = 2,
    TreasuryLedger = 3,
    OwnerFinalization = 4,
}

/// Exact adapter-authenticated account closure. Only native-lamport rent and
/// donation disposition are represented here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalFeeAccountClosureV1 {
    role: CandidateFeeAccountRoleV1,
    outcome: FeeTerminalOutcomeV1,
    runtime_program: Id,
    runtime_release: Id,
    fee_record: Id,
    account: Id,
    semantic_owner: Id,
    close_receipt: Id,
    rent_payer: Id,
    neutral_sink: Id,
    balance_before_lamports: u64,
    rent_principal_lamports: u64,
    donation_lamports: u64,
}

impl ExternalFeeAccountClosureV1 {
    pub const EMPTY: Self = Self {
        role: CandidateFeeAccountRoleV1::OwnerFinalization,
        outcome: FeeTerminalOutcomeV1::Aborted,
        runtime_program: Id([0; 32]),
        runtime_release: Id([0; 32]),
        fee_record: Id([0; 32]),
        account: Id([0; 32]),
        semantic_owner: Id([0; 32]),
        close_receipt: Id([0; 32]),
        rent_payer: Id([0; 32]),
        neutral_sink: Id([0; 32]),
        balance_before_lamports: 0,
        rent_principal_lamports: 0,
        donation_lamports: 0,
    };

    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        role: CandidateFeeAccountRoleV1,
        outcome: FeeTerminalOutcomeV1,
        runtime_program: Id,
        runtime_release: Id,
        fee_record: Id,
        account: Id,
        semantic_owner: Id,
        close_receipt: Id,
        rent_payer: Id,
        neutral_sink: Id,
        balance_before_lamports: u64,
        rent_principal_lamports: u64,
        donation_lamports: u64,
    ) -> Result<Self> {
        let value = Self {
            role,
            outcome,
            runtime_program,
            runtime_release,
            fee_record,
            account,
            semantic_owner,
            close_receipt,
            rent_payer,
            neutral_sink,
            balance_before_lamports,
            rent_principal_lamports,
            donation_lamports,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        independent(&[
            self.runtime_program,
            self.runtime_release,
            self.account,
            self.close_receipt,
            self.neutral_sink,
        ])?;
        live(self.fee_record)?;
        if (self.role == CandidateFeeAccountRoleV1::SelectedFeeRecord)
            != (self.account == self.fee_record)
        {
            return Err(Error::MismatchedBinding);
        }
        if self.role == CandidateFeeAccountRoleV1::OwnerFinalization {
            live(self.semantic_owner)?;
        } else if !self.semantic_owner.is_zero() {
            return Err(Error::InvalidAccountData);
        }
        if self.rent_principal_lamports == 0 {
            if !self.rent_payer.is_zero() {
                return Err(Error::InvalidTerminalDisposition);
            }
        } else {
            live(self.rent_payer)?;
        }
        if add(self.rent_principal_lamports, self.donation_lamports)?
            != self.balance_before_lamports
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        Ok(())
    }

    pub const fn account(&self) -> Id {
        self.account
    }
    pub const fn close_receipt(&self) -> Id {
        self.close_receipt
    }
    pub const fn rent_refund_lamports(&self) -> u64 {
        self.rent_principal_lamports
    }
    pub const fn neutral_credit_lamports(&self) -> u64 {
        self.donation_lamports
    }
}

/// Candidate-wide account closures present on both success and abort paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeeAccountClosuresV1 {
    pub selected_record: ExternalFeeAccountClosureV1,
    pub recipient_allocation: ExternalFeeAccountClosureV1,
    pub treasury_ledger: ExternalFeeAccountClosureV1,
}

/// Immutable aggregate receipt for every fee account closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeClosureManifestReceiptV1 {
    receipt: Id,
    runtime_program: Id,
    runtime_release: Id,
    fee_record: Id,
    terminal_authority_receipt: Id,
    /// Digest of the canonical ordered `ExternalFeeAccountClosureV1` set.
    closure_set_data_id: Id,
    outcome: FeeTerminalOutcomeV1,
    owner_count: u8,
    account_count: u16,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
}

impl FeeClosureManifestReceiptV1 {
    pub const fn receipt(&self) -> Id {
        self.receipt
    }
    pub const fn runtime_program(&self) -> Id {
        self.runtime_program
    }
    pub const fn runtime_release(&self) -> Id {
        self.runtime_release
    }
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }
    pub const fn terminal_authority_receipt(&self) -> Id {
        self.terminal_authority_receipt
    }
    pub const fn outcome(&self) -> FeeTerminalOutcomeV1 {
        self.outcome
    }
    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }
    pub const fn account_count(&self) -> u16 {
        self.account_count
    }
    pub const fn closure_set_data_id(&self) -> Id {
        self.closure_set_data_id
    }
    pub const fn payer_refund_lamports(&self) -> u64 {
        self.payer_refund_lamports
    }
    pub const fn neutral_credit_lamports(&self) -> u64 {
        self.neutral_credit_lamports
    }

    pub fn encode(&self) -> Result<[u8; FEE_CLOSURE_MANIFEST_V1_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FEE_CLOSURE_MANIFEST_V1_BYTES];
        let mut at = 0usize;
        put(&mut output, &mut at, &FEE_CLOSURE_MANIFEST_MAGIC_V1)?;
        put(&mut output, &mut at, &FEE_TERMINAL_RECEIPT_VERSION_V1.to_le_bytes())?;
        put(&mut output, &mut at, &[self.outcome as u8, self.owner_count])?;
        put(&mut output, &mut at, &self.account_count.to_le_bytes())?;
        put(&mut output, &mut at, &[0; 2])?;
        for identity in [
            self.receipt,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.terminal_authority_receipt,
            self.closure_set_data_id,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        put(&mut output, &mut at, &self.payer_refund_lamports.to_le_bytes())?;
        put(&mut output, &mut at, &self.neutral_credit_lamports.to_le_bytes())?;
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, FEE_CLOSURE_MANIFEST_V1_BYTES, FEE_CLOSURE_MANIFEST_MAGIC_V1)?;
        if input[14..16] != [0; 2] {
            return Err(Error::NonCanonicalPadding);
        }
        let mut at = 16usize;
        let value = Self {
            receipt: take_id(input, &mut at)?,
            runtime_program: take_id(input, &mut at)?,
            runtime_release: take_id(input, &mut at)?,
            fee_record: take_id(input, &mut at)?,
            terminal_authority_receipt: take_id(input, &mut at)?,
            closure_set_data_id: take_id(input, &mut at)?,
            outcome: FeeTerminalOutcomeV1::decode(input[10])?,
            owner_count: input[11],
            account_count: u16::from_le_bytes([input[12], input[13]]),
            payer_refund_lamports: take_u64(input, &mut at)?,
            neutral_credit_lamports: take_u64(input, &mut at)?,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        independent(&[
            self.receipt,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.terminal_authority_receipt,
            self.closure_set_data_id,
        ])?;
        if self.owner_count == 0
            || usize::from(self.owner_count) > MAX_FEE_ROWS_V1
            || self.account_count != u16::from(self.owner_count) + 3
        {
            return Err(Error::InvalidWidth);
        }
        Ok(())
    }
}

/// Canonical terminal or abort receipt for one selected fee record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeRecordTerminalReceiptV1 {
    terminal_receipt: Id,
    closure_manifest: Id,
    runtime_program: Id,
    runtime_release: Id,
    fee_record: Id,
    realm: Id,
    market: Id,
    epoch: Id,
    settlement_candidate: Id,
    batch_policy: Id,
    revenue_policy: Id,
    treasury_position: Id,
    value_disposition_receipt: Id,
    terminal_authority_receipt: Id,
    outcome: FeeTerminalOutcomeV1,
    owner_count: u8,
    collected_fee_atoms: u128,
    released_authorization_atoms: u128,
    maker_rebate_atoms: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
}

impl FeeRecordTerminalReceiptV1 {
    pub const fn terminal_receipt(&self) -> Id {
        self.terminal_receipt
    }
    pub const fn closure_manifest(&self) -> Id {
        self.closure_manifest
    }
    pub const fn runtime_program(&self) -> Id {
        self.runtime_program
    }
    pub const fn runtime_release(&self) -> Id {
        self.runtime_release
    }
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }
    pub const fn terminal_authority_receipt(&self) -> Id {
        self.terminal_authority_receipt
    }
    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }
    pub const fn outcome(&self) -> FeeTerminalOutcomeV1 {
        self.outcome
    }
    pub const fn collected_fee_atoms(&self) -> u128 {
        self.collected_fee_atoms
    }
    pub const fn released_authorization_atoms(&self) -> u128 {
        self.released_authorization_atoms
    }

    pub fn project_general(&self) -> GeneralFeeTerminalProjectionV1 {
        GeneralFeeTerminalProjectionV1 {
            terminal_receipt: self.terminal_receipt,
            closure_manifest: self.closure_manifest,
            fee_record: self.fee_record,
            market: self.market,
            epoch: self.epoch,
            settlement_candidate: self.settlement_candidate,
            outcome: self.outcome,
            owner_count: self.owner_count,
            collected_fee_atoms: self.collected_fee_atoms,
            released_authorization_atoms: self.released_authorization_atoms,
            value_disposition_receipt: self.value_disposition_receipt,
            payer_refund_lamports: self.payer_refund_lamports,
            neutral_credit_lamports: self.neutral_credit_lamports,
        }
    }

    pub fn project_dealer(&self) -> DealerFeeTerminalProjectionV1 {
        DealerFeeTerminalProjectionV1 {
            terminal_receipt: self.terminal_receipt,
            fee_record: self.fee_record,
            settlement_candidate: self.settlement_candidate,
            fee_policy: self.batch_policy,
            outcome: self.outcome,
        }
    }

    pub fn encode(&self) -> Result<[u8; FEE_TERMINAL_RECEIPT_V1_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FEE_TERMINAL_RECEIPT_V1_BYTES];
        let mut at = 0usize;
        put(&mut output, &mut at, &FEE_TERMINAL_RECEIPT_MAGIC_V1)?;
        put(&mut output, &mut at, &FEE_TERMINAL_RECEIPT_VERSION_V1.to_le_bytes())?;
        put(&mut output, &mut at, &[self.outcome as u8, self.owner_count])?;
        put(&mut output, &mut at, &[0; 4])?;
        for identity in [
            self.terminal_receipt,
            self.closure_manifest,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.realm,
            self.market,
            self.epoch,
            self.settlement_candidate,
            self.batch_policy,
            self.revenue_policy,
            self.treasury_position,
            self.value_disposition_receipt,
            self.terminal_authority_receipt,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        put(&mut output, &mut at, &self.collected_fee_atoms.to_le_bytes())?;
        put(
            &mut output,
            &mut at,
            &self.released_authorization_atoms.to_le_bytes(),
        )?;
        for amount in [
            self.maker_rebate_atoms,
            self.executor_atoms,
            self.treasury_atoms,
            self.payer_refund_lamports,
            self.neutral_credit_lamports,
        ] {
            put(&mut output, &mut at, &amount.to_le_bytes())?;
        }
        put(&mut output, &mut at, &[0; 8])?;
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, FEE_TERMINAL_RECEIPT_V1_BYTES, FEE_TERMINAL_RECEIPT_MAGIC_V1)?;
        if input[12..16] != [0; 4] || input[536..544] != [0; 8] {
            return Err(Error::NonCanonicalPadding);
        }
        let outcome = FeeTerminalOutcomeV1::decode(input[10])?;
        let owner_count = input[11];
        let mut at = 16usize;
        let terminal_receipt = take_id(input, &mut at)?;
        let closure_manifest = take_id(input, &mut at)?;
        let runtime_program = take_id(input, &mut at)?;
        let runtime_release = take_id(input, &mut at)?;
        let fee_record = take_id(input, &mut at)?;
        let realm = take_id(input, &mut at)?;
        let market = take_id(input, &mut at)?;
        let epoch = take_id(input, &mut at)?;
        let settlement_candidate = take_id(input, &mut at)?;
        let batch_policy = take_id(input, &mut at)?;
        let revenue_policy = take_id(input, &mut at)?;
        let treasury_position = take_id(input, &mut at)?;
        let value_disposition_receipt = take_id(input, &mut at)?;
        let terminal_authority_receipt = take_id(input, &mut at)?;
        let collected_fee_atoms = take_u128(input, &mut at)?;
        let released_authorization_atoms = take_u128(input, &mut at)?;
        let maker_rebate_atoms = take_u64(input, &mut at)?;
        let executor_atoms = take_u64(input, &mut at)?;
        let treasury_atoms = take_u64(input, &mut at)?;
        let payer_refund_lamports = take_u64(input, &mut at)?;
        let neutral_credit_lamports = take_u64(input, &mut at)?;
        let value = Self {
            terminal_receipt,
            closure_manifest,
            runtime_program,
            runtime_release,
            fee_record,
            realm,
            market,
            epoch,
            settlement_candidate,
            batch_policy,
            revenue_policy,
            treasury_position,
            value_disposition_receipt,
            terminal_authority_receipt,
            outcome,
            owner_count,
            collected_fee_atoms,
            released_authorization_atoms,
            maker_rebate_atoms,
            executor_atoms,
            treasury_atoms,
            payer_refund_lamports,
            neutral_credit_lamports,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        independent(&[
            self.terminal_receipt,
            self.closure_manifest,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.realm,
            self.market,
            self.epoch,
            self.settlement_candidate,
            self.batch_policy,
            self.revenue_policy,
            self.treasury_position,
            self.value_disposition_receipt,
            self.terminal_authority_receipt,
        ])?;
        if self.owner_count == 0 || usize::from(self.owner_count) > MAX_FEE_ROWS_V1 {
            return Err(Error::InvalidWidth);
        }
        match self.outcome {
            FeeTerminalOutcomeV1::Settled => {
                if self.released_authorization_atoms != 0
                    || u128::from(add(
                        add(self.maker_rebate_atoms, self.executor_atoms)?,
                        self.treasury_atoms,
                    )?) != self.collected_fee_atoms
                {
                    return Err(Error::ConservationFailure);
                }
            }
            FeeTerminalOutcomeV1::Aborted => {
                if self.collected_fee_atoms != 0
                    || self.maker_rebate_atoms != 0
                    || self.executor_atoms != 0
                    || self.treasury_atoms != 0
                {
                    return Err(Error::InvalidTerminalDisposition);
                }
            }
        }
        Ok(())
    }
}

/// Complete result of candidate-wide terminal construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeTerminalReceiptBundleV1 {
    pub closure_manifest: FeeClosureManifestReceiptV1,
    pub terminal: FeeRecordTerminalReceiptV1,
}

/// General V2's read-only terminal dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFeeTerminalProjectionV1 {
    pub terminal_receipt: Id,
    pub closure_manifest: Id,
    pub fee_record: Id,
    pub market: Id,
    pub epoch: Id,
    pub settlement_candidate: Id,
    pub outcome: FeeTerminalOutcomeV1,
    pub owner_count: u8,
    pub collected_fee_atoms: u128,
    pub released_authorization_atoms: u128,
    pub value_disposition_receipt: Id,
    /// Refunds from accounts closed by the candidate-wide terminal action.
    pub payer_refund_lamports: u64,
    /// Hostile prefunding on those accounts, assigned only to the neutral sink.
    pub neutral_credit_lamports: u64,
}

/// Dealer's read-only fee-policy terminal dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFeeTerminalProjectionV1 {
    pub terminal_receipt: Id,
    pub fee_record: Id,
    pub settlement_candidate: Id,
    pub fee_policy: Id,
    pub outcome: FeeTerminalOutcomeV1,
}

impl DealerFeeTerminalProjectionV1 {
    pub const fn available_liveness_lamports(&self) -> u64 {
        0
    }
    pub const fn available_hoard_atoms(&self) -> u64 {
        0
    }
    pub const fn available_fee_funding_atoms(&self) -> u64 {
        0
    }
}

/// Build a settled candidate receipt from the exact owner book/allocation and
/// the temporary owner receipts which are closed in this terminal action.
#[allow(clippy::too_many_arguments)]
pub fn build_settled_fee_terminal_receipt_v1(
    terminal_receipt: Id,
    closure_manifest_receipt: Id,
    closure_set_data_id: Id,
    runtime_program: Id,
    runtime_release: Id,
    value_disposition_receipt: Id,
    terminal_authority_receipt: Id,
    selected: &SelectedCompositeFeeV1,
    book: &SelectedOwnerFeeBookV1,
    settlement: &CandidateFeeSettlementV1,
    recipients: &RecipientAllocationV1,
    recipient_intent: &RecipientAllocationIntentV1,
    treasury: &TreasuryLedgerV1,
    owners: &[AuthenticatedOwnerFeeFinalizationV1; MAX_FEE_ROWS_V1],
    owner_closures: &[ExternalFeeAccountClosureV1; MAX_FEE_ROWS_V1],
    owner_len: u8,
    global: CandidateFeeAccountClosuresV1,
) -> Result<FeeTerminalReceiptBundleV1> {
    settlement.validate(book, recipients)?;
    validate_global(selected, recipient_intent, &global, FeeTerminalOutcomeV1::Settled)?;
    if owner_len != book.owner_count()
        || runtime_program != global.selected_record.runtime_program
        || runtime_release != global.selected_record.runtime_release
        || treasury.fee_record() != selected.fee_record()
        || treasury.treasury_position() != selected.treasury_position()
        || treasury.outstanding_epochs() != 0
        || treasury.credited_atoms() != recipients.treasury_atoms()
        || treasury.withdrawn_atoms() != 0
        || treasury.available_atoms() != recipients.treasury_atoms()
        || treasury.is_closed()
    {
        return Err(Error::InvalidTerminalDisposition);
    }
    let mut collected = 0u128;
    let (mut rent_refund, mut neutral_credit) = global_lamports(&global)?;
    let mut index = 0usize;
    while index < usize::from(owner_len) {
        let owner = owners[index];
        let closure = owner_closures[index];
        if owner.receipt.outcome != OwnerFeeFinalizationOutcomeV2::Settled
            || owner.receipt.runtime_release != runtime_release
            || owner.receipt.fee_record != selected.fee_record()
            || owner.receipt.settlement_candidate != selected.selected_candidate()
            || owner.receipt.owner.0 != book.rows()[index].owner
            || owner.receipt.authorized_fee_atoms != book.rows()[index].fee_atoms
        {
            return Err(Error::MismatchedBinding);
        }
        validate_owner_terminal_close(
            &owner,
            &closure,
            runtime_program,
            runtime_release,
            selected,
            FeeTerminalOutcomeV1::Settled,
            &global,
            &owner_closures[..index],
        )?;
        collected = collected
            .checked_add(u128::from(owner.receipt.authorized_fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        rent_refund = add(rent_refund, closure.rent_refund_lamports())?;
        neutral_credit = add(neutral_credit, closure.neutral_credit_lamports())?;
        index += 1;
    }
    while index < MAX_FEE_ROWS_V1 {
        if owners[index] != AuthenticatedOwnerFeeFinalizationV1::EMPTY
            || owner_closures[index] != ExternalFeeAccountClosureV1::EMPTY
        {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if collected != book.selected_fee_atoms() {
        return Err(Error::ConservationFailure);
    }
    finish_terminal(
        terminal_receipt,
        closure_manifest_receipt,
        closure_set_data_id,
        runtime_program,
        runtime_release,
        value_disposition_receipt,
        terminal_authority_receipt,
        selected,
        FeeTerminalOutcomeV1::Settled,
        owner_len,
        collected,
        0,
        recipients.maker_rebate_total(),
        recipients.executor_atoms(),
        recipients.treasury_atoms(),
        rent_refund,
        neutral_credit,
    )
}

/// Build an abort receipt after every owner authorization was released and no
/// recipient/treasury collateral credit was created.
#[allow(clippy::too_many_arguments)]
pub fn build_aborted_fee_terminal_receipt_v1(
    terminal_receipt: Id,
    closure_manifest_receipt: Id,
    closure_set_data_id: Id,
    runtime_program: Id,
    runtime_release: Id,
    value_disposition_receipt: Id,
    terminal_authority_receipt: Id,
    selected: &SelectedCompositeFeeV1,
    book: &SelectedOwnerFeeBookV1,
    recipients: &RecipientAllocationV1,
    recipient_intent: &RecipientAllocationIntentV1,
    treasury: &TreasuryLedgerV1,
    owners: &[AuthenticatedOwnerFeeFinalizationV1; MAX_FEE_ROWS_V1],
    owner_closures: &[ExternalFeeAccountClosureV1; MAX_FEE_ROWS_V1],
    owner_len: u8,
    global: CandidateFeeAccountClosuresV1,
) -> Result<FeeTerminalReceiptBundleV1> {
    validate_global(selected, recipient_intent, &global, FeeTerminalOutcomeV1::Aborted)?;
    if owner_len != book.owner_count()
        || runtime_program != global.selected_record.runtime_program
        || runtime_release != global.selected_record.runtime_release
        || treasury.fee_record() != selected.fee_record()
        || treasury.treasury_position() != selected.treasury_position()
        || treasury.credited_atoms() != 0
        || treasury.withdrawn_atoms() != 0
        || treasury.available_atoms() != 0
        || treasury.outstanding_epochs() != 0
        || treasury.is_closed()
    {
        return Err(Error::InvalidTerminalDisposition);
    }
    let mut released = 0u128;
    let (mut rent_refund, mut neutral_credit) = global_lamports(&global)?;
    let mut index = 0usize;
    while index < usize::from(owner_len) {
        let owner = owners[index];
        let closure = owner_closures[index];
        if owner.receipt.outcome != OwnerFeeFinalizationOutcomeV2::Aborted
            || owner.receipt.runtime_release != runtime_release
            || owner.receipt.fee_record != selected.fee_record()
            || owner.receipt.settlement_candidate != selected.selected_candidate()
            || owner.receipt.owner.0 != book.rows()[index].owner
            || owner.receipt.authorized_fee_atoms != book.rows()[index].fee_atoms
        {
            return Err(Error::MismatchedBinding);
        }
        validate_owner_terminal_close(
            &owner,
            &closure,
            runtime_program,
            runtime_release,
            selected,
            FeeTerminalOutcomeV1::Aborted,
            &global,
            &owner_closures[..index],
        )?;
        released = released
            .checked_add(u128::from(owner.receipt.authorized_fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        rent_refund = add(rent_refund, closure.rent_refund_lamports())?;
        neutral_credit = add(neutral_credit, closure.neutral_credit_lamports())?;
        index += 1;
    }
    while index < MAX_FEE_ROWS_V1 {
        if owners[index] != AuthenticatedOwnerFeeFinalizationV1::EMPTY
            || owner_closures[index] != ExternalFeeAccountClosureV1::EMPTY
        {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if released != book.selected_fee_atoms()
        || recipients.fee_record() != selected.fee_record()
        || u128::from(recipients.collected_fee_atoms()) != released
    {
        return Err(Error::ConservationFailure);
    }
    finish_terminal(
        terminal_receipt,
        closure_manifest_receipt,
        closure_set_data_id,
        runtime_program,
        runtime_release,
        value_disposition_receipt,
        terminal_authority_receipt,
        selected,
        FeeTerminalOutcomeV1::Aborted,
        owner_len,
        0,
        released,
        0,
        0,
        0,
        rent_refund,
        neutral_credit,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_terminal(
    terminal_receipt: Id,
    closure_manifest_receipt: Id,
    closure_set_data_id: Id,
    runtime_program: Id,
    runtime_release: Id,
    value_disposition_receipt: Id,
    terminal_authority_receipt: Id,
    selected: &SelectedCompositeFeeV1,
    outcome: FeeTerminalOutcomeV1,
    owner_count: u8,
    collected_fee_atoms: u128,
    released_authorization_atoms: u128,
    maker_rebate_atoms: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
) -> Result<FeeTerminalReceiptBundleV1> {
    let closure_manifest = FeeClosureManifestReceiptV1 {
        receipt: closure_manifest_receipt,
        runtime_program,
        runtime_release,
        fee_record: selected.fee_record(),
        terminal_authority_receipt,
        closure_set_data_id,
        outcome,
        owner_count,
        account_count: u16::from(owner_count) + 3,
        payer_refund_lamports,
        neutral_credit_lamports,
    };
    closure_manifest.validate()?;
    let terminal = FeeRecordTerminalReceiptV1 {
        terminal_receipt,
        closure_manifest: closure_manifest_receipt,
        runtime_program,
        runtime_release,
        fee_record: selected.fee_record(),
        realm: selected.realm(),
        market: selected.market(),
        epoch: selected.epoch(),
        settlement_candidate: selected.selected_candidate(),
        batch_policy: selected.batch_policy(),
        revenue_policy: selected.revenue_policy(),
        treasury_position: selected.treasury_position(),
        value_disposition_receipt,
        terminal_authority_receipt,
        outcome,
        owner_count,
        collected_fee_atoms,
        released_authorization_atoms,
        maker_rebate_atoms,
        executor_atoms,
        treasury_atoms,
        payer_refund_lamports,
        neutral_credit_lamports,
    };
    terminal.validate()?;
    Ok(FeeTerminalReceiptBundleV1 {
        closure_manifest,
        terminal,
    })
}

fn validate_global(
    selected: &SelectedCompositeFeeV1,
    intent: &RecipientAllocationIntentV1,
    global: &CandidateFeeAccountClosuresV1,
    outcome: FeeTerminalOutcomeV1,
) -> Result<()> {
    let expected = [
        (
            &global.selected_record,
            CandidateFeeAccountRoleV1::SelectedFeeRecord,
            selected.fee_record(),
        ),
        (
            &global.recipient_allocation,
            CandidateFeeAccountRoleV1::RecipientAllocation,
            intent.recipient_allocation().identity(),
        ),
        (
            &global.treasury_ledger,
            CandidateFeeAccountRoleV1::TreasuryLedger,
            intent.treasury_ledger().identity(),
        ),
    ];
    for (closure, role, account) in expected {
        closure.validate()?;
        if closure.role != role
            || closure.outcome != outcome
            || closure.fee_record != selected.fee_record()
            || closure.account != account
            || !closure.semantic_owner.is_zero()
        {
            return Err(Error::MissingClosure);
        }
    }
    if intent.fee_record().identity() != selected.fee_record()
        || intent.settlement_candidate() != selected.selected_candidate()
        || intent.revenue_policy() != selected.revenue_policy()
        || intent.treasury_position() != selected.treasury_position()
        || global.selected_record.runtime_program != global.recipient_allocation.runtime_program
        || global.selected_record.runtime_program != global.treasury_ledger.runtime_program
        || global.selected_record.runtime_release != global.recipient_allocation.runtime_release
        || global.selected_record.runtime_release != global.treasury_ledger.runtime_release
        || global.selected_record.neutral_sink != global.recipient_allocation.neutral_sink
        || global.selected_record.neutral_sink != global.treasury_ledger.neutral_sink
    {
        return Err(Error::MismatchedBinding);
    }
    distinct_closure(&global.selected_record, &global.recipient_allocation)?;
    distinct_closure(&global.selected_record, &global.treasury_ledger)?;
    distinct_closure(&global.recipient_allocation, &global.treasury_ledger)
}

#[allow(clippy::too_many_arguments)]
fn validate_owner_terminal_close(
    owner: &AuthenticatedOwnerFeeFinalizationV1,
    closure: &ExternalFeeAccountClosureV1,
    runtime_program: Id,
    runtime_release: Id,
    selected: &SelectedCompositeFeeV1,
    outcome: FeeTerminalOutcomeV1,
    global: &CandidateFeeAccountClosuresV1,
    prior: &[ExternalFeeAccountClosureV1],
) -> Result<()> {
    closure.validate()?;
    if closure.role != CandidateFeeAccountRoleV1::OwnerFinalization
        || closure.outcome != outcome
        || closure.runtime_program != runtime_program
        || closure.runtime_release != runtime_release
        || closure.fee_record != selected.fee_record()
        || closure.account != owner.carry_account
        || closure.semantic_owner != owner.receipt.owner
        || closure.neutral_sink != global.selected_record.neutral_sink
    {
        return Err(Error::MissingClosure);
    }
    for candidate in [
        &global.selected_record,
        &global.recipient_allocation,
        &global.treasury_ledger,
    ] {
        distinct_closure(closure, candidate)?;
    }
    for candidate in prior {
        distinct_closure(closure, candidate)?;
    }
    Ok(())
}

fn distinct_closure(
    left: &ExternalFeeAccountClosureV1,
    right: &ExternalFeeAccountClosureV1,
) -> Result<()> {
    if left.account == right.account || left.close_receipt == right.close_receipt {
        Err(Error::DuplicateIdentity)
    } else {
        Ok(())
    }
}

fn global_lamports(global: &CandidateFeeAccountClosuresV1) -> Result<(u64, u64)> {
    Ok((
        add(
            add(
                global.selected_record.rent_refund_lamports(),
                global.recipient_allocation.rent_refund_lamports(),
            )?,
            global.treasury_ledger.rent_refund_lamports(),
        )?,
        add(
            add(
                global.selected_record.neutral_credit_lamports(),
                global.recipient_allocation.neutral_credit_lamports(),
            )?,
            global.treasury_ledger.neutral_credit_lamports(),
        )?,
    ))
}

fn require_header(input: &[u8], width: usize, magic: [u8; 8]) -> Result<()> {
    if input.len() != width
        || input[..8] != magic
        || u16::from_le_bytes([input[8], input[9]]) != FEE_TERMINAL_RECEIPT_VERSION_V1
    {
        return Err(Error::InvalidAccountData);
    }
    Ok(())
}

fn put(output: &mut [u8], at: &mut usize, value: &[u8]) -> Result<()> {
    let end = at.checked_add(value.len()).ok_or(Error::ArithmeticOverflow)?;
    let destination = output.get_mut(*at..end).ok_or(Error::InvalidWidth)?;
    destination.copy_from_slice(value);
    *at = end;
    Ok(())
}

fn take_id(input: &[u8], at: &mut usize) -> Result<Id> {
    let mut output = [0u8; 32];
    take(input, at, &mut output)?;
    Ok(Id(output))
}

fn take_u8(input: &[u8], at: &mut usize) -> Result<u8> {
    let value = *input.get(*at).ok_or(Error::InvalidWidth)?;
    *at += 1;
    Ok(value)
}

fn take_u16(input: &[u8], at: &mut usize) -> Result<u16> {
    let mut output = [0u8; 2];
    take(input, at, &mut output)?;
    Ok(u16::from_le_bytes(output))
}

fn take_u64(input: &[u8], at: &mut usize) -> Result<u64> {
    let mut output = [0u8; 8];
    take(input, at, &mut output)?;
    Ok(u64::from_le_bytes(output))
}

fn take_u128(input: &[u8], at: &mut usize) -> Result<u128> {
    let mut output = [0u8; 16];
    take(input, at, &mut output)?;
    Ok(u128::from_le_bytes(output))
}

fn take<const N: usize>(input: &[u8], at: &mut usize, output: &mut [u8; N]) -> Result<()> {
    let end = at.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    output.copy_from_slice(input.get(*at..end).ok_or(Error::InvalidWidth)?);
    *at = end;
    Ok(())
}

const EMPTY_OWNER_FINALIZATION: OwnerFeeFinalizationReceiptV1 =
    OwnerFeeFinalizationReceiptV1 {
        runtime_release: Id([0; 32]),
        fee_record: Id([0; 32]),
        settlement_candidate: Id([0; 32]),
        owner: Id([0; 32]),
        payer_allocation_data_id: Id([0; 32]),
        owner_settlement_account: Id([0; 32]),
        owner_settlement_final_data_id: Id([0; 32]),
        position: Id([0; 32]),
        settlement_cash_pot: Id([0; 32]),
        rent_disposition_data_id: Id([0; 32]),
        outcome: OwnerFeeFinalizationOutcomeV2::Aborted,
        authorized_fee_atoms: 0,
        position_debit_atoms: 0,
        position_credit_atoms: 0,
        released_cash_atoms: 0,
        position_cash_before: 0,
        position_cash_after: 0,
        position_reserved_before: 0,
        position_reserved_after: 0,
        pot_available_before: 0,
        pot_available_after: 0,
        pot_collected_fee_before: 0,
        pot_collected_fee_after: 0,
        owner_rounding_residue_price_units: 0,
        pot_rounding_before_price_units: 0,
        pot_rounding_after_price_units: 0,
        pot_finalized_owner_count_before: 0,
        pot_finalized_owner_count_after: 0,
        pot_state_before: 0,
        pot_state_after: 0,
    };
