// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical General V2 settlement retirement joins.
//!
//! This module authenticates existing semantic owners and emits only
//! private-field terminal capabilities. It deliberately does not move
//! lamports or delete accounts. The counted `0xa9/1` SettlementRoot now owns
//! the exhaustive candidate-scoped Receipt/owner-row/Reservation/fee/pot and
//! Dealer-child graph. Its exact terminal account may be promoted here, but
//! whole-Epoch and whole-Market close authority still requires the separate
//! page, occurrence, Source, Failure, Position, and collateral terminal joins.

use clutch_general_v2_contract::{
    derive_owner_finalized_row_data_id_v2, fee_runtime_id_from_bytes,
    AuthenticatedSelectedCandidateV1, FeeTerminalOutcomeV1, FeeTerminalReceiptBundleV1,
    FinalPotAdapterBindingV1, FinalPotRetirementProjectionV1, FinalPotV1AccountV1,
    GeneralEpochPhaseV1, GeneralEpochV6AccountV1, GeneralFeeTerminalProjectionV1,
    GeneralOwnerFeeFinalizationProjectionV2, Id32, OwnerFeeFinalizationOutcomeV2,
    OwnerFeeFinalizationV2AccountV1, OwnerFinalizedRowDataHashV2, OwnerSettlementExpectationV2,
    OwnerSettlementV2AccountV1, SelectedCandidateV1AccountV1, SettlementRootTerminalProjectionV1,
    SettlementRootV1AccountV1, Sha256BackendV1, FEE_CLOSURE_MANIFEST_V1_BYTES,
    FEE_TERMINAL_RECEIPT_V1_BYTES,
};
use clutch_retirement::{Identity32V1, RetirementErrorV2};

use crate::{
    authenticate_general_epoch_v6_exact, authenticate_general_final_pot_v1_exact,
    authenticate_general_owner_fee_finalization_v2_exact,
    authenticate_general_owner_settlement_v2_exact,
    authenticate_general_selected_candidate_v1_exact,
    authenticate_general_settlement_root_v1_exact, AccountAccessV2, AccountViewV2, CanonicalPdaV1,
    RetirementAdapterErrorV2,
};

fn identity(value: Id32) -> Result<Identity32V1, RetirementAdapterErrorV2> {
    Identity32V1::new(value.bytes()).map_err(Into::into)
}

fn general_id(value: Identity32V1) -> Result<Id32, RetirementAdapterErrorV2> {
    Id32::new(value.bytes()).map_err(Into::into)
}

/// Exact presence-explicit V2 terminal owner row joined to its in-place
/// 0x83/version-2 fee receipt.
///
/// This is not deletion authority. The row's separately persisted creation
/// rent ledger and the fee runtime's exact rent-disposition preimage remain
/// mandatory before either writable account may be closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralOwnerSettlementTerminalV2 {
    owner_settlement_account: Identity32V1,
    owner_fee_finalization_account: Identity32V1,
    program_id: Identity32V1,
    expectation: OwnerSettlementExpectationV2,
    fee: GeneralOwnerFeeFinalizationProjectionV2,
    finalized_row_data_id: Identity32V1,
}

impl AuthenticatedGeneralOwnerSettlementTerminalV2 {
    /// Canonical writable owner-settlement PDA.
    pub const fn owner_settlement_account(self) -> Identity32V1 {
        self.owner_settlement_account
    }

    /// Canonical writable existing owner-fee PDA after its v2 transition.
    pub const fn owner_fee_finalization_account(self) -> Identity32V1 {
        self.owner_fee_finalization_account
    }

    /// Exact General runtime program owner shared by both accounts.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Semantic owner's immutable finalized-row expectation.
    pub const fn expectation(self) -> OwnerSettlementExpectationV2 {
        self.expectation
    }

    /// Exact fee-runtime terminal projection bound to this row.
    pub const fn fee(self) -> GeneralOwnerFeeFinalizationProjectionV2 {
        self.fee
    }

    /// SHA-256 identity of the exact finalized 288-byte row body.
    pub const fn finalized_row_data_id(self) -> Identity32V1 {
        self.finalized_row_data_id
    }
}

