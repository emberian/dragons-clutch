//! Read-only reauthentication of one current Upgradeable Loader V3 deployment.
//!
//! Extracted from Trading's Execution Strategy V2 admission so an external
//! accelerator linking only the published Shadow callback boundary does not
//! also link Trading's dispatch, strategy admission, and entrypoint modules.
//! This is the single implementation; `dclutch-trading-sbf` calls into it.

use dclutch_registry_contract::{
    ArtifactReleaseV1, DeploymentObservationV1, Error as RegistryContractError,
    require_slot_pinned_release_v1, slot_pinned_release_elf_digest_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use solana_program::{account_info::AccountInfo, hash::hash};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::ShadowAcceleratorAuthErrorV4;

/// Reauthenticate one current Loader V3 deployment by hashing its exact ELF.
///
/// A finalized `ArtifactRelease` record proves only its own content identity.
/// Nothing has bound its `elf_digest` to the account being observed, so this
/// path always hashes the complete observed ELF.  Use
/// `authenticate_activated_current_deployment` only where the Registry
/// activation cache already carries that binding.
pub fn authenticate_current_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), ShadowAcceleratorAuthErrorV4> {
    authenticate_deployment_v2(release, program, programdata, false)
}

/// Reauthenticate one activated role's current deployment without re-hashing.
///
/// `release` must come from the Registry activation cache, where
/// `activate_execution_role_into_v1` already authenticated a chain-observed
/// deployment — including the complete ELF digest — before persisting it. That
/// admitted digest is reused while the release's PIN still holds: an
/// `Immutable` deployment can never be redeployed at all, and an
/// `ExactAuthority` deployment cannot have moved while its observed ProgramData
/// still carries the exact slot the activation bound (decision 0012). Either
/// way, hashing a megabyte-scale ELF on every hot action would recompute an
/// already-authenticated fact.
/// `dclutch_registry_contract::slot_pinned_release_elf_digest_v1` owns that
/// argument and the Registry role batch already relies on it. Identity,
/// ProgramData link, Loader ownership, executability, the exact deployment
/// slot, and the exact upgrade authority are still checked here and again by
/// `authenticate_deployment`. A pin that no longer holds REFUSES — it never
/// falls back to hashing, because on any state the Loader can actually reach
/// the hash would only confirm what the slot already said.
pub fn authenticate_activated_current_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), ShadowAcceleratorAuthErrorV4> {
    authenticate_deployment_v2(release, program, programdata, true)
}

fn authenticate_deployment_v2(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    activation_bound_elf: bool,
) -> Result<(), ShadowAcceleratorAuthErrorV4> {
    if program.is_signer
        || program.is_writable
        || !program.executable
        || programdata.is_signer
        || programdata.is_writable
        || programdata.executable
        || program.key == programdata.key
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
    {
        return Err(ShadowAcceleratorAuthErrorV4::Content);
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    if program_view.programdata() != release.programdata()
        || program_view.programdata() != programdata.key.to_bytes()
    {
        return Err(ShadowAcceleratorAuthErrorV4::Content);
    }
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    let programdata_view = ProgramDataV3View::parse(&programdata_bytes)
        .map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    require_slot_pinned_release_v1(release).map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    let elf_digest = if activation_bound_elf {
        slot_pinned_release_elf_digest_v1(
            release,
            programdata_view.upgrade_authority(),
            programdata_view.deployment_slot(),
        )
        .map_err(|error| match error {
            RegistryContractError::ReleaseSupersededByUpgrade => {
                ShadowAcceleratorAuthErrorV4::ReleaseSuperseded
            }
            _ => ShadowAcceleratorAuthErrorV4::Content,
        })?
    } else {
        hash(programdata_view.elf()).to_bytes()
    };
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
        elf_digest,
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| ShadowAcceleratorAuthErrorV4::Content)?;
    release
        .authenticate_deployment(observation)
        .map_err(|error| match error {
            RegistryContractError::ReleaseSupersededByUpgrade => {
                ShadowAcceleratorAuthErrorV4::ReleaseSuperseded
            }
            // A DISCARDED CAUSE IS A SEARCH, and after the 0012 ruling this
            // cause is the whole argument: no hot route hashes an ELF, so a
            // moved slot is the entire difference between the admitted artifact
            // and a substituted one. Folding it into `Content` published a code
            // with over two thousand sites for the single fact everything now
            // rests on.
            RegistryContractError::DeploymentSlotMismatch => {
                ShadowAcceleratorAuthErrorV4::DeploymentSlotMismatch
            }
            _ => ShadowAcceleratorAuthErrorV4::Content,
        })
}
