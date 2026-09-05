//! Noncyclic immutable authority for Registry and Rent infrastructure.

use core::convert::TryFrom;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    DeploymentObservationV1, require_slot_pinned_release_v1, slot_pinned_release_elf_digest_v1,
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
    frame::{FoundAccounts, InitializeInfrastructureAccounts, ProjectedFoundAccountsV2},
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
        ArtifactAdmissionV1::FirstAdmission,
    )?;
    let rent_binding = authenticate_artifact(
        frame.registry_program.key,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        ArtifactAdmissionV1::FirstAdmission,
    )?;
    if frame.registry_program.key == program_id || frame.rent_program.key == program_id {
        return Err(CoreSbfError::Infrastructure.into());
    }
    let profile = ProtocolInfrastructureProfileV1::new(registry, rent_binding)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    create_profile(program_id, &frame, &rent, profile)?;
    // The same two bindings, committed again at the V2 domain with the genesis
    // sentinels in place of predecessor ids.
    //
    // Since `2951b226` every Core route authenticates the V2 profile and
    // nothing else, so without this a cohort that succeeds nothing stands up
    // complete and can never found: the succession ceremony is the only other
    // writer of a V2, and it needs a predecessor release's bound upgrade
    // authority to consent to a move that a day-old cohort has not made.
    // Measured on cohort-9, which reached activation and then refused its
    // founding sixty transactions deep.
    //
    // Written HERE rather than as a V1-shaped fallback in the readers, because
    // `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §6 refused
    // try-V2-then-V1 by name: two live authentication paths, whose failure mode
    // is the ceremony forgotten and V1 silently still ruling. A genesis V2
    // keeps exactly one authentication path. Vacancy still refuses, and still
    // means the ceremony is owed.
    let genesis = ProtocolInfrastructureProfileV2::genesis(registry, rent_binding)
        .map_err(|_| CoreSbfError::Infrastructure)?;
    create_genesis_profile_v2(program_id, &frame, &rent, genesis)?;
    Ok(())
}

