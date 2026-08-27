//! Resumable, one-order execution of the owner-blind RelationV2 core.
//!
//! The bounded reference verifier owns the semantics. This module reuses its
//! exact order-shape, fill, canonical-transcript, conservation, and ScoreV2-Q
//! helpers while keeping the per-call working set to one order and two active
//! flow vectors. The caller persists those vectors and the returned SHA-256
//! continuation in program-owned work state; no `EconomicBookV2` is rebuilt.
//!
//! Page/account authentication is intentionally outside this crate. A live
//! adapter must prove that `order_index`, `previous_order_id`, and `order` are
//! the next dense live record of one immutable frozen page set. Substituting a
//! caller-selected record is a protocol fault, not an economic refusal.

use crate::relation_v1::MAX_OUTCOMES;
use crate::relation_v2::{
    begin_economic_candidate_hash_v2, close_economic_candidate_v2,
    finish_economic_candidate_hash_v2, hash_economic_order_v2, validate_candidate_padding_v2,
    validate_live_order_fill_v2, validate_live_order_shape_v2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, EconomicSha256CheckpointV2,
    PricePreconditionV2, Sha256V2, VerifiedEconomicsV2,
};
use crate::{Side, MAX_ORDERS};

/// Protocol faults in a resumable RelationV2 order walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EconomicRelationStreamErrorV2 {
    /// The adapter supplied an impossible cursor, width, or predecessor tuple.
    InvalidCursor,
    /// An inactive flow or leg cell was nonzero.
    NonCanonicalPadding,
    /// The canonical RelationV2 owner refused the economic input.
    Economic(EconomicErrorV2),
}

impl From<EconomicErrorV2> for EconomicRelationStreamErrorV2 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Economic(value)
    }
}

/// Exact poststate of one accepted order step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicRelationOrderStepV2 {
    /// Next dense live-order index.
    pub next_order_index: u8,
    /// Current order identity, required as the next step's predecessor.
    pub previous_order_id: [u8; 32],
    /// Canonical candidate-identity continuation after this order.
    pub sha256: EconomicSha256CheckpointV2,
    /// Aggregate filled buy legs after this order.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// Aggregate filled sell legs after this order.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
    /// This order's filled coefficient vector for the settlement-slice pass.
    pub filled_legs: [u64; MAX_OUTCOMES],
}

/// Start the canonical RelationV2 candidate transcript.
///
/// The returned checkpoint has consumed the domain and active book length,
/// exactly as the bounded verifier's candidate identity does. The price and
/// candidate must come from immutable authenticated feed state on every later
/// call; their identities are not reordered into this prefix.
pub fn begin_economic_relation_stream_v2(
    domain: &EconomicDomainV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    book_len: u8,
) -> Result<EconomicSha256CheckpointV2, EconomicRelationStreamErrorV2> {
    domain.validate()?;
    price.validate(domain)?;
    if usize::from(book_len) > MAX_ORDERS {
        return Err(EconomicRelationStreamErrorV2::InvalidCursor);
    }
    if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
        return Err(EconomicErrorV2::NonCanonicalVirtualConversion.into());
    }
    validate_candidate_padding_v2(candidate, book_len)?;
    begin_economic_candidate_hash_v2(domain, book_len)?
        .checkpoint()
        .map_err(Into::into)
}

