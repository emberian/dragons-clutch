//! Chain-derived Registry continuation for canonical Core market opening.
//!
//! This wrapper authenticates the finalized activation cache and its current
//! Core and Custody Loader deployments, hostile-decodes the complete existing
//! Core `OpenMarket` instruction, derives the invocation-scoped Registry
//! admission, and emits one unsigned top-level Registry instruction. It does
//! not persist authority, accept caller-authored release truth, sign, or send.

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CustodyAuthoritySeedsV1, CustodyFrameSpecV1,
    CustodyReplaySeedsV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{Action, REQUEST_BYTES, Request};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{
    ProgramDataV3View, ProgramV3View,
    continuation_v1::{
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
        RegistryContinuationRequestV1,
    },
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

/// Exact Registry batch and admission prefix before the nested Core frame.
pub const REGISTRY_OPEN_MARKET_CONTINUATION_PREFIX_ACCOUNTS_V1: usize = 6;
/// Exact Core frame width before its Registry admission for replay creation.
pub const CORE_INITIALIZE_REPLAY_ACCOUNT_COUNT_V1: usize = 14;
/// Exact Core frame width before its Registry admission for vault creation.
pub const CORE_OPEN_VAULT_ACCOUNT_COUNT_V1: usize = 18;

const CORE_MARKET: usize = 1;
const ACTIVATION_CACHE: usize = 2;
const REGISTRY_PROGRAM: usize = 3;
const CORE_PROGRAM: usize = 4;
const CORE_PROGRAMDATA: usize = 5;
const CUSTODY_PROGRAM: usize = 6;
const CUSTODY_PROGRAMDATA: usize = 7;

/// Same-finalized Registry and Loader facts selecting Core and Custody.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryOpenMarketContinuationStateV1 {
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Existing Registry-owned activated execution release set.
    pub activation_cache: ObservedAccount,
    /// Current cache-selected Core program.
    pub core_program: ObservedAccount,
    /// Current cache-selected Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current cache-selected Custody program.
    pub custody_program: ObservedAccount,
    /// Current cache-selected Custody ProgramData.
    pub custody_programdata: ObservedAccount,
}

/// Checked unsigned Registry wrapper around one exact Core open effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryOpenMarketContinuationReportV1 {
    /// Exact top-level Registry instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting both deployments.
    pub observation: Observation,
    /// Exact activated execution release-set identity.
    pub release_set_id: ContentId,
    /// Digest of the complete activation-cache bytes.
    pub activation_cache_digest: ContentId,
    /// Digest of the complete unchanged Core instruction bytes.
    pub core_instruction_digest: ContentId,
    /// Invocation-scoped System-vacant admission candidate.
    pub admission: Pubkey,
    /// Canonical fixed continuation header prepended by Registry.
    pub continuation: RegistryContinuationRequestV1,
    /// Hostile-decoded Custody operation selected by the Core request.
    pub operation: OperationV1,
}

/// Refusal from hostile chain observations or Core open construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryOpenMarketContinuationErrorV1 {
    /// Registry cache or current Loader deployment authentication refused.
    Registry(RegistryOpenMarketObservationErrorV1),
    /// Instruction bytes, release, program, account order, or privileges refused.
    InvalidCoreInstruction,
    /// A required digest or checked width refused.
    Identity,
    /// Admission derivation or account aliasing refused.
    Admission,
}

impl From<RegistryOpenMarketObservationErrorV1> for RegistryOpenMarketContinuationErrorV1 {
    fn from(value: RegistryOpenMarketObservationErrorV1) -> Self {
        Self::Registry(value)
    }
}

