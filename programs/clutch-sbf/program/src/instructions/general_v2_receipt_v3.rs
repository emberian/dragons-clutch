//! Capability-disabled authentication seam for General SettlementReceipt V3.
//!
//! This module authenticates the persisted authority that the action-25/26/36/37
//! pure composers consume: exact program ownership, the counted Epoch and
//! SelectedCandidate PDAs, the retained sealed Feed and its bundle digest, and
//! the fresh receipt PDA. It exports no dispatcher entry and performs no write.
//! Receipt creation remains owned by the strict next-slice action-24 planner.

use core::cell::Ref;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{GeneralEpochPhaseV1, Id32, Sha256BackendV1};
use clutch_solana_layout::settlement_receipt_v3::{
    project_settlement_receipt_evidence_v3, SettlementReceiptAccountV3, SettlementReceiptEvidenceV3,
};
use clutch_solana_layout::{account_len, Hash32 as LayoutHash32};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

/// Counted parent Epoch, SelectedCandidate, retained Feed, and writable receipt.
pub const RECEIPT_V3_AUTH_ACCOUNT_COUNT: usize = 4;
/// Read-only counted Epoch.
pub const IX_EPOCH: usize = 0;
/// Read-only counted SelectedCandidate.
pub const IX_SELECTED: usize = 1;
/// Read-only retained sealed CandidateFeed V2.
pub const IX_SELECTED_FEED: usize = 2;
/// Writable SettlementReceipt V3.
pub const IX_RECEIPT: usize = 3;

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact authenticated receipt authority projected from four SBF accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralReceiptV3 {
    epoch_account: Id32,
    selected_account: Id32,
    selected_feed_account: Id32,
    receipt_account: Id32,
    epoch: contract::GeneralEpochV6AccountV1,
    selected: contract::SelectedCandidateV1AccountV1,
    feed: contract::CandidateFeedHeaderV2,
    receipt: SettlementReceiptAccountV3,
    evidence: SettlementReceiptEvidenceV3,
}

impl AuthenticatedGeneralReceiptV3 {
    /// Canonical counted Epoch PDA.
    pub const fn epoch_account(&self) -> Id32 {
        self.epoch_account
    }

    /// Canonical counted SelectedCandidate PDA.
    pub const fn selected_account(&self) -> Id32 {
        self.selected_account
    }

    /// Canonical retained CandidateFeed V2 PDA.
    pub const fn selected_feed_account(&self) -> Id32 {
        self.selected_feed_account
    }

    /// Canonical SettlementReceipt V3 PDA.
    pub const fn receipt_account(&self) -> Id32 {
        self.receipt_account
    }

    /// Exact decoded counted Epoch.
    pub const fn epoch(&self) -> contract::GeneralEpochV6AccountV1 {
        self.epoch
    }

    /// Exact decoded SelectedCandidate authority.
    pub const fn selected(&self) -> contract::SelectedCandidateV1AccountV1 {
        self.selected
    }

    /// Exact decoded retained Feed header.
    pub const fn feed(&self) -> contract::CandidateFeedHeaderV2 {
        self.feed
    }

    /// Exact decoded mutable receipt.
    pub const fn receipt(&self) -> SettlementReceiptAccountV3 {
        self.receipt
    }

