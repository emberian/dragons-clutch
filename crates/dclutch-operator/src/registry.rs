//! Chain-derived unsigned Registry activation and reauthentication workflows.
//!
//! These builders accept one finalized observation of the exact raw records,
//! Loader V3 accounts, and runtime plumbing that the Registry program will
//! consume. They hostile-decode and reauthenticate those facts before
//! returning an instruction. They never perform RPC, access keys, sign, or
//! submit a transaction.

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetV1,
    ActivatedExecutionReleaseSetViewV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    DeploymentObservationV1, ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View, RegistryInstructionV1};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_message::{VersionedMessage, v0};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{Finality, Observation, ObservedAccount, versioned::PACKET_DATA_BYTES};

/// Chain-derived Core+Trading Registry continuation construction.
pub mod hot_continuation_v1;
/// Chain-derived Core+Custody market-open Registry continuation construction.
pub mod open_market_continuation_v1 {
    pub use dclutch_market_open_v1_operator::*;
}

/// Exact number of accounts consumed by release-set activation.
pub const REGISTRY_ACTIVATE_ACCOUNT_COUNT_V1: usize = 26;
/// Exact number of accounts consumed by one role reauthentication.
pub const REGISTRY_REAUTHENTICATE_ACCOUNT_COUNT_V1: usize = 3;
/// Current chain-profile transaction compute-unit ceiling.
///
/// This is transaction plumbing, not a protocol-semantic bound.
pub const TRANSACTION_COMPUTE_UNIT_LIMIT_V1: u32 = 1_400_000;
/// ELF digest of the exact Registry artifact used for the local measured profile.
pub const MEASURED_REGISTRY_ELF_DIGEST_V1: [u8; 32] = [
    0xfd, 0x7d, 0xdc, 0x66, 0x30, 0x93, 0x26, 0x53, 0x89, 0x3f, 0xa8, 0xcf, 0x9e, 0xd4, 0x92, 0xee,
    0x61, 0xc9, 0x16, 0x9a, 0x81, 0x06, 0x58, 0xbe, 0x64, 0xe5, 0xe8, 0x3a, 0xd0, 0x9e, 0xe5, 0xac,
];
/// Measured first-activation cost for five roles aliased to the measured ELF.
pub const MEASURED_CREATE_ACTIVATION_CU_V1: u32 = 371_988;
/// Measured repeated-activation cost for five roles aliased to the measured ELF.
pub const MEASURED_REPEAT_ACTIVATION_CU_V1: u32 = 351_337;
/// Measured one-role reauthentication cost for the measured ELF.
pub const MEASURED_REAUTHENTICATION_CU_V1: u32 = 65_390;

/// One finalized raw record and its now-vacant canonical staging cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFinalizedRecordState {
    /// Exact Registry-owned headerless record bytes.
    pub record: ObservedAccount,
    /// Exact System-owned, zero-lamport vacant staging cursor.
    pub staging_cursor: ObservedAccount,
}

/// One role's finalized release authority and current Loader V3 deployment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryRoleState {
    /// Finalized canonical `ArtifactReleaseV1` authority.
    pub artifact_release: RegistryFinalizedRecordState,
    /// Current executable Loader V3 Program account.
    pub program: ObservedAccount,
    /// Current non-executable Loader V3 ProgramData account and complete ELF.
    pub programdata: ObservedAccount,
}

/// Named, canonical profile-1 execution roles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryRoleSetState {
    /// Core Registry/Market semantics.
    pub core: RegistryRoleState,
    /// Canonical claims ownership.
    pub claims: RegistryRoleState,
    /// Trading admission.
    pub trading: RegistryRoleState,
    /// Resolution admission.
    pub resolution: RegistryRoleState,
    /// Physical collateral custody.
    pub custody: RegistryRoleState,
}

impl RegistryRoleSetState {
    fn all(&self) -> [&RegistryRoleState; 5] {
        [
            &self.core,
            &self.claims,
            &self.trading,
            &self.resolution,
            &self.custody,
        ]
    }
}

