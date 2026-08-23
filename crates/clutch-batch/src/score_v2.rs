//! Complete-set-quotiented candidate scoring.
//!
//! ScoreV2-Q ranks one already-valid submitted candidate by the model-free
//! range of its aggregate directly crossed outcome flow. It does not validate
//! orders, establish price quality, identify beneficial owners, or compensate
//! solvers. Those are separate relations and policies.
//!
//! The economic objective is
//!
//! ```text
//! d_i    = B_i - sigma = E_i - mu
//! rho(d) = max_i d_i - min_i d_i
//! ```
//!
//! `rho` is invariant to adding a constant complete-set layer, scales exactly
//! with quantity, and is zero exactly on constant flow. The later comparison
//! fields choose a canonical representation; they are not additional claims
//! about transferred risk.

use core::cmp::Ordering;

use crate::relation_v1::MAX_OUTCOMES;

/// Semantic version of the ScoreV2-Q arithmetic and comparison order.
pub const SCORE_V2_Q_VERSION: u8 = 2;

// The public fixed-array and u8 outcome-index contract is deliberately pinned.
const _: () = assert!(MAX_OUTCOMES == 16);

/// The normalization contract applied before ScoreV2 sees candidate flow.
///
/// Only [`Self::OwnerBlindAggregate`] supports a representation-neutral score.
/// The owner-tagged variants are named so a caller translating a V1 policy
/// must do so explicitly; ScoreV2 refuses them because changing public-key
/// labels can change the admitted flow before scoring.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizationPolicyV2 {
    /// Admission and aggregation do not branch on an owner/public-key label.
    OwnerBlindAggregate,
    /// V1 N-a: refuse a same-owner, same-outcome overlap.
    OwnerTaggedRefuseOverlap,
    /// V1 N-b: cancel a same-owner, same-outcome overlap at admission.
    OwnerTaggedNetAtAdmission,
    /// V1 N-c: retain owner tags until the pairing feasibility gate.
    OwnerTaggedGateAtPairing,
}

impl NormalizationPolicyV2 {
    /// Whether relabeling otherwise identical public keys cannot change the
    /// normalization rule.
    pub const fn is_representation_neutral(self) -> bool {
        matches!(self, Self::OwnerBlindAggregate)
    }
}

/// A fixed-array field whose inactive padding was nonzero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowFieldV2 {
    /// Aggregate executed buy atoms, `B_i`.
    AggregateBuy,
    /// Aggregate executed sell atoms, `E_i`.
    AggregateSell,
    /// Candidate-claimed direct flow, `d_i`.
    ClaimedDirect,
}

/// Inputs required to re-derive one candidate's ScoreV2-Q rank.
///
/// These are candidate *deltas*, not persisted balances. Every active
/// coordinate is checked from both sides of the relation. Inactive cells must
/// be canonical zero padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateDeltaV2 {
    /// Explicit pre-score normalization contract.
    pub normalization_policy: NormalizationPolicyV2,
    /// Active prefix of each fixed flow array; must be in `2..=16`.
    pub outcome_count: u8,
    /// `B_i`: aggregate executed buy atoms.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// `E_i`: aggregate executed sell atoms.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
    /// Claimed `d_i`; verified against both aggregate sides.
    pub claimed_direct_flow: [u64; MAX_OUTCOMES],
    /// `sigma`: complete sets created by the virtual split.
    pub virtual_split: u64,
    /// `mu`: complete sets destroyed by the virtual merge.
    pub virtual_merge: u64,
    /// Full candidate identity used only for the final deterministic tie.
    pub candidate_digest: [u8; 32],
}

/// The economic prefix of ScoreV2-Q.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiskObjectiveV2 {
    /// `max_i(d_i) - min_i(d_i)`, maximized.
    pub certified_risk_flow_atoms: u64,
}

