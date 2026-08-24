// SPDX-License-Identifier: AGPL-3.0-or-later

//! Capability-disabled General V2 adapter plans for fee finalization.
//!
//! These plans freeze the account graph and exact atomic poststate without
//! deriving a Solana PDA, reading account memory, moving lamports, or enabling
//! action 38. The SBF adapter must authenticate those outer facts first and
//! must commit every returned write and close in one instruction.

use clutch_fee_runtime_contract::projection::AuthenticatedSelectedOwnerFeeV2;
use clutch_fee_runtime_contract::retirement::FeePositionCreditTransitionV1;
use clutch_fee_runtime_contract::selected::{OwnerFeeCarryV1, SelectedCompositeFeeAccess};
use clutch_fee_runtime_contract::terminal::{
    AuthenticatedOwnerFeeFinalizationV1, GeneralFeeTerminalProjectionV1,
    OwnerFeeFinalizationBindingsV2, OwnerFeeFinalizationReceiptV1, OwnerFeeRentDispositionV2,
};
use clutch_owner_settlement::{
    prepare_realize_owner_cash_v2, OwnerCashRealizationPlanV2, OwnerFinalizedRowDataHashV2,
    OwnerSettlementAccountProjectionV2, SettlementCashPotV1,
};
use clutch_retirement::{PositionV3Sha256Backend, ReplayV3HashBackend};

use crate::{
    CodecError, FinalizeOwnerSettlementPayloadV1, GeneralPositionReplayPrestateV1,
    GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1, Id32,
    OwnerFeeFinalizationV2AccountV1, Sha256BackendV1, OWNER_FEE_CARRY_ACCOUNT_BYTES,
    OWNER_FEE_CARRY_SEED_DOMAIN_V1, OWNER_FEE_FINALIZATION_ACCOUNT_BYTES,
    PAYER_ALLOCATION_ACCOUNT_BYTES, PAYER_ALLOCATION_SEED_DOMAIN_V1,
    RECIPIENT_ALLOCATION_SEED_DOMAIN_V1, SELECTED_FEE_RECORD_SEED_DOMAIN_V1,
    SETTLEMENT_CASH_POT_SEED_DOMAIN_V1, TREASURY_LEDGER_SEED_DOMAIN_V1,
};

/// Canonical data-ID domain for complete General fee outer-account bytes.
pub const GENERAL_FEE_ACCOUNT_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-fee-account-data/v1\0";
/// Canonical data-ID domain for the exact owner fee-rent transition preimage.
pub const OWNER_FEE_RENT_DATA_ID_DOMAIN_V2: &[u8] =
    b"dragons-clutch/owner-fee-rent-transition/v2\0";
/// Canonical data-ID domain for the exact cash-pot semantic poststate body.
pub const SETTLEMENT_CASH_POT_POSTSTATE_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/settlement-cash-pot-poststate/v1\0";
/// Canonical identity for one action-50 fee Position credit.
pub const FEE_POSITION_CREDIT_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-fee-position-credit/v1\0";

