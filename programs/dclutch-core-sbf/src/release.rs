//! Registry-owned release-set and current-deployment authentication.

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Admission, Binding, CoreState, Identity, ReleaseReceipt, ReleaseSet, RetirementAdmissions, Role,
};
use dclutch_registry_activation_auth_v1::authenticate_activated_role_v1;
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{
    batch_v2::RoleBatchRequestV2,
    continuation_v1::{RegistryContinuationAdmissionSeedsV1, RegistryContinuationRequestV1},
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::CoreSbfError;

/// One role and its exact current Loader account pair in a batch request.
#[derive(Clone, Copy)]
pub(crate) struct RoleDeploymentAccounts<'accounts, 'info> {
    role: Role,
    program: &'accounts AccountInfo<'info>,
    programdata: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> RoleDeploymentAccounts<'accounts, 'info> {
    pub(crate) const fn new(
        role: Role,
        program: &'accounts AccountInfo<'info>,
        programdata: &'accounts AccountInfo<'info>,
    ) -> Self {
        Self {
            role,
            program,
            programdata,
        }
    }
}

/// One cache/release-set authentication carrying a fixed requested role set.
#[derive(Clone, Copy)]
pub(crate) struct RoleBatchAdmissions {
    registry: Identity,
    release_set_id: Identity,
    selected: ReleaseSet,
    authenticated_mask: u8,
}

impl RoleBatchAdmissions {
    pub(crate) fn require(self, role: Role) -> Result<(), CoreSbfError> {
        if self.authenticated_mask & role_bit(role) == 0 {
            return Err(CoreSbfError::Release);
        }
        Ok(())
    }

    /// Project the existing semantic Admission only for a role proven active
    /// in the immediate Registry batch receipt.
    pub(crate) fn admission(self, role: Role) -> Result<Admission, CoreSbfError> {
        self.require(role)?;
        let observed = selected_binding(self.selected, role);
        Ok(Admission {
            market_registry_program: self.registry,
            market_release_set_id: self.release_set_id,
            selected: self.selected,
            receipt: ReleaseReceipt {
                registry_program: self.registry,
                release_set_id: self.release_set_id,
                role,
                observed,
                activation_cache_authenticated: true,
                current_deployment_reauthenticated: true,
            },
        })
    }

    /// Lower one exact Core/Claims/Resolution/Custody Registry batch into the
    /// shared fixed-memory retirement admission observation.
    pub(crate) fn retirement(self, state: CoreState) -> Result<RetirementAdmissions, CoreSbfError> {
        for role in [Role::Core, Role::Claims, Role::Resolution, Role::Custody] {
            self.require(role)?;
        }
        RetirementAdmissions::from_authenticated_batch(
            state,
            self.registry,
            self.release_set_id,
            self.selected,
            [Role::Core, Role::Claims, Role::Resolution, Role::Custody],
        )
        .map_err(|_| CoreSbfError::Release)
    }
}

/// Authenticate one Registry cache and a canonical ordered subset of current
/// role deployments, reading the cache directly and invoking nothing.
#[inline(never)]
pub(crate) fn authenticate_roles<'accounts, 'info>(
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    expected_registry: Identity,
    release_set_id: [u8; 32],
    requested: &[RoleDeploymentAccounts<'accounts, 'info>],
) -> Result<RoleBatchAdmissions, CoreSbfError> {
    require_expected_registry(registry.key, expected_registry)?;
    if registry.is_signer || registry.is_writable || !registry.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    let (selected, cache_digest) = {
        let bytes = cache.try_borrow_data().map_err(|_| CoreSbfError::Release)?;
        let view = ActivatedExecutionReleaseSetViewV1::decode(&bytes)
            .map_err(|_| CoreSbfError::Release)?;
        let expected_cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
            registry.key,
        )
        .0;
        if cache.key != &expected_cache
            || cache.owner != registry.key
            || cache.is_signer
            || cache.is_writable
            || cache.executable
            || view
                .execution_release_set_id()
                .map_err(|_| CoreSbfError::Release)?
                .to_bytes()
                != release_set_id
        {
            return Err(CoreSbfError::Release);
        }
        (
            release_projection(view)?,
            ContentId::new(hash(&bytes).to_bytes()).map_err(|_| CoreSbfError::Release)?,
        )
    };

    let registry_roles = requested
        .iter()
        .map(|entry| registry_role(entry.role))
        .collect::<Vec<_>>();
    // The request is still CONSTRUCTED and never sent. It is the owner of the
    // canonical-order rule -- strictly ascending role tags, which also forbids
    // a repeated role -- and of the mask this admission is keyed by, and
    // rebuilding either of those here would be a second copy that could drift.
    // What is gone is the invocation: composing this request and CPI-ing it
    // into the Registry is the same reentrancy the per-role route was, so a
    // Core reached under a Registry continuation could not run it, and there is
    // no fallback to it.
    let request = RoleBatchRequestV2::new(
        ContentId::new(release_set_id).map_err(|_| CoreSbfError::Release)?,
        cache_digest,
        &registry_roles,
    )
    .map_err(|_| CoreSbfError::Release)?;
    for entry in requested {
        validate_release_accounts(cache, registry, entry.program, entry.programdata)?;
        // Every fact the batch receipt carried per role -- program identity,
        // ProgramData link, Loader ownership, executability, deployment slot,
        // artifact and semantic release, and the ELF digest under the release's
        // own upgrade policy -- is established here against the same cache.
        authenticate_activated_role_v1(
            registry,
            cache,
            &release_set_id,
            registry_role(entry.role),
            entry.program,
            entry.programdata,
        )
        .map_err(|_| CoreSbfError::Release)?;
    }
    Ok(RoleBatchAdmissions {
        registry: expected_registry,
        release_set_id: identity(release_set_id)?,
        selected,
        authenticated_mask: request.role_mask(),
    })
}

/// Consume one invocation-scoped Registry admission instead of recursively
/// calling Registry for the same current role batch.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_continuation_roles<'accounts, 'info>(
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    expected_registry: Identity,
    release_set_id: [u8; 32],
    requested: &[RoleDeploymentAccounts<'accounts, 'info>],
    continuation_digest: ContentId,
    continuation_len: u32,
) -> Result<(RoleBatchAdmissions, RegistryContinuationRequestV1), CoreSbfError> {
    require_expected_registry(registry.key, expected_registry)?;
    if registry.is_signer || registry.is_writable || !registry.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    if !admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    let (selected, cache_digest) = {
        let bytes = cache.try_borrow_data().map_err(|_| CoreSbfError::Release)?;
        let view = ActivatedExecutionReleaseSetViewV1::decode(&bytes)
            .map_err(|_| CoreSbfError::Release)?;
        let expected_cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
            registry.key,
        )
        .0;
        if cache.key != &expected_cache
            || cache.owner != registry.key
            || cache.is_signer
            || cache.is_writable
            || cache.executable
            || view
                .execution_release_set_id()
                .map_err(|_| CoreSbfError::Release)?
                .to_bytes()
                != release_set_id
        {
            return Err(CoreSbfError::Release);
        }
        for entry in requested {
            validate_release_accounts(cache, registry, entry.program, entry.programdata)?;
            let release = view
                .role(registry_role(entry.role))
                .map_err(|_| CoreSbfError::Release)?
                .release();
            if release.program().to_bytes() != entry.program.key.to_bytes()
                || release.programdata() != entry.programdata.key.to_bytes()
            {
                return Err(CoreSbfError::Release);
            }
        }
        (
            release_projection(view)?,
            ContentId::new(hash(&bytes).to_bytes()).map_err(|_| CoreSbfError::Release)?,
        )
    };
    let registry_roles = requested
        .iter()
        .map(|entry| registry_role(entry.role))
        .collect::<Vec<_>>();
    let continuation = RegistryContinuationRequestV1::new(
        ContentId::new(release_set_id).map_err(|_| CoreSbfError::Release)?,
        cache_digest,
        continuation_digest,
        continuation_len,
        ExecutionRoleV1::Core,
        &registry_roles,
    )
    .map_err(|_| CoreSbfError::Release)?;
    let batch_request = continuation
        .role_batch_request()
        .map_err(|_| CoreSbfError::Release)?;
    let batch_digest = ContentId::new(hash(&batch_request.to_bytes()).to_bytes())
        .map_err(|_| CoreSbfError::Release)?;
    let seeds =
        RegistryContinuationAdmissionSeedsV1::new(continuation, cache.key.to_bytes(), batch_digest)
            .map_err(|_| CoreSbfError::Release)?;
    let release = seeds.release_set();
    let cache_key = seeds.activation_cache();
    let batch_request_digest = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let continuation_role = seeds.continuation_role();
    let digest = seeds.continuation_digest();
    let expected = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache_key.as_slice(),
            batch_request_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            digest.as_slice(),
        ],
        registry.key,
    )
    .0;
    if expected != *admission.key {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok((
        RoleBatchAdmissions {
            registry: expected_registry,
            release_set_id: identity(release_set_id)?,
            selected,
            authenticated_mask: continuation.role_mask(),
        },
        continuation,
    ))
}