impl RiskObjectiveV2 {
    /// Compare only the economic objective.
    pub fn total_order(&self, other: &Self) -> Ordering {
        self.certified_risk_flow_atoms
            .cmp(&other.certified_risk_flow_atoms)
    }

    /// Whether this objective outranks another objective.
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.total_order(other) == Ordering::Greater
    }
}

/// The exact total ScoreV2-Q comparison key.
///
/// Comparison directions are frozen and named:
///
/// 1. maximize certified quotient-risk flow;
/// 2. minimize directly crossed complete-set flow;
/// 3. minimize virtual split/merge churn; and
/// 4. prefer the lexicographically smaller full candidate digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreV2 {
    /// The sole economic objective.
    pub risk: RiskObjectiveV2,
    /// `min_i(d_i)`, minimized to select the min-zero representative.
    pub cash_equivalent_direct_flow_atoms: u64,
    /// `sigma + mu`, minimized after direct complete-set flow.
    pub virtual_churn_atoms: u64,
    /// Full candidate identity; lexicographically smaller is preferred.
    pub digest: [u8; 32],
}

impl ScoreV2 {
    /// Frozen total ordering. `Greater` means `self` is preferred.
    pub fn total_order(&self, other: &Self) -> Ordering {
        match self.risk.total_order(&other.risk) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match other
            .cash_equivalent_direct_flow_atoms
            .cmp(&self.cash_equivalent_direct_flow_atoms)
        {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match other.virtual_churn_atoms.cmp(&self.virtual_churn_atoms) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        // Smaller digest wins, matching the frozen V1 tie direction.
        other.digest.cmp(&self.digest)
    }

    /// Whether this score outranks another score.
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.total_order(other) == Ordering::Greater
    }
}

/// Immutable economic domain of one checked ScoreV2-Q certificate.
///
/// Identities are opaque semantic digests supplied by a caller such as
/// RelationV2. This kernel compares and retains them but does not authenticate
/// an account, recompute their hashes, or infer a market from flow bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreDomainV2 {
    market_semantics_digest: [u8; 32],
    epoch_semantics_digest: [u8; 32],
    relation_policy_digest: [u8; 32],
    outcome_count: u8,
}

impl ScoreDomainV2 {
    /// Validate and capture the exact Market/epoch/policy/width binding.
    pub fn new(
        market_semantics_digest: [u8; 32],
        epoch_semantics_digest: [u8; 32],
        relation_policy_digest: [u8; 32],
        outcome_count: u8,
    ) -> Result<Self, ScoreErrorV2> {
        if is_zero_digest(&market_semantics_digest)
            || is_zero_digest(&epoch_semantics_digest)
            || is_zero_digest(&relation_policy_digest)
        {
            return Err(ScoreErrorV2::ZeroBindingIdentity);
        }
        if !(2..=MAX_OUTCOMES).contains(&usize::from(outcome_count)) {
            return Err(ScoreErrorV2::InvalidOutcomeCount);
        }
        Ok(Self {
            market_semantics_digest,
            epoch_semantics_digest,
            relation_policy_digest,
            outcome_count,
        })
    }

    /// Immutable Market semantic identity.
    pub const fn market_semantics_digest(&self) -> [u8; 32] {
        self.market_semantics_digest
    }

    /// Immutable recurring epoch semantic identity.
    pub const fn epoch_semantics_digest(&self) -> [u8; 32] {
        self.epoch_semantics_digest
    }

    /// Frozen RelationV2 policy identity under which flow was admitted.
    pub const fn relation_policy_digest(&self) -> [u8; 32] {
        self.relation_policy_digest
    }

    /// Number of active native Egg coordinates.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }
}

/// Private-field proof that exact owner-blind flow produced one ScoreV2-Q key.
///
/// The certificate is not candidate-admission, price-quality, settlement, or
/// execution authority. RelationV2 is responsible for deriving its flow from
/// a valid submitted candidate before invoking this kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCandidateScoreV2 {
    domain: ScoreDomainV2,
    candidate_delta: CandidateDeltaV2,
    direct_flow: [u64; MAX_OUTCOMES],
    score: ScoreV2,
}