/// Same-finalized inputs for permissionless release-set activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryActivationState {
    /// System wallet signing and paying cache rent when creation is required.
    pub payer: ObservedAccount,
    /// Derived vacant destination or exact existing Registry activation cache.
    pub cache: ObservedAccount,
    /// Finalized canonical `ExecutionReleaseSetV1` authority.
    pub execution_release_set: RegistryFinalizedRecordState,
    /// Five named finalized artifact releases and current deployments.
    pub roles: RegistryRoleSetState,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Whether activation creates the cache or byte-identically replays it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryActivationModeV1 {
    /// The derived System-owned vacancy will be created and populated.
    Create,
    /// The existing Registry-owned bytes already equal the derived cache.
    Repeat,
}

/// Compute-relevant evidence derived from authenticated deployment state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryComputeEvidenceV1 {
    /// Complete ELF-tail bytes hashed by this exact instruction.
    pub elf_bytes_hashed: usize,
    /// Exact local ProgramTest measurement when the observed ELF profile matches.
    ///
    /// `None` is an honest absence of a matching measurement, not a zero-cost
    /// claim. Callers must still select an explicit transaction compute limit.
    pub matching_measured_compute_units: Option<u32>,
}

/// Fully checked unsigned activation instruction and derived facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryActivationReport {
    /// Exact unsigned 26-account Registry instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical content identity of the release-set record.
    pub execution_release_set_id: ContentId,
    /// Canonical derived activation-cache address.
    pub cache: Pubkey,
    /// Whether this instruction creates or exactly replays the cache.
    pub mode: RegistryActivationModeV1,
    /// Exact cache rent debit, or zero for a repeat.
    pub cache_rent_debit_lamports: u64,
    /// Complete semantic cache derived from the sole finalized authorities.
    pub expected_cache: ActivatedExecutionReleaseSetV1,
    /// Compute-relevant chain evidence and, when available, matching measurement.
    pub compute: RegistryComputeEvidenceV1,
}

/// Same-finalized inputs for one read-only role reauthentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryReauthenticationState {
    /// Current executable Registry Program account invoked by the instruction.
    pub registry_program: ObservedAccount,
    /// Existing Registry-owned activation cache.
    pub cache: ObservedAccount,
    /// Current executable Program account selected by the cached role.
    pub role_program: ObservedAccount,
    /// Current ProgramData account selected by the cached role.
    pub role_programdata: ObservedAccount,
}

/// Fully checked unsigned one-role reauthentication instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryReauthenticationReport {
    /// Exact unsigned three-account Registry instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical activation-cache address.
    pub cache: Pubkey,
    /// Reauthenticated semantic role.
    pub role: ExecutionRoleV1,
    /// Cached execution-release-set content identity.
    pub execution_release_set_id: ContentId,
    /// Exact executable program selected by the cached role.
    pub role_program: Pubkey,
    /// Finalized artifact-release identity selected by the cached role.
    pub artifact_release_id: ArtifactReleaseIdV1,
    /// Semantic release implemented by the authenticated artifact.
    pub semantic_release_id: ContentId,
    /// Compute-relevant chain evidence and, when available, matching measurement.
    pub compute: RegistryComputeEvidenceV1,
}

/// Unsigned v0 transaction message with exact packet and compute geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryPacketPlanV0 {
    /// Unsigned v0 message containing Compute Budget then Registry instruction.
    pub message: VersionedMessage,
    /// Exact number of signature slots in the final transaction.
    pub required_signatures: u8,
    /// Fully signed serialized transaction bytes.
    pub wire_bytes: usize,
    /// Explicit transaction compute limit encoded in the message.
    pub compute_unit_limit: u32,
    /// Matching measured cost when this exact ELF-shape profile is known.
    pub matching_measured_compute_units: Option<u32>,
    /// Nonnegative measured headroom when a matching measurement exists.
    pub measured_headroom: Option<u32>,
}

