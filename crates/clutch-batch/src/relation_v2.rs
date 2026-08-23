//! Owner-blind, fixed-width economic candidate verification.
//!
//! `EconomicRelationV2` deliberately has no owner, signer, account, fee,
//! dealer, or settlement field. It validates one submitted coefficient-vector
//! fill witness, derives its aggregate flow from both sides, binds an upstream
//! price-policy precondition, and recomputes [`crate::score_v2::ScoreV2`].
//! Changing an external account or controller label cannot change this
//! relation because no such label is representable in its inputs.
//!
//! This first core is not a matching allocator, price-coherence verifier,
//! settlement authorization, fee transition, dealer transition, or SBF
//! profile. In particular, proof or certificate bytes are not relation inputs.
//! This module recomputes a proof-independent semantic price digest, checks its
//! exact policy binding and integer simplex, but does not restate V1's partial
//! moment-cone gate or trust an account projection.

use crate::relation_v1::MAX_OUTCOMES;
use crate::score_v2::{
    score_candidate_v2, CandidateDeltaV2, NormalizationPolicyV2, ScoreErrorV2, ScoreV2,
};
use crate::{PartialPolicy, Side, MAX_ORDERS};

/// Exact semantic version of this owner-blind relation.
pub const ECONOMIC_RELATION_VERSION_V2: u32 = 2;

const ECONOMIC_CANDIDATE_DIGEST_DOMAIN_V2: &[u8] = b"dragons-clutch/economic-candidate/v2\0";
const PRICE_SEMANTICS_DIGEST_DOMAIN_V2: &[u8] = b"dragons-clutch/price-semantics/v2\0";

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);

/// Immutable economic domain shared by one frozen book and all its candidates.
///
/// Every digest is a semantic content identity. None is interpreted as a
/// Solana address or account authority by this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicDomainV2 {
    /// Must equal [`ECONOMIC_RELATION_VERSION_V2`].
    pub relation_version: u32,
    /// Content identity of the immutable market semantics.
    pub market_semantics_digest: [u8; 32],
    /// Content identity of the exact recurring instance or epoch semantics.
    pub epoch_semantics_digest: [u8; 32],
    /// Content identity of the frozen RelationV2 policy.
    pub relation_policy_digest: [u8; 32],
    /// Content identity of the required upstream price-coherence policy.
    pub price_policy_digest: [u8; 32],
    /// Monotone epoch index used only for order expiry.
    pub epoch_index: u64,
    /// Active prefix of every outcome vector, in `2..=16`.
    pub outcome_count: u8,
    /// Exact integer simplex scale.
    pub price_scale: u64,
}

impl EconomicDomainV2 {
    /// Refuse malformed shape and semantic identities.
    pub fn validate(&self) -> Result<(), EconomicErrorV2> {
        if self.relation_version != ECONOMIC_RELATION_VERSION_V2 {
            return Err(EconomicErrorV2::UnknownRelationVersion);
        }
        if is_zero_digest(&self.market_semantics_digest)
            || is_zero_digest(&self.epoch_semantics_digest)
            || is_zero_digest(&self.relation_policy_digest)
            || is_zero_digest(&self.price_policy_digest)
        {
            return Err(EconomicErrorV2::ZeroSemanticDigest);
        }
        if !(2..=MAX_OUTCOMES).contains(&usize::from(self.outcome_count)) {
            return Err(EconomicErrorV2::InvalidOutcomeCount);
        }
        if self.price_scale == 0 {
            return Err(EconomicErrorV2::InvalidPriceScale);
        }
        Ok(())
    }

    fn outcomes(&self) -> usize {
        usize::from(self.outcome_count)
    }
}

/// Upstream price-policy decision consumed as a semantic precondition.
///
/// The adapter must authenticate its proof under `policy_digest` before
/// invoking this pure relation. Proof and certificate representations are not
/// fields here. The relation independently checks the policy identity, active
/// width, exact simplex, canonical padding, and proof-independent semantic
/// price digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PricePreconditionV2 {
    /// Must equal [`EconomicDomainV2::price_policy_digest`].
    pub policy_digest: [u8; 32],
    /// Recomputed content identity of the exact price semantics.
    pub semantic_price_digest: [u8; 32],
    /// Exact state-price vector followed by canonical zero padding.
    pub prices: [u64; MAX_OUTCOMES],
}

