// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{CodecError, Id32};

/// Active bytes in `ScoreV2QFirstAdmittedTieV1`.
pub const SCORE_V2_Q_ACTIVE_RANK_BYTES: usize = 88;
/// Fixed account capacity; the final eight bytes are canonical zero padding.
pub const SCORE_V2_Q_RANK_CAPACITY: usize = 96;
/// Active bytes in the owner-net cost-aware ScoreV2-Q successor.
pub const SCORE_V2_Q_COST_ACTIVE_RANK_BYTES: usize = 96;

/// Exact ScoreV2-Q economic fields before canonical byte-order conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreV2QComponentsV1 {
    /// Maximize quotient-risk flow.
    pub certified_risk_flow_atoms: u64,
    /// Minimize direct complete-set-equivalent flow.
    pub cash_equivalent_direct_flow_atoms: u64,
    /// Minimize virtual split/merge churn.
    pub virtual_churn_atoms: u64,
    /// Typed final candidate identity. Direct uses verified RelationV2;
    /// CoveredDealer uses the checked dealer-economic candidate identity.
    pub settlement_candidate_id: Id32,
}

/// Exact ScoreV2-Q fields plus the preselection owner-net cost coordinate.
///
/// `owner_net_cost_atoms` is derived from authenticated frozen-order ownership,
/// exact RelationV2 fills and the immutable batch policy. It is not a fee,
/// volume-quality claim, identity score, or optimality certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreV2QCostComponentsV1 {
    /// Existing ScoreV2-Q coordinates, unchanged.
    pub score: ScoreV2QComponentsV1,
    /// Minimize complete-set-quotiented owner capital at the named boundary.
    pub owner_net_cost_atoms: u64,
}

/// Non-grindable final tie within an exact duplicate final candidate.
///
/// The Window assigns `ordinal` atomically. The canonical 32-byte coordinate
/// is 24 zero bytes followed by its big-endian ordinal. No submitter-selected
/// secret, authority, commitment, or node address enters this tie.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirstAdmittedTieV1 {
    /// One-based admission ordinal.
    pub ordinal: u64,
}

impl FirstAdmittedTieV1 {
    /// Encode the sortable 32-byte coordinate.
    pub fn coordinate(self) -> Result<[u8; 32], CodecError> {
        if self.ordinal == 0 {
            return Err(CodecError::InvalidState);
        }
        let mut out = [0u8; 32];
        let bytes = self.ordinal.to_be_bytes();
        let mut index = 0usize;
        while index < 8 {
            out[24 + index] = bytes[index];
            index += 1;
        }
        Ok(out)
    }
}

/// Encode the canonical descending 88-byte rank into a zero-padded 96-byte
/// container.
///
/// Greater lexicographic bytes win. Risk is direct big-endian; every minimized
/// field is complemented. The middle identity preserves ScoreV2's frozen full
/// digest tie, now typed as the final settlement candidate. The last identity
/// prefers the first admitted exact duplicate without using a grindable node.
pub fn encode_score_v2_q_first_admitted_tie_v1(
    score: ScoreV2QComponentsV1,
    tie: FirstAdmittedTieV1,
) -> Result<[u8; SCORE_V2_Q_RANK_CAPACITY], CodecError> {
    if score.settlement_candidate_id.is_zero() {
        return Err(CodecError::ZeroIdentity);
    }
    let mut out = [0u8; SCORE_V2_Q_RANK_CAPACITY];
    out[0..8].copy_from_slice(&score.certified_risk_flow_atoms.to_be_bytes());
    complement_into(
        &mut out[8..16],
        &score.cash_equivalent_direct_flow_atoms.to_be_bytes(),
    );
    complement_into(&mut out[16..24], &score.virtual_churn_atoms.to_be_bytes());
    complement_into(&mut out[24..56], &score.settlement_candidate_id.bytes());
    complement_into(&mut out[56..88], &tie.coordinate()?);
    Ok(out)
}

