//! Expiry rollback for one Resolution-owned pre-Market funding ledger.

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    ControllerFundingCheckpointAbortKindV1, ControllerFundingCheckpointDerivationV1,
    ControllerFundingCheckpointV1, ControllerFundingControllerV1, FundingLedgerStatusV2,
    FundingLedgerV2, controller_funding_ledger_account_digest_v1,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1, PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1,
    PreMarketFundingAbortReceiptV1, PreMarketFundingAbortRequestV1,
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, pre_market_funding_ledger_account_digest_v1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    RecordKind, ResolutionError, authenticate_clock, authenticate_finalized_record,
    authenticate_rent, cached_deployment_observation,
};

/// Exact expiry-close account count.
pub const PRE_MARKET_FUNDING_ABORT_ACCOUNT_COUNT_V1: usize = 16;

const CALLER_AUTHORITY: usize = 0;
const CALLER_PROGRAM: usize = 1;
const CALLER_PROGRAMDATA: usize = 2;
const RESOLUTION_PROGRAM: usize = 3;
const RESOLUTION_PROGRAMDATA: usize = 4;
const CHECKPOINT: usize = 5;
const LEDGER: usize = 6;
const FUNDING_SOURCE: usize = 7;
const RENT_CREDIT: usize = 8;
const ACTIVATION_CACHE: usize = 9;
const REGISTRY_PROGRAM: usize = 10;
const MANIFEST_RAW: usize = 11;
const MANIFEST_STAGING: usize = 12;
const RENT: usize = 13;
const CLOCK: usize = 14;
const SYSTEM: usize = 15;

/// Return whether bytes select the exact expiry-close action.
pub fn is_pre_market_funding_abort_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1
        && instruction_data.get(..PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1.len())
            == Some(PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1.as_slice())
}

/// Authenticate one expired checkpoint and close its exact Pending Resolution ledger.
#[inline(never)]
pub fn process_pre_market_funding_abort_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = PreMarketFundingAbortRequestV1::decode(instruction_data)
        .map_err(|_| ResolutionError::Instruction)?;
    authenticate_frame(program_id, accounts, request)?;
    let rent = authenticate_rent(account(accounts, RENT)?)?;
    let clock = authenticate_clock(account(accounts, CLOCK)?)?;
    let expected_resolution_ledger_digest =
        authenticate_live_cleanup_checkpoint(program_id, accounts, request, clock.slot)?;
    let ledger = account(accounts, LEDGER)?;

    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let manifest_raw = account(accounts, MANIFEST_RAW)?;
    let manifest_staging = account(accounts, MANIFEST_STAGING)?;
    let manifest_data = manifest_raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *registry.key,
        manifest_raw,
        manifest_staging,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id =
        CapabilityContentId::new(request.manifest).map_err(|_| ResolutionError::Funding)?;
    let ledger_data = ledger
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    if ledger.owner != program_id
        || hash(&ledger_data).to_bytes() != expected_resolution_ledger_digest
        || pre_market_funding_ledger_account_digest_v1(
            ledger.key.to_bytes(),
            ledger.owner.to_bytes(),
            ledger.lamports(),
            &ledger_data,
        ) != request.ledger_account_digest
    {
        return Err(ResolutionError::Funding.into());
    }
    let decoded = FundingLedgerV2::decode(&ledger_data).map_err(|_| ResolutionError::Funding)?;
    if decoded.selected_mask() != request.selected_mask {
        return Err(ResolutionError::Funding.into());
    }
    let authenticated = decoded
        .authenticate(manifest_id, manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        if request.selected_mask & (1_u16 << index) != 0
            && authenticated
                .slot(index)
                .map_err(|_| ResolutionError::Funding)?
                .status()
                != FundingLedgerStatusV2::Pending
        {
            return Err(ResolutionError::Funding.into());
        }
        index = index.checked_add(1).ok_or(ResolutionError::Arithmetic)?;
    }
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        request.market,
        request.generation,
        manifest_id,
        decoded,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *ledger.key {
        return Err(ResolutionError::Funding.into());
    }
    let exact_rent = rent.minimum_balance(ledger_data.len());
    let native_principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| ResolutionError::Funding)?;
    authenticated
        .validate_native_custody(ledger.lamports(), exact_rent, false)
        .map_err(|_| ResolutionError::Funding)?;
    drop(ledger_data);

    close_ledger(
        ledger,
        account(accounts, FUNDING_SOURCE)?,
        account(accounts, RENT_CREDIT)?,
        native_principal,
        exact_rent,
    )?;
    let closed_digest = pre_market_funding_ledger_account_digest_v1(
        ledger.key.to_bytes(),
        ledger.owner.to_bytes(),
        ledger.lamports(),
        &[],
    );
    let total = native_principal
        .checked_add(exact_rent)
        .ok_or(ResolutionError::Arithmetic)?;
    let receipt = PreMarketFundingAbortReceiptV1 {
        checkpoint_phase: request.checkpoint_phase,
        checkpoint_revision: request.checkpoint_revision,
        request_digest: hash(instruction_data).to_bytes(),
        release_set: request.release_set,
        checkpoint: request.checkpoint,
        checkpoint_digest: request.checkpoint_digest,
        market: request.market,
        generation: request.generation,
        manifest: request.manifest,
        funding_list: request.funding_list,
        selected_mask: request.selected_mask,
        ledger: request.ledger,
        ledger_account_digest: request.ledger_account_digest,
        funding_source: request.funding_source,
        rent_credit: request.rent_credit,
        expiry_slot: request.expiry_slot,
        native_principal_refund_lamports: native_principal,
        rent_refund_lamports: exact_rent,
        total_refund_lamports: total,
        closed_account_digest: closed_digest,
        producer: program_id.to_bytes(),
    };
    set_return_data(&receipt.encode().map_err(|_| ResolutionError::Instruction)?);
    Ok(())
}