impl PricePreconditionV2 {
    /// Check binding, simplex membership, and inactive padding.
    pub fn validate(&self, domain: &EconomicDomainV2) -> Result<(), EconomicErrorV2> {
        domain.validate()?;
        if self.policy_digest != domain.price_policy_digest {
            return Err(EconomicErrorV2::PricePolicyMismatch);
        }
        validate_price_vector(domain, &self.prices)?;
        if self.semantic_price_digest != price_semantics_digest_v2(domain, &self.prices)? {
            return Err(EconomicErrorV2::PriceSemanticDigestMismatch);
        }
        Ok(())
    }
}

fn validate_price_vector(
    domain: &EconomicDomainV2,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<(), EconomicErrorV2> {
    let mut sum = 0u128;
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let price = prices[outcome];
        if price > domain.price_scale {
            return Err(EconomicErrorV2::PriceOutOfRange {
                outcome: bounded_index(outcome)?,
            });
        }
        sum = sum
            .checked_add(u128::from(price))
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        outcome += 1;
    }
    if sum != u128::from(domain.price_scale) {
        return Err(EconomicErrorV2::SimplexSumMismatch);
    }
    while outcome < MAX_OUTCOMES {
        if prices[outcome] != 0 {
            return Err(EconomicErrorV2::NonCanonicalPricePadding {
                outcome: bounded_index(outcome)?,
            });
        }
        outcome += 1;
    }
    Ok(())
}

/// Recompute the proof-independent semantic identity of an exact price vector.
///
/// No proof, certificate, signer, transport, or account bytes enter this
/// digest. Adapters may authenticate nonunique proofs, but must project them to
/// this single canonical semantic identity before invoking RelationV2.
pub fn price_semantics_digest_v2(
    domain: &EconomicDomainV2,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<[u8; 32], EconomicErrorV2> {
    domain.validate()?;
    validate_price_vector(domain, prices)?;
    let mut hash = Sha256V2::new();
    hash.update(PRICE_SEMANTICS_DIGEST_DOMAIN_V2)?;
    hash.update(&domain.relation_version.to_le_bytes())?;
    hash.update(&domain.market_semantics_digest)?;
    hash.update(&domain.epoch_semantics_digest)?;
    hash.update(&domain.price_policy_digest)?;
    hash.update(&domain.epoch_index.to_le_bytes())?;
    hash.update(&[domain.outcome_count])?;
    hash.update(&domain.price_scale.to_le_bytes())?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hash.update(&prices[outcome].to_le_bytes())?;
        outcome += 1;
    }
    hash.finalize()
}

/// One ownerless nonnegative coefficient-vector order.
///
/// A single-Egg order is the sparse vector with one coefficient equal to one.
/// Portfolio orders use the same representation, so there is no parallel
/// single/portfolio economic truth. `quantity` and `minimum_fill` are order
/// units; filling `f` units transfers `f * coefficients[i]` Egg atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicOrderV2 {
    /// Nonzero canonical content identity, strictly increasing in the book.
    pub order_id: [u8; 32],
    /// Buy or sell side. No owner or counterparty is represented.
    pub side: Side,
    /// Nonnegative Egg atoms per order unit and outcome.
    pub coefficients: [u64; MAX_OUTCOMES],
    /// Maximum order units available, strictly positive.
    pub quantity: u64,
    /// Zero or the smallest accepted nonzero fill.
    pub minimum_fill: u64,
    /// Partial-fill or all-or-none semantics.
    pub partial_policy: PartialPolicy,
    /// Last epoch index at which the order remains eligible.
    pub expiry_epoch: u64,
    /// Exact price-unit limit per order unit.
    ///
    /// A filled buy requires `dot(coefficients, prices) <= limit`; a filled
    /// sell requires the reverse inequality. No division or rounding occurs.
    pub limit_value_price_units_per_unit: u128,
}

/// Canonical unused order slot.
pub const EMPTY_ECONOMIC_ORDER_V2: EconomicOrderV2 = EconomicOrderV2 {
    order_id: [0; 32],
    side: Side::Buy,
    coefficients: [0; MAX_OUTCOMES],
    quantity: 0,
    minimum_fill: 0,
    partial_policy: PartialPolicy::Allow,
    expiry_epoch: 0,
    limit_value_price_units_per_unit: 0,
};