/// Authenticate a finalized `0x81/2` 288-byte semantic row and its 0x83/version-2
/// in-place fee receipt before any shrink, transfer, or delete starts.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_general_owner_settlement_terminal_v2<B: OwnerFinalizedRowDataHashV2>(
    owner_settlement_view: AccountViewV2<'_>,
    owner_settlement_pda: CanonicalPdaV1,
    owner_fee_view: AccountViewV2<'_>,
    owner_fee_pda: CanonicalPdaV1,
    program_id: Identity32V1,
    hash_backend: &B,
) -> Result<AuthenticatedGeneralOwnerSettlementTerminalV2, RetirementAdapterErrorV2> {
    let owner_settlement = authenticate_general_owner_settlement_v2_exact(
        owner_settlement_view,
        program_id,
        owner_settlement_pda,
    )?;
    let owner_fee = authenticate_general_owner_fee_finalization_v2_exact(
        owner_fee_view,
        program_id,
        owner_fee_pda,
    )?;
    if owner_settlement.address() == owner_fee.address() {
        return Err(RetirementErrorV2::AccountAlias.into());
    }

    let row = OwnerSettlementV2AccountV1::decode(owner_settlement.data())?;
    let row_terminal = row.retirement_projection()?;
    let expectation = row_terminal.expectation();
    let finalized_body = row_terminal.finalized_body();
    let finalized_row_data_id = Identity32V1::new(
        derive_owner_finalized_row_data_id_v2(finalized_body, hash_backend)
            .map_err(|_| RetirementAdapterErrorV2::InvalidSchema)?,
    )?;

    let fee = OwnerFeeFinalizationV2AccountV1::decode(owner_fee.data())?
        .terminal_projection(fee_runtime_id_from_bytes(owner_fee.address().bytes()))?;
    if fee.outcome != OwnerFeeFinalizationOutcomeV2::Settled
        || fee.owner_settlement_account.0 != owner_settlement.address().bytes()
        || fee.owner_settlement_final_data_id.0 != finalized_row_data_id.bytes()
        || fee.settlement_candidate.0 != expectation.candidate
        || fee.owner.0 != expectation.owner
        || fee.authorized_fee_atoms != expectation.selected_fee_atoms
        || fee.position.0 == owner_settlement.address().bytes()
        || fee.settlement_cash_pot.0 == owner_settlement.address().bytes()
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }

    Ok(AuthenticatedGeneralOwnerSettlementTerminalV2 {
        owner_settlement_account: owner_settlement.address(),
        owner_fee_finalization_account: owner_fee.address(),
        program_id,
        expectation,
        fee,
        finalized_row_data_id,
    })
}

/// Read-only canonical candidate-wide fee terminal and closure manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralFeeTerminalV1 {
    terminal_receipt_account: Identity32V1,
    closure_manifest_account: Identity32V1,
    receipt_program_id: Identity32V1,
    projection: GeneralFeeTerminalProjectionV1,
}

impl AuthenticatedGeneralFeeTerminalV1 {
    /// Canonical fee terminal receipt PDA.
    pub const fn terminal_receipt_account(self) -> Identity32V1 {
        self.terminal_receipt_account
    }

    /// Canonical aggregate closure-manifest receipt PDA.
    pub const fn closure_manifest_account(self) -> Identity32V1 {
        self.closure_manifest_account
    }

    /// Exact fee-runtime receipt program owner.
    pub const fn receipt_program_id(self) -> Identity32V1 {
        self.receipt_program_id
    }

    /// Typed General terminal projection from the fee semantic owner.
    pub const fn projection(self) -> GeneralFeeTerminalProjectionV1 {
        self.projection
    }
}

