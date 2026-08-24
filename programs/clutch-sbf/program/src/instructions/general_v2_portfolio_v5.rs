//! Current General action 42: consume one exact coefficient-portfolio pair.
//!
//! This adapter owns no coefficients and accepts no caller-shaped economic
//! rows. It authenticates the counted SettlementRoot, retained sealed Feed,
//! immutable EconomicDomain/PriceGrid owners, the complete
//! bounded OrderPage V5 set, both rent-owned Reservation V9 accounts, both
//! ordinary Position V3 accounts, both GEN1 Replay accounts, and the complete
//! canonical active prefix of one through sixteen pending Receipt V5 sibling
//! accounts. Before any of those local facts are projected, the common current
//! General prefix joins MarketBindingV5/RuntimeV3 to Product RootV3, LinkV3,
//! FundingV5, Registry/loader release, the compiled Source graph, and Revenue
//! policy. It then consumes the private
//! `clutch-batch` portfolio capability and writes every successor without CPI
//! or lamport movement. Any missing, extra, duplicated, or reordered sibling
//! refuses before the first write.

use core::cell::{Cell, Ref};

use clutch_batch::portfolio_book_v2::{
    authenticate_complete_portfolio_book_ref_v2, PortfolioBookAccountExpectationV2,
    PortfolioBookAccountRoleV2, PortfolioBookAdapterV2, PortfolioBookPageSetRecordV2,
    PortfolioCompleteBookProjectionExpectationV2, PORTFOLIO_BOOK_AUTHORITY_VERSION_V2,
    PORTFOLIO_BOOK_MAX_PAGES_V2,
};
use clutch_batch::portfolio_execution_v2::{
    authenticate_exact_portfolio_pair_v2, authenticate_portfolio_receipt_sibling_set_v2,
    authenticate_selected_portfolio_order_v2, portfolio_settlement_receipt_v5_set_digest_v2,
    prepare_portfolio_pair_execution_borrowed_v2, PortfolioAccountExpectationV2,
    PortfolioAccountRoleV2, PortfolioAdapterV2, PortfolioPairExecutionInputV2,
    PortfolioPairPostSemanticIdsV2, PortfolioPositionPrestateV2,
    PortfolioReceiptSiblingTraversalSetV2, PortfolioReceiptSiblingTraversalV2,
    PortfolioReplayPrestateV2, PortfolioReservationLifecycleV2, PortfolioReservationPrestateV2,
    PortfolioSelectionMembershipExpectationV2, PortfolioSettlementReceiptV5Prestate,
    PortfolioSettlementReceiptV5SetPrestate, PortfolioSettlementReceiptV5TransitionExpectationV2,
    PortfolioSourceOrderKindV2, PortfolioTransitionExpectationV2, PortfolioValuationBoundaryV2,
    SelectedPortfolioOrderRecordV2, SettlementReceiptTransitionKindV2,
    PORTFOLIO_EXECUTION_VERSION_V2, PORTFOLIO_PAIR_MAX_RECEIPTS_V2,
    PORTFOLIO_PAIR_RECEIPT_V2_BYTES, PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2,
};
use clutch_batch::relation_v1::MAX_OUTCOMES;
use clutch_batch::relation_v2::{
    EconomicBookV2, EconomicCandidateV2, EconomicOrderV2, PricePreconditionV2,
};
use clutch_batch::Side;
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_portfolio_settlement_payload_v1, project_general_position_replay_prestate_v1,
    project_general_replay_transition_v1, ConsumePortfolioPairEggsPayloadV1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1,
    Id32, PortfolioSettlementPayloadV1, Sha256BackendV1,
};
use clutch_general_v2_runtime::{
    decode_sealed_candidate_feed_v1, project_owner_blind_book_costed_v1, GeneralOrderPageInputV5,
    OwnerBlindBookProjectionV2,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, ReplayV3Envelope, ReplayV3HashBackend, POSITION_V3_BYTES,
};
use clutch_solana_layout::order_page_v5::{verify_page_v5, OrderSlotCursorV5};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation::{RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED};
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v5::{
    portfolio_pair_transition_commitment_v2 as layout_portfolio_commitment_v2,
    project_settlement_receipt_evidence_v5, SettlementReceiptAccountV5,
    SettlementReceiptTransitionCommitmentV5,
    PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2 as LAYOUT_PORTFOLIO_DOMAIN_V2,
    SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use clutch_solana_layout::{
    account_len, Hash32, OrderSlot, PriceGridAccount, ORDER_KIND_PORTFOLIO,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::seeds;

use super::general_market_current_v5::{
    authenticate_general_market_current_prefix_v5, CURRENT_V5_IX_MARKET_BINDING,
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5,
};
use super::general_v2_settlement_root::authenticate_readonly_general_settlement_root_v1;

/// Action-local accounts after the exact current-market prefix and before the
/// active OrderPage V5 and Receipt V5 suffixes.
pub const PORTFOLIO_PAIR_LOCAL_FIXED_ACCOUNTS_V5: usize = 10;
/// Fixed accounts before the active OrderPage V5 and Receipt V5 suffixes.
pub const PORTFOLIO_PAIR_FIXED_ACCOUNTS_V2: usize =
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5 + PORTFOLIO_PAIR_LOCAL_FIXED_ACCOUNTS_V5;
/// Minimum exact account count: fixed prefix plus one page and one receipt.
pub const PORTFOLIO_PAIR_MIN_ACCOUNTS_V2: usize = PORTFOLIO_PAIR_FIXED_ACCOUNTS_V2 + 2;
/// Maximum exact account count: fixed prefix plus four pages and 16 receipts.
pub const PORTFOLIO_PAIR_MAX_ACCOUNTS_V2: usize = PORTFOLIO_PAIR_FIXED_ACCOUNTS_V2 + 20;

pub const IX_SETTLEMENT_ROOT: usize = GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5;
pub const IX_RETAINED_FEED: usize = IX_SETTLEMENT_ROOT + 1;
pub const IX_ECONOMIC_DOMAIN: usize = IX_RETAINED_FEED + 1;
pub const IX_PRICE_GRID: usize = IX_ECONOMIC_DOMAIN + 1;
pub const IX_BUYER_RESERVATION_V9: usize = IX_PRICE_GRID + 1;
pub const IX_SELLER_RESERVATION_V9: usize = IX_BUYER_RESERVATION_V9 + 1;
pub const IX_BUYER_POSITION_V3: usize = IX_SELLER_RESERVATION_V9 + 1;
pub const IX_SELLER_POSITION_V3: usize = IX_BUYER_POSITION_V3 + 1;
pub const IX_BUYER_REPLAY_GEN1: usize = IX_SELLER_POSITION_V3 + 1;
pub const IX_SELLER_REPLAY_GEN1: usize = IX_BUYER_REPLAY_GEN1 + 1;
pub const IX_FIRST_ORDER_PAGE_V5: usize = PORTFOLIO_PAIR_FIXED_ACCOUNTS_V2;

static EMPTY_RECEIPT_LAYOUT_V2: [Option<SettlementReceiptAccountV5>;
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2] = [None; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
static EMPTY_RECEIPT_ACCOUNTS_V2: [Hash32; PORTFOLIO_PAIR_MAX_RECEIPTS_V2] =
    [Hash32::ZERO; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
static EMPTY_RECEIPT_PRESTATE_SET_V2: PortfolioSettlementReceiptV5SetPrestate =
    PortfolioSettlementReceiptV5SetPrestate {
        receipt_count: 0,
        receipts: [PortfolioSettlementReceiptV5Prestate::EMPTY; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
    };
static EMPTY_RECEIPT_TRAVERSAL_V2: [PortfolioReceiptSiblingTraversalV2;
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2] =
    [PortfolioReceiptSiblingTraversalV2::EMPTY; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
static EMPTY_EXECUTION_INPUT_V2: PortfolioPairExecutionInputV2 = PortfolioPairExecutionInputV2 {
    settlement_receipts: PortfolioSettlementReceiptV5SetPrestate {
        receipt_count: 0,
        receipts: [PortfolioSettlementReceiptV5Prestate::EMPTY; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
    },
    buyer_reservation: EMPTY_RESERVATION_PRESTATE_V2,
    seller_reservation: EMPTY_RESERVATION_PRESTATE_V2,
        buyer_position: EMPTY_POSITION_PRESTATE_V2,
        seller_position: EMPTY_POSITION_PRESTATE_V2,
        buyer_replay: EMPTY_REPLAY_PRESTATE_V2,
        seller_replay: EMPTY_REPLAY_PRESTATE_V2,
        post_semantic_ids: PortfolioPairPostSemanticIdsV2 {
            buyer_reservation: [0; 32],
            seller_reservation: [0; 32],
            buyer_position: [0; 32],
            seller_position: [0; 32],
            buyer_replay: [0; 32],
            seller_replay: [0; 32],
            settlement_receipts: [[0; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
        },
    };
const EMPTY_RESERVATION_PRESTATE_V2: PortfolioReservationPrestateV2 =
    PortfolioReservationPrestateV2 {
        account_id: [0; 32],
        semantic_id: [0; 32],
        generation: 0,
        lifecycle: PortfolioReservationLifecycleV2::Entitled,
        owner_id: [0; 32],
        order_id: [0; 32],
        position_account_id: [0; 32],
        position_generation: 0,
        entitled_units: 0,
        consumed_units: 0,
        paid_units: 0,
        remaining_cash_atoms: 0,
        remaining_claim_atoms: [0; MAX_OUTCOMES],
        maximum_fee_atoms: 0,
    };
const EMPTY_POSITION_PRESTATE_V2: PortfolioPositionPrestateV2 = PortfolioPositionPrestateV2 {
    account_id: [0; 32],
    semantic_id: [0; 32],
    owner_id: [0; 32],
    generation: 0,
    cash_atoms: 0,
    reserved_cash_atoms: 0,
    native_eggs: [0; MAX_OUTCOMES],
    outstanding_reservations: 0,
};
const EMPTY_REPLAY_PRESTATE_V2: PortfolioReplayPrestateV2 = PortfolioReplayPrestateV2 {
    account_id: [0; 32],
    semantic_id: [0; 32],
    ordinal: 0,
};

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

#[derive(Clone, Copy, Debug)]
struct EndpointPlanV2 {
    selected: SelectedPortfolioOrderRecordV2,
    reservation_pre: ReservationAccountV9,
    reservation_pre_data_id: [u8; 32],
    reservation_post: ReservationAccountV9,
    reservation_post_data_id: [u8; 32],
    position_pre: AuthenticatedPositionV3,
    position_post: PositionSettlementPoststateV3,
    position_post_semantic_id: [u8; 32],
    replay_pre: GeneralPositionReplayPrestateV1,
    replay_post: GeneralReplayTransitionPlanV1,
}

struct CompleteBookAdapterV2 {
    program_id: [u8; 32],
    page_set: PortfolioBookPageSetRecordV2,
    projection: Box<OwnerBlindBookProjectionV2>,
}

impl PortfolioBookAdapterV2 for CompleteBookAdapterV2 {
    fn authenticate_book_account(&self, expected: &PortfolioBookAccountExpectationV2) -> bool {
        if expected.owner_program_id != self.program_id || expected.writable {
            return false;
        }
        match expected.role {
            PortfolioBookAccountRoleV2::SettlementRoot => {
                expected.account_id == self.page_set.settlement_root_account_id
                    && expected.data_semantic_id == self.page_set.settlement_root_pre_semantic_id
                    && expected.generation == Some(self.page_set.settlement_root_epoch_generation)
                    && expected.page_index.is_none()
            }
            PortfolioBookAccountRoleV2::RetainedFeed => {
                expected.account_id == self.page_set.retained_feed_account_id
                    && expected.data_semantic_id == self.page_set.retained_feed_semantic_id
                    && expected.generation.is_none()
                    && expected.page_index.is_none()
            }
            PortfolioBookAccountRoleV2::OrderPage => {
                let Some(page_index) = expected.page_index else {
                    return false;
                };
                let page = usize::from(page_index);
                page < usize::from(self.page_set.page_count)
                    && expected.account_id == self.page_set.page_account_ids[page]
                    && expected.data_semantic_id == self.page_set.page_semantic_ids[page]
                    && expected.generation.is_none()
            }
        }
    }

    fn project_complete_economic_book_ref<'a>(
        &'a self,
        expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<&'a EconomicBookV2> {
        if expected.page_set == self.page_set {
            Some(self.projection.base().book())
        } else {
            None
        }
    }
}

struct ExecutionAdapterV2 {
    program_id: [u8; 32],
    selected: [SelectedPortfolioOrderRecordV2; 2],
    account_expectations: Box<[PortfolioAccountExpectationV2; 10]>,
    transition_expectations: Box<[PortfolioTransitionExpectationV2; 6]>,
    receipt_pre: Box<[Option<SettlementReceiptAccountV5>; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]>,
    receipt_prestate: Box<PortfolioSettlementReceiptV5SetPrestate>,
    receipt_accounts: Box<[Hash32; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]>,
    receipt_post:
        Cell<Option<Box<[Option<SettlementReceiptAccountV5>; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]>>>,
}

impl PortfolioAdapterV2 for ExecutionAdapterV2 {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool {
        if expected.owner_program_id != self.program_id {
            return false;
        }
        if expected.role == PortfolioAccountRoleV2::SettlementReceipt {
            let mut index = 0usize;
            while index < usize::from(self.receipt_prestate.receipt_count) {
                let receipt = self.receipt_prestate.receipts[index];
                if expected.account_id == receipt.account_id
                    && expected.data_semantic_id == receipt.pre_data_id
                    && expected.generation.is_none()
                    && expected.writable
                    && expected.must_exist
                {
                    return true;
                }
                index += 1;
            }
            return false;
        }
        self.account_expectations
            .iter()
            .any(|observed| observed == expected)
    }

    fn authenticate_selection_membership(
        &self,
        expected: &PortfolioSelectionMembershipExpectationV2,
        relation_order: &EconomicOrderV2,
        candidate: &EconomicCandidateV2,
    ) -> bool {
        let Some(record) = self
            .selected
            .iter()
            .find(|record| **record == expected.record)
        else {
            return false;
        };
        let at = usize::from(record.order_index);
        relation_order.order_id == record.order_id
            && relation_order.side == record.side
            && candidate.fills.get(at).copied() == Some(record.selected_fill_units)
    }

    fn authenticate_transition(&self, expected: &PortfolioTransitionExpectationV2) -> bool {
        self.transition_expectations
            .iter()
            .any(|observed| observed == expected)
    }

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[[u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        if expected.prestate != *self.receipt_prestate
            || expected.post_transition_kind != SettlementReceiptTransitionKindV2::PortfolioPairV2
            || expected.transition_commitment == [0; 32]
        {
            return None;
        }
        let mut post = super::orders_batch::boxed_copy_of(&EMPTY_RECEIPT_LAYOUT_V2).ok()?;
        let mut post_ids = [[0u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
        let mut index = 0usize;
        while index < usize::from(expected.prestate.receipt_count) {
            let successor = self.receipt_pre[index]?
                .commit_portfolio_pair_delivery(Hash32::from_bytes(expected.transition_commitment))
                .ok()?;
            post_ids[index] = successor
                .data_id(self.receipt_accounts[index])
                .ok()?
                .bytes();
            post[index] = Some(successor);
            index += 1;
        }
        self.receipt_post.set(Some(post));
        Some(post_ids)
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

fn require_program_account(
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

fn account_frame(count: usize, page_count: u8, receipt_count: u8) -> Outcome<(usize, usize)> {
    let pages = usize::from(page_count);
    let receipts = usize::from(receipt_count);
    if !(1..=PORTFOLIO_BOOK_MAX_PAGES_V2).contains(&pages)
        || !(1..=PORTFOLIO_PAIR_MAX_RECEIPTS_V2).contains(&receipts)
    {
        return Err(Refusal::Adapter(ClutchError::AccountCount));
    }
    let expected = PORTFOLIO_PAIR_FIXED_ACCOUNTS_V2
        .checked_add(pages)
        .and_then(|value| value.checked_add(receipts))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    if count != expected {
        return Err(Refusal::Adapter(ClutchError::AccountCount));
    }
    Ok((pages, receipts))
}

fn require_distinct_accounts(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
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
    Ok(())
}

fn project_complete_pages(
    accounts: &[AccountInfo<'_>],
    page_count: usize,
    order_set: Id32,
    domain: &contract::EconomicDomainV2AccountV1,
    binding: &contract::MarketBindingV2,
    grid: &PriceGridAccount,
) -> Outcome<Box<OwnerBlindBookProjectionV2>> {
    let project = |pages: &[GeneralOrderPageInputV5<'_>]| {
        project_owner_blind_book_costed_v1(pages, order_set, domain, binding, grid)
            .map(Box::new)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    };
    match page_count {
        1 => {
            let page0 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5])?;
            project(&[GeneralOrderPageInputV5 {
                account: id(accounts[IX_FIRST_ORDER_PAGE_V5].key),
                body: &page0,
            }])
        }
        2 => {
            let page0 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5])?;
            let page1 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 1])?;
            project(&[
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5].key),
                    body: &page0,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 1].key),
                    body: &page1,
                },
            ])
        }
        3 => {
            let page0 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5])?;
            let page1 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 1])?;
            let page2 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 2])?;
            project(&[
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5].key),
                    body: &page0,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 1].key),
                    body: &page1,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 2].key),
                    body: &page2,
                },
            ])
        }
        4 => {
            let page0 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5])?;
            let page1 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 1])?;
            let page2 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 2])?;
            let page3 = borrow_data(&accounts[IX_FIRST_ORDER_PAGE_V5 + 3])?;
            project(&[
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5].key),
                    body: &page0,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 1].key),
                    body: &page1,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 2].key),
                    body: &page2,
                },
                GeneralOrderPageInputV5 {
                    account: id(accounts[IX_FIRST_ORDER_PAGE_V5 + 3].key),
                    body: &page3,
                },
            ])
        }
        _ => Err(Refusal::Adapter(ClutchError::AccountCount)),
    }
}

