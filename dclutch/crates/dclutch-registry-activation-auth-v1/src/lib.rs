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
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
    Error as RegistryContractError, RELEASE_LINEAGE_BYTES_V1, RELEASE_LINEAGE_PDA_DOMAIN_V1,
    require_slot_pinned_release_v1, slot_pinned_release_elf_digest_v1,
};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
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
    /// The account is not the Registry-owned lineage record it claims to be.
    ReleaseLineage,
    /// The activated release's pinned deployment slot moved: it was upgraded.
    ///
    /// This is the operator-actionable half of [`Self::Deployment`], and it is
    /// the one refusal decision 0012 made reachable in normal operation: the
    /// substrate's upgrade authority shipped new bytes, so every open market on
    /// the previous generation refuses until a re-release re-authenticates and
    /// re-pins. Callers surface it under their own banded name rather than
    /// folding it back into the generic deployment refusal.
    ReleaseSuperseded,
}

impl From<ActivationAuthErrorV1> for ProgramError {
    fn from(value: ActivationAuthErrorV1) -> Self {
        match value {
            ActivationAuthErrorV1::AccountFrame => Self::InvalidArgument,
            ActivationAuthErrorV1::ActivationCache
            | ActivationAuthErrorV1::Deployment
            | ActivationAuthErrorV1::ReleaseLineage
            | ActivationAuthErrorV1::ReleaseSuperseded => Self::InvalidAccountData,
        }
    }
}

type Result<T> = core::result::Result<T, ActivationAuthErrorV1>;

/// Canonical activation-cache bump authenticated by one complete PDA search.
///
/// The field is private so downstream adapters cannot turn an untrusted byte
/// into an authentication shortcut. A caller obtains this witness only from
/// [`authenticate_activated_role_and_bump_v1`], after the searched address has
/// matched the Registry-owned cache. The same adapter can then authenticate
/// additional roles from that exact cache with
/// [`authenticate_activated_role_with_bump_v1`], which reproduces the address
/// with `create_program_address` instead of paying for the same 256-way search
/// again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedActivationCacheBumpV1(u8);

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
    authenticate_activated_role_and_bump_v1(
        registry,
        cache,
        release_set_id,
        role,
        program,
        programdata,
    )
    .map(|(receipt, _)| receipt)
}

/// Authenticate one role and return the canonical cache bump found on the
/// same exact address check.
///
/// This is the first role in a multi-role adapter frame. It performs the sole
/// canonical search and returns an opaque witness that can be reused only by
/// [`authenticate_activated_role_with_bump_v1`].
#[inline(never)]
pub fn authenticate_activated_role_and_bump_v1(
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    release_set_id: &[u8; 32],
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(
    AuthenticatedRoleReceiptV1,
    AuthenticatedActivationCacheBumpV1,
)> {
    let bump = authenticate_activation_cache_bump_v1(registry, cache, release_set_id)?;
    let receipt =
        authenticate_role_in_account(registry, cache, release_set_id, role, program, programdata)?;
    Ok((receipt, bump))
}

/// Authenticate the canonical Registry-owned cache coordinate and return its
/// opaque bump witness without selecting a deployment role.
///
/// Adapters that must first join other immutable cache facts (for example, a
/// Market's selected Core program) use this once, then pass the returned
/// witness to [`authenticate_activated_role_with_bump_v1`] for every role in
/// the same frame. The complete fixed-width hostile decoder runs here, so the
/// witness cannot come from address/owner checks alone.
#[inline(never)]
pub fn authenticate_activation_cache_bump_v1(
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    release_set_id: &[u8; 32],
) -> Result<AuthenticatedActivationCacheBumpV1> {
    let (expected, bump) = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_slice()],
        registry.key,
    );
    if cache.key != &expected {
        return Err(ActivationAuthErrorV1::ActivationCache);
    }
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
    Ok(AuthenticatedActivationCacheBumpV1(bump))
}

