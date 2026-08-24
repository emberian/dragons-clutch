// SPDX-License-Identifier: AGPL-3.0-or-later

//! Same-tag Window/AdmissionNode successors for the 96-byte cost-aware rank.

use crate::{
    encode_score_v2_q_cost_first_admitted_tie_v1, AdmissionNodeStatusV1,
    AdmissionNodeV3AccountV1, CandidateWindowV4AccountV1, CodecError, FirstAdmittedTieV1, Id32,
    MarketBindingV2, ScoreV2QCostComponentsV1, ADMISSION_NODE_ACCOUNT_BYTES,
    ADMISSION_NODE_ACCOUNT_BYTES_V2, ADMISSION_NODE_ACCOUNT_TAG, ADMISSION_NODE_ACCOUNT_VERSION,
    ADMISSION_NODE_ACCOUNT_VERSION_V2, ID_BYTES, SCORE_V2_Q_ACTIVE_RANK_BYTES,
    SCORE_V2_Q_COST_ACTIVE_RANK_BYTES, SCORE_V2_Q_RANK_CAPACITY, WINDOW_ACCOUNT_BYTES,
    WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION, WINDOW_ACCOUNT_VERSION_V2,
};

const WINDOW_RANK_OFFSET: usize = 2 + (5 * 32) + (6 * 8) + (4 * 32);
const WINDOW_RANK_LEN_OFFSET: usize = WINDOW_ACCOUNT_BYTES - 3;
const NODE_RANK_OFFSET: usize = 2 + (16 * 32);
const NODE_RANK_LEN_OFFSET: usize = ADMISSION_NODE_ACCOUNT_BYTES - 5;

/// Candidate Window successor with the full 96-byte cost-aware rank active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateWindowV5AccountV1 {
    base: CandidateWindowV4AccountV1,
}

impl CandidateWindowV5AccountV1 {
    /// Construct from the complete existing fact set with `rank_key_len=96`.
    pub fn new(base: CandidateWindowV4AccountV1) -> Result<Self, CodecError> {
        let value = Self { base };
        value.validate()?;
        Ok(value)
    }

    /// Complete Window fact set; only its rank semantics are V5.
    pub const fn base(&self) -> &CandidateWindowV4AccountV1 {
        &self.base
    }

    /// Validate schedule/count geometry through V4 and the breaking rank layout
    /// through the cost-aware candidate/ordinal coordinates.
    pub fn validate(&self) -> Result<(), CodecError> {
        if usize::from(self.base.rank_key_len) != SCORE_V2_Q_COST_ACTIVE_RANK_BYTES {
            return Err(CodecError::InvalidState);
        }
        let projection = window_v4_projection(self.base)?;
        projection.validate()?;
        if self.base.finalized_slot == 0 && self.base.valid_verdict_count != 0 {
            validate_cost_rank_candidate_and_ordinal(
                self.base.best_rank_key,
                self.base.best_settlement_candidate_id,
                self.base.best_ordinal,
            )?;
        }
        Ok(())
    }

    /// Encode the exact V5 version at the existing Window account width.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != WINDOW_ACCOUNT_BYTES {
            return Err(CodecError::WrongLength);
        }
        let mut bytes = [0u8; WINDOW_ACCOUNT_BYTES];
        window_v4_projection(self.base)?.encode(&mut bytes)?;
        bytes[1] = WINDOW_ACCOUNT_VERSION_V2;
        bytes[WINDOW_RANK_OFFSET..WINDOW_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
            .copy_from_slice(&self.base.best_rank_key);
        bytes[WINDOW_RANK_LEN_OFFSET] = self.base.rank_key_len;
        output.copy_from_slice(&bytes);
        Ok(())
    }

    /// Decode only tag 24/version 5 with the full cost-aware rank layout.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != WINDOW_ACCOUNT_BYTES {
            return Err(CodecError::WrongLength);
        }
        if input[0] != WINDOW_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != WINDOW_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        let cost_rank: [u8; SCORE_V2_Q_RANK_CAPACITY] = input
            [WINDOW_RANK_OFFSET..WINDOW_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?;
        let mut bytes: [u8; WINDOW_ACCOUNT_BYTES] =
            input.try_into().map_err(|_| CodecError::WrongLength)?;
        bytes[1] = WINDOW_ACCOUNT_VERSION;
        bytes[WINDOW_RANK_OFFSET..WINDOW_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
            .copy_from_slice(&cost_rank_to_v1(cost_rank));
        bytes[WINDOW_RANK_LEN_OFFSET] = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
        let mut base = CandidateWindowV4AccountV1::decode(&bytes)?;
        base.best_rank_key = cost_rank;
        base.rank_key_len = SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8;
        Self::new(base)
    }
}

