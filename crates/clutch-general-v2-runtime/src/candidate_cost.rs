// SPDX-License-Identifier: AGPL-3.0-or-later

//! Preselection owner-net cost certificate for one verified RelationV2 candidate.
//!
//! This module consumes only the frozen General order projection, exact prices,
//! exact fills, an independently rechecked RelationV2 result, and a
//! content-addressed immutable batch policy. It cannot observe a selected fee
//! record, payer allocation, treasury, signer, or postselection account.

use clutch_batch::relation_v1::{
    FrozenPolicyV1, RoundingBoundaryV1, MAX_OUTCOMES,
};
use clutch_batch::relation_v2::{
    verify_economic_candidate_v2, EconomicCandidateV2, EconomicErrorV2,
    PricePreconditionV2, VerifiedEconomicsV2,
};
use clutch_batch::Side;
use clutch_batch_policy_identity::batch_policy_digest;
use clutch_general_v2_contract::{
    encode_score_v2_q_cost_first_admitted_tie_v1, AdmissionNodeStatusV1,
    AdmissionNodeV4AccountV1, CandidateWindowV5AccountV1, CodecError,
    CompleteCostedCandidateRankPoststateV1, CompleteCostedCandidateRankTransitionV1,
    CostedCandidateVerdictProjectionV1, FirstAdmittedTieV1, Id32, MarketBindingV2,
    ScoreV2QComponentsV1, ScoreV2QCostComponentsV1, MAX_ORDERS,
    SCORE_V2_Q_COST_ACTIVE_RANK_BYTES, SCORE_V2_Q_RANK_CAPACITY,
};
use sha2::{Digest, Sha256};

use crate::{builder::OwnerBlindBookProjectionV2, VerifiedCostedSmoothDirectCandidateV1};

/// Domain for the canonical aggregate certificate identity.
pub const CANDIDATE_COST_CERTIFICATE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/candidate-cost-certificate/v1\0";
/// Domain for the canonical sorted owner-net row transcript.
pub const CANDIDATE_COST_OWNER_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/candidate-cost-owner-transcript/v1\0";
/// Exact fixed width of [`CandidateCostCertificateV1::canonical_bytes`].
pub const CANDIDATE_COST_CERTIFICATE_BYTES_V1: usize = 288;

const CERTIFICATE_MAGIC_V1: [u8; 8] = *b"DCCOST1\0";
const CERTIFICATE_VERSION_V1: u8 = 1;

/// Content-addressed immutable batch-policy preimage used by candidate cost.
///
/// This value proves only that `policy_id` is the canonical digest of `policy`.
/// The SBF adapter must still authenticate the immutable policy account and its
/// market binding. There is deliberately no caller-supplied authentication
/// boolean.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCostPolicyV1 {
    policy: FrozenPolicyV1,
    policy_id: Id32,
}

impl CandidateCostPolicyV1 {
    /// Bind a policy preimage to its exact canonical content identity.
    pub fn bind(policy: FrozenPolicyV1, policy_id: Id32) -> Result<Self, CandidateCostErrorV1> {
        policy
            .validate()
            .map_err(|_| CandidateCostErrorV1::InvalidBatchPolicy)?;
        if policy.rounding != RoundingBoundaryV1::TerminalOwnerFloor {
            return Err(CandidateCostErrorV1::UnsupportedRoundingBoundary);
        }
        let observed = batch_policy_digest(&policy)
            .map_err(|_| CandidateCostErrorV1::InvalidBatchPolicy)?;
        if observed.0 != policy_id.bytes() {
            return Err(CandidateCostErrorV1::BindingMismatch);
        }
        Ok(Self { policy, policy_id })
    }

    /// Exact immutable policy preimage.
    pub const fn policy(&self) -> &FrozenPolicyV1 {
        &self.policy
    }

    /// Canonical content identity of the immutable policy.
    pub const fn policy_id(&self) -> Id32 {
        self.policy_id
    }

    /// Exact-join this content-addressed preimage to the immutable V2 Market
    /// owner and the breaking score-policy identity.
    pub fn binds_market(&self, market: &MarketBindingV2) -> Result<(), CandidateCostErrorV1> {
        market
            .validate()
            .map_err(CandidateCostErrorV1::Codec)?;
        let score_policy_id = crate::score_v2_q_cost_policy_id_v1()
            .map_err(|_| CandidateCostErrorV1::BindingMismatch)?;
        if market.batch_policy_id() != self.policy_id
            || market.base().score_policy_id != score_policy_id
        {
            return Err(CandidateCostErrorV1::BindingMismatch);
        }
        Ok(())
    }
}