/// Refusal from hostile observations, release authority, deployment, or sizing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// At least one account was not observed at finalized commitment.
    ObservationNotFinalized,
    /// Inputs did not share one exact finalized observation.
    ObservationMismatch,
    /// Equal keys carried conflicting observed account facts.
    InconsistentAlias,
    /// The payer was not a usable System wallet.
    InvalidPayer,
    /// The System Program or Rent sysvar was not canonical.
    InvalidRuntimePlumbing,
    /// A raw record, digest, owner, rent reserve, or vacant cursor refused.
    InvalidFinalizedRecord,
    /// The release-set bytes or Core binding refused.
    InvalidReleaseSet,
    /// An artifact-release record or role binding refused.
    InvalidArtifactRelease,
    /// Program/ProgramData Loader V3 state, slot, ELF, or upgrade policy refused.
    InvalidDeployment,
    /// The activation-cache address, owner, reserve, or bytes refused.
    InvalidActivationCache,
    /// The payer could not cover the exact cache rent debit.
    InsufficientPayer,
    /// The requested compute limit was zero, above the chain profile, or below
    /// an exact matching measured profile.
    InvalidComputeLimit,
    /// Message construction or checked sizing overflowed.
    Encoding,
    /// The fully signed transaction exceeded the current packet limit.
    PacketTooLarge,
}

/// Build the exact permissionless 26-account Registry activation instruction.
pub fn build_registry_activation_v1(
    registry_program: Pubkey,
    state: &RegistryActivationState,
) -> Result<RegistryActivationReport, Error> {
    let observation = activation_observation(state)?;
    authenticate_aliases(&activation_accounts(state))?;
    authenticate_payer(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;
    let (release_set_id, release_set) =
        authenticate_release_set(registry_program, &rent, &state.execution_release_set)?;

    let core = authenticate_role(
        registry_program,
        &rent,
        release_set.binding(ExecutionRoleV1::Core),
        &state.roles.core,
    )?;
    let claims = authenticate_role(
        registry_program,
        &rent,
        release_set.binding(ExecutionRoleV1::Claims),
        &state.roles.claims,
    )?;
    let trading = authenticate_role(
        registry_program,
        &rent,
        release_set.binding(ExecutionRoleV1::Trading),
        &state.roles.trading,
    )?;
    let resolution = authenticate_role(
        registry_program,
        &rent,
        release_set.binding(ExecutionRoleV1::Resolution),
        &state.roles.resolution,
    )?;
    let custody = authenticate_role(
        registry_program,
        &rent,
        release_set.binding(ExecutionRoleV1::Custody),
        &state.roles.custody,
    )?;
    let expected_cache = activate_execution_release_set_v1(
        release_set_id,
        &release_set,
        &ExecutionReleaseActivationInputsV1::new(
            core.input,
            claims.input,
            trading.input,
            resolution.input,
            custody.input,
        ),
    )
    .map_err(|_| Error::InvalidReleaseSet)?;
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &registry_program,
    )
    .0;
    let (mode, cache_rent_debit_lamports) = authenticate_activation_cache(
        registry_program,
        &rent,
        &state.payer,
        &state.cache,
        cache,
        expected_cache,
    )?;

    let mut accounts = Vec::with_capacity(REGISTRY_ACTIVATE_ACCOUNT_COUNT_V1);
    accounts.extend([
        AccountMeta::new(state.payer.key, true),
        AccountMeta::new(cache, false),
        AccountMeta::new_readonly(state.execution_release_set.record.key, false),
        AccountMeta::new_readonly(state.execution_release_set.staging_cursor.key, false),
    ]);
    for role in state.roles.all() {
        accounts.extend([
            AccountMeta::new_readonly(role.artifact_release.record.key, false),
            AccountMeta::new_readonly(role.artifact_release.staging_cursor.key, false),
            AccountMeta::new_readonly(role.program.key, false),
            AccountMeta::new_readonly(role.programdata.key, false),
        ]);
    }
    accounts.extend([
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ]);
    if accounts.len() != REGISTRY_ACTIVATE_ACCOUNT_COUNT_V1 {
        return Err(Error::Encoding);
    }
    let elf_bytes_hashed = [core, claims, trading, resolution, custody]
        .iter()
        .try_fold(0_usize, |total, authenticated| {
            total.checked_add(authenticated.elf_bytes)
        })
        .ok_or(Error::Encoding)?;
    let core_binding = release_set.binding(ExecutionRoleV1::Core);
    let measured_profile = [core, claims, trading, resolution, custody]
        .iter()
        .all(|authenticated| authenticated.release.elf_digest() == MEASURED_REGISTRY_ELF_DIGEST_V1)
        && [
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Trading,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ]
        .into_iter()
        .all(|role| release_set.binding(role) == core_binding);
    let matching_measured_compute_units = if measured_profile {
        Some(match mode {
            RegistryActivationModeV1::Create => MEASURED_CREATE_ACTIVATION_CU_V1,
            RegistryActivationModeV1::Repeat => MEASURED_REPEAT_ACTIVATION_CU_V1,
        })
    } else {
        None
    };
    Ok(RegistryActivationReport {
        instruction: Instruction {
            program_id: registry_program,
            accounts,
            data: RegistryInstructionV1::Activate.to_bytes().to_vec(),
        },
        observation,
        execution_release_set_id: release_set_id,
        cache,
        mode,
        cache_rent_debit_lamports,
        expected_cache,
        compute: RegistryComputeEvidenceV1 {
            elf_bytes_hashed,
            matching_measured_compute_units,
        },
    })
}