/// Validate and fold exactly the next dense live order.
#[allow(clippy::too_many_arguments)]
pub fn advance_economic_relation_order_v2(
    domain: &EconomicDomainV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    book_len: u8,
    order_index: u8,
    previous_order_id: [u8; 32],
    sha256: EconomicSha256CheckpointV2,
    mut aggregate_buy_flow: [u64; MAX_OUTCOMES],
    mut aggregate_sell_flow: [u64; MAX_OUTCOMES],
    order: &EconomicOrderV2,
) -> Result<EconomicRelationOrderStepV2, EconomicRelationStreamErrorV2> {
    domain.validate()?;
    price.validate(domain)?;
    validate_candidate_padding_v2(candidate, book_len)?;
    let at = usize::from(order_index);
    if usize::from(book_len) > MAX_ORDERS
        || at >= usize::from(book_len)
        || (order_index == 0 && previous_order_id != [0; 32])
        || (order_index != 0 && previous_order_id == [0; 32])
    {
        return Err(EconomicRelationStreamErrorV2::InvalidCursor);
    }
    require_flow_padding(domain, &aggregate_buy_flow, &aggregate_sell_flow)?;
    validate_live_order_shape_v2(domain, order, order_index, previous_order_id)?;
    let filled_legs = validate_live_order_fill_v2(domain, price, candidate, order, order_index)?;

    let mut outcome = 0usize;
    while outcome < usize::from(domain.outcome_count) {
        let target = match order.side {
            Side::Buy => &mut aggregate_buy_flow[outcome],
            Side::Sell => &mut aggregate_sell_flow[outcome],
        };
        *target =
            target
                .checked_add(filled_legs[outcome])
                .ok_or(EconomicErrorV2::FlowOverflow {
                    order: order_index,
                    outcome: u8::try_from(outcome)
                        .map_err(|_| EconomicErrorV2::ArithmeticOverflow)?,
                })?;
        outcome += 1;
    }

    let mut hash = Sha256V2::from_checkpoint(sha256)?;
    hash_economic_order_v2(&mut hash, order)?;
    let next_order_index = order_index
        .checked_add(1)
        .ok_or(EconomicRelationStreamErrorV2::InvalidCursor)?;
    Ok(EconomicRelationOrderStepV2 {
        next_order_index,
        previous_order_id: order.order_id,
        sha256: hash.checkpoint()?,
        aggregate_buy_flow,
        aggregate_sell_flow,
        filled_legs,
    })
}

/// Close a complete order walk under the exact bounded RelationV2 digest,
/// conservation equation, and ScoreV2-Q implementation.
#[allow(clippy::too_many_arguments)]
pub fn finalize_economic_relation_stream_v2(
    domain: &EconomicDomainV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    book_len: u8,
    order_index: u8,
    previous_order_id: [u8; 32],
    sha256: EconomicSha256CheckpointV2,
    aggregate_buy_flow: [u64; MAX_OUTCOMES],
    aggregate_sell_flow: [u64; MAX_OUTCOMES],
) -> Result<VerifiedEconomicsV2, EconomicRelationStreamErrorV2> {
    domain.validate()?;
    price.validate(domain)?;
    validate_candidate_padding_v2(candidate, book_len)?;
    if order_index != book_len
        || usize::from(book_len) > MAX_ORDERS
        || (book_len == 0 && previous_order_id != [0; 32])
        || (book_len != 0 && previous_order_id == [0; 32])
    {
        return Err(EconomicRelationStreamErrorV2::InvalidCursor);
    }
    require_flow_padding(domain, &aggregate_buy_flow, &aggregate_sell_flow)?;
    let hash = Sha256V2::from_checkpoint(sha256)?;
    let digest = finish_economic_candidate_hash_v2(hash, book_len, price, candidate)?;
    close_economic_candidate_v2(
        domain,
        candidate,
        aggregate_buy_flow,
        aggregate_sell_flow,
        digest,
    )
    .map_err(Into::into)
}

