//! Noncyclic immutable authority for Registry and Rent infrastructure.

use core::convert::TryFrom;

use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    program::{invoke, invoke_signed},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    CoreSbfError,
    frame::{FoundAccounts, InitializeInfrastructureAccounts},
    records::authenticate_finalized_record,
};

/// Initialize the sole immutable per-Core profile under current Loader authority.
#[inline(never)]
pub(crate) fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = InitializeInfrastructureAccounts::parse(accounts)?;
    authenticate_current_core_upgrade_authority(
        program_id,
        frame.core_programdata,
        frame.upgrade_authority,
    )?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Infrastructure)?;
    let registry = authenticate_artifact(
        frame.registry_program.key,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        &rent,
        true,
    )?;
    let rent_binding = authenticate_artifact(
        frame.registry_program.key,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        &rent,
        true,
    )?;
    if frame.registry_program.key == program_id || frame.rent_program.key == program_id {
        return Err(CoreSbfError::Infrastructure.into());
    }
    let profile = ProtocolInfrastructureProfileV1::new(registry, rent_binding)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    create_profile(program_id, &frame, &rent, profile)?;
    Ok(())
}

/// Authenticate the immutable profile and exact current Registry/Rent releases for Found.
#[inline(never)]
pub(crate) fn authenticate_found(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let expected =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], program_id)
            .0;
    if frame.infrastructure_profile.key != &expected
        || frame.infrastructure_profile.owner != program_id
        || frame.infrastructure_profile.data_len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || frame.infrastructure_profile.executable
        || !rent.is_exempt(
            frame.infrastructure_profile.lamports(),
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
        )
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let bytes = frame
        .infrastructure_profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let profile = ProtocolInfrastructureProfileV1::decode(&bytes)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if profile.registry().program().to_bytes() != frame.registry_program.key.to_bytes()
        || profile.rent().program().to_bytes() != frame.rent_program.key.to_bytes()
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let registry = authenticate_artifact(
        frame.registry_program.key,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        rent,
        true,
    )?;
    let rent_binding = authenticate_artifact(
        frame.registry_program.key,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        rent,
        true,
    )?;
    if registry != profile.registry() || rent_binding != profile.rent() {
        return Err(CoreSbfError::Infrastructure);
    }
    Ok(())
}

/// Require the Registry-selected current Core artifact to be immutable too.
pub(crate) fn authenticate_immutable_core_release(
    frame: &FoundAccounts<'_, '_>,
    release_set_id: [u8; 32],
) -> Result<(), CoreSbfError> {
    use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
    use dclutch_release_set_contract::ExecutionRoleV1;

    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        frame.registry_program.key,
    )
    .0;
    if frame.activation_cache.key != &expected_cache
        || frame.activation_cache.owner != frame.registry_program.key
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let cache = frame
        .activation_cache
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&cache)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if view
        .execution_release_set_id()
        .map_err(|_| CoreSbfError::Infrastructure)?
        .to_bytes()
        != release_set_id
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let release = view
        .role(ExecutionRoleV1::Core)
        .map_err(|_| CoreSbfError::Infrastructure)?
        .release();
    require_immutable_current_deployment(frame.core_program, frame.core_programdata, release)
}

