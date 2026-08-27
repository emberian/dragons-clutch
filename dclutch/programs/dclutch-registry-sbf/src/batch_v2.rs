//! Family-neutral read-only batch authentication of activated execution roles.

use std::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1};
use dclutch_registry_svm::batch_v2::{
    ROLE_BATCH_RECEIPT_BYTES_V2, RoleBatchReceiptInputV2, RoleBatchRequestV2,
    RoleDeploymentObservationV2, encode_role_batch_receipt_v2,
};
use dclutch_release_set_contract::{ExecutionRoleV1, ProgramIdentityV1};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

use super::{RegistryError, authenticate_cache_identity, cached_role_deployment_observation};

pub(super) struct AuthenticatedBatchV2 {
    pub(super) cache_digest: ContentId,
    pub(super) observations: Vec<RoleDeploymentObservationV2>,
}

/// Process one fixed, canonical role batch.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = RoleBatchRequestV2::decode(instruction_data)
        .map_err(|_| ProgramError::from(RegistryError::Batch))?;
    let authenticated = authenticate_request(program_id, accounts, request)?;
    let cache = accounts.first().ok_or(RegistryError::AccountFrame)?;
    let request_digest =
        ContentId::new(hash(instruction_data).to_bytes()).map_err(|_| RegistryError::Batch)?;
    let registry_program =
        ProgramIdentityV1::new(program_id.to_bytes()).map_err(|_| RegistryError::Batch)?;
    let mut receipt = [0_u8; ROLE_BATCH_RECEIPT_BYTES_V2];
    encode_role_batch_receipt_v2(
        RoleBatchReceiptInputV2 {
            registry_program,
            activation_cache: cache.key.to_bytes(),
            activation_cache_digest: authenticated.cache_digest,
            release_set_id: request.release_set_id(),
            request_digest,
            observations: &authenticated.observations,
        },
        &mut receipt,
    )
    .map_err(|_| RegistryError::Batch)?;
    set_return_data(&receipt);
    Ok(())
}

/// Authenticate one exact read-only batch without materializing return data.
#[inline(never)]
pub(super) fn authenticate_request(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RoleBatchRequestV2,
) -> Result<AuthenticatedBatchV2, ProgramError> {
    let expected_accounts = 1_usize
        .checked_add(
            usize::from(request.role_count())
                .checked_mul(2)
                .ok_or(RegistryError::Arithmetic)?,
        )
        .ok_or(RegistryError::Arithmetic)?;
    if accounts.len() != expected_accounts {
        return Err(RegistryError::AccountFrame.into());
    }
    let cache = accounts.first().ok_or(RegistryError::AccountFrame)?;
    require_cache_privileges(cache)?;
    let cache_data = cache.try_borrow_data().map_err(|_| RegistryError::Borrow)?;
    let cache_digest =
        ContentId::new(hash(&cache_data).to_bytes()).map_err(|_| RegistryError::ActivationCache)?;
    if cache_digest != request.activation_cache_digest() {
        return Err(RegistryError::ActivationCache.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| RegistryError::ActivationCache)?;
    authenticate_cache_identity(program_id, cache, activated)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?
        != request.release_set_id()
    {
        return Err(RegistryError::ActivationCache.into());
    }

    let count = usize::from(request.role_count());
    let mut observations = Vec::with_capacity(count);
    for index in 0..count {
        let role = request.role(index).ok_or(RegistryError::Batch)?;
        let account_offset = 1_usize
            .checked_add(index.checked_mul(2).ok_or(RegistryError::Arithmetic)?)
            .ok_or(RegistryError::Arithmetic)?;
        let program = accounts
            .get(account_offset)
            .ok_or(RegistryError::AccountFrame)?;
        let programdata = accounts
            .get(account_offset + 1)
            .ok_or(RegistryError::AccountFrame)?;
        require_role_privileges(program, programdata)?;
        observations.push(authenticate_role(activated, role, program, programdata)?);
    }
    drop(cache_data);

    Ok(AuthenticatedBatchV2 {
        cache_digest,
        observations,
    })
}

#[inline(never)]
fn authenticate_role(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<RoleDeploymentObservationV2, ProgramError> {
    let activated_role = activated
        .role(role)
        .map_err(|_| RegistryError::ActivationCache)?;
    let release = activated_role.release();
    // `cached_role_deployment_observation` owns the argument that an immutable
    // deployment's admitted ELF digest is still its exact current digest.
    // An upgradeable release keeps the full current-ELF hash path.
    let observation = cached_role_deployment_observation(program, programdata, release)?;
    activated_role
        .authenticate_current_deployment(observation)
        .map_err(|_| RegistryError::Deployment)?;
    encode_observation(role, release, activated_role.artifact_release_id())
}

fn encode_observation(
    role: ExecutionRoleV1,
    release: ArtifactReleaseV1,
    artifact_release_id: dclutch_release_set_contract::ArtifactReleaseIdV1,
) -> Result<RoleDeploymentObservationV2, ProgramError> {
    RoleDeploymentObservationV2::new(
        role,
        release.program(),
        release.programdata(),
        artifact_release_id,
        release.semantic_release_id(),
        release.deployment_slot(),
    )
    .map_err(|_| RegistryError::Batch.into())
}

fn require_cache_privileges(cache: &AccountInfo<'_>) -> ProgramResult {
    if cache.is_signer || cache.is_writable || cache.executable {
        return Err(RegistryError::AccountFrame.into());
    }
    Ok(())
}

fn require_role_privileges(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> ProgramResult {
    if program.is_signer
        || program.is_writable
        || !program.executable
        || programdata.is_signer
        || programdata.is_writable
        || programdata.executable
    {
        return Err(RegistryError::AccountFrame.into());
    }
    Ok(())
}
