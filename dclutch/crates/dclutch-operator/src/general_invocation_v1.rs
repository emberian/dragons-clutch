//! Durable caller construction over the executable seven-action General Hot path.
//!
//! This module does not accept a prebuilt instruction or a caller-supplied
//! account list. It invokes the existing chain-derived General successor
//! builder, compiles the exact v0 packet, then seals the resulting capability,
//! release, request, signer, lookup-table, and lock geometry into a durable
//! content-addressed intent. The wider GEN-SEVEN V3 request remains refused by
//! the underlying artifact join until all seven new triples land together.

use dclutch_market::capability_program::hot_v3::{
    HOT_MARKET_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_trading::general::{
    artifacts_v3::{GeneralArtifactBytesV3, GeneralArtifactSelectionV3, decode_general_request_v3},
    invocation_v1::{
        GENERAL_INVOCATION_ACCOUNT_METAS_DOMAIN_V1, GENERAL_INVOCATION_ARTIFACT_GRAPH_DOMAIN_V1,
        GENERAL_INVOCATION_LOCK_SET_DOMAIN_V1, GENERAL_INVOCATION_MAX_UNIQUE_LOCKS_V1,
        GENERAL_INVOCATION_SIGNER_SET_DOMAIN_V1, GeneralInvocationErrorV1,
        GeneralInvocationFieldsV1, GeneralInvocationReplayV1, GeneralInvocationV1,
    },
};
use dclutch_trading::general_codec::{Action, successor_request_v2::CONTROLLER_REQUEST_BYTES_V2};
use solana_hash::Hash;
use solana_program::{hash::hashv, instruction::AccountMeta, pubkey::Pubkey};

use crate::{
    ObservedAccount,
    general_hot_v3::{
        GeneralHotArtifactDigestsV3, GeneralHotOperatorErrorV3, GeneralHotStateV3,
        GeneralSuccessorInstructionV5, GeneralSuccessorTransactionPlanV0,
        build_general_successor_instruction_v5, compile_general_successor_v0,
    },
};

/// Complete unsigned packet plus its durable caller and replay projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralInvocationPlanV1 {
    /// Chain-derived General successor instruction and exact child topology.
    pub successor: GeneralSuccessorInstructionV5,
    /// Packet-safe unsigned v0 transaction plan.
    pub transaction: GeneralSuccessorTransactionPlanV0,
    /// Message-independent content-addressed caller intent.
    pub invocation: GeneralInvocationV1,
    /// Canonical replay poststate to persist only after transaction success.
    pub replay_after: GeneralInvocationReplayV1,
}

/// Stable refusal from durable General caller construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralInvocationOperatorErrorV1 {
    /// Existing General artifact, state, release, or packet construction refused.
    General(GeneralHotOperatorErrorV3),
    /// Durable invocation or replay semantics refused.
    Invocation(GeneralInvocationErrorV1),
    /// The built instruction did not rejoin its exact state or report.
    Join,
    /// Count or preimage construction overflowed.
    Arithmetic,
}

impl From<GeneralHotOperatorErrorV3> for GeneralInvocationOperatorErrorV1 {
    fn from(value: GeneralHotOperatorErrorV3) -> Self {
        Self::General(value)
    }
}

impl From<GeneralInvocationErrorV1> for GeneralInvocationOperatorErrorV1 {
    fn from(value: GeneralInvocationErrorV1) -> Self {
        Self::Invocation(value)
    }
}

/// Result alias for durable General caller construction.
pub type Result<T> = core::result::Result<T, GeneralInvocationOperatorErrorV1>;

