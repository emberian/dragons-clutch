//! Noncyclic immutable authority for Registry and Rent infrastructure.

use core::convert::TryFrom;

use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, immutable_release_elf_digest_v1,
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
        ArtifactAdmissionV1::FirstAdmission,
    )?;
    let rent_binding = authenticate_artifact(
        frame.registry_program.key,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        &rent,
        ArtifactAdmissionV1::FirstAdmission,
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
    authenticate_profile(
        program_id,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        rent,
    )?;
    Ok(())
}

/// Authenticate the immutable infrastructure profile from a non-Found frame.
///
/// The profile is the noncyclic authority root for both programs. Callers may
/// use Registry records only after this observation succeeds.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_profile(
    program_id: &Pubkey,
    infrastructure_profile: &AccountInfo<'_>,
    registry_artifact_raw: &AccountInfo<'_>,
    registry_artifact_staging: &AccountInfo<'_>,
    registry_program: &AccountInfo<'_>,
    registry_programdata: &AccountInfo<'_>,
    rent_artifact_raw: &AccountInfo<'_>,
    rent_artifact_staging: &AccountInfo<'_>,
    rent_program: &AccountInfo<'_>,
    rent_programdata: &AccountInfo<'_>,
    rent: &Rent,
) -> Result<ProtocolInfrastructureProfileV1, CoreSbfError> {
    let expected =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], program_id)
            .0;
    if infrastructure_profile.key != &expected
        || infrastructure_profile.owner != program_id
        || infrastructure_profile.data_len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || infrastructure_profile.executable
        || !rent.is_exempt(
            infrastructure_profile.lamports(),
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
        )
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let bytes = infrastructure_profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let profile = ProtocolInfrastructureProfileV1::decode(&bytes)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if profile.registry().program().to_bytes() != registry_program.key.to_bytes()
        || profile.rent().program().to_bytes() != rent_program.key.to_bytes()
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let registry = authenticate_artifact(
        registry_program.key,
        registry_artifact_raw,
        registry_artifact_staging,
        registry_program,
        registry_programdata,
        rent,
        ArtifactAdmissionV1::AlreadyPinned,
    )?;
    let rent_binding = authenticate_artifact(
        registry_program.key,
        rent_artifact_raw,
        rent_artifact_staging,
        rent_program,
        rent_programdata,
        rent,
        ArtifactAdmissionV1::AlreadyPinned,
    )?;
    if registry != profile.registry() || rent_binding != profile.rent() {
        return Err(CoreSbfError::Infrastructure);
    }
    Ok(profile)
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
    // `release` comes from the Registry activation cache, which hashed this
    // artifact's complete ELF once before persisting it — the same admission
    // argument `dclutch-registry-sbf`'s role batch relies on.
    require_pinned_immutable_deployment(frame.core_program, frame.core_programdata, release)
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

/// Whether this observation is the artifact's first admission or a recurring read.
///
/// The two differ by exactly one fact: whether the complete deployed ELF must
/// be hashed to check the artifact record's *claimed* `elf_digest`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactAdmissionV1 {
    /// The claimed digest has never been checked against the deployed bytes.
    ///
    /// A finalized artifact-release record is an attacker-publishable
    /// assertion until this happens, so the full ELF is hashed here and only
    /// here. `process_initialize` is the sole site, it runs once per Core under
    /// Core's own Loader upgrade authority, and it is what makes the immutable
    /// profile's pinned record a truthful description of the deployed code.
    FirstAdmission,
    /// The immutable profile already pinned this exact record.
    ///
    /// The profile is immutable and content-pins the artifact record; the
    /// record is content-addressed and finalized, so its bytes cannot change;
    /// the record admits `Immutable` with no upgrade authority and the observed
    /// ProgramData must currently carry none, so the deployed ELF cannot change
    /// either. The digest checked at first admission is therefore still exact,
    /// and re-hashing a multi-hundred-kilobyte ELF on every Found recomputes an
    /// already authenticated fact. The deployment slot, identity, link,
    /// ownership, and authority are all still rechecked.
    AlreadyPinned,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_artifact(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    rent: &Rent,
    admission: ArtifactAdmissionV1,
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
        || release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
    {
        return Err(CoreSbfError::Infrastructure);
    }
    match admission {
        ArtifactAdmissionV1::FirstAdmission => {
            require_current_deployment(program, programdata, release)?;
        }
        ArtifactAdmissionV1::AlreadyPinned => {
            require_pinned_immutable_deployment(program, programdata, release)?;
        }
    }
    let artifact = ArtifactReleaseIdV1::new(digest).map_err(|_| CoreSbfError::Infrastructure)?;
    Ok(ExecutionRoleBindingV1::new(release.program(), artifact))
}

