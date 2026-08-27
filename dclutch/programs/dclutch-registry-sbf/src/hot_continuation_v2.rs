//! Headerless Registry-authenticated Trading Hot continuation.
//!
//! Registry is the top-level release-authentication waist, but the instruction
//! data is byte-for-byte the canonical Trading Hot instruction.  Every signer
//! coordinate is reconstructed from the authenticated activation cache and Hot
//! envelope; no caller-provided wrapper or mode byte participates in authority.

use std::vec::Vec;

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_registry_svm::continuation_v2::{
    TransparentHotAdmissionSeedsV2, TransparentHotContinuationV2,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{RegistryError, batch_v2};

/// Exact Registry-owned authentication prefix before the nested Hot frame.
pub(super) const TRANSPARENT_HOT_PREFIX_ACCOUNTS_V2: usize = 6;
/// Admission signer location inside the nested Hot frame.
pub(super) const TRANSPARENT_HOT_ADMISSION_ACCOUNT_V2: usize = HOT_FIXED_ACCOUNT_COUNT_V3;

const BATCH_ACCOUNT_COUNT_V2: usize = 5;
const ADMISSION_ACCOUNT_V2: usize = BATCH_ACCOUNT_COUNT_V2;
const CONTINUATION_ACCOUNTS_START_V2: usize = TRANSPARENT_HOT_PREFIX_ACCOUNTS_V2;

/// Authenticate the fixed Core+Trading release batch and forward exact Hot bytes.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(instruction_data)
        .map_err(|_| RegistryError::Continuation)?;
    let batch_accounts = accounts
        .get(..BATCH_ACCOUNT_COUNT_V2)
        .ok_or(RegistryError::AccountFrame)?;
    let admission = accounts
        .get(ADMISSION_ACCOUNT_V2)
        .ok_or(RegistryError::AccountFrame)?;
    let continuation_accounts = accounts
        .get(CONTINUATION_ACCOUNTS_START_V2..)
        .ok_or(RegistryError::AccountFrame)?;
    if continuation_accounts.len() <= TRANSPARENT_HOT_ADMISSION_ACCOUNT_V2 {
        return Err(RegistryError::AccountFrame.into());
    }
    require_vacant_admission(admission)?;

    let cache = batch_accounts.first().ok_or(RegistryError::AccountFrame)?;
    let cache_data = cache.try_borrow_data().map_err(|_| RegistryError::Borrow)?;
    let cache_digest =
        ContentId::new(hash(&cache_data).to_bytes()).map_err(|_| RegistryError::Continuation)?;
    drop(cache_data);
    let release_set =
        ContentId::new(envelope.release_set()).map_err(|_| RegistryError::Continuation)?;
    let hot_digest = ContentId::new(hash(instruction_data).to_bytes())
        .map_err(|_| RegistryError::Continuation)?;
    let hot_len = u32::try_from(instruction_data.len()).map_err(|_| RegistryError::Arithmetic)?;
    let continuation =
        TransparentHotContinuationV2::new(release_set, cache_digest, hot_digest, hot_len)
            .map_err(|_| RegistryError::Continuation)?;
    let batch_request = continuation
        .role_batch_request()
        .map_err(|_| RegistryError::Continuation)?;
    let authenticated = batch_v2::authenticate_request(program_id, batch_accounts, batch_request)?;
    if authenticated.cache_digest != cache_digest {
        return Err(RegistryError::Continuation.into());
    }
    let selected = authenticated
        .observations
        .iter()
        .find(|observation| observation.role() == ExecutionRoleV1::Trading)
        .ok_or(RegistryError::Continuation)?;
    let trading_program = Pubkey::new_from_array(selected.program().to_bytes());
    let selected_program_info = batch_accounts
        .get(3)
        .filter(|account| account.key == &trading_program)
        .ok_or(RegistryError::Continuation)?;

    authenticate_hot_coordinates(
        program_id,
        envelope,
        batch_accounts,
        admission,
        continuation_accounts,
    )?;

    let batch_request_digest = ContentId::new(hash(&batch_request.to_bytes()).to_bytes())
        .map_err(|_| RegistryError::Batch)?;
    let seeds = TransparentHotAdmissionSeedsV2::new(
        continuation,
        cache.key.to_bytes(),
        batch_request_digest,
    )
    .map_err(|_| RegistryError::Continuation)?;
    let release = seeds.release_set();
    let cache_seed = seeds.activation_cache();
    let batch_digest = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let continuation_role = seeds.continuation_role();
    let continuation_digest = seeds.hot_instruction_digest();
    let (expected_admission, bump) = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache_seed.as_slice(),
            batch_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
        ],
        program_id,
    );
    if expected_admission != *admission.key
        || continuation_accounts
            .iter()
            .filter(|account| account.key == admission.key)
            .count()
            != 1
    {
        return Err(RegistryError::Continuation.into());
    }

    let mut metas = Vec::with_capacity(continuation_accounts.len());
    let mut infos = Vec::with_capacity(continuation_accounts.len() + 1);
    for account in continuation_accounts {
        let signer = account.key == admission.key || account.is_signer;
        let meta = if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        };
        metas.push(meta);
        infos.push(account.clone());
    }
    infos.push(selected_program_info.clone());
    let instruction = Instruction {
        program_id: trading_program,
        accounts: metas,
        data: instruction_data.to_vec(),
    };
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            seeds.domain(),
            release.as_slice(),
            cache_seed.as_slice(),
            batch_digest.as_slice(),
            role_mask.as_slice(),
            continuation_role.as_slice(),
            continuation_digest.as_slice(),
            bump_seed.as_slice(),
        ]],
    )
    .map_err(|_| RegistryError::Continuation.into())
}

