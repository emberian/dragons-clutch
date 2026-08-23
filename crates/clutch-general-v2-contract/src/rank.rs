// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{CodecError, Id32};

/// Active bytes in `ScoreV2QFirstAdmittedTieV1`.
pub const SCORE_V2_Q_ACTIVE_RANK_BYTES: usize = 88;
/// Fixed account capacity; the final eight bytes are canonical zero padding.
pub const SCORE_V2_Q_RANK_CAPACITY: usize = 96;

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

fn complement_into(output: &mut [u8], input: &[u8]) {
    let mut index = 0usize;
    while index < output.len() {
        output[index] = !input[index];
        index += 1;
    }
}

const _: () = assert!(SCORE_V2_Q_ACTIVE_RANK_BYTES == 8 + 8 + 8 + 32 + 32);
const _: () = assert!(SCORE_V2_Q_ACTIVE_RANK_BYTES < SCORE_V2_Q_RANK_CAPACITY);