/// AdmissionNode successor carrying the checked cost-certificate content ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionNodeV4AccountV1 {
    base: AdmissionNodeV3AccountV1,
    cost_certificate_id: Id32,
}

impl AdmissionNodeV4AccountV1 {
    /// Construct one V2 outer account. Only `VerifiedValid` carries a live
    /// certificate ID and a 96-byte rank; every other state carries neither.
    pub fn new(
        base: AdmissionNodeV3AccountV1,
        cost_certificate_id: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            base,
            cost_certificate_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Complete AdmissionNode fact set; only rank semantics are V4.
    pub const fn base(&self) -> &AdmissionNodeV3AccountV1 {
        &self.base
    }

    /// Canonical checked cost-certificate content ID, or absent before/refusal.
    pub const fn cost_certificate_id(&self) -> Id32 {
        self.cost_certificate_id
    }

    /// Validate the status-dependent certificate and rank without weakening
    /// any V3 lifecycle, funding, rent, or identity invariant.
    pub fn validate(&self) -> Result<(), CodecError> {
        let valid = self.base.status == AdmissionNodeStatusV1::VerifiedValid;
        if valid {
            if usize::from(self.base.rank_key_len) != SCORE_V2_Q_COST_ACTIVE_RANK_BYTES
                || self.cost_certificate_id.is_zero()
            {
                return Err(CodecError::InvalidState);
            }
            validate_cost_rank_candidate_and_ordinal(
                self.base.rank_key,
                self.base.settlement_candidate_id,
                self.base.ordinal,
            )?;
        } else if self.base.rank_key_len != 0
            || self.base.rank_key != [0; SCORE_V2_Q_RANK_CAPACITY]
            || !self.cost_certificate_id.is_zero()
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        node_v3_projection(self.base)?.validate()
    }

    /// Encode tag `0x77`, version 2, the exact V3 semantic prefix, then the
    /// checked certificate ID.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != ADMISSION_NODE_ACCOUNT_BYTES_V2 {
            return Err(CodecError::WrongLength);
        }
        let mut prefix = [0u8; ADMISSION_NODE_ACCOUNT_BYTES];
        node_v3_projection(self.base)?.encode(&mut prefix)?;
        prefix[1] = ADMISSION_NODE_ACCOUNT_VERSION_V2;
        prefix[NODE_RANK_OFFSET..NODE_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
            .copy_from_slice(&self.base.rank_key);
        prefix[NODE_RANK_LEN_OFFSET] = self.base.rank_key_len;
        output[..ADMISSION_NODE_ACCOUNT_BYTES].copy_from_slice(&prefix);
        output[ADMISSION_NODE_ACCOUNT_BYTES..].copy_from_slice(&self.cost_certificate_id.bytes());
        Ok(())
    }

    /// Decode only the exact V2 outer width and certificate state.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != ADMISSION_NODE_ACCOUNT_BYTES_V2 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != ADMISSION_NODE_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != ADMISSION_NODE_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        let cost_rank: [u8; SCORE_V2_Q_RANK_CAPACITY] = input
            [NODE_RANK_OFFSET..NODE_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?;
        let mut prefix: [u8; ADMISSION_NODE_ACCOUNT_BYTES] = input
            [..ADMISSION_NODE_ACCOUNT_BYTES]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?;
        prefix[1] = ADMISSION_NODE_ACCOUNT_VERSION;
        let valid = prefix[ADMISSION_NODE_ACCOUNT_BYTES - 3]
            == AdmissionNodeStatusV1::VerifiedValid as u8;
        if valid {
            prefix[NODE_RANK_OFFSET..NODE_RANK_OFFSET + SCORE_V2_Q_RANK_CAPACITY]
                .copy_from_slice(&cost_rank_to_v1(cost_rank));
            prefix[NODE_RANK_LEN_OFFSET] = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
        }
        let mut base = AdmissionNodeV3AccountV1::decode(&prefix)?;
        if valid {
            base.rank_key = cost_rank;
            base.rank_key_len = SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8;
        }
        let cost_certificate_id = Id32::from_bytes(
            input[ADMISSION_NODE_ACCOUNT_BYTES..]
                .try_into()
                .map_err(|_| CodecError::WrongLength)?,
        );
        Self::new(base, cost_certificate_id)
    }
}