fn require_vacant_admission(admission: &AccountInfo<'_>) -> ProgramResult {
    if admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
    {
        return Err(RegistryError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_hot_coordinates(
    program_id: &Pubkey,
    envelope: HotExecutionEnvelopeV3,
    batch: &[AccountInfo<'_>],
    admission: &AccountInfo<'_>,
    hot: &[AccountInfo<'_>],
) -> ProgramResult {
    let hot_cache = hot
        .get(HOT_ACTIVATION_CACHE_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let hot_core = hot
        .get(HOT_CORE_PROGRAM_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let hot_core_programdata = hot
        .get(HOT_CORE_PROGRAMDATA_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let hot_trading = hot
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let hot_trading_programdata = hot
        .get(HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let hot_registry = hot
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let batch_cache = batch.first().ok_or(RegistryError::AccountFrame)?;
    let batch_core = batch.get(1).ok_or(RegistryError::AccountFrame)?;
    let batch_core_programdata = batch.get(2).ok_or(RegistryError::AccountFrame)?;
    let batch_trading = batch.get(3).ok_or(RegistryError::AccountFrame)?;
    let batch_trading_programdata = batch.get(4).ok_or(RegistryError::AccountFrame)?;
    if hot_cache.key != batch_cache.key
        || hot_core.key != batch_core.key
        || hot_core_programdata.key != batch_core_programdata.key
        || hot_trading.key != batch_trading.key
        || hot_trading_programdata.key != batch_trading_programdata.key
        || hot_registry.key != program_id
        || hot
            .get(TRANSPARENT_HOT_ADMISSION_ACCOUNT_V2)
            .is_none_or(|account| account.key != admission.key)
    {
        return Err(RegistryError::Continuation.into());
    }

    let market = hot
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    let root = hot
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(RegistryError::AccountFrame)?;
    if market.key.to_bytes() != envelope.market()
        || root.owner != hot_trading.key
        || root.executable
        || root.is_signer
    {
        return Err(RegistryError::Continuation.into());
    }
    let root_data = root.try_borrow_data().map_err(|_| RegistryError::Borrow)?;
    if hash(&root_data).to_bytes() != envelope.root_prestate_digest() {
        return Err(RegistryError::Continuation.into());
    }
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(RegistryError::Continuation)?,
    )
    .map_err(|_| RegistryError::Continuation)?;
    if header.release_set().to_bytes() != envelope.release_set()
        || header.market() != envelope.market()
        || header.generation() != envelope.generation()
    {
        return Err(RegistryError::Continuation.into());
    }
    Ok(())
}
