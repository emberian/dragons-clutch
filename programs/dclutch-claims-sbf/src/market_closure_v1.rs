//! Core-authorized market-wide aggregate-empty closure.
//!
//! This adapter is the sole physical producer of
//! [`ClaimsMarketClosureReceiptV1`]. It authenticates the selected Core caller,
//! proves every runtime-width aggregate supply is zero, credits all aggregate
//! lamports to the immutable RentCredit, and emits the receipt only after the
//! aggregate is closed.

use dclutch_claims_svm::{
    liability_basis_state_v2::{LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2},
    market_closure_v1::{
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1, ClaimsMarketClosureReceiptInputV1,
        ClaimsMarketClosureReceiptV1, ClaimsMarketClosureRequestV1,
    },
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{ClaimsSbfError, reauthenticate};

/// Core caller PDA signer.
pub const AUTHORITY_ACCOUNT_V1: usize = 0;
/// Writable canonical LiabilityBasisV2 aggregate.
pub const AGGREGATE_ACCOUNT_V1: usize = 1;
/// Writable immutable RentCredit beneficiary.
pub const RENT_CREDIT_ACCOUNT_V1: usize = 2;
/// Registry activation cache.
pub const ACTIVATION_CACHE_ACCOUNT_V1: usize = 3;
/// Immutable Market-selected Registry program.
pub const REGISTRY_PROGRAM_ACCOUNT_V1: usize = 4;
/// Current Claims program.
pub const CLAIMS_PROGRAM_ACCOUNT_V1: usize = 5;
/// Current Claims ProgramData.
pub const CLAIMS_PROGRAMDATA_ACCOUNT_V1: usize = 6;
/// Current Core program.
pub const CORE_PROGRAM_ACCOUNT_V1: usize = 7;
/// Current Core ProgramData.
pub const CORE_PROGRAMDATA_ACCOUNT_V1: usize = 8;
/// Canonical Retiring Core Market.
pub const CORE_MARKET_ACCOUNT_V1: usize = 9;
/// Infrastructure-selected Rent program owning RentCredit.
pub const RENT_PROGRAM_ACCOUNT_V1: usize = 10;
/// Exact Claims market-closure frame width.
pub const MARKET_CLOSURE_ACCOUNT_COUNT_V1: usize = 11;

/// Stable physical closure refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsMarketClosureSbfErrorV1 {
    /// The fixed account frame or privileges refused.
    Accounts = 500,
    /// Caller PDA or current Registry releases refused.
    Authority = 501,
    /// Core/aggregate/RentCredit identities or revisions refused.
    Identity = 502,
    /// A nonzero aggregate supply prevented closure.
    Liability = 503,
    /// Checked refund accounting or commit-last closure refused.
    Commit = 504,
    /// Typed receipt construction refused.
    Receipt = 505,
}

impl From<ClaimsMarketClosureSbfErrorV1> for ProgramError {
    fn from(value: ClaimsMarketClosureSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct ClosureAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> ClosureAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        let [
            authority,
            aggregate,
            rent_credit,
            cache,
            registry,
            claims_program,
            claims_programdata,
            core_program,
            core_programdata,
            core_market,
            rent_program,
        ] = accounts
        else {
            return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
        };
        Ok(Self {
            authority,
            aggregate,
            rent_credit,
            cache,
            registry,
            claims_program,
            claims_programdata,
            core_program,
            core_programdata,
            core_market,
            rent_program,
        })
    }
}

/// Close one exact empty Claims aggregate and return its typed receipt.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = ClaimsMarketClosureRequestV1::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let request_input = request.input();
    let request_digest = hash(instruction_data).to_bytes();
    let accounts = ClosureAccounts::parse(accounts)?;
    authenticate_privileges(program_id, accounts)?;
    authenticate_authority(accounts, request_input, request_digest)?;
    authenticate_releases(accounts, request_input.release_set)?;
    let core = authenticate_core(accounts, request_input)?;
    authenticate_rent_credit(accounts, core)?;
    let (pre_digest, refund_lamports) = authenticate_empty_aggregate(accounts, request_input)?;
    let rent_after = accounts
        .rent_credit
        .lamports()
        .checked_add(refund_lamports)
        .ok_or(ClaimsMarketClosureSbfErrorV1::Commit)?;
    close_aggregate(accounts, rent_after)?;
    let post_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        accounts.aggregate.key.as_ref(),
        accounts.rent_credit.key.as_ref(),
        request_input.resulting_revision.to_le_bytes().as_slice(),
        refund_lamports.to_le_bytes().as_slice(),
        rent_after.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    let receipt = ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: program_id.to_bytes(),
        release_set: request_input.release_set,
        market: request_input.market,
        aggregate: request_input.aggregate,
        rent_credit: request_input.rent_credit,
        request_digest,
        pre_resource_digest: pre_digest,
        post_resource_digest: post_digest,
        generation: request_input.generation,
        pre_revision: request_input.expected_revision,
        post_revision: request_input.resulting_revision,
        liability_units: 0,
        refund_lamports,
        claim_count: request_input.claim_count,
    })
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: ClosureAccounts<'_, '_>,
) -> ProgramResult {
    if accounts.authority.is_writable
        || !accounts.authority.is_signer
        || accounts.authority.executable
        || !accounts.aggregate.is_writable
        || accounts.aggregate.is_signer
        || accounts.aggregate.executable
        || !accounts.rent_credit.is_writable
        || accounts.rent_credit.is_signer
        || accounts.rent_credit.executable
        || accounts.cache.is_writable
        || accounts.cache.is_signer
        || accounts.cache.executable
        || accounts.registry.is_writable
        || accounts.registry.is_signer
        || !accounts.registry.executable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.is_writable
        || accounts.claims_program.is_signer
        || !accounts.claims_program.executable
        || accounts.claims_programdata.is_writable
        || accounts.claims_programdata.is_signer
        || accounts.claims_programdata.executable
        || accounts.core_program.is_writable
        || accounts.core_program.is_signer
        || !accounts.core_program.executable
        || accounts.core_programdata.is_writable
        || accounts.core_programdata.is_signer
        || accounts.core_programdata.executable
        || accounts.core_market.is_writable
        || accounts.core_market.is_signer
        || accounts.core_market.executable
        || accounts.rent_program.is_writable
        || accounts.rent_program.is_signer
        || !accounts.rent_program.executable
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
    }
    require_distinct(&[
        accounts.authority,
        accounts.aggregate,
        accounts.rent_credit,
        accounts.cache,
        accounts.registry,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.core_programdata,
        accounts.core_market,
        accounts.rent_program,
    ])
}

