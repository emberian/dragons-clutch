//! Compact hostile authentication for the durable General fee terminal pair.
//!
//! `0xb9/v2` and `0xb9/v3` survive candidate fee retirement so Dealer and the
//! final indexed-root close can consume the same immutable evidence.  This
//! adapter authenticates the exact persisted outers and then retains only the
//! compact projections and rent facts needed by those consumers.  It cannot
//! be constructed from an in-memory terminal bundle.

use core::cell::Ref;

use clutch_fee_runtime_contract::retirement::{
    FeeRetirementHashV1, FEE_RETIREMENT_AUTHORITY_DOMAIN_V1,
};
use clutch_fee_runtime_contract::projection::SelectedOwnerFeeBookHashV1;
use clutch_fee_runtime_contract::terminal::{
    DealerFeeTerminalProjectionV1, FeeTerminalOutcomeV1, FeeTerminalReceiptBundleV2,
    GeneralFeeTerminalProjectionV1,
};
use clutch_fee_runtime_contract::Id as FeeId;
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{DeletableRentOwnerV1, Id32};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl contract::Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl SelectedOwnerFeeBookHashV1 for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl FeeRetirementHashV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact root-owned coordinates against which the durable pair is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FeeTerminalPairExpectationV1 {
    pub fee_record: Id32,
    pub settlement_root: Id32,
    pub selected_feed_data_id: Id32,
    pub market: Id32,
    pub epoch: Id32,
    pub settlement_candidate: Id32,
}

/// Compact non-forgeable authority derived only from two persisted accounts.
pub(crate) struct AuthenticatedFeeTerminalPairV1 {
    manifest_account: Id32,
    terminal_account: Id32,
    manifest_account_data_id: Id32,
    terminal_account_data_id: Id32,
    terminal_semantic_data_id: Id32,
    closure_set_data_id: Id32,
    manifest_rent: DeletableRentOwnerV1,
    terminal_rent: DeletableRentOwnerV1,
    manifest_observed_balance_lamports: u64,
    terminal_observed_balance_lamports: u64,
    general: GeneralFeeTerminalProjectionV1,
    dealer: DealerFeeTerminalProjectionV1,
}

impl AuthenticatedFeeTerminalPairV1 {
    pub(crate) const fn manifest_account(&self) -> Id32 {
        self.manifest_account
    }

    pub(crate) const fn terminal_account(&self) -> Id32 {
        self.terminal_account
    }

    pub(crate) const fn manifest_account_data_id(&self) -> Id32 {
        self.manifest_account_data_id
    }

    pub(crate) const fn terminal_account_data_id(&self) -> Id32 {
        self.terminal_account_data_id
    }

    pub(crate) const fn terminal_semantic_data_id(&self) -> Id32 {
        self.terminal_semantic_data_id
    }

    pub(crate) const fn closure_set_data_id(&self) -> Id32 {
        self.closure_set_data_id
    }

    pub(crate) const fn manifest_rent(&self) -> DeletableRentOwnerV1 {
        self.manifest_rent
    }

    pub(crate) const fn terminal_rent(&self) -> DeletableRentOwnerV1 {
        self.terminal_rent
    }

    pub(crate) const fn manifest_observed_balance_lamports(&self) -> u64 {
        self.manifest_observed_balance_lamports
    }

    pub(crate) const fn terminal_observed_balance_lamports(&self) -> u64 {
        self.terminal_observed_balance_lamports
    }

    pub(crate) const fn general(&self) -> GeneralFeeTerminalProjectionV1 {
        self.general
    }

    pub(crate) const fn dealer(&self) -> DealerFeeTerminalProjectionV1 {
        self.dealer
    }
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}

fn checked_persisted_balance(account: &AccountInfo<'_>, rent: DeletableRentOwnerV1) -> Outcome<u64> {
    rent.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let minimum = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let observed = account.lamports();
    require(observed >= minimum, ClutchError::MismatchedState)?;
    Ok(observed)
}

