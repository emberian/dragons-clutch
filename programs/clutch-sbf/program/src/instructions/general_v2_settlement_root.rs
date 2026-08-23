//! Capability-disabled authentication seam for the counted General settlement root.
//!
//! The `0xa9/1` root is the candidate-scoped owner of every live settlement
//! child count. This module authenticates one existing writable root from the
//! exact General program owner, canonical epoch/candidate seed tuple, stored
//! bump, frozen width, and full semantic decoder. It does not expose a
//! dispatcher route or construct action-39 expectations from caller integers.

use core::cell::Ref;

use clutch_general_v2_contract::{Id32, SettlementRootV1AccountV1, SETTLEMENT_ROOT_ACCOUNT_BYTES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

/// Exact one-account authentication frame for an existing root.
pub const SETTLEMENT_ROOT_AUTH_ACCOUNT_COUNT_V1: usize = 1;
/// Writable counted SettlementRoot PDA.
pub const IX_SETTLEMENT_ROOT: usize = 0;

/// Existing program-owned root whose complete account body and PDA passed the
/// SBF adapter checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralSettlementRootV1 {
    account: Id32,
    root: SettlementRootV1AccountV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootAccessV1 {
    ReadOnly,
    Writable,
}

impl AuthenticatedGeneralSettlementRootV1 {
    /// Canonical writable `0xa9/1` account.
    pub const fn account(&self) -> Id32 {
        self.account
    }

    /// Exact hostile-byte-decoded semantic root.
    pub const fn root(&self) -> &SettlementRootV1AccountV1 {
        &self.root
    }
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

/// Authenticate one existing mutable settlement root.
///
/// `expected_epoch` and `expected_candidate` must already come from the
/// instruction's independently authenticated semantic parents. This function
/// never derives either identity from caller-provided account bytes alone.
pub fn authenticate_writable_general_settlement_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
    expected_candidate: Id32,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    authenticate_general_settlement_root_v1(
        program_id,
        accounts,
        expected_epoch,
        expected_candidate,
        RootAccessV1::Writable,
    )
}

/// Authenticate one existing immutable settlement root.
///
/// This is the disjoint read-only sibling of
/// [`authenticate_writable_general_settlement_root_v1`]; callers cannot pass a
/// boolean or promote an arbitrary decoded body into account authority.
pub fn authenticate_readonly_general_settlement_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
    expected_candidate: Id32,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    authenticate_general_settlement_root_v1(
        program_id,
        accounts,
        expected_epoch,
        expected_candidate,
        RootAccessV1::ReadOnly,
    )
}

fn authenticate_general_settlement_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
    expected_candidate: Id32,
    access: RootAccessV1,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    require_count(accounts, SETTLEMENT_ROOT_AUTH_ACCOUNT_COUNT_V1)?;
    let account = &accounts[IX_SETTLEMENT_ROOT];
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    match access {
        RootAccessV1::ReadOnly => {
            require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        }
        RootAccessV1::Writable => {
            require(account.is_writable, ClutchError::NotWritable)?;
        }
    }
    require(
        account.data_len() == SETTLEMENT_ROOT_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;

    let root_account = id(account.key);
    let root = SettlementRootV1AccountV1::decode(&borrow_data(account)?)?;
    let canonical = seeds::general_v2_settlement_root_pda(
        program_id,
        &expected_epoch.bytes(),
        &expected_candidate.bytes(),
    );
    require(
        *account.key == canonical.0 && root.stored_bump() == canonical.1,
        ClutchError::WrongPda,
    )?;
    require(
        root.epoch() == expected_epoch && root.settlement_candidate_id() == expected_candidate,
        ClutchError::MismatchedState,
    )?;

    Ok(AuthenticatedGeneralSettlementRootV1 {
        account: root_account,
        root,
    })
}