/// Encode the breaking cost-aware ScoreV2-Q successor.
///
/// Greater lexicographic bytes still win. The existing three economic
/// coordinates retain their exact order. The complemented owner-net cost is
/// inserted before candidate identity and admission ordinal, so it is only a
/// deterministic cost tie-break among candidates equal on ScoreV2-Q proper.
pub fn encode_score_v2_q_cost_first_admitted_tie_v1(
    components: ScoreV2QCostComponentsV1,
    tie: FirstAdmittedTieV1,
) -> Result<[u8; SCORE_V2_Q_RANK_CAPACITY], CodecError> {
    if components.score.settlement_candidate_id.is_zero() {
        return Err(CodecError::ZeroIdentity);
    }
    let mut out = [0u8; SCORE_V2_Q_RANK_CAPACITY];
    out[0..8].copy_from_slice(
        &components
            .score
            .certified_risk_flow_atoms
            .to_be_bytes(),
    );
    complement_into(
        &mut out[8..16],
        &components
            .score
            .cash_equivalent_direct_flow_atoms
            .to_be_bytes(),
    );
    complement_into(
        &mut out[16..24],
        &components.score.virtual_churn_atoms.to_be_bytes(),
    );
    complement_into(
        &mut out[24..32],
        &components.owner_net_cost_atoms.to_be_bytes(),
    );
    complement_into(
        &mut out[32..64],
        &components.score.settlement_candidate_id.bytes(),
    );
    complement_into(&mut out[64..96], &tie.coordinate()?);
    Ok(out)
}

fn complement_into(output: &mut [u8], input: &[u8]) {
    let mut index = 0usize;
    while index < output.len() {
        output[index] = !input[index];
        index += 1;
    }
}

const _: () = assert!(SCORE_V2_Q_ACTIVE_RANK_BYTES == 8 + 8 + 8 + 32 + 32);
const _: () = assert!(SCORE_V2_Q_ACTIVE_RANK_BYTES < SCORE_V2_Q_RANK_CAPACITY);
const _: () = assert!(SCORE_V2_Q_COST_ACTIVE_RANK_BYTES == 8 + 8 + 8 + 8 + 32 + 32);
const _: () = assert!(SCORE_V2_Q_COST_ACTIVE_RANK_BYTES == SCORE_V2_Q_RANK_CAPACITY);

#[cfg(test)]
mod tests {
    use super::*;

    fn score(candidate: u8) -> ScoreV2QComponentsV1 {
        ScoreV2QComponentsV1 {
            certified_risk_flow_atoms: 9,
            cash_equivalent_direct_flow_atoms: 4,
            virtual_churn_atoms: 2,
            settlement_candidate_id: Id32::new([candidate; 32]).unwrap(),
        }
    }

    #[test]
    fn lower_owner_net_cost_wins_before_candidate_identity() {
        let tie = FirstAdmittedTieV1 { ordinal: 1 };
        let cheap = encode_score_v2_q_cost_first_admitted_tie_v1(
            ScoreV2QCostComponentsV1 {
                score: score(0xff),
                owner_net_cost_atoms: 3,
            },
            tie,
        )
        .unwrap();
        let expensive = encode_score_v2_q_cost_first_admitted_tie_v1(
            ScoreV2QCostComponentsV1 {
                score: score(1),
                owner_net_cost_atoms: 4,
            },
            tie,
        )
        .unwrap();
        assert!(cheap > expensive);
    }

    #[test]
    fn cost_successor_has_no_unauthenticated_padding_coordinate() {
        let rank = encode_score_v2_q_cost_first_admitted_tie_v1(
            ScoreV2QCostComponentsV1 {
                score: score(7),
                owner_net_cost_atoms: 11,
            },
            FirstAdmittedTieV1 { ordinal: 13 },
        )
        .unwrap();
        assert_eq!(&rank[24..32], &(!11u64).to_be_bytes());
        assert_ne!(&rank[88..96], &[0; 8]);
    }
}