/// Forgeable projection admitted only from the private runtime action-14
/// certificate wrapper in the same adapter invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostedCandidateVerdictProjectionV1 {
    /// Exact cost-aware rank inputs.
    pub components: ScoreV2QCostComponentsV1,
    /// Canonical checked certificate content identity.
    pub certificate_id: Id32,
    /// Exact independently rederived rank bytes.
    pub rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

/// Pure state inputs for the action-14 cost-aware winner transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteCostedCandidateRankTransitionV1<'a> {
    /// Current authenticated Clock slot.
    pub current_slot: u64,
    /// Private-runtime projection; public here only across the crate boundary.
    pub verdict: CostedCandidateVerdictProjectionV1,
    /// Prestate cost-aware Window.
    pub window: &'a CandidateWindowV5AccountV1,
    /// Prestate revealed cost-aware AdmissionNode.
    pub node: &'a AdmissionNodeV4AccountV1,
    /// Immutable batch-policy-owning MarketBinding.
    pub market: &'a MarketBindingV2,
}

/// Node/Window poststate to compose atomically with Work V3 terminalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteCostedCandidateRankPoststateV1 {
    /// Window after verdict count and possible best replacement.
    pub window: CandidateWindowV5AccountV1,
    /// Terminal valid Node with its checked certificate ID.
    pub node: AdmissionNodeV4AccountV1,
}

/// Apply the cost-aware portion of action 14 after a private verifier has
/// terminalized Work V3. This function authorizes no reward or Work mutation
/// by itself; the live adapter must compose both transitions atomically.
pub fn complete_costed_candidate_rank_poststate_v1(
    request: CompleteCostedCandidateRankTransitionV1<'_>,
) -> Result<CompleteCostedCandidateRankPoststateV1, CodecError> {
    request.market.validate()?;
    request.window.validate()?;
    request.node.validate()?;
    let window_before = *request.window.base();
    let node_before = *request.node.base();
    if node_before.status != AdmissionNodeStatusV1::Revealed
        || !request.node.cost_certificate_id().is_zero()
        || request.verdict.certificate_id.is_zero()
        || request.verdict.components.score.settlement_candidate_id
            != node_before.settlement_candidate_id
        || node_before.market != request.market.base().market
        || window_before.market != request.market.base().market
        || node_before.relation_policy_id != request.market.base().relation_policy_id
        || window_before.relation_policy_id != request.market.base().relation_policy_id
        || node_before.admission_policy_id != request.market.base().admission_policy_id
        || window_before.admission_policy_id != request.market.base().admission_policy_id
        || node_before.score_policy_id != request.market.base().score_policy_id
        || window_before.score_policy_id != request.market.base().score_policy_id
        || node_before.epoch != window_before.epoch
        || node_before.epoch_generation != window_before.epoch_generation
        || request.current_slot < window_before.submission_closes_slot
        || request.current_slot >= window_before.verification_closes_slot
    {
        return Err(CodecError::MismatchedBinding);
    }
    let expected_rank = encode_score_v2_q_cost_first_admitted_tie_v1(
        request.verdict.components,
        FirstAdmittedTieV1 {
            ordinal: node_before.ordinal,
        },
    )?;
    if expected_rank != request.verdict.rank_key {
        return Err(CodecError::MismatchedBinding);
    }
    let mut node_after = node_before;
    node_after.rank_key = expected_rank;
    node_after.rank_key_len = SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8;
    node_after.status = AdmissionNodeStatusV1::VerifiedValid;
    node_after.terminal_slot = request.current_slot;
    let node = AdmissionNodeV4AccountV1::new(node_after, request.verdict.certificate_id)?;

    let mut window_after = window_before;
    window_after.verdict_count = window_after
        .verdict_count
        .checked_add(1)
        .ok_or(CodecError::ArithmeticOverflow)?;
    window_after.valid_verdict_count = window_after
        .valid_verdict_count
        .checked_add(1)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if window_after.best_candidate_node.is_zero() || expected_rank > window_after.best_rank_key {
        window_after.best_candidate_node = node_after.node;
        window_after.best_settlement_candidate_id = node_after.settlement_candidate_id;
        window_after.best_rank_key = expected_rank;
        window_after.best_ordinal = node_after.ordinal;
    }
    let window = CandidateWindowV5AccountV1::new(window_after)?;
    Ok(CompleteCostedCandidateRankPoststateV1 { window, node })
}

fn window_v4_projection(
    mut window: CandidateWindowV4AccountV1,
) -> Result<CandidateWindowV4AccountV1, CodecError> {
    window.rank_key_len = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
    window.best_rank_key = cost_rank_to_v1(window.best_rank_key);
    Ok(window)
}