fn terminal_authority_receipt(
    accumulator_account: Id32,
    expectation: FeeTerminalPairExpectationV1,
    closure_set_data_id: Id32,
    value_disposition_receipt: FeeId,
) -> FeeId {
    FeeId(
        solana_sha256_hasher::hashv(&[
            FEE_RETIREMENT_AUTHORITY_DOMAIN_V1,
            &accumulator_account.bytes(),
            &expectation.settlement_root.bytes(),
            &expectation.selected_feed_data_id.bytes(),
            &closure_set_data_id.bytes(),
            &value_disposition_receipt.0,
        ])
        .to_bytes(),
    )
}

/// Authenticate the exact immutable durable fee pair.
///
/// `writable` is an exact access contract: read consumers cannot smuggle write
/// locks, while the final indexed-root close must name both accounts writable.
#[inline(never)]
pub(crate) fn authenticate_fee_terminal_pair_v1(
    program_id: &Pubkey,
    manifest_account: &AccountInfo<'_>,
    terminal_account: &AccountInfo<'_>,
    expectation: FeeTerminalPairExpectationV1,
    writable: bool,
) -> Outcome<AuthenticatedFeeTerminalPairV1> {
    require(
        manifest_account.key != terminal_account.key
            && *manifest_account.owner == *program_id
            && *terminal_account.owner == *program_id
            && manifest_account.data_len() == contract::FEE_RETIREMENT_ACCOUNT_BYTES_V2
            && terminal_account.data_len() == contract::FEE_RETIREMENT_ACCOUNT_BYTES_V3
            && manifest_account.is_writable == writable
            && terminal_account.is_writable == writable
            && !manifest_account.is_signer
            && !terminal_account.is_signer
            && !manifest_account.executable
            && !terminal_account.executable,
        ClutchError::MismatchedState,
    )?;
    let manifest_pda = seeds::general_v2_fee_closure_manifest_pda(
        program_id,
        &expectation.fee_record.bytes(),
    );
    let terminal_pda = seeds::general_v2_fee_terminal_receipt_pda(
        program_id,
        &expectation.fee_record.bytes(),
    );
    require(
        *manifest_account.key == manifest_pda.0 && *terminal_account.key == terminal_pda.0,
        ClutchError::MismatchedState,
    )?;

    let manifest_bytes = borrow_data(manifest_account)?;
    let manifest_account_data_id = contract::fee_closure_manifest_account_data_id_v2(
        &manifest_bytes,
        &RuntimeSha256,
    )?;
    let manifest = Box::new(contract::FeeClosureManifestV2AccountV1::decode(
        &manifest_bytes,
    )?);
    let terminal_bytes = borrow_data(terminal_account)?;
    let terminal_account_data_id = contract::fee_terminal_account_data_id_v3(
        &terminal_bytes,
        &RuntimeSha256,
    )?;
    let terminal = Box::new(contract::FeeRecordTerminalV3AccountV1::decode(
        &terminal_bytes,
    )?);
    require(
        manifest.stored_bump == manifest_pda.1 && terminal.stored_bump == terminal_pda.1,
        ClutchError::MismatchedState,
    )?;
    FeeTerminalReceiptBundleV2 {
        closure_manifest: manifest.semantic,
        terminal: terminal.semantic,
    }
    .validate(&RuntimeSha256)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let general = terminal.semantic.project_general();
    let dealer = terminal.semantic.project_dealer();
    let runtime_release = contract::fee_runtime_semantic_release_id_v2(&RuntimeSha256)?;
    let selected_pda = seeds::general_v2_selected_fee_record_pda(
        program_id,
        &expectation.settlement_candidate.bytes(),
    );
    let recipient_pda = seeds::general_v2_recipient_allocation_pda(
        program_id,
        &expectation.fee_record.bytes(),
    );
    let treasury_pda = seeds::general_v2_treasury_ledger_pda(
        program_id,
        &expectation.fee_record.bytes(),
    );
    let accumulator_pda = seeds::general_v2_fee_retirement_accumulator_pda(
        program_id,
        &expectation.fee_record.bytes(),
    );
    let closure_set_data_id = Id32::from_bytes(manifest.semantic.closure_set_data_id().0);
    let terminal_semantic_data_id = Id32::from_bytes(manifest.semantic.terminal_data_id().0);
    require(
        manifest.semantic.receipt().0 == manifest_account.key.to_bytes()
            && manifest.semantic.terminal_receipt().0 == terminal_account.key.to_bytes()
            && terminal.semantic.terminal_receipt().0 == terminal_account.key.to_bytes()
            && terminal.semantic.closure_manifest().0 == manifest_account.key.to_bytes()
            && manifest.semantic.runtime_program().0 == program_id.to_bytes()
            && terminal.semantic.runtime_program().0 == program_id.to_bytes()
            && manifest.semantic.runtime_release().0 == runtime_release.bytes()
            && terminal.semantic.runtime_release().0 == runtime_release.bytes()
            && manifest.semantic.fee_record().0 == expectation.fee_record.bytes()
            && terminal.semantic.fee_record().0 == expectation.fee_record.bytes()
            && manifest.semantic.selected_record().0 == selected_pda.0.to_bytes()
            && manifest.semantic.recipient_allocation().0 == recipient_pda.0.to_bytes()
            && manifest.semantic.treasury_ledger().0 == treasury_pda.0.to_bytes()
            && manifest.semantic.retirement_accumulator().0 == accumulator_pda.0.to_bytes()
            && general.market.0 == expectation.market.bytes()
            && general.epoch.0 == expectation.epoch.bytes()
            && general.settlement_candidate.0 == expectation.settlement_candidate.bytes()
            && general.fee_record.0 == expectation.fee_record.bytes()
            && general.outcome == FeeTerminalOutcomeV1::Settled
            && dealer.outcome == FeeTerminalOutcomeV1::Settled
            && manifest.semantic.terminal_authority_receipt()
                == terminal_authority_receipt(
                    Id32::from_bytes(accumulator_pda.0.to_bytes()),
                    expectation,
                    closure_set_data_id,
                    general.value_disposition_receipt,
                ),
        ClutchError::MismatchedState,
    )?;
    let manifest_observed_balance_lamports =
        checked_persisted_balance(manifest_account, manifest.rent)?;
    let terminal_observed_balance_lamports =
        checked_persisted_balance(terminal_account, terminal.rent)?;
    Ok(AuthenticatedFeeTerminalPairV1 {
        manifest_account: Id32::from_bytes(manifest_account.key.to_bytes()),
        terminal_account: Id32::from_bytes(terminal_account.key.to_bytes()),
        manifest_account_data_id,
        terminal_account_data_id,
        terminal_semantic_data_id,
        closure_set_data_id,
        manifest_rent: manifest.rent,
        terminal_rent: terminal.rent,
        manifest_observed_balance_lamports,
        terminal_observed_balance_lamports,
        general,
        dealer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::from_bytes([byte; 32])
    }

    #[test]
    fn terminal_authority_binds_root_and_selected_feed_independently() {
        let base = FeeTerminalPairExpectationV1 {
            fee_record: id(1),
            settlement_root: id(2),
            selected_feed_data_id: id(3),
            market: id(4),
            epoch: id(5),
            settlement_candidate: id(6),
        };
        let authority = terminal_authority_receipt(id(7), base, id(8), FeeId([9; 32]));
        let mut wrong_root = base;
        wrong_root.settlement_root = id(10);
        let mut wrong_feed = base;
        wrong_feed.selected_feed_data_id = id(11);
        assert_ne!(
            authority,
            terminal_authority_receipt(id(7), wrong_root, id(8), FeeId([9; 32]))
        );
        assert_ne!(
            authority,
            terminal_authority_receipt(id(7), wrong_feed, id(8), FeeId([9; 32]))
        );
    }
}
