// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hostile-byte codecs for reusable Failure Market interval accounts.
//!
//! `0xab/v2` is a once-capitalized reusable cell. Session archive resets it to
//! canonical Idle without moving rent. `0xac/v2` is the sole append-only,
//! counted Market interval history. Neither account holds liveness work
//! capital, and neither v2 codec accepts the withdrawn one-shot v1 layouts.

use crate::{is_zero, registry, CodecError, Result, HASH_BYTES};

/// Exact Product structural interval-work body width.
pub const PRODUCT_INTERVAL_WORK_BODY_BYTES_V2: usize = 592;
const CELL_AMOUNT_COUNT_V2: usize = 7;
const CELL_ID_COUNT_V2: usize = 13;
const CELL_RESERVED_BYTES_V2: usize = 20;
const HISTORY_AMOUNT_COUNT_V2: usize = 6;
const HISTORY_ID_COUNT_V2: usize = 12;
const HISTORY_RESERVED_BYTES_V2: usize = 76;

const _: () = assert!(
    4 + CELL_AMOUNT_COUNT_V2 * 8
        + CELL_ID_COUNT_V2 * HASH_BYTES
        + PRODUCT_INTERVAL_WORK_BODY_BYTES_V2
        + CELL_RESERVED_BYTES_V2
        == registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES
);
const _: () = assert!(
    4 + HISTORY_AMOUNT_COUNT_V2 * 8 + HISTORY_ID_COUNT_V2 * HASH_BYTES + HISTORY_RESERVED_BYTES_V2
        == registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES
);

/// Reusable cell lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketIntervalCellPhaseV2 {
    /// No session is pinned; every session field and Product byte is zero.
    Idle = 1,
    /// One exclusively pinned session may advance.
    Active = 2,
    /// The session is terminal and must be appended before reset.
    Resolved = 3,
}

impl FailureMarketIntervalCellPhaseV2 {
    fn byte(self) -> u8 {
        match self {
            Self::Idle => 1,
            Self::Active => 2,
            Self::Resolved => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Idle),
            2 => Ok(Self::Active),
            3 => Ok(Self::Resolved),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Complete fixed 1,088-byte reusable `0xab/v2` cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellAccountV2 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Current reusable-cell phase.
    pub phase: FailureMarketIntervalCellPhaseV2,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Exact Product-prepaid cell Rent principal.
    pub work_rent_principal_lamports: u64,
    /// Sessions already folded into `0xac/v2`.
    pub completed_session_count: u64,
    /// Current-session bounded transition nonce.
    pub transition_nonce: u64,
    /// Current-session cumulative accepted progress.
    pub accepted_progress_units: u64,
    /// Current-session paid work-call count.
    pub completed_work_calls: u64,
    /// Current-session exact keeper rewards.
    pub exact_reward_lamports: u64,
    /// Immutable shared Failure policy.
    pub failure_policy_binding_id: [u8; HASH_BYTES],
    /// Full-width shared economic Market.
    pub market_instance_id: [u8; HASH_BYTES],
    /// Product private foundation-step/funding receipt.
    pub funding_receipt_id: [u8; HASH_BYTES],
    /// Canonical append-only `0xac/v2` account.
    pub history_account: [u8; HASH_BYTES],
    /// Immutable Rent-principal recipient.
    pub rent_refund_owner: [u8; HASH_BYTES],
    /// Immutable donation sink.
    pub neutral_sink: [u8; HASH_BYTES],
    /// Current subordinate session binding; zero only while Idle.
    pub session_binding_id: [u8; HASH_BYTES],
    /// Exact Source-owned successful/refused handoff; zero only while Idle.
    pub source_handoff_id: [u8; HASH_BYTES],
    /// Per-session absolute recovery schedule; zero only while Idle.
    pub session_schedule_id: [u8; HASH_BYTES],
    /// Authenticated Market quote admission; zero only while Idle.
    pub quote_admission_receipt_id: [u8; HASH_BYTES],
    /// Last local transition receipt; zero before the first paid advance.
    pub last_transition_receipt_id: [u8; HASH_BYTES],
    /// Last exact liveness work receipt; zero before the first paid advance.
    pub last_liveness_work_receipt_id: [u8; HASH_BYTES],
    /// Private session resolution/terminal receipt; zero until Resolved.
    pub resolution_receipt_id: [u8; HASH_BYTES],
    /// Exact canonical Product structural-work bytes; all zero while Idle.
    pub product_work_body: [u8; PRODUCT_INTERVAL_WORK_BODY_BYTES_V2],
}

impl FailureMarketIntervalCellAccountV2 {
    /// Encode every field and zero every reserved byte.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION;
        output[2] = self.bump;
        output[3] = self.phase.byte();
        let mut cursor = 4usize;
        for value in [
            self.generation,
            self.work_rent_principal_lamports,
            self.completed_session_count,
            self.transition_nonce,
            self.accepted_progress_units,
            self.completed_work_calls,
            self.exact_reward_lamports,
        ] {
            put_u64(output, &mut cursor, value);
        }
        for id in [
            self.failure_policy_binding_id,
            self.market_instance_id,
            self.funding_receipt_id,
            self.history_account,
            self.rent_refund_owner,
            self.neutral_sink,
            self.session_binding_id,
            self.source_handoff_id,
            self.session_schedule_id,
            self.quote_admission_receipt_id,
            self.last_transition_receipt_id,
            self.last_liveness_work_receipt_id,
            self.resolution_receipt_id,
        ] {
            put_id(output, &mut cursor, id);
        }
        output[cursor..cursor + PRODUCT_INTERVAL_WORK_BODY_BYTES_V2]
            .copy_from_slice(&self.product_work_body);
        cursor += PRODUCT_INTERVAL_WORK_BODY_BYTES_V2;
        debug_assert_eq!(
            cursor + CELL_RESERVED_BYTES_V2,
            registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES
        );
        Ok(())
    }

