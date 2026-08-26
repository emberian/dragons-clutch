#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Thin successor Registry activation and deployment-reauthentication adapter.
//!
//! This program is the sole writer of the release-set activation cache. It
//! authenticates finalized headerless records, parses current Loader V3 state,
//! hashes the complete deployed ELF tail of the role being admitted, invokes
//! the SDK-free Registry contract once, and persists only that derived result.
//! Capability programs may CPI into the read-only reauthentication route and
//! consume its fixed return-data receipt after checking this program as the
//! producer.
//!
//! Activation admits **one role per transaction**. Whole-ELF hashing costs
//! about one compute unit per two bytes, so admitting five real multi-hundred-
//! kilobyte artifacts in one transaction cannot fit under the chain compute
//! maximum. The activation cache was already an incrementally written,
//! idempotent, alias-checked buffer, and a partially written cache cannot
//! `decode`, so no reader can consume a half-activated release set.

extern crate std;

use core::convert::TryFrom;

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, immutable_release_elf_digest_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::{
    AuthenticatedRoleReceiptV1, ProgramDataV3View, ProgramV3View,
    REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1, RegistryInstructionV1,
};
use dclutch_release_set_contract::{
    EXECUTION_RELEASE_SET_BYTES_V1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::hash,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

mod batch_v2;
mod continuation_v1;
mod hot_continuation_v2;
mod record_v1;

/// Exact account count for one read-only role reauthentication.
pub const REAUTHENTICATE_ACCOUNT_COUNT_V1: usize = 3;

/// Stable Registry SBF refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// Instruction bytes were not the one canonical Registry wire.
    Instruction = 0,
    /// Account count, order, privilege, or aliasing was invalid.
    AccountFrame = 1,
    /// A finalized record owner, digest, PDA, rent, or vacancy proof refused.
    FinalizedRecord = 2,
    /// Loader Program, ProgramData, linkage, slot, ELF, or authority refused.
    Deployment = 3,
    /// Release-set or artifact semantic admission refused.
    Release = 4,
    /// The activation cache owner, PDA, bytes, or lifecycle refused.
    ActivationCache = 5,
    /// System account creation failed or produced the wrong account.
    CreateCpi = 6,
    /// Account data could not be borrowed.
    Borrow = 7,
    /// Checked width or lamport arithmetic refused.
    Arithmetic = 8,
    /// Clock-independent Rent or native-program authentication refused.
    Sysvar = 9,
    /// A batched request or receipt failed its canonical fixed-width contract.
    Batch = 10,
    /// Registry-authenticated continuation header, signer, or child refused.
    Continuation = 11,
    /// Immutable-record publication wire, frame, transition, or account refused.
    Record = 12,
}