    /// PDA-derived transition identities and exact mutable prestate data ID.
    pub const fn evidence(&self) -> SettlementReceiptEvidenceV3 {
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

/// Authenticate one existing General SettlementReceipt V3 and every stable
/// identity it inherits from the counted selected Feed.
///
/// This is not receipt-creation authority and not an executable action. A live
/// transition must additionally consume the private pure action plan and its
/// owner-row, page, Reservation, Position, Replay, fee, rent, and liveness joins.
pub fn authenticate_general_receipt_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedGeneralReceiptV3> {
    require_count(accounts, RECEIPT_V3_AUTH_ACCOUNT_COUNT)?;
    require_program_state(
        program_id,
        &accounts[IX_EPOCH],
        false,
        Some(contract::GENERAL_EPOCH_ACCOUNT_BYTES),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_SELECTED],
        false,
        Some(contract::SELECTED_CANDIDATE_ACCOUNT_BYTES),
    )?;
    require_program_state(program_id, &accounts[IX_SELECTED_FEED], false, None)?;
    require_program_state(
        program_id,
        &accounts[IX_RECEIPT],
        true,
        Some(account_len::SETTLEMENT_RECEIPT_V3),
    )?;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(
                accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }

    let epoch_account = id(accounts[IX_EPOCH].key);
    let selected_account = id(accounts[IX_SELECTED].key);
    let selected_feed_account = id(accounts[IX_SELECTED_FEED].key);
    let receipt_account = id(accounts[IX_RECEIPT].key);
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[IX_EPOCH])?)?;
    let selected =
        contract::SelectedCandidateV1AccountV1::decode(&borrow_data(&accounts[IX_SELECTED])?)?;
    let feed_data = borrow_data(&accounts[IX_SELECTED_FEED])?;
    let (feed, tail) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    let candidate_bundle_digest =
        contract::candidate_bundle_digest_v1(&RuntimeSha256, &feed_data, true)?;
    let receipt_data = borrow_data(&accounts[IX_RECEIPT])?;
    let receipt_pda_hash = LayoutHash32::new(receipt_account.bytes())?;
    let (receipt, evidence) =
        project_settlement_receipt_evidence_v3(receipt_pda_hash, &receipt_data)?;

    let epoch_pda =
        seeds::general_v2_epoch_pda(program_id, &epoch.market_binding.bytes(), epoch.epoch_index);
    let selected_pda = seeds::general_v2_selected_pda(
        program_id,
        &epoch_account.bytes(),
        &selected.settlement_candidate_id.bytes(),
    );
    let feed_pda = seeds::general_v2_feed_pda(program_id, &selected.source_admission_node.bytes());
    let receipt_pda = seeds::general_v2_receipt_pda(
        program_id,
        &epoch_account.bytes(),
        &selected.settlement_candidate_id.bytes(),
        receipt.slice_index,
    );
    require(
        *accounts[IX_EPOCH].key == epoch_pda.0 && epoch.stored_bump == epoch_pda.1,
        ClutchError::WrongPda,
    )?;
    require(
        *accounts[IX_SELECTED].key == selected_pda.0 && selected.stored_bump == selected_pda.1,
        ClutchError::WrongPda,
    )?;
    require(
        *accounts[IX_SELECTED_FEED].key == feed_pda.0 && feed.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;
    require(
        *accounts[IX_RECEIPT].key == receipt_pda.0 && receipt.stored_bump == receipt_pda.1,
        ClutchError::WrongPda,
    )?;

    require(
        epoch.phase == GeneralEpochPhaseV1::Finalized
            && epoch.selected_candidate_count == 1
            && selected.epoch == epoch_account
            && selected.epoch_generation == epoch.generation
            && selected.market_binding == epoch.market_binding
            && selected.market == epoch.market_runtime
            && selected.order_set == epoch.order_set
            && selected.selected_feed == selected_feed_account
            && feed.epoch == epoch_account
            && feed.epoch_generation == epoch.generation
            && feed.market == selected.market
            && feed.node == selected.source_admission_node
            && feed.order_set == selected.order_set
            && feed.economic_domain_digest == selected.economic_domain_digest
            && candidate_bundle_digest == selected.candidate_bundle_digest
            && feed.settlement_candidate_id == selected.settlement_candidate_id
            && feed.base_relation_candidate_id == selected.base_relation_candidate_id
            && feed.settlement_witness_digest == selected.settlement_witness_digest
            && feed.relation_policy_id == selected.relation_policy_id
            && feed.price_measure_policy_v1_id == selected.price_measure_policy_v1_id
            && feed.native_claim_basis_id == selected.native_claim_basis_id
            && feed.candidate_price_digest == selected.candidate_price_digest
            && feed.price_body_digest == selected.price_body_digest
            && feed.slice_count == selected.slice_count
            && feed.candidate_kind == selected.candidate_kind
            && feed.price_witness_schema == selected.price_witness_schema
            && feed.quantized_semantics_version == selected.quantized_semantics_version
            && receipt.epoch.0 == epoch_account.bytes()
            && receipt.market.0 == selected.market.bytes()
            && receipt.candidate.0 == selected.settlement_candidate_id.bytes()
            && receipt.slice_index < selected.slice_count
            && receipt.outcome < feed.outcome_count
            && receipt.price == exact_price(tail.prices_le(), receipt.outcome)?,
        ClutchError::MismatchedState,
    )?;

    Ok(AuthenticatedGeneralReceiptV3 {
        epoch_account,
        selected_account,
        selected_feed_account,
        receipt_account,
        epoch,
        selected,
        feed,
        receipt,
        evidence,
    })
}
