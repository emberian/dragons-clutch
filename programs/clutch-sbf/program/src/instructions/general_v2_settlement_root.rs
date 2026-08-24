//! Capability-disabled authentication seam for the counted General settlement root.
//!
//! The `0xa9/1` and disabled `0xa9/2` roots are the candidate-scoped owners of
//! every live settlement child count. This module authenticates either exact
//! version from the General program owner, canonical epoch/candidate seed
//! tuple, stored bump, exact width, and full semantic decoder. Version-aware
//! writers expose only named action transitions, so an indexed suffix cannot
//! be dropped by a legacy 980-byte rewrite. There is no dispatcher route.

use core::cell::Ref;
use std::boxed::Box;

use clutch_general_v2_contract::{
    Id32, IndexedSettlementRootV1AccountV1, SettlementRootV1AccountV1, Sha256BackendV1,
    INDEXED_SETTLEMENT_ROOT_BYTES_V1, SETTLEMENT_ROOT_ACCOUNT_BYTES,
};
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
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralSettlementRootV1 {
    account: Id32,
    body: AuthenticatedGeneralSettlementRootBodyV1,
}

#[derive(Debug, Eq, PartialEq)]
enum AuthenticatedGeneralSettlementRootBodyV1 {
    Legacy(Box<SettlementRootV1AccountV1>),
    Indexed(Box<IndexedSettlementRootV1AccountV1>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NamedRootTransitionV1 {
    AdmitMaterialization {
        owner_rows_created: u8,
        reservations_admitted: u8,
        merge_receipt: bool,
    },
    ReleaseUnfilledReservation,
    ActivateMergeCashPot,
    CompleteOwnerFinalization { fee_receipt_created: bool },
    CompleteMergePayment,
    RetirePortfolioPairArchives { receipt_count: u8 },
    AdmitDealerChild,
    RetireDealerChild,
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
    pub fn root(&self) -> &SettlementRootV1AccountV1 {
        match &self.body {
            AuthenticatedGeneralSettlementRootBodyV1::Legacy(root) => root,
            AuthenticatedGeneralSettlementRootBodyV1::Indexed(root) => root.base(),
        }
    }

    /// Exact authenticated account width for this root version.
    pub fn account_bytes(&self) -> usize {
        match &self.body {
            AuthenticatedGeneralSettlementRootBodyV1::Legacy(_) => {
                SETTLEMENT_ROOT_ACCOUNT_BYTES
            }
            AuthenticatedGeneralSettlementRootBodyV1::Indexed(_) => {
                INDEXED_SETTLEMENT_ROOT_BYTES_V1
            }
        }
    }

    /// Whether this is the disabled counted exact-index successor.
    pub fn is_indexed(&self) -> bool {
        matches!(&self.body, AuthenticatedGeneralSettlementRootBodyV1::Indexed(_))
    }

    /// Version-specific full root body ID. Indexed roots never collapse to
    /// the legacy base transcript.
    pub fn data_id<B: Sha256BackendV1>(&self, backend: &B) -> Outcome<Id32> {
        match &self.body {
            AuthenticatedGeneralSettlementRootBodyV1::Legacy(root) => {
                root.data_id(backend, self.account).map_err(Into::into)
            }
            AuthenticatedGeneralSettlementRootBodyV1::Indexed(root) => {
                root.data_id(backend, self.account).map_err(Into::into)
            }
        }
    }

    /// Encode exactly one action-24 materialization successor while preserving
    /// an authenticated indexed suffix when present.
    pub fn encode_materialization_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        let before = self.root().counts();
        let after = expected.counts();
        let owner_rows_created = u8::try_from(
            after
                .admitted_owner_rows
                .checked_sub(before.admitted_owner_rows)
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let reservations_admitted = u8::try_from(after
            .admitted_reservations
            .checked_sub(before.admitted_reservations)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let merge_delta = after
            .admitted_merge_payments
            .checked_sub(before.admitted_merge_payments)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require(merge_delta <= 1, ClutchError::MismatchedState)?;
        self.encode_named_successor(
            expected,
            NamedRootTransitionV1::AdmitMaterialization {
                owner_rows_created,
                reservations_admitted,
                merge_receipt: merge_delta == 1,
            },
            output,
        )
    }

    /// Encode exactly one authenticated zero-fill Reservation release.
    pub fn encode_unfilled_release_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(
            expected,
            NamedRootTransitionV1::ReleaseUnfilledReservation,
            output,
        )
    }

    /// Encode the unique action-37 merge cash-pot activation successor.
    pub fn encode_merge_cash_activation_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(expected, NamedRootTransitionV1::ActivateMergeCashPot, output)
    }

    /// Encode exactly one owner-finalization successor.
    pub fn encode_owner_finalization_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        fee_receipt_created: bool,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(
            expected,
            NamedRootTransitionV1::CompleteOwnerFinalization {
                fee_receipt_created,
            },
            output,
        )
    }

