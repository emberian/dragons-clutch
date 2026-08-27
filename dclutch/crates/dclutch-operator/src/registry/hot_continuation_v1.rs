//! Chain-derived top-level Registry continuation for common Trading Hot execution.
//!
//! This wrapper reauthenticates the exact Core and Trading deployments from one
//! finalized Registry cache observation, hashes the byte-exact existing Hot
//! instruction, derives the invocation-scoped admission PDA, and inserts that
//! candidate at the fixed boundary immediately before strategy extras. It does
//! not persist an admission account, sign, submit, or reinterpret Hot bytes.

use dclutch_capability_program_contract::hot_v3::{
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::{
    Error as RegistryError, RegistryReauthenticationState, build_registry_reauthentication_v1,
};
use crate::{Observation, ObservedAccount};

/// Exact Registry-owned account prefix before the nested Trading Hot frame.
pub const REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1: usize = 6;
/// Exact admission location in the nested Trading continuation frame.
pub const TRADING_HOT_CONTINUATION_ADMISSION_ACCOUNT_V1: usize = HOT_FIXED_ACCOUNT_COUNT_V3;

/// Same-finalized Registry and Loader facts selecting Core and Trading.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryHotContinuationStateV1 {
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Existing Registry-owned release activation cache.
    pub activation_cache: ObservedAccount,
    /// Current cache-selected Core program.
    pub core_program: ObservedAccount,
    /// Current cache-selected Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current cache-selected Trading program.
    pub trading_program: ObservedAccount,
    /// Current cache-selected Trading ProgramData.
    pub trading_programdata: ObservedAccount,
}

/// Checked unsigned Registry wrapper around one byte-exact Trading Hot call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryHotContinuationReportV1 {
    /// Exact top-level Registry instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting both deployments.
    pub observation: Observation,
    /// Exact activated release-set identity.
    pub release_set_id: ContentId,
    /// Digest of the complete Registry activation-cache bytes.
    pub activation_cache_digest: ContentId,
    /// Digest of the complete unchanged Trading Hot instruction bytes.
    pub hot_instruction_digest: ContentId,
    /// Invocation-scoped System-vacant admission candidate.
    pub admission: Pubkey,
    /// Canonical fixed continuation header prepended by Registry.
    pub continuation: RegistryContinuationRequestV1,
}

/// Refusal from the chain-derived Hot Registry wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryHotContinuationErrorV1 {
    /// Registry cache or current Loader deployment authentication refused.
    Registry(RegistryError),
    /// Hot bytes, selected release, program, fixed accounts, or privileges refused.
    InvalidHotInstruction,
    /// A required digest or checked length refused.
    Identity,
    /// Admission derivation or checked account geometry refused.
    Admission,
}

impl From<RegistryError> for RegistryHotContinuationErrorV1 {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

/// Build one exact unsigned top-level Registry Core+Trading Hot continuation.
pub fn build_registry_hot_continuation_v1(
    state: &RegistryHotContinuationStateV1,
    hot_instruction: &Instruction,
) -> Result<RegistryHotContinuationReportV1, RegistryHotContinuationErrorV1> {
    let core = build_registry_reauthentication_v1(
        &RegistryReauthenticationState {
            registry_program: state.registry_program.clone(),
            cache: state.activation_cache.clone(),
            role_program: state.core_program.clone(),
            role_programdata: state.core_programdata.clone(),
        },
        ExecutionRoleV1::Core,
    )?;
    let trading = build_registry_reauthentication_v1(
        &RegistryReauthenticationState {
            registry_program: state.registry_program.clone(),
            cache: state.activation_cache.clone(),
            role_program: state.trading_program.clone(),
            role_programdata: state.trading_programdata.clone(),
        },
        ExecutionRoleV1::Trading,
    )?;
    if core.observation != trading.observation
        || core.cache != trading.cache
        || core.execution_release_set_id != trading.execution_release_set_id
        || core.role_program != state.core_program.key
        || trading.role_program != state.trading_program.key
    {
        return Err(RegistryHotContinuationErrorV1::InvalidHotInstruction);
    }

    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(&hot_instruction.data)
        .map_err(|_| RegistryHotContinuationErrorV1::InvalidHotInstruction)?;
    if hot_instruction.program_id != state.trading_program.key
        || envelope.release_set() != core.execution_release_set_id.to_bytes()
        || hot_instruction.accounts.len() < HOT_FIXED_ACCOUNT_COUNT_V3
    {
        return Err(RegistryHotContinuationErrorV1::InvalidHotInstruction);
    }
    for (index, expected) in [
        (HOT_ACTIVATION_CACHE_ACCOUNT_V3, state.activation_cache.key),
        (HOT_CORE_PROGRAM_ACCOUNT_V3, state.core_program.key),
        (HOT_CORE_PROGRAMDATA_ACCOUNT_V3, state.core_programdata.key),
        (HOT_TRADING_PROGRAM_ACCOUNT_V3, state.trading_program.key),
        (
            HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            state.trading_programdata.key,
        ),
        (HOT_REGISTRY_PROGRAM_ACCOUNT_V3, state.registry_program.key),
    ] {
        let meta = hot_instruction
            .accounts
            .get(index)
            .ok_or(RegistryHotContinuationErrorV1::InvalidHotInstruction)?;
        if meta.pubkey != expected || meta.is_signer || meta.is_writable {
            return Err(RegistryHotContinuationErrorV1::InvalidHotInstruction);
        }
    }

    let activation_cache_digest = ContentId::new(hash(&state.activation_cache.data).to_bytes())
        .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let hot_instruction_digest = ContentId::new(hash(&hot_instruction.data).to_bytes())
        .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let hot_instruction_len = u32::try_from(hot_instruction.data.len())
        .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
        core.execution_release_set_id,
        activation_cache_digest,
        hot_instruction_digest,
        hot_instruction_len,
    )
    .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let batch = continuation
        .role_batch_request()
        .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        state.activation_cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch_request_digest = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let continuation_role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch_request_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &state.registry_program.key,
    )
    .0;
    if hot_instruction
        .accounts
        .iter()
        .any(|meta| meta.pubkey == admission)
    {
        return Err(RegistryHotContinuationErrorV1::Admission);
    }

    let mut child_accounts = hot_instruction.accounts.clone();
    child_accounts.insert(
        TRADING_HOT_CONTINUATION_ADMISSION_ACCOUNT_V1,
        AccountMeta::new_readonly(admission, false),
    );
    let mut accounts =
        Vec::with_capacity(REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1 + child_accounts.len());
    accounts.extend([
        AccountMeta::new_readonly(state.activation_cache.key, false),
        AccountMeta::new_readonly(state.core_program.key, false),
        AccountMeta::new_readonly(state.core_programdata.key, false),
        AccountMeta::new_readonly(state.trading_program.key, false),
        AccountMeta::new_readonly(state.trading_programdata.key, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(child_accounts);
    let mut data =
        Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + hot_instruction.data.len());
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&hot_instruction.data);

    Ok(RegistryHotContinuationReportV1 {
        instruction: Instruction {
            program_id: state.registry_program.key,
            accounts,
            data,
        },
        observation: core.observation,
        release_set_id: core.execution_release_set_id,
        activation_cache_digest,
        hot_instruction_digest,
        admission,
        continuation,
    })
}

#[cfg(test)]
mod tests;