fn authenticate_read_only_receipt(
    view: AccountViewV2<'_>,
    canonical_pda: CanonicalPdaV1,
    expected_owner: Identity32V1,
    exact_len: usize,
) -> Result<(), RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.owner != expected_owner {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if view.is_writable {
        return Err(RetirementAdapterErrorV2::UnexpectedWritable);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    if view.data.len() < exact_len {
        return Err(RetirementErrorV2::Truncated.into());
    }
    if view.data.len() > exact_len {
        return Err(RetirementErrorV2::TrailingBytes.into());
    }
    Ok(())
}

/// Authenticate the exact 544-byte fee terminal and 224-byte closure
/// manifest bodies, their PDAs/owners, and their mutual aggregate bindings.
pub fn authenticate_general_fee_terminal_v1(
    terminal_view: AccountViewV2<'_>,
    terminal_pda: CanonicalPdaV1,
    manifest_view: AccountViewV2<'_>,
    manifest_pda: CanonicalPdaV1,
    receipt_program_id: Identity32V1,
) -> Result<AuthenticatedGeneralFeeTerminalV1, RetirementAdapterErrorV2> {
    authenticate_read_only_receipt(
        terminal_view,
        terminal_pda,
        receipt_program_id,
        FEE_TERMINAL_RECEIPT_V1_BYTES,
    )?;
    authenticate_read_only_receipt(
        manifest_view,
        manifest_pda,
        receipt_program_id,
        FEE_CLOSURE_MANIFEST_V1_BYTES,
    )?;
    if terminal_view.address == manifest_view.address {
        return Err(RetirementErrorV2::AccountAlias.into());
    }

    let bundle = FeeTerminalReceiptBundleV1::decode(manifest_view.data, terminal_view.data)
        .map_err(|_| RetirementAdapterErrorV2::InvalidSchema)?;
    let terminal = bundle.terminal();
    let manifest = bundle.closure_manifest();
    let terminal_projection = terminal.project_general();
    if terminal.terminal_receipt().0 != terminal_view.address.bytes()
        || manifest.receipt().0 != manifest_view.address.bytes()
        || terminal.runtime_program().0 != receipt_program_id.bytes()
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }

    Ok(AuthenticatedGeneralFeeTerminalV1 {
        terminal_receipt_account: terminal_view.address,
        closure_manifest_account: manifest_view.address,
        receipt_program_id,
        projection: terminal_projection,
    })
}

/// Exact selected-candidate join for a candidate-wide fee abort.
///
/// An abort has no settled FinalPot authority to authenticate. This projection
/// therefore binds the immutable SelectedCandidate directly to the paired fee
/// terminal/manifest and deliberately grants neither account-close nor value
/// movement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralFeeAbortV1 {
    selected_candidate_account: Identity32V1,
    selected_fee_record_account: Identity32V1,
    program_id: Identity32V1,
    fee_terminal: AuthenticatedGeneralFeeTerminalV1,
}

impl AuthenticatedGeneralFeeAbortV1 {
    /// Immutable SelectedCandidate account joined to this abort.
    pub const fn selected_candidate_account(self) -> Identity32V1 {
        self.selected_candidate_account
    }

    /// Exact General runtime program identity.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Canonically derived selected fee-record PDA consumed by the terminal.
    pub const fn selected_fee_record_account(self) -> Identity32V1 {
        self.selected_fee_record_account
    }

    /// Strictly authenticated paired terminal and closure manifest.
    pub const fn fee_terminal(self) -> AuthenticatedGeneralFeeTerminalV1 {
        self.fee_terminal
    }

    /// Terminal evidence is not a liveness capitalization source.
    pub const fn available_liveness_lamports(self) -> u64 {
        0
    }

    /// Terminal evidence is not Hoard principal.
    pub const fn available_hoard_atoms(self) -> u64 {
        0
    }

    /// Released authorization is not collected future fee revenue.
    pub const fn available_future_fee_atoms(self) -> u64 {
        0
    }
}

/// Authenticate the strict SelectedCandidate/fee-abort identity join.
pub fn authenticate_general_fee_abort_v1(
    selected_view: AccountViewV2<'_>,
    selected_pda: CanonicalPdaV1,
    selected_fee_record_pda: CanonicalPdaV1,
    program_id: Identity32V1,
    fee_terminal: AuthenticatedGeneralFeeTerminalV1,
) -> Result<AuthenticatedGeneralFeeAbortV1, RetirementAdapterErrorV2> {
    let selected = authenticate_general_selected_candidate_v1_exact(
        selected_view,
        program_id,
        selected_pda,
        AccountAccessV2::ReadOnly,
    )?;
    if fee_terminal.receipt_program_id != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if selected.address() == fee_terminal.terminal_receipt_account
        || selected.address() == fee_terminal.closure_manifest_account
        || selected.address() == selected_fee_record_pda.address()
        || selected_fee_record_pda.address() == fee_terminal.terminal_receipt_account
        || selected_fee_record_pda.address() == fee_terminal.closure_manifest_account
    {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    let selected_body = SelectedCandidateV1AccountV1::decode(selected.data())?;
    let fee = fee_terminal.projection;
    if fee.outcome != FeeTerminalOutcomeV1::Aborted
        || fee.market.0 != selected_body.market.bytes()
        || fee.epoch.0 != selected_body.epoch.bytes()
        || fee.settlement_candidate.0 != selected_body.settlement_candidate_id.bytes()
        || fee.fee_record.0 != selected_fee_record_pda.address().bytes()
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    Ok(AuthenticatedGeneralFeeAbortV1 {
        selected_candidate_account: selected.address(),
        selected_fee_record_account: selected_fee_record_pda.address(),
        program_id,
        fee_terminal,
    })
}

/// Exact zero-liability FinalPot joined to SelectedCandidate and candidate-wide
/// fee terminal authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralFinalPotTerminalV1 {
    final_pot_account: Identity32V1,
    selected_candidate_account: Identity32V1,
    selected_fee_record_account: Identity32V1,
    program_id: Identity32V1,
    terminal: FinalPotRetirementProjectionV1,
    fee_terminal: AuthenticatedGeneralFeeTerminalV1,
}

