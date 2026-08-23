// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hostile-byte codecs for Recovery78 interval-consensus accounts.
//!
//! The mutable `0xab/v1` account persists the exact 592-byte Product work body
//! plus the minimum Failure/liveness/rent chain needed to authenticate every
//! transition. The permanent `0xac/v1` account persists replay and terminal
//! facts; it is never a work-capital or refund source.

use crate::{is_zero, registry, CodecError, Result, HASH_BYTES};

/// Exact Product structural interval-work body width.
pub const PRODUCT_INTERVAL_WORK_BODY_BYTES_V1: usize = 592;
const WORK_RESERVED_BYTES_V1: usize = 132;
const REPLAY_RESERVED_BYTES_V1: usize = 4;

const _: () = assert!(
    4 + 5 * 8 + 10 * HASH_BYTES + PRODUCT_INTERVAL_WORK_BODY_BYTES_V1 + WORK_RESERVED_BYTES_V1
        == registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES
);
const _: () = assert!(
    4 + 3 * 8 + 12 * HASH_BYTES + REPLAY_RESERVED_BYTES_V1
        == registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES
);

/// Persisted interval lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureIntervalConsensusPhaseV1 {
    /// Bounded Product work may still advance or be resolved.
    Active = 1,
    /// Product's private certificate capability resolved Failure.
    Resolved = 2,
    /// Mutable work was closed; permanent replay remains.
    Closed = 3,
}

impl FailureIntervalConsensusPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Active),
            2 => Ok(Self::Resolved),
            3 => Ok(Self::Closed),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Complete fixed `0xab/v1` mutable work-account body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusWorkAccountV1 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Current lifecycle phase; Closed is an authenticated close preimage only.
    pub phase: FailureIntervalConsensusPhaseV1,
    /// Exact Failure/liveness generation.
    pub generation: u64,
    /// Monotone bounded-transition nonce.
    pub transition_nonce: u64,
    /// Cumulative semantic recovery progress including this interval chain.
    pub accepted_recovery_progress_total: u64,
    /// Exact refundable work-account rent principal.
    pub work_rent_principal_lamports: u64,
    /// Exact permanent replay-account rent principal.
    pub replay_rent_principal_lamports: u64,
    /// Immutable interval lifecycle identity.
    pub interval_binding_id: [u8; HASH_BYTES],
    /// Immutable parent Failure policy binding.
    pub failure_policy_binding_id: [u8; HASH_BYTES],
    /// Present-funding admission receipt.
    pub funding_receipt_id: [u8; HASH_BYTES],
    /// Canonical permanent `0xac/v1` account.
    pub replay_account: [u8; HASH_BYTES],
    /// Immutable rent-principal refund recipient.
    pub rent_payer: [u8; HASH_BYTES],
    /// Immutable donation sink.
    pub neutral_sink: [u8; HASH_BYTES],
    /// Last bounded Failure transition receipt, zero only before first advance.
    pub last_transition_receipt_id: [u8; HASH_BYTES],
    /// Last liveness work receipt, zero only before first advance.
    pub last_liveness_receipt_id: [u8; HASH_BYTES],
    /// Failure interval resolution receipt, zero until Resolved.
    pub resolution_receipt_id: [u8; HASH_BYTES],
    /// Exact work-account close authorization, zero until close.
    pub close_authorization_id: [u8; HASH_BYTES],
    /// Exact canonical Product structural-work bytes.
    pub product_work_body: [u8; PRODUCT_INTERVAL_WORK_BODY_BYTES_V1],
}