    /// Encode exactly one merge-payment latch successor.
    pub fn encode_merge_payment_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(
            expected,
            NamedRootTransitionV1::CompleteMergePayment,
            output,
        )
    }

    /// Encode one authenticated portfolio archive-pair retirement successor.
    pub fn encode_portfolio_retirement_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        receipt_count: u8,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(
            expected,
            NamedRootTransitionV1::RetirePortfolioPairArchives { receipt_count },
            output,
        )
    }

    /// Encode exactly one Dealer child admission successor.
    pub fn encode_dealer_admission_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(expected, NamedRootTransitionV1::AdmitDealerChild, output)
    }

    /// Encode exactly one Dealer child retirement successor.
    pub fn encode_dealer_retirement_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        self.encode_named_successor(expected, NamedRootTransitionV1::RetireDealerChild, output)
    }

    fn encode_named_successor(
        &self,
        expected: &SettlementRootV1AccountV1,
        transition: NamedRootTransitionV1,
        output: &mut [u8],
    ) -> Outcome<()> {
        require(output.len() == self.account_bytes(), ClutchError::WrongDataLength)?;
        match &self.body {
            AuthenticatedGeneralSettlementRootBodyV1::Legacy(root) => {
                let successor = apply_legacy_transition(root, transition)?;
                require(&successor == expected, ClutchError::MismatchedState)?;
                successor.encode(output)?;
            }
            AuthenticatedGeneralSettlementRootBodyV1::Indexed(root) => {
                let successor = apply_indexed_transition(root, transition)?;
                require(successor.base() == expected, ClutchError::MismatchedState)?;
                successor.encode(output)?;
            }
        }
        Ok(())
    }
}

fn apply_legacy_transition(
    root: &SettlementRootV1AccountV1,
    transition: NamedRootTransitionV1,
) -> Result<SettlementRootV1AccountV1, clutch_general_v2_contract::CodecError> {
    match transition {
        NamedRootTransitionV1::AdmitMaterialization {
            owner_rows_created,
            reservations_admitted,
            merge_receipt,
        } => root.admit_materialization_delta(
            owner_rows_created,
            reservations_admitted,
            merge_receipt,
        ),
        NamedRootTransitionV1::ReleaseUnfilledReservation => {
            root.release_unfilled_reservation()
        }
        NamedRootTransitionV1::ActivateMergeCashPot => {
            Ok(*clutch_general_v2_contract::prepare_activate_merge_cash_pot_v1(root)?.root())
        }
        NamedRootTransitionV1::CompleteOwnerFinalization {
            fee_receipt_created,
        } => root.complete_owner_finalization(fee_receipt_created),
        NamedRootTransitionV1::CompleteMergePayment => root.complete_merge_payment(),
        NamedRootTransitionV1::RetirePortfolioPairArchives { receipt_count } => {
            root.retire_portfolio_pair_archives(receipt_count)
        }
        NamedRootTransitionV1::AdmitDealerChild => root.admit_dealer_child(),
        NamedRootTransitionV1::RetireDealerChild => root.retire_dealer_child(),
    }
}