/// Commit the genesis V2 at its own domain, under `create_profile`'s discipline.
///
/// Vacancy is exact here and stays exact: this runs once, in the same
/// instruction that writes the V1, against a PDA nothing has touched. The
/// ceremony's conjunct 6 is what later distinguishes this profile from a
/// succeeded one, and it does that by reading the sentinels rather than by
/// finding the account empty.
fn create_genesis_profile_v2(
    program_id: &Pubkey,
    frame: &InitializeInfrastructureAccounts<'_, '_>,
    rent: &Rent,
    profile: ProtocolInfrastructureProfileV2,
) -> Result<(), solana_program::program_error::ProgramError> {
    let (expected, bump) =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], program_id);
    if frame.genesis_profile.key != &expected
        || frame.genesis_profile.owner != &system_program::ID
        || frame.genesis_profile.data_len() != 0
        || frame.genesis_profile.executable
    {
        return Err(CoreSbfError::Infrastructure.into());
    }
    let required = rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2);
    let top_up = required.saturating_sub(frame.genesis_profile.lamports());
    if top_up > 0 {
        invoke(
            &transfer(frame.payer.key, frame.genesis_profile.key, top_up),
            &[
                frame.payer.clone(),
                frame.genesis_profile.clone(),
                frame.system.clone(),
            ],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let bump_seed = [bump];
    let signer = [
        PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            frame.genesis_profile.key,
            u64::try_from(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2)
                .map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        &[frame.genesis_profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    invoke_signed(
        &assign(frame.genesis_profile.key, program_id),
        &[frame.genesis_profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    let encoded = profile.to_bytes();
    {
        let mut data = frame
            .genesis_profile
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Infrastructure)?;
        if data.len() != encoded.len() {
            return Err(CoreSbfError::Infrastructure.into());
        }
        data.copy_from_slice(&encoded);
    }
    let committed = frame
        .genesis_profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if frame.genesis_profile.owner != program_id
        || ProtocolInfrastructureProfileV2::decode(&committed) != Ok(profile)
    {
        return Err(CoreSbfError::Infrastructure.into());
    }
    Ok(())
}

/// Authenticate the immutable profile and exact current Registry/Rent releases for Found.
#[inline(never)]
pub(crate) fn authenticate_found(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
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
    )?;
    Ok(())
}

/// Authenticate the same infrastructure root for the compact projected route.
#[inline(never)]
pub(crate) fn authenticate_projected_found(
    program_id: &Pubkey,
    frame: &ProjectedFoundAccountsV2<'_, '_>,
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
    )?;
    Ok(())
}

/// Authenticate the immutable infrastructure profile from a non-Found frame.
///
/// The profile is the noncyclic authority root for both programs. Callers may
/// use Registry records only after this observation succeeds.
///
/// **V2 only, and never a fallback.** Every route reaching here reads the
/// succession profile at `dclutch:infrastructure:v2` and nothing else. A
/// try-V2-then-V1 read was considered and refused by
/// `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §6: it would stand up
/// two live authentication paths (the O-005 parallel-authority smell), and
/// its failure mode — the ceremony forgotten and V1 silently still ruling —
/// is the exact silent divergence the succession exists to end. V1 stays on
/// chain byte-identical, a sealed historical record still content-walkable
/// from V2's predecessor artifact ids, and is never again an authority here.
///
/// **What un-refuses this read is a V2 existing, which is no longer only the
/// ceremony.** It used to be: before the ceremony the read refused on vacancy
/// (`CoreSbfError::Infrastructure`, the width check below, a vacant PDA being
/// System-owned and zero-length), and that sentence was true until a genesis
/// cohort had no way to reach V2 at all — cohort-9 stood up complete and could
/// never found. `process_initialize` now commits a genesis V2 alongside the
/// V1, so a cohort that succeeds nothing is foundable on the day it deploys,
/// and this read is unchanged because a genesis profile is simply a V2.
/// Vacancy still refuses, and now means only that initialization has not run.
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
) -> Result<ProtocolInfrastructureProfileV2, CoreSbfError> {
    let expected =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], program_id)
            .0;
    if infrastructure_profile.key != &expected
        || infrastructure_profile.owner != program_id
        || infrastructure_profile.data_len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || infrastructure_profile.executable
        || !funded_rent_persists_v1(infrastructure_profile.lamports())
    {
        return Err(CoreSbfError::Infrastructure);
    }
    let bytes = infrastructure_profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    let profile = ProtocolInfrastructureProfileV2::decode(&bytes)
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
        ArtifactAdmissionV1::AlreadyPinned,
    )?;
    let rent_binding = authenticate_artifact(
        registry_program.key,
        rent_artifact_raw,
        rent_artifact_staging,
        rent_program,
        rent_programdata,
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
    authenticate_immutable_core_release_accounts(
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        release_set_id,
    )
}

/// Require the compact route's selected current Core artifact to remain pinned.
pub(crate) fn authenticate_projected_immutable_core_release(
    frame: &ProjectedFoundAccountsV2<'_, '_>,
    release_set_id: [u8; 32],
) -> Result<(), CoreSbfError> {
    authenticate_immutable_core_release_accounts(
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        release_set_id,
    )
}

fn authenticate_immutable_core_release_accounts(
    activation_cache: &AccountInfo<'_>,
    registry_program: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    core_programdata: &AccountInfo<'_>,
    release_set_id: [u8; 32],
) -> Result<(), CoreSbfError> {
    use dclutch_registry::release_set::ExecutionRoleV1;
    use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};

    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        registry_program.key,
    )
    .0;
    if activation_cache.key != &expected_cache || activation_cache.owner != registry_program.key {
        return Err(CoreSbfError::Infrastructure);
    }
    let cache = activation_cache
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
    require_pinned_deployment(core_program, core_programdata, release)
}

