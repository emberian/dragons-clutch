// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exhaustive action-44 coefficient-portfolio archive retirement.
//!
//! This module owns no coefficient, fill, rent, or account bytes. It joins the
//! counted SettlementRoot and complete retained-Feed traversal to the exact
//! committed Receipt V5 active prefix, the two consumed Reservation V9
//! accounts, their Position V3/GEN1 children, and the persisted rent owners.
//! The returned private capability is indivisible: every sibling and both
//! Reservations close, both Position child counts and Replays advance, and
//! the root counters change once, or no plan exists.

use clutch_batch::portfolio_execution_v2::{
    PORTFOLIO_EXECUTION_VERSION_V2, PORTFOLIO_PAIR_RECEIPT_SET_DOMAIN_V2,
    PORTFOLIO_PAIR_RECEIPT_TRANSITION_KIND_V2_BYTE,
};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, complete_candidate_feed_v2,
    project_general_replay_transition_v1, verify_general_replay_last_transition_v1,
    GeneralPositionReplayPrestateV1,
    GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1, Id32, MarketBindingV2,
    RetirePortfolioPairArchivesPayloadV1, SettlementCandidateKindV1,
    SettlementRootChildStateV1, SettlementRootPhaseV1, SettlementRootV1AccountV1,
    SettlementSliceLegKindV1, SettlementSliceV1, Sha256BackendV1, SETTLEMENT_SLICE_BYTES,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    PositionAccountV3, PositionV3Fields, PositionV3Sha256Backend, ReplayV3HashBackend,
    MAX_OUTCOMES,
};
use clutch_solana_layout::reservation::RESERVATION_STATE_CONSUMED;
use clutch_solana_layout::reservation_v9::{DeletableRentOwnerV1, ReservationAccountV9};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SettlementReceiptTransitionCommitmentV5,
};
use clutch_solana_layout::{
    Hash32 as LayoutHash32, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT, ORDER_KIND_PORTFOLIO,
};
use sha2::{Digest, Sha256};

/// Maximum complete Receipt V5 sibling width of one portfolio pair.
pub const PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2: usize = 16;
/// Exactly two consumed Reservation V9 endpoints retire together.
pub const PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2: usize = 2;
/// At most one distinct payer per closed Receipt or Reservation.
pub const PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2: usize = 18;

/// Stable action-44 transition domain. Post-Replay IDs are excluded to avoid
/// circularity and are included by the terminal receipt domain below.
pub const PORTFOLIO_ARCHIVE_TRANSITION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-archive-retirement/transition/v2\0";
/// Stable action-44 evidence domain consumed by both GEN1 successors.
pub const PORTFOLIO_ARCHIVE_EVIDENCE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-archive-retirement/evidence/v2\0";
/// Complete private terminal-receipt domain, including both Replay post-IDs.
pub const PORTFOLIO_ARCHIVE_TERMINAL_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-archive-retirement/terminal/v2\0";

/// One exact committed Receipt V5 account and its observed close balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioArchiveReceiptInputV2 {
    /// Program-authenticated Receipt V5 PDA.
    pub account: Id32,
    /// Exact hostile-byte-decoded Receipt V5 owner.
    pub receipt: SettlementReceiptAccountV5,
    /// Lamports observed before retirement.
    pub balance_lamports: u64,
}

/// One exact consumed Reservation V9 and its observed close balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioArchiveReservationInputV2 {
    /// Program-authenticated Reservation V9 PDA.
    pub account: Id32,
    /// Exact hostile-byte-decoded Reservation V9 owner.
    pub reservation: ReservationAccountV9,
    /// Lamports observed before retirement.
    pub balance_lamports: u64,
}

/// One sorted unique refund-owner meta and its observed prebalance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioArchiveRefundOwnerInputV2 {
    /// Persisted rent payer account.
    pub account: Id32,
    /// Lamports observed before all aggregated principal refunds.
    pub balance_lamports: u64,
}

impl PortfolioArchiveRefundOwnerInputV2 {
    /// Canonical inactive suffix value.
    pub const EMPTY: Self = Self {
        account: Id32::ZERO,
        balance_lamports: 0,
    };
}

/// Complete authenticated input to the pure action-44 planner.
#[derive(Clone, Copy, Debug)]
pub struct RetirePortfolioPairArchivesInputV2<'a> {
    /// Strict action-44 structural selector.
    pub payload: RetirePortfolioPairArchivesPayloadV1,
    /// Counted SettlementRoot account.
    pub settlement_root_account: Id32,
    /// Exact current counted SettlementRoot bytes.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Counted retained sealed Feed account.
    pub retained_feed_account: Id32,
    /// Exact retained sealed Feed body.
    pub retained_feed_body: &'a [u8],
    /// Immutable MarketBinding V2 account.
    pub market_binding_account: Id32,
    /// Exact decoded immutable MarketBinding V2.
    pub market_binding: &'a MarketBindingV2,
    /// Complete active Receipt prefix followed by `None` padding.
    pub receipts: &'a [Option<PortfolioArchiveReceiptInputV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    /// Buyer consumed Reservation V9.
    pub buyer_reservation: &'a PortfolioArchiveReservationInputV2,
    /// Seller consumed Reservation V9.
    pub seller_reservation: &'a PortfolioArchiveReservationInputV2,
    /// Buyer Position authenticated by the outer owner/PDA adapter.
    pub buyer_position: &'a AuthenticatedPositionV3,
    /// Seller Position authenticated by the outer owner/PDA adapter.
    pub seller_position: &'a AuthenticatedPositionV3,
    /// Buyer GEN1 prestate authenticated against the exact Position.
    pub buyer_replay: &'a GeneralPositionReplayPrestateV1,
    /// Seller GEN1 prestate authenticated against the exact Position.
    pub seller_replay: &'a GeneralPositionReplayPrestateV1,
    /// Sorted unique active refund-owner prefix and canonical zero tail.
    pub refund_owners: &'a [PortfolioArchiveRefundOwnerInputV2;
        PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    /// MarketBinding-owned neutral surplus sink.
    pub neutral_sink_account: Id32,
    /// Neutral sink prebalance.
    pub neutral_sink_balance_lamports: u64,
}

/// Exact deletion facts for one Receipt V5 or Reservation V9.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioArchiveClosePlanV2 {
    account: Id32,
    pre_data_id: Id32,
    balance_before: u64,
    payer: Id32,
    principal_refund_lamports: u64,
    surplus_sink_lamports: u64,
}

