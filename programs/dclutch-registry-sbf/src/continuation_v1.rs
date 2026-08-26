//! One-shot Registry-authenticated continuation invocation.

use std::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{RegistryError, batch_v2};

/// Authenticate the canonical role batch and invoke its selected continuation.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let header_bytes = instruction_data
        .get(..REGISTRY_CONTINUATION_REQUEST_BYTES_V1)
        .ok_or(RegistryError::Continuation)?;
    let request = RegistryContinuationRequestV1::decode(header_bytes)
        .map_err(|_| RegistryError::Continuation)?;
    let continuation = instruction_data
        .get(REGISTRY_CONTINUATION_REQUEST_BYTES_V1..)
        .ok_or(RegistryError::Continuation)?;
    if usize::try_from(request.continuation_len()).map_err(|_| RegistryError::Arithmetic)?
        != continuation.len()
        || hash(continuation).to_bytes() != request.continuation_digest().to_bytes()
    {
        return Err(RegistryError::Continuation.into());
    }

    let batch_request = request
        .role_batch_request()
        .map_err(|_| RegistryError::Continuation)?;
    let batch_count = 1_usize
        .checked_add(
            usize::from(request.role_count())
                .checked_mul(2)
                .ok_or(RegistryError::Arithmetic)?,
        )
        .ok_or(RegistryError::Arithmetic)?;
    let admission_index = batch_count;
    let continuation_start = admission_index
        .checked_add(1)
        .ok_or(RegistryError::Arithmetic)?;
    let batch_accounts = accounts
        .get(..batch_count)
        .ok_or(RegistryError::AccountFrame)?;
    let admission = accounts
        .get(admission_index)
        .ok_or(RegistryError::AccountFrame)?;
    let continuation_accounts = accounts
        .get(continuation_start..)
        .ok_or(RegistryError::AccountFrame)?;
    if continuation_accounts.is_empty()
        || admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
    {
        return Err(RegistryError::AccountFrame.into());
    }

    let authenticated = batch_v2::authenticate_request(program_id, batch_accounts, batch_request)?;
    if authenticated.cache_digest != request.activation_cache_digest() {
        return Err(RegistryError::Continuation.into());
    }
    let selected = authenticated
        .observations
        .iter()
        .find(|observation| observation.role() == request.continuation_role())
        .ok_or(RegistryError::Continuation)?;
    let continuation_program = Pubkey::new_from_array(selected.program().to_bytes());
    let selected_program_info = batch_accounts
        .iter()
        .find(|account| account.key == &continuation_program)
        .ok_or(RegistryError::Continuation)?;

    let batch_request_bytes = batch_request.to_bytes();
    let batch_request_digest =
        ContentId::new(hash(&batch_request_bytes).to_bytes()).map_err(|_| RegistryError::Batch)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        request,
        batch_accounts
            .first()
            .ok_or(RegistryError::AccountFrame)?
            .key
            .to_bytes(),
        batch_request_digest,
    )
    .map_err(|_| RegistryError::Continuation)?;
    let release_set = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch_digest = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let continuation_role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let (expected_admission, bump) = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release_set.as_slice(),
            cache.as_slice(),
            batch_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
        ],
        program_id,
    );
    if expected_admission != *admission.key
        || continuation_accounts
            .iter()
            .filter(|account| account.key == admission.key)
            .count()
            != 1
    {
        return Err(RegistryError::Continuation.into());
    }

    let mut metas = Vec::with_capacity(continuation_accounts.len());
    let mut infos = Vec::with_capacity(continuation_accounts.len() + 1);
    for account in continuation_accounts {
        let signer = if account.key == admission.key {
            true
        } else {
            account.is_signer
        };
        let meta = if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        };
        metas.push(meta);
        infos.push(account.clone());
    }
    infos.push(selected_program_info.clone());
    let instruction = Instruction {
        program_id: continuation_program,
        accounts: metas,
        data: continuation.to_vec(),
    };
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            seeds.domain(),
            release_set.as_slice(),
            cache.as_slice(),
            batch_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
            bump_seed.as_slice(),
        ]],
    )
    .map_err(|_| RegistryError::Continuation.into())
}