pub(crate) fn authenticate_current_core_upgrade_authority(
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
pub(crate) enum ArtifactAdmissionV1 {
    /// The claimed digest has never been checked against the deployed bytes.
    ///
    /// A finalized artifact-release record is an attacker-publishable
    /// assertion until this happens, so the full ELF is hashed here and only
    /// here. `process_initialize` is the sole site, it runs once per Core under
    /// Core's own Loader upgrade authority, and it is what makes the immutable
    /// profile's pinned record a truthful description of the deployed code.
    FirstAdmission,
    /// The profile already pinned this exact record.
    ///
    /// The profile content-pins the artifact record; the record is
    /// content-addressed and finalized, so its bytes cannot change; and the
    /// slot pin says the deployed ELF has not changed either. Decision 0012
    /// widened that last step and this doc had not caught up — it read "the
    /// record admits `Immutable` with no upgrade authority and the observed
    /// ProgramData must currently carry none", which is one of the two admitted
    /// shapes, not the rule. The rule is `slot_pinned_release_elf_digest_v1`:
    /// an `Immutable` release cannot move at all, and an `ExactAuthority`
    /// release cannot have moved while the observed deployment slot still
    /// equals the slot it bound, because Loader V3 writes the current slot on
    /// every `Upgrade` and refuses one in the deployment's own slot. Either
    /// way the digest checked at first admission is still exact, and re-hashing
    /// a multi-hundred-kilobyte ELF on every Found recomputes an already
    /// authenticated fact. The deployment slot, identity, link, ownership, and
    /// authority are all still rechecked.
    AlreadyPinned,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_artifact(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    admission: ArtifactAdmissionV1,
) -> Result<ExecutionRoleBindingV1, CoreSbfError> {
    let (binding, _) =
        authenticate_artifact_release(registry, raw, staging, program, programdata, admission)?;
    Ok(binding)
}

/// [`authenticate_artifact`], also returning the decoded release record.
///
/// The succession ceremony needs the record's own facts (its deployment slot
/// for the forward-only conjunct) beside the binding; every check is
/// identical.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_artifact_release(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    admission: ArtifactAdmissionV1,
) -> Result<(ExecutionRoleBindingV1, ArtifactReleaseV1), CoreSbfError> {
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
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        digest,
        &bytes,
    )?;
    let release = ArtifactReleaseV1::decode(&bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    if release.program().to_bytes() != program.key.to_bytes() {
        return Err(CoreSbfError::Infrastructure);
    }
    require_slot_pinned_release_v1(release).map_err(|_| CoreSbfError::Infrastructure)?;
    match admission {
        ArtifactAdmissionV1::FirstAdmission => {
            require_current_deployment(program, programdata, release)?;
        }
        ArtifactAdmissionV1::AlreadyPinned => {
            require_pinned_deployment(program, programdata, release)?;
        }
    }
    let artifact = ArtifactReleaseIdV1::new(digest).map_err(|_| CoreSbfError::Infrastructure)?;
    Ok((
        ExecutionRoleBindingV1::new(release.program(), artifact),
        release,
    ))
}

/// Observe a pinned deployment whose ELF digest was already authenticated.
///
/// This is strictly stronger than the hashing path, never weaker.
/// `slot_pinned_release_elf_digest_v1` refuses unless the release is one of the
/// two canonical pinned shapes AND its pin still holds against this exact
/// observation — for `Immutable`, that no authority was ever retained; for
/// `ExactAuthority` (decision 0012), that the observed ProgramData still
/// carries the exact bound authority and the exact bound deployment slot.
/// Everything else `authenticate_deployment` checks is unchanged: program and
/// ProgramData identity, the Loader link, both owners, executability, the exact
/// deployment slot, and the upgrade authority. Only the recomputation of a
/// digest that provably cannot have changed is dropped.
///
/// Callers must have an admission argument for the digest. Today those are the
/// Registry activation cache and the Core infrastructure profile. First
/// admission belongs to [`require_current_deployment`].
fn require_pinned_deployment(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<(), CoreSbfError> {
    let linkage = require_loader_linkage(program, programdata, release)?;
    let elf_digest = slot_pinned_release_elf_digest_v1(
        release,
        linkage.upgrade_authority,
        linkage.deployment_slot,
    )
    .map_err(pinned_deployment_refusal)?;
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        linkage.programdata_link,
        bpf_loader_upgradeable::ID.to_bytes(),
        linkage.deployment_slot,
        elf_digest,
        linkage.upgrade_authority,
    )
    .map_err(|_| CoreSbfError::Infrastructure)?;
    release
        .authenticate_deployment(observation)
        .map_err(pinned_deployment_refusal)
}

/// Name a pinned-deployment refusal, keeping the superseded case operator-legible.
///
/// Every other reason folds into `Infrastructure`, which is what an operator
/// wants for "the profile and the chain disagree". A moved slot is different:
/// it is the expected consequence of upgrading the substrate, and its remedy is
/// a re-release rather than an investigation (decision 0012).
///
/// Which remedy, though, depends on WHICH account moved, and for eight cohorts
/// this comment named one that did not exist here. Decision 0012's
/// re-release-then-reactivate remedy belongs to the five cache-pinned roles,
/// whose selection a later release set can rewrite. The infrastructure profile
/// is write-once by vacancy with no second write route, so a Registry or Rent
/// upgrade left every route reading it refusing with nothing to re-release INTO
/// (P-008). The profile's actual remedy is the succession ceremony in
/// `infrastructure_v2.rs`: a new profile version at its own domain, naming the
/// records it succeeded. So an operator reading `ReleaseSuperseded` from a
/// profile-backed route should reach for that ceremony, not for a re-release.
const fn pinned_deployment_refusal(error: dclutch_registry::Error) -> CoreSbfError {
    match error {
        dclutch_registry::Error::ReleaseSupersededByUpgrade => CoreSbfError::ReleaseSuperseded,
        _ => CoreSbfError::Infrastructure,
    }
}

/// Loader V3 facts observed without hashing the ELF.
struct LoaderLinkageV1 {
    /// ProgramData link recorded by the Program account itself.
    programdata_link: [u8; 32],
    /// Deployment slot recorded by the ProgramData account.
    deployment_slot: u64,
    /// Upgrade authority the ProgramData account currently carries.
    upgrade_authority: Option<[u8; 32]>,
}

/// Hostile-check Loader V3 shape and linkage without hashing the ELF.
fn require_loader_linkage(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<LoaderLinkageV1, CoreSbfError> {
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
    Ok(LoaderLinkageV1 {
        programdata_link: link,
        deployment_slot: programdata_view.deployment_slot(),
        upgrade_authority: programdata_view.upgrade_authority(),
    })
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
        .map_err(pinned_deployment_refusal)
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

/// Lean-decided decision cases for the decision-0012 slot pin.
///
/// `DClutchSemantics.ProtocolInfrastructure` owns the rule; the Rust below
/// stays a hand-written mirror. This corpus is what makes the two answerable
/// to each other, and the guard in `tests/` is what stops it rotting.
#[cfg(test)]
#[path = "generated_slot_pin_corpus.rs"]
#[allow(dead_code, missing_docs)]
mod generated_slot_pin_corpus;

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_core_contract::ContentId;

    use super::*;
    use dclutch_registry::ArtifactUpgradePolicyV1;

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
            dclutch_registry::release_set::ProgramIdentityV1::new(program_key.to_bytes())
                .expect("program"),
            dclutch_registry::release_set::ProgramIdentityV1::new(
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

    /// Replay every Lean-decided slot-pin case through the real adapter.
    ///
    /// `ProtocolInfrastructure.lean` proves the rule; `require_pinned_deployment`
    /// mirrors it by hand, and until now nothing checked one against the other.
    /// Each vector is built into a genuine Loader V3 observation -- a Program
    /// account carrying its ProgramData link, and a ProgramData account carrying
    /// the observed slot and authority -- and the outcome Lean decided is
    /// asserted against the code the adapter actually returns.
    ///
    /// The two non-canonical pairings are replayed at the release CONSTRUCTOR
    /// rather than at the pin: `ArtifactReleaseV1::new` validates the pairing,
    /// so Rust refuses those records a gate earlier than Lean's
    /// `canonicalReleaseShape` does. Same verdict, earlier gate, and the corpus
    /// says which.
    #[test]
    fn lean_slot_pin_corpus_replays_through_the_adapter() {
        use generated_slot_pin_corpus::{
            SLOT_PIN_OUTCOME_ADMIT, SLOT_PIN_OUTCOME_REFUSE_INFRASTRUCTURE,
            SLOT_PIN_OUTCOME_REFUSE_SUPERSEDED, SLOT_PIN_VECTORS_V1,
        };

        let elf = [0xa5_u8; 96];
        let mut admitted = 0_usize;
        let mut superseded = 0_usize;
        let mut infrastructure = 0_usize;
        let mut noncanonical = 0_usize;

        for vector in SLOT_PIN_VECTORS_V1 {
            let policy = if vector.bound_policy_immutable {
                ArtifactUpgradePolicyV1::Immutable
            } else {
                ArtifactUpgradePolicyV1::ExactAuthority
            };
            let bound_authority = vector.bound_authority.map(|fill| [fill; 32]);
            let observed_authority = vector.observed_authority.map(|fill| [fill; 32]);

            let program_key = Pubkey::new_from_array([11; 32]);
            let programdata_key =
                Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID)
                    .0;
            let built = ArtifactReleaseV1::new(
                dclutch_registry::release_set::ProgramIdentityV1::new(program_key.to_bytes())
                    .expect("program"),
                dclutch_registry::release_set::ProgramIdentityV1::new(
                    bpf_loader_upgradeable::ID.to_bytes(),
                )
                .expect("loader"),
                programdata_key.to_bytes(),
                ContentId::new([3; 32]).expect("semantic"),
                hash(&elf).to_bytes(),
                vector.bound_slot,
                policy,
                bound_authority,
            );

            if !vector.canonical_release_shape {
                assert!(
                    built.is_err(),
                    "{}: a non-canonical pairing must not become a release",
                    vector.name
                );
                assert_ne!(
                    vector.outcome, SLOT_PIN_OUTCOME_ADMIT,
                    "{}: Lean must refuse what Rust cannot construct",
                    vector.name
                );
                noncanonical += 1;
                continue;
            }

            let release = built.expect("canonical release");
            let program = account(
                program_key,
                loader_program_bytes(programdata_key),
                bpf_loader_upgradeable::ID,
                true,
            );
            let programdata =
                programdata_account(release, vector.observed_slot, observed_authority, &elf);
            let observed = require_pinned_deployment(&program, &programdata, release);

            let expected = if vector.outcome == SLOT_PIN_OUTCOME_ADMIT {
                admitted += 1;
                Ok(())
            } else if vector.outcome == SLOT_PIN_OUTCOME_REFUSE_SUPERSEDED {
                superseded += 1;
                Err(CoreSbfError::ReleaseSuperseded)
            } else {
                assert_eq!(vector.outcome, SLOT_PIN_OUTCOME_REFUSE_INFRASTRUCTURE);
                infrastructure += 1;
                Err(CoreSbfError::Infrastructure)
            };
            assert_eq!(observed, expected, "{}", vector.name);
        }

        // The corpus is not one answer repeated: Lean's own coverage theorem
        // says all three outcomes occur, and this is that claim arriving.
        assert_eq!(
            (admitted, superseded, infrastructure, noncanonical),
            (2, 2, 4, 2)
        );
    }

    /// The pinned fast path is strictly stronger than hashing, never weaker.
    ///
    /// Both paths accept exactly the canonical deployment. Only first admission
    /// checks the record's *claimed* digest against the deployed bytes, and it
    /// must keep doing so. The pinned path additionally requires a canonical
    /// pinned release shape and that the pin still holds against this exact
    /// observation — neither of which the hashing path demanded on its own.
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
            require_pinned_deployment(&program, &canonical, release),
            Ok(())
        );

        // An `Immutable` release over ProgramData that retained an authority is
        // still refused: that release's pin claims irrevocability it does not
        // have. Decision 0012 relaxed which releases are admissible, not which
        // observations satisfy a given release.
        let live_authority = programdata_account(release, 7, Some([0x42; 32]), &elf);
        assert_eq!(
            require_pinned_deployment(&program, &live_authority, release),
            Err(CoreSbfError::Infrastructure)
        );

        // The deployment slot is still rechecked on the pinned path. For an
        // `Immutable` release a moved slot is a substituted observation, not an
        // upgrade, so it keeps the generic name.
        let stale = programdata_account(release, 8, None, &elf);
        assert_eq!(
            require_pinned_deployment(&program, &stale, release),
            Err(CoreSbfError::Infrastructure)
        );
        assert_eq!(
            require_current_deployment(&program, &stale, release),
            Err(CoreSbfError::Infrastructure)
        );
    }

    /// Decision 0012: a mutable deployment is admitted while its slot pin holds.
    ///
    /// This is the whole iteration substrate in one test. The same
    /// `ExactAuthority` release is accepted against the ProgramData its
    /// activation observed, and refused by name the moment an `Upgrade` moves
    /// the slot out from under it. Both directions matter: without the first,
    /// the market life cannot fit under the compute ceiling on a mutable
    /// substrate; without the second, a mutable substrate would be unsound.
    #[test]
    fn slot_pinned_mutable_deployment_is_admitted_and_a_moved_slot_is_superseded() {
        let elf = [0xa5_u8; 96];
        let authority = [0x42_u8; 32];
        let (program, release) = deployment(
            &elf,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        );

        // Positive: the exact observed authority at the exact pinned slot.
        let pinned = programdata_account(release, 7, Some(authority), &elf);
        assert_eq!(
            require_pinned_deployment(&program, &pinned, release),
            Ok(())
        );

        // The upgrade lands. The Loader wrote a strictly later slot, and every
        // reader of this release generation now refuses BY NAME, with a remedy
        // in the name: re-release. The bytes here are deliberately UNCHANGED --
        // the refusal is the slot, not a digest comparison, which is exactly
        // what makes it cost one `u64` compare instead of a megabyte hash.
        let upgraded = programdata_account(release, 9, Some(authority), &elf);
        assert_eq!(
            require_pinned_deployment(&program, &upgraded, release),
            Err(CoreSbfError::ReleaseSuperseded)
        );

        // And with genuinely different bytes at the later slot, the same
        // refusal arrives at the same cost: the pin is checked before anything
        // would have hashed.
        let replaced = programdata_account(release, 9, Some(authority), &[0x5a_u8; 96]);
        assert_eq!(
            require_pinned_deployment(&program, &replaced, release),
            Err(CoreSbfError::ReleaseSuperseded)
        );

        // HOSTILE: pin substitution. A different authority at the pinned slot
        // is a substituted ProgramData, not an upgrade, and keeps the generic
        // refusal rather than borrowing the operator-actionable one.
        let substituted = programdata_account(release, 7, Some([0x43; 32]), &elf);
        assert_eq!(
            require_pinned_deployment(&program, &substituted, release),
            Err(CoreSbfError::Infrastructure)
        );

        // HOSTILE: a revoked authority at the pinned slot. `SetAuthority` moves
        // no slot, so the pin still "holds" -- and the release's own identity
        // contract refuses anyway.
        let revoked = programdata_account(release, 7, None, &elf);
        assert_eq!(
            require_pinned_deployment(&program, &revoked, release),
            Err(CoreSbfError::Infrastructure)
        );

        // HOSTILE: the same-slot redeploy edge. A slot BELOW the pin cannot be
        // an upgrade (the Loader only ever writes the current slot), so it is a
        // substituted observation and must not be named a supersession.
        let earlier = programdata_account(release, 6, Some(authority), &elf);
        assert_eq!(
            require_pinned_deployment(&program, &earlier, release),
            Err(CoreSbfError::Infrastructure)
        );

        // First admission still hashes, and still agrees on the canonical case.
        assert_eq!(
            require_current_deployment(&program, &pinned, release),
            Ok(())
        );
        assert_eq!(
            require_current_deployment(&program, &replaced, release),
            Err(CoreSbfError::ReleaseSuperseded)
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
            dclutch_registry::release_set::ProgramIdentityV1::new([1; 32]).expect("program"),
            dclutch_registry::release_set::ProgramIdentityV1::new(
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