fn live(value: Id32) -> Result<(), CodecError> {
    if value.is_zero() {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn distinct(values: &[Id32]) -> Result<(), CodecError> {
    let mut left = 0usize;
    while left < values.len() {
        live(values[left])?;
        let mut right = left + 1;
        while right < values.len() {
            if values[left] == values[right] {
                return Err(CodecError::MismatchedBinding);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Hash the exact temporary payer-allocation outer bytes before atomic close.
pub fn payer_allocation_account_data_id_v1<B: Sha256BackendV1>(
    bytes: &[u8],
    backend: &B,
) -> Result<Id32, CodecError> {
    if bytes.len() != PAYER_ALLOCATION_ACCOUNT_BYTES {
        return Err(CodecError::WrongLength);
    }
    Id32::new(backend.sha256(&[GENERAL_FEE_ACCOUNT_DATA_ID_DOMAIN_V1, bytes]))
}

/// Hash the exact canonical SettlementCashPot semantic successor body.
pub fn settlement_cash_pot_poststate_data_id_v1<B: Sha256BackendV1>(
    pot: SettlementCashPotV1,
    backend: &B,
) -> Result<Id32, CodecError> {
    let body = pot.encode_body().map_err(|_| CodecError::InvalidState)?;
    Id32::new(backend.sha256(&[SETTLEMENT_CASH_POT_POSTSTATE_DATA_ID_DOMAIN_V1, &body]))
}

/// Exact atomic Position/Replay/cash-pot successor for one fee recipient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeePositionCreditPlanV1 {
    position: clutch_owner_settlement::PositionSettlementPoststateV3,
    replay: Option<GeneralReplayTransitionPlanV1>,
    cash_pot: SettlementCashPotV1,
    semantic: FeePositionCreditTransitionV1,
}

impl FeePositionCreditPlanV1 {
    pub const fn position(&self) -> clutch_owner_settlement::PositionSettlementPoststateV3 {
        self.position
    }
    pub const fn replay(&self) -> Option<GeneralReplayTransitionPlanV1> { self.replay }
    pub const fn cash_pot(&self) -> SettlementCashPotV1 { self.cash_pot }
    pub const fn semantic(&self) -> FeePositionCreditTransitionV1 { self.semantic }
}

/// Re-derive one exact fee credit from authenticated semantic owners. A zero
/// allocation advances only the fee cursor; it cannot fabricate a Position or
/// Replay transition.
#[allow(clippy::too_many_arguments)]
pub fn prepare_fee_position_credit_v1<B>(
    fee_record: Id32,
    recipient_allocation_data_id: Id32,
    settlement_cash_pot_account: Id32,
    recipient_kind: u8,
    recipient_ordinal: u8,
    credited_atoms: u64,
    position_replay: GeneralPositionReplayPrestateV1,
    cash_pot_before: SettlementCashPotV1,
    backend: &B,
) -> Result<FeePositionCreditPlanV1, CodecError>
where
    B: Sha256BackendV1 + PositionV3Sha256Backend + ReplayV3HashBackend,
{
    distinct(&[
        fee_record,
        recipient_allocation_data_id,
        settlement_cash_pot_account,
        Id32::from_bytes(position_replay.position().account),
        position_replay.replay_account(),
    ])?;
    if recipient_kind != 1 && recipient_kind != 2 {
        return Err(CodecError::InvalidState);
    }
    let expectation = cash_pot_before.expectation;
    if expectation.fee_record != fee_record.bytes()
        || cash_pot_before.state == 0
        || cash_pot_before.finalized_owner_count != expectation.owner_count
    {
        return Err(CodecError::MismatchedBinding);
    }
    let position_before = position_replay.position();
    let position_after = position_before
        .credit_free_cash_poststate(credited_atoms)
        .map_err(|_| CodecError::ArithmeticOverflow)?;
    let cash_pot_after = cash_pot_before
        .distribute_collected_fee(credited_atoms)
        .map_err(|_| CodecError::InvalidState)?;
    let pot_prestate = settlement_cash_pot_poststate_data_id_v1(cash_pot_before, backend)?;
    let pot_poststate = settlement_cash_pot_poststate_data_id_v1(cash_pot_after, backend)?;
    let position_poststate = Id32::new(
        position_after
            .semantic
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let transition_id = Id32::new(backend.sha256(&[
        FEE_POSITION_CREDIT_TRANSITION_DOMAIN_V1,
        &fee_record.bytes(),
        &recipient_allocation_data_id.bytes(),
        &[recipient_kind, recipient_ordinal],
        &position_before.account,
        &position_before.semantic_id,
        &position_poststate.bytes(),
        &settlement_cash_pot_account.bytes(),
        &pot_prestate.bytes(),
        &pot_poststate.bytes(),
        &credited_atoms.to_le_bytes(),
    ]))?;
    let replay = if credited_atoms == 0 {
        None
    } else {
        Some(crate::project_general_replay_transition_v1(
            position_replay,
            position_after,
            GeneralReplayTransitionKindV1::DistributeTradingFee,
            transition_id,
            recipient_allocation_data_id,
            backend,
        )?)
    };
    let (replay_prestate, replay_poststate) = match replay {
        Some(value) => (
            value.replay_prestate_semantic_id(),
            value.replay_poststate_semantic_id(),
        ),
        None => (
            position_replay.replay_semantic_id(),
            position_replay.replay_semantic_id(),
        ),
    };
    let semantic = FeePositionCreditTransitionV1 {
        position_account: clutch_fee_runtime_contract::Id(position_before.account),
        replay_account: clutch_fee_runtime_contract::Id(position_replay.replay_account().bytes()),
        position_prestate: clutch_fee_runtime_contract::Id(position_before.semantic_id),
        position_poststate: clutch_fee_runtime_contract::Id(position_poststate.bytes()),
        replay_prestate: clutch_fee_runtime_contract::Id(replay_prestate.bytes()),
        replay_poststate: clutch_fee_runtime_contract::Id(replay_poststate.bytes()),
        cash_pot_account: clutch_fee_runtime_contract::Id(settlement_cash_pot_account.bytes()),
        cash_pot_prestate: clutch_fee_runtime_contract::Id(pot_prestate.bytes()),
        cash_pot_poststate: clutch_fee_runtime_contract::Id(pot_poststate.bytes()),
        credited_atoms,
    };
    Ok(FeePositionCreditPlanV1 {
        position: position_after,
        replay,
        cash_pot: cash_pot_after,
        semantic,
    })
}

/// Recompute the canonical rent-transition data ID without accepting a digest
/// summary as authority. The existing rent ledger remains the semantic owner
/// of every principal/refund field supplied to this function.
pub fn owner_fee_rent_disposition_data_id_v2<B: Sha256BackendV1>(
    rent: OwnerFeeRentDispositionV2,
    backend: &B,
) -> Result<Id32, CodecError> {
    rent.validate().map_err(|_| CodecError::InvalidState)?;
    let carry_balance_before = rent.carry_balance_before_lamports.to_le_bytes();
    let carry_principal_before = rent.carry_rent_principal_before_lamports.to_le_bytes();
    let carry_donation_before = rent.carry_donation_before_lamports.to_le_bytes();
    let carry_v2_minimum = rent.carry_v2_rent_minimum_lamports.to_le_bytes();
    let carry_top_up = rent.carry_top_up_lamports.to_le_bytes();
    let carry_balance_after = rent.carry_balance_after_lamports.to_le_bytes();
    let carry_principal_after = rent.carry_rent_principal_after_lamports.to_le_bytes();
    let carry_donation_after = rent.carry_donation_after_lamports.to_le_bytes();
    let payer_balance_before = rent.payer_balance_before_lamports.to_le_bytes();
    let payer_principal = rent.payer_rent_principal_lamports.to_le_bytes();
    let payer_donation = rent.payer_donation_lamports.to_le_bytes();
    let expected = Id32::new(backend.sha256(&[
        OWNER_FEE_RENT_DATA_ID_DOMAIN_V2,
        &rent.carry_account.0,
        &rent.payer_allocation_account.0,
        &rent.carry_rent_refund_owner.0,
        &rent.carry_top_up_payer.0,
        &rent.payer_rent_refund_owner.0,
        &rent.neutral_sink.0,
        &carry_balance_before,
        &carry_principal_before,
        &carry_donation_before,
        &carry_v2_minimum,
        &carry_top_up,
        &carry_balance_after,
        &carry_principal_after,
        &carry_donation_after,
        &payer_balance_before,
        &payer_principal,
        &payer_donation,
    ]))?;
    if expected.bytes() != rent.data_id.0 {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(expected)
}

/// Same-PDA seed tuple shared by mutable carry and immutable finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeePdaSeedTupleV2 {
    fee_record: [u8; 32],
    owner: [u8; 32],
}

impl OwnerFeePdaSeedTupleV2 {
    /// Bind the selected fee-record PDA and semantic owner.
    pub fn new(fee_record: Id32, owner: Id32) -> Result<Self, CodecError> {
        distinct(&[fee_record, owner])?;
        Ok(Self {
            fee_record: fee_record.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First canonical seed.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_FEE_CARRY_SEED_DOMAIN_V1
    }

    /// Second canonical seed.
    pub const fn fee_record(&self) -> &[u8; 32] {
        &self.fee_record
    }

    /// Third canonical seed.
    pub const fn owner(&self) -> &[u8; 32] {
        &self.owner
    }
}

/// Temporary payer-allocation PDA seed tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerAllocationPdaSeedTupleV1 {
    fee_record: [u8; 32],
    owner: [u8; 32],
}

impl PayerAllocationPdaSeedTupleV1 {
    /// Bind the selected fee-record PDA and semantic owner.
    pub fn new(fee_record: Id32, owner: Id32) -> Result<Self, CodecError> {
        distinct(&[fee_record, owner])?;
        Ok(Self {
            fee_record: fee_record.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First canonical seed.
    pub const fn domain(&self) -> &'static [u8] {
        PAYER_ALLOCATION_SEED_DOMAIN_V1
    }

    /// Second canonical seed.
    pub const fn fee_record(&self) -> &[u8; 32] {
        &self.fee_record
    }

    /// Third canonical seed.
    pub const fn owner(&self) -> &[u8; 32] {
        &self.owner
    }
}

/// Candidate-wide SettlementCashPot PDA seed tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCashPotPdaSeedTupleV1 {
    epoch: [u8; 32],
    settlement_candidate: [u8; 32],
}

impl SettlementCashPotPdaSeedTupleV1 {
    /// Bind the counted Epoch PDA and final settlement-candidate identity.
    pub fn new(epoch: Id32, settlement_candidate: Id32) -> Result<Self, CodecError> {
        distinct(&[epoch, settlement_candidate])?;
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
        })
    }

    /// First canonical seed.
    pub const fn domain(&self) -> &'static [u8] {
        SETTLEMENT_CASH_POT_SEED_DOMAIN_V1
    }

    /// Second canonical seed.
    pub const fn epoch(&self) -> &[u8; 32] {
        &self.epoch
    }

    /// Third canonical seed.
    pub const fn settlement_candidate(&self) -> &[u8; 32] {
        &self.settlement_candidate
    }
}

/// Candidate-scoped fee PDA seed tuple used by selected, recipient, or treasury.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeePdaSeedTupleV1 {
    parent: [u8; 32],
}

impl CandidateFeePdaSeedTupleV1 {
    /// Selected fee record is keyed by the SelectedCandidate PDA.
    pub fn selected(selected_candidate: Id32) -> Result<Self, CodecError> {
        live(selected_candidate)?;
        Ok(Self {
            parent: selected_candidate.bytes(),
        })
    }

    /// Recipient allocation and treasury ledger are keyed by fee-record PDA.
    pub fn child(selected_fee_record: Id32) -> Result<Self, CodecError> {
        live(selected_fee_record)?;
        Ok(Self {
            parent: selected_fee_record.bytes(),
        })
    }

    /// Selected-record domain.
    pub const fn selected_domain(&self) -> &'static [u8] {
        SELECTED_FEE_RECORD_SEED_DOMAIN_V1
    }

    /// Recipient-allocation domain.
    pub const fn recipient_domain(&self) -> &'static [u8] {
        RECIPIENT_ALLOCATION_SEED_DOMAIN_V1
    }

    /// Treasury-ledger domain.
    pub const fn treasury_domain(&self) -> &'static [u8] {
        TREASURY_LEDGER_SEED_DOMAIN_V1
    }

    /// Exact parent seed.
    pub const fn parent(&self) -> &[u8; 32] {
        &self.parent
    }
}

/// Exact pre/post account identities and bump facts authenticated by the SBF loader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAction38AccountsV2 {
    /// Counted SettlementRoot PDA named by the action selector.
    pub settlement_root_account: Id32,
    /// Selected fee-record PDA.
    pub fee_record_account: Id32,
    /// Existing carry PDA, retained across the version transition.
    pub carry_account: Id32,
    /// Temporary payer-allocation PDA deleted atomically.
    pub payer_allocation_account: Id32,
    /// Owner-settlement PDA written to state one.
    pub owner_settlement_account: Id32,
    /// Canonical Position V3 PDA.
    pub position_account: Id32,
    /// Canonical purpose-owned Replay V3 PDA paired with the Position.
    pub replay_account: Id32,
    /// Candidate-wide cash-pot PDA.
    pub settlement_cash_pot_account: Id32,
    /// Semantic owner used in both owner-scoped PDA derivations.
    pub owner: Id32,
    /// Existing carry bump, preserved in the v2 envelope.
    pub carry_bump: u8,
    /// Existing payer-allocation bump authenticated before close.
    pub payer_allocation_bump: u8,
}

