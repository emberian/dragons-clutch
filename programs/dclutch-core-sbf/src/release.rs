//! Registry-owned release-set and current-deployment authentication.

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Admission, Binding, CoreState, Identity, ReleaseReceipt, ReleaseSet, RetirementAdmissions, Role,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{
    AuthenticatedRoleReceiptV1, RegistryInstructionV1,
    batch_v2::{AuthenticatedRoleBatchReceiptV2, RoleBatchRequestV2},
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke},
    pubkey::Pubkey,
};

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

#[derive(Clone, Copy)]
struct ExpectedRoleObservation {
    role: Role,
    program: [u8; 32],
    programdata: [u8; 32],
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
/// role deployments through a single CPI and immediate fixed receipt.
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
    let request = RoleBatchRequestV2::new(
        ContentId::new(release_set_id).map_err(|_| CoreSbfError::Release)?,
        cache_digest,
        &registry_roles,
    )
    .map_err(|_| CoreSbfError::Release)?;
    let request_bytes = request.to_bytes();
    let mut metas = Vec::with_capacity(1 + requested.len() * 2);
    let mut infos = Vec::with_capacity(2 + requested.len() * 2);
    let mut expected_observations = Vec::with_capacity(requested.len());
    metas.push(AccountMeta::new_readonly(*cache.key, false));
    infos.push(cache.clone());
    for entry in requested {
        validate_release_accounts(cache, registry, entry.program, entry.programdata)?;
        metas.push(AccountMeta::new_readonly(*entry.program.key, false));
        metas.push(AccountMeta::new_readonly(*entry.programdata.key, false));
        infos.push(entry.program.clone());
        infos.push(entry.programdata.clone());
        expected_observations.push(ExpectedRoleObservation {
            role: entry.role,
            program: entry.program.key.to_bytes(),
            programdata: entry.programdata.key.to_bytes(),
        });
    }
    infos.push(registry.clone());
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    invoke(&instruction, &infos).map_err(|_| CoreSbfError::Release)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::Release)?;
    if producer != *registry.key {
        return Err(CoreSbfError::Release);
    }
    let expected_request_digest =
        ContentId::new(hash(&request_bytes).to_bytes()).map_err(|_| CoreSbfError::Release)?;
    verify_role_batch_receipt(
        &receipt_bytes,
        registry.key.to_bytes(),
        cache.key.to_bytes(),
        cache_digest,
        release_set_id,
        expected_request_digest,
        request.role_mask(),
        &expected_observations,
        selected,
    )?;
    Ok(RoleBatchAdmissions {
        registry: expected_registry,
        release_set_id: identity(release_set_id)?,
        selected,
        authenticated_mask: request.role_mask(),
    })
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn verify_role_batch_receipt(
    receipt_bytes: &[u8],
    registry_program: [u8; 32],
    activation_cache: [u8; 32],
    activation_cache_digest: ContentId,
    release_set_id: [u8; 32],
    request_digest: ContentId,
    role_mask: u8,
    expected_observations: &[ExpectedRoleObservation],
    selected: ReleaseSet,
) -> Result<(), CoreSbfError> {
    let receipt = AuthenticatedRoleBatchReceiptV2::decode(receipt_bytes)
        .map_err(|_| CoreSbfError::Release)?;
    if receipt.registry_program().to_bytes() != registry_program
        || receipt.activation_cache() != activation_cache
        || receipt.activation_cache_digest() != activation_cache_digest
        || receipt.release_set_id().to_bytes() != release_set_id
        || receipt.request_digest() != request_digest
        || usize::from(receipt.role_count()) != expected_observations.len()
        || receipt.role_mask() != role_mask
    {
        return Err(CoreSbfError::Release);
    }
    for (index, expected) in expected_observations.iter().copied().enumerate() {
        let observation = receipt
            .observation(index)
            .ok_or(CoreSbfError::Release)?
            .map_err(|_| CoreSbfError::Release)?;
        let selected = selected_binding(selected, expected.role);
        if observation.role() != registry_role(expected.role)
            || observation.program().to_bytes() != expected.program
            || observation.programdata() != expected.programdata
            || observation.program().to_bytes() != selected.program.to_bytes()
            || observation.artifact_release_id().to_bytes() != selected.artifact_release.to_bytes()
            || observation.semantic_release_id().to_bytes() != selected.semantic_release.to_bytes()
        {
            return Err(CoreSbfError::Release);
        }
    }
    Ok(())
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
    use dclutch_registry_svm::batch_v2::{
        ROLE_BATCH_RECEIPT_BYTES_V2, RoleBatchReceiptInputV2, RoleDeploymentObservationV2,
        encode_role_batch_receipt_v2,
    };
    use dclutch_release_set_contract::{ArtifactReleaseIdV1, ProgramIdentityV1};

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

    fn receipt_observation(
        role: Role,
        binding: Binding,
        programdata: [u8; 32],
    ) -> RoleDeploymentObservationV2 {
        RoleDeploymentObservationV2::new(
            registry_role(role),
            ProgramIdentityV1::new(binding.program.to_bytes()).expect("program"),
            programdata,
            ArtifactReleaseIdV1::new(binding.artifact_release.to_bytes()).expect("artifact"),
            ContentId::new(binding.semantic_release.to_bytes()).expect("semantic release"),
            7,
        )
        .expect("observation")
    }

    #[allow(clippy::too_many_arguments)]
    fn receipt_bytes(
        registry: [u8; 32],
        cache: [u8; 32],
        cache_digest: ContentId,
        release_set_id: ContentId,
        request_digest: ContentId,
        observations: &[RoleDeploymentObservationV2],
    ) -> [u8; ROLE_BATCH_RECEIPT_BYTES_V2] {
        let mut output = [0; ROLE_BATCH_RECEIPT_BYTES_V2];
        encode_role_batch_receipt_v2(
            RoleBatchReceiptInputV2 {
                registry_program: ProgramIdentityV1::new(registry).expect("Registry"),
                activation_cache: cache,
                activation_cache_digest: cache_digest,
                release_set_id,
                request_digest,
                observations,
            },
            &mut output,
        )
        .expect("receipt");
        output
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

    #[test]
    fn batch_receipt_substitution_refuses_before_admission_projection() {
        let registry = [90; 32];
        let cache = [91; 32];
        let cache_digest = ContentId::new([92; 32]).expect("cache digest");
        let release_set_id = ContentId::new([93; 32]).expect("release set");
        let request_digest = ContentId::new([94; 32]).expect("request digest");
        let selected = ReleaseSet {
            release_set_id: test_identity(93),
            bindings: [
                test_binding(1),
                test_binding(2),
                test_binding(3),
                test_binding(4),
                test_binding(5),
            ],
        };
        let [core, claims, _trading, _resolution, _custody] = selected.bindings;
        let core_programdata = [31; 32];
        let claims_programdata = [32; 32];
        let expected = [
            ExpectedRoleObservation {
                role: Role::Core,
                program: core.program.to_bytes(),
                programdata: core_programdata,
            },
            ExpectedRoleObservation {
                role: Role::Claims,
                program: claims.program.to_bytes(),
                programdata: claims_programdata,
            },
        ];
        let core_observation = receipt_observation(Role::Core, core, core_programdata);
        let claims_observation = receipt_observation(Role::Claims, claims, claims_programdata);
        let observations = [core_observation, claims_observation];
        let valid = receipt_bytes(
            registry,
            cache,
            cache_digest,
            release_set_id,
            request_digest,
            &observations,
        );
        assert_eq!(
            verify_role_batch_receipt(
                &valid,
                registry,
                cache,
                cache_digest,
                release_set_id.to_bytes(),
                request_digest,
                0b11,
                &expected,
                selected,
            ),
            Ok(())
        );

        let substituted_programdata = [
            receipt_observation(Role::Core, core, [99; 32]),
            claims_observation,
        ];
        let hostile = receipt_bytes(
            registry,
            cache,
            cache_digest,
            release_set_id,
            request_digest,
            &substituted_programdata,
        );
        assert_eq!(
            verify_role_batch_receipt(
                &hostile,
                registry,
                cache,
                cache_digest,
                release_set_id.to_bytes(),
                request_digest,
                0b11,
                &expected,
                selected,
            ),
            Err(CoreSbfError::Release)
        );

        let hostile_cache = receipt_bytes(
            registry,
            [88; 32],
            cache_digest,
            release_set_id,
            request_digest,
            &observations,
        );
        assert_eq!(
            verify_role_batch_receipt(
                &hostile_cache,
                registry,
                cache,
                cache_digest,
                release_set_id.to_bytes(),
                request_digest,
                0b11,
                &expected,
                selected,
            ),
            Err(CoreSbfError::Release)
        );
    }
}
