// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resumable account-local RelationV2 order verification.
//!
//! The first step authenticates the complete frozen V5 page set once. Later
//! steps re-open only the canonical current page while the program-owned Work
//! phase retains the successful set-authentication fact. Every call skips a
//! bounded run of tombstones and folds exactly one dense live order, so the
//! present-funded order schedule never relies on an unpaid page-only call.
//! The SBF path must authenticate the sealed feed's exact finite atom-mixture
//! certificate before creating Work. ClearWork V3 does not retain enough
//! versioned Product/Grid authority for a successor policy bit to stand in for
//! that proof. Every successor resume therefore remints the full private
//! Product/Grid capability and joins it to Work before entering the raw
//! streaming transition. This module exposes no execution authority.

use clutch_batch::relation_v1::MAX_OUTCOMES as RELATION_MAX_OUTCOMES;
use clutch_batch::relation_v2::{EconomicCandidateV2, PricePreconditionV2, VerifiedEconomicsV2};
use clutch_batch::relation_v2_stream::{
    advance_economic_relation_order_v2, begin_economic_relation_stream_v2,
    finalize_economic_relation_stream_v2, EconomicRelationStreamErrorV2,
};
use clutch_batch::score_v2::{
    score_candidate_v2, CandidateDeltaV2, NormalizationPolicyV2, ScoreErrorV2,
};
use clutch_batch::Side;
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, clear_work_v3_slice_remainders_complete,
    decode_clear_work_v3_filled_legs,
    decode_clear_work_v3_relation_flows, economic_domain_digest_v2,
    replace_clear_work_v3_order_state, replace_clear_work_v3_slice_state,
    ClearWorkRelationFlowsV1, ClearWorkSliceDebitV1, ClearWorkSliceDebitsV1,
    ClearWorkV3AccountV1, ClearWorkVerificationStateV1, CodecError,
    EconomicDomainV2AccountV1, Id32, MarketBindingV1, ScoreV2QComponentsV1,
    SettlementCandidateKindV1, SettlementSliceLegKindV1, SettlementSliceV1,
    Sha256CheckpointV1, SETTLEMENT_SLICE_BYTES, MAX_OUTCOMES,
};
use clutch_solana_layout::order_page_v5::{
    verify_page_set_v5_streaming, verify_page_v5, OrderSlotCursorV5,
};
use clutch_solana_layout::{CodecError as LayoutError, MAX_ORDER_PAGES};

