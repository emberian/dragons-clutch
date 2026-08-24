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
    AuthenticatedPositionV3, OwnerCashRealizationPlanV2, OwnerCashRealizationSemanticPlanV4,
    SettlementCashPotV1,
};

use crate::allocation::RecipientAllocationV1;
use crate::integration::CandidateFeeSettlementV1;
use crate::intent::RecipientAllocationIntentV1;
use crate::projection::{
    AuthenticatedSelectedOwnerFeeV2, AuthenticatedSelectedOwnerFeeV4, SelectedOwnerFeeBookV1,
};
use crate::retirement::{CompletedFeeRetirementV1, FeeRetirementHashV1};
use crate::selected::{
    OwnerFeeCarryV1, SelectedCompositeFeeAccess, SelectedCompositeFeeV1,
    SelectedCompositeFeeV2,
};
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
/// Exact canonical streaming-retirement closure-manifest width.
pub const FEE_CLOSURE_MANIFEST_V2_BYTES: usize = 528;
/// Terminal receipt magic.
pub const FEE_TERMINAL_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCFEEEND";
/// Closure-manifest receipt magic.
pub const FEE_CLOSURE_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCFEECLS";
/// Closure-manifest magic for the accumulator-authenticated live path.
pub const FEE_CLOSURE_MANIFEST_MAGIC_V2: [u8; 8] = *b"DCFEECL2";
/// Shared terminal receipt version.
pub const FEE_TERMINAL_RECEIPT_VERSION_V1: u16 = 1;
/// Fresh streaming closure-manifest semantic version.
pub const FEE_CLOSURE_MANIFEST_VERSION_V2: u16 = 2;
/// Domain for the exact canonical terminal semantic body identity.
pub const FEE_TERMINAL_DATA_ID_DOMAIN_V2: &[u8] =
    b"dragons-clutch/fee-terminal-data/v2\0";

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
    /// Adapter-authenticated semantic ID of the canonical Position V3 poststate.
    /// The selected Position book owns the prestate ID. Abort requires the
    /// poststate ID to equal the authenticated prestate ID.
    pub position_poststate_semantic_id: Id,
    /// Adapter-authenticated semantic ID of the canonical purpose-owned
    /// Replay V3 poststate committed atomically with the Position successor.
    pub replay_poststate_semantic_id: Id,
    /// Exact `next_sequence` in that Replay V3 successor. Settled realization
    /// advances the live envelope exactly once; abort may retain the
    /// authenticated current envelope without claiming a mutation.
    pub replay_next_sequence: u64,
    /// Digest of the exact canonical SettlementCashPot poststate body. The
    /// pot body, rather than copied balance fields, remains the semantic owner
    /// of collected fee, consideration, rounding, count, and lifecycle.
    pub settlement_cash_pot_poststate_data_id: Id,
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
            self.position_poststate_semantic_id,
            self.replay_poststate_semantic_id,
            self.settlement_cash_pot_poststate_data_id,
            self.rent_disposition.data_id,
        ])?;
        if self.replay_next_sequence == 0 {
            return Err(Error::InvalidTerminalDisposition);
        }
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
    position_poststate_semantic_id: Id,
    replay_poststate_semantic_id: Id,
    settlement_cash_pot_poststate_data_id: Id,
    outcome: OwnerFeeFinalizationOutcomeV2,
    authorized_fee_atoms: u64,
    position_debit_atoms: u64,
    position_credit_atoms: u64,
    released_cash_atoms: u64,
    replay_next_sequence: u64,
}