/// Refusal from finalized Registry cache or current Loader observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryOpenMarketObservationErrorV1 {
    /// At least one account observation was not finalized.
    ObservationNotFinalized,
    /// Accounts came from different slots, times, or finality observations.
    ObservationMismatch,
    /// One key carried conflicting observed account facts.
    InconsistentAlias,
    /// Registry program ownership, execution, or Loader state refused.
    InvalidRegistry,
    /// Activation-cache owner, bytes, release identity, or PDA refused.
    InvalidActivationCache,
    /// A selected Core/Custody Program or ProgramData deployment refused.
    InvalidDeployment,
}

#[derive(Clone, Copy)]
struct AuthenticatedRegistryOpenV1 {
    observation: Observation,
    release_set_id: ContentId,
}

/// Build one exact unsigned top-level Registry Core+Custody open continuation.
pub fn build_registry_open_market_continuation_v1(
    state: &RegistryOpenMarketContinuationStateV1,
    core_instruction: &Instruction,
) -> Result<RegistryOpenMarketContinuationReportV1, RegistryOpenMarketContinuationErrorV1> {
    let authenticated = authenticate_registry_open(state)?;

    let custody_start = REQUEST_BYTES;
    let custody_end = custody_start
        .checked_add(CUSTODY_REQUEST_BYTES_V1)
        .ok_or(RegistryOpenMarketContinuationErrorV1::Identity)?;
    if core_instruction.program_id != state.core_program.key
        || core_instruction.data.len() != custody_end
    {
        return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
    }
    let request = Request::decode(
        core_instruction
            .data
            .get(..REQUEST_BYTES)
            .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?,
    )
    .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    let custody_request = CustodyRequestV1::decode(
        core_instruction
            .data
            .get(custody_start..custody_end)
            .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?,
    )
    .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    if request.action != Action::OpenMarket
        || !matches!(
            custody_request.operation,
            OperationV1::InitializeReplay | OperationV1::OpenVault
        )
        || request.market.to_bytes() != custody_request.market
        || request.generation != custody_request.semantic.generation
        || custody_request.caller_role != CallerRoleV1::Core
        || custody_request.caller_program != state.core_program.key.to_bytes()
        || custody_request.release_set != authenticated.release_set_id.to_bytes()
    {
        return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
    }
    authenticate_core_frame(state, core_instruction, custody_request)?;

    let activation_cache_digest = ContentId::new(hash(&state.activation_cache.data).to_bytes())
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::Identity)?;
    let core_instruction_digest = ContentId::new(hash(&core_instruction.data).to_bytes())
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::Identity)?;
    let core_instruction_len = u32::try_from(core_instruction.data.len())
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::Identity)?;
    let continuation = RegistryContinuationRequestV1::new(
        authenticated.release_set_id,
        activation_cache_digest,
        core_instruction_digest,
        core_instruction_len,
        ExecutionRoleV1::Core,
        &[ExecutionRoleV1::Core, ExecutionRoleV1::Custody],
    )
    .map_err(|_| RegistryOpenMarketContinuationErrorV1::Identity)?;
    let batch = continuation
        .role_batch_request()
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::Admission)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::Admission)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        state.activation_cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| RegistryOpenMarketContinuationErrorV1::Admission)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch_request_digest = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let continuation_role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch_request_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &state.registry_program.key,
    )
    .0;
    if core_instruction
        .accounts
        .iter()
        .any(|meta| meta.pubkey == admission)
    {
        return Err(RegistryOpenMarketContinuationErrorV1::Admission);
    }

    let mut child_accounts = core_instruction.accounts.clone();
    child_accounts.push(AccountMeta::new_readonly(admission, false));
    let mut accounts = Vec::with_capacity(
        REGISTRY_OPEN_MARKET_CONTINUATION_PREFIX_ACCOUNTS_V1 + child_accounts.len(),
    );
    accounts.extend([
        AccountMeta::new_readonly(state.activation_cache.key, false),
        AccountMeta::new_readonly(state.core_program.key, false),
        AccountMeta::new_readonly(state.core_programdata.key, false),
        AccountMeta::new_readonly(state.custody_program.key, false),
        AccountMeta::new_readonly(state.custody_programdata.key, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(child_accounts);
    let mut data =
        Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + core_instruction.data.len());
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&core_instruction.data);

    Ok(RegistryOpenMarketContinuationReportV1 {
        instruction: Instruction {
            program_id: state.registry_program.key,
            accounts,
            data,
        },
        observation: authenticated.observation,
        release_set_id: authenticated.release_set_id,
        activation_cache_digest,
        core_instruction_digest,
        admission,
        continuation,
        operation: custody_request.operation,
    })
}