/// Private, relation-derived preselection cost result.
///
/// The owner-net coordinate quotients every owner's signed contingent payoff
/// by constant complete-set translations before valuation. Thus adding a
/// risk-free complete-set component with its exact simplex cash equivalent
/// cannot improve or worsen `owner_net_cost_atoms`. Gross execution cash and
/// virtual conversion are retained as factual work counters, not folded into
/// that economic coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCostCertificateV1 {
    economic_candidate_id: Id32,
    batch_policy_id: Id32,
    order_set_id: Id32,
    price_semantics_id: Id32,
    outcome_count: u8,
    owner_count: u8,
    filled_order_count: u8,
    owner_net_risk_atoms: u128,
    owner_net_cost_price_units: u128,
    owner_net_cost_atoms: u64,
    execution_buy_price_units: u128,
    execution_sell_price_units: u128,
    terminal_rounding_residue_price_units: u128,
    virtual_split_atoms: u64,
    virtual_merge_atoms: u64,
    owner_transcript_id: Id32,
}

/// Private action-14 projection from a successful cost-aware runtime verdict.
///
/// A caller cannot construct this value or replace its rank/certificate facts.
/// It is request-scoped and not a persisted authority. A future SBF action 14
/// must obtain it in the same invocation that mutates Node/Window state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostedCandidateAction14ProjectionV1 {
    economic_candidate_id: Id32,
    batch_policy_id: Id32,
    score_policy_id: Id32,
    certificate_id: Id32,
    owner_transcript_id: Id32,
    components: ScoreV2QCostComponentsV1,
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

impl CostedCandidateAction14ProjectionV1 {
    /// Checked RelationV2 candidate identity.
    pub const fn economic_candidate_id(&self) -> Id32 {
        self.economic_candidate_id
    }

    /// Immutable batch-policy identity owned by MarketBinding V2.
    pub const fn batch_policy_id(&self) -> Id32 {
        self.batch_policy_id
    }

    /// Breaking cost-aware ScoreV2-Q policy identity.
    pub const fn score_policy_id(&self) -> Id32 {
        self.score_policy_id
    }

    /// Canonical ephemeral certificate content identity.
    pub const fn certificate_id(&self) -> Id32 {
        self.certificate_id
    }

    /// Canonical sorted owner-net transcript identity.
    pub const fn owner_transcript_id(&self) -> Id32 {
        self.owner_transcript_id
    }

    /// Exact components consumed by the General mutation owner.
    pub const fn components(&self) -> ScoreV2QCostComponentsV1 {
        self.components
    }

    /// Exact 96-byte rank consumed by the General mutation owner.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }
}

/// Request-scoped action-15 join to a rank persisted only by action 14.
///
/// This join does not recreate or independently own the certificate. It checks
/// that the selected Node and Window carry the exact 96-byte rank written by
/// the cost-aware action-14 policy and that the same immutable MarketBinding
/// still owns the batch/score policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostedCandidateAction15ProjectionV1 {
    selected_candidate_id: Id32,
    selected_node_id: Id32,
    batch_policy_id: Id32,
    score_policy_id: Id32,
    certificate_id: Id32,
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

impl CostedCandidateAction15ProjectionV1 {
    /// Best valid submitted settlement candidate selected by the Window.
    pub const fn selected_candidate_id(&self) -> Id32 {
        self.selected_candidate_id
    }

    /// AdmissionNode whose action-14 transition persisted the winning rank.
    pub const fn selected_node_id(&self) -> Id32 {
        self.selected_node_id
    }

    /// Immutable batch-policy identity reread from MarketBinding V2.
    pub const fn batch_policy_id(&self) -> Id32 {
        self.batch_policy_id
    }

    /// Breaking score-policy identity shared by Market, Window, and Node.
    pub const fn score_policy_id(&self) -> Id32 {
        self.score_policy_id
    }

    /// Exact winning certificate ID persisted by action 14 for the counted
    /// settlement root that will own action-15 output.
    pub const fn certificate_id(&self) -> Id32 {
        self.certificate_id
    }

    /// Exact winning rank already persisted by action 14.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }
}