impl AuthenticatedGeneralFinalPotTerminalV1 {
    /// Canonical writable FinalPot PDA.
    pub const fn final_pot_account(self) -> Identity32V1 {
        self.final_pot_account
    }

    /// Read-only SelectedCandidate semantic authority.
    pub const fn selected_candidate_account(self) -> Identity32V1 {
        self.selected_candidate_account
    }

    /// Exact General program owner shared by FinalPot and SelectedCandidate.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Canonically derived selected fee-record PDA consumed by the terminal.
    pub const fn selected_fee_record_account(self) -> Identity32V1 {
        self.selected_fee_record_account
    }

    /// Zero-liability projection minted only by the FinalPot semantic owner.
    pub const fn terminal(self) -> FinalPotRetirementProjectionV1 {
        self.terminal
    }

    /// Candidate-wide fee terminal bound to this same selection.
    pub const fn fee_terminal(self) -> AuthenticatedGeneralFeeTerminalV1 {
        self.fee_terminal
    }
}

/// Authenticate FinalPot, SelectedCandidate, and candidate-wide fee terminal
/// before any close or Epoch count decrement.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_general_final_pot_terminal_v1(
    final_pot_view: AccountViewV2<'_>,
    final_pot_pda: CanonicalPdaV1,
    selected_view: AccountViewV2<'_>,
    selected_pda: CanonicalPdaV1,
    selected_fee_record_pda: CanonicalPdaV1,
    program_id: Identity32V1,
    fee_terminal: AuthenticatedGeneralFeeTerminalV1,
) -> Result<AuthenticatedGeneralFinalPotTerminalV1, RetirementAdapterErrorV2> {
    let final_pot =
        authenticate_general_final_pot_v1_exact(final_pot_view, program_id, final_pot_pda)?;
    let selected = authenticate_general_selected_candidate_v1_exact(
        selected_view,
        program_id,
        selected_pda,
        AccountAccessV2::ReadOnly,
    )?;
    if fee_terminal.receipt_program_id != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if final_pot.address() == selected.address()
        || selected.address() == fee_terminal.terminal_receipt_account
        || selected.address() == fee_terminal.closure_manifest_account
        || final_pot.address() == fee_terminal.terminal_receipt_account
        || final_pot.address() == fee_terminal.closure_manifest_account
        || selected.address() == selected_fee_record_pda.address()
        || final_pot.address() == selected_fee_record_pda.address()
        || selected_fee_record_pda.address() == fee_terminal.terminal_receipt_account
        || selected_fee_record_pda.address() == fee_terminal.closure_manifest_account
    {
        return Err(RetirementErrorV2::AccountAlias.into());
    }

    let selected_body = SelectedCandidateV1AccountV1::decode(selected.data())?;
    let selected_binding = AuthenticatedSelectedCandidateV1 {
        artifact: general_id(selected.address())?,
        account: &selected_body,
    };
    let binding = FinalPotAdapterBindingV1 {
        final_pot: general_id(final_pot.address())?,
        derived_bump: final_pot.bump(),
        selected: selected_binding,
        final_pot_pda_authenticated: true,
        final_pot_program_owner_authenticated: true,
        selected_pda_authenticated: true,
        selected_program_owner_authenticated: true,
        writable: true,
    };
    let terminal =
        FinalPotV1AccountV1::decode(final_pot.data(), binding)?.retirement_projection(binding)?;
    let fee = fee_terminal.projection;
    if fee.outcome != FeeTerminalOutcomeV1::Settled
        || fee.market.0 != terminal.market()
        || fee.epoch.0 != terminal.epoch()
        || fee.settlement_candidate.0 != terminal.candidate()
        || fee.fee_record.0 != selected_fee_record_pda.address().bytes()
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }

    Ok(AuthenticatedGeneralFinalPotTerminalV1 {
        final_pot_account: final_pot.address(),
        selected_candidate_account: selected.address(),
        selected_fee_record_account: selected_fee_record_pda.address(),
        program_id,
        terminal,
        fee_terminal,
    })
}

