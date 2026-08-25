//! Registry-owned release-set and current-deployment authentication.

use alloc::vec::Vec;

use dclutch_market_core_codec::{Admission, Binding, Identity, ReleaseReceipt, ReleaseSet, Role};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke},
    pubkey::Pubkey,
};

use crate::CoreSbfError;

/// Authenticate the Registry cache and one role's current Loader deployment.
pub(crate) fn authenticate_role<'info>(
    cache: &AccountInfo<'info>,
    registry: &AccountInfo<'info>,
    role_program: &AccountInfo<'info>,
    role_programdata: &AccountInfo<'info>,
    expected_registry: Identity,
    release_set_id: [u8; 32],
    role: Role,
) -> Result<Admission, CoreSbfError> {
    require_expected_registry(registry.key, expected_registry)?;
    validate_release_accounts(cache, registry, role_program, role_programdata)?;
    let expected_cache =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_id], registry.key).0;
    if cache.key != &expected_cache || cache.owner != registry.key {
        return Err(CoreSbfError::Release);
    }
    let selected = {
        let bytes = cache.try_borrow_data().map_err(|_| CoreSbfError::Release)?;
        let view = ActivatedExecutionReleaseSetViewV1::decode(&bytes)
            .map_err(|_| CoreSbfError::Release)?;
        if view
            .execution_release_set_id()
            .map_err(|_| CoreSbfError::Release)?
            .to_bytes()
            != release_set_id
        {
            return Err(CoreSbfError::Release);
        }
        release_projection(view)?
    };
    let registry_role = registry_role(role);
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*role_program.key, false),
            AccountMeta::new_readonly(*role_programdata.key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(registry_role)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            role_program.clone(),
            role_programdata.clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| CoreSbfError::Release)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::Release)?;
    if producer != *registry.key {
        return Err(CoreSbfError::Release);
    }
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&receipt_bytes).map_err(|_| CoreSbfError::Release)?;
    if receipt.role() != registry_role
        || receipt.execution_release_set_id().to_bytes() != release_set_id
        || receipt.program().to_bytes() != role_program.key.to_bytes()
    {
        return Err(CoreSbfError::Release);
    }
    let observed = selected_binding(selected, role);
    if observed.program.to_bytes() != role_program.key.to_bytes()
        || observed.artifact_release.to_bytes() != receipt.artifact_release_id().to_bytes()
        || observed.semantic_release.to_bytes() != receipt.semantic_release_id().to_bytes()
    {
        return Err(CoreSbfError::Release);
    }
    Ok(Admission {
        market_registry_program: expected_registry,
        market_release_set_id: identity(release_set_id)?,
        selected,
        receipt: ReleaseReceipt {
            registry_program: identity(registry.key.to_bytes())?,
            release_set_id: identity(release_set_id)?,
            role,
            observed,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    })
}

fn require_expected_registry(
    registry: &Pubkey,
    expected_registry: Identity,
) -> Result<(), CoreSbfError> {
    if registry.to_bytes() != expected_registry.to_bytes() {
        return Err(CoreSbfError::Release);
    }
    Ok(())
}

fn validate_release_accounts(
    cache: &AccountInfo<'_>,
    registry: &AccountInfo<'_>,
    role_program: &AccountInfo<'_>,
    role_programdata: &AccountInfo<'_>,
) -> Result<(), CoreSbfError> {
    if cache.is_signer
        || cache.is_writable
        || cache.executable
        || registry.is_signer
        || registry.is_writable
        || !registry.executable
        || role_program.is_signer
        || role_program.is_writable
        || !role_program.executable
        || role_programdata.is_signer
        || role_programdata.is_writable
        || role_programdata.executable
    {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn release_projection(
    view: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<ReleaseSet, CoreSbfError> {
    Ok(ReleaseSet {
        release_set_id: identity(
            view.execution_release_set_id()
                .map_err(|_| CoreSbfError::Release)?
                .to_bytes(),
        )?,
        bindings: [
            projection_binding(view, ExecutionRoleV1::Core)?,
            projection_binding(view, ExecutionRoleV1::Claims)?,
            projection_binding(view, ExecutionRoleV1::Trading)?,
            projection_binding(view, ExecutionRoleV1::Resolution)?,
            projection_binding(view, ExecutionRoleV1::Custody)?,
        ],
    })
}

fn projection_binding(
    view: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
) -> Result<Binding, CoreSbfError> {
    let activated = view.role(role).map_err(|_| CoreSbfError::Release)?;
    let release = activated.release();
    Ok(Binding {
        program: identity(release.program().to_bytes())?,
        artifact_release: identity(activated.artifact_release_id().to_bytes())?,
        semantic_release: identity(release.semantic_release_id().to_bytes())?,
    })
}

pub(crate) const fn registry_role(role: Role) -> ExecutionRoleV1 {
    match role {
        Role::Core => ExecutionRoleV1::Core,
        Role::Claims => ExecutionRoleV1::Claims,
        Role::Trading => ExecutionRoleV1::Trading,
        Role::Resolution => ExecutionRoleV1::Resolution,
        Role::Custody => ExecutionRoleV1::Custody,
    }
}

fn selected_binding(selected: ReleaseSet, role: Role) -> Binding {
    let [core, claims, trading, resolution, custody] = selected.bindings;
    match role {
        Role::Core => core,
        Role::Claims => claims,
        Role::Trading => trading,
        Role::Resolution => resolution,
        Role::Custody => custody,
    }
}

pub(crate) fn identity(bytes: [u8; 32]) -> Result<Identity, CoreSbfError> {
    Identity::new(bytes).map_err(|_| CoreSbfError::Reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substituted_registry_refuses_before_cpi() {
        let expected = Identity::new([9; 32]).expect("nonzero Registry");
        assert_eq!(
            require_expected_registry(&Pubkey::new_from_array([9; 32]), expected),
            Ok(()),
        );
        assert_eq!(
            require_expected_registry(&Pubkey::new_from_array([11; 32]), expected),
            Err(CoreSbfError::Release),
            "the selected Core executable cannot substitute for the persisted Registry",
        );
    }
}