impl From<RegistryError> for ProgramError {
    fn from(value: RegistryError) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct RoleFrame<'accounts, 'info> {
    artifact_record: &'accounts AccountInfo<'info>,
    artifact_staging: &'accounts AccountInfo<'info>,
    program: &'accounts AccountInfo<'info>,
    programdata: &'accounts AccountInfo<'info>,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Dispatch one exact activation or reauthentication request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.get(..8)
        == Some(dclutch_capability_program_contract::hot_v3::HOT_EXECUTION_MAGIC_V3.as_slice())
    {
        return hot_continuation_v2::process(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..8)
        == Some(dclutch_record_contract::RECORD_INSTRUCTION_MAGIC_V1.as_slice())
        && (instruction_data
            .get(10)
            .copied()
            .is_some_and(|action| action >= 2)
            || instruction_data.len() != dclutch_registry_svm::REGISTRY_INSTRUCTION_BYTES_V1)
    {
        return record_v1::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..8)
        == Some(
            dclutch_registry_svm::continuation_v1::REGISTRY_CONTINUATION_REQUEST_MAGIC_V1
                .as_slice(),
        )
    {
        return continuation_v1::process(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..8)
        == Some(dclutch_registry_svm::batch_v2::ROLE_BATCH_REQUEST_MAGIC_V2.as_slice())
    {
        return batch_v2::process(program_id, accounts, instruction_data);
    }
    match RegistryInstructionV1::decode(instruction_data).map_err(|_| RegistryError::Instruction)? {
        RegistryInstructionV1::ActivateRole(role) => {
            process_activate_role(program_id, accounts, role)
        }
        RegistryInstructionV1::Reauthenticate(role) => {
            process_reauthenticate(program_id, accounts, role)
        }
    }
}

#[inline(never)]
fn process_activate_role(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    role: ExecutionRoleV1,
) -> ProgramResult {
    if accounts.len() != REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1 {
        return Err(RegistryError::AccountFrame.into());
    }
    let mut iterator = accounts.iter();
    let payer = next(&mut iterator)?;
    let cache = next(&mut iterator)?;
    let release_set_record = next(&mut iterator)?;
    let release_set_staging = next(&mut iterator)?;
    let frame = role_frame(&mut iterator)?;
    let system = next(&mut iterator)?;
    let rent_sysvar = next(&mut iterator)?;
    validate_activate_role_privileges(payer, cache, system, rent_sysvar, frame)?;
    let rent = authenticate_rent_and_system(system, rent_sysvar)?;
    let (release_set_id, release_set) =
        authenticate_release_set_record(program_id, release_set_record, release_set_staging, &rent)?;
    let created =
        ensure_activation_cache_account(program_id, payer, cache, system, &rent, release_set_id)?;
    let mut output = cache
        .try_borrow_mut_data()
        .map_err(|_| RegistryError::Borrow)?;
    if created {
        initialize_activation_cache_v1(&mut output, release_set_id)
            .map_err(|_| RegistryError::ActivationCache)?;
    }
    // `activate_execution_role_into_v1` revalidates the cache header and refuses
    // unless the buffer already names exactly `release_set_id`, so a cache opened
    // for one release set can never accumulate roles from another. It also refuses
    // a conflicting rewrite of this slot or of any aliased role.
    activate_and_write_role(
        program_id,
        &mut output,
        release_set_id,
        &release_set,
        &rent,
        role,
        frame,
    )?;
    require_consistent_completion(&output, release_set_id, &release_set)
}

/// Refuse a *completed* cache that does not project to the selected release set.
///
/// A cache with any role slot still unwritten cannot `decode`, so no reader can
/// consume it; leaving one in place between activation transactions is the whole
/// point of per-role admission. Once the final role lands the buffer becomes
/// readable, and at that instant it must name exactly the finalized release set
/// this transaction authenticated. Every slot was written under that same
/// selection, so this is a belt on a fact the write path already enforces, not
/// the safety argument for it.
fn require_consistent_completion(
    output: &[u8],
    release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
) -> ProgramResult {
    let Ok(completed) = ActivatedExecutionReleaseSetViewV1::decode(output) else {
        return Ok(());
    };
    if completed
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?
        != release_set_id
        || completed
            .release_set_projection()
            .map_err(|_| RegistryError::ActivationCache)?
            != *release_set
    {
        return Err(RegistryError::ActivationCache.into());
    }
    Ok(())
}

#[inline(never)]
fn process_reauthenticate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    role: ExecutionRoleV1,
) -> ProgramResult {
    if accounts.len() != REAUTHENTICATE_ACCOUNT_COUNT_V1 {
        return Err(RegistryError::AccountFrame.into());
    }
    let mut iterator = accounts.iter();
    let cache = next(&mut iterator)?;
    let program = next(&mut iterator)?;
    let programdata = next(&mut iterator)?;
    if cache.is_signer
        || cache.is_writable
        || cache.executable
        || program.is_signer
        || program.is_writable
        || !program.executable
        || programdata.is_signer
        || programdata.is_writable
        || programdata.executable
    {
        return Err(RegistryError::AccountFrame.into());
    }
    let cache_data = cache.try_borrow_data().map_err(|_| RegistryError::Borrow)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| RegistryError::ActivationCache)?;
    authenticate_cache_identity(program_id, cache, activated)?;
    let activated_role = activated
        .role(role)
        .map_err(|_| RegistryError::ActivationCache)?;
    let release = activated_role.release();
    let observation = cached_role_deployment_observation(program, programdata, release)?;
    activated_role
        .authenticate_current_deployment(observation)
        .map_err(|_| RegistryError::Deployment)?;
    let receipt = AuthenticatedRoleReceiptV1::new(
        role,
        activated
            .execution_release_set_id()
            .map_err(|_| RegistryError::ActivationCache)?,
        release.program(),
        activated_role.artifact_release_id(),
        release.semantic_release_id(),
    );
    set_return_data(&receipt.to_bytes());
    Ok(())
}

fn authenticate_release_set_record(
    program_id: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
) -> Result<(ContentId, ExecutionReleaseSetV1), ProgramError> {
    let data = raw.try_borrow_data().map_err(|_| RegistryError::Borrow)?;
    if data.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(RegistryError::FinalizedRecord.into());
    }
    let digest = hash(&data).to_bytes();
    authenticate_finalized_record(
        program_id,
        raw,
        staging,
        rent,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        digest,
        &data,
    )?;
    let release_set = ExecutionReleaseSetV1::decode(&data).map_err(|_| RegistryError::Release)?;
    let release_set_id = ContentId::new(digest).map_err(|_| RegistryError::Release)?;
    Ok((release_set_id, release_set))
}