fn require_flow_padding(
    domain: &EconomicDomainV2,
    buy: &[u64; MAX_OUTCOMES],
    sell: &[u64; MAX_OUTCOMES],
) -> Result<(), EconomicRelationStreamErrorV2> {
    let mut outcome = usize::from(domain.outcome_count);
    while outcome < MAX_OUTCOMES {
        if buy[outcome] != 0 || sell[outcome] != 0 {
            return Err(EconomicRelationStreamErrorV2::NonCanonicalPadding);
        }
        outcome += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation_v2::{
        price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2,
        ECONOMIC_RELATION_VERSION_V2,
    };
    use crate::PartialPolicy;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn fixture() -> (
        EconomicDomainV2,
        EconomicBookV2,
        PricePreconditionV2,
        EconomicCandidateV2,
    ) {
        let domain = EconomicDomainV2 {
            relation_version: ECONOMIC_RELATION_VERSION_V2,
            market_semantics_digest: id(1),
            epoch_semantics_digest: id(2),
            relation_policy_digest: id(3),
            price_policy_digest: id(4),
            epoch_index: 7,
            outcome_count: 2,
            price_scale: 10_000,
        };
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 5_000;
        prices[1] = 5_000;
        let price = PricePreconditionV2 {
            policy_digest: id(4),
            semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
            prices,
        };
        let mut buy_coefficients = [0u64; MAX_OUTCOMES];
        buy_coefficients[0] = 1;
        let buy = EconomicOrderV2 {
            order_id: id(11),
            side: Side::Buy,
            coefficients: buy_coefficients,
            quantity: 7,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_value_price_units_per_unit: 10_000,
        };
        let sell = EconomicOrderV2 {
            order_id: id(12),
            side: Side::Sell,
            limit_value_price_units_per_unit: 0,
            ..buy
        };
        let mut book = EconomicBookV2::empty();
        book.orders[0] = buy;
        book.orders[1] = sell;
        book.len = 2;
        let mut candidate = EconomicCandidateV2::EMPTY;
        candidate.fills[0] = 7;
        candidate.fills[1] = 7;
        (domain, book, price, candidate)
    }

    #[test]
    fn resumable_order_walk_equals_the_bounded_relation() {
        let (domain, book, price, candidate) = fixture();
        let expected = verify_economic_candidate_v2(&domain, &book, &price, &candidate).unwrap();
        let mut sha =
            begin_economic_relation_stream_v2(&domain, &price, &candidate, book.len).unwrap();
        let mut buy = [0u64; MAX_OUTCOMES];
        let mut sell = [0u64; MAX_OUTCOMES];
        let mut previous = [0u8; 32];
        let mut cursor = 0u8;
        while cursor < book.len {
            let step = advance_economic_relation_order_v2(
                &domain,
                &price,
                &candidate,
                book.len,
                cursor,
                previous,
                sha,
                buy,
                sell,
                &book.orders[usize::from(cursor)],
            )
            .unwrap();
            cursor = step.next_order_index;
            previous = step.previous_order_id;
            sha = step.sha256;
            buy = step.aggregate_buy_flow;
            sell = step.aggregate_sell_flow;
        }
        let observed = finalize_economic_relation_stream_v2(
            &domain, &price, &candidate, book.len, cursor, previous, sha, buy, sell,
        )
        .unwrap();
        assert_eq!(observed, expected);
    }

    #[test]
    fn checkpoint_padding_and_predecessor_substitution_refuse() {
        let (domain, book, price, candidate) = fixture();
        let mut sha =
            begin_economic_relation_stream_v2(&domain, &price, &candidate, book.len).unwrap();
        sha.block[63] = 1;
        assert_eq!(
            advance_economic_relation_order_v2(
                &domain,
                &price,
                &candidate,
                book.len,
                0,
                [0; 32],
                sha,
                [0; MAX_OUTCOMES],
                [0; MAX_OUTCOMES],
                &book.orders[0],
            ),
            Err(EconomicRelationStreamErrorV2::Economic(
                EconomicErrorV2::InvalidHashCheckpoint
            ))
        );

        let sha = begin_economic_relation_stream_v2(&domain, &price, &candidate, book.len).unwrap();
        assert_eq!(
            advance_economic_relation_order_v2(
                &domain,
                &price,
                &candidate,
                book.len,
                1,
                id(99),
                sha,
                [0; MAX_OUTCOMES],
                [0; MAX_OUTCOMES],
                &book.orders[1],
            ),
            Err(EconomicRelationStreamErrorV2::Economic(
                EconomicErrorV2::NonCanonicalOrderOrder { order: 1 }
            ))
        );
    }
}