/// Project a successful private runtime verdict into the only action-14 facts
/// the General state transition may consume.
pub fn project_costed_candidate_action14_v1(
    verified: &VerifiedCostedSmoothDirectCandidateV1,
    market: &MarketBindingV2,
    node: &AdmissionNodeV4AccountV1,
) -> Result<CostedCandidateAction14ProjectionV1, CandidateCostErrorV1> {
    market
        .validate()
        .map_err(CandidateCostErrorV1::Codec)?;
    node.validate().map_err(CandidateCostErrorV1::Codec)?;
    let node_base = node.base();
    let certificate = verified.cost_certificate();
    let economics = verified.economics();
    let score_policy_id = crate::score_v2_q_cost_policy_id_v1()
        .map_err(|_| CandidateCostErrorV1::BindingMismatch)?;
    let candidate_id = Id32::new(economics.economic_candidate_digest)
        .map_err(CandidateCostErrorV1::Codec)?;
    if market.batch_policy_id() != certificate.batch_policy_id()
        || market.base().score_policy_id != score_policy_id
        || node_base.market != market.base().market
        || node_base.relation_policy_id != market.base().relation_policy_id
        || node_base.admission_policy_id != market.base().admission_policy_id
        || node_base.score_policy_id != score_policy_id
        || node_base.settlement_candidate_id != candidate_id
        || certificate.economic_candidate_id() != candidate_id
        || !node.cost_certificate_id().is_zero()
    {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    let components = ScoreV2QCostComponentsV1 {
        score: ScoreV2QComponentsV1 {
            certified_risk_flow_atoms: economics.score.risk.certified_risk_flow_atoms,
            cash_equivalent_direct_flow_atoms: economics.score.cash_equivalent_direct_flow_atoms,
            virtual_churn_atoms: economics.score.virtual_churn_atoms,
            settlement_candidate_id: candidate_id,
        },
        owner_net_cost_atoms: certificate.owner_net_cost_atoms(),
    };
    let rank_key = encode_score_v2_q_cost_first_admitted_tie_v1(
        components,
        FirstAdmittedTieV1 {
            ordinal: node_base.ordinal,
        },
    )
    .map_err(CandidateCostErrorV1::Codec)?;
    if rank_key != *verified.rank_key() {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    Ok(CostedCandidateAction14ProjectionV1 {
        economic_candidate_id: candidate_id,
        batch_policy_id: certificate.batch_policy_id(),
        score_policy_id,
        certificate_id: certificate.content_id()?,
        owner_transcript_id: certificate.owner_transcript_id(),
        components,
        rank_key,
    })
}

/// Rejoin action 15 to the rank and policy chain persisted by action 14.
pub fn project_costed_candidate_action15_v1(
    market: &MarketBindingV2,
    window: &CandidateWindowV5AccountV1,
    selected_node: &AdmissionNodeV4AccountV1,
) -> Result<CostedCandidateAction15ProjectionV1, CandidateCostErrorV1> {
    market
        .validate()
        .map_err(CandidateCostErrorV1::Codec)?;
    window.validate().map_err(CandidateCostErrorV1::Codec)?;
    selected_node
        .validate()
        .map_err(CandidateCostErrorV1::Codec)?;
    let window_base = window.base();
    let node_base = selected_node.base();
    let score_policy_id = crate::score_v2_q_cost_policy_id_v1()
        .map_err(|_| CandidateCostErrorV1::BindingMismatch)?;
    if market.base().score_policy_id != score_policy_id
        || window_base.market != market.base().market
        || node_base.market != market.base().market
        || window_base.relation_policy_id != market.base().relation_policy_id
        || node_base.relation_policy_id != market.base().relation_policy_id
        || window_base.admission_policy_id != market.base().admission_policy_id
        || node_base.admission_policy_id != market.base().admission_policy_id
        || window_base.score_policy_id != score_policy_id
        || node_base.score_policy_id != score_policy_id
        || window_base.epoch != node_base.epoch
        || window_base.epoch_generation != node_base.epoch_generation
        || node_base.status != AdmissionNodeStatusV1::VerifiedValid
        || usize::from(node_base.rank_key_len) != SCORE_V2_Q_COST_ACTIVE_RANK_BYTES
        || selected_node.cost_certificate_id().is_zero()
        || window_base.best_candidate_node != node_base.node
        || window_base.best_settlement_candidate_id != node_base.settlement_candidate_id
        || window_base.best_rank_key != node_base.rank_key
        || window_base.best_ordinal != node_base.ordinal
    {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    Ok(CostedCandidateAction15ProjectionV1 {
        selected_candidate_id: node_base.settlement_candidate_id,
        selected_node_id: node_base.node,
        batch_policy_id: market.batch_policy_id(),
        score_policy_id,
        certificate_id: selected_node.cost_certificate_id(),
        rank_key: node_base.rank_key,
    })
}

/// Private executable action-14 plan. Work V3 terminalization must still be
/// composed atomically by the SBF adapter; this plan owns only the exact
/// Node/Window cost-rank mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCostedCandidateAction14V1 {
    poststate: CompleteCostedCandidateRankPoststateV1,
    certificate_id: Id32,
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

impl PreparedCostedCandidateAction14V1 {
    /// Exact Window successor poststate.
    pub const fn window(&self) -> &CandidateWindowV5AccountV1 {
        &self.poststate.window
    }

    /// Exact AdmissionNode successor poststate.
    pub const fn node(&self) -> &AdmissionNodeV4AccountV1 {
        &self.poststate.node
    }

    /// Checked certificate content identity persisted on the Node.
    pub const fn certificate_id(&self) -> Id32 {
        self.certificate_id
    }

    /// Exact 96-byte rank persisted by the transition.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }
}

/// Compose the private checked certificate into the contract-owned action-14
/// Node/Window successor transition.
pub fn prepare_costed_candidate_action14_v1(
    verified: &VerifiedCostedSmoothDirectCandidateV1,
    market: &MarketBindingV2,
    window: &CandidateWindowV5AccountV1,
    node: &AdmissionNodeV4AccountV1,
    current_slot: u64,
) -> Result<PreparedCostedCandidateAction14V1, CandidateCostErrorV1> {
    let projection = project_costed_candidate_action14_v1(verified, market, node)?;
    let poststate = clutch_general_v2_contract::complete_costed_candidate_rank_poststate_v1(
        CompleteCostedCandidateRankTransitionV1 {
            current_slot,
            verdict: CostedCandidateVerdictProjectionV1 {
                components: projection.components(),
                certificate_id: projection.certificate_id(),
                rank_key: *projection.rank_key(),
            },
            window,
            node,
            market,
        },
    )
    .map_err(CandidateCostErrorV1::Codec)?;
    if poststate.node.cost_certificate_id() != projection.certificate_id()
        || poststate.node.base().rank_key != *projection.rank_key()
    {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    Ok(PreparedCostedCandidateAction14V1 {
        poststate,
        certificate_id: projection.certificate_id(),
        rank_key: *projection.rank_key(),
    })
}

impl CandidateCostCertificateV1 {
    /// Checked RelationV2 candidate identity.
    pub const fn economic_candidate_id(&self) -> Id32 {
        self.economic_candidate_id
    }

    /// Immutable content-addressed batch-policy identity.
    pub const fn batch_policy_id(&self) -> Id32 {
        self.batch_policy_id
    }

    /// Frozen General order-set identity supplying owner membership.
    pub const fn order_set_id(&self) -> Id32 {
        self.order_set_id
    }

    /// Exact RelationV2 price-semantics identity.
    pub const fn price_semantics_id(&self) -> Id32 {
        self.price_semantics_id
    }

    /// Number of active outcomes.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }

    /// Number of lexicographically distinct owners with a nonzero fill.
    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }

    /// Number of RelationV2 orders with a nonzero fill.
    pub const fn filled_order_count(&self) -> u8 {
        self.filled_order_count
    }

    /// Sum of per-owner quotient payoff ranges, in Egg atoms.
    pub const fn owner_net_risk_atoms(&self) -> u128 {
        self.owner_net_risk_atoms
    }

    /// Sum of exact state-price values of per-owner quotient payoffs.
    pub const fn owner_net_cost_price_units(&self) -> u128 {
        self.owner_net_cost_price_units
    }

    /// Sum of per-owner terminal ceilings of quotient payoff value.
    ///
    /// This is the sole new ranking coordinate. It is not an assessed fee or
    /// evidence that collateral was reserved, paid, or collected.
    pub const fn owner_net_cost_atoms(&self) -> u64 {
        self.owner_net_cost_atoms
    }

    /// Exact filled buy consideration before the terminal conversion boundary.
    pub const fn execution_buy_price_units(&self) -> u128 {
        self.execution_buy_price_units
    }

    /// Exact filled sell consideration before the terminal conversion boundary.
    pub const fn execution_sell_price_units(&self) -> u128 {
        self.execution_sell_price_units
    }

    /// Exact non-fee buyer-ceil plus seller-floor residue in price units.
    pub const fn terminal_rounding_residue_price_units(&self) -> u128 {
        self.terminal_rounding_residue_price_units
    }

    /// Exact RelationV2 virtual split work.
    pub const fn virtual_split_atoms(&self) -> u64 {
        self.virtual_split_atoms
    }

    /// Exact RelationV2 virtual merge work.
    pub const fn virtual_merge_atoms(&self) -> u64 {
        self.virtual_merge_atoms
    }

    /// Canonical sorted transcript identity of every owner quotient row.
    pub const fn owner_transcript_id(&self) -> Id32 {
        self.owner_transcript_id
    }

    /// Encode the one canonical fixed-width certificate preimage.
    pub fn canonical_bytes(&self) -> [u8; CANDIDATE_COST_CERTIFICATE_BYTES_V1] {
        let mut output = [0u8; CANDIDATE_COST_CERTIFICATE_BYTES_V1];
        let mut at = 0usize;
        put(&mut output, &mut at, &CERTIFICATE_MAGIC_V1);
        put(&mut output, &mut at, &[CERTIFICATE_VERSION_V1]);
        put(&mut output, &mut at, &[self.outcome_count]);
        put(&mut output, &mut at, &[self.owner_count]);
        put(&mut output, &mut at, &[self.filled_order_count]);
        put(&mut output, &mut at, &[0; 4]);
        put(&mut output, &mut at, &self.economic_candidate_id.bytes());
        put(&mut output, &mut at, &self.batch_policy_id.bytes());
        put(&mut output, &mut at, &self.order_set_id.bytes());
        put(&mut output, &mut at, &self.price_semantics_id.bytes());
        put(&mut output, &mut at, &self.owner_net_risk_atoms.to_le_bytes());
        put(
            &mut output,
            &mut at,
            &self.owner_net_cost_price_units.to_le_bytes(),
        );
        put(&mut output, &mut at, &self.owner_net_cost_atoms.to_le_bytes());
        put(
            &mut output,
            &mut at,
            &self.execution_buy_price_units.to_le_bytes(),
        );
        put(
            &mut output,
            &mut at,
            &self.execution_sell_price_units.to_le_bytes(),
        );
        put(
            &mut output,
            &mut at,
            &self.terminal_rounding_residue_price_units.to_le_bytes(),
        );
        put(&mut output, &mut at, &self.virtual_split_atoms.to_le_bytes());
        put(&mut output, &mut at, &self.virtual_merge_atoms.to_le_bytes());
        put(&mut output, &mut at, &self.owner_transcript_id.bytes());
        put(&mut output, &mut at, &[0; 8]);
        debug_assert_eq!(at, CANDIDATE_COST_CERTIFICATE_BYTES_V1);
        output
    }

    /// SHA-256 content identity over the canonical fixed-width preimage.
    pub fn content_id(&self) -> Result<Id32, CandidateCostErrorV1> {
        let bytes = self.canonical_bytes();
        Id32::new(hash_parts(&[CANDIDATE_COST_CERTIFICATE_DOMAIN_V1, &bytes]))
            .map_err(CandidateCostErrorV1::Codec)
    }
}

/// Deterministic refusals owned by candidate-cost derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateCostErrorV1 {
    /// A required content identity was zero.
    Codec(CodecError),
    /// The immutable batch-policy preimage or registered parameter was invalid.
    InvalidBatchPolicy,
    /// Candidate Cost V1 only implements General's terminal-owner boundary.
    UnsupportedRoundingBoundary,
    /// A book, owner membership, candidate, price, or content identity differed.
    BindingMismatch,
    /// RelationV2 refused the supplied economic inputs.
    Relation(EconomicErrorV2),
    /// A checked fixed-width integer operation overflowed.
    ArithmeticOverflow,
}

impl From<EconomicErrorV2> for CandidateCostErrorV1 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Relation(value)
    }
}