/// Fixed-capacity canonical owner-blind book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicBookV2 {
    /// Orders in strictly increasing content-identity order.
    pub orders: [EconomicOrderV2; MAX_ORDERS],
    /// Active order prefix.
    pub len: u8,
}

impl EconomicBookV2 {
    /// Canonical empty book.
    pub const fn empty() -> Self {
        Self {
            orders: [EMPTY_ECONOMIC_ORDER_V2; MAX_ORDERS],
            len: 0,
        }
    }

    /// Validate every live order and every padding slot.
    pub fn validate(&self, domain: &EconomicDomainV2) -> Result<(), EconomicErrorV2> {
        domain.validate()?;
        if usize::from(self.len) > MAX_ORDERS {
            return Err(EconomicErrorV2::TooManyOrders);
        }
        let mut previous = [0u8; 32];
        let mut order_index = 0usize;
        while order_index < usize::from(self.len) {
            let order = self.orders[order_index];
            if is_zero_digest(&order.order_id) || (order_index != 0 && previous >= order.order_id) {
                return Err(EconomicErrorV2::NonCanonicalOrderOrder {
                    order: bounded_index(order_index)?,
                });
            }
            previous = order.order_id;
            if order.quantity == 0 || order.minimum_fill > order.quantity {
                return Err(EconomicErrorV2::InvalidQuantity {
                    order: bounded_index(order_index)?,
                });
            }
            if order.partial_policy == PartialPolicy::AllOrNone
                && order.minimum_fill != order.quantity
            {
                return Err(EconomicErrorV2::InvalidMinimumFill {
                    order: bounded_index(order_index)?,
                });
            }
            if order.expiry_epoch < domain.epoch_index {
                return Err(EconomicErrorV2::ExpiredOrder {
                    order: bounded_index(order_index)?,
                });
            }
            let mut nonzero = false;
            let mut outcome = 0usize;
            while outcome < domain.outcomes() {
                let coefficient = order.coefficients[outcome];
                nonzero |= coefficient != 0;
                coefficient
                    .checked_mul(order.quantity)
                    .ok_or(EconomicErrorV2::FlowOverflow {
                        order: bounded_index(order_index)?,
                        outcome: bounded_index(outcome)?,
                    })?;
                outcome += 1;
            }
            if !nonzero {
                return Err(EconomicErrorV2::InvalidQuantity {
                    order: bounded_index(order_index)?,
                });
            }
            while outcome < MAX_OUTCOMES {
                if order.coefficients[outcome] != 0 {
                    return Err(EconomicErrorV2::NonCanonicalCoefficientPadding {
                        order: bounded_index(order_index)?,
                        outcome: bounded_index(outcome)?,
                    });
                }
                outcome += 1;
            }
            order_index += 1;
        }
        while order_index < MAX_ORDERS {
            if self.orders[order_index] != EMPTY_ECONOMIC_ORDER_V2 {
                return Err(EconomicErrorV2::NonCanonicalOrderPadding {
                    order: bounded_index(order_index)?,
                });
            }
            order_index += 1;
        }
        Ok(())
    }
}

/// Submitted economic fill coordinates.
///
/// There is deliberately no claimed score or digest. Both are recomputed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EconomicCandidateV2 {
    /// Filled order units followed by canonical zero padding.
    pub fills: [u64; MAX_ORDERS],
    /// Exactly the filled all-or-none orders; no other bit may be set.
    pub honored_aon_mask: u64,
    /// Complete sets created by the virtual split.
    pub virtual_split: u64,
    /// Complete sets destroyed by the virtual merge.
    pub virtual_merge: u64,
}

impl EconomicCandidateV2 {
    /// Canonical empty fill witness.
    pub const EMPTY: Self = Self {
        fills: [0; MAX_ORDERS],
        honored_aon_mask: 0,
        virtual_split: 0,
        virtual_merge: 0,
    };
}