#[inline(never)]
fn find_page_slot(
    accounts: &[AccountInfo<'_>],
    page_index: u16,
    order_id: [u8; 32],
    owner: [u8; 32],
    side: u8,
    order_generation: u64,
    position_generation: u64,
) -> Outcome<u8> {
    let account_index = IX_FIRST_ORDER_PAGE_V5
        .checked_add(usize::from(page_index))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let data = borrow_data(
        accounts
            .get(account_index)
            .ok_or(Refusal::Adapter(ClutchError::AccountCount))?,
    )?;
    let header = verify_page_v5(&data)?;
    require(
        header.page_index == page_index,
        ClutchError::MismatchedState,
    )?;
    let mut cursor = OrderSlotCursorV5::new(&data)?;
    while let Some(step) = cursor.next_slot() {
        let verified = step?;
        if let OrderSlot::Portfolio(record) = verified.slot {
            if record.order_id.bytes() == order_id
                && record.owner.bytes() == owner
                && record.side == side
                && record.generation == order_generation
                && verified.position_generation == position_generation
            {
                return Ok(verified.slot_index);
            }
        }
    }
    Err(Refusal::Adapter(ClutchError::MismatchedState))
}

fn authenticate_position_replay(
    program_id: &Pubkey,
    root: contract::SettlementRootV1AccountV1,
    owner: [u8; 32],
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
) -> Outcome<(AuthenticatedPositionV3, GeneralPositionReplayPrestateV1)> {
    let position = PositionAccountV3::decode(&borrow_data(position_account)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fields = position.fields();
    let purpose_binding = Identity32V1::new(root.market().bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_position = seeds::position_v3_pda(
        program_id,
        &root.market_instance_v2_id().bytes(),
        &owner,
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    let expected_replay = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    require(
        *position_account.key == expected_position.0
            && position.stored_bump() == expected_position.1
            && *replay_account.key == expected_replay.0
            && fields.purpose == PositionPurposeV3::General
            && fields.lifecycle == PositionLifecycleV3::Open
            && fields.market_instance_id.bytes() == root.market_instance_v2_id().bytes()
            && fields.owner.bytes() == owner
            && fields.controller.bytes() == owner
            && fields.purpose_binding_id == purpose_binding
            && fields.replay_account.bytes() == replay_account.key.to_bytes()
            && fields.outcome_count == root.outcome_count(),
        ClutchError::MismatchedState,
    )?;
    let semantic_id = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let authenticated = AuthenticatedPositionV3 {
        account: position_account.key.to_bytes(),
        general_market_runtime: root.market().bytes(),
        semantic: position,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    authenticated
        .validate_writable()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_data = borrow_data(replay_account)?;
    let envelope = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    let replay = project_general_position_replay_prestate_v1(
        id(replay_account.key),
        expected_replay.1,
        envelope.header().next_sequence(),
        &replay_data,
        authenticated,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    Ok((authenticated, replay))
}

fn consumed_reservation(
    pre: ReservationAccountV9,
    expected_entitled_units: u64,
) -> Outcome<ReservationAccountV9> {
    let mut body = pre.body();
    require(
        body.state == RESERVATION_STATE_ENTITLED
            && expected_entitled_units != 0
            && body.entitled_units == expected_entitled_units
            && body.consumed_units == 0
            && body.paid_units == 0,
        ClutchError::MismatchedState,
    )?;
    body.remaining_cash_atoms = 0;
    body.remaining_internal = [0; MAX_OUTCOMES];
    body.consumed_units = expected_entitled_units;
    body.paid_units = expected_entitled_units;
    body.release_generation = 0;
    body.state = RESERVATION_STATE_CONSUMED;
    ReservationAccountV9::new(body, pre.rent()).map_err(Into::into)
}

fn account_expectation(
    role: PortfolioAccountRoleV2,
    account_id: [u8; 32],
    program_id: [u8; 32],
    data_semantic_id: [u8; 32],
    generation: Option<u64>,
    writable: bool,
) -> PortfolioAccountExpectationV2 {
    PortfolioAccountExpectationV2 {
        role,
        account_id,
        owner_program_id: program_id,
        data_semantic_id,
        generation,
        writable,
        must_exist: true,
    }
}

fn replay_transition_expectation(endpoint: &EndpointPlanV2) -> PortfolioTransitionExpectationV2 {
    PortfolioTransitionExpectationV2 {
        role: PortfolioAccountRoleV2::Replay,
        account_id: endpoint.replay_pre.replay_account().bytes(),
        pre_semantic_id: endpoint.replay_pre.replay_semantic_id().bytes(),
        post_semantic_id: endpoint.replay_post.replay_poststate_semantic_id().bytes(),
        stable_generation: None,
        pre_replay_ordinal: endpoint.replay_pre.next_sequence(),
        post_replay_ordinal: endpoint.replay_post.next_sequence(),
        cash_debit_atoms: 0,
        cash_credit_atoms: 0,
        reserved_cash_release_atoms: 0,
        claim_debits: [0; MAX_OUTCOMES],
        claim_credits: [0; MAX_OUTCOMES],
        reservation_consumed: false,
    }
}

fn write_exact(account: &AccountInfo<'_>, body: &[u8]) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(data.len() == body.len(), ClutchError::WrongDataLength)?;
    data.copy_from_slice(body);
    Ok(())
}

#[inline(never)]
fn write_reservation_post(account: &AccountInfo<'_>, post: ReservationAccountV9) -> Outcome<()> {
    let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    post.encode(&mut bytes)?;
    write_exact(account, &bytes)
}

#[inline(never)]
fn write_position_post(
    account: &AccountInfo<'_>,
    post: &PositionSettlementPoststateV3,
) -> Outcome<()> {
    let bytes = post
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact(account, &bytes)
}

#[inline(never)]
fn write_replay_post(
    account: &AccountInfo<'_>,
    post: &GeneralReplayTransitionPlanV1,
) -> Outcome<()> {
    write_exact(account, post.replay_poststate_body())
}

/// Dispatch-compatible current action-42 entrypoint.
///
/// Exact account contract:
///
/// - `0..25`: exact common GeneralMarketCurrentV5 authority prefix;
/// - `25`: counted SettlementRoot V1, read-only;
/// - `26`: retained sealed Feed, read-only;
/// - `27..=28`: EconomicDomain V2 and PriceGrid, read-only;
/// - `29..=30`: buyer and seller Reservation V9, writable;
/// - `31..=32`: buyer and seller Position V3, writable;
/// - `33..=34`: buyer and seller GEN1 purpose Replay, writable;
/// - `35..35+page_count`: complete OrderPage V5 prefix, read-only and ordered
///   by `page_index`; and
/// - the exact remaining suffix: every Receipt V5 sibling, writable and
///   ordered by retained-Feed `slice_index`.
///
/// `page_count` and `receipt_count` delimit this hostile frame only. The
/// complete page projection and payoff-derived sibling capability must derive
/// the same lengths before either count acquires any meaning.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::ConsumePortfolioPairEggs
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let PortfolioSettlementPayloadV1::ConsumePortfolioPairEggs(request) =
        decode_portfolio_settlement_payload_v1(action.tag(), payload)?;
    consume_portfolio_pair(program_id, accounts, request)
}

fn consume_portfolio_pair(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ConsumePortfolioPairEggsPayloadV1,
) -> Outcome<()> {
    let (page_count, receipt_count) =
        account_frame(accounts.len(), request.page_count, request.receipt_count)?;
    let first_receipt = IX_FIRST_ORDER_PAGE_V5
        .checked_add(page_count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require_distinct_accounts(accounts)?;
    require(
        LAYOUT_PORTFOLIO_DOMAIN_V2 == &PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2[..],
        ClutchError::MismatchedState,
    )?;
    let current = authenticate_general_market_current_prefix_v5(program_id, accounts)?;

    for index in [
        IX_SETTLEMENT_ROOT,
        IX_RETAINED_FEED,
        IX_ECONOMIC_DOMAIN,
        IX_PRICE_GRID,
    ] {
        require_program_account(program_id, &accounts[index], false, None)?;
    }
    for index in [
        IX_BUYER_RESERVATION_V9,
        IX_SELLER_RESERVATION_V9,
        IX_BUYER_POSITION_V3,
        IX_SELLER_POSITION_V3,
        IX_BUYER_REPLAY_GEN1,
        IX_SELLER_REPLAY_GEN1,
    ] {
        require_program_account(program_id, &accounts[index], true, None)?;
    }
    let mut page = 0usize;
    while page < page_count {
        require_program_account(
            program_id,
            &accounts[IX_FIRST_ORDER_PAGE_V5 + page],
            false,
            Some(account_len::ORDER_PAGE_V5),
        )?;
        page += 1;
    }
    let mut receipt_index = 0usize;
    while receipt_index < receipt_count {
        require_program_account(
            program_id,
            &accounts[first_receipt + receipt_index],
            true,
            Some(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5),
        )?;
        receipt_index += 1;
    }
    require(
        accounts[IX_ECONOMIC_DOMAIN].data_len() == contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES
            && accounts[IX_PRICE_GRID].data_len() == account_len::PRICE_GRID
            && accounts[IX_BUYER_RESERVATION_V9].data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && accounts[IX_SELLER_RESERVATION_V9].data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && accounts[IX_BUYER_POSITION_V3].data_len() == POSITION_V3_BYTES
            && accounts[IX_SELLER_POSITION_V3].data_len() == POSITION_V3_BYTES
            && accounts[IX_BUYER_REPLAY_GEN1].data_len()
                == contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES
            && accounts[IX_SELLER_REPLAY_GEN1].data_len()
                == contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    let root_account = id(accounts[IX_SETTLEMENT_ROOT].key);
    let entry_receipt = SettlementReceiptAccountV5::decode(&borrow_data(
        &accounts[first_receipt],
    )?)?;
    let entry_semantic = entry_receipt.semantic();
    let root_authority = authenticate_readonly_general_settlement_root_v1(
        program_id,
        core::slice::from_ref(&accounts[IX_SETTLEMENT_ROOT]),
        request.epoch,
        Id32::new(entry_semantic.candidate.0)?,
    )?;
    require(root_authority.is_indexed(), ClutchError::MismatchedState)?;
    let root = *root_authority.root();
    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    require(
        request.epoch == root.epoch()
            && request.settlement_root == root_account
            && *accounts[IX_SETTLEMENT_ROOT].key == root_pda.0
            && root.stored_bump() == root_pda.1
            && root.phase() == contract::SettlementRootPhaseV1::Settling
            && root.retained_feed_state() == contract::SettlementRootChildStateV1::Live,
        ClutchError::MismatchedState,
    )?;
    let root_pre_data_id = root_authority.data_id(&RuntimeSha256)?;

    let feed_account = id(accounts[IX_RETAINED_FEED].key);
    let feed_data = borrow_data(&accounts[IX_RETAINED_FEED])?;
    let (feed, feed_economics) = decode_sealed_candidate_feed_v1(&feed_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (_, feed_tail) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    let feed_data_id = contract::candidate_bundle_digest_v1(&RuntimeSha256, &feed_data, true)?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &root.source_admission_node().bytes());
    let counts = root.counts();
    require(
        root.retained_feed() == feed_account
            && *accounts[IX_RETAINED_FEED].key == feed_pda.0
            && feed.stored_bump == feed_pda.1
            && feed.epoch == root.epoch()
            && feed.market == root.market()
            && feed.node == root.source_admission_node()
            && feed.order_set == root.order_set()
            && feed.epoch_generation == root.epoch_generation()
            && feed.order_count == root.order_count()
            && feed.outcome_count == root.outcome_count()
            && feed.settlement_candidate_id == root.settlement_candidate_id()
            && feed.settlement_witness_digest == root.settlement_witness_digest()
            && feed_data_id == root.candidate_bundle_digest()
            && feed.candidate_kind == contract::SettlementCandidateKindV1::Direct
            && feed.settlement_candidate_id == feed.base_relation_candidate_id
            && counts.expected_receipts == feed.slice_count
            && counts.admitted_receipts == counts.expected_receipts
            && counts.live_receipts == counts.expected_receipts
            && counts.expected_filled_reservations == 2
            && counts.admitted_reservations == counts.expected_filled_reservations
            && counts.live_reservations == counts.expected_filled_reservations
            && usize::from(feed.slice_count) == receipt_count,
        ClutchError::MismatchedState,
    )?;

    let domain =
        contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[IX_ECONOMIC_DOMAIN])?)?;
    let domain_pda = seeds::general_v2_economic_domain_pda(program_id, &root.epoch().bytes());
    let domain_digest = contract::economic_domain_digest_v2(&RuntimeSha256, domain.transcript)?;
    require(
        *accounts[IX_ECONOMIC_DOMAIN].key == domain_pda.0
            && domain.stored_bump == domain_pda.1
            && domain.epoch == root.epoch()
            && domain.transcript.market_instance_v2_id == root.market_instance_v2_id()
            && domain.transcript.outcome_count == root.outcome_count()
            && domain_digest == feed.economic_domain_digest,
        ClutchError::MismatchedState,
    )?;

    let binding = *current.binding().base();
    let binding_pda = seeds::general_v2_market_binding_pda(
        program_id,
        &binding.base().market_instance_v2_id.bytes(),
    );
    require(
        request.settlement_root == root_account
            && root.market_binding() == id(&current.binding_account())
            && current.binding_account() == *accounts[CURRENT_V5_IX_MARKET_BINDING].key
            && current.binding_account() == binding_pda.0
            && root.market() == Id32::from_bytes(current.runtime_account().to_bytes())
            && binding.base().stored_bump == binding_pda.1
            && binding.base().market == root.market()
            && binding.base().market_instance_v2_id == root.market_instance_v2_id()
            && binding.batch_policy_id() == root.batch_policy_id()
            && binding.base().score_policy_id == root.score_policy_id()
            && binding.base().outcome_count == root.outcome_count()
            && binding.base().relation_policy_id == feed.relation_policy_id
            && binding.base().price_measure_policy_v1_id == feed.price_measure_policy_v1_id,
        ClutchError::MismatchedState,
    )?;

    let grid = PriceGridAccount::decode(&borrow_data(&accounts[IX_PRICE_GRID])?)?;
    let grid_pda = seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes());
    require(
        *accounts[IX_PRICE_GRID].key == grid_pda.0
            && grid.stored_bump == grid_pda.1
            && grid.price_scale == domain.transcript.price_scale,
        ClutchError::MismatchedState,
    )?;

    let projection = project_complete_pages(
        accounts,
        page_count,
        root.order_set(),
        &domain,
        &binding,
        &grid,
    )?;
    require(
        usize::from(projection.page_count()) == page_count
            && projection.base().book().len == root.order_count()
            && projection.base().economic_domain_digest() == domain_digest,
        ClutchError::MismatchedState,
    )?;
    let economic_domain = *projection.base().domain();

    let mut page_account_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    let mut page_semantic_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    page = 0;
    while page < page_count {
        let account = &accounts[IX_FIRST_ORDER_PAGE_V5 + page];
        let page_data = borrow_data(account)?;
        let decoded = verify_page_v5(&page_data)?;
        let expected_pda = seeds::general_v2_order_page_v5_pda(
            program_id,
            &root.epoch().bytes(),
            decoded.page_index,
        );
        require(
            usize::from(decoded.page_index) == page
                && *account.key == expected_pda.0
                && decoded.stored_bump == expected_pda.1,
            ClutchError::WrongPda,
        )?;
        page_account_ids[page] = account.key.to_bytes();
        page_semantic_ids[page] = decoded.page_digest.bytes();
        page += 1;
    }

    let mut receipt_layout = super::orders_batch::boxed_copy_of(&EMPTY_RECEIPT_LAYOUT_V2)?;
    let mut receipt_accounts = super::orders_batch::boxed_copy_of(&EMPTY_RECEIPT_ACCOUNTS_V2)?;
    let mut receipt_prestate = super::orders_batch::boxed_copy_of(&EMPTY_RECEIPT_PRESTATE_SET_V2)?;
    let mut receipt_traversal = super::orders_batch::boxed_copy_of(&EMPTY_RECEIPT_TRAVERSAL_V2)?;
    let mut entry_slice: Option<contract::SettlementSliceV1> = None;
    receipt_index = 0;
    while receipt_index < receipt_count {
        let account = &accounts[first_receipt + receipt_index];
        let receipt_account = Hash32::new(account.key.to_bytes())?;
        let (receipt, receipt_evidence) =
            project_settlement_receipt_evidence_v5(receipt_account, &borrow_data(account)?)?;
        let semantic = receipt.semantic();
        let canonical_slice_index =
            u16::try_from(receipt_index).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let canonical_sequence = u64::from(canonical_slice_index)
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let receipt_pda = seeds::general_v2_receipt_v5_pda(
            program_id,
            &root.epoch().bytes(),
            &root.settlement_candidate_id().bytes(),
            canonical_slice_index,
        );
        let slice_at = receipt_index
            .checked_mul(contract::SETTLEMENT_SLICE_BYTES)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let slice_end = slice_at
            .checked_add(contract::SETTLEMENT_SLICE_BYTES)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let slice = contract::SettlementSliceV1::decode(
            feed_tail
                .slices_le()
                .get(slice_at..slice_end)
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
            feed.order_count,
            feed.outcome_count,
        )?;
        require(
            semantic.slice_index == canonical_slice_index
                && semantic.sequence == canonical_sequence
                && *account.key == receipt_pda.0
                && semantic.stored_bump == receipt_pda.1
                && receipt.transition()
                    == SettlementReceiptTransitionCommitmentV5::PortfolioPairPending
                && semantic.epoch.bytes() == root.epoch().bytes()
                && semantic.market.bytes() == root.market().bytes()
                && semantic.candidate.bytes() == root.settlement_candidate_id().bytes()
                && semantic.accounted_end_mask == semantic.expected_end_mask()
                && semantic.delivered_end_mask() == 0
                && slice.buy_kind == contract::SettlementSliceLegKindV1::Order
                && slice.sell_kind == contract::SettlementSliceLegKindV1::Order
                && slice.outcome == semantic.outcome
                && slice.quantity == semantic.quantity
                && semantic.price == feed_economics.prices[usize::from(slice.outcome)],
            ClutchError::MismatchedState,
        )?;
        if let Some(entry) = entry_slice {
            require(
                slice.buy_index == entry.buy_index
                    && slice.sell_index == entry.sell_index
                    && slice.outcome > entry.outcome,
                ClutchError::MismatchedState,
            )?;
        } else {
            require(
                request.receipt == id(account.key),
                ClutchError::MismatchedState,
            )?;
            entry_slice = Some(slice);
        }
        receipt_layout[receipt_index] = Some(receipt);
        receipt_accounts[receipt_index] = receipt_account;
        receipt_prestate.receipts[receipt_index] = PortfolioSettlementReceiptV5Prestate {
            account_id: account.key.to_bytes(),
            pre_data_id: receipt_evidence.receipt_data_id().bytes(),
            slice_index: semantic.slice_index,
            sequence: semantic.sequence,
            outcome: semantic.outcome,
            quantity: semantic.quantity,
            price: semantic.price,
            accounted_end_mask: semantic.accounted_end_mask,
            delivered_end_mask: semantic.delivered_end_mask(),
            expected_end_mask: semantic.expected_end_mask(),
            transition_kind: SettlementReceiptTransitionKindV2::PortfolioPairV2,
            transition_commitment: [0; 32],
            rent_owner_id: receipt.rent().payer.bytes(),
            rent_principal_lamports: receipt.rent().refundable_principal,
            rent_donation_floor_lamports: receipt.rent().donation_floor,
        };
        receipt_traversal[receipt_index] = PortfolioReceiptSiblingTraversalV2 {
            slice_index: semantic.slice_index,
            sequence: semantic.sequence,
            buy_order_index: slice.buy_index,
            sell_order_index: slice.sell_index,
            outcome: semantic.outcome,
            quantity: semantic.quantity,
            price: semantic.price,
        };
        receipt_index += 1;
    }
    let slice = entry_slice.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    receipt_prestate.receipt_count = request.receipt_count;
    let entry_receipt_semantic = receipt_layout[0]
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
        .semantic();
    receipt_index = 0;
    while receipt_index < receipt_count {
        let semantic = receipt_layout[receipt_index]
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
            .semantic();
        require(
            semantic.buy_order_id == entry_receipt_semantic.buy_order_id
                && semantic.sell_order_id == entry_receipt_semantic.sell_order_id,
            ClutchError::MismatchedState,
        )?;
        receipt_index += 1;
    }

    let buy_membership = projection
        .base()
        .order_membership(slice.buy_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sell_membership = projection
        .base()
        .order_membership(slice.sell_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        buy_membership.kind() == clutch_general_v2_runtime::FrozenOrderKindV1::Portfolio
            && sell_membership.kind() == clutch_general_v2_runtime::FrozenOrderKindV1::Portfolio
            && buy_membership.order_id().bytes() == entry_receipt_semantic.buy_order_id.bytes()
            && sell_membership.order_id().bytes() == entry_receipt_semantic.sell_order_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let buy_page_index = projection
        .order_page_index(slice.buy_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sell_page_index = projection
        .order_page_index(slice.sell_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let buy_page_account = projection
        .order_page_account(slice.buy_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sell_page_account = projection
        .order_page_account(slice.sell_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let buy_position_generation = projection
        .position_generation(slice.buy_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sell_position_generation = projection
        .position_generation(slice.sell_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;

    let (buyer_position, buyer_replay) = authenticate_position_replay(
        program_id,
        root,
        buy_membership.owner().bytes(),
        &accounts[IX_BUYER_POSITION_V3],
        &accounts[IX_BUYER_REPLAY_GEN1],
    )?;
    let (seller_position, seller_replay) = authenticate_position_replay(
        program_id,
        root,
        sell_membership.owner().bytes(),
        &accounts[IX_SELLER_POSITION_V3],
        &accounts[IX_SELLER_REPLAY_GEN1],
    )?;
    require(
        buyer_position.semantic.fields().generation == buy_position_generation
            && seller_position.semantic.fields().generation == sell_position_generation,
        ClutchError::MismatchedState,
    )?;

    let buy_record = SelectedPortfolioOrderRecordV2 {
        version: PORTFOLIO_EXECUTION_VERSION_V2,
        outcome_count: root.outcome_count(),
        source_kind: PortfolioSourceOrderKindV2::Portfolio,
        side: Side::Buy,
        order_index: slice.buy_index,
        page_slot: find_page_slot(
            accounts,
            buy_page_index,
            buy_membership.order_id().bytes(),
            buy_membership.owner().bytes(),
            0,
            buy_membership.generation(),
            buy_position_generation,
        )?,
        traversal_index: entry_receipt_semantic.slice_index,
        page_index: buy_page_index,
        settlement_root_epoch_generation: root.epoch_generation(),
        position_generation: buy_position_generation,
        selected_fill_units: feed_economics.fills[usize::from(slice.buy_index)],
        market_semantics_digest: economic_domain.market_semantics_digest,
        epoch_semantics_digest: economic_domain.epoch_semantics_digest,
        economic_candidate_digest: feed.base_relation_candidate_id.bytes(),
        order_set_digest: root.order_set().bytes(),
        settlement_root_account_id: root_account.bytes(),
        settlement_root_pre_semantic_id: root_pre_data_id.bytes(),
        settlement_candidate_id: root.settlement_candidate_id().bytes(),
        retained_feed_account_id: feed_account.bytes(),
        retained_feed_semantic_id: feed_data_id.bytes(),
        settlement_witness_id: root.settlement_witness_digest().bytes(),
        order_page_account_id: buy_page_account.bytes(),
        order_page_semantic_id: *page_semantic_ids
            .get(usize::from(buy_page_index))
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        position_account_id: buyer_position.account,
        position_pre_semantic_id: buyer_position.semantic_id,
        order_id: buy_membership.order_id().bytes(),
        owner_id: buy_membership.owner().bytes(),
    };
    let sell_record = SelectedPortfolioOrderRecordV2 {
        version: PORTFOLIO_EXECUTION_VERSION_V2,
        outcome_count: root.outcome_count(),
        source_kind: PortfolioSourceOrderKindV2::Portfolio,
        side: Side::Sell,
        order_index: slice.sell_index,
        page_slot: find_page_slot(
            accounts,
            sell_page_index,
            sell_membership.order_id().bytes(),
            sell_membership.owner().bytes(),
            1,
            sell_membership.generation(),
            sell_position_generation,
        )?,
        traversal_index: entry_receipt_semantic.slice_index,
        page_index: sell_page_index,
        settlement_root_epoch_generation: root.epoch_generation(),
        position_generation: sell_position_generation,
        selected_fill_units: feed_economics.fills[usize::from(slice.sell_index)],
        market_semantics_digest: economic_domain.market_semantics_digest,
        epoch_semantics_digest: economic_domain.epoch_semantics_digest,
        economic_candidate_digest: feed.base_relation_candidate_id.bytes(),
        order_set_digest: root.order_set().bytes(),
        settlement_root_account_id: root_account.bytes(),
        settlement_root_pre_semantic_id: root_pre_data_id.bytes(),
        settlement_candidate_id: root.settlement_candidate_id().bytes(),
        retained_feed_account_id: feed_account.bytes(),
        retained_feed_semantic_id: feed_data_id.bytes(),
        settlement_witness_id: root.settlement_witness_digest().bytes(),
        order_page_account_id: sell_page_account.bytes(),
        order_page_semantic_id: *page_semantic_ids
            .get(usize::from(sell_page_index))
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        position_account_id: seller_position.account,
        position_pre_semantic_id: seller_position.semantic_id,
        order_id: sell_membership.order_id().bytes(),
        owner_id: sell_membership.owner().bytes(),
    };

    let page_set = PortfolioBookPageSetRecordV2 {
        version: PORTFOLIO_BOOK_AUTHORITY_VERSION_V2,
        outcome_count: root.outcome_count(),
        page_count: u8::try_from(page_count)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
        order_count: root.order_count(),
        traversal_index: entry_receipt_semantic.slice_index,
        settlement_root_epoch_generation: root.epoch_generation(),
        market_semantics_digest: economic_domain.market_semantics_digest,
        epoch_semantics_digest: economic_domain.epoch_semantics_digest,
        order_set_digest: root.order_set().bytes(),
        settlement_root_account_id: root_account.bytes(),
        settlement_root_pre_semantic_id: root_pre_data_id.bytes(),
        retained_feed_account_id: feed_account.bytes(),
        retained_feed_semantic_id: feed_data_id.bytes(),
        settlement_candidate_id: root.settlement_candidate_id().bytes(),
        settlement_witness_id: root.settlement_witness_digest().bytes(),
        page_account_ids,
        page_semantic_ids,
    };
    let complete_book_adapter = CompleteBookAdapterV2 {
        program_id: program_id.to_bytes(),
        page_set,
        projection,
    };
    let complete_book = authenticate_complete_portfolio_book_ref_v2(
        &complete_book_adapter,
        program_id.to_bytes(),
        &economic_domain,
        page_set,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let candidate = EconomicCandidateV2 {
        fills: feed_economics.fills,
        honored_aon_mask: feed.honored_aon_mask,
        virtual_split: feed.virtual_split,
        virtual_merge: feed.virtual_merge,
    };
    let price = PricePreconditionV2 {
        policy_digest: domain.transcript.price_measure_policy_v1_id.bytes(),
        semantic_price_digest: feed.candidate_price_digest.bytes(),
        prices: feed_economics.prices,
    };

    let buyer_reservation =
        ReservationAccountV9::decode(&borrow_data(&accounts[IX_BUYER_RESERVATION_V9])?)?;
    let seller_reservation =
        ReservationAccountV9::decode(&borrow_data(&accounts[IX_SELLER_RESERVATION_V9])?)?;
    let buyer_reservation_body = buyer_reservation.body();
    let seller_reservation_body = seller_reservation.body();
    let buyer_reservation_pda = seeds::general_v2_reservation_v9_pda(
        program_id,
        &buyer_reservation_body.reservation.bytes(),
    );
    let seller_reservation_pda = seeds::general_v2_reservation_v9_pda(
        program_id,
        &seller_reservation_body.reservation.bytes(),
    );
    require(
        *accounts[IX_BUYER_RESERVATION_V9].key == buyer_reservation_pda.0
            && buyer_reservation_body.stored_bump == buyer_reservation_pda.1
            && *accounts[IX_SELLER_RESERVATION_V9].key == seller_reservation_pda.0
            && seller_reservation_body.stored_bump == seller_reservation_pda.1
            && buyer_reservation_body.market.bytes() == root.market().bytes()
            && seller_reservation_body.market.bytes() == root.market().bytes()
            && buyer_reservation_body.epoch.bytes() == root.epoch().bytes()
            && seller_reservation_body.epoch.bytes() == root.epoch().bytes()
            && buyer_reservation_body.owner.bytes() == buy_record.owner_id
            && seller_reservation_body.owner.bytes() == sell_record.owner_id
            && buyer_reservation_body.order_id.bytes() == buy_record.order_id
            && seller_reservation_body.order_id.bytes() == sell_record.order_id
            && buyer_reservation_body.position_generation == buy_record.position_generation
            && seller_reservation_body.position_generation == sell_record.position_generation
            && buyer_reservation_body.order_generation == buy_membership.generation()
            && seller_reservation_body.order_generation == sell_membership.generation()
            && buyer_reservation_body.page_index == buy_page_index
            && seller_reservation_body.page_index == sell_page_index
            && buyer_reservation_body.price_grid == grid.grid
            && seller_reservation_body.price_grid == grid.grid
            && buyer_reservation_body.order_kind == ORDER_KIND_PORTFOLIO
            && seller_reservation_body.order_kind == ORDER_KIND_PORTFOLIO
            && buyer_reservation_body.side == 0
            && seller_reservation_body.side == 1
            && buyer_reservation_body.state == RESERVATION_STATE_ENTITLED
            && seller_reservation_body.state == RESERVATION_STATE_ENTITLED
            && buyer_reservation_body.outcome_count == root.outcome_count()
            && seller_reservation_body.outcome_count == root.outcome_count(),
        ClutchError::MismatchedState,
    )?;

    let selected_adapter_seed = ExecutionAdapterV2 {
        program_id: program_id.to_bytes(),
        selected: [buy_record, sell_record],
        account_expectations: Box::new([
            account_expectation(
                PortfolioAccountRoleV2::SettlementRoot,
                root_account.bytes(),
                program_id.to_bytes(),
                root_pre_data_id.bytes(),
                Some(root.epoch_generation()),
                false,
            ),
            account_expectation(
                PortfolioAccountRoleV2::RetainedFeed,
                feed_account.bytes(),
                program_id.to_bytes(),
                feed_data_id.bytes(),
                None,
                false,
            ),
            account_expectation(
                PortfolioAccountRoleV2::OrderPage,
                buy_record.order_page_account_id,
                program_id.to_bytes(),
                buy_record.order_page_semantic_id,
                None,
                false,
            ),
            account_expectation(
                PortfolioAccountRoleV2::OrderPage,
                sell_record.order_page_account_id,
                program_id.to_bytes(),
                sell_record.order_page_semantic_id,
                None,
                false,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Position,
                buyer_position.account,
                program_id.to_bytes(),
                buyer_position.semantic_id,
                Some(buy_position_generation),
                true,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Position,
                seller_position.account,
                program_id.to_bytes(),
                seller_position.semantic_id,
                Some(sell_position_generation),
                true,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Reservation,
                accounts[IX_BUYER_RESERVATION_V9].key.to_bytes(),
                program_id.to_bytes(),
                buyer_reservation.data_id()?.bytes(),
                Some(buyer_reservation_body.order_generation),
                true,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Reservation,
                accounts[IX_SELLER_RESERVATION_V9].key.to_bytes(),
                program_id.to_bytes(),
                seller_reservation.data_id()?.bytes(),
                Some(seller_reservation_body.order_generation),
                true,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Replay,
                accounts[IX_BUYER_REPLAY_GEN1].key.to_bytes(),
                program_id.to_bytes(),
                buyer_replay.replay_semantic_id().bytes(),
                None,
                true,
            ),
            account_expectation(
                PortfolioAccountRoleV2::Replay,
                accounts[IX_SELLER_REPLAY_GEN1].key.to_bytes(),
                program_id.to_bytes(),
                seller_replay.replay_semantic_id().bytes(),
                None,
                true,
            ),
        ]),
        transition_expectations: Box::new([replay_transition_expectation_placeholder(); 6]),
        receipt_pre: receipt_layout,
        receipt_prestate,
        receipt_accounts,
        receipt_post: Cell::new(None),
    };

    let buyer_selected = authenticate_selected_portfolio_order_v2(
        &selected_adapter_seed,
        program_id.to_bytes(),
        &economic_domain,
        complete_book.economic_book(),
        &candidate,
        feed.base_relation_candidate_id.bytes(),
        buy_record,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let seller_selected = authenticate_selected_portfolio_order_v2(
        &selected_adapter_seed,
        program_id.to_bytes(),
        &economic_domain,
        complete_book.economic_book(),
        &candidate,
        feed.base_relation_candidate_id.bytes(),
        sell_record,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pair = Box::new(
        authenticate_exact_portfolio_pair_v2(
            &economic_domain,
            complete_book.economic_book(),
            &price,
            &candidate,
            PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
            buyer_selected,
            seller_selected,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    );
    let sibling_set = Box::new(
        authenticate_portfolio_receipt_sibling_set_v2(
            *pair,
            PortfolioReceiptSiblingTraversalSetV2 {
                sibling_count: request.receipt_count,
                siblings: *receipt_traversal,
            },
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    );
    require(
        usize::from(sibling_set.sibling_count()) == receipt_count,
        ClutchError::MismatchedState,
    )?;

    let buyer_reservation_post = consumed_reservation(buyer_reservation, pair.pair_units())?;
    let seller_reservation_post = consumed_reservation(seller_reservation, pair.pair_units())?;
    let buyer_fields = buyer_position.semantic.fields();
    let seller_fields = seller_position.semantic.fields();
    let buyer_cash = buyer_fields
        .cash_atoms
        .checked_sub(pair.consideration_atoms())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let buyer_reserved_cash = buyer_fields
        .reserved_cash_atoms
        .checked_sub(buyer_reservation_body.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let seller_cash = seller_fields
        .cash_atoms
        .checked_add(pair.consideration_atoms())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut buyer_eggs = buyer_fields.native_eggs;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        buyer_eggs[outcome] = buyer_eggs[outcome]
            .checked_add(pair.payoff()[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        outcome += 1;
    }
    let buyer_position_post = buyer_position
        .settlement_poststate(buyer_cash, buyer_reserved_cash, buyer_eggs)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let seller_position_post = seller_position
        .settlement_poststate(
            seller_cash,
            seller_fields.reserved_cash_atoms,
            seller_fields.native_eggs,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let buyer_position_post_id = buyer_position_post
        .semantic
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let seller_position_post_id = seller_position_post
        .semantic
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let entry_receipt_evidence = selected_adapter_seed.receipt_pre[0]
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
        .evidence(selected_adapter_seed.receipt_accounts[0])?;
    let transition_id = Id32::new(entry_receipt_evidence.delivery_transition_id().bytes())?;
    let transition_evidence_id = Id32::new(
        portfolio_settlement_receipt_v5_set_digest_v2(&selected_adapter_seed.receipt_prestate)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )?;
    let buyer_replay_post = project_general_replay_transition_v1(
        buyer_replay,
        buyer_position_post,
        GeneralReplayTransitionKindV1::PortfolioPairBuyer,
        transition_id,
        transition_evidence_id,
        &RuntimeSha256,
    )?;
    let seller_replay_post = project_general_replay_transition_v1(
        seller_replay,
        seller_position_post,
        GeneralReplayTransitionKindV1::PortfolioPairSeller,
        transition_id,
        transition_evidence_id,
        &RuntimeSha256,
    )?;

    let buyer_endpoint = Box::new(EndpointPlanV2 {
        selected: buy_record,
        reservation_pre: buyer_reservation,
        reservation_pre_data_id: buyer_reservation.data_id()?.bytes(),
        reservation_post: buyer_reservation_post,
        reservation_post_data_id: buyer_reservation_post.data_id()?.bytes(),
        position_pre: buyer_position,
        position_post: buyer_position_post,
        position_post_semantic_id: buyer_position_post_id,
        replay_pre: buyer_replay,
        replay_post: buyer_replay_post,
    });
    let seller_endpoint = Box::new(EndpointPlanV2 {
        selected: sell_record,
        reservation_pre: seller_reservation,
        reservation_pre_data_id: seller_reservation.data_id()?.bytes(),
        reservation_post: seller_reservation_post,
        reservation_post_data_id: seller_reservation_post.data_id()?.bytes(),
        position_pre: seller_position,
        position_post: seller_position_post,
        position_post_semantic_id: seller_position_post_id,
        replay_pre: seller_replay,
        replay_post: seller_replay_post,
    });

    let transition_expectations = Box::new([
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Reservation,
            account_id: accounts[IX_BUYER_RESERVATION_V9].key.to_bytes(),
            pre_semantic_id: buyer_endpoint.reservation_pre_data_id,
            post_semantic_id: buyer_endpoint.reservation_post_data_id,
            stable_generation: Some(buyer_reservation_body.order_generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: buyer_reservation_body.remaining_cash_atoms,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: true,
        },
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Reservation,
            account_id: accounts[IX_SELLER_RESERVATION_V9].key.to_bytes(),
            pre_semantic_id: seller_endpoint.reservation_pre_data_id,
            post_semantic_id: seller_endpoint.reservation_post_data_id,
            stable_generation: Some(seller_reservation_body.order_generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: 0,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: *pair.payoff(),
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: true,
        },
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Position,
            account_id: buyer_position.account,
            pre_semantic_id: buyer_position.semantic_id,
            post_semantic_id: buyer_position_post_id,
            stable_generation: Some(buy_position_generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: pair.consideration_atoms(),
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: buyer_reservation_body.remaining_cash_atoms,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: *pair.payoff(),
            reservation_consumed: false,
        },
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Position,
            account_id: seller_position.account,
            pre_semantic_id: seller_position.semantic_id,
            post_semantic_id: seller_position_post_id,
            stable_generation: Some(sell_position_generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: 0,
            cash_credit_atoms: pair.consideration_atoms(),
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: false,
        },
        replay_transition_expectation(&buyer_endpoint),
        replay_transition_expectation(&seller_endpoint),
    ]);
    let adapter = ExecutionAdapterV2 {
        transition_expectations,
        ..selected_adapter_seed
    };
    let mut input = super::orders_batch::boxed_copy_of(&EMPTY_EXECUTION_INPUT_V2)?;
    input.settlement_receipts = *adapter.receipt_prestate;
    input.buyer_reservation = PortfolioReservationPrestateV2 {
            account_id: accounts[IX_BUYER_RESERVATION_V9].key.to_bytes(),
            semantic_id: buyer_endpoint.reservation_pre_data_id,
            generation: buyer_reservation_body.order_generation,
            lifecycle: PortfolioReservationLifecycleV2::Entitled,
            owner_id: buy_record.owner_id,
            order_id: buy_record.order_id,
            position_account_id: buyer_position.account,
            position_generation: buy_position_generation,
            entitled_units: buyer_reservation_body.entitled_units,
            consumed_units: buyer_reservation_body.consumed_units,
            paid_units: buyer_reservation_body.paid_units,
            remaining_cash_atoms: buyer_reservation_body.remaining_cash_atoms,
            remaining_claim_atoms: buyer_reservation_body.remaining_internal,
            maximum_fee_atoms: buyer_reservation_body.max_fee_atoms,
        };
    input.seller_reservation = PortfolioReservationPrestateV2 {
            account_id: accounts[IX_SELLER_RESERVATION_V9].key.to_bytes(),
            semantic_id: seller_endpoint.reservation_pre_data_id,
            generation: seller_reservation_body.order_generation,
            lifecycle: PortfolioReservationLifecycleV2::Entitled,
            owner_id: sell_record.owner_id,
            order_id: sell_record.order_id,
            position_account_id: seller_position.account,
            position_generation: sell_position_generation,
            entitled_units: seller_reservation_body.entitled_units,
            consumed_units: seller_reservation_body.consumed_units,
            paid_units: seller_reservation_body.paid_units,
            remaining_cash_atoms: seller_reservation_body.remaining_cash_atoms,
            remaining_claim_atoms: seller_reservation_body.remaining_internal,
            maximum_fee_atoms: seller_reservation_body.max_fee_atoms,
        };
    input.buyer_position = PortfolioPositionPrestateV2 {
            account_id: buyer_position.account,
            semantic_id: buyer_position.semantic_id,
            owner_id: buy_record.owner_id,
            generation: buy_position_generation,
            cash_atoms: buyer_fields.cash_atoms,
            reserved_cash_atoms: buyer_fields.reserved_cash_atoms,
            native_eggs: buyer_fields.native_eggs,
            outstanding_reservations: buyer_fields.outstanding_reservations,
        };
    input.seller_position = PortfolioPositionPrestateV2 {
            account_id: seller_position.account,
            semantic_id: seller_position.semantic_id,
            owner_id: sell_record.owner_id,
            generation: sell_position_generation,
            cash_atoms: seller_fields.cash_atoms,
            reserved_cash_atoms: seller_fields.reserved_cash_atoms,
            native_eggs: seller_fields.native_eggs,
            outstanding_reservations: seller_fields.outstanding_reservations,
        };
    input.buyer_replay = PortfolioReplayPrestateV2 {
            account_id: buyer_replay.replay_account().bytes(),
            semantic_id: buyer_replay.replay_semantic_id().bytes(),
            ordinal: buyer_replay.next_sequence(),
        };
    input.seller_replay = PortfolioReplayPrestateV2 {
            account_id: seller_replay.replay_account().bytes(),
            semantic_id: seller_replay.replay_semantic_id().bytes(),
            ordinal: seller_replay.next_sequence(),
        };
    input.post_semantic_ids = PortfolioPairPostSemanticIdsV2 {
            buyer_reservation: buyer_endpoint.reservation_post_data_id,
            seller_reservation: seller_endpoint.reservation_post_data_id,
            buyer_position: buyer_position_post_id,
            seller_position: seller_position_post_id,
            buyer_replay: buyer_replay_post.replay_poststate_semantic_id().bytes(),
            seller_replay: seller_replay_post.replay_poststate_semantic_id().bytes(),
            settlement_receipts: [[0; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
        };
    let prepared = prepare_portfolio_pair_execution_borrowed_v2(
        &adapter,
        program_id.to_bytes(),
        &pair,
        &input,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let receipt_post = adapter
        .receipt_post
        .take()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut preimage = [0u8; PORTFOLIO_PAIR_RECEIPT_V2_BYTES];
    prepared
        .receipt()
        .encode_into(&mut preimage)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let layout_commitment = layout_portfolio_commitment_v2(&preimage)?;
    require(
        layout_commitment.bytes() == prepared.transition_commitment(),
        ClutchError::MismatchedState,
    )?;
    receipt_index = 0;
    while receipt_index < receipt_count {
        let post =
            receipt_post[receipt_index].ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            post.data_id(adapter.receipt_accounts[receipt_index])?
                .bytes()
                == prepared.post_semantic_ids().settlement_receipts[receipt_index],
            ClutchError::MismatchedState,
        )?;
        receipt_index += 1;
    }

    write_reservation_post(
        &accounts[IX_BUYER_RESERVATION_V9],
        buyer_endpoint.reservation_post,
    )?;
    write_reservation_post(
        &accounts[IX_SELLER_RESERVATION_V9],
        seller_endpoint.reservation_post,
    )?;
    write_position_post(
        &accounts[IX_BUYER_POSITION_V3],
        &buyer_endpoint.position_post,
    )?;
    write_position_post(
        &accounts[IX_SELLER_POSITION_V3],
        &seller_endpoint.position_post,
    )?;
    write_replay_post(&accounts[IX_BUYER_REPLAY_GEN1], &buyer_endpoint.replay_post)?;
    write_replay_post(
        &accounts[IX_SELLER_REPLAY_GEN1],
        &seller_endpoint.replay_post,
    )?;
    receipt_index = 0;
    while receipt_index < receipt_count {
        let receipt_post_bytes = receipt_post[receipt_index]
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
            .encode_exact()?;
        write_exact(
            &accounts[first_receipt + receipt_index],
            &receipt_post_bytes,
        )?;
        receipt_index += 1;
    }
    Ok(())
}

const fn replay_transition_expectation_placeholder() -> PortfolioTransitionExpectationV2 {
    PortfolioTransitionExpectationV2 {
        role: PortfolioAccountRoleV2::Replay,
        account_id: [0; 32],
        pre_semantic_id: [0; 32],
        post_semantic_id: [0; 32],
        stable_generation: None,
        pre_replay_ordinal: 0,
        post_replay_ordinal: 0,
        cash_debit_atoms: 0,
        cash_credit_atoms: 0,
        reserved_cash_release_atoms: 0,
        claim_debits: [0; MAX_OUTCOMES],
        claim_credits: [0; MAX_OUTCOMES],
        reservation_consumed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_frame_binds_complete_page_and_receipt_suffixes() {
        assert_eq!(
            account_frame(PORTFOLIO_PAIR_MIN_ACCOUNTS_V2, 1, 1),
            Ok((1, 1))
        );
        assert_eq!(
            account_frame(PORTFOLIO_PAIR_MAX_ACCOUNTS_V2, 4, 16),
            Ok((4, 16))
        );
        assert_eq!(
            account_frame(PORTFOLIO_PAIR_MAX_ACCOUNTS_V2 - 1, 4, 16),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
        assert_eq!(
            account_frame(PORTFOLIO_PAIR_MIN_ACCOUNTS_V2, 0, 2),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
        assert_eq!(
            account_frame(PORTFOLIO_PAIR_MIN_ACCOUNTS_V2, 2, 0),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
    }

    #[test]
    fn kernel_and_layout_commitment_domains_are_byte_exact() {
        assert_eq!(
            LAYOUT_PORTFOLIO_DOMAIN_V2,
            &PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2[..]
        );
    }
}
