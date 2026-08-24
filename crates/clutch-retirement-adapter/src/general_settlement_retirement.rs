// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical General V2 settlement retirement joins.
//!
//! This module authenticates existing semantic owners and emits only
//! private-field terminal capabilities. It deliberately does not move
//! lamports or delete accounts. The indexed counted `0xa9/2` SettlementRoot owns
//! the exhaustive candidate-scoped Receipt/owner-row/Reservation/fee/pot and
//! Dealer-child graph. Its exact terminal account may be promoted here, but
//! whole-Epoch and whole-Market close authority still requires the separate
//! page, occurrence, Source, Failure, Position, and collateral terminal joins.

use clutch_general_v2_contract::{
    FeeTerminalReceiptBundleV1, GeneralEpochPhaseV1, GeneralEpochV6AccountV1,
    GeneralFeeTerminalProjectionV1, Id32,
    IndexedSettlementRootTerminalProjectionV1, IndexedSettlementRootV1AccountV1,
    Sha256BackendV1, FEE_CLOSURE_MANIFEST_V1_BYTES, FEE_TERMINAL_RECEIPT_V1_BYTES,
};
use clutch_retirement::{Identity32V1, RetirementErrorV2};

use crate::{
    authenticate_general_epoch_v6_exact, authenticate_general_indexed_settlement_root_v1_exact,
    AccountAccessV2, AccountViewV2, CanonicalPdaV1, RetirementAdapterErrorV2,
};

fn identity(value: Id32) -> Result<Identity32V1, RetirementAdapterErrorV2> {
    Identity32V1::new(value.bytes()).map_err(Into::into)
}

fn general_id(value: Identity32V1) -> Result<Id32, RetirementAdapterErrorV2> {
    Id32::new(value.bytes()).map_err(Into::into)
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
    projection: IndexedSettlementRootTerminalProjectionV1,
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
    pub const fn projection(self) -> IndexedSettlementRootTerminalProjectionV1 {
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

/// Authenticate the exact terminal indexed SettlementRoot PDA, General program
/// owner, writable role, 1,228-byte body, stored bump, retired exact-index
/// children, and semantic zero-count projection before promoting it into
/// candidate-scoped terminal authority.
pub fn authenticate_general_settlement_root_terminal_v1<B: Sha256BackendV1>(
    root_view: AccountViewV2<'_>,
    root_pda: CanonicalPdaV1,
    program_id: Identity32V1,
    hash_backend: &B,
) -> Result<AuthenticatedGeneralSettlementRootTerminalV1, RetirementAdapterErrorV2> {
    let authenticated = authenticate_general_indexed_settlement_root_v1_exact(
        root_view,
        program_id,
        root_pda,
        AccountAccessV2::Writable,
    )?;
    let root = IndexedSettlementRootV1AccountV1::decode(authenticated.data())?;
    let projection =
        root.terminal_projection(hash_backend, general_id(authenticated.address())?)?;
    if projection.base().root_account().bytes() != authenticated.address().bytes() {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    Ok(AuthenticatedGeneralSettlementRootTerminalV1 {
        root_account: authenticated.address(),
        program_id,
        projection,
    })
}