impl OwnerFeeAction38AccountsV2 {
    fn validate(&self) -> Result<(), CodecError> {
        distinct(&[
            self.settlement_root_account,
            self.fee_record_account,
            self.carry_account,
            self.payer_allocation_account,
            self.owner_settlement_account,
            self.position_account,
            self.replay_account,
            self.settlement_cash_pot_account,
            self.owner,
        ])?;
        let _ = OwnerFeePdaSeedTupleV2::new(self.fee_record_account, self.owner)?;
        let _ = PayerAllocationPdaSeedTupleV1::new(self.fee_record_account, self.owner)?;
        Ok(())
    }
}

/// One exact lamport transfer performed by the disabled handler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeLamportTransferV2 {
    /// Debit account.
    pub source: Id32,
    /// Credit account.
    pub destination: Id32,
    /// Exact native-lamport amount; zero is canonical and causes no CPI/write.
    pub lamports: u64,
}

/// Complete atomic action-38 plan. No field independently authorizes execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAction38PlanV2 {
    /// Original pure realization, re-bound to the canonical finalized-row data ID.
    realization: OwnerCashRealizationPlanV2,
    /// Canonical purpose-owned Replay V3 successor staged with the Position.
    replay: GeneralReplayTransitionPlanV1,
    /// Exact outer v2 carry/finalization poststate.
    finalization: OwnerFeeFinalizationV2AccountV1,
    /// Existing carry PDA, resized in place.
    carry_account: Id32,
    /// Payer-allocation PDA deleted only after every other write is staged.
    payer_allocation_account: Id32,
    /// Exact carry realloc width before transition.
    carry_bytes_before: u16,
    /// Exact carry realloc width after transition.
    carry_bytes_after: u16,
    /// Present-funding transfer into the carry before realloc.
    carry_top_up: FeeLamportTransferV2,
    /// Refund of only authenticated payer-allocation rent principal.
    payer_rent_refund: FeeLamportTransferV2,
    /// Hostile prefunding/donation credit to the neutral sink.
    payer_donation_credit: FeeLamportTransferV2,
    /// Exact payer-allocation lamport balance before close.
    payer_balance_before_lamports: u64,
    /// Exact carry lamport balance required after top-up and realloc.
    carry_balance_after_lamports: u64,
    /// Canonical data ID of the deleted complete payer outer bytes.
    payer_allocation_data_id: Id32,
    /// Canonical data ID of the complete rent transition.
    rent_disposition_data_id: Id32,
}