/// Decode the enlarged durable checkpoint in an isolated SBF frame.
///
/// Keeping this 768-byte value alive alongside the manifest, authenticated
/// ledger, and 488-byte terminal child receipt exceeds the SBF 4 KiB frame.
/// Only the immutable Resolution-ledger data digest is needed after this
/// authentication boundary, so return that scalar fact and drop the typed
/// checkpoint before the economic close begins.
#[inline(never)]
fn authenticate_live_cleanup_checkpoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: PreMarketFundingAbortRequestV1,
    current_slot: u64,
) -> Result<[u8; 32], ProgramError> {
    let checkpoint_account = account(accounts, CHECKPOINT)?;
    let checkpoint_data = checkpoint_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    if hash(&checkpoint_data).to_bytes() != request.checkpoint_digest {
        return Err(ResolutionError::Funding.into());
    }
    let checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_data)
        .map_err(|_| ResolutionError::Funding)?;
    drop(checkpoint_data);
    let ledger = account(accounts, LEDGER)?;
    let ledger_data = ledger
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let controller_ledger_account_digest = controller_funding_ledger_account_digest_v1(
        ledger.key.to_bytes(),
        ledger.owner.to_bytes(),
        ledger.lamports(),
        &ledger_data,
    );
    drop(ledger_data);
    authenticate_checkpoint(
        checkpoint,
        checkpoint_account,
        account(accounts, CALLER_PROGRAM)?,
        request,
        current_slot,
        controller_ledger_account_digest,
    )?;
    let expected = checkpoint.input().resolution_ledger_digest;
    if ledger.owner != program_id {
        return Err(ResolutionError::Funding.into());
    }
    Ok(expected)
}

fn authenticate_checkpoint(
    checkpoint: ControllerFundingCheckpointV1,
    checkpoint_account: &AccountInfo<'_>,
    caller_program: &AccountInfo<'_>,
    request: PreMarketFundingAbortRequestV1,
    current_slot: u64,
    controller_ledger_account_digest: [u8; 32],
) -> ProgramResult {
    let input = checkpoint.input();
    if checkpoint_account.owner != caller_program.key
        || input.release_set != request.release_set
        || input.market != request.market
        || input.generation != request.generation
        || input.manifest != request.manifest
        || input.funding_list != request.funding_list
        || input.resolution_mask != request.selected_mask
        || input.resolution_ledger != request.ledger
        || input.funding_source != request.funding_source
        || input.rent_credit != request.rent_credit
        || input.expiry_slot != request.expiry_slot
    {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_resolution_cleanup_position(
        checkpoint,
        request.checkpoint_phase,
        request.checkpoint_revision,
        request.selected_mask,
        current_slot,
        controller_ledger_account_digest,
    )?;
    let derivation = ControllerFundingCheckpointDerivationV1::new(
        input.release_set,
        input.market,
        input.generation,
        input.manifest,
        input.funding_list,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), caller_program.key).0
        != *checkpoint_account.key
    {
        return Err(ResolutionError::Funding.into());
    }
    Ok(())
}