impl PortfolioArchiveClosePlanV2 {
    /// Exact program account deleted by this plan.
    pub const fn account(&self) -> Id32 { self.account }
    /// Exact current account-data identity before deletion.
    pub const fn pre_data_id(&self) -> Id32 { self.pre_data_id }
    /// Exact observed account balance before deletion.
    pub const fn balance_before(&self) -> u64 { self.balance_before }
    /// Persisted principal owner.
    pub const fn payer(&self) -> Id32 { self.payer }
    /// Principal-only refund credited to the payer.
    pub const fn principal_refund_lamports(&self) -> u64 {
        self.principal_refund_lamports
    }
    /// Donation floor plus unsolicited surplus credited to the neutral sink.
    pub const fn surplus_sink_lamports(&self) -> u64 { self.surplus_sink_lamports }
}

/// One canonical aggregated principal refund.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioArchiveRefundTransferV2 {
    owner: Id32,
    balance_before: u64,
    principal_refund_lamports: u64,
    balance_after: u64,
}

impl PortfolioArchiveRefundTransferV2 {
    /// Canonical inactive suffix.
    pub const EMPTY: Self = Self {
        owner: Id32::ZERO,
        balance_before: 0,
        principal_refund_lamports: 0,
        balance_after: 0,
    };
    /// Persisted refund owner.
    pub const fn owner(&self) -> Id32 { self.owner }
    /// Exact observed prebalance.
    pub const fn balance_before(&self) -> u64 { self.balance_before }
    /// Sum of only persisted refundable principals.
    pub const fn principal_refund_lamports(&self) -> u64 {
        self.principal_refund_lamports
    }
    /// Exact checked postbalance.
    pub const fn balance_after(&self) -> u64 { self.balance_after }
}

/// Private, nonpersisted authority handed to later root retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairArchiveTerminalReceiptV2 {
    terminal_receipt_id: Id32,
    transition_id: Id32,
    transition_evidence_id: Id32,
    settlement_root_account: Id32,
    settlement_root_pre_data_id: Id32,
    settlement_root_post_data_id: Id32,
    transition_commitment: Id32,
    action42_receipt_set_digest: Id32,
    receipt_count: u8,
    refund_owner_count: u8,
    neutral_sink: Id32,
    neutral_sink_credit_lamports: u64,
}

impl PortfolioPairArchiveTerminalReceiptV2 {
    /// Content identity binding the complete action-44 pre/post transcript.
    pub const fn terminal_receipt_id(&self) -> Id32 { self.terminal_receipt_id }
    /// Stable replay transition identity.
    pub const fn transition_id(&self) -> Id32 { self.transition_id }
    /// Stable replay evidence identity.
    pub const fn transition_evidence_id(&self) -> Id32 { self.transition_evidence_id }
    /// Counted root mutated by the transition.
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    /// Exact root pre-data identity.
    pub const fn settlement_root_pre_data_id(&self) -> Id32 {
        self.settlement_root_pre_data_id
    }
    /// Exact root successor data identity.
    pub const fn settlement_root_post_data_id(&self) -> Id32 {
        self.settlement_root_post_data_id
    }
    /// Common immutable action-42 commitment stored by every sibling.
    pub const fn transition_commitment(&self) -> Id32 { self.transition_commitment }
    /// Exact reconstructed pending sibling-set digest consumed by action 42.
    pub const fn action42_receipt_set_digest(&self) -> Id32 {
        self.action42_receipt_set_digest
    }
    /// Exhaustive Receipt sibling count.
    pub const fn receipt_count(&self) -> u8 { self.receipt_count }
    /// Sorted unique refund-owner count.
    pub const fn refund_owner_count(&self) -> u8 { self.refund_owner_count }
    /// Immutable neutral surplus sink.
    pub const fn neutral_sink(&self) -> Id32 { self.neutral_sink }
    /// Aggregate donation floor and unsolicited surplus credit.
    pub const fn neutral_sink_credit_lamports(&self) -> u64 {
        self.neutral_sink_credit_lamports
    }
}

