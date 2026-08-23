// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resumable account-local RelationV2 order verification.
//!
//! The first step authenticates the complete frozen V5 page set once. Later
//! steps re-open only the canonical current page while the program-owned Work
//! phase retains the successful set-authentication fact. Every call skips a
//! bounded run of tombstones and folds exactly one dense live order, so the
//! present-funded order schedule never relies on an unpaid page-only call.
//! The SBF mint path must authenticate the sealed feed's exact finite
//! atom-mixture certificate before creating Work. This module then rechecks
//! the successor Relation policy and immutable feed/domain bindings on every
//! resumed step; it does not expose price admission or execution authority.

use clutch_batch::relation_v1::MAX_OUTCOMES as RELATION_MAX_OUTCOMES;
use clutch_batch::relation_v2::{EconomicCandidateV2, PricePreconditionV2, VerifiedEconomicsV2};
use clutch_batch::relation_v2_stream::{
    advance_economic_relation_order_v2, begin_economic_relation_stream_v2,
    finalize_economic_relation_stream_v2, EconomicRelationStreamErrorV2,
};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, decode_clear_work_v3_relation_flows, economic_domain_digest_v2,
    replace_clear_work_v3_order_state, ClearWorkRelationFlowsV1, ClearWorkV3AccountV1,
    ClearWorkVerificationStateV1, CodecError, EconomicDomainV2AccountV1, Id32, MarketBindingV1,
    SettlementCandidateKindV1, Sha256CheckpointV1, MAX_OUTCOMES,
};
use clutch_solana_layout::order_page_v5::{
    verify_page_set_v5_streaming, verify_page_v5, OrderSlotCursorV5,
};
use clutch_solana_layout::{CodecError as LayoutError, MAX_ORDER_PAGES};

use crate::builder::{project_owner_blind_slot, relation_domain_from_account};
use crate::{
    decode_sealed_candidate_feed_v1, quantized_relation_v2_policy_id_v2,
    score_v2_q_policy_id_v1, CandidateBuilderErrorV1, CanonicalSha256, GeneralV2RuntimeError,
};

const _: () = assert!(MAX_OUTCOMES == RELATION_MAX_OUTCOMES);

/// Protocol/authentication faults distinct from checked candidate refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralV2WorkErrorV1 {
    /// General-owned account bytes or mutation framing refused.
    Contract(CodecError),
    /// A frozen page envelope or page-set fold refused.
    Layout(LayoutError),
    /// A typed domain/order projection refused before candidate economics.
    Builder(CandidateBuilderErrorV1),
    /// A RelationV2 cursor/checkpoint fault, never a candidate verdict.
    RelationProtocol(EconomicRelationStreamErrorV2),
    /// Authenticated semantic identities disagreed.
    BindingMismatch,
    /// Checked reward/cursor arithmetic overflowed.
    ArithmeticOverflow,
}

impl From<CodecError> for GeneralV2WorkErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Contract(value)
    }
}

impl From<LayoutError> for GeneralV2WorkErrorV1 {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<CandidateBuilderErrorV1> for GeneralV2WorkErrorV1 {
    fn from(value: CandidateBuilderErrorV1) -> Self {
        Self::Builder(value)
    }
}

/// Private exact poststate of one action-12 order attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceClearOrderPlanV1 {
    pre: ClearWorkV3AccountV1,
    post: ClearWorkV3AccountV1,
    flows: ClearWorkRelationFlowsV1,
    filled_legs: [u64; MAX_OUTCOMES],
    accepted_order: bool,
    keeper_reward: u64,
    economics: Option<VerifiedEconomicsV2>,
}

impl AdvanceClearOrderPlanV1 {
    /// Exact V3 Work header after this attempt.
    pub const fn work(&self) -> &ClearWorkV3AccountV1 {
        &self.post
    }

    /// Exact present-funded order reward authorized by this attempt.
    pub const fn keeper_reward(&self) -> u64 {
        self.keeper_reward
    }