impl CheckedCandidateScoreV2 {
    /// Exact Market/epoch/policy/width binding checked at construction.
    pub const fn domain(&self) -> ScoreDomainV2 {
        self.domain
    }

    /// Exact canonical aggregate flow, conversion, and candidate identity
    /// from which the certificate was independently derived.
    pub const fn candidate_delta(&self) -> &CandidateDeltaV2 {
        &self.candidate_delta
    }

    /// Independently re-derived active direct flow with canonical padding.
    pub const fn direct_flow(&self) -> &[u64; MAX_OUTCOMES] {
        &self.direct_flow
    }

    /// Exact total ScoreV2-Q comparison key.
    pub const fn score(&self) -> &ScoreV2 {
        &self.score
    }

    /// Compare two checked keys only when every domain binding is identical.
    /// `Greater` means `self` is the preferred valid submitted candidate.
    pub fn total_order_same_domain(
        &self,
        other: &Self,
    ) -> Result<Ordering, ScoreErrorV2> {
        if self.domain != other.domain {
            return Err(ScoreErrorV2::MismatchedScoreDomain);
        }
        Ok(self.score.total_order(&other.score))
    }
}

/// Result of admitting one more checked candidate to a bounded selection fold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SelectionUpdateV2 {
    /// The incoming checked candidate became the retained best.
    ReplacedBest = 0,
    /// The existing best remained preferred or exactly equal.
    RetainedBest = 1,
}

/// Bounded fold retaining the best submitted checked ScoreV2-Q certificate.
///
/// The fold begins from one checked certificate, never an artificial sentinel.
/// Equal keys retain the earlier submission; distinct candidate-digest bytes
/// make the score order total without treating collision resistance as a
/// theorem. This fold does not itself certify RelationV2 candidate validity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BestSubmittedScoreV2 {
    best: CheckedCandidateScoreV2,
    checked_submission_count: u64,
}

impl BestSubmittedScoreV2 {
    /// Begin a selection fold from the first already-checked candidate.
    pub const fn begin(first: CheckedCandidateScoreV2) -> Self {
        Self {
            best: first,
            checked_submission_count: 1,
        }
    }

    /// Consider one more checked certificate in the exact same score domain.
    ///
    /// State changes only after the domain and count checks succeed.
    pub fn consider(
        &mut self,
        incoming: CheckedCandidateScoreV2,
    ) -> Result<SelectionUpdateV2, ScoreErrorV2> {
        let order = incoming.total_order_same_domain(&self.best)?;
        let next_count = self
            .checked_submission_count
            .checked_add(1)
            .ok_or(ScoreErrorV2::CheckedSubmissionCountOverflow)?;
        let update = if order == Ordering::Greater {
            self.best = incoming;
            SelectionUpdateV2::ReplacedBest
        } else {
            SelectionUpdateV2::RetainedBest
        };
        self.checked_submission_count = next_count;
        Ok(update)
    }

    /// Retained best submitted checked score certificate.
    pub const fn best(&self) -> &CheckedCandidateScoreV2 {
        &self.best
    }

    /// Number of checked score submissions folded so far.
    pub const fn checked_submission_count(&self) -> u64 {
        self.checked_submission_count
    }

    /// Force the private counter to a boundary value in crate tests.
    #[cfg(test)]
    pub(crate) fn set_checked_submission_count_for_test(&mut self, count: u64) {
        self.checked_submission_count = count;
    }
}