fn node_v3_projection(
    mut node: AdmissionNodeV3AccountV1,
) -> Result<AdmissionNodeV3AccountV1, CodecError> {
    if node.status == AdmissionNodeStatusV1::VerifiedValid {
        node.rank_key_len = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
        node.rank_key = cost_rank_to_v1(node.rank_key);
    }
    Ok(node)
}

fn cost_rank_to_v1(
    cost_rank: [u8; SCORE_V2_Q_RANK_CAPACITY],
) -> [u8; SCORE_V2_Q_RANK_CAPACITY] {
    let mut rank = [0u8; SCORE_V2_Q_RANK_CAPACITY];
    rank[..24].copy_from_slice(&cost_rank[..24]);
    rank[24..56].copy_from_slice(&cost_rank[32..64]);
    rank[56..88].copy_from_slice(&cost_rank[64..96]);
    rank
}

fn validate_cost_rank_candidate_and_ordinal(
    rank: [u8; SCORE_V2_Q_RANK_CAPACITY],
    candidate: Id32,
    ordinal: u64,
) -> Result<(), CodecError> {
    if candidate.is_zero() || ordinal == 0 {
        return Err(CodecError::InvalidState);
    }
    let candidate_bytes = candidate.bytes();
    let ordinal_bytes = FirstAdmittedTieV1 { ordinal }.coordinate()?;
    let mut index = 0usize;
    while index < ID_BYTES {
        if rank[32 + index] != !candidate_bytes[index]
            || rank[64 + index] != !ordinal_bytes[index]
        {
            return Err(CodecError::MismatchedBinding);
        }
        index += 1;
    }
    Ok(())
}