    /// Whether this call proved the candidate economically invalid.
    pub const fn refused(&self) -> bool {
        matches!(
            self.post.verification_state,
            ClearWorkVerificationStateV1::Refused
        )
    }

    /// Final checked RelationV2 economics, present only after the last order
    /// accepted and the candidate identity matched.
    pub const fn economics(&self) -> Option<&VerifiedEconomicsV2> {
        self.economics.as_ref()
    }

    /// Apply the private plan to the exact writable Work account bytes.
    pub fn write_account(self, account: &mut [u8]) -> Result<(), GeneralV2WorkErrorV1> {
        replace_clear_work_v3_order_state(
            account,
            self.pre,
            self.post,
            self.flows.aggregate_buy_flow,
            self.flows.aggregate_sell_flow,
            self.filled_legs,
            self.accepted_order,
        )?;
        Ok(())
    }
}

/// Authenticate a frozen V5 page set and fold exactly the next dense live
/// order into the account-local RelationV2 checkpoint.
#[allow(clippy::too_many_arguments)]
pub fn advance_clear_order_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    work_account: &[u8],
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    page_bodies: &[&[u8]],
) -> Result<AdvanceClearOrderPlanV1, GeneralV2WorkErrorV1> {
    let work = ClearWorkV3AccountV1::decode_account(work_account)?;
    let prior_flows = decode_clear_work_v3_relation_flows(work_account)?;
    economic_domain_account.validate()?;
    market_binding.validate()?;
    let (feed_header, feed) = decode_sealed_candidate_feed_v1(sealed_candidate_feed)
        .map_err(map_runtime_projection_error)?;
    let domain = relation_domain_from_account(economic_domain_account)?;
    let domain_digest =
        economic_domain_digest_v2(&CanonicalSha256, economic_domain_account.transcript)?;
    let candidate_bundle =
        candidate_bundle_digest_v1(&CanonicalSha256, sealed_candidate_feed, true)?;
    if page_bodies.is_empty()
        || page_bodies.len() > MAX_ORDER_PAGES
        || work.phase > 1
        || work.verification_state != ClearWorkVerificationStateV1::Pending
        || work.order_cursor >= work.order_count
        || work.order_count == 0
        || candidate_feed_identity != work.feed
        || feed_header.epoch != work.epoch
        || feed_header.node != work.node
        || feed_header.market != work.market
        || feed_header.order_set != work.order_set
        || feed_header.order_count != work.order_count
        || feed_header.outcome_count != work.outcome_count
        || feed_header.candidate_kind != SettlementCandidateKindV1::Direct
        || feed_header.settlement_candidate_id != work.settlement_candidate_id
        || feed_header.base_relation_candidate_id != work.base_relation_candidate_id
        || feed_header.candidate_price_digest != work.candidate_price_digest
        || feed_header.economic_domain_digest != work.economic_domain_digest
        || feed_header.price_measure_policy_v1_id != work.price_measure_policy_v1_id
        || feed_header.relation_policy_id != work.relation_policy_id
        || feed_header.price_body_digest != work.price_body_digest
        || candidate_bundle != work.candidate_bundle_digest
        || domain_digest != work.economic_domain_digest
        || economic_domain_account.epoch != work.epoch
        || market_binding.binding.is_zero()
        || market_binding.market != work.market
        || market_binding.relation_policy_id != work.relation_policy_id
        || market_binding.price_measure_policy_v1_id != work.price_measure_policy_v1_id
        || market_binding.native_claim_basis_id != work.native_claim_basis_id
        || market_binding.score_policy_id != work.score_policy_id
        || market_binding.outcome_count != work.outcome_count
        || market_binding.price_scale != feed_header.price_scale
        || market_binding.relation_policy_id
            != quantized_relation_v2_policy_id_v2().map_err(map_runtime_projection_error)?
        || market_binding.score_policy_id
            != score_v2_q_policy_id_v1().map_err(map_runtime_projection_error)?
        || (work.phase != 0 && page_bodies.len() != usize::from(work.page_count))
    {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }

    let page_count =
        u16::try_from(page_bodies.len()).map_err(|_| GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    if work.phase == 0 {
        let observed = verify_page_set_v5_streaming(page_bodies)?;
        let mut live_orders = 0u16;
        let mut page = 0usize;
        while page < page_bodies.len() {
            let header = verify_page_v5(page_bodies[page])?;
            if header.market.bytes() != work.market.bytes()
                || header.epoch.bytes() != work.epoch.bytes()
                || header.order_set.bytes() != work.order_set.bytes()
                || usize::from(header.page_index) != page
                || header.page_count != page_count
            {
                return Err(GeneralV2WorkErrorV1::BindingMismatch);
            }
            live_orders = live_orders
                .checked_add(u16::from(header.live_count()))
                .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
            page += 1;
        }
        if observed.bytes() != work.order_set.bytes() || live_orders != u16::from(work.order_count)
        {
            return Err(GeneralV2WorkErrorV1::BindingMismatch);
        }
    } else if work.page_count != page_count {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }

    let price = PricePreconditionV2 {
        policy_digest: economic_domain_account
            .transcript
            .price_measure_policy_v1_id
            .bytes(),
        semantic_price_digest: work.candidate_price_digest.bytes(),
        prices: feed.prices,
    };
    let candidate = EconomicCandidateV2 {
        fills: feed.fills,
        honored_aon_mask: feed_header.honored_aon_mask,
        virtual_split: feed_header.virtual_split,
        virtual_merge: feed_header.virtual_merge,
    };
    let mut sha256 = if work.phase == 0 {
        match begin_economic_relation_stream_v2(&domain, &price, &candidate, work.order_count) {
            Ok(value) => value,
            Err(EconomicRelationStreamErrorV2::Economic(_)) => {
                return refused_plan(
                    work,
                    prior_flows,
                    [0; MAX_OUTCOMES],
                    false,
                    market_binding.order_reward,
                    page_count,
                )
            }
            Err(error) => return Err(GeneralV2WorkErrorV1::RelationProtocol(error)),
        }
    } else {
        work.sha256.relation_v2()?
    };

    let mut page_index = if work.phase == 0 { 0 } else { work.page_cursor };
    let mut slot_index = if work.phase == 0 { 0 } else { work.slot_cursor };
    let (order, next_page, next_slot) = loop {
        let body = page_bodies
            .get(usize::from(page_index))
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?;
        let header = verify_page_v5(body)?;
        if header.market.bytes() != work.market.bytes()
            || header.epoch.bytes() != work.epoch.bytes()
            || header.order_set.bytes() != work.order_set.bytes()
            || header.page_index != page_index
            || header.page_count != page_count
            || slot_index > header.order_count
        {
            return Err(GeneralV2WorkErrorV1::BindingMismatch);
        }
        let mut cursor = OrderSlotCursorV5::new(body)?;
        let mut skipped = 0u8;
        while skipped < slot_index {
            cursor
                .next_slot()
                .ok_or(GeneralV2WorkErrorV1::BindingMismatch)??;
            skipped += 1;
        }
        let mut found = None;
        while slot_index < header.order_count {
            let verified = cursor
                .next_slot()
                .ok_or(GeneralV2WorkErrorV1::BindingMismatch)??;
            slot_index = slot_index
                .checked_add(1)
                .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
            if let Some((economic_order, _membership)) =
                project_owner_blind_slot(verified.slot, &domain)?
            {
                found = Some(economic_order);
                break;
            }
        }
        if let Some(economic_order) = found {
            let (next_page, next_slot) = if slot_index == header.order_count {
                (
                    page_index
                        .checked_add(1)
                        .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?,
                    0,
                )
            } else {
                (page_index, slot_index)
            };
            break (economic_order, next_page, next_slot);
        }
        page_index = page_index
            .checked_add(1)
            .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
        slot_index = 0;
        if page_index >= page_count {
            return Err(GeneralV2WorkErrorV1::BindingMismatch);
        }
    };

    let step = match advance_economic_relation_order_v2(
        &domain,
        &price,
        &candidate,
        work.order_count,
        work.order_cursor,
        work.previous_order_id.bytes(),
        sha256,
        prior_flows.aggregate_buy_flow,
        prior_flows.aggregate_sell_flow,
        &order,
    ) {
        Ok(value) => value,
        Err(EconomicRelationStreamErrorV2::Economic(_)) => {
            return refused_plan(
                work,
                prior_flows,
                [0; MAX_OUTCOMES],
                false,
                market_binding.order_reward,
                page_count,
            )
        }
        Err(error) => return Err(GeneralV2WorkErrorV1::RelationProtocol(error)),
    };
    sha256 = step.sha256;
    let flows = ClearWorkRelationFlowsV1 {
        aggregate_buy_flow: step.aggregate_buy_flow,
        aggregate_sell_flow: step.aggregate_sell_flow,
    };
    let reward_remaining = work
        .reward_remaining
        .checked_sub(market_binding.order_reward)
        .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?;
    let reward_earned = work
        .reward_earned
        .checked_add(market_binding.order_reward)
        .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    let complete = step.next_order_index == work.order_count;
    let mut post = ClearWorkV3AccountV1 {
        previous_order_id: Id32::new(step.previous_order_id)?,
        reward_remaining,
        reward_earned,
        page_count,
        page_cursor: if complete { page_count } else { next_page },
        slot_cursor: if complete { 0 } else { next_slot },
        order_cursor: step.next_order_index,
        phase: 1,
        sha256: Sha256CheckpointV1::from_relation_v2(sha256)?,
        ..work
    };
    let mut economics = None;
    if complete {
        match finalize_economic_relation_stream_v2(
            &domain,
            &price,
            &candidate,
            work.order_count,
            step.next_order_index,
            step.previous_order_id,
            sha256,
            step.aggregate_buy_flow,
            step.aggregate_sell_flow,
        ) {
            Ok(value)
                if value.economic_candidate_digest == work.base_relation_candidate_id.bytes()
                    && value.economic_candidate_digest == work.settlement_candidate_id.bytes() =>
            {
                post.phase = if post.slice_count == 0 { 3 } else { 2 };
                post.verification_state = ClearWorkVerificationStateV1::Valid;
                economics = Some(value);
            }
            Ok(_) | Err(EconomicRelationStreamErrorV2::Economic(_)) => {
                post.phase = 3;
                post.verification_state = ClearWorkVerificationStateV1::Refused;
            }
            Err(error) => return Err(GeneralV2WorkErrorV1::RelationProtocol(error)),
        }
    }
    post.validate()?;
    Ok(AdvanceClearOrderPlanV1 {
        pre: work,
        post,
        flows,
        filled_legs: step.filled_legs,
        accepted_order: true,
        keeper_reward: market_binding.order_reward,
        economics,
    })
}

fn refused_plan(
    work: ClearWorkV3AccountV1,
    flows: ClearWorkRelationFlowsV1,
    filled_legs: [u64; MAX_OUTCOMES],
    accepted_order: bool,
    order_reward: u64,
    page_count: u16,
) -> Result<AdvanceClearOrderPlanV1, GeneralV2WorkErrorV1> {
    let post = ClearWorkV3AccountV1 {
        reward_remaining: work
            .reward_remaining
            .checked_sub(order_reward)
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?,
        reward_earned: work
            .reward_earned
            .checked_add(order_reward)
            .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?,
        page_count,
        phase: 3,
        verification_state: ClearWorkVerificationStateV1::Refused,
        ..work
    };
    post.validate()?;
    Ok(AdvanceClearOrderPlanV1 {
        pre: work,
        post,
        flows,
        filled_legs,
        accepted_order,
        keeper_reward: order_reward,
        economics: None,
    })
}

fn map_runtime_projection_error(error: GeneralV2RuntimeError) -> GeneralV2WorkErrorV1 {
    match error {
        GeneralV2RuntimeError::Contract(error) => GeneralV2WorkErrorV1::Contract(error),
        GeneralV2RuntimeError::PriceGrid(error) => GeneralV2WorkErrorV1::Layout(error),
        _ => GeneralV2WorkErrorV1::BindingMismatch,
    }
}