fn authenticate_artifact_role(
    program_id: &Pubkey,
    expected: ExecutionRoleBindingV1,
    rent: &Rent,
    frame: RoleFrame<'_, '_>,
) -> Result<ArtifactActivationInputV1, ProgramError> {
    let data = frame
        .artifact_record
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    if data.len() != ARTIFACT_RELEASE_BYTES_V1
        || hash(&data).to_bytes() != expected.artifact_release().to_bytes()
    {
        return Err(RegistryError::FinalizedRecord.into());
    }
    authenticate_finalized_record(
        program_id,
        frame.artifact_record,
        frame.artifact_staging,
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        expected.artifact_release().to_bytes(),
        &data,
    )?;
    let release = ArtifactReleaseV1::decode(&data).map_err(|_| RegistryError::Release)?;
    if release.program() != expected.program() {
        return Err(RegistryError::Release.into());
    }
    drop(data);
    let observation = deployment_observation(frame.program, frame.programdata, release)?;
    Ok(ArtifactActivationInputV1::new(
        expected.artifact_release(),
        release,
        observation,
    ))
}

/// Observe one deployment already admitted into the activation cache.
///
/// Activation hashed this artifact's complete ELF once, before the cache
/// persisted `release`. `immutable_release_elf_digest_v1` owns the argument
/// that an immutable Loader V3 deployment's admitted digest is therefore still
/// its exact current digest: the release must be `Immutable`, carry no upgrade
/// authority, and the observed ProgramData must currently carry none either.
/// Re-hashing a multi-hundred-kilobyte ELF on every recurring reauthentication
/// recomputes an already authenticated fact, and at about one compute unit per
/// two bytes that single hash was the whole reason canonical Found exceeded the
/// chain compute maximum.
///
/// This is strictly stronger than hashing, not weaker: the fast path *requires*
/// the immutable policy and an absent live upgrade authority, which the hashing
/// path never demanded on its own. An `ExactAuthority` release has no such
/// guarantee and keeps the full current-ELF hash. Identity, link, ownership,
/// executability, deployment slot, and authority are rechecked either way by
/// `authenticate_deployment`.
fn cached_role_deployment_observation(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, ProgramError> {
    match release.upgrade_policy() {
        ArtifactUpgradePolicyV1::Immutable => {
            immutable_deployment_observation(program, programdata, release)
        }
        ArtifactUpgradePolicyV1::ExactAuthority => {
            deployment_observation(program, programdata, release)
        }
    }
}

/// Observe one immutable deployment without re-hashing its complete ELF.
///
/// Only callable for a release the activation cache already admitted; see
/// [`cached_role_deployment_observation`] for the argument. First admission
/// must still use [`deployment_observation`], because the claimed digest of a
/// finalized artifact-release record is checked against the deployed bytes
/// exactly once, there.
fn immutable_deployment_observation(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, ProgramError> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.upgrade_authority().is_some()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(RegistryError::Deployment.into());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| RegistryError::Deployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata() || programdata.key != &derived {
        return Err(RegistryError::Deployment.into());
    }
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| RegistryError::Deployment)?;
    let elf_digest = immutable_release_elf_digest_v1(release, programdata_view.upgrade_authority())
        .map_err(|_| RegistryError::Deployment)?;
    DeploymentObservationV1::new(
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
        None,
    )
    .map_err(|_| RegistryError::Deployment.into())
}