    /// Hostile-decode one exact v2 cell; withdrawn v1 bytes are refused.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<Self> {
        if input[0] != registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let phase = FailureMarketIntervalCellPhaseV2::decode(input[3])?;
        let mut cursor = 4usize;
        let generation = take_u64(input, &mut cursor);
        let work_rent_principal_lamports = take_u64(input, &mut cursor);
        let completed_session_count = take_u64(input, &mut cursor);
        let transition_nonce = take_u64(input, &mut cursor);
        let accepted_progress_units = take_u64(input, &mut cursor);
        let completed_work_calls = take_u64(input, &mut cursor);
        let exact_reward_lamports = take_u64(input, &mut cursor);
        let failure_policy_binding_id = take_id(input, &mut cursor);
        let market_instance_id = take_id(input, &mut cursor);
        let funding_receipt_id = take_id(input, &mut cursor);
        let history_account = take_id(input, &mut cursor);
        let rent_refund_owner = take_id(input, &mut cursor);
        let neutral_sink = take_id(input, &mut cursor);
        let session_binding_id = take_id(input, &mut cursor);
        let source_handoff_id = take_id(input, &mut cursor);
        let session_schedule_id = take_id(input, &mut cursor);
        let quote_admission_receipt_id = take_id(input, &mut cursor);
        let last_transition_receipt_id = take_id(input, &mut cursor);
        let last_liveness_work_receipt_id = take_id(input, &mut cursor);
        let resolution_receipt_id = take_id(input, &mut cursor);
        let mut product_work_body = [0; PRODUCT_INTERVAL_WORK_BODY_BYTES_V2];
        product_work_body
            .copy_from_slice(&input[cursor..cursor + PRODUCT_INTERVAL_WORK_BODY_BYTES_V2]);
        cursor += PRODUCT_INTERVAL_WORK_BODY_BYTES_V2;
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let value = Self {
            bump: input[2],
            phase,
            generation,
            work_rent_principal_lamports,
            completed_session_count,
            transition_nonce,
            accepted_progress_units,
            completed_work_calls,
            exact_reward_lamports,
            failure_policy_binding_id,
            market_instance_id,
            funding_receipt_id,
            history_account,
            rent_refund_owner,
            neutral_sink,
            session_binding_id,
            source_handoff_id,
            session_schedule_id,
            quote_admission_receipt_id,
            last_transition_receipt_id,
            last_liveness_work_receipt_id,
            resolution_receipt_id,
            product_work_body,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for id in [
            self.failure_policy_binding_id,
            self.market_instance_id,
            self.funding_receipt_id,
            self.history_account,
            self.rent_refund_owner,
            self.neutral_sink,
        ] {
            require_live(id)?;
        }
        if self.generation == 0
            || self.work_rent_principal_lamports == 0
            || self.history_account == self.rent_refund_owner
            || self.history_account == self.neutral_sink
            || self.rent_refund_owner == self.neutral_sink
        {
            return Err(CodecError::ZeroValue);
        }
        let session_prefix = [
            self.session_binding_id,
            self.source_handoff_id,
            self.session_schedule_id,
            self.quote_admission_receipt_id,
        ];
        let advanced = self.transition_nonce != 0;
        let complete_advance = !is_zero(&self.last_transition_receipt_id)
            && !is_zero(&self.last_liveness_work_receipt_id)
            && self.accepted_progress_units != 0
            && self.completed_work_calls != 0
            && self.exact_reward_lamports != 0;
        let any_advance = !is_zero(&self.last_transition_receipt_id)
            || !is_zero(&self.last_liveness_work_receipt_id)
            || self.accepted_progress_units != 0
            || self.completed_work_calls != 0
            || self.exact_reward_lamports != 0;
        if (advanced && !complete_advance) || (!advanced && any_advance) {
            return Err(CodecError::InvalidEnum);
        }
        match self.phase {
            FailureMarketIntervalCellPhaseV2::Idle => {
                if session_prefix.iter().any(|id| !is_zero(id))
                    || advanced
                    || !is_zero(&self.resolution_receipt_id)
                    || self.product_work_body.iter().any(|byte| *byte != 0)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureMarketIntervalCellPhaseV2::Active => {
                if session_prefix.iter().any(|id| is_zero(id))
                    || !is_zero(&self.resolution_receipt_id)
                    || self.product_work_body.iter().all(|byte| *byte == 0)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureMarketIntervalCellPhaseV2::Resolved => {
                if session_prefix.iter().any(|id| is_zero(id))
                    || is_zero(&self.resolution_receipt_id)
                    || self.product_work_body.iter().all(|byte| *byte == 0)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
        }
        Ok(())
    }
}

/// Complete fixed 512-byte append-only `0xac/v2` history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalHistoryAccountV2 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Exact reusable-cell Rent principal.
    pub work_rent_principal_lamports: u64,
    /// Exact permanent-history Rent principal.
    pub history_rent_principal_lamports: u64,
    /// Number of terminal sessions folded into the root.
    pub completed_session_count: u64,
    /// Aggregate paid Recovery calls across all sessions.
    pub completed_work_calls: u64,
    /// Aggregate exact keeper rewards across all sessions.
    pub exact_reward_lamports: u64,
    /// Immutable shared Failure policy.
    pub failure_policy_binding_id: [u8; HASH_BYTES],
    /// Full-width shared economic Market.
    pub market_instance_id: [u8; HASH_BYTES],
    /// Product private foundation-step/funding receipt.
    pub funding_receipt_id: [u8; HASH_BYTES],
    /// Canonical reusable `0xab/v2` cell.
    pub work_account: [u8; HASH_BYTES],
    /// Immutable Rent-principal recipient.
    pub rent_refund_owner: [u8; HASH_BYTES],
    /// Immutable donation sink.
    pub neutral_sink: [u8; HASH_BYTES],
    /// Exact authenticated Market quote admission.
    pub quote_admission_receipt_id: [u8; HASH_BYTES],
    /// Sole append-only root, zero only while empty.
    pub history_root: [u8; HASH_BYTES],
    /// Latest folded session binding, zero only while empty.
    pub latest_session_binding_id: [u8; HASH_BYTES],
    /// Latest folded terminal receipt, zero only while empty.
    pub latest_terminal_receipt_id: [u8; HASH_BYTES],
    /// Latest terminal reusable-cell postimage, zero only while empty.
    pub latest_terminal_state_commitment: [u8; HASH_BYTES],
    /// Exhaustive Failure-family receipt, zero until history is sealed.
    pub family_terminal_receipt_id: [u8; HASH_BYTES],
}

impl FailureMarketIntervalHistoryAccountV2 {
    /// Encode every history byte and zero all reserved space.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION;
        output[2] = self.bump;
        let mut cursor = 4usize;
        for value in [
            self.generation,
            self.work_rent_principal_lamports,
            self.history_rent_principal_lamports,
            self.completed_session_count,
            self.completed_work_calls,
            self.exact_reward_lamports,
        ] {
            put_u64(output, &mut cursor, value);
        }
        for id in [
            self.failure_policy_binding_id,
            self.market_instance_id,
            self.funding_receipt_id,
            self.work_account,
            self.rent_refund_owner,
            self.neutral_sink,
            self.quote_admission_receipt_id,
            self.history_root,
            self.latest_session_binding_id,
            self.latest_terminal_receipt_id,
            self.latest_terminal_state_commitment,
            self.family_terminal_receipt_id,
        ] {
            put_id(output, &mut cursor, id);
        }
        debug_assert_eq!(
            cursor + HISTORY_RESERVED_BYTES_V2,
            registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES
        );
        Ok(())
    }

    /// Hostile-decode one exact v2 history; withdrawn v1 bytes are refused.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<Self> {
        if input[0] != registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        if input[3] != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        let mut cursor = 4usize;
        let generation = take_u64(input, &mut cursor);
        let work_rent_principal_lamports = take_u64(input, &mut cursor);
        let history_rent_principal_lamports = take_u64(input, &mut cursor);
        let completed_session_count = take_u64(input, &mut cursor);
        let completed_work_calls = take_u64(input, &mut cursor);
        let exact_reward_lamports = take_u64(input, &mut cursor);
        let failure_policy_binding_id = take_id(input, &mut cursor);
        let market_instance_id = take_id(input, &mut cursor);
        let funding_receipt_id = take_id(input, &mut cursor);
        let work_account = take_id(input, &mut cursor);
        let rent_refund_owner = take_id(input, &mut cursor);
        let neutral_sink = take_id(input, &mut cursor);
        let quote_admission_receipt_id = take_id(input, &mut cursor);
        let history_root = take_id(input, &mut cursor);
        let latest_session_binding_id = take_id(input, &mut cursor);
        let latest_terminal_receipt_id = take_id(input, &mut cursor);
        let latest_terminal_state_commitment = take_id(input, &mut cursor);
        let family_terminal_receipt_id = take_id(input, &mut cursor);
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let value = Self {
            bump: input[2],
            generation,
            work_rent_principal_lamports,
            history_rent_principal_lamports,
            completed_session_count,
            completed_work_calls,
            exact_reward_lamports,
            failure_policy_binding_id,
            market_instance_id,
            funding_receipt_id,
            work_account,
            rent_refund_owner,
            neutral_sink,
            quote_admission_receipt_id,
            history_root,
            latest_session_binding_id,
            latest_terminal_receipt_id,
            latest_terminal_state_commitment,
            family_terminal_receipt_id,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for id in [
            self.failure_policy_binding_id,
            self.market_instance_id,
            self.funding_receipt_id,
            self.work_account,
            self.rent_refund_owner,
            self.neutral_sink,
            self.quote_admission_receipt_id,
        ] {
            require_live(id)?;
        }
        if self.generation == 0
            || self.work_rent_principal_lamports == 0
            || self.history_rent_principal_lamports == 0
            || self.work_account == self.rent_refund_owner
            || self.work_account == self.neutral_sink
            || self.rent_refund_owner == self.neutral_sink
        {
            return Err(CodecError::ZeroValue);
        }
        let history_fields = [
            self.history_root,
            self.latest_session_binding_id,
            self.latest_terminal_receipt_id,
            self.latest_terminal_state_commitment,
        ];
        if self.completed_session_count == 0 {
            if history_fields.iter().any(|id| !is_zero(id))
                || self.completed_work_calls != 0
                || self.exact_reward_lamports != 0
            {
                return Err(CodecError::InvalidEnum);
            }
        } else if history_fields.iter().any(|id| is_zero(id)) {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }
}

fn require_live(id: [u8; HASH_BYTES]) -> Result<()> {
    if is_zero(&id) {
        Err(CodecError::ZeroValue)
    } else {
        Ok(())
    }
}

fn put_u64(output: &mut [u8], cursor: &mut usize, value: u64) {
    output[*cursor..*cursor + 8].copy_from_slice(&value.to_le_bytes());
    *cursor += 8;
}

fn take_u64(input: &[u8], cursor: &mut usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&input[*cursor..*cursor + 8]);
    *cursor += 8;
    u64::from_le_bytes(bytes)
}

fn put_id(output: &mut [u8], cursor: &mut usize, value: [u8; HASH_BYTES]) {
    output[*cursor..*cursor + HASH_BYTES].copy_from_slice(&value);
    *cursor += HASH_BYTES;
}

fn take_id(input: &[u8], cursor: &mut usize) -> [u8; HASH_BYTES] {
    let mut value = [0; HASH_BYTES];
    value.copy_from_slice(&input[*cursor..*cursor + HASH_BYTES]);
    *cursor += HASH_BYTES;
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_cell() -> FailureMarketIntervalCellAccountV2 {
        FailureMarketIntervalCellAccountV2 {
            bump: 7,
            phase: FailureMarketIntervalCellPhaseV2::Idle,
            generation: 3,
            work_rent_principal_lamports: 100,
            completed_session_count: 0,
            transition_nonce: 0,
            accepted_progress_units: 0,
            completed_work_calls: 0,
            exact_reward_lamports: 0,
            failure_policy_binding_id: [1; HASH_BYTES],
            market_instance_id: [2; HASH_BYTES],
            funding_receipt_id: [3; HASH_BYTES],
            history_account: [4; HASH_BYTES],
            rent_refund_owner: [5; HASH_BYTES],
            neutral_sink: [6; HASH_BYTES],
            session_binding_id: [0; HASH_BYTES],
            source_handoff_id: [0; HASH_BYTES],
            session_schedule_id: [0; HASH_BYTES],
            quote_admission_receipt_id: [0; HASH_BYTES],
            last_transition_receipt_id: [0; HASH_BYTES],
            last_liveness_work_receipt_id: [0; HASH_BYTES],
            resolution_receipt_id: [0; HASH_BYTES],
            product_work_body: [0; PRODUCT_INTERVAL_WORK_BODY_BYTES_V2],
        }
    }

    #[test]
    fn idle_cell_refuses_hidden_session_or_v1_version() {
        let idle = idle_cell();
        let mut encoded = [0; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES];
        idle.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketIntervalCellAccountV2::decode(&encoded),
            Ok(idle)
        );
        encoded[1] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_V1_VERSION;
        assert_eq!(
            FailureMarketIntervalCellAccountV2::decode(&encoded),
            Err(CodecError::WrongVersion)
        );
        let mut hidden = idle;
        hidden.session_binding_id = [9; HASH_BYTES];
        assert_eq!(
            hidden.encode_into(&mut encoded),
            Err(CodecError::InvalidEnum)
        );

        let mut partial_advance = idle;
        partial_advance.phase = FailureMarketIntervalCellPhaseV2::Active;
        partial_advance.session_binding_id = [10; HASH_BYTES];
        partial_advance.source_handoff_id = [11; HASH_BYTES];
        partial_advance.session_schedule_id = [12; HASH_BYTES];
        partial_advance.quote_admission_receipt_id = [13; HASH_BYTES];
        partial_advance.product_work_body[0] = 1;
        partial_advance.accepted_progress_units = 1;
        assert_eq!(
            partial_advance.encode_into(&mut encoded),
            Err(CodecError::InvalidEnum)
        );

        let mut zero_work_terminal = partial_advance;
        zero_work_terminal.phase = FailureMarketIntervalCellPhaseV2::Resolved;
        zero_work_terminal.accepted_progress_units = 0;
        zero_work_terminal.resolution_receipt_id = [14; HASH_BYTES];
        zero_work_terminal.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketIntervalCellAccountV2::decode(&encoded),
            Ok(zero_work_terminal)
        );
    }

    #[test]
    fn empty_history_rejects_partial_root_and_reserved_bytes() {
        let history = FailureMarketIntervalHistoryAccountV2 {
            bump: 8,
            generation: 3,
            work_rent_principal_lamports: 100,
            history_rent_principal_lamports: 200,
            completed_session_count: 0,
            completed_work_calls: 0,
            exact_reward_lamports: 0,
            failure_policy_binding_id: [1; HASH_BYTES],
            market_instance_id: [2; HASH_BYTES],
            funding_receipt_id: [3; HASH_BYTES],
            work_account: [4; HASH_BYTES],
            rent_refund_owner: [5; HASH_BYTES],
            neutral_sink: [6; HASH_BYTES],
            quote_admission_receipt_id: [7; HASH_BYTES],
            history_root: [0; HASH_BYTES],
            latest_session_binding_id: [0; HASH_BYTES],
            latest_terminal_receipt_id: [0; HASH_BYTES],
            latest_terminal_state_commitment: [0; HASH_BYTES],
            family_terminal_receipt_id: [0; HASH_BYTES],
        };
        let mut encoded = [0; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES];
        history.encode_into(&mut encoded).unwrap();
        encoded[registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES - 1] = 1;
        assert_eq!(
            FailureMarketIntervalHistoryAccountV2::decode(&encoded),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut partial = history;
        partial.history_root = [8; HASH_BYTES];
        assert_eq!(
            partial.encode_into(&mut encoded),
            Err(CodecError::InvalidEnum)
        );
    }
}
