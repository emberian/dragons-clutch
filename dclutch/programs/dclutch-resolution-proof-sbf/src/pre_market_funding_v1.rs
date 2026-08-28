//! Resolution-owned subset-ledger initialization before Core creates a Market.

use alloc::{vec, vec::Vec};

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerV2, funding_ledger_bytes_v2,
};
use dclutch_market_core_codec::{PROJECT_FOUND_RECEIPT_BYTES_V2, ProjectFoundReceiptV2};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PRE_MARKET_FUNDING_REQUEST_BYTES_V1, PRE_MARKET_FUNDING_REQUEST_MAGIC_V1,
    PreMarketFundingReceiptV1, PreMarketFundingRequestV1, RESOLUTION_CONTROLLER_RELEASE_ID_V5,
    pre_market_funding_prestate_digest_v1,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    RecordKind, ResolutionError, authenticate_finalized_record, authenticate_rent,
    deployment_observation,
};

/// Fixed deployment-authenticated accounts before Core's ProjectFound frame.
pub const PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1: usize = 7;
/// Exact total account count for pre-Market subset-ledger initialization.
pub const PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1: usize =
    PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1 + 37;

const CALLER_AUTHORITY: usize = 0;
const CALLER_PROGRAM: usize = 1;
const CALLER_PROGRAMDATA: usize = 2;
const RESOLUTION_PROGRAM: usize = 3;
const RESOLUTION_PROGRAMDATA: usize = 4;
const FUNDING_SOURCE: usize = 5;
const LEDGER: usize = 6;
const FOUND_START: usize = PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1;
const FOUND_RENT_PROGRAM: usize = 3;
const FOUND_RENT_CREDIT: usize = 2;
const FOUND_MANIFEST_RAW: usize = 22;
const FOUND_MANIFEST_STAGING: usize = 23;
const FOUND_ACTIVATION_CACHE: usize = 24;
const FOUND_CORE_PROGRAM: usize = 25;
const FOUND_REGISTRY_PROGRAM: usize = 27;
const FOUND_RENT: usize = 28;
const FOUND_SYSTEM: usize = 29;

/// Return whether bytes select the pre-Market subset-ledger initializer.
pub fn is_pre_market_funding_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == PRE_MARKET_FUNDING_REQUEST_BYTES_V1
        && instruction_data.get(..PRE_MARKET_FUNDING_REQUEST_MAGIC_V1.len())
            == Some(PRE_MARKET_FUNDING_REQUEST_MAGIC_V1.as_slice())
}