/// Build the exact read-only three-account reauthentication instruction.
pub fn build_registry_reauthentication_v1(
    state: &RegistryReauthenticationState,
    role: ExecutionRoleV1,
) -> Result<RegistryReauthenticationReport, Error> {
    let observation = same_observation(&[
        &state.registry_program,
        &state.cache,
        &state.role_program,
        &state.role_programdata,
    ])?;
    authenticate_aliases(&[
        &state.registry_program,
        &state.cache,
        &state.role_program,
        &state.role_programdata,
    ])?;
    authenticate_registry_program(&state.registry_program)?;
    let registry_program = state.registry_program.key;
    let activated = authenticate_cache_identity(registry_program, &state.cache)?;
    let activated_role = activated
        .role(role)
        .map_err(|_| Error::InvalidActivationCache)?;
    let release = activated_role.release();
    let deployment = deployment_observation(&state.role_program, &state.role_programdata, release)?;
    activated_role
        .authenticate_current_deployment(deployment)
        .map_err(|_| Error::InvalidDeployment)?;
    let execution_release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| Error::InvalidActivationCache)?;
    let elf_bytes_hashed = ProgramDataV3View::parse(&state.role_programdata.data)
        .map_err(|_| Error::InvalidDeployment)?
        .elf()
        .len();
    Ok(RegistryReauthenticationReport {
        instruction: Instruction {
            program_id: registry_program,
            accounts: vec![
                AccountMeta::new_readonly(state.cache.key, false),
                AccountMeta::new_readonly(state.role_program.key, false),
                AccountMeta::new_readonly(state.role_programdata.key, false),
            ],
            data: RegistryInstructionV1::Reauthenticate(role)
                .to_bytes()
                .to_vec(),
        },
        observation,
        cache: state.cache.key,
        role,
        execution_release_set_id,
        role_program: state.role_program.key,
        artifact_release_id: activated_role.artifact_release_id(),
        semantic_release_id: release.semantic_release_id(),
        compute: RegistryComputeEvidenceV1 {
            elf_bytes_hashed,
            matching_measured_compute_units: (release.elf_digest()
                == MEASURED_REGISTRY_ELF_DIGEST_V1)
                .then_some(MEASURED_REAUTHENTICATION_CU_V1),
        },
    })
}

/// Compile activation plus an explicit Compute Budget instruction into an
/// unsigned packet-safe v0 message.
pub fn compile_registry_activation_packet_v0(
    report: &RegistryActivationReport,
    fee_payer: Pubkey,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
) -> Result<RegistryPacketPlanV0, Error> {
    compile_registry_packet_v0(
        fee_payer,
        &report.instruction,
        recent_blockhash,
        compute_unit_limit,
        report.compute.matching_measured_compute_units,
    )
}

/// Compile reauthentication plus an explicit Compute Budget instruction into
/// an unsigned packet-safe v0 message.
pub fn compile_registry_reauthentication_packet_v0(
    report: &RegistryReauthenticationReport,
    fee_payer: Pubkey,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
) -> Result<RegistryPacketPlanV0, Error> {
    compile_registry_packet_v0(
        fee_payer,
        &report.instruction,
        recent_blockhash,
        compute_unit_limit,
        report.compute.matching_measured_compute_units,
    )
}