/// Observe one deployment by hashing its complete current ELF tail.
///
/// This is first admission: the claimed `elf_digest` of a finalized
/// artifact-release record is an attacker-publishable assertion until it is
/// checked against the bytes actually deployed, and this is the sole site that
/// checks it. It must never be replaced by a cached-digest fast path.
fn deployment_observation(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, ProgramError> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(RegistryError::Deployment.into());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| RegistryError::Deployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata() || programdata.key != &derived {
        return Err(RegistryError::Deployment.into());
    }
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| RegistryError::Deployment)?;
    DeploymentObservationV1::new(
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
    .map_err(|_| RegistryError::Deployment.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_finalized_record(
    program_id: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema_id: [u8; 32],
    digest: [u8; 32],
    exact_content: &[u8],
) -> ProgramResult {
    if raw.owner != program_id
        || raw.executable
        || !rent.is_exempt(raw.lamports(), exact_content.len())
        || hash(exact_content).to_bytes() != digest
    {
        return Err(RegistryError::FinalizedRecord.into());
    }
    let raw_pda =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema_id, &digest], program_id).0;
    let staging_pda = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema_id, &digest],
        program_id,
    )
    .0;
    if raw.key != &raw_pda
        || staging.key != &staging_pda
        || staging.owner != &system_program::ID
        || staging.lamports() != 0
        || staging.data_len() != 0
        || staging.executable
    {
        return Err(RegistryError::FinalizedRecord.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn activate_and_write_role(
    program_id: &Pubkey,
    output: &mut [u8],
    release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
    rent: &Rent,
    role: ExecutionRoleV1,
    frame: RoleFrame<'_, '_>,
) -> ProgramResult {
    let input = authenticate_artifact_role(program_id, release_set.binding(role), rent, frame)?;
    activate_execution_role_into_v1(output, release_set_id, release_set, role, &input)
        .map_err(|_| RegistryError::Release.into())
}

fn authenticate_cache_identity(
    program_id: &Pubkey,
    cache: &AccountInfo<'_>,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> ProgramResult {
    if cache.owner != program_id
        || cache.executable
        || cache.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(RegistryError::ActivationCache.into());
    }
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        program_id,
    )
    .0;
    if cache.key != &expected {
        return Err(RegistryError::ActivationCache.into());
    }
    Ok(())
}

fn ensure_activation_cache_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    cache: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    release_set_id: ContentId,
) -> Result<bool, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        program_id,
    );
    if cache.key != &expected {
        return Err(RegistryError::ActivationCache.into());
    }
    if cache.owner == program_id {
        if cache.executable
            || cache.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
            || !rent.is_exempt(cache.lamports(), cache.data_len())
        {
            return Err(RegistryError::ActivationCache.into());
        }
        return Ok(false);
    }
    if cache.owner != &system_program::ID
        || cache.executable
        || cache.lamports() != 0
        || cache.data_len() != 0
    {
        return Err(RegistryError::ActivationCache.into());
    }
    let space = u64::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
        .map_err(|_| RegistryError::Arithmetic)?;
    let lamports = rent.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1);
    let instruction = create_account(payer.key, cache.key, lamports, space, program_id);
    let bump_seed = [bump];
    let signer: [&[u8]; 3] = [
        ACTIVATION_PDA_DOMAIN_V1,
        release_set_id.as_bytes(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &instruction,
        &[payer.clone(), cache.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| RegistryError::CreateCpi)?;
    if cache.owner != program_id
        || cache.executable
        || cache.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || cache.lamports() != lamports
    {
        return Err(RegistryError::CreateCpi.into());
    }
    Ok(true)
}

fn validate_activate_role_privileges(
    payer: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    frame: RoleFrame<'_, '_>,
) -> ProgramResult {
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || cache.is_signer
        || !cache.is_writable
        || cache.executable
        || system.is_signer
        || system.is_writable
        || !system.executable
        || rent.is_signer
        || rent.is_writable
        || rent.executable
    {
        return Err(RegistryError::AccountFrame.into());
    }
    if frame.artifact_record.is_signer
        || frame.artifact_record.is_writable
        || frame.artifact_record.executable
        || frame.artifact_staging.is_signer
        || frame.artifact_staging.is_writable
        || frame.artifact_staging.executable
        || frame.program.is_signer
        || frame.program.is_writable
        || !frame.program.executable
        || frame.programdata.is_signer
        || frame.programdata.is_writable
        || frame.programdata.executable
    {
        return Err(RegistryError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_rent_and_system(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<Rent, ProgramError> {
    if system.key != &system_program::ID
        || system.owner != &native_loader::ID
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
    {
        return Err(RegistryError::Sysvar.into());
    }
    Rent::from_account_info(rent).map_err(|_| RegistryError::Sysvar.into())
}

fn role_frame<'accounts, 'info, I>(
    iterator: &mut I,
) -> Result<RoleFrame<'accounts, 'info>, ProgramError>
where
    I: Iterator<Item = &'accounts AccountInfo<'info>>,
{
    Ok(RoleFrame {
        artifact_record: next(iterator)?,
        artifact_staging: next(iterator)?,
        program: next(iterator)?,
        programdata: next(iterator)?,
    })
}

fn next<'accounts, 'info, I>(
    iterator: &mut I,
) -> Result<&'accounts AccountInfo<'info>, ProgramError>
where
    I: Iterator<Item = &'accounts AccountInfo<'info>>,
{
    next_account_info(iterator).map_err(|_| RegistryError::AccountFrame.into())
}

#[cfg(test)]
mod tests;