impl OwnerFeeAction38PlanV2 {
    /// Exact presence-explicit owner row/Position/pot successor.
    pub const fn realization(&self) -> OwnerCashRealizationPlanV2 {
        self.realization
    }

    /// Exact paired purpose Replay V3 successor.
    pub const fn replay(&self) -> GeneralReplayTransitionPlanV1 {
        self.replay
    }

    /// Exact `0x83/2` terminal outer account poststate.
    pub const fn finalization(&self) -> OwnerFeeFinalizationV2AccountV1 {
        self.finalization
    }

    /// Existing carry PDA retained across the version transition.
    pub const fn carry_account(&self) -> Id32 {
        self.carry_account
    }

    /// Temporary payer-allocation PDA deleted atomically.
    pub const fn payer_allocation_account(&self) -> Id32 {
        self.payer_allocation_account
    }

    /// Exact carry width before in-place reallocation.
    pub const fn carry_bytes_before(&self) -> u16 {
        self.carry_bytes_before
    }

    /// Exact finalization width after in-place reallocation.
    pub const fn carry_bytes_after(&self) -> u16 {
        self.carry_bytes_after
    }

    /// Exact present-funded carry top-up.
    pub const fn carry_top_up(&self) -> FeeLamportTransferV2 {
        self.carry_top_up
    }