/// Fresh Epoch V6's three authoritative counters after its semantic owner
/// proves terminal phase and zero children in the currently represented
/// candidate/work/selected families.
///
/// This cannot stand in for absent Receipt/Reservation/Page counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralEpochTerminalCountsV1 {
    epoch_account: Identity32V1,
    program_id: Identity32V1,
    market: Identity32V1,
    generation: u64,
}

impl AuthenticatedGeneralEpochTerminalCountsV1 {
    /// Canonical writable fresh Epoch PDA.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch_account
    }

    /// Exact General program owner.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Full General MarketRuntime identity stored by Epoch V6.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Nonzero retirement generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Authenticate the fresh Epoch's currently authoritative zero counts.
pub fn authenticate_general_epoch_terminal_counts_v1(
    epoch_view: AccountViewV2<'_>,
    epoch_pda: CanonicalPdaV1,
    program_id: Identity32V1,
) -> Result<AuthenticatedGeneralEpochTerminalCountsV1, RetirementAdapterErrorV2> {
    let authenticated = authenticate_general_epoch_v6_exact(
        epoch_view,
        program_id,
        epoch_pda,
        AccountAccessV2::Writable,
    )?;
    let epoch = GeneralEpochV6AccountV1::decode(authenticated.data())?;
    if epoch.phase != GeneralEpochPhaseV1::Finalized
        || epoch.candidate_bundle_count != 0
        || epoch.work_count != 0
        || epoch.selected_candidate_count != 0
    {
        return Err(RetirementErrorV2::ChildOutstanding.into());
    }
    Ok(AuthenticatedGeneralEpochTerminalCountsV1 {
        epoch_account: authenticated.address(),
        program_id,
        market: identity(epoch.market_runtime)?,
        generation: epoch.generation,
    })
}

/// Exact program-owned terminal `0xa9/1` root promoted from the structural
/// contract projection.
///
/// This is candidate-scoped settlement terminality. It is not by itself
/// General Epoch, Product occurrence, MarketInstance, or rent-close authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralSettlementRootTerminalV1 {
    root_account: Identity32V1,
    program_id: Identity32V1,
    projection: SettlementRootTerminalProjectionV1,
}

impl AuthenticatedGeneralSettlementRootTerminalV1 {
    /// Canonical writable SettlementRoot PDA.
    pub const fn root_account(self) -> Identity32V1 {
        self.root_account
    }

    /// Exact General runtime program owner.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Exhaustive candidate-scoped terminal projection.
    pub const fn projection(self) -> SettlementRootTerminalProjectionV1 {
        self.projection
    }

    /// Settlement terminality never capitalizes future liveness work.
    pub const fn available_liveness_lamports(self) -> u64 {
        0
    }

    /// Settlement terminality never releases Hoard principal.
    pub const fn available_hoard_atoms(self) -> u64 {
        0
    }

    /// Settlement terminality is not future fee revenue.
    pub const fn available_future_fee_atoms(self) -> u64 {
        0
    }
}

/// Authenticate the exact terminal SettlementRoot PDA, General program owner,
/// writable role, frozen 980-byte body, stored bump, and semantic zero-count
/// projection before promoting it into candidate-scoped terminal authority.
pub fn authenticate_general_settlement_root_terminal_v1<B: Sha256BackendV1>(
    root_view: AccountViewV2<'_>,
    root_pda: CanonicalPdaV1,
    program_id: Identity32V1,
    hash_backend: &B,
) -> Result<AuthenticatedGeneralSettlementRootTerminalV1, RetirementAdapterErrorV2> {
    let authenticated = authenticate_general_settlement_root_v1_exact(
        root_view,
        program_id,
        root_pda,
        AccountAccessV2::Writable,
    )?;
    let root = SettlementRootV1AccountV1::decode(authenticated.data())?;
    let projection =
        root.terminal_projection(hash_backend, general_id(authenticated.address())?)?;
    if projection.root_account().bytes() != authenticated.address().bytes() {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    Ok(AuthenticatedGeneralSettlementRootTerminalV1 {
        root_account: authenticated.address(),
        program_id,
        projection,
    })
}