fn authenticate_registry_open(
    state: &RegistryOpenMarketContinuationStateV1,
) -> Result<AuthenticatedRegistryOpenV1, RegistryOpenMarketObservationErrorV1> {
    let accounts = [
        &state.registry_program,
        &state.activation_cache,
        &state.core_program,
        &state.core_programdata,
        &state.custody_program,
        &state.custody_programdata,
    ];
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(RegistryOpenMarketObservationErrorV1::ObservationMismatch)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(RegistryOpenMarketObservationErrorV1::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(RegistryOpenMarketObservationErrorV1::ObservationMismatch);
    }
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key && left != right {
                return Err(RegistryOpenMarketObservationErrorV1::InconsistentAlias);
            }
        }
    }
    if state.registry_program.owner != bpf_loader_upgradeable::ID
        || !state.registry_program.executable
        || ProgramV3View::parse(&state.registry_program.data).is_err()
    {
        return Err(RegistryOpenMarketObservationErrorV1::InvalidRegistry);
    }
    if state.activation_cache.owner != state.registry_program.key
        || state.activation_cache.executable
    {
        return Err(RegistryOpenMarketObservationErrorV1::InvalidActivationCache);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&state.activation_cache.data)
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidActivationCache)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidActivationCache)?;
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &state.registry_program.key,
    )
    .0;
    if state.activation_cache.key != expected_cache {
        return Err(RegistryOpenMarketObservationErrorV1::InvalidActivationCache);
    }
    authenticate_current_role(
        activated,
        ExecutionRoleV1::Core,
        &state.core_program,
        &state.core_programdata,
    )?;
    authenticate_current_role(
        activated,
        ExecutionRoleV1::Custody,
        &state.custody_program,
        &state.custody_programdata,
    )?;
    Ok(AuthenticatedRegistryOpenV1 {
        observation,
        release_set_id,
    })
}

fn authenticate_current_role(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), RegistryOpenMarketObservationErrorV1> {
    let selected = activated
        .role(role)
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidActivationCache)?;
    let release = selected.release();
    if release.program().to_bytes() != program.key.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(RegistryOpenMarketObservationErrorV1::InvalidDeployment);
    }
    let deployment = deployment_observation(program, programdata)?;
    selected
        .authenticate_current_deployment(deployment)
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidDeployment)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<DeploymentObservationV1, RegistryOpenMarketObservationErrorV1> {
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidDeployment)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes()
        || programdata.key != expected_programdata
    {
        return Err(RegistryOpenMarketObservationErrorV1::InvalidDeployment);
    }
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidDeployment)?;
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
    .map_err(|_| RegistryOpenMarketObservationErrorV1::InvalidDeployment)
}

