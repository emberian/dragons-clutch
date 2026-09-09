//! Terminal close of one never-activated Direct funding ledger.
//!
//! Founding creates the Trading-owned prepaid-lazy ledger before Direct's
//! first use creates its root. A Market that reaches retirement without a
//! trade therefore has a live Pending ledger and no root. This route proves
//! that exact split from the immutable manifest and Market, then returns the
//! ledger's native principal and recorded rent to the Market RentCredit. It
//! cannot close an Active ledger and never changes Core capability count.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, FundingLedgerStatusV2,
    FundingLedgerV2, funding::funded_rent_persists_v1,
};
use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_market::rent::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CapabilityExecutionSelectionV1, ExecutionRoleV1};
use dclutch_trading::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3;
use dclutch_trading::retirement_v1::DirectCloseUnusedRequestV1;
pub use dclutch_trading::retirement_v1::is_direct_close_unused_v1;
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::TradingSbfError;

/// Exact account count.
pub const DIRECT_CLOSE_UNUSED_ACCOUNT_COUNT_V1: usize = 14;

const MARKET: usize = 0;
const LEDGER: usize = 1;
const VACANT_ROOT: usize = 2;
const RENT_CREDIT: usize = 3;
const MANIFEST_RAW: usize = 4;
const MANIFEST_STAGING: usize = 5;
const ACTIVATION_CACHE: usize = 6;
const CORE_PROGRAM: usize = 7;
const CORE_PROGRAMDATA: usize = 8;
const TRADING_PROGRAM: usize = 9;
const TRADING_PROGRAMDATA: usize = 10;
const REGISTRY: usize = 11;
const RENT_PROGRAM: usize = 12;
const SYSTEM_PROGRAM: usize = 13;

fn decode_request(input: &[u8]) -> Result<u16, ProgramError> {
    DirectCloseUnusedRequestV1::decode(input)
        .map(|request| request.entry_index)
        .map_err(|_| TradingSbfError::UnsupportedContent.into())
}