/// Recomputed accepted economics. No field grants settlement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedEconomicsV2 {
    /// Active outcome width.
    pub outcome_count: u8,
    /// Aggregate filled buy legs, including no unmodeled counterparty.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// Aggregate filled sell legs, including no unmodeled counterparty.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
    /// `B_i - sigma = E_i - mu`.
    pub direct_flow: [u64; MAX_OUTCOMES],
    /// Accepted virtual split.
    pub virtual_split: u64,
    /// Accepted virtual merge.
    pub virtual_merge: u64,
    /// Full SHA-256 identity of every canonical economic input.
    pub economic_candidate_digest: [u8; 32],
    /// Independently recomputed ScoreV2-Q key.
    pub score: ScoreV2,
}

/// Validated owner-blind order flow before a counterparty relation closes it.
///
/// This is a crate-internal composition seam, not a weaker public verdict.
/// [`verify_economic_candidate_v2`] still accepts only flows closed by the
/// original RelationV2 equation. The covered dealer extension consumes this
/// value and supplies its own independently checked conservation equation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnbalancedEconomicsV2 {
    /// Aggregate filled user buy legs.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// Aggregate filled user sell legs.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
    /// Full RelationV2 identity before any counterparty extension.
    pub economic_candidate_digest: [u8; 32],
}

/// Every deterministic refusal in the first owner-blind core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EconomicErrorV2 {
    /// Relation version was not V2.
    UnknownRelationVersion,
    /// A required semantic content identity was all zero.
    ZeroSemanticDigest,
    /// Outcome width was outside `2..=16`.
    InvalidOutcomeCount,
    /// Price scale was zero.
    InvalidPriceScale,
    /// Price precondition named a different policy.
    PricePolicyMismatch,
    /// Claimed semantic price identity did not equal the canonical projection.
    PriceSemanticDigestMismatch,
    /// One active price exceeded the scale.
    PriceOutOfRange { outcome: u8 },
    /// Active prices did not sum exactly to the scale.
    SimplexSumMismatch,
    /// An inactive price was nonzero.
    NonCanonicalPricePadding { outcome: u8 },
    /// Book length exceeded the fixed capacity.
    TooManyOrders,
    /// Order identities were zero, duplicated, or unordered.
    NonCanonicalOrderOrder { order: u8 },
    /// Quantity was zero, minimum exceeded quantity, or coefficients were zero.
    InvalidQuantity { order: u8 },
    /// An all-or-none order did not bind its full quantity as minimum.
    InvalidMinimumFill { order: u8 },
    /// Order expired before this epoch.
    ExpiredOrder { order: u8 },
    /// An inactive coefficient was nonzero.
    NonCanonicalCoefficientPadding { order: u8, outcome: u8 },
    /// An inactive order slot was not the exact empty value.
    NonCanonicalOrderPadding { order: u8 },
    /// An inactive fill was nonzero.
    NonCanonicalFillPadding { order: u8 },
    /// Fill exceeded the frozen quantity.
    FillExceedsQuantity { order: u8 },
    /// A nonzero partial fill was below its minimum.
    MinimumFillViolation { order: u8 },
    /// An all-or-none fill was neither zero nor full.
    AllOrNoneViolation { order: u8 },
    /// AON mask did not exactly describe a filled AON order.
    AonMaskMismatch { order: u8 },
    /// A mask bit was set for a partial-fill order or padding slot.
    AonMaskNotApplicable { order: u8 },
    /// A filled order violated its exact price-unit limit.
    LimitViolation { order: u8 },
    /// Coefficient multiplication or aggregate flow exceeded `u64`.
    FlowOverflow { order: u8, outcome: u8 },
    /// Both virtual split and merge were nonzero.
    NonCanonicalVirtualConversion,
    /// Virtual split exceeded aggregate buy flow.
    VirtualSplitExceedsBuy { outcome: u8 },
    /// Virtual merge exceeded aggregate sell flow.
    VirtualMergeExceedsSell { outcome: u8 },
    /// `B_i + mu != E_i + sigma`.
    OutcomeConservationMismatch { outcome: u8 },
    /// A checked non-flow integer calculation overflowed.
    ArithmeticOverflow,
    /// The already-validated aggregate unexpectedly failed ScoreV2.
    Score(ScoreErrorV2),
}