#[inline(never)]
fn authenticate_authority(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims_svm::market_closure_v1::ClaimsMarketClosureRequestInputV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Core,
        request.parent_request_digest,
        request_digest,
    )
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.core_program.key).0;
    if expected != *accounts.authority.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: ClosureAccounts<'_, '_>,
    release_set: [u8; 32],
) -> ProgramResult {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        ),
    ] {
        let receipt = reauthenticate(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
        )?;
        if receipt.execution_release_set_id().as_bytes() != &release_set {
            return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_core(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims_svm::market_closure_v1::ClaimsMarketClosureRequestInputV1,
) -> Result<CoreState, ProgramError> {
    if request.core_program != accounts.core_program.key.to_bytes()
        || accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.key.to_bytes() != request.market
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let core_bytes = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let core =
        CoreState::decode(&core_bytes).map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        accounts.core_program.key,
    )
    .0;
    if expected != *accounts.core_market.key
        || core.phase != Phase::Retiring
        || core.identity.market_id.to_bytes() != request.market
        || core.identity.selected_release_set.to_bytes() != request.release_set
        || core.identity.registry_program.to_bytes() != accounts.registry.key.to_bytes()
        || core.identity.generation != request.generation
        || core.outstanding_capabilities != 0
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok(core)
}

#[inline(never)]
fn authenticate_rent_credit(accounts: ClosureAccounts<'_, '_>, core: CoreState) -> ProgramResult {
    if accounts.rent_credit.owner != accounts.rent_program.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let data = accounts
        .rent_credit
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let credit = LifecycleRentCreditV2::decode(&data)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let authority = credit.refund_wallet().to_bytes();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        accounts.rent_program.key,
    )
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    if expected != *accounts.rent_credit.key
        || accounts.rent_credit.key.to_bytes() == core.rent_beneficiary.to_bytes()
        || authority != core.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != core.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != core.identity.selected_release_set.to_bytes()
        || credit.generation() != core.identity.generation
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_empty_aggregate(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims_svm::market_closure_v1::ClaimsMarketClosureRequestInputV1,
) -> Result<([u8; 32], u64), ProgramError> {
    if accounts.aggregate.owner != accounts.claims_program.key
        || accounts.aggregate.key.to_bytes() != request.aggregate
        || accounts.rent_credit.key.to_bytes() != request.rent_credit
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, request.market.as_slice()],
        accounts.claims_program.key,
    )
    .0;
    if expected != *accounts.aggregate.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let bytes = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let market = LiabilityBasisMarketViewV2::decode(&bytes)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    if market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.generation != request.generation
        || market.claim_count != request.claim_count
        || market.revision != request.expected_revision
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let mut claim_index = 0;
    while claim_index < market.claim_count {
        if market
            .supply(&bytes, claim_index)
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?
            != 0
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Liability.into());
        }
        claim_index = claim_index
            .checked_add(1)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Liability)?;
    }
    let pre_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        accounts.aggregate.key.as_ref(),
        bytes.as_ref(),
    ])
    .to_bytes();
    let refund_lamports = accounts.aggregate.lamports();
    if refund_lamports == 0 {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok((pre_digest, refund_lamports))
}

#[inline(never)]
fn close_aggregate(accounts: ClosureAccounts<'_, '_>, rent_after: u64) -> ProgramResult {
    {
        let mut data = accounts
            .aggregate
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        data.fill(0);
    }
    {
        let mut aggregate_lamports = accounts
            .aggregate
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut credit_lamports = accounts
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        **aggregate_lamports = 0;
        **credit_lamports = rent_after;
    }
    accounts
        .aggregate
        .resize(0)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
    accounts.aggregate.assign(&system_program::ID);
    if accounts.aggregate.owner != &system_program::ID
        || !accounts.aggregate.data_is_empty()
        || accounts.aggregate.lamports() != 0
        || accounts.rent_credit.lamports() != rent_after
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Commit.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[&AccountInfo<'_>]) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?
            .iter()
            .any(|other| other.key == account.key)
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
        }
    }
    Ok(())
}