/// Project the exact future Market and initialize its Resolution-owned ledger.
pub fn process_pre_market_funding_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = PreMarketFundingRequestV1::decode(instruction_data)
        .map_err(|_| ResolutionError::Instruction)?;
    authenticate_frame(program_id, accounts, request)?;
    let found = accounts
        .get(FOUND_START..)
        .ok_or(ResolutionError::AccountFrame)?;
    let core_program = found
        .get(FOUND_CORE_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    let receipt = project_found(core_program, found, request)?;
    let exact_found = request
        .project_found
        .found
        .encode()
        .map_err(|_| ResolutionError::Instruction)?;
    receipt
        .verify_found_request(hash(&exact_found).to_bytes())
        .map_err(|_| ResolutionError::MarketAuthority)?;
    if receipt.market.to_bytes() != request.project_found.found.market.to_bytes()
        || receipt.generation != request.project_found.found.generation
    {
        return Err(ResolutionError::MarketAuthority.into());
    }

    let manifest_raw = found
        .get(FOUND_MANIFEST_RAW)
        .ok_or(ResolutionError::AccountFrame)?;
    let manifest_staging = found
        .get(FOUND_MANIFEST_STAGING)
        .ok_or(ResolutionError::AccountFrame)?;
    let registry_program = found
        .get(FOUND_REGISTRY_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    let rent_account = found.get(FOUND_RENT).ok_or(ResolutionError::AccountFrame)?;
    let system = found
        .get(FOUND_SYSTEM)
        .ok_or(ResolutionError::AccountFrame)?;
    let rent = authenticate_rent(rent_account)?;
    let manifest_data = manifest_raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *registry_program.key,
        manifest_raw,
        manifest_staging,
        &rent,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let canonical_mask = resolution_mask(manifest)?;
    if canonical_mask != request.selected_mask {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_release_and_caller(program_id, accounts, found, request, receipt)?;

    let manifest_id =
        CapabilityContentId::new(request.manifest).map_err(|_| ResolutionError::Funding)?;
    let width = funding_ledger_bytes_v2(3).map_err(|_| ResolutionError::Funding)?;
    let mut ledger_bytes = vec![0_u8; width];
    FundingLedgerV2::initialize(
        &mut ledger_bytes,
        manifest_id,
        manifest,
        request.selected_mask,
    )
    .map_err(|_| ResolutionError::Funding)?;
    let ledger_view = FundingLedgerV2::decode(&ledger_bytes)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .map_err(|_| ResolutionError::Funding)?;
    let native_principal = ledger_view
        .remaining_native_lamports_total()
        .map_err(|_| ResolutionError::Funding)?;
    for entry_index in 0_u16..manifest.entry_count() {
        if request.selected_mask & (1_u16 << entry_index) != 0
            && manifest
                .entry(entry_index)
                .map_err(|_| ResolutionError::Funding)?
                .funding_quote()
                .realm_collateral()
                .is_some()
        {
            return Err(ResolutionError::Funding.into());
        }
    }
    let exact_rent = rent.minimum_balance(width);
    let target = exact_rent
        .checked_add(native_principal)
        .ok_or(ResolutionError::Arithmetic)?;
    let funding_source = accounts
        .get(FUNDING_SOURCE)
        .ok_or(ResolutionError::AccountFrame)?;
    let ledger = accounts.get(LEDGER).ok_or(ResolutionError::AccountFrame)?;
    authenticate_vacant_ledger(ledger, request.ledger, request.prestate_digest)?;
    require_funding_source(funding_source.lamports(), target)?;
    invoke(
        &transfer(funding_source.key, ledger.key, target),
        &[funding_source.clone(), ledger.clone(), system.clone()],
    )
    .map_err(|_| ResolutionError::Funding)?;
    initialize_ledger(
        program_id,
        ledger,
        receipt.market.to_bytes(),
        receipt.generation,
        manifest_id,
        &ledger_bytes,
        system,
    )?;
    {
        let mut output = ledger
            .try_borrow_mut_data()
            .map_err(|_| ResolutionError::OutputState)?;
        if output.len() != ledger_bytes.len() || output.iter().any(|byte| *byte != 0) {
            return Err(ResolutionError::OutputState.into());
        }
        output.copy_from_slice(&ledger_bytes);
    }
    if ledger.lamports() != target {
        return Err(ResolutionError::Funding.into());
    }
    let receipt = PreMarketFundingReceiptV1 {
        market: receipt.market.to_bytes(),
        generation: receipt.generation,
        manifest: request.manifest,
        selected_mask: request.selected_mask,
        ledger: request.ledger,
        prestate_digest: request.prestate_digest,
        poststate_digest: hash(&ledger_bytes).to_bytes(),
        exact_rent_lamports: exact_rent,
        exact_native_principal: native_principal,
        found_request_digest: receipt.found_request_digest,
        funding_source: request.funding_source,
        rent_credit: found
            .get(FOUND_RENT_CREDIT)
            .ok_or(ResolutionError::AccountFrame)?
            .key
            .to_bytes(),
    };
    set_return_data(&receipt.encode().map_err(|_| ResolutionError::Instruction)?);
    Ok(())
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: PreMarketFundingRequestV1,
) -> ProgramResult {
    if accounts.len() != PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1 {
        return Err(ResolutionError::AccountFrame.into());
    }
    let authority = accounts
        .get(CALLER_AUTHORITY)
        .ok_or(ResolutionError::AccountFrame)?;
    let caller = accounts
        .get(CALLER_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    let caller_programdata = accounts
        .get(CALLER_PROGRAMDATA)
        .ok_or(ResolutionError::AccountFrame)?;
    let resolution_program = accounts
        .get(RESOLUTION_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    let resolution_programdata = accounts
        .get(RESOLUTION_PROGRAMDATA)
        .ok_or(ResolutionError::AccountFrame)?;
    let funding = accounts
        .get(FUNDING_SOURCE)
        .ok_or(ResolutionError::AccountFrame)?;
    let ledger = accounts.get(LEDGER).ok_or(ResolutionError::AccountFrame)?;
    if !authority.is_signer
        || authority.is_writable
        || authority.executable
        || !caller.executable
        || caller.is_signer
        || caller.is_writable
        || caller_programdata.is_signer
        || caller_programdata.is_writable
        || caller_programdata.executable
        || resolution_program.key != program_id
        || resolution_program.is_signer
        || resolution_program.is_writable
        || !resolution_program.executable
        || resolution_programdata.is_signer
        || resolution_programdata.is_writable
        || resolution_programdata.executable
        || !funding.is_signer
        || !funding.is_writable
        || funding.executable
        || funding.key.to_bytes() != request.funding_source
        || ledger.key.to_bytes() != request.ledger
        || ledger.is_signer
        || !ledger.is_writable
        || ledger.executable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for prefix_index in 0..PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1 {
        let prefix = accounts
            .get(prefix_index)
            .ok_or(ResolutionError::AccountFrame)?;
        if accounts
            .iter()
            .take(PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1)
            .skip(
                prefix_index
                    .checked_add(1)
                    .ok_or(ResolutionError::Arithmetic)?,
            )
            .any(|other| other.key == prefix.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let found = accounts
        .get(FOUND_START..)
        .ok_or(ResolutionError::AccountFrame)?;
    for (index, account) in found.iter().enumerate() {
        let executable = matches!(
            index,
            FOUND_RENT_PROGRAM | FOUND_CORE_PROGRAM | FOUND_REGISTRY_PROGRAM | FOUND_SYSTEM
        );
        if account.is_signer || account.is_writable || account.executable != executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    for (index, account) in found.iter().enumerate() {
        if found
            .iter()
            .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
            .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    for prefix_index in 0..PRE_MARKET_FUNDING_PREFIX_ACCOUNT_COUNT_V1 {
        let prefix = accounts
            .get(prefix_index)
            .ok_or(ResolutionError::AccountFrame)?;
        if found.iter().any(|account| account.key == prefix.key) {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    if accounts
        .get(FOUND_START + FOUND_CORE_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?
        .key
        == program_id
        || accounts
            .get(FOUND_START + FOUND_SYSTEM)
            .ok_or(ResolutionError::AccountFrame)?
            .key
            != &system_program::ID
        || accounts
            .get(FOUND_START + FOUND_RENT)
            .ok_or(ResolutionError::AccountFrame)?
            .key
            != &sysvar::rent::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

fn project_found(
    core_program: &AccountInfo<'_>,
    found: &[AccountInfo<'_>],
    request: PreMarketFundingRequestV1,
) -> Result<ProjectFoundReceiptV2, solana_program::program_error::ProgramError> {
    let metas: Vec<AccountMeta> = found
        .iter()
        .map(|account| AccountMeta::new_readonly(*account.key, false))
        .collect();
    let instruction = Instruction {
        program_id: *core_program.key,
        accounts: metas,
        data: request
            .project_found
            .encode()
            .map_err(|_| ResolutionError::Instruction)?
            .to_vec(),
    };
    invoke(&instruction, found).map_err(|_| ResolutionError::MarketAuthority)?;
    let (producer, bytes) = get_return_data().ok_or(ResolutionError::MarketAuthority)?;
    if producer != *core_program.key || bytes.len() != PROJECT_FOUND_RECEIPT_BYTES_V2 {
        return Err(ResolutionError::MarketAuthority.into());
    }
    ProjectFoundReceiptV2::decode(&bytes).map_err(|_| ResolutionError::MarketAuthority.into())
}

fn resolution_mask(
    manifest: CapabilityManifestV1<'_>,
) -> Result<u16, solana_program::program_error::ProgramError> {
    let mut mask = 0_u16;
    for entry_index in 0_u16..manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| ResolutionError::Funding)?;
        if entry.release_id().to_bytes() == RESOLUTION_CONTROLLER_RELEASE_ID_V5 {
            mask |= 1_u16 << entry_index;
        }
    }
    if mask.count_ones() != 3 {
        return Err(ResolutionError::Funding.into());
    }
    Ok(mask)
}

fn authenticate_release_and_caller(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    found: &[AccountInfo<'_>],
    request: PreMarketFundingRequestV1,
    receipt: ProjectFoundReceiptV2,
) -> ProgramResult {
    let cache = found
        .get(FOUND_ACTIVATION_CACHE)
        .ok_or(ResolutionError::AccountFrame)?;
    let registry = found
        .get(FOUND_REGISTRY_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    let data = cache
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if cache.owner != registry.key
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &receipt.release_set.to_bytes()],
            registry.key,
        )
        .0 != *cache.key
        || activated
            .execution_release_set_id()
            .map_err(|_| ResolutionError::ResolutionRelease)?
            .to_bytes()
            != receipt.release_set.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let caller_program = accounts
        .get(CALLER_PROGRAM)
        .ok_or(ResolutionError::AccountFrame)?;
    if trading.release().program().to_bytes() != caller_program.key.to_bytes()
        || resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V5
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    trading
        .authenticate_current_deployment(deployment_observation(
            caller_program,
            accounts
                .get(CALLER_PROGRAMDATA)
                .ok_or(ResolutionError::AccountFrame)?,
            trading.release().programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    resolution
        .authenticate_current_deployment(deployment_observation(
            accounts
                .get(RESOLUTION_PROGRAM)
                .ok_or(ResolutionError::AccountFrame)?,
            accounts
                .get(RESOLUTION_PROGRAMDATA)
                .ok_or(ResolutionError::AccountFrame)?,
            resolution.release().programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let digest = hash(&request.encode().map_err(|_| ResolutionError::Instruction)?).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        receipt.release_set.to_bytes(),
        receipt.market.to_bytes(),
        ExecutionRoleV1::Trading,
        request.manifest,
        digest,
    )
    .map_err(|_| ResolutionError::ResolutionRelease)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), caller_program.key).0;
    if accounts
        .get(CALLER_AUTHORITY)
        .ok_or(ResolutionError::AccountFrame)?
        .key
        != &expected
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    Ok(())
}

fn prestate_digest(
    ledger: &AccountInfo<'_>,
) -> Result<[u8; 32], solana_program::program_error::ProgramError> {
    let data_len = u64::try_from(ledger.data_len()).map_err(|_| ResolutionError::Arithmetic)?;
    Ok(pre_market_funding_prestate_digest_v1(
        ledger.key.to_bytes(),
        ledger.owner.to_bytes(),
        ledger.lamports(),
        data_len,
    ))
}

fn authenticate_vacant_ledger(
    ledger: &AccountInfo<'_>,
    expected_key: [u8; 32],
    expected_digest: [u8; 32],
) -> ProgramResult {
    if ledger.key.to_bytes() != expected_key
        || ledger.owner != &system_program::ID
        || ledger.executable
        || ledger.data_len() != 0
        || ledger.lamports() != 0
        || prestate_digest(ledger)? != expected_digest
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn require_funding_source(observed_lamports: u64, exact_target: u64) -> ProgramResult {
    if observed_lamports < exact_target {
        Err(ResolutionError::Funding.into())
    } else {
        Ok(())
    }
}

fn initialize_ledger<'info>(
    program_id: &Pubkey,
    output: &AccountInfo<'info>,
    market: [u8; 32],
    generation: u64,
    manifest_id: CapabilityContentId,
    ledger_bytes: &[u8],
    system: &AccountInfo<'info>,
) -> ProgramResult {
    let ledger = FundingLedgerV2::decode(ledger_bytes).map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        market,
        generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *output.key {
        return Err(ResolutionError::OutputState.into());
    }
    let (_, bump) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    let components = derivation.seed_components();
    let bump_seed = [bump];
    let [domain, controller, market, generation, manifest, mask] = components;
    let signer: [&[u8]; 7] = [
        domain, controller, market, generation, manifest, mask, &bump_seed,
    ];
    invoke_signed(
        &allocate(
            output.key,
            u64::try_from(ledger_bytes.len()).map_err(|_| ResolutionError::Arithmetic)?,
        ),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market_core_codec::{Action, Identity, ProjectFoundRequestV2, Request};

    #[test]
    fn request_detector_is_version_and_width_exact() {
        let request = PreMarketFundingRequestV1 {
            project_found: ProjectFoundRequestV2::new(Request::administrative(
                Action::Found,
                7,
                Identity::new([1; 32]).expect("market"),
            ))
            .expect("ProjectFound"),
            manifest: [2; 32],
            selected_mask: 0b111,
            funding_source: [3; 32],
            ledger: [4; 32],
            prestate_digest: [5; 32],
        }
        .encode()
        .expect("request");
        assert!(is_pre_market_funding_v1(&request));
        assert!(!is_pre_market_funding_v1(&request[..request.len() - 1]));
        let mut wrong = request;
        wrong[0] ^= 1;
        assert!(!is_pre_market_funding_v1(&wrong));
    }

    #[test]
    fn replay_substitution_and_partial_funding_refuse() {
        let key = Pubkey::new_from_array([1; 32]);
        let mut lamports = 0_u64;
        let mut data = [];
        let owner = system_program::ID;
        let exact = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let digest = prestate_digest(&exact).expect("prestate digest");
        assert_eq!(
            authenticate_vacant_ledger(&exact, key.to_bytes(), digest),
            Ok(())
        );
        assert_eq!(
            authenticate_vacant_ledger(&exact, [2; 32], digest),
            Err(ResolutionError::OutputState.into())
        );
        assert_eq!(
            require_funding_source(99, 100),
            Err(ResolutionError::Funding.into())
        );
        assert_eq!(require_funding_source(100, 100), Ok(()));

        let occupied_key = Pubkey::new_from_array([3; 32]);
        let mut occupied_lamports = 1_u64;
        let mut occupied_data = [0_u8; 1];
        let occupied_owner = Pubkey::new_from_array([4; 32]);
        let replay = AccountInfo::new(
            &occupied_key,
            false,
            true,
            &mut occupied_lamports,
            &mut occupied_data,
            &occupied_owner,
            false,
        );
        let replay_digest = prestate_digest(&replay).expect("occupied digest");
        assert_eq!(
            authenticate_vacant_ledger(&replay, occupied_key.to_bytes(), replay_digest),
            Err(ResolutionError::OutputState.into())
        );
    }
}