/// Build one caller-backed General packet and its exact durable invocation.
///
/// `replay_bytes` must be the complete caller-owned replay observation. The
/// returned `replay_after` is a projection only: a future onchain caller must
/// authenticate ownership and write it after the Hot instruction succeeds.
/// This host operator performs no signing, submission, or state mutation.
#[allow(clippy::too_many_arguments)]
pub fn build_general_invocation_v1(
    state: &GeneralHotStateV3,
    artifact_selection: GeneralArtifactSelectionV3,
    artifact_bytes: GeneralArtifactBytesV3<'_>,
    action: Action,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
    replay_bytes: &[u8],
) -> Result<GeneralInvocationPlanV1> {
    let replay = GeneralInvocationReplayV1::decode(replay_bytes)?;
    let successor =
        build_general_successor_instruction_v5(state, artifact_selection, artifact_bytes, action)?;
    let transaction =
        compile_general_successor_v0(&successor, payer, recent_blockhash, lookup_table)?;
    let report = &successor.hot;
    let instruction = &report.instruction;
    let (envelope, family_request) = HotExecutionEnvelopeV3::split_instruction(&instruction.data)
        .map_err(|_| GeneralInvocationOperatorErrorV1::Join)?;
    if family_request.len() != CONTROLLER_REQUEST_BYTES_V2
        || decode_general_request_v3(family_request)
            .map_err(|_| GeneralInvocationOperatorErrorV1::Join)?
            != successor.request
        || successor.request.action != action
        || report.action != action
        || transaction.hot.action != action
        || successor.heap_frame_bytes != transaction.heap_frame_bytes
        || successor.heap_frame_bytes != transaction.hot.heap_frame_bytes
    {
        return Err(GeneralInvocationOperatorErrorV1::Join);
    }

    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralInvocationOperatorErrorV1::Join)?;
    let root = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(GeneralInvocationOperatorErrorV1::Join)?;
    let checked = state
        .checked_release
        .ok_or(GeneralInvocationOperatorErrorV1::Join)?;
    let family_request_digest = solana_program::hash::hash(family_request).to_bytes();
    let root_prestate_digest = solana_program::hash::hash(&root.account.data).to_bytes();
    if envelope.market() != market.account.key.to_bytes()
        || envelope.market() != replay.market()
        || envelope.release_set() != state.release_set
        || envelope.generation() != state.generation
        || envelope.generation() != replay.generation()
        || envelope.root_prestate_digest() != root_prestate_digest
        || instruction.program_id != checked.trading_program
        || report.family_request_digest != family_request_digest
        || report.checked_manifest_digest != checked.checked_manifest_digest
        || report.trading_artifact_release != checked.trading_artifact_release
        || report.general_artifact_release != checked.general_artifact_release
        || transaction.hot.checked_manifest_digest != report.checked_manifest_digest
        || transaction.hot.trading_artifact_release != report.trading_artifact_release
        || transaction.hot.general_artifact_release != report.general_artifact_release
        || transaction.hot.artifacts != report.artifacts
        || payer.to_bytes() != replay.payer()
    {
        return Err(GeneralInvocationOperatorErrorV1::Join);
    }

    let lookup_tables = &transaction.hot.message.lookup_tables;
    if lookup_tables.as_slice() != [lookup_table.key] {
        return Err(GeneralInvocationOperatorErrorV1::Join);
    }
    let (lock_set_digest, unique_lock_count) = lock_set_digest_v1(
        payer,
        instruction.program_id,
        lookup_tables,
        &instruction.accounts,
    )?;
    let (signer_set_digest, signer_count) = signer_set_digest_v1(
        payer,
        &transaction.hot.required_signers,
        &instruction.accounts,
    )?;
    let account_meta_count = u16::try_from(instruction.accounts.len())
        .map_err(|_| GeneralInvocationOperatorErrorV1::Arithmetic)?;
    let invocation = GeneralInvocationV1::new(GeneralInvocationFieldsV1 {
        action,
        market: envelope.market(),
        root: root.account.key.to_bytes(),
        root_prestate_digest,
        release_set: envelope.release_set(),
        checked_manifest_digest: report.checked_manifest_digest,
        trading_artifact_release: report.trading_artifact_release,
        general_artifact_release: report.general_artifact_release,
        artifact_graph_digest: artifact_graph_digest_v1(report.artifacts),
        family_request_digest,
        account_metas_digest: account_metas_digest_v1(&instruction.accounts)?,
        lock_set_digest,
        signer_set_digest,
        trading_program: instruction.program_id.to_bytes(),
        payer: payer.to_bytes(),
        lookup_table: lookup_table.key.to_bytes(),
        nonce: replay.next_nonce(),
        generation: envelope.generation(),
        account_meta_count,
        unique_lock_count,
        signer_count,
    })?;
    let replay_after = replay.advance(invocation)?;
    Ok(GeneralInvocationPlanV1 {
        successor,
        transaction,
        invocation,
        replay_after,
    })
}