/// Verify one submitted owner-blind economic candidate.
///
/// Success means only that this fixed book, price precondition, fill witness,
/// and virtual conversion satisfy the exact RelationV2 equations. It is not an
/// optimality result, price theorem, settlement authorization, or deployment
/// statement.
pub fn verify_economic_candidate_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> Result<VerifiedEconomicsV2, EconomicErrorV2> {
    let unbalanced = derive_unbalanced_economics_v2(domain, book, price, candidate)?;
    let buy_flow = unbalanced.aggregate_buy_flow;
    let sell_flow = unbalanced.aggregate_sell_flow;

    let mut direct_flow = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let from_buy = buy_flow[outcome]
            .checked_sub(candidate.virtual_split)
            .ok_or(EconomicErrorV2::VirtualSplitExceedsBuy {
                outcome: bounded_index(outcome)?,
            })?;
        let from_sell = sell_flow[outcome]
            .checked_sub(candidate.virtual_merge)
            .ok_or(EconomicErrorV2::VirtualMergeExceedsSell {
                outcome: bounded_index(outcome)?,
            })?;
        let left = buy_flow[outcome]
            .checked_add(candidate.virtual_merge)
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        let right = sell_flow[outcome]
            .checked_add(candidate.virtual_split)
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        if left != right || from_buy != from_sell {
            return Err(EconomicErrorV2::OutcomeConservationMismatch {
                outcome: bounded_index(outcome)?,
            });
        }
        direct_flow[outcome] = from_buy;
        outcome += 1;
    }

    let digest = unbalanced.economic_candidate_digest;
    let delta = CandidateDeltaV2 {
        normalization_policy: NormalizationPolicyV2::OwnerBlindAggregate,
        outcome_count: domain.outcome_count,
        aggregate_buy_flow: buy_flow,
        aggregate_sell_flow: sell_flow,
        claimed_direct_flow: direct_flow,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        candidate_digest: digest,
    };
    let score = score_candidate_v2(&delta).map_err(EconomicErrorV2::Score)?;
    Ok(VerifiedEconomicsV2 {
        outcome_count: domain.outcome_count,
        aggregate_buy_flow: buy_flow,
        aggregate_sell_flow: sell_flow,
        direct_flow,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        economic_candidate_digest: digest,
        score,
    })
}

/// Validate every RelationV2 input and derive user flow without declaring it
/// conserved.
///
/// Only crate-owned counterparty relations may consume this seam. Returning a
/// value here is not candidate acceptance: it deliberately carries no score or
/// public verified type.
pub(crate) fn derive_unbalanced_economics_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> Result<UnbalancedEconomicsV2, EconomicErrorV2> {
    domain.validate()?;
    book.validate(domain)?;
    price.validate(domain)?;
    if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
        return Err(EconomicErrorV2::NonCanonicalVirtualConversion);
    }

    let mut order_index = usize::from(book.len);
    while order_index < MAX_ORDERS {
        if candidate.fills[order_index] != 0 {
            return Err(EconomicErrorV2::NonCanonicalFillPadding {
                order: bounded_index(order_index)?,
            });
        }
        if mask_bit(candidate.honored_aon_mask, order_index) {
            return Err(EconomicErrorV2::AonMaskNotApplicable {
                order: bounded_index(order_index)?,
            });
        }
        order_index += 1;
    }

    let mut buy_flow = [0u64; MAX_OUTCOMES];
    let mut sell_flow = [0u64; MAX_OUTCOMES];
    order_index = 0;
    while order_index < usize::from(book.len) {
        let order = book.orders[order_index];
        let fill = candidate.fills[order_index];
        let aon_bit = mask_bit(candidate.honored_aon_mask, order_index);
        if fill > order.quantity {
            return Err(EconomicErrorV2::FillExceedsQuantity {
                order: bounded_index(order_index)?,
            });
        }
        match order.partial_policy {
            PartialPolicy::Allow => {
                if aon_bit {
                    return Err(EconomicErrorV2::AonMaskNotApplicable {
                        order: bounded_index(order_index)?,
                    });
                }
                if fill != 0 && fill < order.minimum_fill {
                    return Err(EconomicErrorV2::MinimumFillViolation {
                        order: bounded_index(order_index)?,
                    });
                }
            }
            PartialPolicy::AllOrNone => {
                if fill != 0 && fill != order.quantity {
                    return Err(EconomicErrorV2::AllOrNoneViolation {
                        order: bounded_index(order_index)?,
                    });
                }
                if aon_bit != (fill != 0) {
                    return Err(EconomicErrorV2::AonMaskMismatch {
                        order: bounded_index(order_index)?,
                    });
                }
            }
        }

        let unit_value = order_unit_value(&order, &price.prices, domain.outcomes())?;
        if fill != 0 {
            let limit_ok = match order.side {
                Side::Buy => unit_value <= order.limit_value_price_units_per_unit,
                Side::Sell => unit_value >= order.limit_value_price_units_per_unit,
            };
            if !limit_ok {
                return Err(EconomicErrorV2::LimitViolation {
                    order: bounded_index(order_index)?,
                });
            }
        }

        let mut outcome = 0usize;
        while outcome < domain.outcomes() {
            let leg = order.coefficients[outcome].checked_mul(fill).ok_or(
                EconomicErrorV2::FlowOverflow {
                    order: bounded_index(order_index)?,
                    outcome: bounded_index(outcome)?,
                },
            )?;
            let cell = match order.side {
                Side::Buy => &mut buy_flow[outcome],
                Side::Sell => &mut sell_flow[outcome],
            };
            *cell = cell.checked_add(leg).ok_or(EconomicErrorV2::FlowOverflow {
                order: bounded_index(order_index)?,
                outcome: bounded_index(outcome)?,
            })?;
            outcome += 1;
        }
        order_index += 1;
    }

    let digest = economic_candidate_digest(domain, book, price, candidate)?;
    Ok(UnbalancedEconomicsV2 {
        aggregate_buy_flow: buy_flow,
        aggregate_sell_flow: sell_flow,
        economic_candidate_digest: digest,
    })
}

