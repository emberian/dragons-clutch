//! Headerless chain-derived Registry continuation for common Trading Hot.
//!
//! The top-level Registry instruction carries the canonical Hot bytes without
//! a second request header. This builder retains the established admission PDA
//! authority while proving that every formerly serialized coordinate is
//! independently reconstructible from the checked release/cache/Hot state.

use dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;
use dclutch_core_contract::ContentId;
use dclutch_registry_svm::continuation_v2::{
    TransparentHotAdmissionSeedsV2, TransparentHotContinuationV2,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::hot_continuation_v1::{
    RegistryHotContinuationErrorV1, RegistryHotContinuationStateV1,
    build_registry_hot_continuation_v1,
};
use crate::Observation;

/// Exact Registry-owned account prefix before the nested Trading Hot frame.
pub const TRANSPARENT_HOT_PREFIX_ACCOUNTS_V2: usize = 6;
/// Exact admission location in the nested Trading continuation frame.
pub const TRANSPARENT_HOT_ADMISSION_ACCOUNT_V2: usize = HOT_FIXED_ACCOUNT_COUNT_V3;

/// Chain observations required by the headerless successor.
pub type RegistryHotContinuationStateV2 = RegistryHotContinuationStateV1;
/// Stable refusal inherited from the same checked Registry/Loader/Hot joins.
pub type RegistryHotContinuationErrorV2 = RegistryHotContinuationErrorV1;

/// Checked unsigned headerless Registry wrapper around exact Trading Hot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryHotContinuationReportV2 {
    /// Exact top-level Registry instruction with byte-identical Hot data.
    pub instruction: Instruction,
    /// Shared finalized observation selecting Core and Trading.
    pub observation: Observation,
    /// Exact activated release-set identity.
    pub release_set_id: ContentId,
    /// Digest of the complete activation-cache bytes.
    pub activation_cache_digest: ContentId,
    /// Digest of the complete unchanged Trading Hot bytes.
    pub hot_instruction_digest: ContentId,
    /// Invocation-scoped System-vacant admission candidate.
    pub admission: Pubkey,
    /// Headerless facts reconstructing the established admission authority.
    pub continuation: TransparentHotContinuationV2,
}

/// Build one exact unsigned headerless Registry Core+Trading Hot call.
pub fn build_registry_hot_continuation_v2(
    state: &RegistryHotContinuationStateV2,
    hot_instruction: &Instruction,
) -> Result<RegistryHotContinuationReportV2, RegistryHotContinuationErrorV2> {
    let checked = build_registry_hot_continuation_v1(state, hot_instruction)?;
    let hot_instruction_len = u32::try_from(hot_instruction.data.len())
        .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let continuation = TransparentHotContinuationV2::new(
        checked.release_set_id,
        checked.activation_cache_digest,
        checked.hot_instruction_digest,
        hot_instruction_len,
    )
    .map_err(|_| RegistryHotContinuationErrorV1::Identity)?;
    let batch = continuation
        .role_batch_request()
        .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let seeds = TransparentHotAdmissionSeedsV2::new(
        continuation,
        state.activation_cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| RegistryHotContinuationErrorV1::Admission)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.hot_instruction_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        &state.registry_program.key,
    )
    .0;
    if admission != checked.admission
        || hot_instruction
            .accounts
            .iter()
            .any(|meta| meta.pubkey == admission)
    {
        return Err(RegistryHotContinuationErrorV1::Admission);
    }

    let mut child_accounts = hot_instruction.accounts.clone();
    child_accounts.insert(
        TRANSPARENT_HOT_ADMISSION_ACCOUNT_V2,
        AccountMeta::new_readonly(admission, false),
    );
    let mut accounts =
        Vec::with_capacity(TRANSPARENT_HOT_PREFIX_ACCOUNTS_V2 + child_accounts.len());
    accounts.extend([
        AccountMeta::new_readonly(state.activation_cache.key, false),
        AccountMeta::new_readonly(state.core_program.key, false),
        AccountMeta::new_readonly(state.core_programdata.key, false),
        AccountMeta::new_readonly(state.trading_program.key, false),
        AccountMeta::new_readonly(state.trading_programdata.key, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(child_accounts);

    Ok(RegistryHotContinuationReportV2 {
        instruction: Instruction {
            program_id: state.registry_program.key,
            accounts,
            data: hot_instruction.data.clone(),
        },
        observation: checked.observation,
        release_set_id: checked.release_set_id,
        activation_cache_digest: checked.activation_cache_digest,
        hot_instruction_digest: checked.hot_instruction_digest,
        admission,
        continuation,
    })
}