fn artifact_graph_digest_v1(artifacts: GeneralHotArtifactDigestsV3) -> [u8; 32] {
    let mut bytes = [0_u8; 11 * 32];
    for (chunk, identity) in bytes.chunks_exact_mut(32).zip([
        artifacts.program_set,
        artifacts.descriptor,
        artifacts.config,
        artifacts.account_profile,
        artifacts.lifecycle_policy,
        artifacts.request_profile,
        artifacts.strategy,
        artifacts.certificate,
        artifacts.admission,
        artifacts.transition,
        artifacts.effect,
    ]) {
        chunk.copy_from_slice(&identity);
    }
    hashv(&[GENERAL_INVOCATION_ARTIFACT_GRAPH_DOMAIN_V1, &bytes]).to_bytes()
}

fn account_metas_digest_v1(accounts: &[AccountMeta]) -> Result<[u8; 32]> {
    let count =
        u16::try_from(accounts.len()).map_err(|_| GeneralInvocationOperatorErrorV1::Arithmetic)?;
    let capacity = accounts
        .len()
        .checked_mul(33)
        .and_then(|value| value.checked_add(2))
        .ok_or(GeneralInvocationOperatorErrorV1::Arithmetic)?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(&count.to_le_bytes());
    for account in accounts {
        bytes.extend_from_slice(&account.pubkey.to_bytes());
        bytes.push(u8::from(account.is_signer) | (u8::from(account.is_writable) << 1));
    }
    Ok(hashv(&[GENERAL_INVOCATION_ACCOUNT_METAS_DOMAIN_V1, &bytes]).to_bytes())
}

fn lock_set_digest_v1(
    payer: Pubkey,
    program: Pubkey,
    lookup_tables: &[Pubkey],
    accounts: &[AccountMeta],
) -> Result<([u8; 32], u16)> {
    let capacity = accounts
        .len()
        .checked_add(lookup_tables.len())
        .and_then(|value| value.checked_add(2))
        .ok_or(GeneralInvocationOperatorErrorV1::Arithmetic)?;
    let mut keys = Vec::with_capacity(capacity);
    keys.push(payer);
    keys.push(program);
    keys.extend_from_slice(lookup_tables);
    keys.extend(accounts.iter().map(|account| account.pubkey));
    canonical_key_set_digest_v1(&mut keys, GENERAL_INVOCATION_LOCK_SET_DOMAIN_V1, Some(64))
}

fn signer_set_digest_v1(
    payer: Pubkey,
    reported: &[Pubkey],
    accounts: &[AccountMeta],
) -> Result<([u8; 32], u16)> {
    let mut expected = vec![payer];
    for account in accounts {
        if account.is_signer && !expected.contains(&account.pubkey) {
            expected.push(account.pubkey);
        }
    }
    if expected != reported || reported.iter().any(|key| *key == Pubkey::default()) {
        return Err(GeneralInvocationOperatorErrorV1::Join);
    }
    canonical_key_set_digest_v1(&mut expected, GENERAL_INVOCATION_SIGNER_SET_DOMAIN_V1, None)
}