fn apply_indexed_transition(
    root: &IndexedSettlementRootV1AccountV1,
    transition: NamedRootTransitionV1,
) -> Result<IndexedSettlementRootV1AccountV1, clutch_general_v2_contract::CodecError> {
    match transition {
        NamedRootTransitionV1::AdmitMaterialization {
            owner_rows_created,
            reservations_admitted,
            merge_receipt,
        } => root.admit_materialization(
            owner_rows_created,
            reservations_admitted,
            merge_receipt,
        ),
        NamedRootTransitionV1::ReleaseUnfilledReservation => {
            root.release_unfilled_reservation()
        }
        NamedRootTransitionV1::ActivateMergeCashPot => root.activate_merge_cash_pot(),
        NamedRootTransitionV1::CompleteOwnerFinalization {
            fee_receipt_created,
        } => root.complete_owner_finalization(fee_receipt_created),
        NamedRootTransitionV1::CompleteMergePayment => root.complete_merge_payment(),
        NamedRootTransitionV1::RetirePortfolioPairArchives { receipt_count } => {
            root.retire_portfolio_pair_archives(receipt_count)
        }
        NamedRootTransitionV1::AdmitDealerChild => root.admit_dealer_child(),
        NamedRootTransitionV1::RetireDealerChild => root.retire_dealer_child(),
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
        Some(expected_candidate),
        RootAccessV1::Writable,
    )
}

/// Authenticate one writable root from an independently authenticated Epoch;
/// the candidate coordinate is accepted only from the decoded root and the
/// canonical PDA derived from that body.
pub fn authenticate_writable_general_settlement_root_epoch_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    authenticate_general_settlement_root_v1(
        program_id,
        accounts,
        expected_epoch,
        None,
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
        Some(expected_candidate),
        RootAccessV1::ReadOnly,
    )
}

/// Authenticate one read-only root from an independently authenticated Epoch;
/// the candidate coordinate is accepted only from the decoded root and the
/// canonical PDA derived from that body.
pub fn authenticate_readonly_general_settlement_root_epoch_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    authenticate_general_settlement_root_v1(
        program_id,
        accounts,
        expected_epoch,
        None,
        RootAccessV1::ReadOnly,
    )
}

fn authenticate_general_settlement_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
    expected_candidate: Option<Id32>,
    access: RootAccessV1,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    require_count(accounts, SETTLEMENT_ROOT_AUTH_ACCOUNT_COUNT_V1)?;
    let account = &accounts[IX_SETTLEMENT_ROOT];
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    match access {
        RootAccessV1::ReadOnly => {
            require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        }
        RootAccessV1::Writable => {
            require(account.is_writable, ClutchError::NotWritable)?;
        }
    }
    let root_account = id(account.key);
    let body = borrow_data(account)?;
    let decoded = match body.len() {
        SETTLEMENT_ROOT_ACCOUNT_BYTES => AuthenticatedGeneralSettlementRootBodyV1::Legacy(
            Box::new(SettlementRootV1AccountV1::decode(&body)?),
        ),
        INDEXED_SETTLEMENT_ROOT_BYTES_V1 => {
            AuthenticatedGeneralSettlementRootBodyV1::Indexed(Box::new(
                IndexedSettlementRootV1AccountV1::decode(&body)?,
            ))
        }
        _ => return Err(Refusal::Adapter(ClutchError::WrongDataLength)),
    };
    let root = match &decoded {
        AuthenticatedGeneralSettlementRootBodyV1::Legacy(root) => &**root,
        AuthenticatedGeneralSettlementRootBodyV1::Indexed(root) => root.base(),
    };
    let canonical = seeds::general_v2_settlement_root_pda(
        program_id,
        &expected_epoch.bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    require(
        *account.key == canonical.0 && root.stored_bump() == canonical.1,
        ClutchError::WrongPda,
    )?;
    require(
        root.epoch() == expected_epoch
            && expected_candidate
                .map(|candidate| root.settlement_candidate_id() == candidate)
                .unwrap_or(true),
        ClutchError::MismatchedState,
    )?;

    Ok(AuthenticatedGeneralSettlementRootV1 {
        account: root_account,
        body: decoded,
    })
}