    /// Exact payer-account refundable principal transfer.
    pub const fn payer_rent_refund(&self) -> FeeLamportTransferV2 {
        self.payer_rent_refund
    }

    /// Exact payer-account hostile-prefunding disposition.
    pub const fn payer_donation_credit(&self) -> FeeLamportTransferV2 {
        self.payer_donation_credit
    }

    /// Exact payer-account balance that must be fully disposed.
    pub const fn payer_balance_before_lamports(&self) -> u64 {
        self.payer_balance_before_lamports
    }

    /// Exact carry balance after top-up and reallocation.
    pub const fn carry_balance_after_lamports(&self) -> u64 {
        self.carry_balance_after_lamports
    }

    /// Complete-data identity of the deleted payer outer prestate.
    pub const fn payer_allocation_data_id(&self) -> Id32 {
        self.payer_allocation_data_id
    }

    /// Canonical identity of the authenticated rent-ledger transition.
    pub const fn rent_disposition_data_id(&self) -> Id32 {
        self.rent_disposition_data_id
    }
}

/// Construct the settled action-38 plan from exact authenticated semantic owners.
#[allow(clippy::too_many_arguments)]
pub fn prepare_owner_fee_action38_v2<B, S>(
    request: FinalizeOwnerSettlementPayloadV1,
    accounts: OwnerFeeAction38AccountsV2,
    selected: &S,
    projection: &AuthenticatedSelectedOwnerFeeV2,
    carry: &OwnerFeeCarryV1,
    bindings: OwnerFeeFinalizationBindingsV2,
    position_replay_before: GeneralPositionReplayPrestateV1,
    owner_settlement_before: OwnerSettlementAccountProjectionV2,
    pot_before: SettlementCashPotV1,
    payer_allocation_outer_bytes: &[u8],
    backend: &B,
) -> Result<OwnerFeeAction38PlanV2, CodecError>
where
    B: Sha256BackendV1
        + PositionV3Sha256Backend
        + ReplayV3HashBackend
        + OwnerFinalizedRowDataHashV2,
    S: SelectedCompositeFeeAccess + ?Sized,
{
    accounts.validate()?;
    let rent = bindings.rent_disposition;
    let position_before = position_replay_before.position();
    let position_before_fields = position_before.semantic.fields();
    let realization = prepare_realize_owner_cash_v2(
        owner_settlement_before,
        position_before,
        pot_before,
        backend,
    )
    .map_err(|_| CodecError::InvalidState)?;
    let expectation = realization.expectation();
    let position_after = realization.position();
    let position_after_fields = position_after.semantic.fields();
    let finalized_owner_row_data_id = Id32::new(realization.finalized_row_data_id())?;
    let payer_allocation_data_id =
        payer_allocation_account_data_id_v1(payer_allocation_outer_bytes, backend)?;
    let pot_poststate_data_id =
        settlement_cash_pot_poststate_data_id_v1(realization.settlement_cash_pot(), backend)?;
    let rent_disposition_data_id = owner_fee_rent_disposition_data_id_v2(rent, backend)?;
    let replay = crate::project_general_replay_transition_v1(
        position_replay_before,
        position_after,
        GeneralReplayTransitionKindV1::FinalizeOwnerSettlement,
        finalized_owner_row_data_id,
        payer_allocation_data_id,
        backend,
    )?;
    if request.settlement_root != accounts.settlement_root_account
        || request.owner_settlement != accounts.owner_settlement_account
        || request.position != accounts.position_account
        || request.settlement_cash_pot != accounts.settlement_cash_pot_account
        || finalized_owner_row_data_id.bytes() != bindings.owner_settlement_final_data_id.0
        || request.epoch.bytes() != expectation.epoch
        || expectation.owner != accounts.owner.bytes()
        || expectation.candidate != selected.selected_candidate().0
        || accounts.fee_record_account.bytes() != selected.fee_record().0
        || accounts.carry_account != Id32::from_bytes(projection.carry_account().0)
        || accounts.payer_allocation_account
            != Id32::from_bytes(projection.payer_allocation_account().0)
        || accounts.owner_settlement_account
            != Id32::from_bytes(projection.owner_settlement_account().0)
        || accounts.position_account.bytes() != position_after.account
        || accounts.replay_account.bytes() != position_before_fields.replay_account.bytes()
        || accounts.replay_account.bytes() != position_after_fields.replay_account.bytes()
        || accounts.replay_account != replay.replay_account()
        || accounts.settlement_cash_pot_account.bytes() != bindings.settlement_cash_pot.0
        || payer_allocation_data_id.bytes() != bindings.payer_allocation_data_id.0
        || pot_poststate_data_id.bytes() != bindings.settlement_cash_pot_poststate_data_id.0
        || rent_disposition_data_id.bytes() != rent.data_id.0
        || replay.kind() != GeneralReplayTransitionKindV1::FinalizeOwnerSettlement
        || replay.transition_id() != finalized_owner_row_data_id
        || replay.transition_evidence_id() != payer_allocation_data_id
        || replay.position_poststate_semantic_id().bytes()
            != bindings.position_poststate_semantic_id.0
        || replay.replay_poststate_semantic_id().bytes() != bindings.replay_poststate_semantic_id.0
        || replay.next_sequence() != bindings.replay_next_sequence
        || rent.carry_account.0 != accounts.carry_account.bytes()
        || rent.payer_allocation_account.0 != accounts.payer_allocation_account.bytes()
    {
        return Err(CodecError::MismatchedBinding);
    }
    let semantic =
        OwnerFeeFinalizationReceiptV1::settle(selected, projection, carry, bindings, realization)
            .map_err(|_| CodecError::InvalidState)?;
    let finalization = OwnerFeeFinalizationV2AccountV1 {
        semantic,
        stored_bump: accounts.carry_bump,
    };
    Ok(OwnerFeeAction38PlanV2 {
        realization,
        replay,
        finalization,
        carry_account: accounts.carry_account,
        payer_allocation_account: accounts.payer_allocation_account,
        carry_bytes_before: u16::try_from(OWNER_FEE_CARRY_ACCOUNT_BYTES)
            .map_err(|_| CodecError::ArithmeticOverflow)?,
        carry_bytes_after: u16::try_from(OWNER_FEE_FINALIZATION_ACCOUNT_BYTES)
            .map_err(|_| CodecError::ArithmeticOverflow)?,
        carry_top_up: FeeLamportTransferV2 {
            source: Id32::from_bytes(rent.carry_top_up_payer.0),
            destination: accounts.carry_account,
            lamports: rent.carry_top_up_lamports,
        },
        payer_rent_refund: FeeLamportTransferV2 {
            source: accounts.payer_allocation_account,
            destination: Id32::from_bytes(rent.payer_rent_refund_owner.0),
            lamports: rent.payer_rent_principal_lamports,
        },
        payer_donation_credit: FeeLamportTransferV2 {
            source: accounts.payer_allocation_account,
            destination: Id32::from_bytes(rent.neutral_sink.0),
            lamports: rent.payer_donation_lamports,
        },
        payer_balance_before_lamports: rent.payer_balance_before_lamports,
        carry_balance_after_lamports: rent.carry_balance_after_lamports,
        payer_allocation_data_id,
        rent_disposition_data_id,
    })
}