/// Every refusal produced while deriving ScoreV2-Q.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreErrorV2 {
    /// Active outcome width was outside `2..=16`.
    InvalidOutcomeCount,
    /// The caller selected an owner-tag-dependent normalization contract.
    NormalizationNotRepresentationNeutral,
    /// An inactive fixed-array cell was nonzero.
    NonCanonicalPadding {
        /// Aggregate or claimed vector containing the nonzero padding cell.
        field: FlowFieldV2,
        /// First inactive coordinate observed to be nonzero.
        outcome: u8,
    },
    /// Both virtual split and virtual merge were nonzero.
    NonCanonicalVirtualConversion,
    /// `sigma` exceeded `B_i` at this active outcome.
    VirtualSplitExceedsBuyFlow {
        /// Active coordinate at which subtraction would underflow.
        outcome: u8,
    },
    /// `mu` exceeded `E_i` at this active outcome.
    VirtualMergeExceedsSellFlow {
        /// Active coordinate at which subtraction would underflow.
        outcome: u8,
    },
    /// A checked exact aggregate or churn calculation exceeded `u64`.
    ArithmeticOverflow {
        /// Active coordinate, or `u8::MAX` for scalar churn overflow.
        outcome: u8,
    },
    /// `B_i + mu != E_i + sigma` or the two derived direct flows differed.
    OutcomeConservationMismatch {
        /// First active coordinate violating the two-sided equality.
        outcome: u8,
    },
    /// The candidate's claimed `d_i` did not equal the independently derived
    /// direct flow.
    DirectFlowMismatch {
        /// First active coordinate disagreeing with recomputation.
        outcome: u8,
    },
    /// A claimed total score did not equal the independently derived score.
    ScoreMismatch,
    /// A required Market, epoch, or relation-policy identity was all zero.
    ZeroBindingIdentity,
    /// Score-domain width and exact flow width differed.
    ScoreDomainWidthMismatch,
    /// Checked candidate scores came from different immutable score domains.
    MismatchedScoreDomain,
    /// The checked score-submission counter exceeded its fixed width.
    CheckedSubmissionCountOverflow,
}

/// Re-derive the canonical direct-flow vector from both sides of a candidate.
///
/// Refusal order is policy, width, padding, virtual-conversion canonicality,
/// then active coordinates in ascending order. Public callers may rely on this
/// order when building deterministic refusal fixtures.
pub fn derive_direct_flow_v2(
    candidate: &CandidateDeltaV2,
) -> Result<[u64; MAX_OUTCOMES], ScoreErrorV2> {
    if !candidate.normalization_policy.is_representation_neutral() {
        return Err(ScoreErrorV2::NormalizationNotRepresentationNeutral);
    }
    let outcomes = usize::from(candidate.outcome_count);
    if !(2..=MAX_OUTCOMES).contains(&outcomes) {
        return Err(ScoreErrorV2::InvalidOutcomeCount);
    }
    validate_padding(
        &candidate.aggregate_buy_flow,
        candidate.outcome_count,
        FlowFieldV2::AggregateBuy,
    )?;
    validate_padding(
        &candidate.aggregate_sell_flow,
        candidate.outcome_count,
        FlowFieldV2::AggregateSell,
    )?;
    validate_padding(
        &candidate.claimed_direct_flow,
        candidate.outcome_count,
        FlowFieldV2::ClaimedDirect,
    )?;
    if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
        return Err(ScoreErrorV2::NonCanonicalVirtualConversion);
    }

    let mut direct_flow = [0u64; MAX_OUTCOMES];
    let mut outcome = 0u8;
    while usize::from(outcome) < outcomes {
        let index = usize::from(outcome);
        let buy_direct = candidate.aggregate_buy_flow[index]
            .checked_sub(candidate.virtual_split)
            .ok_or(ScoreErrorV2::VirtualSplitExceedsBuyFlow { outcome })?;
        let sell_direct = candidate.aggregate_sell_flow[index]
            .checked_sub(candidate.virtual_merge)
            .ok_or(ScoreErrorV2::VirtualMergeExceedsSellFlow { outcome })?;
        let total_from_buy = candidate.aggregate_buy_flow[index]
            .checked_add(candidate.virtual_merge)
            .ok_or(ScoreErrorV2::ArithmeticOverflow { outcome })?;
        let total_from_sell = candidate.aggregate_sell_flow[index]
            .checked_add(candidate.virtual_split)
            .ok_or(ScoreErrorV2::ArithmeticOverflow { outcome })?;
        if total_from_buy != total_from_sell || buy_direct != sell_direct {
            return Err(ScoreErrorV2::OutcomeConservationMismatch { outcome });
        }
        if candidate.claimed_direct_flow[index] != buy_direct {
            return Err(ScoreErrorV2::DirectFlowMismatch { outcome });
        }
        direct_flow[index] = buy_direct;
        outcome += 1;
    }
    Ok(direct_flow)
}