/// Bind Resolution's close to its exact position in the durable cleanup prefix.
///
/// Prepared and CustodyAborted are first-close phases. Once a first close has
/// persisted, Resolution may only consume the checkpoint when it is the
/// canonical remaining controller and the live account state is the exact
/// prestate committed by that prefix. CustodyStaged is deliberately absent:
/// Custody rollback must become durable before either controller ledger moves.
fn authenticate_resolution_cleanup_position(
    checkpoint: ControllerFundingCheckpointV1,
    checkpoint_phase: u8,
    checkpoint_revision: u64,
    selected_mask: u16,
    current_slot: u64,
    controller_ledger_account_digest: [u8; 32],
) -> ProgramResult {
    if checkpoint.phase() as u8 != checkpoint_phase
        || checkpoint.revision() != checkpoint_revision
        || checkpoint.controller_mask(ControllerFundingControllerV1::Resolution) != selected_mask
    {
        return Err(ResolutionError::Funding.into());
    }
    let kind = checkpoint
        .authenticate_expiry_abort(current_slot)
        .map_err(|_| ResolutionError::Funding)?;
    match (checkpoint_phase, kind) {
        (1, ControllerFundingCheckpointAbortKindV1::PreparedExpired)
        | (3, ControllerFundingCheckpointAbortKindV1::CustodyAborted) => {
            if checkpoint.canonical_first_controller() != ControllerFundingControllerV1::Resolution
            {
                return Err(ResolutionError::Funding.into());
            }
        }
        (4 | 5, ControllerFundingCheckpointAbortKindV1::FirstLedgerClosed) => {
            let cleanup = checkpoint.cleanup().ok_or(ResolutionError::Funding)?;
            if checkpoint.canonical_remaining_controller()
                != ControllerFundingControllerV1::Resolution
                || cleanup.remaining_ledger_prestate_digest() != controller_ledger_account_digest
            {
                return Err(ResolutionError::Funding.into());
            }
        }
        _ => return Err(ResolutionError::Funding.into()),
    }
    Ok(())
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: PreMarketFundingAbortRequestV1,
) -> ProgramResult {
    if accounts.len() != PRE_MARKET_FUNDING_ABORT_ACCOUNT_COUNT_V1 {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, item) in accounts.iter().enumerate() {
        let signer = index == CALLER_AUTHORITY;
        let writable = matches!(index, LEDGER | FUNDING_SOURCE | RENT_CREDIT);
        let executable = matches!(
            index,
            CALLER_PROGRAM | RESOLUTION_PROGRAM | REGISTRY_PROGRAM | SYSTEM
        );
        if item.is_signer != signer || item.is_writable != writable || item.executable != executable
        {
            return Err(ResolutionError::AccountFrame.into());
        }
        if accounts
            .iter()
            .skip(index + 1)
            .any(|other| other.key == item.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    if account(accounts, RESOLUTION_PROGRAM)?.key != program_id
        || account(accounts, CHECKPOINT)?.key.to_bytes() != request.checkpoint
        || account(accounts, LEDGER)?.key.to_bytes() != request.ledger
        || account(accounts, FUNDING_SOURCE)?.key.to_bytes() != request.funding_source
        || account(accounts, RENT_CREDIT)?.key.to_bytes() != request.rent_credit
        || account(accounts, RENT)?.key != &sysvar::rent::ID
        || account(accounts, CLOCK)?.key != &sysvar::clock::ID
        || account(accounts, SYSTEM)?.key != &system_program::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    authenticate_release_and_caller(program_id, accounts, request)
}

fn authenticate_release_and_caller(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: PreMarketFundingAbortRequestV1,
) -> ProgramResult {
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| ResolutionError::ActivationCache)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| ResolutionError::ActivationCache)?;
    if cache.owner != registry.key
        || activated
            .execution_release_set_id()
            .map_err(|_| ResolutionError::ActivationCache)?
            .to_bytes()
            != request.release_set
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
            registry.key,
        )
        .0 != *cache.key
    {
        return Err(ResolutionError::ActivationCache.into());
    }
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| ResolutionError::ActivatedRole)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let caller = account(accounts, CALLER_PROGRAM)?;
    if trading.release().program().to_bytes() != caller.key.to_bytes() {
        return Err(ResolutionError::ActivatedRole.into());
    }
    if resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    trading
        .authenticate_current_deployment(cached_deployment_observation(
            caller,
            account(accounts, CALLER_PROGRAMDATA)?,
            trading.release(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    resolution
        .authenticate_current_deployment(cached_deployment_observation(
            account(accounts, RESOLUTION_PROGRAM)?,
            account(accounts, RESOLUTION_PROGRAMDATA)?,
            resolution.release(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.manifest,
        hash(&request.encode().map_err(|_| ResolutionError::Instruction)?).to_bytes(),
    )
    .map_err(|_| ResolutionError::CallerAuthority)?;
    if Pubkey::find_program_address(&seeds.as_slices(), caller.key).0
        != *account(accounts, CALLER_AUTHORITY)?.key
    {
        return Err(ResolutionError::CallerAuthority.into());
    }
    Ok(())
}

fn close_ledger(
    ledger: &AccountInfo<'_>,
    funding_source: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    principal: u64,
    rent: u64,
) -> ProgramResult {
    let total = principal
        .checked_add(rent)
        .ok_or(ResolutionError::Arithmetic)?;
    {
        let mut source = funding_source
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::Funding)?;
        **source = source
            .checked_add(principal)
            .ok_or(ResolutionError::Arithmetic)?;
    }
    {
        let mut beneficiary = rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::Funding)?;
        **beneficiary = beneficiary
            .checked_add(rent)
            .ok_or(ResolutionError::Arithmetic)?;
    }
    {
        let mut custody = ledger
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::Funding)?;
        **custody = custody
            .checked_sub(total)
            .ok_or(ResolutionError::Arithmetic)?;
    }
    ledger.resize(0).map_err(|_| ResolutionError::OutputState)?;
    ledger.assign(&system_program::ID);
    if ledger.lamports() != 0 || ledger.data_len() != 0 || ledger.owner != &system_program::ID {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, solana_program::program_error::ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ResolutionError::AccountFrame.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::ControllerFundingCheckpointInputV1;

    const EXPIRY_SLOT: u64 = 90;

    fn checkpoint_input(resolution_first: bool) -> ControllerFundingCheckpointInputV1 {
        let (resolution_mask, trading_mask) = if resolution_first {
            (0b0111, 0b1000)
        } else {
            (0b1110, 0b0001)
        };
        ControllerFundingCheckpointInputV1 {
            release_set: [1; 32],
            market: [2; 32],
            generation: 3,
            manifest: [4; 32],
            funding_list: [5; 32],
            found_request_digest: [6; 32],
            project_found_receipt_digest: [7; 32],
            resolution_ledger: [8; 32],
            resolution_ledger_digest: [9; 32],
            trading_ledger: [10; 32],
            trading_ledger_digest: [11; 32],
            funding_source: [12; 32],
            rent_credit: [13; 32],
            lock_request_digest: [14; 32],
            expiry_slot: EXPIRY_SLOT,
            prepared_slot: 40,
            resolution_mask,
            trading_mask,
        }
    }

    fn prepared(resolution_first: bool) -> ControllerFundingCheckpointV1 {
        ControllerFundingCheckpointV1::prepared(checkpoint_input(resolution_first))
            .expect("prepared checkpoint")
    }

    #[test]
    fn detector_and_phase_wires_are_exact() {
        let request = PreMarketFundingAbortRequestV1 {
            checkpoint_phase: 1,
            checkpoint_revision: 1,
            release_set: [1; 32],
            checkpoint: [2; 32],
            checkpoint_digest: [3; 32],
            market: [4; 32],
            generation: 5,
            manifest: [6; 32],
            funding_list: [7; 32],
            selected_mask: 0b111,
            ledger: [8; 32],
            ledger_account_digest: [9; 32],
            funding_source: [10; 32],
            rent_credit: [11; 32],
            expiry_slot: 12,
        }
        .encode()
        .expect("request");
        assert!(is_pre_market_funding_abort_v1(&request));
        assert!(!is_pre_market_funding_abort_v1(
            &request[..request.len() - 1]
        ));
        let mut legacy = request;
        legacy[..8].copy_from_slice(b"DCLRPFQ1");
        assert!(!is_pre_market_funding_abort_v1(&legacy));
    }

    #[test]
    fn first_close_accepts_resolution_only_in_canonical_position() {
        let exact = prepared(true);
        assert!(
            authenticate_resolution_cleanup_position(
                exact,
                1,
                1,
                exact.input().resolution_mask,
                EXPIRY_SLOT + 1,
                [21; 32],
            )
            .is_ok()
        );
        for hostile in [
            authenticate_resolution_cleanup_position(
                prepared(false),
                1,
                1,
                prepared(false).input().resolution_mask,
                EXPIRY_SLOT + 1,
                [21; 32],
            ),
            authenticate_resolution_cleanup_position(
                exact,
                2,
                2,
                exact.input().resolution_mask,
                EXPIRY_SLOT + 1,
                [21; 32],
            ),
            authenticate_resolution_cleanup_position(
                exact,
                1,
                1,
                exact.input().trading_mask,
                EXPIRY_SLOT + 1,
                [21; 32],
            ),
        ] {
            assert!(hostile.is_err());
        }

        let custody_aborted = prepared(true)
            .stage_custody(50, [15; 32])
            .expect("staged")
            .abort_custody(EXPIRY_SLOT + 1, [16; 32], [17; 32], [18; 32])
            .expect("custody aborted");
        assert!(
            authenticate_resolution_cleanup_position(
                custody_aborted,
                3,
                3,
                custody_aborted.input().resolution_mask,
                EXPIRY_SLOT + 2,
                [21; 32],
            )
            .is_ok()
        );
        let wrong_first = prepared(false)
            .stage_custody(50, [15; 32])
            .expect("staged")
            .abort_custody(EXPIRY_SLOT + 1, [16; 32], [17; 32], [18; 32])
            .expect("custody aborted");
        assert!(
            authenticate_resolution_cleanup_position(
                wrong_first,
                3,
                3,
                wrong_first.input().resolution_mask,
                EXPIRY_SLOT + 2,
                [21; 32],
            )
            .is_err()
        );
    }

    #[test]
    fn remaining_close_requires_resolution_and_exact_persisted_live_digest() {
        let remaining_digest = [31; 32];
        let trading_first = prepared(false);
        let prepared_prefix = trading_first
            .close_first_ledger(
                EXPIRY_SLOT + 1,
                [20; 32],
                ControllerFundingControllerV1::Trading,
                trading_first.input().trading_mask,
                [21; 32],
                [22; 32],
                [23; 32],
                remaining_digest,
                24,
                25,
            )
            .expect("prepared cleanup prefix");
        assert!(
            authenticate_resolution_cleanup_position(
                prepared_prefix,
                4,
                4,
                prepared_prefix.input().resolution_mask,
                EXPIRY_SLOT + 2,
                remaining_digest,
            )
            .is_ok()
        );
        assert!(
            authenticate_resolution_cleanup_position(
                prepared_prefix,
                4,
                4,
                prepared_prefix.input().resolution_mask,
                EXPIRY_SLOT + 2,
                [32; 32],
            )
            .is_err()
        );

        let staged = prepared(false)
            .stage_custody(50, [15; 32])
            .expect("staged")
            .abort_custody(EXPIRY_SLOT + 1, [16; 32], [17; 32], [18; 32])
            .expect("custody aborted");
        let custody_prefix = staged
            .close_first_ledger(
                EXPIRY_SLOT + 2,
                [26; 32],
                ControllerFundingControllerV1::Trading,
                staged.input().trading_mask,
                [21; 32],
                [22; 32],
                [23; 32],
                remaining_digest,
                24,
                25,
            )
            .expect("custody cleanup prefix");
        assert!(
            authenticate_resolution_cleanup_position(
                custody_prefix,
                5,
                5,
                custody_prefix.input().resolution_mask,
                EXPIRY_SLOT + 3,
                remaining_digest,
            )
            .is_ok()
        );

        let resolution_first = prepared(true);
        let resolution_already_closed = resolution_first
            .close_first_ledger(
                EXPIRY_SLOT + 1,
                [20; 32],
                ControllerFundingControllerV1::Resolution,
                resolution_first.input().resolution_mask,
                [21; 32],
                [22; 32],
                [23; 32],
                remaining_digest,
                24,
                25,
            )
            .expect("resolution-first prefix");
        assert!(
            authenticate_resolution_cleanup_position(
                resolution_already_closed,
                4,
                4,
                resolution_already_closed.input().resolution_mask,
                EXPIRY_SLOT + 2,
                remaining_digest,
            )
            .is_err()
        );
    }
}