fn canonical_key_set_digest_v1(
    keys: &mut Vec<Pubkey>,
    domain: &[u8],
    maximum: Option<usize>,
) -> Result<([u8; 32], u16)> {
    keys.sort_unstable_by_key(Pubkey::to_bytes);
    keys.dedup();
    if keys.is_empty() || maximum.is_some_and(|limit| keys.len() > limit) {
        return Err(GeneralInvocationOperatorErrorV1::Invocation(
            GeneralInvocationErrorV1::NonCanonical,
        ));
    }
    let count =
        u16::try_from(keys.len()).map_err(|_| GeneralInvocationOperatorErrorV1::Arithmetic)?;
    if maximum.is_some() && count > GENERAL_INVOCATION_MAX_UNIQUE_LOCKS_V1 {
        return Err(GeneralInvocationOperatorErrorV1::Invocation(
            GeneralInvocationErrorV1::NonCanonical,
        ));
    }
    let capacity = keys
        .len()
        .checked_mul(32)
        .and_then(|value| value.checked_add(2))
        .ok_or(GeneralInvocationOperatorErrorV1::Arithmetic)?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(&count.to_le_bytes());
    for key in keys {
        bytes.extend_from_slice(&key.to_bytes());
    }
    Ok((hashv(&[domain, &bytes]).to_bytes(), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn artifacts() -> GeneralHotArtifactDigestsV3 {
        GeneralHotArtifactDigestsV3 {
            program_set: [1; 32],
            descriptor: [2; 32],
            config: [3; 32],
            account_profile: [4; 32],
            lifecycle_policy: [5; 32],
            request_profile: [6; 32],
            strategy: [7; 32],
            certificate: [8; 32],
            admission: [9; 32],
            transition: [10; 32],
            effect: [11; 32],
        }
    }

    #[test]
    fn ordered_alias_and_privilege_substitution_change_the_meta_commitment() {
        let canonical = vec![
            AccountMeta::new(key(1), false),
            AccountMeta::new_readonly(key(2), true),
        ];
        let mut aliased = canonical.clone();
        aliased[1].pubkey = key(1);
        let mut escalated = canonical.clone();
        escalated[1].is_writable = true;
        assert_ne!(
            account_metas_digest_v1(&canonical).expect("canonical"),
            account_metas_digest_v1(&aliased).expect("aliased")
        );
        assert_ne!(
            account_metas_digest_v1(&canonical).expect("canonical"),
            account_metas_digest_v1(&escalated).expect("escalated")
        );
    }

    #[test]
    fn lock_census_is_unique_and_refuses_the_sixty_fifth_lock() {
        let aliased = vec![
            AccountMeta::new_readonly(key(3), false),
            AccountMeta::new_readonly(key(3), false),
        ];
        let (_, count) =
            lock_set_digest_v1(key(1), key(2), &[key(4)], &aliased).expect("four unique locks");
        assert_eq!(count, 4);

        let mut accounts = Vec::new();
        for value in 3_u8..=65 {
            accounts.push(AccountMeta::new_readonly(key(value), false));
        }
        assert_eq!(
            lock_set_digest_v1(key(1), key(2), &[], &accounts),
            Err(GeneralInvocationOperatorErrorV1::Invocation(
                GeneralInvocationErrorV1::NonCanonical
            ))
        );
    }

    #[test]
    fn signer_set_requires_the_exact_compiled_order_before_canonical_commitment() {
        let accounts = vec![
            AccountMeta::new_readonly(key(2), true),
            AccountMeta::new_readonly(key(3), false),
        ];
        let (_, count) =
            signer_set_digest_v1(key(1), &[key(1), key(2)], &accounts).expect("exact report");
        assert_eq!(count, 2);
        assert_eq!(
            signer_set_digest_v1(key(1), &[key(2), key(1)], &accounts),
            Err(GeneralInvocationOperatorErrorV1::Join)
        );
    }

    #[test]
    fn every_artifact_coordinate_changes_the_graph_identity() {
        let canonical = artifact_graph_digest_v1(artifacts());
        let mut value = artifacts();
        value.effect = [12; 32];
        assert_ne!(artifact_graph_digest_v1(value), canonical);
        let mut value = artifacts();
        value.program_set = [12; 32];
        assert_ne!(artifact_graph_digest_v1(value), canonical);
    }
}