use crate::builder::{project_owner_blind_slot, relation_domain_from_account};
use crate::{
    decode_sealed_candidate_feed_v1, quantized_relation_v2_policy_id_v2,
    score_v2_q_policy_id_v1, CandidateBuilderErrorV1, CanonicalSha256, GeneralV2RuntimeError,
    QuantizedRelationProductPriceAdmissionV2,
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
    /// A terminal ScoreV2-Q derivation disagreed with completed Work state.
    Score(ScoreErrorV2),
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

impl From<ScoreErrorV2> for GeneralV2WorkErrorV1 {
    fn from(value: ScoreErrorV2) -> Self {
        Self::Score(value)
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

/// Reauthenticate exact Product/Grid admission and fold the next live order.
///
/// WorkV3 predates the quantized Relation policy and therefore cannot assert
/// from its version alone that the complete Product/Grid tuple was checked at
/// creation. This successor requires a freshly minted full-tuple capability on
/// every resume, joins it to every immutable identity retained by Work, and
/// only then enters the existing streaming transition.
#[allow(clippy::too_many_arguments)]
pub fn advance_quantized_clear_order_v2(
    product_admission: QuantizedRelationProductPriceAdmissionV2,
    candidate_feed_identity: Id32,
    market_binding_identity: Id32,
    sealed_candidate_feed: &[u8],
    work_account: &[u8],
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    page_bodies: &[&[u8]],
) -> Result<AdvanceClearOrderPlanV1, GeneralV2WorkErrorV1> {
    let work = ClearWorkV3AccountV1::decode_account(work_account)?;
    verify_quantized_clear_work_authority_v2(
        product_admission,
        candidate_feed_identity,
        market_binding_identity,
        &work,
        market_binding,
    )?;
    advance_clear_order_v1(
        candidate_feed_identity,
        sealed_candidate_feed,
        work_account,
        economic_domain_account,
        market_binding,
        page_bodies,
    )
}

/// Join a freshly checked Product/Grid capability to every immutable exact-
/// price identity retained by a ClearWork V3 account.
///
/// Settlement-slice and terminal successors use the same gate before they
/// consume Work. Returning success proves only identity/coherence admission;
/// it is not execution authority, a price-quality judgment, or fair value.
pub fn verify_quantized_clear_work_authority_v2(
    product_admission: QuantizedRelationProductPriceAdmissionV2,
    candidate_feed_identity: Id32,
    market_binding_identity: Id32,
    work: &ClearWorkV3AccountV1,
    market_binding: &MarketBindingV1,
) -> Result<(), GeneralV2WorkErrorV1> {
    let admission = product_admission.price_admission();
    let certificate = admission.certificate();
    let certificate_bindings = certificate.bindings();
    if candidate_feed_identity != admission.candidate_feed()
        || candidate_feed_identity != work.feed
        || admission.economic_domain_digest() != work.economic_domain_digest
        || admission.price_body_digest() != work.price_body_digest
        || admission.price().semantic_price_digest != work.candidate_price_digest.bytes()
        || admission.price().policy_digest != work.price_measure_policy_v1_id.bytes()
        || admission.domain().relation_policy_digest != work.relation_policy_id.bytes()
        || admission.domain().market_semantics_digest
            != product_admission.market_instance_v2_id().bytes()
        || admission.domain().outcome_count != work.outcome_count
        || admission.domain().price_scale != market_binding.price_scale
        || product_admission.market_binding() != market_binding_identity
        || product_admission.market_genesis_profile_v2_id()
            != market_binding.market_genesis_profile_v2_id
        || product_admission.market_instance_v2_id() != market_binding.market_instance_v2_id
        || certificate_bindings.market_id != work.market.bytes()
        || certificate_bindings.terms_id
            != product_admission.market_genesis_profile_v2_id().bytes()
        || certificate_bindings.basis_id != work.native_claim_basis_id.bytes()
        || certificate_bindings.price_id != work.candidate_price_digest.bytes()
        || certificate.outcome_count() != work.outcome_count
        || certificate.payout_denominator() != market_binding.price_scale
    {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }
    Ok(())
}

/// Authenticate a frozen V5 page set and fold exactly the next dense live
/// order into the account-local RelationV2 checkpoint.
///
/// This V1 entry point is retained for legacy callers. The successor SBF path
/// uses [`advance_quantized_clear_order_v2`] so a policy ID alone cannot stand
/// in for the exact retained price authority.
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

/// Private exact poststate of one action-13 settlement-slice attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceClearSlicePlanV1 {
    pre: ClearWorkV3AccountV1,
    post: ClearWorkV3AccountV1,
    debits: ClearWorkSliceDebitsV1,
    accepted_slice: bool,
    keeper_reward: u64,
}

impl AdvanceClearSlicePlanV1 {
    /// Exact V3 Work header after this checked attempt.
    pub const fn work(&self) -> &ClearWorkV3AccountV1 {
        &self.post
    }

    /// Exact present-funded slice reward authorized by monotone progress.
    pub const fn keeper_reward(&self) -> u64 {
        self.keeper_reward
    }

    /// Whether the candidate became checked-refused on this slice.
    pub const fn refused(&self) -> bool {
        !self.accepted_slice
    }

    /// Apply the private plan to the exact writable Work account bytes.
    pub fn write_account(self, account: &mut [u8]) -> Result<(), GeneralV2WorkErrorV1> {
        replace_clear_work_v3_slice_state(
            account,
            self.pre,
            self.post,
            self.debits,
            self.accepted_slice,
            self.keeper_reward,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SliceOrderFactV1 {
    side: Side,
    owner: Id32,
}

/// Reauthenticate exact Product/Grid admission and consume one settlement
/// slice from a previously accepted RelationV2 candidate.
///
/// Phase two is still a resumed candidate-verification phase: route and
/// remaining-leg checks can change the verdict to checked-refused. It must
/// therefore consume the same call-local exact-price authority as order
/// streaming rather than inheriting authority from a V3 policy field.
#[allow(clippy::too_many_arguments)]
pub fn advance_quantized_clear_slice_v2(
    product_admission: QuantizedRelationProductPriceAdmissionV2,
    candidate_feed_identity: Id32,
    market_binding_identity: Id32,
    sealed_candidate_feed: &[u8],
    work_account: &[u8],
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    page_bodies: &[&[u8]],
) -> Result<AdvanceClearSlicePlanV1, GeneralV2WorkErrorV1> {
    let work = ClearWorkV3AccountV1::decode_account(work_account)?;
    verify_quantized_clear_work_authority_v2(
        product_admission,
        candidate_feed_identity,
        market_binding_identity,
        &work,
        market_binding,
    )?;
    advance_clear_slice_v1(
        candidate_feed_identity,
        sealed_candidate_feed,
        work_account,
        economic_domain_account,
        market_binding,
        page_bodies,
    )
}

/// Authenticate the sealed feed and complete V5 page set, then consume
/// exactly one canonical settlement slice.
///
/// RelationV2's accepted filled-leg matrix becomes a remaining-leg matrix in
/// phase two.  Direct slices debit both real legs; split and merge slices debit
/// exactly one.  Wrong side, same-owner direct pairing, wrong virtual route,
/// over-consumption, and terminal under-consumption are checked candidate
/// refusals.  Account/PDA binding and codec faults remain protocol errors.
/// This raw V1 entry point is retained for legacy callers; successor SBF uses
/// [`advance_quantized_clear_slice_v2`].
#[allow(clippy::too_many_arguments)]
pub fn advance_clear_slice_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    work_account: &[u8],
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    page_bodies: &[&[u8]],
) -> Result<AdvanceClearSlicePlanV1, GeneralV2WorkErrorV1> {
    let work = ClearWorkV3AccountV1::decode_account(work_account)?;
    economic_domain_account.validate()?;
    market_binding.validate()?;
    let (feed, tail) = clutch_general_v2_contract::complete_candidate_feed_v2(
        sealed_candidate_feed,
        true,
    )?;
    let domain = relation_domain_from_account(economic_domain_account)?;
    let domain_digest =
        economic_domain_digest_v2(&CanonicalSha256, economic_domain_account.transcript)?;
    let candidate_bundle =
        candidate_bundle_digest_v1(&CanonicalSha256, sealed_candidate_feed, true)?;
    let page_count = u16::try_from(page_bodies.len())
        .map_err(|_| GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    if page_bodies.is_empty()
        || page_bodies.len() > MAX_ORDER_PAGES
        || work.phase != 2
        || work.verification_state != ClearWorkVerificationStateV1::Valid
        || work.order_cursor != work.order_count
        || work.slice_cursor >= work.slice_count
        || work.page_cursor != work.page_count
        || work.slot_cursor != 0
        || work.page_count != page_count
        || candidate_feed_identity != work.feed
        || feed.epoch != work.epoch
        || feed.node != work.node
        || feed.market != work.market
        || feed.order_set != work.order_set
        || feed.order_count != work.order_count
        || feed.outcome_count != work.outcome_count
        || feed.slice_count != work.slice_count
        || feed.candidate_kind != SettlementCandidateKindV1::Direct
        || feed.settlement_candidate_id != work.settlement_candidate_id
        || feed.base_relation_candidate_id != work.base_relation_candidate_id
        || feed.candidate_price_digest != work.candidate_price_digest
        || feed.economic_domain_digest != work.economic_domain_digest
        || feed.price_measure_policy_v1_id != work.price_measure_policy_v1_id
        || feed.relation_policy_id != work.relation_policy_id
        || feed.price_body_digest != work.price_body_digest
        || candidate_bundle != work.candidate_bundle_digest
        || domain_digest != work.economic_domain_digest
        || economic_domain_account.epoch != work.epoch
        || market_binding.market != work.market
        || market_binding.relation_policy_id != work.relation_policy_id
        || market_binding.price_measure_policy_v1_id != work.price_measure_policy_v1_id
        || market_binding.native_claim_basis_id != work.native_claim_basis_id
        || market_binding.score_policy_id != work.score_policy_id
        || market_binding.outcome_count != work.outcome_count
        || market_binding.price_scale != feed.price_scale
        || market_binding.relation_policy_id
            != quantized_relation_v2_policy_id_v2().map_err(map_runtime_projection_error)?
        || market_binding.score_policy_id
            != score_v2_q_policy_id_v1().map_err(map_runtime_projection_error)?
    {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }
    let observed = verify_page_set_v5_streaming(page_bodies)?;
    if observed.bytes() != work.order_set.bytes() {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }
    let mut live_count = 0u8;
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
        live_count = live_count
            .checked_add(header.live_count())
            .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
        page += 1;
    }
    if live_count != work.order_count {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }

    let slice_at = usize::from(work.slice_cursor)
        .checked_mul(SETTLEMENT_SLICE_BYTES)
        .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    let slice_end = slice_at
        .checked_add(SETTLEMENT_SLICE_BYTES)
        .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    let slice = SettlementSliceV1::decode(
        tail.slices_le()
            .get(slice_at..slice_end)
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?,
        work.order_count,
        work.outcome_count,
    )?;
    let buy_fact = if slice.buy_kind == SettlementSliceLegKindV1::Order {
        Some(project_slice_order_fact(
            page_bodies,
            &domain,
            slice.buy_index,
        )?)
    } else {
        None
    };
    let sell_fact = if slice.sell_kind == SettlementSliceLegKindV1::Order {
        Some(project_slice_order_fact(
            page_bodies,
            &domain,
            slice.sell_index,
        )?)
    } else {
        None
    };
    let debits = ClearWorkSliceDebitsV1 {
        buy: buy_fact.map(|_| ClearWorkSliceDebitV1 {
            order_index: slice.buy_index,
            outcome: slice.outcome,
            quantity: slice.quantity,
        }),
        sell: sell_fact.map(|_| ClearWorkSliceDebitV1 {
            order_index: slice.sell_index,
            outcome: slice.outcome,
            quantity: slice.quantity,
        }),
    };
    let route_valid = match (slice.buy_kind, slice.sell_kind, buy_fact, sell_fact) {
        (
            SettlementSliceLegKindV1::Order,
            SettlementSliceLegKindV1::Order,
            Some(buy),
            Some(sell),
        ) => buy.side == Side::Buy && sell.side == Side::Sell && buy.owner != sell.owner,
        (
            SettlementSliceLegKindV1::Order,
            SettlementSliceLegKindV1::CoveredDealerSell,
            Some(buy),
            None,
        ) => buy.side == Side::Buy && feed.virtual_split != 0 && feed.virtual_merge == 0,
        (
            SettlementSliceLegKindV1::CoveredDealerBuy,
            SettlementSliceLegKindV1::Order,
            None,
            Some(sell),
        ) => sell.side == Side::Sell && feed.virtual_merge != 0 && feed.virtual_split == 0,
        _ => false,
    };
    if !route_valid || !slice_debits_fit(work_account, debits)? {
        return refused_slice_plan(work, market_binding.slice_reward);
    }
    let next_slice = work
        .slice_cursor
        .checked_add(1)
        .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
    if next_slice == work.slice_count && !slice_debits_finish(work_account, work, debits)? {
        return refused_slice_plan(work, market_binding.slice_reward);
    }
    let post = ClearWorkV3AccountV1 {
        reward_remaining: work
            .reward_remaining
            .checked_sub(market_binding.slice_reward)
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?,
        reward_earned: work
            .reward_earned
            .checked_add(market_binding.slice_reward)
            .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?,
        slice_cursor: next_slice,
        phase: if next_slice == work.slice_count { 3 } else { 2 },
        ..work
    };
    post.validate()?;
    Ok(AdvanceClearSlicePlanV1 {
        pre: work,
        post,
        debits,
        accepted_slice: true,
        keeper_reward: market_binding.slice_reward,
    })
}

/// Recompute ScoreV2-Q from terminal Work's immutable aggregate flows.
///
/// This does not replace terminal Product/price-measure verification.  It
/// provides the exact rank components consumed after that independent checker
/// accepts the same candidate.
pub fn score_completed_clear_work_v1(
    sealed_candidate_feed: &[u8],
    work_account: &[u8],
) -> Result<ScoreV2QComponentsV1, GeneralV2WorkErrorV1> {
    clear_work_v3_slice_remainders_complete(work_account)?;
    let work = ClearWorkV3AccountV1::decode_account(work_account)?;
    let (feed, _) =
        clutch_general_v2_contract::complete_candidate_feed_v2(sealed_candidate_feed, true)?;
    let flows = decode_clear_work_v3_relation_flows(work_account)?;
    if feed.epoch != work.epoch
        || feed.node != work.node
        || feed.market != work.market
        || feed.order_set != work.order_set
        || feed.base_relation_candidate_id != work.base_relation_candidate_id
        || feed.settlement_candidate_id != work.settlement_candidate_id
        || feed.outcome_count != work.outcome_count
        || feed.order_count != work.order_count
        || feed.slice_count != work.slice_count
        || candidate_bundle_digest_v1(&CanonicalSha256, sealed_candidate_feed, true)?
            != work.candidate_bundle_digest
        || work.relation_policy_id
            != quantized_relation_v2_policy_id_v2().map_err(map_runtime_projection_error)?
        || work.score_policy_id
            != score_v2_q_policy_id_v1().map_err(map_runtime_projection_error)?
    {
        return Err(GeneralV2WorkErrorV1::BindingMismatch);
    }
    let mut claimed_direct_flow = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(work.outcome_count) {
        claimed_direct_flow[outcome] = flows.aggregate_buy_flow[outcome]
            .checked_sub(feed.virtual_split)
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?;
        outcome += 1;
    }
    let score = score_candidate_v2(&CandidateDeltaV2 {
        normalization_policy: NormalizationPolicyV2::OwnerBlindAggregate,
        outcome_count: work.outcome_count,
        aggregate_buy_flow: flows.aggregate_buy_flow,
        aggregate_sell_flow: flows.aggregate_sell_flow,
        claimed_direct_flow,
        virtual_split: feed.virtual_split,
        virtual_merge: feed.virtual_merge,
        candidate_digest: work.base_relation_candidate_id.bytes(),
    })?;
    Ok(ScoreV2QComponentsV1 {
        certified_risk_flow_atoms: score.risk.certified_risk_flow_atoms,
        cash_equivalent_direct_flow_atoms: score.cash_equivalent_direct_flow_atoms,
        virtual_churn_atoms: score.virtual_churn_atoms,
        settlement_candidate_id: work.settlement_candidate_id,
    })
}

fn project_slice_order_fact(
    page_bodies: &[&[u8]],
    domain: &clutch_batch::relation_v2::EconomicDomainV2,
    wanted: u8,
) -> Result<SliceOrderFactV1, GeneralV2WorkErrorV1> {
    let mut dense = 0u8;
    let mut page = 0usize;
    while page < page_bodies.len() {
        let header = verify_page_v5(page_bodies[page])?;
        let mut cursor = OrderSlotCursorV5::new(page_bodies[page])?;
        let mut slot = 0u8;
        while slot < header.order_count {
            let verified = cursor
                .next_slot()
                .ok_or(GeneralV2WorkErrorV1::BindingMismatch)??;
            if let Some((order, membership)) = project_owner_blind_slot(verified.slot, domain)? {
                if dense == wanted {
                    return Ok(SliceOrderFactV1 {
                        side: order.side,
                        owner: membership.owner(),
                    });
                }
                dense = dense
                    .checked_add(1)
                    .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
            }
            slot = slot
                .checked_add(1)
                .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?;
        }
        page += 1;
    }
    Err(GeneralV2WorkErrorV1::BindingMismatch)
}

fn slice_debits_fit(
    work_account: &[u8],
    debits: ClearWorkSliceDebitsV1,
) -> Result<bool, GeneralV2WorkErrorV1> {
    let mut which = 0u8;
    while which < 2 {
        let debit = if which == 0 { debits.buy } else { debits.sell };
        if let Some(value) = debit {
            let row = decode_clear_work_v3_filled_legs(work_account, value.order_index)?;
            let combined = if debits.buy.map(|item| (item.order_index, item.outcome))
                == debits.sell.map(|item| (item.order_index, item.outcome))
            {
                value
                    .quantity
                    .checked_mul(2)
                    .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?
            } else {
                value.quantity
            };
            if row[usize::from(value.outcome)] < combined {
                return Ok(false);
            }
        }
        which += 1;
    }
    Ok(true)
}

fn slice_debits_finish(
    work_account: &[u8],
    work: ClearWorkV3AccountV1,
    debits: ClearWorkSliceDebitsV1,
) -> Result<bool, GeneralV2WorkErrorV1> {
    let mut order = 0u8;
    while order < work.order_count {
        let row = decode_clear_work_v3_filled_legs(work_account, order)?;
        let mut outcome = 0u8;
        while outcome < work.outcome_count {
            let mut remaining = row[usize::from(outcome)];
            for debit in [debits.buy, debits.sell].into_iter().flatten() {
                if debit.order_index == order && debit.outcome == outcome {
                    remaining = remaining
                        .checked_sub(debit.quantity)
                        .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?;
                }
            }
            if remaining != 0 {
                return Ok(false);
            }
            outcome += 1;
        }
        order += 1;
    }
    Ok(true)
}

fn refused_slice_plan(
    work: ClearWorkV3AccountV1,
    slice_reward: u64,
) -> Result<AdvanceClearSlicePlanV1, GeneralV2WorkErrorV1> {
    let post = ClearWorkV3AccountV1 {
        reward_remaining: work
            .reward_remaining
            .checked_sub(slice_reward)
            .ok_or(GeneralV2WorkErrorV1::BindingMismatch)?,
        reward_earned: work
            .reward_earned
            .checked_add(slice_reward)
            .ok_or(GeneralV2WorkErrorV1::ArithmeticOverflow)?,
        phase: 3,
        verification_state: ClearWorkVerificationStateV1::Refused,
        ..work
    };
    post.validate()?;
    Ok(AdvanceClearSlicePlanV1 {
        pre: work,
        post,
        debits: ClearWorkSliceDebitsV1::NONE,
        accepted_slice: false,
        keeper_reward: slice_reward,
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

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_general_v2_contract::{
        DeletableRentOwnerV1, CLEAR_WORK_V3_HEADER_BYTES, PRICE_MEASURE_WITNESS_SCHEMA_V3,
        QUANTIZED_PRICE_MEASURE_SEMANTICS_V1, SHA256_INITIAL_STATE_V1,
    };

    fn live(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn phase_two_work() -> ClearWorkV3AccountV1 {
        ClearWorkV3AccountV1 {
            epoch: live(1),
            node: live(2),
            market: live(3),
            order_set: live(4),
            feed: live(5),
            candidate_bundle_digest: live(6),
            settlement_candidate_id: live(7),
            base_relation_candidate_id: live(7),
            relation_policy_id: live(8),
            economic_domain_digest: live(9),
            native_claim_basis_id: live(10),
            candidate_price_digest: live(11),
            price_measure_policy_v1_id: live(12),
            score_policy_id: live(13),
            price_body_digest: live(14),
            previous_order_id: live(15),
            epoch_generation: 1,
            rent: DeletableRentOwnerV1 {
                payer: live(16),
                refundable_principal: 100,
                donation_floor: 0,
            },
            reward_remaining: 20,
            reward_earned: 6,
            slice_count: 2,
            slice_cursor: 0,
            page_count: 1,
            page_cursor: 1,
            outcome_count: 2,
            order_count: 2,
            order_cursor: 2,
            slot_cursor: 0,
            phase: 2,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            stored_bump: 1,
            verification_state: ClearWorkVerificationStateV1::Valid,
            flags: 0,
            sha256: Sha256CheckpointV1 {
                state: SHA256_INITIAL_STATE_V1,
                block: [0; 64],
                block_len: 0,
                total_len: 0,
            },
        }
    }

    #[test]
    fn slice_fit_and_terminal_coverage_are_exact() {
        const WORK_LEN: usize = CLEAR_WORK_V3_HEADER_BYTES + 32 + 32;
        let work = phase_two_work();
        let mut bytes = [0u8; WORK_LEN];
        work.encode(&mut bytes[..CLEAR_WORK_V3_HEADER_BYTES])
            .unwrap();
        let matrix_at = CLEAR_WORK_V3_HEADER_BYTES + 32;
        bytes[matrix_at..matrix_at + 8].copy_from_slice(&3u64.to_le_bytes());
        bytes[matrix_at + 16..matrix_at + 24].copy_from_slice(&3u64.to_le_bytes());
        let exact = ClearWorkSliceDebitsV1 {
            buy: Some(ClearWorkSliceDebitV1 {
                order_index: 0,
                outcome: 0,
                quantity: 3,
            }),
            sell: Some(ClearWorkSliceDebitV1 {
                order_index: 1,
                outcome: 0,
                quantity: 3,
            }),
        };
        assert_eq!(slice_debits_fit(&bytes, exact), Ok(true));
        assert_eq!(slice_debits_finish(&bytes, work, exact), Ok(true));

        let partial = ClearWorkSliceDebitsV1 {
            buy: exact.buy.map(|value| ClearWorkSliceDebitV1 {
                quantity: 2,
                ..value
            }),
            sell: exact.sell.map(|value| ClearWorkSliceDebitV1 {
                quantity: 2,
                ..value
            }),
        };
        assert_eq!(slice_debits_fit(&bytes, partial), Ok(true));
        assert_eq!(slice_debits_finish(&bytes, work, partial), Ok(false));

        let over = ClearWorkSliceDebitsV1 {
            buy: exact.buy.map(|value| ClearWorkSliceDebitV1 {
                quantity: 4,
                ..value
            }),
            sell: exact.sell,
        };
        assert_eq!(slice_debits_fit(&bytes, over), Ok(false));
    }
}