/// One indivisible action-44 root/close/Position/Replay/refund plan.
#[derive(Clone, Copy, Debug)]
pub struct RetirePortfolioPairArchivesPlanV2 {
    settlement_root_poststate: SettlementRootV1AccountV1,
    receipt_closes: [Option<PortfolioArchiveClosePlanV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    reservation_closes: [PortfolioArchiveClosePlanV2;
        PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2],
    buyer_position_poststate: PositionSettlementPoststateV3,
    seller_position_poststate: PositionSettlementPoststateV3,
    buyer_replay_poststate: GeneralReplayTransitionPlanV1,
    seller_replay_poststate: GeneralReplayTransitionPlanV1,
    refund_transfers: [PortfolioArchiveRefundTransferV2;
        PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    neutral_sink_balance_after: u64,
    terminal_receipt: PortfolioPairArchiveTerminalReceiptV2,
}

impl RetirePortfolioPairArchivesPlanV2 {
    /// Exact counted root successor.
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    /// One active Receipt close plan, or `None` outside the active prefix.
    pub fn receipt_close(&self, index: u8) -> Option<&PortfolioArchiveClosePlanV2> {
        self.receipt_closes.get(usize::from(index)).and_then(Option::as_ref)
    }
    /// Buyer then seller Reservation close plans.
    pub const fn reservation_closes(
        &self,
    ) -> &[PortfolioArchiveClosePlanV2; PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2] {
        &self.reservation_closes
    }
    /// Buyer Position child-count successor.
    pub const fn buyer_position_poststate(&self) -> PositionSettlementPoststateV3 {
        self.buyer_position_poststate
    }
    /// Seller Position child-count successor.
    pub const fn seller_position_poststate(&self) -> PositionSettlementPoststateV3 {
        self.seller_position_poststate
    }
    /// Buyer GEN1 successor.
    pub const fn buyer_replay_poststate(&self) -> &GeneralReplayTransitionPlanV1 {
        &self.buyer_replay_poststate
    }
    /// Seller GEN1 successor.
    pub const fn seller_replay_poststate(&self) -> &GeneralReplayTransitionPlanV1 {
        &self.seller_replay_poststate
    }
    /// Canonical sorted refund vector with zero tail.
    pub const fn refund_transfers(
        &self,
    ) -> &[PortfolioArchiveRefundTransferV2; PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2] {
        &self.refund_transfers
    }
    /// Neutral sink checked postbalance.
    pub const fn neutral_sink_balance_after(&self) -> u64 {
        self.neutral_sink_balance_after
    }
    /// Sole private authority consumable by later root retirement.
    pub const fn terminal_receipt(&self) -> PortfolioPairArchiveTerminalReceiptV2 {
        self.terminal_receipt
    }
}

/// Deterministic retirement refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioArchiveRetirementErrorV2 {
    /// A contract-owned codec or state invariant refused.
    Contract,
    /// A layout-owned codec or state invariant refused.
    Layout,
    /// Root, Feed, MarketBinding, or endpoint identities do not join.
    BindingMismatch,
    /// Receipt active prefix is missing, extra, duplicated, or reordered.
    ReceiptSetMismatch,
    /// Reservation state is not exact consumed terminal economic state.
    ReservationMismatch,
    /// Position or Replay does not prove the immediate action-42 endpoint.
    ReplayMismatch,
    /// Refund-owner prefix is not the exact sorted unique persisted set.
    RefundOwnerMismatch,
    /// An account balance cannot cover persisted principal plus donation floor.
    InsufficientCloseBalance,
    /// Checked arithmetic failed.
    ArithmeticOverflow,
}

/// Prepare the sole exhaustive portfolio-pair archive retirement transition.
pub fn prepare_retire_portfolio_pair_archives_v2<B>(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    backend: &B,
) -> Result<RetirePortfolioPairArchivesPlanV2, PortfolioArchiveRetirementErrorV2>
where
    B: Sha256BackendV1 + PositionV3Sha256Backend + ReplayV3HashBackend,
{
    let root = input.settlement_root;
    root.validate().map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    input
        .market_binding
        .validate()
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    if input.payload.epoch != root.epoch()
        || input.payload.settlement_root != input.settlement_root_account
        || input.settlement_root_account.is_zero()
        || input.retained_feed_account != root.retained_feed()
        || input.market_binding_account != root.market_binding()
        || input.market_binding.base().market != root.market()
        || input.market_binding.base().market_instance_v2_id != root.market_instance_v2_id()
        || input.market_binding.base().outcome_count != root.outcome_count()
        || input.market_binding.batch_policy_id() != root.batch_policy_id()
        || input.market_binding.base().score_policy_id != root.score_policy_id()
        || input.neutral_sink_account != input.market_binding.base().neutral_sink
        || input.neutral_sink_account.is_zero()
        || root.phase() != SettlementRootPhaseV1::Settling
        || root.retained_feed_state() != SettlementRootChildStateV1::Live
    {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }
    let fixed_accounts = [
        input.settlement_root_account,
        input.retained_feed_account,
        input.market_binding_account,
        input.neutral_sink_account,
        input.buyer_reservation.account,
        input.seller_reservation.account,
        Id32::from_bytes(input.buyer_position.account),
        Id32::from_bytes(input.seller_position.account),
        input.buyer_replay.replay_account(),
        input.seller_replay.replay_account(),
    ];
    if !distinct_nonzero(&fixed_accounts) {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }

    let (feed, tail) = complete_candidate_feed_v2(input.retained_feed_body, true)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    let feed_bundle = candidate_bundle_digest_v1(backend, input.retained_feed_body, true)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    let receipt_count = usize::from(input.payload.receipt_count);
    let refund_owner_count = usize::from(input.payload.refund_owner_count);
    if receipt_count == 0
        || receipt_count > PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2
        || refund_owner_count == 0
        || refund_owner_count > PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2
        || usize::from(feed.slice_count) != receipt_count
        || root.counts().expected_receipts != u16::from(input.payload.receipt_count)
        || root.counts().admitted_receipts != u16::from(input.payload.receipt_count)
        || root.counts().live_receipts != u16::from(input.payload.receipt_count)
        || feed_bundle != root.candidate_bundle_digest()
        || feed.epoch != root.epoch()
        || feed.market != root.market()
        || feed.order_set != root.order_set()
        || feed.settlement_candidate_id != root.settlement_candidate_id()
        || feed.settlement_witness_digest != root.settlement_witness_digest()
        || feed.epoch_generation != root.epoch_generation()
        || feed.outcome_count != root.outcome_count()
        || feed.order_count != root.order_count()
        || feed.relation_policy_id != input.market_binding.base().relation_policy_id
        || feed.price_measure_policy_v1_id
            != input.market_binding.base().price_measure_policy_v1_id
        || feed.native_claim_basis_id != input.market_binding.base().native_claim_basis_id
        || feed.price_scale != input.market_binding.base().price_scale
        || feed.candidate_kind != SettlementCandidateKindV1::Direct
        || feed.base_relation_candidate_id != root.settlement_candidate_id()
    {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }

    let root_pre_data_id = root
        .data_id(backend, input.settlement_root_account)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    let mut receipt_closes = [None; PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2];
    let mut common_commitment = Id32::ZERO;
    let mut entry_buy_order = Id32::ZERO;
    let mut entry_sell_order = Id32::ZERO;
    let mut entry_delivery_transition = Id32::ZERO;
    let mut entry_buy_index = 0u8;
    let mut entry_sell_index = 0u8;
    let mut prior_outcome = None;
    let mut payoff = [0u64; MAX_OUTCOMES];
    let mut consideration_price_units = 0u128;
    let mut pending_set_hash = Sha256::new();
    pending_set_hash.update(PORTFOLIO_PAIR_RECEIPT_SET_DOMAIN_V2);
    pending_set_hash.update([PORTFOLIO_EXECUTION_VERSION_V2, input.payload.receipt_count]);
    let mut receipt_index = 0usize;
    while receipt_index < PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2 {
        if receipt_index < receipt_count {
            let current = input.receipts[receipt_index]
                .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
            current
                .receipt
                .validate()
                .map_err(|_| PortfolioArchiveRetirementErrorV2::Layout)?;
            let account = LayoutHash32::new(current.account.bytes())
                .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
            let evidence = current
                .receipt
                .evidence(account)
                .map_err(|_| PortfolioArchiveRetirementErrorV2::Layout)?;
            let semantic = current.receipt.semantic();
            let slice_offset = receipt_index
                .checked_mul(SETTLEMENT_SLICE_BYTES)
                .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            let slice_end = slice_offset
                .checked_add(SETTLEMENT_SLICE_BYTES)
                .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            let slice = SettlementSliceV1::decode(
                tail.slices_le()
                    .get(slice_offset..slice_end)
                    .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?,
                feed.order_count,
                feed.outcome_count,
            )
            .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
            let canonical_slice = u16::try_from(receipt_index)
                .map_err(|_| PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            let canonical_sequence = u64::from(canonical_slice)
                .checked_add(1)
                .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            let transition = match current.receipt.transition() {
                SettlementReceiptTransitionCommitmentV5::PortfolioPairCommitted(value) => {
                    Id32::new(value.bytes())
                        .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?
                }
                _ => return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch),
            };
            let expected_flags = RECEIPT_FLAG_BUY_CONSUMED
                | RECEIPT_FLAG_SELL_CONSUMED
                | RECEIPT_FLAG_SLICE_EXHAUSTED;
            if current.account.is_zero()
                || fixed_accounts.contains(&current.account)
                || semantic.slice_index != canonical_slice
                || semantic.sequence != canonical_sequence
                || semantic.epoch.bytes() != root.epoch().bytes()
                || semantic.market.bytes() != root.market().bytes()
                || semantic.candidate.bytes() != root.settlement_candidate_id().bytes()
                || semantic.leg_kind != RECEIPT_LEG_DIRECT
                || semantic.accounted_end_mask != semantic.expected_end_mask()
                || semantic.delivered_end_mask() != semantic.expected_end_mask()
                || semantic.consumed_flags != expected_flags
                || semantic.settled_quantity != semantic.quantity
                || slice.buy_kind != SettlementSliceLegKindV1::Order
                || slice.sell_kind != SettlementSliceLegKindV1::Order
                || slice.outcome != semantic.outcome
                || slice.quantity != semantic.quantity
                || semantic.price != read_u64_at(tail.prices_le(), usize::from(slice.outcome))?
                || prior_outcome.is_some_and(|prior| semantic.outcome <= prior)
            {
                return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
            }
            prior_outcome = Some(semantic.outcome);
            let payoff_at = usize::from(semantic.outcome);
            if payoff[payoff_at] != 0 {
                return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
            }
            payoff[payoff_at] = semantic.quantity;
            consideration_price_units = consideration_price_units
                .checked_add(semantic.consideration_price_units)
                .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            if receipt_index == 0 {
                if input.payload.entry_receipt != current.account {
                    return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
                }
                common_commitment = transition;
                entry_buy_order = Id32::new(semantic.buy_order_id.bytes())
                    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
                entry_sell_order = Id32::new(semantic.sell_order_id.bytes())
                    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
                entry_delivery_transition = Id32::new(evidence.delivery_transition_id().bytes())
                    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
                entry_buy_index = slice.buy_index;
                entry_sell_index = slice.sell_index;
            } else if transition != common_commitment
                || semantic.buy_order_id.bytes() != entry_buy_order.bytes()
                || semantic.sell_order_id.bytes() != entry_sell_order.bytes()
                || slice.buy_index != entry_buy_index
                || slice.sell_index != entry_sell_index
            {
                return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
            }
            let rent = current.receipt.rent();
            let pending_pre_data_id = pending_receipt_pre_data_id(current)?;
            pending_set_hash.update(current.account.bytes());
            pending_set_hash.update(pending_pre_data_id.bytes());
            pending_set_hash.update(semantic.slice_index.to_le_bytes());
            pending_set_hash.update(semantic.sequence.to_le_bytes());
            pending_set_hash.update([semantic.outcome]);
            pending_set_hash.update(semantic.quantity.to_le_bytes());
            pending_set_hash.update(semantic.price.to_le_bytes());
            pending_set_hash.update([
                semantic.accounted_end_mask,
                0,
                semantic.expected_end_mask(),
                PORTFOLIO_PAIR_RECEIPT_TRANSITION_KIND_V2_BYTE,
            ]);
            pending_set_hash.update([0u8; 32]);
            pending_set_hash.update(rent.payer.bytes());
            pending_set_hash.update(rent.refundable_principal.to_le_bytes());
            pending_set_hash.update(rent.donation_floor.to_le_bytes());
            receipt_closes[receipt_index] = Some(close_plan(
                current.account,
                Id32::new(evidence.receipt_data_id().bytes())
                    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?,
                current.balance_lamports,
                rent,
            )?);
            let mut earlier = 0usize;
            while earlier < receipt_index {
                let prior = receipt_closes[earlier]
                    .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
                if prior.account == current.account || prior.pre_data_id == receipt_closes[receipt_index]
                    .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?
                    .pre_data_id
                    || pending_receipt_pre_data_id(
                        input.receipts[earlier]
                            .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?,
                    )? == pending_pre_data_id
                {
                    return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
                }
                earlier += 1;
            }
        } else if input.receipts[receipt_index].is_some() {
            return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
        }
        receipt_index += 1;
    }
    let action42_receipt_set_digest = nonzero_digest(pending_set_hash)?;

    let buyer_close = validate_reservation(
        *input.buyer_reservation,
        root,
        *input.buyer_position,
        entry_buy_order,
        0,
    )?;
    let seller_close = validate_reservation(
        *input.seller_reservation,
        root,
        *input.seller_position,
        entry_sell_order,
        1,
    )?;
    let reservation_closes = [buyer_close, seller_close];
    if buyer_close.account == seller_close.account
        || buyer_close.pre_data_id == seller_close.pre_data_id
        || input.buyer_position.account == input.seller_position.account
        || input.buyer_replay.replay_account() == input.seller_replay.replay_account()
    {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }
    let mut close_index = 0usize;
    while close_index < receipt_count {
        let receipt_close = receipt_closes[close_index]
            .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
        if reservation_closes
            .iter()
            .any(|reservation| reservation.account == receipt_close.account)
        {
            return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
        }
        close_index += 1;
    }
    let price_scale = u128::from(input.market_binding.base().price_scale);
    if consideration_price_units % price_scale != 0 {
        return Err(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch);
    }
    let consideration_atoms = u64::try_from(consideration_price_units / price_scale)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    let buyer_reservation_body = input.buyer_reservation.reservation.body();
    let seller_reservation_body = input.seller_reservation.reservation.body();
    if consideration_atoms == 0
        || consideration_atoms != root
            .cash_pot_expectation()
            .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?
            .consideration_debit_atoms
        || buyer_reservation_body.initial_cash_atoms < consideration_atoms
        || buyer_reservation_body.initial_internal.iter().any(|amount| *amount != 0)
        || seller_reservation_body.initial_cash_atoms != 0
        || seller_reservation_body.initial_internal != payoff
        || buyer_reservation_body.entitled_units != seller_reservation_body.entitled_units
    {
        return Err(PortfolioArchiveRetirementErrorV2::ReservationMismatch);
    }
    let buyer_prior_position_id = action42_prior_position_id(
        *input.buyer_position,
        true,
        consideration_atoms,
        buyer_reservation_body.initial_cash_atoms,
        payoff,
        backend,
    )?;
    let seller_prior_position_id = action42_prior_position_id(
        *input.seller_position,
        false,
        consideration_atoms,
        0,
        payoff,
        backend,
    )?;
    require_immediate_portfolio_replay(
        *input.buyer_replay,
        *input.buyer_position,
        GeneralReplayTransitionKindV1::PortfolioPairBuyer,
        entry_delivery_transition,
        buyer_prior_position_id,
        action42_receipt_set_digest,
        backend,
    )?;
    require_immediate_portfolio_replay(
        *input.seller_replay,
        *input.seller_position,
        GeneralReplayTransitionKindV1::PortfolioPairSeller,
        entry_delivery_transition,
        seller_prior_position_id,
        action42_receipt_set_digest,
        backend,
    )?;

    let semantic_id_partition = [
        root_pre_data_id,
        common_commitment,
        reservation_closes[0].pre_data_id,
        reservation_closes[1].pre_data_id,
        Id32::new(input.buyer_position.semantic_id)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?,
        Id32::new(input.seller_position.semantic_id)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?,
        input.buyer_replay.replay_semantic_id(),
        input.seller_replay.replay_semantic_id(),
    ];
    if !distinct_nonzero(&semantic_id_partition) {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }
    close_index = 0;
    while close_index < receipt_count {
        let receipt_close = receipt_closes[close_index]
            .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
        if semantic_id_partition.contains(&receipt_close.pre_data_id) {
            return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
        }
        close_index += 1;
    }
    let settlement_root_poststate = root
        .retire_portfolio_pair_archives(input.payload.receipt_count)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    let root_post_data_id = settlement_root_poststate
        .data_id(backend, input.settlement_root_account)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Contract)?;
    let buyer_position_poststate = input
        .buyer_position
        .release_reservation_poststate(0, [0; MAX_OUTCOMES])
        .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    let seller_position_poststate = input
        .seller_position
        .release_reservation_poststate(0, [0; MAX_OUTCOMES])
        .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    let buyer_position_post_id = Id32::new(
        buyer_position_poststate
            .semantic
            .semantic_id(backend)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?
            .bytes(),
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    let seller_position_post_id = Id32::new(
        seller_position_poststate
            .semantic
            .semantic_id(backend)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?
            .bytes(),
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    if buyer_position_post_id == seller_position_post_id
        || semantic_id_partition.contains(&buyer_position_post_id)
        || semantic_id_partition.contains(&seller_position_post_id)
    {
        return Err(PortfolioArchiveRetirementErrorV2::BindingMismatch);
    }

    let (refund_transfers, neutral_sink_credit_lamports) = build_refund_vector(
        &input,
        &receipt_closes,
        &reservation_closes,
    )?;
    let neutral_sink_balance_after = input
        .neutral_sink_balance_lamports
        .checked_add(neutral_sink_credit_lamports)
        .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    let transition_id = transition_id(
        &input,
        root_pre_data_id,
        feed_bundle,
        common_commitment,
        action42_receipt_set_digest,
        &receipt_closes,
        &reservation_closes,
    )?;
    let transition_evidence_id = transition_evidence_id(
        &input,
        transition_id,
        root_pre_data_id,
        root_post_data_id,
        buyer_position_post_id,
        seller_position_post_id,
        &refund_transfers,
        neutral_sink_credit_lamports,
        neutral_sink_balance_after,
    )?;
    let buyer_replay_poststate = project_general_replay_transition_v1(
        *input.buyer_replay,
        buyer_position_poststate,
        GeneralReplayTransitionKindV1::RetirePortfolioPairBuyerArchive,
        transition_id,
        transition_evidence_id,
        backend,
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReplayMismatch)?;
    let seller_replay_poststate = project_general_replay_transition_v1(
        *input.seller_replay,
        seller_position_poststate,
        GeneralReplayTransitionKindV1::RetirePortfolioPairSellerArchive,
        transition_id,
        transition_evidence_id,
        backend,
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReplayMismatch)?;
    let terminal_receipt_id = terminal_receipt_id(
        &input,
        transition_id,
        transition_evidence_id,
        root_pre_data_id,
        root_post_data_id,
        common_commitment,
        action42_receipt_set_digest,
        &receipt_closes,
        &reservation_closes,
        buyer_position_post_id,
        seller_position_post_id,
        &buyer_replay_poststate,
        &seller_replay_poststate,
        &refund_transfers,
        neutral_sink_credit_lamports,
        neutral_sink_balance_after,
    )?;
    Ok(RetirePortfolioPairArchivesPlanV2 {
        settlement_root_poststate,
        receipt_closes,
        reservation_closes,
        buyer_position_poststate,
        seller_position_poststate,
        buyer_replay_poststate,
        seller_replay_poststate,
        refund_transfers,
        neutral_sink_balance_after,
        terminal_receipt: PortfolioPairArchiveTerminalReceiptV2 {
            terminal_receipt_id,
            transition_id,
            transition_evidence_id,
            settlement_root_account: input.settlement_root_account,
            settlement_root_pre_data_id: root_pre_data_id,
            settlement_root_post_data_id: root_post_data_id,
            transition_commitment: common_commitment,
            action42_receipt_set_digest,
            receipt_count: input.payload.receipt_count,
            refund_owner_count: input.payload.refund_owner_count,
            neutral_sink: input.neutral_sink_account,
            neutral_sink_credit_lamports,
        },
    })
}

fn validate_reservation(
    input: PortfolioArchiveReservationInputV2,
    root: &SettlementRootV1AccountV1,
    position: AuthenticatedPositionV3,
    expected_order: Id32,
    expected_side: u8,
) -> Result<PortfolioArchiveClosePlanV2, PortfolioArchiveRetirementErrorV2> {
    input
        .reservation
        .validate()
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Layout)?;
    position
        .validate_writable()
        .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    let body = input.reservation.body();
    let fields = position.semantic.fields();
    if input.account.is_zero()
        || body.market.bytes() != root.market().bytes()
        || body.epoch.bytes() != root.epoch().bytes()
        || body.order_id.bytes() != expected_order.bytes()
        || body.owner.bytes() != fields.owner.bytes()
        || body.position_generation != fields.generation
        || body.outcome_count != root.outcome_count()
        || body.side != expected_side
        || body.order_kind != ORDER_KIND_PORTFOLIO
        || body.state != RESERVATION_STATE_CONSUMED
        || body.entitled_units == 0
        || body.entitled_units != body.consumed_units
        || body.entitled_units != body.paid_units
        || !body.remaining_is_zero()
        || body.max_fee_atoms != 0
        || body.fee_debited_atoms != 0
        || body.fee_carry_numerator != 0
        || body.release_generation != 0
        || fields.outstanding_reservations == 0
    {
        return Err(PortfolioArchiveRetirementErrorV2::ReservationMismatch);
    }
    close_plan(
        input.account,
        Id32::new(
            input
                .reservation
                .data_id()
                .map_err(|_| PortfolioArchiveRetirementErrorV2::Layout)?
                .bytes(),
        )
        .map_err(|_| PortfolioArchiveRetirementErrorV2::ReservationMismatch)?,
        input.balance_lamports,
        input.reservation.rent(),
    )
}

fn pending_receipt_pre_data_id(
    input: PortfolioArchiveReceiptInputV2,
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let mut semantic = input.receipt.semantic();
    semantic.settled_quantity = 0;
    semantic.consumed_flags = 0;
    let pending = SettlementReceiptAccountV5::new(
        semantic,
        SettlementReceiptTransitionCommitmentV5::PortfolioPairPending,
        input.receipt.rent(),
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
    let account = LayoutHash32::new(input.account.bytes())
        .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
    Id32::new(
        pending
            .data_id(account)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?
            .bytes(),
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)
}

fn action42_prior_position_id<B: PositionV3Sha256Backend>(
    current: AuthenticatedPositionV3,
    buyer: bool,
    consideration_atoms: u64,
    buyer_reserved_release: u64,
    payoff: [u64; MAX_OUTCOMES],
    backend: &B,
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let current_fields = current.semantic.fields();
    let mut prior_fields: PositionV3Fields = current_fields;
    if buyer {
        prior_fields.cash_atoms = current_fields
            .cash_atoms
            .checked_add(consideration_atoms)
            .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
        prior_fields.reserved_cash_atoms = current_fields
            .reserved_cash_atoms
            .checked_add(buyer_reserved_release)
            .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            prior_fields.native_eggs[outcome] = current_fields.native_eggs[outcome]
                .checked_sub(payoff[outcome])
                .ok_or(PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
            outcome += 1;
        }
    } else {
        prior_fields.cash_atoms = current_fields
            .cash_atoms
            .checked_sub(consideration_atoms)
            .ok_or(PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    }
    let prior = PositionAccountV3::new(prior_fields)
        .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    Id32::new(
        prior
            .semantic_id(backend)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?
            .bytes(),
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)
}

fn require_immediate_portfolio_replay<B: ReplayV3HashBackend>(
    replay: GeneralPositionReplayPrestateV1,
    position: AuthenticatedPositionV3,
    expected_kind: GeneralReplayTransitionKindV1,
    expected_transition_id: Id32,
    prior_position_semantic_id: Id32,
    expected_transition_evidence_id: Id32,
    backend: &B,
) -> Result<(), PortfolioArchiveRetirementErrorV2> {
    let extension = replay.extension();
    if replay.position() != position
        || replay.replay_account().bytes() != position.semantic.fields().replay_account.bytes()
        || replay.next_sequence() < 2
        || extension.last_kind() != Some(expected_kind)
        || extension.last_transition_id() != expected_transition_id
        || extension.current_position_semantic_id().bytes() != position.semantic_id
    {
        return Err(PortfolioArchiveRetirementErrorV2::ReplayMismatch);
    }
    verify_general_replay_last_transition_v1(
        replay,
        prior_position_semantic_id,
        expected_kind,
        expected_transition_id,
        expected_transition_evidence_id,
        backend,
    )
    .map_err(|_| PortfolioArchiveRetirementErrorV2::ReplayMismatch)
}

fn close_plan(
    account: Id32,
    pre_data_id: Id32,
    balance_before: u64,
    rent: DeletableRentOwnerV1,
) -> Result<PortfolioArchiveClosePlanV2, PortfolioArchiveRetirementErrorV2> {
    rent.validate()
        .map_err(|_| PortfolioArchiveRetirementErrorV2::Layout)?;
    let payer = Id32::new(rent.payer.bytes())
        .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?;
    let minimum = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    if account.is_zero()
        || pre_data_id.is_zero()
        || payer == account
        || balance_before < minimum
    {
        return Err(PortfolioArchiveRetirementErrorV2::InsufficientCloseBalance);
    }
    let surplus = balance_before
        .checked_sub(rent.refundable_principal)
        .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    Ok(PortfolioArchiveClosePlanV2 {
        account,
        pre_data_id,
        balance_before,
        payer,
        principal_refund_lamports: rent.refundable_principal,
        surplus_sink_lamports: surplus,
    })
}

fn build_refund_vector(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    receipt_closes: &[Option<PortfolioArchiveClosePlanV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    reservation_closes: &[PortfolioArchiveClosePlanV2;
        PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2],
) -> Result<([PortfolioArchiveRefundTransferV2;
    PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2], u64), PortfolioArchiveRetirementErrorV2> {
    let count = usize::from(input.payload.refund_owner_count);
    let mut transfers = [PortfolioArchiveRefundTransferV2::EMPTY;
        PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2];
    let mut index = 0usize;
    while index < PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2 {
        let owner = input.refund_owners[index];
        if index < count {
            if owner.account.is_zero()
                || owner.account == input.neutral_sink_account
                || (index != 0 && owner.account.bytes() <= input.refund_owners[index - 1].account.bytes())
                || protected_account(input, receipt_closes, owner.account)
            {
                return Err(PortfolioArchiveRetirementErrorV2::RefundOwnerMismatch);
            }
            transfers[index] = PortfolioArchiveRefundTransferV2 {
                owner: owner.account,
                balance_before: owner.balance_lamports,
                principal_refund_lamports: 0,
                balance_after: 0,
            };
        } else if owner != PortfolioArchiveRefundOwnerInputV2::EMPTY {
            return Err(PortfolioArchiveRetirementErrorV2::RefundOwnerMismatch);
        }
        index += 1;
    }
    let mut sink_credit = 0u64;
    let mut receipt_index = 0usize;
    while receipt_index < usize::from(input.payload.receipt_count) {
        let close = receipt_closes[receipt_index]
            .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
        add_close_transfer(&mut transfers, count, close)?;
        sink_credit = sink_credit
            .checked_add(close.surplus_sink_lamports)
            .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
        receipt_index += 1;
    }
    for close in reservation_closes {
        add_close_transfer(&mut transfers, count, *close)?;
        sink_credit = sink_credit
            .checked_add(close.surplus_sink_lamports)
            .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    }
    index = 0;
    while index < count {
        if transfers[index].principal_refund_lamports == 0 {
            return Err(PortfolioArchiveRetirementErrorV2::RefundOwnerMismatch);
        }
        transfers[index].balance_after = transfers[index]
            .balance_before
            .checked_add(transfers[index].principal_refund_lamports)
            .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
        index += 1;
    }
    Ok((transfers, sink_credit))
}

fn add_close_transfer(
    transfers: &mut [PortfolioArchiveRefundTransferV2;
        PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    count: usize,
    close: PortfolioArchiveClosePlanV2,
) -> Result<(), PortfolioArchiveRetirementErrorV2> {
    let mut index = 0usize;
    while index < count {
        if transfers[index].owner == close.payer {
            transfers[index].principal_refund_lamports = transfers[index]
                .principal_refund_lamports
                .checked_add(close.principal_refund_lamports)
                .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
            return Ok(());
        }
        index += 1;
    }
    Err(PortfolioArchiveRetirementErrorV2::RefundOwnerMismatch)
}

fn protected_account(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    receipt_closes: &[Option<PortfolioArchiveClosePlanV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    account: Id32,
) -> bool {
    if [
        input.settlement_root_account,
        input.retained_feed_account,
        input.market_binding_account,
        input.neutral_sink_account,
        input.buyer_reservation.account,
        input.seller_reservation.account,
        Id32::from_bytes(input.buyer_position.account),
        Id32::from_bytes(input.seller_position.account),
        input.buyer_replay.replay_account(),
        input.seller_replay.replay_account(),
    ]
    .contains(&account)
    {
        return true;
    }
    let mut index = 0usize;
    while index < usize::from(input.payload.receipt_count) {
        if receipt_closes[index].map(|close| close.account) == Some(account) {
            return true;
        }
        index += 1;
    }
    false
}

fn transition_id(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    root_pre_data_id: Id32,
    feed_bundle: Id32,
    commitment: Id32,
    action42_receipt_set_digest: Id32,
    receipt_closes: &[Option<PortfolioArchiveClosePlanV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    reservation_closes: &[PortfolioArchiveClosePlanV2;
        PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2],
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let mut hash = Sha256::new();
    hash.update(PORTFOLIO_ARCHIVE_TRANSITION_DOMAIN_V2);
    hash.update([input.payload.receipt_count, input.payload.refund_owner_count]);
    for id in [
        input.settlement_root_account,
        root_pre_data_id,
        input.retained_feed_account,
        feed_bundle,
        input.market_binding_account,
        input.neutral_sink_account,
        commitment,
        action42_receipt_set_digest,
    ] {
        hash.update(id.bytes());
    }
    let mut index = 0usize;
    while index < usize::from(input.payload.receipt_count) {
        let close = receipt_closes[index]
            .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
        hash_close(&mut hash, close);
        index += 1;
    }
    for close in reservation_closes {
        hash_close(&mut hash, *close);
    }
    hash_endpoint_pre(&mut hash, *input.buyer_position, *input.buyer_replay);
    hash_endpoint_pre(&mut hash, *input.seller_position, *input.seller_replay);
    nonzero_digest(hash)
}

#[allow(clippy::too_many_arguments)]
fn transition_evidence_id(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    transition_id: Id32,
    root_pre_data_id: Id32,
    root_post_data_id: Id32,
    buyer_position_post_id: Id32,
    seller_position_post_id: Id32,
    refunds: &[PortfolioArchiveRefundTransferV2; PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    sink_credit: u64,
    sink_after: u64,
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let mut hash = Sha256::new();
    hash.update(PORTFOLIO_ARCHIVE_EVIDENCE_DOMAIN_V2);
    for id in [
        transition_id,
        input.settlement_root_account,
        root_pre_data_id,
        root_post_data_id,
        Id32::from_bytes(input.buyer_position.account),
        Id32::new(input.buyer_position.semantic_id)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?,
        buyer_position_post_id,
        Id32::from_bytes(input.seller_position.account),
        Id32::new(input.seller_position.semantic_id)
            .map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)?,
        seller_position_post_id,
    ] {
        hash.update(id.bytes());
    }
    hash.update(input.buyer_replay.replay_account().bytes());
    hash.update(input.buyer_replay.replay_semantic_id().bytes());
    hash.update(input.buyer_replay.next_sequence().to_le_bytes());
    hash.update(input.seller_replay.replay_account().bytes());
    hash.update(input.seller_replay.replay_semantic_id().bytes());
    hash.update(input.seller_replay.next_sequence().to_le_bytes());
    hash_refunds(&mut hash, input.payload.refund_owner_count, refunds);
    hash.update(input.neutral_sink_account.bytes());
    hash.update(input.neutral_sink_balance_lamports.to_le_bytes());
    hash.update(sink_credit.to_le_bytes());
    hash.update(sink_after.to_le_bytes());
    nonzero_digest(hash)
}

#[allow(clippy::too_many_arguments)]
fn terminal_receipt_id(
    input: &RetirePortfolioPairArchivesInputV2<'_>,
    transition_id: Id32,
    evidence_id: Id32,
    root_pre_data_id: Id32,
    root_post_data_id: Id32,
    commitment: Id32,
    action42_receipt_set_digest: Id32,
    receipt_closes: &[Option<PortfolioArchiveClosePlanV2>;
        PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    reservation_closes: &[PortfolioArchiveClosePlanV2;
        PORTFOLIO_ARCHIVE_RESERVATION_COUNT_V2],
    buyer_position_post_id: Id32,
    seller_position_post_id: Id32,
    buyer_replay_post: &GeneralReplayTransitionPlanV1,
    seller_replay_post: &GeneralReplayTransitionPlanV1,
    refunds: &[PortfolioArchiveRefundTransferV2; PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    sink_credit: u64,
    sink_after: u64,
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let mut hash = Sha256::new();
    hash.update(PORTFOLIO_ARCHIVE_TERMINAL_DOMAIN_V2);
    hash.update([input.payload.receipt_count, input.payload.refund_owner_count]);
    for id in [
        transition_id,
        evidence_id,
        input.settlement_root_account,
        root_pre_data_id,
        root_post_data_id,
        commitment,
        action42_receipt_set_digest,
    ] {
        hash.update(id.bytes());
    }
    let mut index = 0usize;
    while index < usize::from(input.payload.receipt_count) {
        hash_close(
            &mut hash,
            receipt_closes[index]
                .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?,
        );
        index += 1;
    }
    for close in reservation_closes {
        hash_close(&mut hash, *close);
    }
    hash_position_transition(&mut hash, *input.buyer_position, buyer_position_post_id);
    hash_position_transition(&mut hash, *input.seller_position, seller_position_post_id);
    hash_replay_transition(&mut hash, *input.buyer_replay, buyer_replay_post);
    hash_replay_transition(&mut hash, *input.seller_replay, seller_replay_post);
    hash_refunds(&mut hash, input.payload.refund_owner_count, refunds);
    hash.update(input.neutral_sink_account.bytes());
    hash.update(input.neutral_sink_balance_lamports.to_le_bytes());
    hash.update(sink_credit.to_le_bytes());
    hash.update(sink_after.to_le_bytes());
    nonzero_digest(hash)
}

fn hash_close(hash: &mut Sha256, close: PortfolioArchiveClosePlanV2) {
    hash.update(close.account.bytes());
    hash.update(close.pre_data_id.bytes());
    hash.update(close.balance_before.to_le_bytes());
    hash.update(close.payer.bytes());
    hash.update(close.principal_refund_lamports.to_le_bytes());
    hash.update(close.surplus_sink_lamports.to_le_bytes());
}

fn hash_endpoint_pre(
    hash: &mut Sha256,
    position: AuthenticatedPositionV3,
    replay: GeneralPositionReplayPrestateV1,
) {
    hash.update(position.account);
    hash.update(position.semantic_id);
    hash.update(position.semantic.fields().generation.to_le_bytes());
    hash.update(replay.replay_account().bytes());
    hash.update(replay.replay_semantic_id().bytes());
    hash.update(replay.next_sequence().to_le_bytes());
    hash.update(replay.extension().last_transition_id().bytes());
    hash.update(replay.extension().last_delta_id().bytes());
}

fn hash_position_transition(
    hash: &mut Sha256,
    position: AuthenticatedPositionV3,
    post_id: Id32,
) {
    hash.update(position.account);
    hash.update(position.semantic_id);
    hash.update(post_id.bytes());
}

fn hash_replay_transition(
    hash: &mut Sha256,
    pre: GeneralPositionReplayPrestateV1,
    post: &GeneralReplayTransitionPlanV1,
) {
    hash.update(pre.replay_account().bytes());
    hash.update(pre.replay_semantic_id().bytes());
    // These are the immediate action-42 persisted authorities. Include both
    // explicitly in the terminal transcript rather than relying only on the
    // transition-ID hash's transitive binding.
    hash.update(pre.extension().last_transition_id().bytes());
    hash.update(pre.extension().last_delta_id().bytes());
    hash.update(post.replay_poststate_semantic_id().bytes());
    hash.update(pre.next_sequence().to_le_bytes());
    hash.update(post.next_sequence().to_le_bytes());
}

fn hash_refunds(
    hash: &mut Sha256,
    count: u8,
    refunds: &[PortfolioArchiveRefundTransferV2; PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
) {
    hash.update([count]);
    let mut index = 0usize;
    while index < usize::from(count) {
        hash.update(refunds[index].owner.bytes());
        hash.update(refunds[index].balance_before.to_le_bytes());
        hash.update(refunds[index].principal_refund_lamports.to_le_bytes());
        hash.update(refunds[index].balance_after.to_le_bytes());
        index += 1;
    }
}

fn nonzero_digest(
    hash: Sha256,
) -> Result<Id32, PortfolioArchiveRetirementErrorV2> {
    let bytes: [u8; 32] = hash.finalize().into();
    Id32::new(bytes).map_err(|_| PortfolioArchiveRetirementErrorV2::BindingMismatch)
}

fn read_u64_at(
    bytes: &[u8],
    index: usize,
) -> Result<u64, PortfolioArchiveRetirementErrorV2> {
    let offset = index
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    let end = offset
        .checked_add(core::mem::size_of::<u64>())
        .ok_or(PortfolioArchiveRetirementErrorV2::ArithmeticOverflow)?;
    let encoded: [u8; 8] = bytes
        .get(offset..end)
        .ok_or(PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?
        .try_into()
        .map_err(|_| PortfolioArchiveRetirementErrorV2::ReceiptSetMismatch)?;
    Ok(u64::from_le_bytes(encoded))
}

fn distinct_nonzero(values: &[Id32]) -> bool {
    let mut left = 0usize;
    while left < values.len() {
        if values[left].is_zero() {
            return false;
        }
        let mut right = left + 1;
        while right < values.len() {
            if values[left] == values[right] {
                return false;
            }
            right += 1;
        }
        left += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 { Id32::from_bytes([byte; 32]) }

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer: LayoutHash32::from_bytes(id(2).bytes()),
            refundable_principal: 1_000,
            donation_floor: 10,
        }
    }

    #[test]
    fn close_refunds_only_principal_and_sinks_every_other_lamport() {
        let close = close_plan(id(1), id(3), 1_017, rent()).unwrap();
        assert_eq!(close.payer(), id(2));
        assert_eq!(close.principal_refund_lamports(), 1_000);
        assert_eq!(close.surplus_sink_lamports(), 17);
        assert_eq!(
            close_plan(id(1), id(3), 1_009, rent()),
            Err(PortfolioArchiveRetirementErrorV2::InsufficientCloseBalance)
        );
    }

    #[test]
    fn refund_vector_aggregates_repeated_payer_without_extra_owner() {
        let mut transfers = [PortfolioArchiveRefundTransferV2::EMPTY;
            PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2];
        transfers[0] = PortfolioArchiveRefundTransferV2 {
            owner: id(2),
            balance_before: 5,
            principal_refund_lamports: 0,
            balance_after: 0,
        };
        let close = close_plan(id(1), id(3), 1_017, rent()).unwrap();
        add_close_transfer(&mut transfers, 1, close).unwrap();
        add_close_transfer(&mut transfers, 1, close).unwrap();
        assert_eq!(transfers[0].principal_refund_lamports(), 2_000);
        assert_eq!(
            add_close_transfer(&mut transfers, 0, close),
            Err(PortfolioArchiveRetirementErrorV2::RefundOwnerMismatch)
        );
    }

    #[test]
    fn account_partition_refuses_zero_and_aliases() {
        assert!(distinct_nonzero(&[id(1), id(2), id(3)]));
        assert!(!distinct_nonzero(&[id(1), id(1)]));
        assert!(!distinct_nonzero(&[id(1), Id32::ZERO]));
    }
}