impl FailureIntervalConsensusWorkAccountV1 {
    /// Encode every byte and zero all reserved space.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION;
        output[2] = self.bump;
        output[3] = self.phase as u8;
        let mut cursor = 4;
        put_u64(output, &mut cursor, self.generation);
        put_u64(output, &mut cursor, self.transition_nonce);
        put_u64(output, &mut cursor, self.accepted_recovery_progress_total);
        put_u64(output, &mut cursor, self.work_rent_principal_lamports);
        put_u64(output, &mut cursor, self.replay_rent_principal_lamports);
        for id in [
            self.interval_binding_id,
            self.failure_policy_binding_id,
            self.funding_receipt_id,
            self.replay_account,
            self.rent_payer,
            self.neutral_sink,
            self.last_transition_receipt_id,
            self.last_liveness_receipt_id,
            self.resolution_receipt_id,
            self.close_authorization_id,
        ] {
            put_id(output, &mut cursor, id);
        }
        output[cursor..cursor + PRODUCT_INTERVAL_WORK_BODY_BYTES_V1]
            .copy_from_slice(&self.product_work_body);
        cursor += PRODUCT_INTERVAL_WORK_BODY_BYTES_V1;
        debug_assert_eq!(
            cursor + WORK_RESERVED_BYTES_V1,
            registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES
        );
        Ok(())
    }

    /// Decode hostile account bytes and reject every noncanonical field.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<Self> {
        if input[0] != registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let phase = FailureIntervalConsensusPhaseV1::decode(input[3])?;
        let mut cursor = 4;
        let generation = take_u64(input, &mut cursor);
        let transition_nonce = take_u64(input, &mut cursor);
        let accepted_recovery_progress_total = take_u64(input, &mut cursor);
        let work_rent_principal_lamports = take_u64(input, &mut cursor);
        let replay_rent_principal_lamports = take_u64(input, &mut cursor);
        let interval_binding_id = take_id(input, &mut cursor);
        let failure_policy_binding_id = take_id(input, &mut cursor);
        let funding_receipt_id = take_id(input, &mut cursor);
        let replay_account = take_id(input, &mut cursor);
        let rent_payer = take_id(input, &mut cursor);
        let neutral_sink = take_id(input, &mut cursor);
        let last_transition_receipt_id = take_id(input, &mut cursor);
        let last_liveness_receipt_id = take_id(input, &mut cursor);
        let resolution_receipt_id = take_id(input, &mut cursor);
        let close_authorization_id = take_id(input, &mut cursor);
        let mut product_work_body = [0; PRODUCT_INTERVAL_WORK_BODY_BYTES_V1];
        product_work_body
            .copy_from_slice(&input[cursor..cursor + PRODUCT_INTERVAL_WORK_BODY_BYTES_V1]);
        cursor += PRODUCT_INTERVAL_WORK_BODY_BYTES_V1;
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let value = Self {
            bump: input[2],
            phase,
            generation,
            transition_nonce,
            accepted_recovery_progress_total,
            work_rent_principal_lamports,
            replay_rent_principal_lamports,
            interval_binding_id,
            failure_policy_binding_id,
            funding_receipt_id,
            replay_account,
            rent_payer,
            neutral_sink,
            last_transition_receipt_id,
            last_liveness_receipt_id,
            resolution_receipt_id,
            close_authorization_id,
            product_work_body,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for id in [
            self.interval_binding_id,
            self.failure_policy_binding_id,
            self.funding_receipt_id,
            self.replay_account,
            self.rent_payer,
            self.neutral_sink,
        ] {
            require_live(id)?;
        }
        if self.generation == 0
            || self.work_rent_principal_lamports == 0
            || self.replay_rent_principal_lamports == 0
            || self.replay_account == self.rent_payer
            || self.replay_account == self.neutral_sink
            || self.rent_payer == self.neutral_sink
            || self.product_work_body.iter().all(|byte| *byte == 0)
        {
            return Err(CodecError::ZeroValue);
        }
        let advanced = self.transition_nonce != 0;
        if advanced
            != (!is_zero(&self.last_transition_receipt_id)
                && !is_zero(&self.last_liveness_receipt_id))
        {
            return Err(CodecError::InvalidEnum);
        }
        match self.phase {
            FailureIntervalConsensusPhaseV1::Active => {
                if !is_zero(&self.resolution_receipt_id) || !is_zero(&self.close_authorization_id) {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureIntervalConsensusPhaseV1::Resolved => {
                if is_zero(&self.resolution_receipt_id) || !is_zero(&self.close_authorization_id) {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureIntervalConsensusPhaseV1::Closed => {
                if is_zero(&self.resolution_receipt_id) || is_zero(&self.close_authorization_id) {
                    return Err(CodecError::InvalidEnum);
                }
            }
        }
        Ok(())
    }
}

/// Complete fixed `0xac/v1` permanent replay-account body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusReplayAccountV1 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Current lifecycle phase.
    pub phase: FailureIntervalConsensusPhaseV1,
    /// Exact Failure/liveness generation.
    pub generation: u64,
    /// Monotone bounded-transition nonce.
    pub transition_nonce: u64,
    /// Exact permanent balance last authenticated by the adapter.
    pub preserved_lamports: u64,
    /// Immutable interval lifecycle identity.
    pub interval_binding_id: [u8; HASH_BYTES],
    /// Immutable parent Failure policy binding.
    pub failure_policy_binding_id: [u8; HASH_BYTES],
    /// Full-width V2 economic occurrence.
    pub market_instance_v2_id: [u8; HASH_BYTES],
    /// Closed mutable `0xab/v1` account, retained as history.
    pub work_account: [u8; HASH_BYTES],
    /// Initial Product work identity.
    pub initial_work_id: [u8; HASH_BYTES],
    /// Current or final Product work identity.
    pub current_work_id: [u8; HASH_BYTES],
    /// Current or final rolling Product transcript.
    pub current_transcript: [u8; HASH_BYTES],
    /// Last bounded Failure transition receipt.
    pub last_transition_receipt_id: [u8; HASH_BYTES],
    /// Last liveness work receipt.
    pub last_liveness_receipt_id: [u8; HASH_BYTES],
    /// Exhaustive Product certificate, zero before resolution.
    pub certificate_id: [u8; HASH_BYTES],
    /// Failure interval resolution receipt, zero before resolution.
    pub resolution_receipt_id: [u8; HASH_BYTES],
    /// Exact work close authorization, zero before close.
    pub close_authorization_id: [u8; HASH_BYTES],
}

impl FailureIntervalConsensusReplayAccountV1 {
    /// Encode every permanent replay byte canonically.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION;
        output[2] = self.bump;
        output[3] = self.phase as u8;
        let mut cursor = 4;
        put_u64(output, &mut cursor, self.generation);
        put_u64(output, &mut cursor, self.transition_nonce);
        put_u64(output, &mut cursor, self.preserved_lamports);
        for id in [
            self.interval_binding_id,
            self.failure_policy_binding_id,
            self.market_instance_v2_id,
            self.work_account,
            self.initial_work_id,
            self.current_work_id,
            self.current_transcript,
            self.last_transition_receipt_id,
            self.last_liveness_receipt_id,
            self.certificate_id,
            self.resolution_receipt_id,
            self.close_authorization_id,
        ] {
            put_id(output, &mut cursor, id);
        }
        debug_assert_eq!(
            cursor + REPLAY_RESERVED_BYTES_V1,
            registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES
        );
        Ok(())
    }

    /// Decode hostile permanent replay bytes.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<Self> {
        if input[0] != registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let phase = FailureIntervalConsensusPhaseV1::decode(input[3])?;
        let mut cursor = 4;
        let generation = take_u64(input, &mut cursor);
        let transition_nonce = take_u64(input, &mut cursor);
        let preserved_lamports = take_u64(input, &mut cursor);
        let interval_binding_id = take_id(input, &mut cursor);
        let failure_policy_binding_id = take_id(input, &mut cursor);
        let market_instance_v2_id = take_id(input, &mut cursor);
        let work_account = take_id(input, &mut cursor);
        let initial_work_id = take_id(input, &mut cursor);
        let current_work_id = take_id(input, &mut cursor);
        let current_transcript = take_id(input, &mut cursor);
        let last_transition_receipt_id = take_id(input, &mut cursor);
        let last_liveness_receipt_id = take_id(input, &mut cursor);
        let certificate_id = take_id(input, &mut cursor);
        let resolution_receipt_id = take_id(input, &mut cursor);
        let close_authorization_id = take_id(input, &mut cursor);
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let value = Self {
            bump: input[2],
            phase,
            generation,
            transition_nonce,
            preserved_lamports,
            interval_binding_id,
            failure_policy_binding_id,
            market_instance_v2_id,
            work_account,
            initial_work_id,
            current_work_id,
            current_transcript,
            last_transition_receipt_id,
            last_liveness_receipt_id,
            certificate_id,
            resolution_receipt_id,
            close_authorization_id,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for id in [
            self.interval_binding_id,
            self.failure_policy_binding_id,
            self.market_instance_v2_id,
            self.work_account,
            self.initial_work_id,
            self.current_work_id,
            self.current_transcript,
        ] {
            require_live(id)?;
        }
        if self.generation == 0 || self.preserved_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        let advanced = self.transition_nonce != 0;
        if advanced
            != (!is_zero(&self.last_transition_receipt_id)
                && !is_zero(&self.last_liveness_receipt_id))
        {
            return Err(CodecError::InvalidEnum);
        }
        match self.phase {
            FailureIntervalConsensusPhaseV1::Active => {
                if !is_zero(&self.certificate_id)
                    || !is_zero(&self.resolution_receipt_id)
                    || !is_zero(&self.close_authorization_id)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureIntervalConsensusPhaseV1::Resolved => {
                if is_zero(&self.certificate_id)
                    || is_zero(&self.resolution_receipt_id)
                    || !is_zero(&self.close_authorization_id)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
            FailureIntervalConsensusPhaseV1::Closed => {
                if is_zero(&self.certificate_id)
                    || is_zero(&self.resolution_receipt_id)
                    || is_zero(&self.close_authorization_id)
                {
                    return Err(CodecError::InvalidEnum);
                }
            }
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