/// Recompute the exact ScoreV2-Q total key of one candidate.
pub fn score_candidate_v2(candidate: &CandidateDeltaV2) -> Result<ScoreV2, ScoreErrorV2> {
    let direct_flow = derive_direct_flow_v2(candidate)?;
    score_from_direct_flow_v2(candidate, &direct_flow)
}

/// Bind exact owner-blind flow and its independently recomputed key into one
/// private-field candidate score certificate.
pub fn certify_candidate_score_v2(
    domain: ScoreDomainV2,
    candidate: &CandidateDeltaV2,
) -> Result<CheckedCandidateScoreV2, ScoreErrorV2> {
    if domain.outcome_count != candidate.outcome_count {
        return Err(ScoreErrorV2::ScoreDomainWidthMismatch);
    }
    let direct_flow = derive_direct_flow_v2(candidate)?;
    let score = score_from_direct_flow_v2(candidate, &direct_flow)?;
    Ok(CheckedCandidateScoreV2 {
        domain,
        candidate_delta: *candidate,
        direct_flow,
        score,
    })
}

fn score_from_direct_flow_v2(
    candidate: &CandidateDeltaV2,
    direct_flow: &[u64; MAX_OUTCOMES],
) -> Result<ScoreV2, ScoreErrorV2> {
    let outcomes = usize::from(candidate.outcome_count);
    let mut lowest = direct_flow[0];
    let mut highest = direct_flow[0];
    let mut outcome = 1usize;
    while outcome < outcomes {
        let value = direct_flow[outcome];
        if value < lowest {
            lowest = value;
        }
        if value > highest {
            highest = value;
        }
        outcome += 1;
    }
    let churn = candidate
        .virtual_split
        .checked_add(candidate.virtual_merge)
        .ok_or(ScoreErrorV2::ArithmeticOverflow { outcome: u8::MAX })?;
    Ok(ScoreV2 {
        risk: RiskObjectiveV2 {
            // `lowest <= highest` by construction, so this subtraction is total.
            certified_risk_flow_atoms: highest - lowest,
        },
        cash_equivalent_direct_flow_atoms: lowest,
        virtual_churn_atoms: churn,
        digest: candidate.candidate_digest,
    })
}

/// Recompute and compare a candidate's claimed ScoreV2-Q key.
pub fn verify_candidate_score_v2(
    candidate: &CandidateDeltaV2,
    claimed_score: &ScoreV2,
) -> Result<ScoreV2, ScoreErrorV2> {
    let recomputed = score_candidate_v2(candidate)?;
    if recomputed != *claimed_score {
        return Err(ScoreErrorV2::ScoreMismatch);
    }
    Ok(recomputed)
}

fn validate_padding(
    flow: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
    field: FlowFieldV2,
) -> Result<(), ScoreErrorV2> {
    let mut outcome = outcome_count;
    while usize::from(outcome) < MAX_OUTCOMES {
        if flow[usize::from(outcome)] != 0 {
            return Err(ScoreErrorV2::NonCanonicalPadding { field, outcome });
        }
        outcome += 1;
    }
    Ok(())
}

fn is_zero_digest(value: &[u8; 32]) -> bool {
    let mut byte = 0usize;
    while byte < value.len() {
        if value[byte] != 0 {
            return false;
        }
        byte += 1;
    }
    true
}