fn order_unit_value(
    order: &EconomicOrderV2,
    prices: &[u64; MAX_OUTCOMES],
    outcomes: usize,
) -> Result<u128, EconomicErrorV2> {
    let mut value = 0u128;
    let mut outcome = 0usize;
    while outcome < outcomes {
        let term = u128::from(order.coefficients[outcome])
            .checked_mul(u128::from(prices[outcome]))
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        value = value
            .checked_add(term)
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        outcome += 1;
    }
    Ok(value)
}

fn mask_bit(mask: u64, order: usize) -> bool {
    order < 64 && ((mask >> order) & 1) != 0
}

fn bounded_index(index: usize) -> Result<u8, EconomicErrorV2> {
    u8::try_from(index).map_err(|_| EconomicErrorV2::ArithmeticOverflow)
}

fn is_zero_digest(digest: &[u8; 32]) -> bool {
    let mut index = 0usize;
    while index < digest.len() {
        if digest[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn economic_candidate_digest(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> Result<[u8; 32], EconomicErrorV2> {
    let mut hash = Sha256V2::new();
    hash.update(ECONOMIC_CANDIDATE_DIGEST_DOMAIN_V2)?;
    hash.update(&domain.relation_version.to_le_bytes())?;
    hash.update(&domain.market_semantics_digest)?;
    hash.update(&domain.epoch_semantics_digest)?;
    hash.update(&domain.relation_policy_digest)?;
    hash.update(&domain.price_policy_digest)?;
    hash.update(&domain.epoch_index.to_le_bytes())?;
    hash.update(&[domain.outcome_count])?;
    hash.update(&domain.price_scale.to_le_bytes())?;
    hash.update(&[book.len])?;
    let mut order_index = 0usize;
    while order_index < MAX_ORDERS {
        let order = book.orders[order_index];
        hash.update(&order.order_id)?;
        hash.update(&[side_byte(order.side)])?;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            hash.update(&order.coefficients[outcome].to_le_bytes())?;
            outcome += 1;
        }
        hash.update(&order.quantity.to_le_bytes())?;
        hash.update(&order.minimum_fill.to_le_bytes())?;
        hash.update(&[partial_byte(order.partial_policy)])?;
        hash.update(&order.expiry_epoch.to_le_bytes())?;
        hash.update(&order.limit_value_price_units_per_unit.to_le_bytes())?;
        order_index += 1;
    }
    hash.update(&price.policy_digest)?;
    hash.update(&price.semantic_price_digest)?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hash.update(&price.prices[outcome].to_le_bytes())?;
        outcome += 1;
    }
    hash.update(&candidate.virtual_split.to_le_bytes())?;
    hash.update(&candidate.virtual_merge.to_le_bytes())?;
    order_index = 0;
    while order_index < MAX_ORDERS {
        hash.update(&candidate.fills[order_index].to_le_bytes())?;
        order_index += 1;
    }
    hash.update(&candidate.honored_aon_mask.to_le_bytes())?;
    hash.finalize()
}

const fn side_byte(side: Side) -> u8 {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
    }
}

const fn partial_byte(policy: PartialPolicy) -> u8 {
    match policy {
        PartialPolicy::Allow => 0,
        PartialPolicy::AllOrNone => 1,
    }
}

/// Small, allocation-free SHA-256 state used solely for canonical semantic
/// identities. The implementation follows FIPS 180-4 and has independent
/// known-answer tests. It is ordinary safe Rust, not an FFI or runtime syscall.
pub(crate) struct Sha256V2 {
    state: [u32; 8],
    block: [u8; 64],
    block_len: usize,
    message_len: u64,
}

impl Sha256V2 {
    pub(crate) const fn new() -> Self {
        Self {
            state: [
                0x6a09_e667,
                0xbb67_ae85,
                0x3c6e_f372,
                0xa54f_f53a,
                0x510e_527f,
                0x9b05_688c,
                0x1f83_d9ab,
                0x5be0_cd19,
            ],
            block: [0; 64],
            block_len: 0,
            message_len: 0,
        }
    }

    pub(crate) fn update(&mut self, input: &[u8]) -> Result<(), EconomicErrorV2> {
        let input_len =
            u64::try_from(input.len()).map_err(|_| EconomicErrorV2::ArithmeticOverflow)?;
        self.message_len = self
            .message_len
            .checked_add(input_len)
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        let mut consumed = 0usize;
        while consumed < input.len() {
            let available = 64 - self.block_len;
            let remaining = input.len() - consumed;
            let take = if available < remaining {
                available
            } else {
                remaining
            };
            self.block[self.block_len..self.block_len + take]
                .copy_from_slice(&input[consumed..consumed + take]);
            self.block_len += take;
            consumed += take;
            if self.block_len == 64 {
                let block = self.block;
                compress_sha256(&mut self.state, &block);
                self.block = [0; 64];
                self.block_len = 0;
            }
        }
        Ok(())
    }

    pub(crate) fn finalize(mut self) -> Result<[u8; 32], EconomicErrorV2> {
        let bit_len = self
            .message_len
            .checked_mul(8)
            .ok_or(EconomicErrorV2::ArithmeticOverflow)?;
        self.block[self.block_len] = 0x80;
        self.block_len += 1;
        if self.block_len > 56 {
            self.block[self.block_len..].fill(0);
            let block = self.block;
            compress_sha256(&mut self.state, &block);
            self.block = [0; 64];
            self.block_len = 0;
        }
        self.block[self.block_len..56].fill(0);
        self.block[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.block;
        compress_sha256(&mut self.state, &block);

        let mut output = [0u8; 32];
        let mut word = 0usize;
        while word < self.state.len() {
            output[word * 4..word * 4 + 4].copy_from_slice(&self.state[word].to_be_bytes());
            word += 1;
        }
        Ok(output)
    }
}

fn compress_sha256(state: &mut [u32; 8], block: &[u8; 64]) {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut schedule = [0u32; 64];
    let mut index = 0usize;
    while index < 16 {
        let at = index * 4;
        schedule[index] =
            u32::from_be_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
        index += 1;
    }
    while index < 64 {
        let x = schedule[index - 15];
        let y = schedule[index - 2];
        let small_zero = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
        let small_one = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(small_zero)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(small_one);
        index += 1;
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];
    index = 0;
    while index < 64 {
        let big_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ ((!e) & g);
        let first = h
            .wrapping_add(big_one)
            .wrapping_add(choose)
            .wrapping_add(K[index])
            .wrapping_add(schedule[index]);
        let big_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = big_zero.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
        index += 1;
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[cfg(test)]
pub(crate) fn sha256_test_vector(input: &[u8]) -> Result<[u8; 32], EconomicErrorV2> {
    let mut hash = Sha256V2::new();
    hash.update(input)?;
    hash.finalize()
}