impl OwnerFeeFinalizationReceiptV1 {
    /// Construct the settled successor from the pure owner-realization plan,
    /// not a caller-provided debit summary.
    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        selected: &SelectedCompositeFeeV1,
        projection: &AuthenticatedSelectedOwnerFeeV2,
        carry: &OwnerFeeCarryV1,
        bindings: OwnerFeeFinalizationBindingsV2,
        plan: OwnerCashRealizationPlanV2,
    ) -> Result<Self> {
        bindings.validate()?;
        let expectation = plan.expectation();
        let disposition = plan.disposition();
        let position = plan.position();
        let settlement_cash_pot = plan.settlement_cash_pot();
        settlement_cash_pot
            .validate()
            .map_err(|_| Error::InvalidAccountData)?;
        let owner = Id(projection.row().owner);
        if projection.fee_record() != selected.fee_record()
            || projection.settlement_candidate() != selected.selected_candidate()
            || projection.revenue_policy() != selected.revenue_policy()
            || carry.fee_record() != selected.fee_record()
            || carry.owner() != owner
            || carry.denominator() != selected.carry_denominator()
            || !carry.is_closed()
            || carry.remainder() != 0
            || carry.paid_atoms() != projection.row().fee_atoms
            || bindings.owner_settlement_account.0 != plan.owner_settlement_account()
            || bindings.owner_settlement_final_data_id.0 != plan.finalized_row_data_id()
            || expectation.owner != owner.0
            || expectation.candidate != selected.selected_candidate().0
            || expectation.selected_fee_atoms != carry.paid_atoms()
            || settlement_cash_pot.expectation.candidate != selected.selected_candidate().0
            || settlement_cash_pot.expectation.fee_record != selected.fee_record().0
            || disposition.selected_fee_atoms != carry.paid_atoms()
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
            position: Id(position.account),
            settlement_cash_pot: bindings.settlement_cash_pot,
            rent_disposition_data_id: bindings.rent_disposition.data_id,
            position_poststate_semantic_id: bindings.position_poststate_semantic_id,
            replay_poststate_semantic_id: bindings.replay_poststate_semantic_id,
            settlement_cash_pot_poststate_data_id: bindings
                .settlement_cash_pot_poststate_data_id,
            outcome: OwnerFeeFinalizationOutcomeV2::Settled,
            authorized_fee_atoms: carry.paid_atoms(),
            position_debit_atoms: disposition.debit_atoms,
            position_credit_atoms: disposition.credit_atoms,
            released_cash_atoms: disposition.released_cash_atoms,
            replay_next_sequence: bindings.replay_next_sequence,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct the current delivery-complete successor from the outer-
    /// neutral V4 realization shared by the rent-owned General V5 row.
    ///
    /// The adapter must independently authenticate and exact-join the full
    /// V5 row poststate data ID, Position semantic ID, Replay successor, pot
    /// data ID, and persisted rent transition carried in `bindings`.
    #[allow(clippy::too_many_arguments)]
    pub fn settle_delivery_complete_v4<S: SelectedCompositeFeeAccess + ?Sized>(
        selected: &S,
        projection: &AuthenticatedSelectedOwnerFeeV4,
        carry: &OwnerFeeCarryV1,
        bindings: OwnerFeeFinalizationBindingsV2,
        plan: &OwnerCashRealizationSemanticPlanV4,
    ) -> Result<Self> {
        bindings.validate()?;
        let expectation = plan.expectation();
        let disposition = plan.disposition();
        let position = plan.position();
        let position_fields = position.semantic.fields();
        let settlement_cash_pot = plan.settlement_cash_pot();
        settlement_cash_pot
            .validate()
            .map_err(|_| Error::InvalidAccountData)?;
        let owner = Id(projection.row().owner);
        if projection.fee_record() != selected.fee_record()
            || projection.settlement_candidate() != selected.selected_candidate()
            || projection.revenue_policy() != selected.revenue_policy()
            || projection.expectation() != expectation
            || projection.owner_settlement_account().0 != plan.owner_settlement_account()
            || carry.fee_record() != selected.fee_record()
            || carry.owner() != owner
            || carry.denominator() != selected.carry_denominator()
            || !carry.is_closed()
            || carry.remainder() != 0
            || carry.paid_atoms() != projection.row().fee_atoms
            || expectation.owner() != owner.0
            || expectation.candidate() != selected.selected_candidate().0
            || expectation.selected_fee_atoms() != carry.paid_atoms()
            || settlement_cash_pot.expectation.candidate != selected.selected_candidate().0
            || settlement_cash_pot.expectation.fee_record != selected.fee_record().0
            || disposition.selected_fee_atoms() != carry.paid_atoms()
            || position_fields.owner.bytes() != owner.0
            || bindings.owner_settlement_account.0 != plan.owner_settlement_account()
            || bindings.rent_disposition.carry_account != projection.carry_account()
            || bindings.rent_disposition.payer_allocation_account
                != projection.payer_allocation_account()
            || bindings.payer_allocation_data_id != projection.payer_allocation_data_id()
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
            position: Id(position.account),
            settlement_cash_pot: bindings.settlement_cash_pot,
            rent_disposition_data_id: bindings.rent_disposition.data_id,
            position_poststate_semantic_id: bindings.position_poststate_semantic_id,
            replay_poststate_semantic_id: bindings.replay_poststate_semantic_id,
            settlement_cash_pot_poststate_data_id: bindings
                .settlement_cash_pot_poststate_data_id,
            outcome: OwnerFeeFinalizationOutcomeV2::Settled,
            authorized_fee_atoms: carry.paid_atoms(),
            position_debit_atoms: disposition.total_debit_atoms(),
            position_credit_atoms: disposition.credit_atoms(),
            released_cash_atoms: disposition.released_cash_atoms(),
            replay_next_sequence: bindings.replay_next_sequence,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct the abort successor. Envelope authorization is released; no
    /// collateral debit/credit or pot mutation may occur through this route.
    #[allow(clippy::too_many_arguments)]
    pub fn abort(
        selected: &SelectedCompositeFeeV1,
        projection: &AuthenticatedSelectedOwnerFeeV2,
        carry: &OwnerFeeCarryV1,
        bindings: OwnerFeeFinalizationBindingsV2,
        position: AuthenticatedPositionV3,
        pot: SettlementCashPotV1,
    ) -> Result<Self> {
        bindings.validate()?;
        position.validate().map_err(|_| Error::InvalidAccountData)?;
        let owner = carry.owner();
        let position_fields = position.semantic.fields();
        if projection.fee_record() != selected.fee_record()
            || projection.settlement_candidate() != selected.selected_candidate()
            || projection.revenue_policy() != selected.revenue_policy()
            || projection.row().owner != owner.0
            || projection.row().fee_atoms != carry.paid_atoms()
            || projection.carry_account() != bindings.rent_disposition.carry_account
            || projection.payer_allocation_account()
                != bindings.rent_disposition.payer_allocation_account
            || projection.owner_settlement_account() != bindings.owner_settlement_account
            || carry.fee_record() != selected.fee_record()
            || carry.denominator() != selected.carry_denominator()
            || !carry.is_closed()
            || carry.remainder() != 0
            || position_fields.owner.bytes() != owner.0
            || bindings.position_poststate_semantic_id != Id(position.semantic_id)
            || pot.expectation.candidate != selected.selected_candidate().0
            || pot.expectation.fee_record != selected.fee_record().0
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
            position: Id(position.account),
            settlement_cash_pot: bindings.settlement_cash_pot,
            rent_disposition_data_id: bindings.rent_disposition.data_id,
            position_poststate_semantic_id: bindings.position_poststate_semantic_id,
            replay_poststate_semantic_id: bindings.replay_poststate_semantic_id,
            settlement_cash_pot_poststate_data_id: bindings
                .settlement_cash_pot_poststate_data_id,
            outcome: OwnerFeeFinalizationOutcomeV2::Aborted,
            authorized_fee_atoms: carry.paid_atoms(),
            position_debit_atoms: 0,
            position_credit_atoms: 0,
            released_cash_atoms: 0,
            replay_next_sequence: bindings.replay_next_sequence,
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
    pub const fn position_poststate_semantic_id(&self) -> Id {
        self.position_poststate_semantic_id
    }
    pub const fn replay_poststate_semantic_id(&self) -> Id {
        self.replay_poststate_semantic_id
    }
    pub const fn settlement_cash_pot_poststate_data_id(&self) -> Id {
        self.settlement_cash_pot_poststate_data_id
    }
    pub const fn replay_next_sequence(&self) -> u64 {
        self.replay_next_sequence
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
            self.position_poststate_semantic_id,
            self.replay_poststate_semantic_id,
            self.settlement_cash_pot_poststate_data_id,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        for amount in [
            self.authorized_fee_atoms,
            self.position_debit_atoms,
            self.position_credit_atoms,
            self.released_cash_atoms,
            self.replay_next_sequence,
        ] {
            put(&mut output, &mut at, &amount.to_le_bytes())?;
        }
        put(&mut output, &mut at, &[0; 24])?;
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
            || input[472..496] != [0; 24]
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
        let position_poststate_semantic_id = take_id(input, &mut at)?;
        let replay_poststate_semantic_id = take_id(input, &mut at)?;
        let settlement_cash_pot_poststate_data_id = take_id(input, &mut at)?;
        let authorized_fee_atoms = take_u64(input, &mut at)?;
        let position_debit_atoms = take_u64(input, &mut at)?;
        let position_credit_atoms = take_u64(input, &mut at)?;
        let released_cash_atoms = take_u64(input, &mut at)?;
        let replay_next_sequence = take_u64(input, &mut at)?;
        at += 24;
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
            position_poststate_semantic_id,
            replay_poststate_semantic_id,
            settlement_cash_pot_poststate_data_id,
            outcome,
            authorized_fee_atoms,
            position_debit_atoms,
            position_credit_atoms,
            released_cash_atoms,
            replay_next_sequence,
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
            self.position_poststate_semantic_id,
            self.replay_poststate_semantic_id,
            self.settlement_cash_pot_poststate_data_id,
        ])?;
        if self.replay_next_sequence == 0 {
            return Err(Error::InvalidTerminalDisposition);
        }
        match self.outcome {
            OwnerFeeFinalizationOutcomeV2::Settled => {
                let _consideration = self
                    .position_debit_atoms
                    .checked_sub(self.authorized_fee_atoms)
                    .ok_or(Error::ConservationFailure)?;
                if self
                    .position_debit_atoms
                    .checked_add(self.released_cash_atoms)
                    .is_none()
                {
                    return Err(Error::ConservationFailure);
                }
            }
            OwnerFeeFinalizationOutcomeV2::Aborted => {
                if self.position_debit_atoms != 0
                    || self.position_credit_atoms != 0
                    || self.released_cash_atoms != 0
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
            position_poststate_semantic_id: self.receipt.position_poststate_semantic_id,
            replay_poststate_semantic_id: self.receipt.replay_poststate_semantic_id,
            settlement_cash_pot_poststate_data_id: self
                .receipt
                .settlement_cash_pot_poststate_data_id,
            replay_next_sequence: self.receipt.replay_next_sequence,
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
    pub position_poststate_semantic_id: Id,
    pub replay_poststate_semantic_id: Id,
    pub settlement_cash_pot_poststate_data_id: Id,
    pub replay_next_sequence: u64,
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
    /// Streaming authority retired only after all value and closure folds.
    RetirementAccumulator = 5,
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
    pub const fn role(&self) -> CandidateFeeAccountRoleV1 { self.role }
    pub const fn outcome(&self) -> FeeTerminalOutcomeV1 { self.outcome }
    pub const fn runtime_program(&self) -> Id { self.runtime_program }
    pub const fn runtime_release(&self) -> Id { self.runtime_release }
    pub const fn fee_record(&self) -> Id { self.fee_record }
    pub const fn semantic_owner(&self) -> Id { self.semantic_owner }
    pub const fn close_receipt(&self) -> Id {
        self.close_receipt
    }
    pub const fn rent_payer(&self) -> Id { self.rent_payer }
    pub const fn neutral_sink(&self) -> Id { self.neutral_sink }
    pub const fn balance_before_lamports(&self) -> u64 { self.balance_before_lamports }
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

/// Immutable closure manifest for the sole accumulator-authenticated live
/// retirement path. Unlike V1, this receipt explicitly commits the four
/// candidate-global temporary accounts, the exact accumulator close, and the
/// canonical terminal semantic body identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeClosureManifestReceiptV2 {
    receipt: Id,
    terminal_receipt: Id,
    terminal_data_id: Id,
    runtime_program: Id,
    runtime_release: Id,
    fee_record: Id,
    terminal_authority_receipt: Id,
    closure_set_data_id: Id,
    selected_record: Id,
    recipient_allocation: Id,
    treasury_ledger: Id,
    retirement_accumulator: Id,
    accumulator_close_receipt: Id,
    accumulator_rent_payer: Id,
    neutral_sink: Id,
    outcome: FeeTerminalOutcomeV1,
    owner_count: u8,
    account_count: u16,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
    accumulator_refund_lamports: u64,
    accumulator_neutral_credit_lamports: u64,
}

impl FeeClosureManifestReceiptV2 {
    pub const fn receipt(&self) -> Id {
        self.receipt
    }

    pub const fn terminal_receipt(&self) -> Id {
        self.terminal_receipt
    }

    pub const fn terminal_data_id(&self) -> Id {
        self.terminal_data_id
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

    pub const fn closure_set_data_id(&self) -> Id {
        self.closure_set_data_id
    }

    pub const fn selected_record(&self) -> Id {
        self.selected_record
    }

    pub const fn recipient_allocation(&self) -> Id {
        self.recipient_allocation
    }

    pub const fn treasury_ledger(&self) -> Id {
        self.treasury_ledger
    }

    pub const fn retirement_accumulator(&self) -> Id {
        self.retirement_accumulator
    }

    pub const fn accumulator_close_receipt(&self) -> Id {
        self.accumulator_close_receipt
    }

    pub const fn accumulator_rent_payer(&self) -> Id {
        self.accumulator_rent_payer
    }

    pub const fn neutral_sink(&self) -> Id {
        self.neutral_sink
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

    pub const fn payer_refund_lamports(&self) -> u64 {
        self.payer_refund_lamports
    }

    pub const fn neutral_credit_lamports(&self) -> u64 {
        self.neutral_credit_lamports
    }

    pub const fn accumulator_refund_lamports(&self) -> u64 {
        self.accumulator_refund_lamports
    }

    pub const fn accumulator_neutral_credit_lamports(&self) -> u64 {
        self.accumulator_neutral_credit_lamports
    }

    pub fn encode(&self) -> Result<[u8; FEE_CLOSURE_MANIFEST_V2_BYTES]> {
        self.validate()?;
        let mut output = [0u8; FEE_CLOSURE_MANIFEST_V2_BYTES];
        let mut at = 0usize;
        put(&mut output, &mut at, &FEE_CLOSURE_MANIFEST_MAGIC_V2)?;
        put(
            &mut output,
            &mut at,
            &FEE_CLOSURE_MANIFEST_VERSION_V2.to_le_bytes(),
        )?;
        put(&mut output, &mut at, &[self.outcome as u8, self.owner_count])?;
        put(&mut output, &mut at, &self.account_count.to_le_bytes())?;
        put(&mut output, &mut at, &[0; 2])?;
        for identity in [
            self.receipt,
            self.terminal_receipt,
            self.terminal_data_id,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.terminal_authority_receipt,
            self.closure_set_data_id,
            self.selected_record,
            self.recipient_allocation,
            self.treasury_ledger,
            self.retirement_accumulator,
            self.accumulator_close_receipt,
            self.accumulator_rent_payer,
            self.neutral_sink,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        for amount in [
            self.payer_refund_lamports,
            self.neutral_credit_lamports,
            self.accumulator_refund_lamports,
            self.accumulator_neutral_credit_lamports,
        ] {
            put(&mut output, &mut at, &amount.to_le_bytes())?;
        }
        if at != output.len() {
            return Err(Error::InvalidWidth);
        }
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != FEE_CLOSURE_MANIFEST_V2_BYTES
            || input[..8] != FEE_CLOSURE_MANIFEST_MAGIC_V2
        {
            return Err(Error::InvalidAccountData);
        }
        if u16::from_le_bytes([input[8], input[9]]) != FEE_CLOSURE_MANIFEST_VERSION_V2 {
            return Err(Error::WrongVersion);
        }
        if input[14..16] != [0; 2] {
            return Err(Error::NonCanonicalPadding);
        }
        let outcome = FeeTerminalOutcomeV1::decode(input[10])?;
        let owner_count = input[11];
        let account_count = u16::from_le_bytes([input[12], input[13]]);
        let mut at = 16usize;
        let value = Self {
            receipt: take_id(input, &mut at)?,
            terminal_receipt: take_id(input, &mut at)?,
            terminal_data_id: take_id(input, &mut at)?,
            runtime_program: take_id(input, &mut at)?,
            runtime_release: take_id(input, &mut at)?,
            fee_record: take_id(input, &mut at)?,
            terminal_authority_receipt: take_id(input, &mut at)?,
            closure_set_data_id: take_id(input, &mut at)?,
            selected_record: take_id(input, &mut at)?,
            recipient_allocation: take_id(input, &mut at)?,
            treasury_ledger: take_id(input, &mut at)?,
            retirement_accumulator: take_id(input, &mut at)?,
            accumulator_close_receipt: take_id(input, &mut at)?,
            accumulator_rent_payer: take_id(input, &mut at)?,
            neutral_sink: take_id(input, &mut at)?,
            outcome,
            owner_count,
            account_count,
            payer_refund_lamports: take_u64(input, &mut at)?,
            neutral_credit_lamports: take_u64(input, &mut at)?,
            accumulator_refund_lamports: take_u64(input, &mut at)?,
            accumulator_neutral_credit_lamports: take_u64(input, &mut at)?,
        };
        if at != input.len() {
            return Err(Error::InvalidWidth);
        }
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        independent(&[
            self.receipt,
            self.terminal_receipt,
            self.terminal_data_id,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.terminal_authority_receipt,
            self.closure_set_data_id,
            self.recipient_allocation,
            self.treasury_ledger,
            self.retirement_accumulator,
            self.accumulator_close_receipt,
            self.neutral_sink,
        ])?;
        live(self.accumulator_rent_payer)?;
        if self.outcome != FeeTerminalOutcomeV1::Settled
            || self.selected_record != self.fee_record
            || usize::from(self.owner_count) > MAX_FEE_ROWS_V1
            || self.account_count != u16::from(self.owner_count) + 4
            || self.accumulator_refund_lamports == 0
            || self.accumulator_refund_lamports > self.payer_refund_lamports
            || self.accumulator_neutral_credit_lamports > self.neutral_credit_lamports
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        ExternalFeeAccountClosureV1::admit(
            CandidateFeeAccountRoleV1::RetirementAccumulator,
            FeeTerminalOutcomeV1::Settled,
            self.runtime_program,
            self.runtime_release,
            self.fee_record,
            self.retirement_accumulator,
            Id([0; 32]),
            self.accumulator_close_receipt,
            self.accumulator_rent_payer,
            self.neutral_sink,
            add(
                self.accumulator_refund_lamports,
                self.accumulator_neutral_credit_lamports,
            )?,
            self.accumulator_refund_lamports,
            self.accumulator_neutral_credit_lamports,
        )?;
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
    pub const fn realm(&self) -> Id {
        self.realm
    }
    pub const fn market(&self) -> Id {
        self.market
    }
    pub const fn epoch(&self) -> Id {
        self.epoch
    }
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
    pub const fn value_disposition_receipt(&self) -> Id {
        self.value_disposition_receipt
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
        if usize::from(self.owner_count) > MAX_FEE_ROWS_V1 {
            return Err(Error::InvalidWidth);
        }
        match self.outcome {
            FeeTerminalOutcomeV1::Settled => {
                if (self.owner_count == 0) != (self.collected_fee_atoms == 0)
                    || self.released_authorization_atoms != 0
                    || u128::from(add(
                        add(self.maker_rebate_atoms, self.executor_atoms)?,
                        self.treasury_atoms,
                    )?) != self.collected_fee_atoms
                {
                    return Err(Error::ConservationFailure);
                }
            }
            FeeTerminalOutcomeV1::Aborted => {
                if self.owner_count == 0
                    || self.collected_fee_atoms != 0
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

impl FeeTerminalReceiptBundleV1 {
    /// Decode and mutually authenticate the two canonical terminal bodies.
    ///
    /// Account ownership, addresses, and PDAs remain adapter facts. This
    /// constructor owns the cross-body semantic join so General and Dealer do
    /// not independently implement subtly different receipt pairing rules.
    pub fn decode(
        closure_manifest: &[u8],
        terminal: &[u8],
    ) -> Result<FeeTerminalReceiptBundleV1> {
        let value = Self {
            closure_manifest: FeeClosureManifestReceiptV1::decode(closure_manifest)?,
            terminal: FeeRecordTerminalReceiptV1::decode(terminal)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Require the manifest and terminal body to describe one exact closure.
    pub fn validate(&self) -> Result<()> {
        self.closure_manifest.validate()?;
        self.terminal.validate()?;
        let projection = self.terminal.project_general();
        if self.terminal.closure_manifest != self.closure_manifest.receipt
            || self.closure_manifest.runtime_program != self.terminal.runtime_program
            || self.closure_manifest.runtime_release != self.terminal.runtime_release
            || self.closure_manifest.fee_record != self.terminal.fee_record
            || self.closure_manifest.terminal_authority_receipt
                != self.terminal.terminal_authority_receipt
            || self.closure_manifest.outcome != self.terminal.outcome
            || self.closure_manifest.owner_count != self.terminal.owner_count
            || self.closure_manifest.payer_refund_lamports
                != projection.payer_refund_lamports
            || self.closure_manifest.neutral_credit_lamports
                != projection.neutral_credit_lamports
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        Ok(())
    }

    pub const fn closure_manifest(&self) -> FeeClosureManifestReceiptV1 {
        self.closure_manifest
    }

    pub const fn terminal(&self) -> FeeRecordTerminalReceiptV1 {
        self.terminal
    }
}

/// Live streaming-retirement pair. The manifest's terminal data identity is
/// re-derived from the exact terminal semantic body on every decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeTerminalReceiptBundleV2 {
    pub closure_manifest: FeeClosureManifestReceiptV2,
    pub terminal: FeeRecordTerminalReceiptV1,
}

impl FeeTerminalReceiptBundleV2 {
    pub fn decode<H: FeeRetirementHashV1>(
        closure_manifest: &[u8],
        terminal: &[u8],
        hash: &H,
    ) -> Result<Self> {
        let value = Self {
            closure_manifest: FeeClosureManifestReceiptV2::decode(closure_manifest)?,
            terminal: FeeRecordTerminalReceiptV1::decode(terminal)?,
        };
        value.validate(hash)?;
        Ok(value)
    }

    pub fn validate<H: FeeRetirementHashV1>(&self, hash: &H) -> Result<()> {
        self.closure_manifest.validate()?;
        self.terminal.validate()?;
        let projection = self.terminal.project_general();
        if self.closure_manifest.receipt != self.terminal.closure_manifest
            || self.closure_manifest.terminal_receipt != self.terminal.terminal_receipt
            || self.closure_manifest.terminal_data_id != terminal_data_id_v2(&self.terminal, hash)?
            || self.closure_manifest.runtime_program != self.terminal.runtime_program
            || self.closure_manifest.runtime_release != self.terminal.runtime_release
            || self.closure_manifest.fee_record != self.terminal.fee_record
            || self.closure_manifest.terminal_authority_receipt
                != self.terminal.terminal_authority_receipt
            || self.closure_manifest.outcome != self.terminal.outcome
            || self.closure_manifest.owner_count != self.terminal.owner_count
            || self.closure_manifest.payer_refund_lamports
                != projection.payer_refund_lamports
            || self.closure_manifest.neutral_credit_lamports
                != projection.neutral_credit_lamports
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        Ok(())
    }

    pub const fn closure_manifest(&self) -> FeeClosureManifestReceiptV2 {
        self.closure_manifest
    }

    pub const fn terminal(&self) -> FeeRecordTerminalReceiptV1 {
        self.terminal
    }
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

/// Build the settled terminal from the compact streaming retirement owner.
///
/// This is the callable large-book path: `CompletedFeeRetirementV1` proves the
/// exact book commitment, every lexicographic owner row, every temporary owner
/// close, and all three global closes. It is therefore not interchangeable
/// with a caller-supplied count or aggregate amount.
#[allow(clippy::too_many_arguments)]
pub fn build_settled_fee_terminal_from_accumulator_v2<
    H: FeeRetirementHashV1,
    C: crate::codec::CertifiedRecipientAllocationAccessV3 + ?Sized,
>(
    terminal_receipt: Id,
    closure_manifest_receipt: Id,
    selected: &SelectedCompositeFeeV2,
    certified: &C,
    settlement: &CandidateFeeSettlementV1,
    recipient_intent: &RecipientAllocationIntentV1,
    treasury: &TreasuryLedgerV1,
    completed: CompletedFeeRetirementV1,
    hash: &H,
) -> Result<FeeTerminalReceiptBundleV2> {
    let accumulator = completed.accumulator();
    if accumulator.fee_record() != selected.fee_record()
        || accumulator.settlement_candidate() != selected.selected_candidate()
        || accumulator.owner_order_set_digest() != certified.owner_order_set_digest()
        || accumulator.expected_owner_count() != certified.nonzero_weight_row_count()
        || accumulator.expected_maker_count() != certified.row_count()
        || accumulator.expected_fee_atoms()
            != u128::from(certified.collected_fee_atoms())
        || recipient_intent.fee_record().identity() != selected.fee_record()
        || recipient_intent.recipient_allocation().identity()
            != accumulator.recipient_allocation()
        || recipient_intent.treasury_ledger().identity() != accumulator.treasury_ledger()
        || recipient_intent.settlement_candidate() != selected.selected_candidate()
        || recipient_intent.revenue_policy() != selected.revenue_policy()
        || recipient_intent.treasury_position() != selected.treasury_position()
        || settlement.fee_record != selected.fee_record()
        || settlement.hoard_collateral_before != settlement.hoard_collateral_after
        || settlement.selected_fee_debit_atoms != accumulator.expected_fee_atoms()
        || settlement.maker_rebate_atoms != certified.maker_rebate_total()
        || settlement.executor_atoms != certified.executor_atoms()
        || settlement.treasury_credit_atoms != certified.treasury_atoms()
        || treasury.fee_record() != selected.fee_record()
        || treasury.treasury_position() != selected.treasury_position()
        || treasury.outstanding_epochs() != 0
        || treasury.credited_atoms() != certified.treasury_atoms()
        || treasury.withdrawn_atoms() != certified.treasury_atoms()
        || treasury.available_atoms() != 0
        || !treasury.is_closed()
        || u128::from(add(
            add(certified.maker_rebate_total(), certified.executor_atoms())?,
            certified.treasury_atoms(),
        )?) != accumulator.expected_fee_atoms()
    {
        return Err(Error::InvalidTerminalDisposition);
    }
    let terminal = FeeRecordTerminalReceiptV1 {
        terminal_receipt,
        closure_manifest: closure_manifest_receipt,
        runtime_program: accumulator.runtime_program(),
        runtime_release: accumulator.runtime_release(),
        fee_record: selected.fee_record(),
        realm: selected.realm(),
        market: selected.market(),
        epoch: selected.epoch(),
        settlement_candidate: selected.selected_candidate(),
        batch_policy: selected.batch_policy(),
        revenue_policy: selected.revenue_policy(),
        treasury_position: selected.treasury_position(),
        value_disposition_receipt: accumulator.value_disposition_receipt(),
        terminal_authority_receipt: completed.terminal_authority_receipt(),
        outcome: FeeTerminalOutcomeV1::Settled,
        owner_count: accumulator.expected_owner_count(),
        collected_fee_atoms: accumulator.expected_fee_atoms(),
        released_authorization_atoms: 0,
        maker_rebate_atoms: certified.maker_rebate_total(),
        executor_atoms: certified.executor_atoms(),
        treasury_atoms: certified.treasury_atoms(),
        payer_refund_lamports: completed.payer_refund_lamports(),
        neutral_credit_lamports: completed.neutral_credit_lamports(),
    };
    terminal.validate()?;
    let manifest = FeeClosureManifestReceiptV2 {
        receipt: closure_manifest_receipt,
        terminal_receipt,
        terminal_data_id: terminal_data_id_v2(&terminal, hash)?,
        runtime_program: accumulator.runtime_program(),
        runtime_release: accumulator.runtime_release(),
        fee_record: selected.fee_record(),
        terminal_authority_receipt: completed.terminal_authority_receipt(),
        closure_set_data_id: completed.closure_set_data_id(),
        selected_record: selected.fee_record(),
        recipient_allocation: accumulator.recipient_allocation(),
        treasury_ledger: accumulator.treasury_ledger(),
        retirement_accumulator: completed.accumulator_account(),
        accumulator_close_receipt: completed.accumulator_close_receipt(),
        accumulator_rent_payer: completed.accumulator_rent_payer(),
        neutral_sink: completed.accumulator_neutral_sink(),
        outcome: FeeTerminalOutcomeV1::Settled,
        owner_count: accumulator.expected_owner_count(),
        account_count: u16::from(accumulator.expected_owner_count()) + 4,
        payer_refund_lamports: completed.payer_refund_lamports(),
        neutral_credit_lamports: completed.neutral_credit_lamports(),
        accumulator_refund_lamports: completed.accumulator_refund_lamports(),
        accumulator_neutral_credit_lamports: completed
            .accumulator_neutral_credit_lamports(),
    };
    manifest.validate()?;
    let value = FeeTerminalReceiptBundleV2 {
        closure_manifest: manifest,
        terminal,
    };
    value.validate(hash)?;
    Ok(value)
}

pub fn terminal_data_id_v2<H: FeeRetirementHashV1>(
    terminal: &FeeRecordTerminalReceiptV1,
    hash: &H,
) -> Result<Id> {
    let body = terminal.encode()?;
    let value = Id(hash.sha256(&[FEE_TERMINAL_DATA_ID_DOMAIN_V2, &body]));
    live(value)?;
    Ok(value)
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
    let value = FeeTerminalReceiptBundleV1 {
        closure_manifest,
        terminal,
    };
    value.validate()?;
    Ok(value)
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
        position_poststate_semantic_id: Id([0; 32]),
        replay_poststate_semantic_id: Id([0; 32]),
        settlement_cash_pot_poststate_data_id: Id([0; 32]),
        outcome: OwnerFeeFinalizationOutcomeV2::Aborted,
        authorized_fee_atoms: 0,
        position_debit_atoms: 0,
        position_credit_atoms: 0,
        released_cash_atoms: 0,
        replay_next_sequence: 0,
    };

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct TerminalHash;

    impl FeeRetirementHashV1 for TerminalHash {
        fn sha256(&self, _parts: &[&[u8]]) -> [u8; 32] {
            [99; 32]
        }
    }

    fn id(byte: u8) -> Id {
        Id([byte; 32])
    }

    fn terminal() -> FeeRecordTerminalReceiptV1 {
        FeeRecordTerminalReceiptV1 {
            terminal_receipt: id(1),
            closure_manifest: id(2),
            runtime_program: id(3),
            runtime_release: id(4),
            fee_record: id(5),
            realm: id(6),
            market: id(7),
            epoch: id(8),
            settlement_candidate: id(9),
            batch_policy: id(10),
            revenue_policy: id(11),
            treasury_position: id(12),
            value_disposition_receipt: id(13),
            terminal_authority_receipt: id(14),
            outcome: FeeTerminalOutcomeV1::Settled,
            owner_count: 2,
            collected_fee_atoms: 20,
            released_authorization_atoms: 0,
            maker_rebate_atoms: 12,
            executor_atoms: 0,
            treasury_atoms: 8,
            payer_refund_lamports: 100,
            neutral_credit_lamports: 7,
        }
    }

    fn manifest(terminal: &FeeRecordTerminalReceiptV1) -> FeeClosureManifestReceiptV2 {
        FeeClosureManifestReceiptV2 {
            receipt: id(2),
            terminal_receipt: id(1),
            terminal_data_id: terminal_data_id_v2(terminal, &TerminalHash).unwrap(),
            runtime_program: id(3),
            runtime_release: id(4),
            fee_record: id(5),
            terminal_authority_receipt: id(14),
            closure_set_data_id: id(15),
            selected_record: id(5),
            recipient_allocation: id(16),
            treasury_ledger: id(17),
            retirement_accumulator: id(18),
            accumulator_close_receipt: id(19),
            accumulator_rent_payer: id(20),
            neutral_sink: id(21),
            outcome: FeeTerminalOutcomeV1::Settled,
            owner_count: 2,
            account_count: 6,
            payer_refund_lamports: 100,
            neutral_credit_lamports: 7,
            accumulator_refund_lamports: 30,
            accumulator_neutral_credit_lamports: 2,
        }
    }

    #[test]
    fn streaming_manifest_binds_four_globals_and_terminal_data() {
        let terminal = terminal();
        let manifest = manifest(&terminal);
        let terminal_bytes = terminal.encode().unwrap();
        let manifest_bytes = manifest.encode().unwrap();
        let decoded = FeeTerminalReceiptBundleV2::decode(
            &manifest_bytes,
            &terminal_bytes,
            &TerminalHash,
        )
        .unwrap();
        assert_eq!(decoded.closure_manifest().account_count(), 6);
        assert_eq!(decoded.closure_manifest().retirement_accumulator(), id(18));
        assert_eq!(decoded.closure_manifest().accumulator_refund_lamports(), 30);
        assert_eq!(decoded.closure_manifest().terminal_data_id(), id(99));
    }

    #[test]
    fn streaming_manifest_admits_canonical_empty_fee_children_with_four_globals() {
        let mut terminal = terminal();
        terminal.owner_count = 0;
        terminal.collected_fee_atoms = 0;
        terminal.maker_rebate_atoms = 0;
        terminal.treasury_atoms = 0;
        let mut manifest = manifest(&terminal);
        manifest.owner_count = 0;
        manifest.account_count = 4;
        manifest.terminal_data_id = terminal_data_id_v2(&terminal, &TerminalHash).unwrap();

        let decoded = FeeTerminalReceiptBundleV2::decode(
            &manifest.encode().unwrap(),
            &terminal.encode().unwrap(),
            &TerminalHash,
        )
        .unwrap();
        assert_eq!(decoded.terminal().owner_count(), 0);
        assert_eq!(decoded.closure_manifest().account_count(), 4);

        terminal.collected_fee_atoms = 1;
        assert_eq!(terminal.encode(), Err(Error::ConservationFailure));
    }

    #[test]
    fn streaming_manifest_refuses_legacy_count_and_missing_accumulator_refund() {
        let terminal = terminal();
        let mut legacy_count = manifest(&terminal).encode().unwrap();
        legacy_count[12..14].copy_from_slice(&5u16.to_le_bytes());
        assert_eq!(
            FeeClosureManifestReceiptV2::decode(&legacy_count),
            Err(Error::InvalidTerminalDisposition)
        );

        let mut missing_refund = manifest(&terminal).encode().unwrap();
        missing_refund[512..520].copy_from_slice(&0u64.to_le_bytes());
        assert_eq!(
            FeeClosureManifestReceiptV2::decode(&missing_refund),
            Err(Error::InvalidTerminalDisposition)
        );
    }

    #[test]
    fn streaming_manifest_refuses_rebound_terminal_and_accumulator() {
        let terminal = terminal();
        let terminal_bytes = terminal.encode().unwrap();
        let mut rebound_terminal = manifest(&terminal).encode().unwrap();
        rebound_terminal[80..112].copy_from_slice(&[98; 32]);
        assert_eq!(
            FeeTerminalReceiptBundleV2::decode(
                &rebound_terminal,
                &terminal_bytes,
                &TerminalHash,
            ),
            Err(Error::InvalidTerminalDisposition)
        );

        let mut aliased_accumulator = manifest(&terminal).encode().unwrap();
        aliased_accumulator[368..400].copy_from_slice(&[17; 32]);
        assert_eq!(
            FeeClosureManifestReceiptV2::decode(&aliased_accumulator),
            Err(Error::IdentityAlias)
        );
    }

    #[test]
    fn streaming_manifest_refuses_wrong_version_and_noncanonical_padding() {
        let terminal = terminal();
        let mut wrong_version = manifest(&terminal).encode().unwrap();
        wrong_version[8..10].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            FeeClosureManifestReceiptV2::decode(&wrong_version),
            Err(Error::WrongVersion)
        );

        let mut padding = manifest(&terminal).encode().unwrap();
        padding[15] = 1;
        assert_eq!(
            FeeClosureManifestReceiptV2::decode(&padding),
            Err(Error::NonCanonicalPadding)
        );
    }
}