#[derive(Clone, Copy)]
struct AuthenticatedRoleInput {
    input: ArtifactActivationInputV1,
    release: ArtifactReleaseV1,
    elf_bytes: usize,
}

fn authenticate_release_set(
    registry_program: Pubkey,
    rent: &Rent,
    state: &RegistryFinalizedRecordState,
) -> Result<(ContentId, ExecutionReleaseSetV1), Error> {
    let digest = authenticate_finalized_record(
        registry_program,
        rent,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        None,
        state,
    )?;
    let release_set =
        ExecutionReleaseSetV1::decode(&state.record.data).map_err(|_| Error::InvalidReleaseSet)?;
    let release_set_id = ContentId::new(digest).map_err(|_| Error::InvalidReleaseSet)?;
    Ok((release_set_id, release_set))
}

fn authenticate_role(
    registry_program: Pubkey,
    rent: &Rent,
    expected: ExecutionRoleBindingV1,
    state: &RegistryRoleState,
) -> Result<AuthenticatedRoleInput, Error> {
    let expected_digest = expected.artifact_release().to_bytes();
    authenticate_finalized_record(
        registry_program,
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        Some(expected_digest),
        &state.artifact_release,
    )?;
    let release = ArtifactReleaseV1::decode(&state.artifact_release.record.data)
        .map_err(|_| Error::InvalidArtifactRelease)?;
    if release.program() != expected.program() {
        return Err(Error::InvalidArtifactRelease);
    }
    let deployment = deployment_observation(&state.program, &state.programdata, release)?;
    release
        .authenticate_deployment(deployment)
        .map_err(|_| Error::InvalidDeployment)?;
    let elf_bytes = ProgramDataV3View::parse(&state.programdata.data)
        .map_err(|_| Error::InvalidDeployment)?
        .elf()
        .len();
    Ok(AuthenticatedRoleInput {
        input: ArtifactActivationInputV1::new(expected.artifact_release(), release, deployment),
        release,
        elf_bytes,
    })
}

fn authenticate_finalized_record(
    registry_program: Pubkey,
    rent: &Rent,
    schema: [u8; 32],
    expected_digest: Option<[u8; 32]>,
    state: &RegistryFinalizedRecordState,
) -> Result<[u8; 32], Error> {
    let record = &state.record;
    let cursor = &state.staging_cursor;
    let digest = hash(&record.data).to_bytes();
    if expected_digest.is_some_and(|expected| expected != digest)
        || record.owner != registry_program
        || record.executable
        || !rent.is_exempt(record.lamports, record.data.len())
    {
        return Err(Error::InvalidFinalizedRecord);
    }
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    if record.key != raw
        || cursor.key != staging
        || cursor.owner != system_program::ID
        || cursor.lamports != 0
        || cursor.executable
        || !cursor.data.is_empty()
    {
        return Err(Error::InvalidFinalizedRecord);
    }
    Ok(digest)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, Error> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(Error::InvalidDeployment);
    }
    let program_view = ProgramV3View::parse(&program.data).map_err(|_| Error::InvalidDeployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(Error::InvalidDeployment);
    }
    let programdata_view =
        ProgramDataV3View::parse(&programdata.data).map_err(|_| Error::InvalidDeployment)?;
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
    .map_err(|_| Error::InvalidDeployment)
}

fn authenticate_activation_cache(
    registry_program: Pubkey,
    rent: &Rent,
    payer: &ObservedAccount,
    cache: &ObservedAccount,
    expected_key: Pubkey,
    expected: ActivatedExecutionReleaseSetV1,
) -> Result<(RegistryActivationModeV1, u64), Error> {
    if cache.key != expected_key {
        return Err(Error::InvalidActivationCache);
    }
    if cache.owner == system_program::ID
        && !cache.executable
        && cache.lamports == 0
        && cache.data.is_empty()
    {
        let debit = rent.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1);
        if payer.lamports < debit {
            return Err(Error::InsufficientPayer);
        }
        return Ok((RegistryActivationModeV1::Create, debit));
    }
    if cache.owner != registry_program
        || cache.executable
        || cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !rent.is_exempt(cache.lamports, cache.data.len())
        || cache.data.as_slice() != expected.to_bytes()
    {
        return Err(Error::InvalidActivationCache);
    }
    Ok((RegistryActivationModeV1::Repeat, 0))
}