/// Close one exact Pending singleton ledger while the corresponding root is absent.
#[inline(never)]
pub fn process_direct_close_unused_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let entry_index = decode_request(instruction_data)?;
    authenticate_frame(program_id, accounts)?;
    let market = authenticate_market(accounts)?;
    authenticate_release(accounts, market.identity.selected_release_set.to_bytes())?;

    let manifest_account = account(accounts, MANIFEST_RAW)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_manifest(accounts, market, &manifest_data)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| TradingSbfError::Content)?;
    let manifest_id = ContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|_| TradingSbfError::Content)?;
    if entry.activation_policy() != ActivationPolicy::PrepaidLazy
        || entry.kind_id().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
    {
        return Err(TradingSbfError::Content.into());
    }
    let selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        manifest_id,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(market.identity.selected_release_set.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        market.identity.market_id.to_bytes(),
        market.identity.generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_root = Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0;
    let root = account(accounts, VACANT_ROOT)?;
    if root.key != &expected_root || root.owner != &system_program::ID || root.data_len() != 0 {
        return Err(TradingSbfError::Root.into());
    }

    let ledger = account(accounts, LEDGER)?;
    let ledger_data = ledger
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let decoded = FundingLedgerV2::decode(&ledger_data).map_err(|_| TradingSbfError::Content)?;
    let selected_bit = 1_u16
        .checked_shl(u32::from(entry_index))
        .ok_or(TradingSbfError::Content)?;
    if decoded.selected_mask() != selected_bit || decoded.slot_count() != 1 {
        return Err(TradingSbfError::Content.into());
    }
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        market.identity.market_id.to_bytes(),
        market.identity.generation,
        manifest_id,
        decoded,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if ledger.owner != program_id
        || Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *ledger.key
    {
        return Err(TradingSbfError::Content.into());
    }
    let authenticated = decoded
        .authenticate(manifest_id, manifest)
        .map_err(|_| TradingSbfError::Content)?;
    if authenticated
        .slot(entry_index)
        .map_err(|_| TradingSbfError::Content)?
        .status()
        != FundingLedgerStatusV2::Pending
    {
        return Err(TradingSbfError::Transition.into());
    }
    let exact_rent = authenticated
        .funded_rent_minimum(ledger_data.len())
        .map_err(|_| TradingSbfError::FundedRent)?;
    let principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| TradingSbfError::Content)?;
    authenticated
        .validate_native_custody(ledger.lamports(), exact_rent, true)
        .map_err(|_| TradingSbfError::FundedRent)?;
    authenticate_rent_credit(accounts, market)?;
    drop(ledger_data);
    drop(manifest_data);
    close_to_rent_credit(
        ledger,
        account(accounts, RENT_CREDIT)?,
        principal,
        exact_rent,
    )
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if accounts.len() != DIRECT_CLOSE_UNUSED_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, value) in accounts.iter().enumerate() {
        let writable = matches!(index, LEDGER | RENT_CREDIT);
        let executable = matches!(
            index,
            CORE_PROGRAM | TRADING_PROGRAM | REGISTRY | RENT_PROGRAM | SYSTEM_PROGRAM
        );
        if value.is_signer
            || value.is_writable != writable
            || value.executable != executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == value.key)
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    if account(accounts, TRADING_PROGRAM)?.key != program_id
        || account(accounts, SYSTEM_PROGRAM)?.key != &system_program::ID
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_market(accounts: &[AccountInfo<'_>]) -> Result<CoreState, ProgramError> {
    let market_account = account(accounts, MARKET)?;
    let core = account(accounts, CORE_PROGRAM)?;
    let registry = account(accounts, REGISTRY)?;
    let bytes = market_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if market_account.owner != core.key || bytes.len() != STATE_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let state = CoreState::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    if state
        .encode()
        .map_err(|_| TradingSbfError::Content)?
        .as_slice()
        != bytes.as_ref()
        || state.phase != Phase::Retiring
        || state.outstanding_capabilities != 0
        || state.identity.market_id.to_bytes() != market_account.key.to_bytes()
        || state.identity.registry_program.to_bytes() != registry.key.to_bytes()
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
            core.key,
        )
        .0 != *market_account.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

fn authenticate_manifest(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let registry = account(accounts, REGISTRY)?;
    let raw = account(accounts, MANIFEST_RAW)?;
    let staging = account(accounts, MANIFEST_STAGING)?;
    let digest = market.identity.capability_manifest.to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        registry.key,
    )
    .0;
    if raw.key != &expected_raw
        || raw.owner != registry.key
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_release(
    accounts: &[AccountInfo<'_>],
    release_set: [u8; 32],
) -> Result<(), ProgramError> {
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let registry = account(accounts, REGISTRY)?;
    require_cache_account(registry.key, cache).map_err(TradingSbfError::from)?;
    let data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    authenticate_activation_cache_identity_v1(registry, cache, &release_set, activated)
        .map_err(TradingSbfError::from)?;
    for (role, program, programdata) in [
        (ExecutionRoleV1::Core, CORE_PROGRAM, CORE_PROGRAMDATA),
        (
            ExecutionRoleV1::Trading,
            TRADING_PROGRAM,
            TRADING_PROGRAMDATA,
        ),
    ] {
        authenticate_activated_role_in_frame_v1(
            cache,
            activated,
            role,
            account(accounts, program)?,
            account(accounts, programdata)?,
        )
        .map_err(TradingSbfError::from)?;
    }
    Ok(())
}

fn authenticate_rent_credit(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
) -> Result<(), ProgramError> {
    let credit_account = account(accounts, RENT_CREDIT)?;
    let rent_program = account(accounts, RENT_PROGRAM)?;
    if credit_account.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || credit_account.owner != rent_program.key
        || credit_account.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !funded_rent_persists_v1(credit_account.lamports())
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = credit_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let seeds = credit.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    if credit.to_bytes().as_slice() != data.as_ref()
        || credit.market().to_bytes() != market.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
        || Pubkey::create_program_address(
            &[
                seeds.domain(),
                market_seed.as_slice(),
                generation.as_slice(),
                &bump,
            ],
            rent_program.key,
        )
        .map_err(|_| TradingSbfError::Content)?
            != *credit_account.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn close_to_rent_credit(
    ledger: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    principal: u64,
    rent: u64,
) -> Result<(), ProgramError> {
    let minimum_backing = principal.checked_add(rent).ok_or(TradingSbfError::Commit)?;
    let total = ledger.lamports();
    let credit_post = rent_credit
        .lamports()
        .checked_add(total)
        .ok_or(TradingSbfError::Commit)?;
    if total < minimum_backing {
        return Err(TradingSbfError::Commit.into());
    }
    **rent_credit
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = credit_post;
    **ledger
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = 0;
    ledger.resize(0).map_err(|_| TradingSbfError::Commit)?;
    ledger.assign(&system_program::ID);
    if ledger.lamports() != 0 || ledger.data_len() != 0 || ledger.owner != &system_program::ID {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_exact_and_reserved_bytes_refuse() {
        let request = DirectCloseUnusedRequestV1 { entry_index: 7 }.to_bytes();
        assert_eq!(decode_request(&request), Ok(7));
        for offset in [0, 8, 9, 12, 15] {
            let mut hostile = request;
            hostile[offset] ^= 1;
            assert_eq!(
                decode_request(&hostile),
                Err(TradingSbfError::UnsupportedContent.into())
            );
        }
        assert!(!is_direct_close_unused_v1(&request[..15]));
    }

    #[test]
    fn physical_close_returns_principal_rent_and_donation_to_one_credit() {
        let ledger_key = Pubkey::new_unique();
        let credit_key = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let rent_program = Pubkey::new_unique();
        let mut ledger_lamports = 160;
        let mut credit_lamports = 20;
        let mut ledger_data = [7_u8; 8];
        let mut credit_data = [];
        let ledger = AccountInfo::new(
            &ledger_key,
            false,
            true,
            &mut ledger_lamports,
            &mut ledger_data,
            &trading,
            false,
        );
        let credit = AccountInfo::new(
            &credit_key,
            false,
            true,
            &mut credit_lamports,
            &mut credit_data,
            &rent_program,
            false,
        );
        close_to_rent_credit(&ledger, &credit, 100, 50).expect("donated close");
        assert_eq!(ledger.lamports(), 0);
        assert_eq!(ledger.data_len(), 0);
        assert_eq!(ledger.owner, &system_program::ID);
        assert_eq!(credit.lamports(), 180);
    }

    #[test]
    fn physical_close_refuses_underfunding_without_mutation() {
        let ledger_key = Pubkey::new_unique();
        let credit_key = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let rent_program = Pubkey::new_unique();
        let mut ledger_lamports = 149;
        let mut credit_lamports = 20;
        let mut ledger_data = [7_u8; 8];
        let mut credit_data = [];
        let ledger = AccountInfo::new(
            &ledger_key,
            false,
            true,
            &mut ledger_lamports,
            &mut ledger_data,
            &trading,
            false,
        );
        let credit = AccountInfo::new(
            &credit_key,
            false,
            true,
            &mut credit_lamports,
            &mut credit_data,
            &rent_program,
            false,
        );
        assert_eq!(
            close_to_rent_credit(&ledger, &credit, 100, 50),
            Err(TradingSbfError::Commit.into())
        );
        assert_eq!(ledger.lamports(), 149);
        assert_eq!(credit.lamports(), 20);
        assert_eq!(ledger.owner, &trading);
    }
}
