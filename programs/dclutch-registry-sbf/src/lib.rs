#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Thin successor Registry activation and deployment-reauthentication adapter.
//!
//! This program is the sole writer of the release-set activation cache. It
//! authenticates finalized headerless records, parses current Loader V3 state,
//! hashes each complete deployed ELF tail, invokes the SDK-free Registry
//! contract once, and persists only that derived result. Capability programs
//! may CPI into the read-only reauthentication route and consume its fixed
//! return-data receipt after checking this program as the producer.

extern crate std;

use core::convert::TryFrom;

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::{
    AuthenticatedRoleReceiptV1, ProgramDataV3View, ProgramV3View, RegistryInstructionV1,
};
use dclutch_release_set_contract::{
    EXECUTION_RELEASE_SET_BYTES_V1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
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

/// Exact account count for permissionless release-set activation.
pub const ACTIVATE_ACCOUNT_COUNT_V1: usize = 26;
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
solana_program::entrypoint_no_alloc!(process_instruction);

/// Dispatch one exact activation or reauthentication request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match RegistryInstructionV1::decode(instruction_data).map_err(|_| RegistryError::Instruction)? {
        RegistryInstructionV1::Activate => process_activate(program_id, accounts),
        RegistryInstructionV1::Reauthenticate(role) => {
            process_reauthenticate(program_id, accounts, role)
        }
    }
}

#[inline(never)]
fn process_activate(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != ACTIVATE_ACCOUNT_COUNT_V1 {
        return Err(RegistryError::AccountFrame.into());
    }
    let mut iterator = accounts.iter();
    let payer = next(&mut iterator)?;
    let cache = next(&mut iterator)?;
    let release_set_record = next(&mut iterator)?;
    let release_set_staging = next(&mut iterator)?;
    let core = role_frame(&mut iterator)?;
    let claims = role_frame(&mut iterator)?;
    let trading = role_frame(&mut iterator)?;
    let resolution = role_frame(&mut iterator)?;
    let custody = role_frame(&mut iterator)?;
    let system = next(&mut iterator)?;
    let rent_sysvar = next(&mut iterator)?;
    validate_activate_privileges(
        payer,
        cache,
        system,
        rent_sysvar,
        [core, claims, trading, resolution, custody],
    )?;
    let rent = authenticate_rent_and_system(system, rent_sysvar)?;
    let (release_set_id, release_set) = authenticate_release_set_record(
        program_id,
        release_set_record,
        release_set_staging,
        &rent,
    )?;
    let created =
        ensure_activation_cache_account(program_id, payer, cache, system, &rent, release_set_id)?;
    let core_program =
        ProgramIdentityV1::new(program_id.to_bytes()).map_err(|_| RegistryError::Release)?;
    let frames = [core, claims, trading, resolution, custody];
    let mut output = cache
        .try_borrow_mut_data()
        .map_err(|_| RegistryError::Borrow)?;
    if created {
        initialize_activation_cache_v1(&mut output, core_program, release_set_id, &release_set)
            .map_err(|_| RegistryError::ActivationCache)?;
    } else {
        authenticate_existing_cache(program_id, cache, &output, release_set_id, &release_set)?;
    }
    activate_and_write_role(
        program_id,
        &mut output,
        core_program,
        release_set_id,
        &release_set,
        &rent,
        ExecutionRoleV1::Core,
        role_at(&frames, 0)?,
    )?;
    activate_and_write_role(
        program_id,
        &mut output,
        core_program,
        release_set_id,
        &release_set,
        &rent,
        ExecutionRoleV1::Claims,
        role_at(&frames, 1)?,
    )?;
    activate_and_write_role(
        program_id,
        &mut output,
        core_program,
        release_set_id,
        &release_set,
        &rent,
        ExecutionRoleV1::Trading,
        role_at(&frames, 2)?,
    )?;
    activate_and_write_role(
        program_id,
        &mut output,
        core_program,
        release_set_id,
        &release_set,
        &rent,
        ExecutionRoleV1::Resolution,
        role_at(&frames, 3)?,
    )?;
    activate_and_write_role(
        program_id,
        &mut output,
        core_program,
        release_set_id,
        &release_set,
        &rent,
        ExecutionRoleV1::Custody,
        role_at(&frames, 4)?,
    )?;
    let completed = ActivatedExecutionReleaseSetViewV1::decode(&output)
        .map_err(|_| RegistryError::ActivationCache)?;
    if completed
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?
        != release_set_id
        || completed
            .release_set_projection()
            .map_err(|_| RegistryError::ActivationCache)?
            != release_set
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
    let observation = deployment_observation(program, programdata, release)?;
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
    core_program: ProgramIdentityV1,
    release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
    rent: &Rent,
    role: ExecutionRoleV1,
    frame: RoleFrame<'_, '_>,
) -> ProgramResult {
    let input = authenticate_artifact_role(program_id, release_set.binding(role), rent, frame)?;
    activate_execution_role_into_v1(
        output,
        core_program,
        release_set_id,
        release_set,
        role,
        &input,
    )
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
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| RegistryError::ActivationCache)?;
    if cache.key != &expected || core.release().program().to_bytes() != program_id.to_bytes() {
        return Err(RegistryError::ActivationCache.into());
    }
    Ok(())
}

fn authenticate_existing_cache(
    program_id: &Pubkey,
    cache: &AccountInfo<'_>,
    bytes: &[u8],
    release_set_id: ContentId,
    release_set: &ExecutionReleaseSetV1,
) -> ProgramResult {
    let activated = ActivatedExecutionReleaseSetViewV1::decode(bytes)
        .map_err(|_| RegistryError::ActivationCache)?;
    authenticate_cache_identity(program_id, cache, activated)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?
        != release_set_id
        || activated
            .release_set_projection()
            .map_err(|_| RegistryError::ActivationCache)?
            != *release_set
    {
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

fn validate_activate_privileges(
    payer: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    frames: [RoleFrame<'_, '_>; 5],
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
    for frame in frames {
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

fn role_at<'accounts, 'info>(
    roles: &[RoleFrame<'accounts, 'info>; 5],
    index: usize,
) -> Result<RoleFrame<'accounts, 'info>, ProgramError> {
    roles
        .get(index)
        .copied()
        .ok_or_else(|| RegistryError::AccountFrame.into())
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