/// Authenticate another role from the same cache using a previously
/// authenticated canonical bump.
///
/// A wrong or cross-release witness cannot reproduce `cache.key` and refuses
/// before account bytes are read. This preserves the exact address, owner,
/// fixed-width, hostile-decode, release-set, and deployment checks of
/// [`authenticate_activated_role_v1`]; only the repeated bump search changes.
#[inline(never)]
pub fn authenticate_activated_role_with_bump_v1(
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    release_set_id: &[u8; 32],
    bump: AuthenticatedActivationCacheBumpV1,
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<AuthenticatedRoleReceiptV1> {
    let bump_seed = [bump.0];
    let expected = Pubkey::create_program_address(
        &[
            ACTIVATION_PDA_DOMAIN_V1,
            release_set_id.as_slice(),
            &bump_seed,
        ],
        registry.key,
    )
    .map_err(|_| ActivationAuthErrorV1::ActivationCache)?;
    if cache.key != &expected {
        return Err(ActivationAuthErrorV1::ActivationCache);
    }
    authenticate_role_in_account(registry, cache, release_set_id, role, program, programdata)
}

fn authenticate_role_in_account(
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    release_set_id: &[u8; 32],
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<AuthenticatedRoleReceiptV1> {
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

/// Derive the sole lineage-record address and bump for one predecessor set.
///
/// This is the only place the lineage seeds are spelled. The Registry derives
/// it to create the record and to sign for it; Core derives it to find the
/// successor its market may hop to. Two programs, one derivation — a second
/// spelling is how a route ends up reading an account nobody wrote.
pub fn release_lineage_address_and_bump_v1(
    registry: &Pubkey,
    predecessor_release_set_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            RELEASE_LINEAGE_PDA_DOMAIN_V1,
            predecessor_release_set_id.as_slice(),
        ],
        registry,
    )
}

/// Derive the sole lineage-record address for one predecessor release set.
pub fn release_lineage_address_v1(
    registry: &Pubkey,
    predecessor_release_set_id: &[u8; 32],
) -> Pubkey {
    release_lineage_address_and_bump_v1(registry, predecessor_release_set_id).0
}

/// Require Registry ownership and the one exact lineage-record width.
///
/// Deliberately does not check privileges: the declaration route needs the
/// record writable and the migration route needs it read-only, so each frame
/// states its own, and neither borrows the other's.
pub fn require_lineage_account_v1(registry: &Pubkey, lineage: &AccountInfo<'_>) -> Result<()> {
    if lineage.owner != registry
        || lineage.executable
        || lineage.data_len() != RELEASE_LINEAGE_BYTES_V1
    {
        return Err(ActivationAuthErrorV1::ReleaseLineage);
    }
    Ok(())
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

/// Observe one activated role's current deployment under its slot pin.
///
/// This is the CU wall decision 0012 was ruled to get past, and the whole of
/// the change lives in one arm.
///
/// An `Immutable` release whose observed ProgramData carries no upgrade
/// authority can never be redeployed, so its activation-bound ELF digest is
/// reused rather than re-hashing a megabyte-scale ELF on every action. An
/// `ExactAuthority` release now reuses that digest too — but only while the
/// ProgramData it is looking at still carries the exact deployment slot and the
/// exact upgrade authority the activation bound. The Loader writes the current
/// slot on every `Upgrade` and refuses an `Upgrade` in the deployment's own
/// slot, so slot equality proves the bytes have not moved. That equality is one
/// `u64` compare over an account this frame already carries: no sysvar, no
/// extra account, no hash.
///
/// The instant the substrate is upgraded the slot moves, this returns
/// [`ActivationAuthErrorV1::ReleaseSuperseded`], and every open market on the
/// superseded generation refuses until a re-release re-authenticates and
/// re-pins. `dclutch_registry_contract::slot_pinned_release_elf_digest_v1` owns
/// that argument; this function only supplies it with chain-observed facts.
pub fn cached_role_deployment_observation_v1(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    require_slot_pinned_release_v1(release).map_err(|_| ActivationAuthErrorV1::Deployment)?;
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
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
    // Loader V3's Program account is the runtime owner of this link. The
    // account is executable, Loader-owned, and hostile-parsed above; the
    // Loader follows this exact stored ProgramData address when it executes or
    // upgrades the program. Re-deriving that already-authenticated link pays a
    // PDA search without authenticating an additional fact. Keep the full
    // three-way equality instead: Loader-owned link == activated release ==
    // supplied Loader-owned ProgramData account.
    if program_view.programdata() != release.programdata()
        || program_view.programdata() != programdata.key.to_bytes()
    {
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
    let observed_slot = programdata_view.deployment_slot();
    let elf_digest = slot_pinned_release_elf_digest_v1(release, observed_authority, observed_slot)
        .map_err(|error| match error {
            RegistryContractError::ReleaseSupersededByUpgrade => {
                ActivationAuthErrorV1::ReleaseSuperseded
            }
            _ => ActivationAuthErrorV1::Deployment,
        })?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        carried_programdata,
        bpf_loader_upgradeable::ID.to_bytes(),
        observed_slot,
        elf_digest,
        observed_authority,
    )
    .map_err(|_| ActivationAuthErrorV1::Deployment)
}

#[cfg(test)]
mod tests;