fn authenticate_current_core_upgrade_authority(
    program_id: &Pubkey,
    programdata: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Result<(), CoreSbfError> {
    let expected =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata.key != &expected
        || programdata.owner != &bpf_loader_upgradeable::ID
        || programdata.executable
        || !authority.is_signer
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let bytes = programdata
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let view = ProgramDataV3View::parse(&bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    if view.upgrade_authority() != Some(authority.key.to_bytes()) {
        return Err(CoreSbfError::Infrastructure);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_artifact(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    rent: &Rent,
    require_immutable: bool,
) -> Result<ExecutionRoleBindingV1, CoreSbfError> {
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if bytes.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(CoreSbfError::Infrastructure);
    }
    let digest = hash(&bytes).to_bytes();
    authenticate_finalized_record(
        registry,
        raw,
        staging,
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        digest,
        &bytes,
    )?;
    let release = ArtifactReleaseV1::decode(&bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    if release.program().to_bytes() != program.key.to_bytes()
        || (require_immutable && release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable)
    {
        return Err(CoreSbfError::Infrastructure);
    }
    require_current_deployment(program, programdata, release)?;
    let artifact = ArtifactReleaseIdV1::new(digest).map_err(|_| CoreSbfError::Infrastructure)?;
    Ok(ExecutionRoleBindingV1::new(release.program(), artifact))
}

fn require_immutable_current_deployment(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<(), CoreSbfError> {
    if release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable {
        return Err(CoreSbfError::Infrastructure);
    }
    require_current_deployment(program, programdata, release)
}

fn require_current_deployment(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<(), CoreSbfError> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata()
        || programdata.key != &expected_programdata
    {
        return Err(CoreSbfError::Infrastructure);
    }
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| CoreSbfError::Infrastructure)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| CoreSbfError::Infrastructure)
}

fn create_profile(
    program_id: &Pubkey,
    frame: &InitializeInfrastructureAccounts<'_, '_>,
    rent: &Rent,
    profile: ProtocolInfrastructureProfileV1,
) -> Result<(), solana_program::program_error::ProgramError> {
    let (expected, bump) =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], program_id);
    if frame.profile.key != &expected
        || frame.profile.owner != &system_program::ID
        || frame.profile.data_len() != 0
        || frame.profile.executable
    {
        return Err(CoreSbfError::Infrastructure.into());
    }
    let required = rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1);
    let top_up = required.saturating_sub(frame.profile.lamports());
    if top_up > 0 {
        invoke(
            &transfer(frame.payer.key, frame.profile.key, top_up),
            &[
                frame.payer.clone(),
                frame.profile.clone(),
                frame.system.clone(),
            ],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let bump_seed = [bump];
    let signer = [
        PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            frame.profile.key,
            u64::try_from(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1)
                .map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        &[frame.profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    invoke_signed(
        &assign(frame.profile.key, program_id),
        &[frame.profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    let encoded = profile.to_bytes();
    {
        let mut data = frame
            .profile
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Infrastructure)?;
        if data.len() != encoded.len() {
            return Err(CoreSbfError::Infrastructure.into());
        }
        data.copy_from_slice(&encoded);
    }
    let committed = frame
        .profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if frame.profile.owner != program_id
        || ProtocolInfrastructureProfileV1::decode(&committed) != Ok(profile)
    {
        return Err(CoreSbfError::Infrastructure.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;

    use super::*;

    #[test]
    fn profile_pda_is_per_core_program_and_has_no_caller_seed() {
        let first = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &Pubkey::new_from_array([1; 32]),
        )
        .0;
        let second = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &Pubkey::new_from_array([2; 32]),
        )
        .0;
        assert_ne!(first, second);
    }

    #[test]
    fn immutable_policy_is_required_at_found() {
        let immutable = ArtifactReleaseV1::new(
            dclutch_release_set_contract::ProgramIdentityV1::new([1; 32]).expect("program"),
            dclutch_release_set_contract::ProgramIdentityV1::new(
                bpf_loader_upgradeable::ID.to_bytes(),
            )
            .expect("loader"),
            [2; 32],
            ContentId::new([3; 32]).expect("semantic"),
            [4; 32],
            5,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("immutable");
        let mutable = ArtifactReleaseV1::new(
            immutable.program(),
            immutable.loader_program(),
            immutable.programdata(),
            immutable.semantic_release_id(),
            immutable.elf_digest(),
            immutable.deployment_slot(),
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([9; 32]),
        )
        .expect("mutable");
        assert_eq!(
            immutable.upgrade_policy(),
            ArtifactUpgradePolicyV1::Immutable
        );
        assert_ne!(mutable.upgrade_policy(), ArtifactUpgradePolicyV1::Immutable);
    }
}