fn authenticate_core_frame(
    state: &RegistryOpenMarketContinuationStateV1,
    instruction: &Instruction,
    request: CustodyRequestV1,
) -> Result<(), RegistryOpenMarketContinuationErrorV1> {
    let spec = CustodyFrameSpecV1::new(request.operation);
    let custody_count = usize::from(spec.account_count());
    let expected_count = custody_count
        .checked_add(2)
        .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    let named_expected = match request.operation {
        OperationV1::InitializeReplay => CORE_INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
        OperationV1::OpenVault => CORE_OPEN_VAULT_ACCOUNT_COUNT_V1,
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    };
    if expected_count != named_expected || instruction.accounts.len() != expected_count {
        return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
    }
    require_distinct(&instruction.accounts)?;
    for (index, expected) in [
        (CORE_MARKET, Pubkey::new_from_array(request.market)),
        (ACTIVATION_CACHE, state.activation_cache.key),
        (REGISTRY_PROGRAM, state.registry_program.key),
        (CORE_PROGRAM, state.core_program.key),
        (CORE_PROGRAMDATA, state.core_programdata.key),
        (CUSTODY_PROGRAM, state.custody_program.key),
        (CUSTODY_PROGRAMDATA, state.custody_programdata.key),
    ] {
        let meta = instruction
            .accounts
            .get(index)
            .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
        if meta.pubkey != expected || meta.is_signer || (index == CORE_MARKET) != meta.is_writable {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    }
    let custody_bytes = request
        .to_bytes()
        .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set)
            .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?,
        request.market,
        ExecutionRoleV1::Core,
        request.context,
        hash(&custody_bytes).to_bytes(),
    )
    .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    let expected_authority =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &state.core_program.key).0;
    let authority = instruction
        .accounts
        .first()
        .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
    if authority.pubkey != expected_authority || authority.is_signer || authority.is_writable {
        return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
    }

    for child_index in 6..custody_count {
        let core_index = child_index
            .checked_add(2)
            .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
        let privileges = spec
            .account(
                u16::try_from(child_index)
                    .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?,
            )
            .map_err(|_| RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?
            .privileges();
        let meta = instruction
            .accounts
            .get(core_index)
            .ok_or(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)?;
        if meta.is_signer != privileges.signer() || meta.is_writable != privileges.writable() {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    }
    authenticate_derived_keys(state, instruction, request)
}

fn authenticate_derived_keys(
    state: &RegistryOpenMarketContinuationStateV1,
    instruction: &Instruction,
    request: CustodyRequestV1,
) -> Result<(), RegistryOpenMarketContinuationErrorV1> {
    let raw_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &request.realm,
        ],
        &state.registry_program.key,
    )
    .0;
    let staging_realm = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &request.realm,
        ],
        &state.registry_program.key,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &state.custody_program.key,
    )
    .0;
    for (index, expected) in [(8, raw_realm), (9, staging_realm), (10, replay)] {
        if instruction.accounts.get(index).map(|meta| meta.pubkey) != Some(expected) {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    }
    match request.operation {
        OperationV1::InitializeReplay => {
            require_key(instruction, 11, Pubkey::new_from_array(request.payer))?;
            require_key(instruction, 12, system_program::ID)?;
            require_key(instruction, 13, sysvar::rent::ID)?;
        }
        OperationV1::OpenVault => {
            let vault = Pubkey::find_program_address(
                &CustodyVaultSeedsV1::from_request(request, false).as_slices(),
                &state.custody_program.key,
            )
            .0;
            let authority = Pubkey::find_program_address(
                &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
                &state.custody_program.key,
            )
            .0;
            require_key(instruction, 11, Pubkey::new_from_array(request.mint))?;
            require_key(instruction, 12, vault)?;
            require_key(instruction, 13, authority)?;
            require_key(
                instruction,
                14,
                Pubkey::new_from_array(request.token_program),
            )?;
            require_key(instruction, 15, Pubkey::new_from_array(request.payer))?;
            require_key(instruction, 16, system_program::ID)?;
            require_key(instruction, 17, sysvar::rent::ID)?;
        }
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    }
    Ok(())
}

fn require_key(
    instruction: &Instruction,
    index: usize,
    expected: Pubkey,
) -> Result<(), RegistryOpenMarketContinuationErrorV1> {
    if instruction.accounts.get(index).map(|meta| meta.pubkey) != Some(expected) {
        return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountMeta]) -> Result<(), RegistryOpenMarketContinuationErrorV1> {
    for (left_index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left_index.saturating_add(1))
            .any(|right| left.pubkey == right.pubkey)
        {
            return Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
