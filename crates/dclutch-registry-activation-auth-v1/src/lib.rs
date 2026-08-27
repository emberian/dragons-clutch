#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The one CPI-free authentication of a Registry-owned activation cache.
//!
//! ## Why this crate exists
//!
//! A role adapter that is entered under a Registry continuation runs at CPI
//! depth three or deeper with the Registry itself already on the stack at depth
//! one. Any `RegistryInstructionV1::Reauthenticate` invocation from there is
//! reentrancy, and Solana refuses it outright — so a child that authenticated
//! its release set by calling back into the Registry could not execute at all
//! under the protocol's own designed authentication shape.
//!
//! The fact the Registry returns from `Reauthenticate` is not privileged
//! knowledge held inside the Registry program: it is written in a
//! Registry-OWNED account at a Registry-DERIVED address, and every child frame
//! already carries that account. Reading it directly is the project's settled
//! immutable-Registry fast-path principle applied one level down, and it is the
//! same read Trading already performs with no CPI when it is entered as a
//! continuation.
//!
//! ## What is preserved exactly
//!
//! [`authenticate_activated_role_v1`] performs, in this order, every check
//! `process_reauthenticate` performed:
//!
//! 1. the three-account privilege frame — the cache is a non-signer,
//!    non-writable, non-executable account; the Program is executable and
//!    read-only; the ProgramData is neither executable nor writable;
//! 2. cache OWNERSHIP by the Registry program and its exact fixed width;
//! 3. cache ADDRESS: `[ACTIVATION_PDA_DOMAIN_V1, execution_release_set_id]`
//!    derived under the Registry program, so no account the Registry did not
//!    open for exactly this release set can stand in;
//! 4. the cache HEADER and complete role projection, through the same hostile
//!    [`ActivatedExecutionReleaseSetViewV1::decode`];
//! 5. the activated role's CURRENT DEPLOYMENT, against the exact release the
//!    cache carries, under the release's own upgrade policy.
//!
//! [`authenticate_activated_role_v1`] adds one check the CPI could not make:
//! the caller states the release set (the activation GENERATION) it believes it
//! is running under, and a cache opened for any other release set refuses at
//! its own address rather than at a receipt comparison the caller had to
//! remember to write.
//!
//! `dclutch-registry-sbf` calls [`authenticate_activated_role_in_cache_v1`]
//! from its own `Reauthenticate` handler, so the surviving top-level CPI and
//! every child-local read are the same code and can never drift apart.
//!
//! ## What this crate does not decide
//!
//! It takes the Registry program as an account and trusts nothing about it
//! beyond identity: the caller must already have bound that address to the
//! Registry its Market names. That obligation is unchanged — a CPI to an
//! attacker-supplied "Registry" could forge a receipt just as freely as an
//! attacker-owned account could carry forged cache bytes, and both are refused
//! by the same downstream Market join.

use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
    DeploymentObservationV1, immutable_release_elf_digest_v1,
};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::bpf_loader_upgradeable;

/// Stable refusal from the CPI-free activation-cache authentication.
///
/// Callers map this onto their own adapter taxonomy; the variants exist so a
/// caller that wants to distinguish a malformed frame from a stale deployment
/// can, not because these discriminants are protocol-visible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAuthErrorV1 {
    /// The three supplied accounts were not the exact read-only frame.
    AccountFrame,
    /// The account is not the Registry-owned cache for the stated release set.
    ActivationCache,
    /// The observed deployment is not the one this activation admitted.
    Deployment,
}

impl From<ActivationAuthErrorV1> for ProgramError {
    fn from(value: ActivationAuthErrorV1) -> Self {
        match value {
            ActivationAuthErrorV1::AccountFrame => Self::InvalidArgument,
            ActivationAuthErrorV1::ActivationCache | ActivationAuthErrorV1::Deployment => {
                Self::InvalidAccountData
            }
        }
    }
}

type Result<T> = core::result::Result<T, ActivationAuthErrorV1>;

/// Authenticate one activated role out of the Registry-owned cache, no CPI.
///
/// `release_set_id` is the activation generation the caller is executing under.
/// The cache address is derived from it, so a cache opened for a different
/// release set — including a perfectly valid one belonging to another Market —
/// is refused before a single byte of it is read.
#[inline(never)]
pub fn authenticate_activated_role_v1(
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    release_set_id: &[u8; 32],
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<AuthenticatedRoleReceiptV1> {
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_slice()],
        registry.key,
    )
    .0;
    if cache.key != &expected {
        return Err(ActivationAuthErrorV1::ActivationCache);
    }
    require_readonly_frame(cache, program, programdata)?;
    require_cache_account(registry.key, cache)?;
    let bytes = cache
        .try_borrow_data()
        .map_err(|_| ActivationAuthErrorV1::ActivationCache)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&bytes)
        .map_err(|_| ActivationAuthErrorV1::ActivationCache)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ActivationAuthErrorV1::ActivationCache)?
        .as_bytes()
        != release_set_id
    {
        return Err(ActivationAuthErrorV1::ActivationCache);
    }
    authenticate_role_in_view(activated, role, program, programdata)
}