/// Recheck RelationV2 and derive its canonical preselection cost certificate.
///
/// Reverification is deliberate: [`VerifiedEconomicsV2`] has public fields and
/// is therefore not accepted as authority by this public entry point.
pub fn verify_candidate_cost_certificate_v1(
    projection: &OwnerBlindBookProjectionV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    policy: &CandidateCostPolicyV1,
) -> Result<CandidateCostCertificateV1, CandidateCostErrorV1> {
    let economics = verify_economic_candidate_v2(
        projection.base().domain(),
        projection.base().book(),
        price,
        candidate,
    )?;
    derive_candidate_cost_certificate_v1(projection, price, candidate, &economics, policy)
}

pub(crate) fn derive_candidate_cost_certificate_v1(
    projection: &OwnerBlindBookProjectionV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    economics: &VerifiedEconomicsV2,
    policy: &CandidateCostPolicyV1,
) -> Result<CandidateCostCertificateV1, CandidateCostErrorV1> {
    let base = projection.base();
    let domain = base.domain();
    price.validate(domain)?;
    if *economics != verify_economic_candidate_v2(domain, base.book(), price, candidate)?
        || policy.policy.rounding != RoundingBoundaryV1::TerminalOwnerFloor
    {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }

    let mut rows = [OwnerCostAccumulatorV1::EMPTY; MAX_ORDERS];
    let mut owner_count = 0usize;
    let mut filled_order_count = 0u8;
    let mut order_index = 0usize;
    while order_index < usize::from(base.book().len) {
        let fill = candidate.fills[order_index];
        if fill != 0 {
            let membership = base
                .order_membership(
                    u8::try_from(order_index)
                        .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(CandidateCostErrorV1::BindingMismatch)?;
            let order = base.book().orders[order_index];
            if membership.owner().is_zero()
                || membership.order_id().bytes() != order.order_id
            {
                return Err(CandidateCostErrorV1::BindingMismatch);
            }
            let row_index = find_or_insert_owner(&mut rows, &mut owner_count, membership.owner())?;
            let row = &mut rows[row_index];
            let mut unit_value = 0u128;
            let mut outcome = 0usize;
            while outcome < usize::from(domain.outcome_count) {
                let leg = order.coefficients[outcome]
                    .checked_mul(fill)
                    .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
                row.contingent_delta_atoms[outcome] = match order.side {
                    Side::Buy => row.contingent_delta_atoms[outcome]
                        .checked_add(i128::from(leg))
                        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
                    Side::Sell => row.contingent_delta_atoms[outcome]
                        .checked_sub(i128::from(leg))
                        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
                };
                unit_value = unit_value
                    .checked_add(
                        u128::from(order.coefficients[outcome])
                            .checked_mul(u128::from(price.prices[outcome]))
                            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
                    )
                    .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
                outcome += 1;
            }
            let consideration = unit_value
                .checked_mul(u128::from(fill))
                .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
            match order.side {
                Side::Buy => {
                    row.buy_price_units = row
                        .buy_price_units
                        .checked_add(consideration)
                        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
                }
                Side::Sell => {
                    row.sell_price_units = row
                        .sell_price_units
                        .checked_add(consideration)
                        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
                }
            }
            row.filled_order_count = row
                .filled_order_count
                .checked_add(1)
                .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
            filled_order_count = filled_order_count
                .checked_add(1)
                .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        }
        order_index += 1;
    }

    let mut owner_hash = Sha256::new();
    owner_hash.update(CANDIDATE_COST_OWNER_TRANSCRIPT_DOMAIN_V1);
    owner_hash.update([domain.outcome_count]);
    owner_hash.update([u8::try_from(owner_count)
        .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?]);
    let mut owner_net_risk_atoms = 0u128;
    let mut owner_net_cost_price_units = 0u128;
    let mut owner_net_cost_atoms = 0u128;
    let mut execution_buy_price_units = 0u128;
    let mut execution_sell_price_units = 0u128;
    let mut terminal_rounding_residue_price_units = 0u128;
    let mut row_index = 0usize;
    while row_index < owner_count {
        let row = finish_owner_row(
            &rows[row_index],
            &price.prices,
            usize::from(domain.outcome_count),
            domain.price_scale,
        )?;
        owner_net_risk_atoms = owner_net_risk_atoms
            .checked_add(row.risk_atoms)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        owner_net_cost_price_units = owner_net_cost_price_units
            .checked_add(row.normalized_value_price_units)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        owner_net_cost_atoms = owner_net_cost_atoms
            .checked_add(u128::from(row.cost_atoms))
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        execution_buy_price_units = execution_buy_price_units
            .checked_add(rows[row_index].buy_price_units)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        execution_sell_price_units = execution_sell_price_units
            .checked_add(rows[row_index].sell_price_units)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        terminal_rounding_residue_price_units = terminal_rounding_residue_price_units
            .checked_add(row.rounding_residue_price_units)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;

        owner_hash.update(rows[row_index].owner.bytes());
        owner_hash.update([rows[row_index].filled_order_count]);
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            owner_hash.update(row.normalized_payoff_atoms[outcome].to_le_bytes());
            outcome += 1;
        }
        owner_hash.update(row.risk_atoms.to_le_bytes());
        owner_hash.update(row.normalized_value_price_units.to_le_bytes());
        owner_hash.update(row.cost_atoms.to_le_bytes());
        owner_hash.update(row.rounding_residue_price_units.to_le_bytes());
        row_index += 1;
    }

    Ok(CandidateCostCertificateV1 {
        economic_candidate_id: Id32::new(economics.economic_candidate_digest)
            .map_err(CandidateCostErrorV1::Codec)?,
        batch_policy_id: policy.policy_id,
        order_set_id: base.order_set(),
        price_semantics_id: Id32::new(price.semantic_price_digest)
            .map_err(CandidateCostErrorV1::Codec)?,
        outcome_count: domain.outcome_count,
        owner_count: u8::try_from(owner_count)
            .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?,
        filled_order_count,
        owner_net_risk_atoms,
        owner_net_cost_price_units,
        owner_net_cost_atoms: u64::try_from(owner_net_cost_atoms)
            .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?,
        execution_buy_price_units,
        execution_sell_price_units,
        terminal_rounding_residue_price_units,
        virtual_split_atoms: economics.virtual_split,
        virtual_merge_atoms: economics.virtual_merge,
        owner_transcript_id: Id32::new(owner_hash.finalize().into())
            .map_err(CandidateCostErrorV1::Codec)?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnerCostAccumulatorV1 {
    owner: Id32,
    contingent_delta_atoms: [i128; MAX_OUTCOMES],
    buy_price_units: u128,
    sell_price_units: u128,
    filled_order_count: u8,
}

impl OwnerCostAccumulatorV1 {
    const EMPTY: Self = Self {
        owner: Id32::ZERO,
        contingent_delta_atoms: [0; MAX_OUTCOMES],
        buy_price_units: 0,
        sell_price_units: 0,
        filled_order_count: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FinishedOwnerCostRowV1 {
    normalized_payoff_atoms: [u128; MAX_OUTCOMES],
    risk_atoms: u128,
    normalized_value_price_units: u128,
    cost_atoms: u64,
    rounding_residue_price_units: u128,
}

fn find_or_insert_owner(
    rows: &mut [OwnerCostAccumulatorV1; MAX_ORDERS],
    owner_count: &mut usize,
    owner: Id32,
) -> Result<usize, CandidateCostErrorV1> {
    if owner.is_zero() || *owner_count >= MAX_ORDERS {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    let mut at = 0usize;
    while at < *owner_count && rows[at].owner < owner {
        at += 1;
    }
    if at < *owner_count && rows[at].owner == owner {
        return Ok(at);
    }
    let mut cursor = *owner_count;
    while cursor > at {
        rows[cursor] = rows[cursor - 1];
        cursor -= 1;
    }
    rows[at] = OwnerCostAccumulatorV1 {
        owner,
        ..OwnerCostAccumulatorV1::EMPTY
    };
    *owner_count += 1;
    Ok(at)
}

fn finish_owner_row(
    row: &OwnerCostAccumulatorV1,
    prices: &[u64; MAX_OUTCOMES],
    outcomes: usize,
    price_scale: u64,
) -> Result<FinishedOwnerCostRowV1, CandidateCostErrorV1> {
    if row.owner.is_zero() || row.filled_order_count == 0 || !(2..=MAX_OUTCOMES).contains(&outcomes)
        || price_scale == 0
    {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    let mut minimum = row.contingent_delta_atoms[0];
    let mut maximum = minimum;
    let mut outcome = 1usize;
    while outcome < outcomes {
        minimum = core::cmp::min(minimum, row.contingent_delta_atoms[outcome]);
        maximum = core::cmp::max(maximum, row.contingent_delta_atoms[outcome]);
        outcome += 1;
    }
    let risk_atoms = u128::try_from(
        maximum
            .checked_sub(minimum)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
    )
    .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?;
    let mut normalized_payoff_atoms = [0u128; MAX_OUTCOMES];
    let mut normalized_value_price_units = 0u128;
    outcome = 0;
    while outcome < outcomes {
        let normalized = u128::try_from(
            row.contingent_delta_atoms[outcome]
                .checked_sub(minimum)
                .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
        )
        .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?;
        normalized_payoff_atoms[outcome] = normalized;
        normalized_value_price_units = normalized_value_price_units
            .checked_add(
                normalized
                    .checked_mul(u128::from(prices[outcome]))
                    .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    while outcome < MAX_OUTCOMES {
        if prices[outcome] != 0 || row.contingent_delta_atoms[outcome] != 0 {
            return Err(CandidateCostErrorV1::BindingMismatch);
        }
        outcome += 1;
    }
    let scale = u128::from(price_scale);
    let cost_atoms = u64::try_from(div_ceil(normalized_value_price_units, scale)?)
        .map_err(|_| CandidateCostErrorV1::ArithmeticOverflow)?;
    let buy_ceil = div_ceil(row.buy_price_units, scale)?;
    let buyer_residue = buy_ceil
        .checked_mul(scale)
        .and_then(|value| value.checked_sub(row.buy_price_units))
        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
    let seller_residue = row.sell_price_units % scale;
    let rounding_residue_price_units = buyer_residue
        .checked_add(seller_residue)
        .ok_or(CandidateCostErrorV1::ArithmeticOverflow)?;
    Ok(FinishedOwnerCostRowV1 {
        normalized_payoff_atoms,
        risk_atoms,
        normalized_value_price_units,
        cost_atoms,
        rounding_residue_price_units,
    })
}

fn div_ceil(numerator: u128, denominator: u128) -> Result<u128, CandidateCostErrorV1> {
    if denominator == 0 {
        return Err(CandidateCostErrorV1::BindingMismatch);
    }
    let quotient = numerator / denominator;
    if numerator % denominator == 0 {
        Ok(quotient)
    } else {
        quotient
            .checked_add(1)
            .ok_or(CandidateCostErrorV1::ArithmeticOverflow)
    }
}

fn put<const N: usize>(output: &mut [u8; N], at: &mut usize, value: &[u8]) {
    let end = *at + value.len();
    output[*at..end].copy_from_slice(value);
    *at = end;
}

fn hash_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    let mut index = 0usize;
    while index < parts.len() {
        hash.update(parts[index]);
        index += 1;
    }
    hash.finalize().into()
}

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(CANDIDATE_COST_CERTIFICATE_BYTES_V1 == 288);

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_POLICY_V1;

    fn owner_row(delta: [i128; MAX_OUTCOMES], buy: u128, sell: u128) -> OwnerCostAccumulatorV1 {
        OwnerCostAccumulatorV1 {
            owner: Id32::new([7; 32]).unwrap(),
            contingent_delta_atoms: delta,
            buy_price_units: buy,
            sell_price_units: sell,
            filled_order_count: 1,
        }
    }

    #[test]
    fn quotient_cost_is_invariant_to_risk_free_complete_set_translation() {
        let prices = [4, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let baseline = finish_owner_row(
            &owner_row([3, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 60, 0),
            &prices,
            2,
            10,
        )
        .unwrap();
        let translated = finish_owner_row(
            &owner_row([12, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 150, 0),
            &prices,
            2,
            10,
        )
        .unwrap();
        assert_eq!(baseline.risk_atoms, translated.risk_atoms);
        assert_eq!(baseline.normalized_payoff_atoms, translated.normalized_payoff_atoms);
        assert_eq!(baseline.normalized_value_price_units, translated.normalized_value_price_units);
        assert_eq!(baseline.cost_atoms, translated.cost_atoms);
        assert_eq!(baseline.rounding_residue_price_units, translated.rounding_residue_price_units);
    }

    #[test]
    fn same_owner_opposite_payoffs_net_before_costing() {
        let prices = [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let row = finish_owner_row(
            &owner_row([0; MAX_OUTCOMES], 50, 50),
            &prices,
            2,
            10,
        )
        .unwrap();
        assert_eq!(row.risk_atoms, 0);
        assert_eq!(row.normalized_value_price_units, 0);
        assert_eq!(row.cost_atoms, 0);
        assert_eq!(row.rounding_residue_price_units, 0);
    }

    #[test]
    fn incomplete_price_padding_is_refused() {
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 5;
        prices[1] = 5;
        prices[15] = 1;
        assert_eq!(
            finish_owner_row(&owner_row([1; MAX_OUTCOMES], 10, 0), &prices, 2, 10),
            Err(CandidateCostErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn canonical_owner_insertion_is_lexicographic_and_deduplicating() {
        let mut rows = [OwnerCostAccumulatorV1::EMPTY; MAX_ORDERS];
        let mut len = 0usize;
        let high = Id32::new([9; 32]).unwrap();
        let low = Id32::new([1; 32]).unwrap();
        assert_eq!(find_or_insert_owner(&mut rows, &mut len, high).unwrap(), 0);
        assert_eq!(find_or_insert_owner(&mut rows, &mut len, low).unwrap(), 0);
        assert_eq!(find_or_insert_owner(&mut rows, &mut len, high).unwrap(), 1);
        assert_eq!(len, 2);
        assert_eq!(rows[0].owner, low);
        assert_eq!(rows[1].owner, high);
    }

    #[test]
    fn hostile_policy_identity_substitution_is_refused() {
        assert_eq!(
            CandidateCostPolicyV1::bind(
                GENERAL_CLEARING_POLICY_V1,
                Id32::new([0x55; 32]).unwrap(),
            ),
            Err(CandidateCostErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn owner_range_overflow_is_an_explicit_refusal() {
        let mut delta = [0i128; MAX_OUTCOMES];
        delta[0] = i128::MIN;
        delta[1] = i128::MAX;
        let prices = [5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            finish_owner_row(&owner_row(delta, 0, 0), &prices, 2, 10),
            Err(CandidateCostErrorV1::ArithmeticOverflow)
        );
    }

    #[test]
    fn certificate_bytes_have_one_magic_version_and_zero_reserved_tail() {
        let certificate = CandidateCostCertificateV1 {
            economic_candidate_id: Id32::new([1; 32]).unwrap(),
            batch_policy_id: Id32::new([2; 32]).unwrap(),
            order_set_id: Id32::new([3; 32]).unwrap(),
            price_semantics_id: Id32::new([4; 32]).unwrap(),
            outcome_count: 2,
            owner_count: 1,
            filled_order_count: 2,
            owner_net_risk_atoms: 3,
            owner_net_cost_price_units: 4,
            owner_net_cost_atoms: 5,
            execution_buy_price_units: 6,
            execution_sell_price_units: 7,
            terminal_rounding_residue_price_units: 8,
            virtual_split_atoms: 9,
            virtual_merge_atoms: 0,
            owner_transcript_id: Id32::new([10; 32]).unwrap(),
        };
        let bytes = certificate.canonical_bytes();
        assert_eq!(&bytes[..8], &CERTIFICATE_MAGIC_V1);
        assert_eq!(bytes[8], CERTIFICATE_VERSION_V1);
        assert_eq!(&bytes[12..16], &[0; 4]);
        assert_eq!(&bytes[280..288], &[0; 8]);
        assert_eq!(certificate.content_id(), certificate.content_id());
    }
}
