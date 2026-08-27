//! Family-neutral read-only batch authentication of activated execution roles.
//!
//! This is authentication material, not a route. The standalone DCLTRGB2 entry
//! that returned an 896-byte role-batch receipt was deleted on 2026-08-27: no
//! tier ever invoked it, and a five-role request cannot execute at real ELF
//! sizes -- the five per-role activation pins sum to 2,407,858 CU against a
//! 1,400,000 ceiling, and the same five deployment authentications are what a
//! batch performs in one transaction. Tier 1 activates and reauthenticates one
//! role per transaction.
//!
//! `authenticate_request` stays because it is live: `continuation_v1::process`
//! and `hot_continuation_v2::process` both reach it in-process, each selecting
//! one observation out of the batch it authenticates.

use std::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1};
use dclutch_registry_svm::batch_v2::{RoleBatchRequestV2, RoleDeploymentObservationV2};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{RegistryError, authenticate_cache_identity, cached_role_deployment_observation};

pub(super) struct AuthenticatedBatchV2 {
    pub(super) cache_digest: ContentId,
    pub(super) observations: Vec<RoleDeploymentObservationV2>,
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
