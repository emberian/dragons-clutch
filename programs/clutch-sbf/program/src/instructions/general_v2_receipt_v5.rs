//! Exact SBF authentication seam for rent-owned General SettlementReceipt V5.
//!
//! The counted `0xa9/1` SettlementRoot, not legacy SelectedCandidate, is the
//! settlement authority. This module authenticates the complete program-owned
//! root, retained sealed Feed, and exact writable V5 receipt; rederives all
//! three PDAs; checks the full candidate/feed/price joins; and admits only the
//! General `kind=0` transition compartment. It performs no mutation by itself.

use core::cell::Ref;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{Id32, Sha256BackendV1};
use clutch_solana_layout::settlement_receipt_v5::{
    project_settlement_receipt_evidence_v5, SettlementReceiptAccountV5,
    SettlementReceiptEvidenceV5, SettlementReceiptTransitionCommitmentV5,
    SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use clutch_solana_layout::Hash32 as LayoutHash32;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

use super::general_v2_settlement_traversal_v5::AuthenticatedRootSettlementTraversalV5;

/// SettlementRoot, retained Feed, and writable V5 receipt.
pub const RECEIPT_V5_AUTH_ACCOUNT_COUNT: usize = 3;
/// Counted SettlementRoot; read-only or writable as fixed by the action.
pub const IX_SETTLEMENT_ROOT: usize = 0;
/// Read-only retained sealed CandidateFeed V2.
pub const IX_RETAINED_FEED: usize = 1;
/// Writable rent-owned SettlementReceipt V5.
pub const IX_RECEIPT: usize = 2;

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact authenticated root/Feed/receipt authority for one General action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralReceiptV5 {
    settlement_root_account: Id32,
    retained_feed_account: Id32,
    receipt_account: Id32,
    root: contract::SettlementRootV1AccountV1,
    feed: contract::CandidateFeedHeaderV2,
    receipt: SettlementReceiptAccountV5,
    evidence: SettlementReceiptEvidenceV5,
}

impl AuthenticatedGeneralReceiptV5 {
    /// Canonical counted SettlementRoot PDA.
    pub const fn settlement_root_account(&self) -> Id32 {
        self.settlement_root_account
    }

    /// Canonical retained CandidateFeed V2 PDA.
    pub const fn retained_feed_account(&self) -> Id32 {
        self.retained_feed_account
    }

    /// Canonical rent-owned SettlementReceipt V5 PDA.
    pub const fn receipt_account(&self) -> Id32 {
        self.receipt_account
    }

    /// Exact hostile-byte-decoded counted root.
    pub const fn root(&self) -> &contract::SettlementRootV1AccountV1 {
        &self.root
    }

    /// Exact retained sealed Feed header.
    pub const fn feed(&self) -> contract::CandidateFeedHeaderV2 {
        self.feed
    }

    /// Exact current V5 receipt including transition and rent compartments.
    pub const fn receipt(&self) -> SettlementReceiptAccountV5 {
        self.receipt
    }

    /// PDA-derived V5 action identities and complete current data ID.
    pub const fn evidence(&self) -> SettlementReceiptEvidenceV5 {
        self.evidence
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

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: Option<usize>,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn exact_price(prices_le: &[u8], outcome: u8) -> Outcome<u64> {
    let start = usize::from(outcome)
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let end = start
        .checked_add(core::mem::size_of::<u64>())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let bytes: [u8; 8] = prices_le
        .get(start..end)
        .ok_or(Refusal::Adapter(ClutchError::WrongDataLength))?
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    Ok(u64::from_le_bytes(bytes))
}

fn authenticate_general_receipt_v5_inner(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root_writable: bool,
) -> Outcome<AuthenticatedGeneralReceiptV5> {
    require_count(accounts, RECEIPT_V5_AUTH_ACCOUNT_COUNT)?;
    require_program_state(
        program_id,
        &accounts[IX_SETTLEMENT_ROOT],
        root_writable,
        Some(contract::SETTLEMENT_ROOT_ACCOUNT_BYTES),
    )?;
    require_program_state(program_id, &accounts[IX_RETAINED_FEED], false, None)?;
    require_program_state(
        program_id,
        &accounts[IX_RECEIPT],
        true,
        Some(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5),
    )?;
    require(
        accounts[IX_SETTLEMENT_ROOT].key != accounts[IX_RETAINED_FEED].key
            && accounts[IX_SETTLEMENT_ROOT].key != accounts[IX_RECEIPT].key
            && accounts[IX_RETAINED_FEED].key != accounts[IX_RECEIPT].key,
        ClutchError::AccountAlias,
    )?;

    let settlement_root_account = id(accounts[IX_SETTLEMENT_ROOT].key);
    let retained_feed_account = id(accounts[IX_RETAINED_FEED].key);
    let receipt_account = id(accounts[IX_RECEIPT].key);
    let root =
        contract::SettlementRootV1AccountV1::decode(&borrow_data(&accounts[IX_SETTLEMENT_ROOT])?)?;
    let feed_data = borrow_data(&accounts[IX_RETAINED_FEED])?;
    let (feed, tail) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    let candidate_bundle_digest =
        contract::candidate_bundle_digest_v1(&RuntimeSha256, &feed_data, true)?;
    let receipt_data = borrow_data(&accounts[IX_RECEIPT])?;
    let (receipt, evidence) = project_settlement_receipt_evidence_v5(
        LayoutHash32::new(receipt_account.bytes())?,
        &receipt_data,
    )?;
    let receipt_semantic = receipt.semantic();
    let counts = root.counts();

    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    let feed_pda = seeds::general_v2_feed_pda(program_id, &root.source_admission_node().bytes());
    let receipt_pda = seeds::general_v2_receipt_v5_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
        receipt_semantic.slice_index,
    );
    require(
        *accounts[IX_SETTLEMENT_ROOT].key == root_pda.0 && root.stored_bump() == root_pda.1,
        ClutchError::WrongPda,
    )?;
    require(
        *accounts[IX_RETAINED_FEED].key == feed_pda.0 && feed.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;
    require(
        *accounts[IX_RECEIPT].key == receipt_pda.0 && receipt_semantic.stored_bump == receipt_pda.1,
        ClutchError::WrongPda,
    )?;

    require(
        root.phase() == contract::SettlementRootPhaseV1::Settling
            && root.retained_feed_state() == contract::SettlementRootChildStateV1::Live
            && root.retained_feed() == retained_feed_account
            && feed.epoch == root.epoch()
            && feed.epoch_generation == root.epoch_generation()
            && feed.market == root.market()
            && feed.node == root.source_admission_node()
            && feed.order_set == root.order_set()
            && feed.settlement_candidate_id == root.settlement_candidate_id()
            && feed.settlement_witness_digest == root.settlement_witness_digest()
            && candidate_bundle_digest == root.candidate_bundle_digest()
            && feed.outcome_count == root.outcome_count()
            && feed.slice_count == counts.expected_receipts
            && counts.admitted_receipts == counts.expected_receipts
            && counts.live_receipts > 0
            && receipt.transition() == SettlementReceiptTransitionCommitmentV5::None
            && receipt_semantic.epoch.0 == root.epoch().bytes()
            && receipt_semantic.market.0 == root.market().bytes()
            && receipt_semantic.candidate.0 == root.settlement_candidate_id().bytes()
            && receipt_semantic.slice_index < counts.admitted_receipts
            && receipt_semantic.outcome < feed.outcome_count
            && receipt_semantic.price == exact_price(tail.prices_le(), receipt_semantic.outcome)?,
        ClutchError::MismatchedState,
    )?;

    Ok(AuthenticatedGeneralReceiptV5 {
        settlement_root_account,
        retained_feed_account,
        receipt_account,
        root,
        feed,
        receipt,
        evidence,
    })
}

/// Authenticate one writable ReceiptV5 against the already authenticated
/// read-only SettlementRoot and exhaustive shared traversal.
///
/// This is the sole action-26/portfolio-neutral bridge. It does not accept raw
/// root, Feed, candidate, price, or page facts and performs no mutation.
pub fn authenticate_general_receipt_v5_root_traversal(
    program_id: &Pubkey,
    authenticated: AuthenticatedRootSettlementTraversalV5<'_>,
    receipt_account_info: &AccountInfo<'_>,
) -> Outcome<AuthenticatedGeneralReceiptV5> {
    require_program_state(
        program_id,
        receipt_account_info,
        true,
        Some(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5),
    )?;
    let authenticated_root = authenticated.root();
    let authenticated_traversal = authenticated.traversal();
    let root = *authenticated_root.root();
    let root_account = authenticated_root.account();
    let retained_feed_account = authenticated_traversal.feed_account();
    let feed = authenticated_traversal.feed();
    let traversal = authenticated_traversal.traversal();
    let receipt_account = id(receipt_account_info.key);
    require(
        receipt_account != root_account && receipt_account != retained_feed_account,
        ClutchError::AccountAlias,
    )?;
    let receipt_data = borrow_data(receipt_account_info)?;
    let (receipt, evidence) = project_settlement_receipt_evidence_v5(
        LayoutHash32::new(receipt_account.bytes())?,
        &receipt_data,
    )?;
    let semantic = receipt.semantic();
    let receipt_pda = seeds::general_v2_receipt_v5_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
        semantic.slice_index,
    );
    require(
        *receipt_account_info.key == receipt_pda.0 && semantic.stored_bump == receipt_pda.1,
        ClutchError::WrongPda,
    )?;
    let counts = root.counts();
    require(
        root.phase() == contract::SettlementRootPhaseV1::Settling
            && root.retained_feed_state() == contract::SettlementRootChildStateV1::Live
            && root.retained_feed() == retained_feed_account
            && feed.epoch == root.epoch()
            && feed.epoch_generation == root.epoch_generation()
            && feed.market == root.market()
            && feed.node == root.source_admission_node()
            && feed.order_set == root.order_set()
            && feed.settlement_candidate_id == root.settlement_candidate_id()
            && feed.settlement_witness_digest == root.settlement_witness_digest()
            && feed.outcome_count == root.outcome_count()
            && feed.slice_count == counts.expected_receipts
            && counts.admitted_receipts == counts.expected_receipts
            && counts.live_receipts > 0
            && receipt.transition() == SettlementReceiptTransitionCommitmentV5::None
            && semantic.epoch.0 == root.epoch().bytes()
            && semantic.market.0 == root.market().bytes()
            && semantic.candidate.0 == root.settlement_candidate_id().bytes()
            && semantic.slice_index < counts.admitted_receipts
            && traversal.settlement_slice(semantic.slice_index).is_some()
            && semantic.outcome < feed.outcome_count
            && traversal.outcome_price(semantic.outcome) == Some(semantic.price),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedGeneralReceiptV5 {
        settlement_root_account: root_account,
        retained_feed_account,
        receipt_account,
        root,
        feed,
        receipt,
        evidence,
    })
}

/// Authenticate an existing V5 receipt for an action that does not mutate root.
pub fn authenticate_general_receipt_v5_readonly_root(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedGeneralReceiptV5> {
    authenticate_general_receipt_v5_inner(program_id, accounts, false)
}

/// Authenticate an existing V5 receipt for an atomic root-count mutation.
pub fn authenticate_general_receipt_v5_writable_root(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedGeneralReceiptV5> {
    authenticate_general_receipt_v5_inner(program_id, accounts, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        signer: bool,
        writable: bool,
        executable: bool,
    }

    impl Cell {
        fn program_owned(program_id: Pubkey, writable: bool, len: usize) -> Self {
            Self {
                key: Pubkey::new_from_array([7; 32]),
                owner: program_id,
                lamports: 1,
                data: vec![0; len],
                signer: false,
                writable,
                executable: false,
            }
        }

        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                self.signer,
                self.writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    #[test]
    fn exact_price_refuses_out_of_range_outcome() {
        assert_eq!(exact_price(&11_u64.to_le_bytes(), 0), Ok(11));
        assert_eq!(
            exact_price(&11_u64.to_le_bytes(), 1),
            Err(ClutchError::WrongDataLength.into())
        );
    }

    #[test]
    fn access_authentication_is_exact() {
        let program_id = Pubkey::new_from_array([9; 32]);
        let mut readonly = Cell::program_owned(program_id, false, 8);
        assert_eq!(
            require_program_state(&program_id, &readonly.info(), false, Some(8)),
            Ok(())
        );
        assert_eq!(
            require_program_state(&program_id, &readonly.info(), true, Some(8)),
            Err(ClutchError::NotWritable.into())
        );

        let mut writable = Cell::program_owned(program_id, true, 8);
        assert_eq!(
            require_program_state(&program_id, &writable.info(), false, Some(8)),
            Err(ClutchError::UnexpectedWritable.into())
        );
        assert_eq!(
            require_program_state(&program_id, &writable.info(), true, Some(7)),
            Err(ClutchError::WrongDataLength.into())
        );

        let mut signer = Cell::program_owned(program_id, false, 8);
        signer.signer = true;
        assert_eq!(
            require_program_state(&program_id, &signer.info(), false, Some(8)),
            Err(ClutchError::MismatchedState.into())
        );
    }
}