/// Authenticate the Registry cache and one role's current Loader deployment.
#[inline(never)]
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
    // The current deployment is authenticated from the cache this function
    // already read, not by invoking the Registry. Core is a child of a Registry
    // continuation, so a CPI from here is reentrancy -- the Registry is already
    // at depth one and Solana refuses depth four onto it. The role, the release
    // set and the Program identity are all checked inside
    // `authenticate_activated_role_v1`, which is the Registry's own code.
    let receipt = authenticate_activated_role_v1(
        registry,
        cache,
        &release_set_id,
        registry_role,
        role_program,
        role_programdata,
    )
    .map_err(|_| CoreSbfError::Release)?;
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

const fn role_bit(role: Role) -> u8 {
    match role {
        Role::Core => 1 << 0,
        Role::Claims => 1 << 1,
        Role::Trading => 1 << 2,
        Role::Resolution => 1 << 3,
        Role::Custody => 1 << 4,
    }
}

pub(crate) fn identity(bytes: [u8; 32]) -> Result<Identity, CoreSbfError> {
    Identity::new(bytes).map_err(|_| CoreSbfError::Reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity(fill: u8) -> Identity {
        Identity::new([fill; 32]).expect("nonzero identity")
    }

    fn test_binding(fill: u8) -> Binding {
        Binding {
            program: test_identity(fill),
            artifact_release: test_identity(fill + 10),
            semantic_release: test_identity(fill + 20),
        }
    }

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

    #[test]
    fn batch_projects_the_exact_legacy_admission_and_refuses_unrequested_roles() {
        let registry = test_identity(90);
        let release_set_id = test_identity(91);
        let selected = ReleaseSet {
            release_set_id,
            bindings: [
                test_binding(1),
                test_binding(2),
                test_binding(3),
                test_binding(4),
                test_binding(5),
            ],
        };
        let batch = RoleBatchAdmissions {
            registry,
            release_set_id,
            selected,
            authenticated_mask: role_bit(Role::Core)
                | role_bit(Role::Claims)
                | role_bit(Role::Trading)
                | role_bit(Role::Custody),
        };
        for role in [Role::Core, Role::Claims, Role::Trading, Role::Custody] {
            let observed = selected_binding(selected, role);
            assert_eq!(
                batch.admission(role),
                Ok(Admission {
                    market_registry_program: registry,
                    market_release_set_id: release_set_id,
                    selected,
                    receipt: ReleaseReceipt {
                        registry_program: registry,
                        release_set_id,
                        role,
                        observed,
                        activation_cache_authenticated: true,
                        current_deployment_reauthenticated: true,
                    },
                })
            );
        }
        assert_eq!(
            batch.admission(Role::Resolution),
            Err(CoreSbfError::Release)
        );
    }
}