const _: () = assert!(WINDOW_RANK_OFFSET == 338);
const _: () = assert!(WINDOW_RANK_LEN_OFFSET == 562);
const _: () = assert!(NODE_RANK_OFFSET == 514);
const _: () = assert!(NODE_RANK_LEN_OFFSET == 738);
const _: () = assert!(ADMISSION_NODE_ACCOUNT_BYTES_V2 == ADMISSION_NODE_ACCOUNT_BYTES + 32);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        DeletableRentOwnerV1, MarketBindingV1, SettlementCandidateKindV1,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    };

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer: id(250),
            refundable_principal: 10,
            donation_floor: 1,
        }
    }

    fn market() -> MarketBindingV2 {
        MarketBindingV2::new(
            MarketBindingV1 {
                market: id(2),
                market_genesis_profile_v2_id: id(20),
                market_instance_v2_id: id(21),
                series_plan_v5_id: id(22),
                series_funding_terms_v2_id: id(23),
                relation_policy_id: id(3),
                price_measure_policy_v1_id: id(24),
                native_claim_basis_id: id(25),
                admission_policy_id: id(5),
                score_policy_id: id(6),
                settlement_policy_id: id(26),
                neutral_sink: id(27),
                price_scale: 100,
                commit_span_slots: 10,
                reveal_span_slots: 10,
                verification_span_slots: 20,
                bond_lamports: 100,
                invalidity_penalty: 10,
                abandonment_penalty: 10,
                node_cleanup_reward: 1,
                price_check_reward: 1,
                order_reward: 1,
                slice_reward: 1,
                completion_reward: 1,
                work_close_reward: 1,
                feed_close_reward: 1,
                freeze_reward: 1,
                finalize_reward: 1,
                solver_prize: 1,
                root_close_reward: 1,
                relation_version: 2,
                outcome_count: 3,
                basis_degree: 2,
                rank_key_len: 96,
                candidate_kind_mask: 1,
                stored_bump: 1,
                flags: 0,
            },
            id(28),
        )
        .unwrap()
    }

    pub(crate) fn window() -> CandidateWindowV5AccountV1 {
        CandidateWindowV5AccountV1::new(CandidateWindowV4AccountV1 {
            epoch: id(1),
            market: id(2),
            relation_policy_id: id(3),
            admission_policy_id: id(5),
            score_policy_id: id(6),
            freeze_deadline_slot: 10,
            frozen_slot: 10,
            reveal_opens_slot: 20,
            submission_closes_slot: 30,
            verification_closes_slot: 50,
            finalized_slot: 0,
            admission_head: id(4),
            best_candidate_node: Id32::ZERO,
            best_settlement_candidate_id: Id32::ZERO,
            selected_candidate_artifact: Id32::ZERO,
            best_rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
            admitted_count: 1,
            revealed_count: 1,
            verdict_count: 0,
            valid_verdict_count: 0,
            expired_commitment_count: 0,
            expired_unverified_count: 0,
            live_node_count: 1,
            closed_node_count: 0,
            best_ordinal: 0,
            epoch_generation: 1,
            rent: rent(),
            rank_key_len: 96,
            stored_bump: 2,
            flags: 0,
        })
        .unwrap()
    }

    fn revealed_node() -> AdmissionNodeV4AccountV1 {
        AdmissionNodeV4AccountV1::new(
            AdmissionNodeV3AccountV1 {
                epoch: id(1),
                market: id(2),
                relation_policy_id: id(3),
                node: id(4),
                previous_node: Id32::ZERO,
                admission_policy_id: id(5),
                score_policy_id: id(6),
                commitment: id(7),
                submitter_authority: id(8),
                solver_reward_destination: id(9),
                payer: id(10),
                refund_destination: id(11),
                candidate_bundle_digest: id(13),
                settlement_candidate_id: id(12),
                base_relation_candidate_id: id(12),
                settlement_witness_digest: id(14),
                rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
                epoch_generation: 1,
                ordinal: 1,
                committed_slot: 11,
                window_frozen_slot: 10,
                revealed_slot: 25,
                terminal_slot: 0,
                rent: rent(),
                bond_lamports: 100,
                cleanup_reward: 1,
                work_escrow_lamports: 0,
                work_funding_initial: 20,
                rank_key_len: 0,
                candidate_kind: SettlementCandidateKindV1::Direct,
                status: AdmissionNodeStatusV1::Revealed,
                stored_bump: 3,
                flags: 0,
            },
            Id32::ZERO,
        )
        .unwrap()
    }

    #[test]
    fn action14_persists_only_the_private_certificate_id_and_full_rank() {
        let market = market();
        let window = window();
        let node = revealed_node();
        let components = ScoreV2QCostComponentsV1 {
            score: crate::ScoreV2QComponentsV1 {
                certified_risk_flow_atoms: 7,
                cash_equivalent_direct_flow_atoms: 2,
                virtual_churn_atoms: 1,
                settlement_candidate_id: id(12),
            },
            owner_net_cost_atoms: 4,
        };
        let rank = encode_score_v2_q_cost_first_admitted_tie_v1(
            components,
            FirstAdmittedTieV1 { ordinal: 1 },
        )
        .unwrap();
        let post = complete_costed_candidate_rank_poststate_v1(
            CompleteCostedCandidateRankTransitionV1 {
                current_slot: 35,
                verdict: CostedCandidateVerdictProjectionV1 {
                    components,
                    certificate_id: id(29),
                    rank_key: rank,
                },
                window: &window,
                node: &node,
                market: &market,
            },
        )
        .unwrap();
        assert_eq!(post.node.cost_certificate_id(), id(29));
        assert_eq!(post.node.base().rank_key, rank);
        assert_eq!(post.window.base().best_rank_key, rank);
        assert_eq!(post.window.base().valid_verdict_count, 1);

        let mut window_bytes = [0u8; WINDOW_ACCOUNT_BYTES];
        post.window.encode(&mut window_bytes).unwrap();
        assert_eq!(CandidateWindowV5AccountV1::decode(&window_bytes), Ok(post.window));
        let mut node_bytes = [0u8; ADMISSION_NODE_ACCOUNT_BYTES_V2];
        post.node.encode(&mut node_bytes).unwrap();
        assert_eq!(AdmissionNodeV4AccountV1::decode(&node_bytes), Ok(post.node));
        let mut market_bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        market.encode(&mut market_bytes).unwrap();
        assert_eq!(MarketBindingV2::decode(&market_bytes), Ok(market));
    }

    #[test]
    fn forged_rank_or_premature_certificate_is_refused() {
        let market = market();
        let window = window();
        let node = revealed_node();
        let components = ScoreV2QCostComponentsV1 {
            score: crate::ScoreV2QComponentsV1 {
                certified_risk_flow_atoms: 1,
                cash_equivalent_direct_flow_atoms: 1,
                virtual_churn_atoms: 1,
                settlement_candidate_id: id(12),
            },
            owner_net_cost_atoms: 1,
        };
        assert_eq!(
            complete_costed_candidate_rank_poststate_v1(
                CompleteCostedCandidateRankTransitionV1 {
                    current_slot: 35,
                    verdict: CostedCandidateVerdictProjectionV1 {
                        components,
                        certificate_id: id(29),
                        rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
                    },
                    window: &window,
                    node: &node,
                    market: &market,
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            AdmissionNodeV4AccountV1::new(*node.base(), id(29)),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