/// Read-only General dependency on a candidate-wide settled or abort receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFeeTerminalJoinV1 {
    /// Exact terminal projection owned by the fee runtime.
    pub terminal: GeneralFeeTerminalProjectionV1,
    /// SelectedCandidate PDA authenticated by General.
    pub selected_candidate_account: Id32,
    /// General FinalPot account whose terminal disposition was authenticated.
    pub final_pot_account: Id32,
    /// Exact candidate-wide fee account-close set data ID.
    pub closure_set_data_id: Id32,
}

impl GeneralFeeTerminalJoinV1 {
    /// Admit no funding authority; this is terminal evidence only.
    pub const fn available_liveness_lamports(&self) -> u64 {
        0
    }

    /// Admit no Hoard principal authority; this is terminal evidence only.
    pub const fn available_hoard_atoms(&self) -> u64 {
        0
    }

    /// Admit no future-fee capitalization authority.
    pub const fn available_future_fee_atoms(&self) -> u64 {
        0
    }
}

/// Bind the fee-runtime terminal projection to General-owned candidate facts.
pub fn bind_general_fee_terminal_v1(
    terminal: GeneralFeeTerminalProjectionV1,
    selected_candidate_account: Id32,
    final_pot_account: Id32,
    closure_set_data_id: Id32,
    expected_market: Id32,
    expected_epoch: Id32,
    expected_settlement_candidate: Id32,
    expected_fee_record: Id32,
) -> Result<GeneralFeeTerminalJoinV1, CodecError> {
    distinct(&[
        selected_candidate_account,
        final_pot_account,
        closure_set_data_id,
        expected_market,
        expected_epoch,
        expected_settlement_candidate,
        expected_fee_record,
    ])?;
    if terminal.market.0 != expected_market.bytes()
        || terminal.epoch.0 != expected_epoch.bytes()
        || terminal.settlement_candidate.0 != expected_settlement_candidate.bytes()
        || terminal.fee_record.0 != expected_fee_record.bytes()
        || terminal.value_disposition_receipt.0 != final_pot_account.bytes()
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(GeneralFeeTerminalJoinV1 {
        terminal,
        selected_candidate_account,
        final_pot_account,
        closure_set_data_id,
    })
}

/// Convert one authenticated v2 outer account into the candidate terminal input.
pub fn authenticated_owner_finalization_v2(
    carry_account: Id32,
    account: OwnerFeeFinalizationV2AccountV1,
) -> Result<AuthenticatedOwnerFeeFinalizationV1, CodecError> {
    live(carry_account)?;
    Ok(AuthenticatedOwnerFeeFinalizationV1 {
        carry_account: clutch_fee_runtime_contract::Id(carry_account.bytes()),
        receipt: account.semantic,
    })
}

const _: () = assert!(OWNER_FEE_CARRY_ACCOUNT_BYTES == 132);
const _: () = assert!(OWNER_FEE_FINALIZATION_ACCOUNT_BYTES == 500);
const _: () = assert!(PAYER_ALLOCATION_ACCOUNT_BYTES == 2_684);