/// Authenticate one activated role from an already-decoded cache view.
///
/// This is the exact body of the Registry's own `Reauthenticate` handler after
/// its frame and identity checks, and the Registry calls it rather than keeping
/// a second copy. The address check is the CALLER's here: a view is already a
/// decoded account, so this entry cannot re-derive the PDA that admitted it.
#[inline(never)]
pub fn authenticate_activated_role_in_cache_v1(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<AuthenticatedRoleReceiptV1> {
    authenticate_role_in_view(activated, role, program, programdata)
}

/// Derive the sole activation-cache address for one release set.
pub fn activation_cache_address_v1(registry: &Pubkey, release_set_id: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_slice()],
        registry,
    )
    .0
}

/// Require the exact read-only three-account reauthentication frame.
///
/// These are the Registry's own frame checks, moved to where the frame is
/// actually presented. A child that let a writable cache through would be
/// admitting an account another instruction in the same transaction could
/// still mutate.
pub fn require_readonly_frame(
    cache: &AccountInfo<'_>,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<()> {
    if cache.is_signer
        || cache.is_writable
        || cache.executable
        || program.is_signer
        || program.is_writable
        || !program.executable
        || programdata.is_signer
        || programdata.is_writable
        || programdata.executable
        || program.key == programdata.key
    {
        return Err(ActivationAuthErrorV1::AccountFrame);
    }
    Ok(())
}

/// Require Registry ownership and the one exact activation-cache width.
pub fn require_cache_account(registry: &Pubkey, cache: &AccountInfo<'_>) -> Result<()> {
    if cache.owner != registry
        || cache.executable
        || cache.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(ActivationAuthErrorV1::ActivationCache);
    }
    Ok(())
}

fn authenticate_role_in_view(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<AuthenticatedRoleReceiptV1> {
    let activated_role = activated
        .role(role)
        .map_err(|_| ActivationAuthErrorV1::ActivationCache)?;
    let release = activated_role.release();
    let observation = cached_role_deployment_observation_v1(program, programdata, release)?;
    activated_role
        .authenticate_current_deployment(observation)
        .map_err(|_| ActivationAuthErrorV1::Deployment)?;
    Ok(AuthenticatedRoleReceiptV1::new(
        role,
        activated
            .execution_release_set_id()
            .map_err(|_| ActivationAuthErrorV1::ActivationCache)?,
        release.program(),
        activated_role.artifact_release_id(),
        release.semantic_release_id(),
    ))
}

/// Observe one activated role's current deployment under its upgrade policy.
///
/// An `Immutable` release whose observed ProgramData carries no upgrade
/// authority can never be redeployed, so its activation-bound ELF digest is
/// reused rather than re-hashing a megabyte-scale ELF on every action. An
/// `ExactAuthority` release keeps the full current-ELF hash, because its bytes
/// can still move.
pub fn cached_role_deployment_observation_v1(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    let immutable = match release.upgrade_policy() {
        ArtifactUpgradePolicyV1::Immutable => true,
        ArtifactUpgradePolicyV1::ExactAuthority => false,
    };
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || (immutable && release.upgrade_authority().is_some())
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(ActivationAuthErrorV1::Deployment);
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| ActivationAuthErrorV1::Deployment)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| ActivationAuthErrorV1::Deployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata() || programdata.key != &derived {
        return Err(ActivationAuthErrorV1::Deployment);
    }
    let carried_programdata = program_view.programdata();
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| ActivationAuthErrorV1::Deployment)?;
    let programdata_view = ProgramDataV3View::parse(&programdata_bytes)
        .map_err(|_| ActivationAuthErrorV1::Deployment)?;
    let observed_authority = programdata_view.upgrade_authority();
    let (elf_digest, reported_authority) = if immutable {
        (
            immutable_release_elf_digest_v1(release, observed_authority)
                .map_err(|_| ActivationAuthErrorV1::Deployment)?,
            None,
        )
    } else {
        (hash(programdata_view.elf()).to_bytes(), observed_authority)
    };
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        carried_programdata,
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        elf_digest,
        reported_authority,
    )
    .map_err(|_| ActivationAuthErrorV1::Deployment)
}

#[cfg(test)]
mod tests;