/// Observe an immutable deployment whose ELF digest was already authenticated.
///
/// This is strictly stronger than the hashing path, never weaker.
/// `immutable_release_elf_digest_v1` refuses unless the release admits
/// `Immutable`, carries no upgrade authority, and the observed ProgramData
/// currently carries none — three requirements the hashing path did not all
/// make on its own. Everything else `authenticate_deployment` checks is
/// unchanged: program and ProgramData identity, the Loader link, both owners,
/// executability, the exact deployment slot, and the upgrade authority. Only
/// the recomputation of a digest that provably cannot have changed is dropped.
///
/// Callers must have an admission argument for the digest. Today those are the
/// Registry activation cache and the immutable Core infrastructure profile.
/// First admission belongs to [`require_current_deployment`].
fn require_pinned_immutable_deployment(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<(), CoreSbfError> {
    let (program_view, programdata_view_slot, observed_authority) =
        require_loader_linkage(program, programdata, release)?;
    let elf_digest = immutable_release_elf_digest_v1(release, observed_authority)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view,
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view_slot,
        elf_digest,
        None,
    )
    .map_err(|_| CoreSbfError::Infrastructure)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| CoreSbfError::Infrastructure)
}

/// Hostile-check Loader V3 shape and linkage without hashing the ELF.
///
/// Returns the ProgramData link recorded by the Program account, the observed
/// deployment slot, and the observed upgrade authority.
fn require_loader_linkage(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<([u8; 32], u64, Option<[u8; 32]>), CoreSbfError> {
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
    let link = program_view.programdata();
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    Ok((
        link,
        programdata_view.deployment_slot(),
        programdata_view.upgrade_authority(),
    ))
}

/// Observe a deployment by hashing its complete current ELF tail.
///
/// This is first admission and the sole site that checks a finalized artifact
/// record's *claimed* `elf_digest` against the bytes actually deployed. It must
/// never be replaced by a pinned-digest fast path.
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
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

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

    fn account(
        key: Pubkey,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(1_u64)),
            std::boxed::Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut output = vec![0_u8; 36];
        output
            .get_mut(..4)
            .expect("variant bytes")
            .copy_from_slice(&2_u32.to_le_bytes());
        output
            .get_mut(4..36)
            .expect("ProgramData link bytes")
            .copy_from_slice(programdata.as_ref());
        output
    }

    fn programdata_bytes(slot: u64, authority: Option<[u8; 32]>, elf: &[u8]) -> Vec<u8> {
        let mut output = vec![0_u8; 45 + elf.len()];
        output
            .get_mut(..4)
            .expect("variant bytes")
            .copy_from_slice(&3_u32.to_le_bytes());
        output
            .get_mut(4..12)
            .expect("slot bytes")
            .copy_from_slice(&slot.to_le_bytes());
        if let Some(authority) = authority {
            *output.get_mut(12).expect("tag byte") = 1;
            output
                .get_mut(13..45)
                .expect("authority bytes")
                .copy_from_slice(&authority);
        }
        output
            .get_mut(45..)
            .expect("ELF bytes")
            .copy_from_slice(elf);
        output
    }

    /// One canonical immutable Loader V3 deployment and its exact release.
    fn deployment(
        elf: &[u8],
        policy: ArtifactUpgradePolicyV1,
        recorded_authority: Option<[u8; 32]>,
    ) -> (AccountInfo<'static>, ArtifactReleaseV1) {
        let program_key = Pubkey::new_from_array([11; 32]);
        let programdata_key =
            Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
        let release = ArtifactReleaseV1::new(
            dclutch_release_set_contract::ProgramIdentityV1::new(program_key.to_bytes())
                .expect("program"),
            dclutch_release_set_contract::ProgramIdentityV1::new(
                bpf_loader_upgradeable::ID.to_bytes(),
            )
            .expect("loader"),
            programdata_key.to_bytes(),
            ContentId::new([3; 32]).expect("semantic"),
            hash(elf).to_bytes(),
            7,
            policy,
            recorded_authority,
        )
        .expect("release");
        let program = account(
            program_key,
            loader_program_bytes(programdata_key),
            bpf_loader_upgradeable::ID,
            true,
        );
        (program, release)
    }

    fn programdata_account(
        release: ArtifactReleaseV1,
        slot: u64,
        authority: Option<[u8; 32]>,
        elf: &[u8],
    ) -> AccountInfo<'static> {
        account(
            Pubkey::new_from_array(release.programdata()),
            programdata_bytes(slot, authority, elf),
            bpf_loader_upgradeable::ID,
            false,
        )
    }

    /// The pinned fast path is strictly stronger than hashing, never weaker.
    ///
    /// Both paths accept exactly the canonical immutable deployment. Only first
    /// admission checks the record's *claimed* digest against the deployed
    /// bytes, and it must keep doing so. The pinned path additionally requires
    /// the immutable policy, an absent recorded authority, and an absent live
    /// authority — none of which the hashing path demanded on its own.
    #[test]
    fn pinned_immutable_observation_is_stronger_than_hashing_and_agrees_on_the_canonical_case() {
        let elf = [0xa5_u8; 96];
        let (program, release) = deployment(&elf, ArtifactUpgradePolicyV1::Immutable, None);
        let canonical = programdata_account(release, 7, None, &elf);

        assert_eq!(
            require_current_deployment(&program, &canonical, release),
            Ok(())
        );
        assert_eq!(
            require_pinned_immutable_deployment(&program, &canonical, release),
            Ok(())
        );

        // A live upgrade authority means the bytes can still change. The pinned
        // path has no admission argument left and must refuse.
        let live_authority = programdata_account(release, 7, Some([0x42; 32]), &elf);
        assert_eq!(
            require_pinned_immutable_deployment(&program, &live_authority, release),
            Err(CoreSbfError::Infrastructure)
        );

        // An upgradeable release never earns the pinned path.
        let (upgradeable_program, upgradeable) = deployment(
            &elf,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([0x42; 32]),
        );
        let upgradeable_data =
            programdata_account(upgradeable, 7, Some([0x42; 32]), &elf);
        assert_eq!(
            require_pinned_immutable_deployment(
                &upgradeable_program,
                &upgradeable_data,
                upgradeable
            ),
            Err(CoreSbfError::Infrastructure)
        );

        // The deployment slot is still rechecked on the pinned path: an upgrade
        // that somehow moved the bytes would also move the slot.
        let stale = programdata_account(release, 8, None, &elf);
        assert_eq!(
            require_pinned_immutable_deployment(&program, &stale, release),
            Err(CoreSbfError::Infrastructure)
        );
        assert_eq!(
            require_current_deployment(&program, &stale, release),
            Err(CoreSbfError::Infrastructure)
        );
    }

    /// First admission is the sole site that checks a claimed digest.
    ///
    /// A finalized artifact-release record can claim any `elf_digest` until the
    /// deployed bytes are hashed against it. `process_initialize` must keep
    /// hashing, and this pins that it still refuses substituted bytes.
    #[test]
    fn first_admission_still_refuses_substituted_deployed_bytes() {
        let elf = [0xa5_u8; 96];
        let (program, release) = deployment(&elf, ArtifactUpgradePolicyV1::Immutable, None);
        let substituted = programdata_account(release, 7, None, &[0x5a_u8; 96]);
        assert_eq!(
            require_current_deployment(&program, &substituted, release),
            Err(CoreSbfError::Infrastructure)
        );
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