fn authenticate_cache_identity<'a>(
    registry_program: Pubkey,
    cache: &'a ObservedAccount,
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, Error> {
    if cache.owner != registry_program
        || cache.executable
        || cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(Error::InvalidActivationCache);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(|_| Error::InvalidActivationCache)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| Error::InvalidActivationCache)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &registry_program,
    )
    .0;
    if cache.key != expected {
        return Err(Error::InvalidActivationCache);
    }
    Ok(activated)
}

fn authenticate_registry_program(program: &ObservedAccount) -> Result<(), Error> {
    if program.owner != bpf_loader_upgradeable::ID || !program.executable {
        return Err(Error::InvalidDeployment);
    }
    ProgramV3View::parse(&program.data)
        .map(|_| ())
        .map_err(|_| Error::InvalidDeployment)
}

fn authenticate_payer(payer: &ObservedAccount) -> Result<(), Error> {
    if payer.owner != system_program::ID || payer.executable || !payer.data.is_empty() {
        return Err(Error::InvalidPayer);
    }
    Ok(())
}

fn authenticate_system_program(system: &ObservedAccount) -> Result<(), Error> {
    if system.key != system_program::ID
        || system.owner != native_loader::ID
        || !system.executable
        || !system.data.is_empty()
    {
        return Err(Error::InvalidRuntimePlumbing);
    }
    Ok(())
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent, Error> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(Error::InvalidRuntimePlumbing);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| Error::InvalidRuntimePlumbing)
}

fn activation_observation(state: &RegistryActivationState) -> Result<Observation, Error> {
    same_observation(&activation_accounts(state))
}

fn activation_accounts(state: &RegistryActivationState) -> Vec<&ObservedAccount> {
    let mut accounts = Vec::with_capacity(REGISTRY_ACTIVATE_ACCOUNT_COUNT_V1);
    accounts.extend([
        &state.payer,
        &state.cache,
        &state.execution_release_set.record,
        &state.execution_release_set.staging_cursor,
    ]);
    for role in state.roles.all() {
        accounts.extend([
            &role.artifact_release.record,
            &role.artifact_release.staging_cursor,
            &role.program,
            &role.programdata,
        ]);
    }
    accounts.extend([&state.system_program, &state.rent_sysvar]);
    accounts
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationMismatch)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn authenticate_aliases(accounts: &[&ObservedAccount]) -> Result<(), Error> {
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key && left != right {
                return Err(Error::InconsistentAlias);
            }
        }
    }
    Ok(())
}

fn compile_registry_packet_v0(
    fee_payer: Pubkey,
    instruction: &Instruction,
    recent_blockhash: Hash,
    compute_unit_limit: u32,
    matching_measured_compute_units: Option<u32>,
) -> Result<RegistryPacketPlanV0, Error> {
    if compute_unit_limit == 0
        || compute_unit_limit > TRANSACTION_COMPUTE_UNIT_LIMIT_V1
        || matching_measured_compute_units.is_some_and(|measured| compute_unit_limit < measured)
    {
        return Err(Error::InvalidComputeLimit);
    }
    let compute_budget = ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit);
    let message = v0::Message::try_compile(
        &fee_payer,
        &[compute_budget, instruction.clone()],
        &[],
        recent_blockhash,
    )
    .map_err(|_| Error::Encoding)?;
    let required_signatures = message.header.num_required_signatures;
    let signature_count = usize::from(required_signatures);
    let message_bytes = message.serialize().len();
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(signature_count.checked_mul(64).ok_or(Error::Encoding)?)
        .and_then(|value| value.checked_add(message_bytes))
        .ok_or(Error::Encoding)?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::PacketTooLarge);
    }
    Ok(RegistryPacketPlanV0 {
        message: VersionedMessage::V0(message),
        required_signatures,
        wire_bytes,
        compute_unit_limit,
        matching_measured_compute_units,
        measured_headroom: matching_measured_compute_units
            .and_then(|measured| compute_unit_limit.checked_sub(measured)),
    })
}

fn short_vec_prefix_bytes(value: usize) -> usize {
    if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests;
