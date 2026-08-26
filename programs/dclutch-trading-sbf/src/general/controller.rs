//! Physical commit-last executor for one prepared General settlement step.
//!
//! The common Trading boundary authenticates the current Trading release and
//! constructs [`PreparedSettlementStepV2`](super::settlement::PreparedSettlementStepV2)
//! from the same borrowed accounts. This module performs the exact Claims and
//! Custody CPIs, verifies their immediate receipts against caller-observed
//! poststate, and writes the General cursor only after every active child has
//! succeeded. Any later refusal rolls all child effects back at the SVM
//! instruction boundary.

use dclutch_claims_svm::NO_POSITION_REVISION;
use dclutch_custody_contract::{CustodyReplayV1, CUSTODY_POSTSTATE_DOMAIN_V1};
use dclutch_economic_slice_kernel::{market_revision, position_owner, position_revision};
use dclutch_general_adapter_contract::child_packets::{
    verify_claims_receipt_v2, verify_custody_receipt_v2, ExpectedClaimsPostV2,
    ExpectedCustodyPostV2,
};
use dclutch_general_codec::{MAX_OUTCOMES, SETTLEMENT_CURSOR_BYTES};
use dclutch_general_config_contract::{GeneralConfigV2, GENERAL_CONFIG_SCHEMA_ID_V2};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::TokenAccount;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    dispatch::TradingFamilyContextV1,
    general::settlement::{derive_caller_authorities_v2, PreparedSettlementStepV2},
    TradingSbfError,
};

/// Exact canonical account count for one General physical settlement step.
pub const GENERAL_SETTLEMENT_ACCOUNT_COUNT_V2: usize = 28;

pub(super) const CORE_MARKET: usize = 0;
pub(super) const ACTIVATION_CACHE: usize = 1;
pub(super) const REGISTRY_PROGRAM: usize = 2;
pub(super) const TRADING_PROGRAM: usize = 3;
pub(super) const TRADING_PROGRAMDATA: usize = 4;
pub(super) const CORE_PROGRAM: usize = 5;
pub(super) const CORE_PROGRAMDATA: usize = 6;
pub(super) const CLAIMS_PROGRAM: usize = 7;
pub(super) const CLAIMS_PROGRAMDATA: usize = 8;
pub(super) const CUSTODY_PROGRAM: usize = 9;
pub(super) const CLAIMS_CALLER_AUTHORITY: usize = 10;
pub(super) const CUSTODY_CALLER_AUTHORITY: usize = 11;
pub(super) const SETTLEMENT_CURSOR: usize = 12;
pub(super) const VERIFIED_CERTIFICATE: usize = 13;
pub(super) const CANDIDATE: usize = 14;
pub(super) const PAGE_OR_MARKET: usize = 15;
pub(super) const CLAIMS_MARKET: usize = 16;
pub(super) const ROW_OWNER_POSITION: usize = 17;
pub(super) const SETTLEMENT_POSITION: usize = 18;
pub(super) const REALM: usize = 19;
pub(super) const REALM_STAGING: usize = 20;
pub(super) const CUSTODY_REPLAY: usize = 21;
pub(super) const COLLATERAL_MINT: usize = 22;
pub(super) const COLLATERAL_SOURCE: usize = 23;
pub(super) const COLLATERAL_DESTINATION: usize = 24;
pub(super) const CUSTODY_TRANSFER_AUTHORITY: usize = 25;
pub(super) const TOKEN_PROGRAM: usize = 26;
pub(super) const GENERAL_CONFIG: usize = 27;

/// Invoke every active fixed-role child and commit the cursor last.
///
/// `prepared` must have been produced from these same borrowed General-owned
/// bytes after [`TradingFamilyContextV1`] authentication. The exact frame has
/// two independent request-derived caller-authority accounts; inactive roles
/// use their child Program account as an inert sentinel.
pub fn apply_prepared_settlement_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    prepared: &PreparedSettlementStepV2,
) -> Result<(), ProgramError> {
    if accounts.len() != GENERAL_SETTLEMENT_ACCOUNT_COUNT_V2
        || context.program_id() != program_id.to_bytes()
        || account(accounts, CORE_MARKET)?.key.to_bytes() != context.market()
        || account(accounts, TRADING_PROGRAM)?.key != program_id
        || !account(accounts, TRADING_PROGRAM)?.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let authorities = derive_caller_authorities_v2(
        prepared,
        program_id.to_bytes(),
        context.release_set().to_bytes(),
        context.market(),
    )
    .map_err(|_| TradingSbfError::Transition)?;
    authenticate_authority_account(
        account(accounts, CLAIMS_CALLER_AUTHORITY)?,
        authorities.claims,
        account(accounts, CLAIMS_PROGRAM)?,
    )?;
    authenticate_authority_account(
        account(accounts, CUSTODY_CALLER_AUTHORITY)?,
        authorities.custody,
        account(accounts, CUSTODY_PROGRAM)?,
    )?;
    authenticate_common_frame(program_id, accounts)?;
    let config = authenticate_config(context, accounts)?;

    if let Some(packet) = prepared.claims() {
        invoke_claims(program_id, context, accounts, packet)?;
    }
    if let Some(packet) = prepared.custody() {
        invoke_custody(program_id, context, accounts, packet, config)?;
    }

    let cursor = account(accounts, SETTLEMENT_CURSOR)?;
    if cursor.owner != program_id
        || !cursor.is_writable
        || cursor.is_signer
        || cursor.executable
        || cursor.data_len() != SETTLEMENT_CURSOR_BYTES
    {
        return Err(TradingSbfError::Commit.into());
    }
    let mut output = cursor
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    output.copy_from_slice(&prepared.cursor_after());
    Ok(())
}

fn invoke_claims(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    packet: &dclutch_general_adapter_contract::child_packets::ClaimsPacketV2,
) -> Result<(), ProgramError> {
    let plan = packet.plan().map_err(|_| TradingSbfError::Transition)?;
    let count = plan.outcome_count();
    if count == 0
        || usize::try_from(count).map_or(true, |value| value > MAX_OUTCOMES)
        || plan.release_set_id() != context.release_set().to_bytes()
        || plan.market() != context.market()
    {
        return Err(TradingSbfError::Transition.into());
    }
    let claims_program = account(accounts, CLAIMS_PROGRAM)?;
    let source = claims_position(accounts, plan.source_owner(), count)?;
    let destination = claims_position(accounts, plan.destination_owner(), count)?;
    let child_infos = [
        account(accounts, CLAIMS_CALLER_AUTHORITY)?.clone(),
        account(accounts, CLAIMS_MARKET)?.clone(),
        source.clone(),
        destination.clone(),
        account(accounts, ACTIVATION_CACHE)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        claims_program.clone(),
        account(accounts, CLAIMS_PROGRAMDATA)?.clone(),
        account(accounts, REGISTRY_PROGRAM)?.clone(),
        account(accounts, CORE_MARKET)?.clone(),
        account(accounts, CORE_PROGRAM)?.clone(),
        account(accounts, CORE_PROGRAMDATA)?.clone(),
    ];
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: [
            signer_readonly(child_infos[0].key),
            writable(child_infos[1].key),
            position_meta(&child_infos[2], plan.source_owner()),
            position_meta(&child_infos[3], plan.destination_owner()),
            readonly(child_infos[4].key),
            readonly(child_infos[5].key),
            readonly(child_infos[6].key),
            readonly(child_infos[7].key),
            readonly(child_infos[8].key),
            readonly(child_infos[9].key),
            readonly(child_infos[10].key),
            readonly(child_infos[11].key),
            readonly(child_infos[12].key),
        ]
        .to_vec(),
        data: packet.bytes().to_vec(),
    };
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        context.release_set().to_bytes(),
        context.market(),
        ExecutionRoleV1::Trading,
        plan.request_id(),
        packet.digest(),
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if child_infos[0].key != &expected {
        return Err(TradingSbfError::Transition.into());
    }
    let seed_slices = seeds.as_slices();
    let bump_seed = [bump];
    let signer = [
        seed_slices[0],
        seed_slices[1],
        seed_slices[2],
        seed_slices[3],
        seed_slices[4],
        seed_slices[5],
        &bump_seed,
    ];
    invoke_signed(&instruction, &child_infos, &[&signer])
        .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let market_data = child_infos[1]
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let market_revision = market_revision(&market_data).map_err(|_| TradingSbfError::Transition)?;
    let source_present = plan.expected_source_revision() != NO_POSITION_REVISION;
    let destination_present = plan.expected_destination_revision() != NO_POSITION_REVISION;
    let source_revision = claims_revision(&child_infos[2], count, source_present)?;
    let destination_revision = claims_revision(&child_infos[3], count, destination_present)?;
    let resource_digest = claims_resource_digest(
        &market_data,
        &child_infos[2],
        &child_infos[3],
        source_present,
        destination_present,
    )?;
    drop(market_data);
    verify_claims_receipt_v2(
        packet,
        producer.to_bytes(),
        &receipt,
        ExpectedClaimsPostV2 {
            claims_program: claims_program.key.to_bytes(),
            market_revision,
            source_revision,
            destination_revision,
            payout: 0,
            resource_digest,
        },
    )
    .map_err(|_| TradingSbfError::Transition.into())
}

fn invoke_custody(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    packet: &dclutch_general_adapter_contract::child_packets::CustodyPacketV2,
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    let request = packet.request();
    if request.release_set != context.release_set().to_bytes()
        || request.market != context.market()
        || request.caller_program != program_id.to_bytes()
        || account(accounts, COLLATERAL_SOURCE)?.key.to_bytes() != request.source
        || account(accounts, COLLATERAL_DESTINATION)?.key.to_bytes() != request.destination
        || account(accounts, COLLATERAL_MINT)?.key.to_bytes() != request.mint
        || account(accounts, TOKEN_PROGRAM)?.key.to_bytes() != request.token_program
        || account(accounts, REALM)?.key.to_bytes() != request.realm
    {
        return Err(TradingSbfError::Transition.into());
    }
    if request.destination_compartment == dclutch_custody_contract::CompartmentV1::External
        && request.semantic.order == [0; 32]
        && request.semantic.destination_owner != config.quote_surplus_beneficiary()
    {
        return Err(TradingSbfError::Transition.into());
    }
    let source_before = token_amount(account(accounts, COLLATERAL_SOURCE)?)?;
    let destination_before = token_amount(account(accounts, COLLATERAL_DESTINATION)?)?;
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let child_infos = [
        account(accounts, CUSTODY_CALLER_AUTHORITY)?.clone(),
        account(accounts, CORE_MARKET)?.clone(),
        account(accounts, ACTIVATION_CACHE)?.clone(),
        account(accounts, REGISTRY_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, CUSTODY_REPLAY)?.clone(),
        account(accounts, COLLATERAL_SOURCE)?.clone(),
        account(accounts, COLLATERAL_DESTINATION)?.clone(),
        account(accounts, COLLATERAL_MINT)?.clone(),
        account(accounts, CUSTODY_TRANSFER_AUTHORITY)?.clone(),
        account(accounts, TOKEN_PROGRAM)?.clone(),
        custody_program.clone(),
    ];
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: [
            signer_readonly(child_infos[0].key),
            readonly(child_infos[1].key),
            readonly(child_infos[2].key),
            readonly(child_infos[3].key),
            readonly(child_infos[4].key),
            readonly(child_infos[5].key),
            readonly(child_infos[6].key),
            readonly(child_infos[7].key),
            writable(child_infos[8].key),
            writable(child_infos[9].key),
            writable(child_infos[10].key),
            readonly(child_infos[11].key),
            readonly(child_infos[12].key),
            readonly(child_infos[13].key),
        ]
        .to_vec(),
        data: packet.bytes().to_vec(),
    };
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        context.release_set().to_bytes(),
        context.market(),
        ExecutionRoleV1::Trading,
        request.context,
        packet.digest(),
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if child_infos[0].key != &expected {
        return Err(TradingSbfError::Transition.into());
    }
    let seed_slices = seeds.as_slices();
    let bump_seed = [bump];
    let signer = [
        seed_slices[0],
        seed_slices[1],
        seed_slices[2],
        seed_slices[3],
        seed_slices[4],
        seed_slices[5],
        &bump_seed,
    ];
    invoke_signed(&instruction, &child_infos, &[&signer])
        .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let source_after = token_amount(&child_infos[9])?;
    let destination_after = token_amount(&child_infos[10])?;
    let replay = child_infos[8]
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    CustodyReplayV1::decode(&replay).map_err(|_| TradingSbfError::Transition)?;
    let replay_state_digest = hash(&replay).to_bytes();
    let poststate_commitment = hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &packet.digest(),
        &request.source,
        &request.destination,
        &source_before.to_le_bytes(),
        &source_after.to_le_bytes(),
        &destination_before.to_le_bytes(),
        &destination_after.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ])
    .to_bytes();
    drop(replay);
    verify_custody_receipt_v2(
        *packet,
        producer.to_bytes(),
        &receipt,
        ExpectedCustodyPostV2 {
            custody_program: custody_program.key.to_bytes(),
            source_before,
            source_after,
            destination_before,
            destination_after,
            poststate_commitment,
            replay_state_digest,
        },
    )
    .map_err(|_| TradingSbfError::Transition.into())
}

fn authenticate_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let readonly_programs = [
        REGISTRY_PROGRAM,
        TRADING_PROGRAM,
        CORE_PROGRAM,
        CLAIMS_PROGRAM,
        CUSTODY_PROGRAM,
        TOKEN_PROGRAM,
    ];
    for index in readonly_programs {
        let observed = account(accounts, index)?;
        if observed.is_signer || observed.is_writable || !observed.executable {
            return Err(TradingSbfError::Content.into());
        }
    }
    for index in [TRADING_PROGRAMDATA, CORE_PROGRAMDATA, CLAIMS_PROGRAMDATA] {
        let observed = account(accounts, index)?;
        if observed.is_signer || observed.is_writable || observed.executable {
            return Err(TradingSbfError::Content.into());
        }
    }
    if account(accounts, SETTLEMENT_CURSOR)?.owner != program_id
        || account(accounts, VERIFIED_CERTIFICATE)?.owner != program_id
        || account(accounts, CANDIDATE)?.owner != program_id
        || account(accounts, PAGE_OR_MARKET)?.is_signer
        || account(accounts, PAGE_OR_MARKET)?.is_writable
        || account(accounts, PAGE_OR_MARKET)?.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn authenticate_config(
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
) -> Result<GeneralConfigV2, ProgramError> {
    let config = account(accounts, GENERAL_CONFIG)?;
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    if config.owner != registry.key || config.is_signer || config.is_writable || config.executable {
        return Err(TradingSbfError::Content.into());
    }
    let data = config
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let digest = hash(&data).to_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &GENERAL_CONFIG_SCHEMA_ID_V2,
            &digest,
        ],
        registry.key,
    )
    .0;
    let value = GeneralConfigV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if config.key != &expected
        || context.selection().config().to_bytes() != digest
        || value.generation() != context.generation()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(value)
}

fn authenticate_authority_account(
    authority: &AccountInfo<'_>,
    expected: Option<[u8; 32]>,
    sentinel: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    match expected {
        Some(expected)
            if authority.key.to_bytes() == expected
                && !authority.is_signer
                && !authority.is_writable
                && !authority.executable =>
        {
            Ok(())
        }
        None if authority.key == sentinel.key && authority.executable => Ok(()),
        _ => Err(TradingSbfError::Content.into()),
    }
}

fn claims_position<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    owner: [u8; 32],
    outcome_count: u32,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    if owner == [0; 32] {
        return account(accounts, CLAIMS_PROGRAM);
    }
    for index in [ROW_OWNER_POSITION, SETTLEMENT_POSITION] {
        let candidate = account(accounts, index)?;
        if candidate.owner == account(accounts, CLAIMS_PROGRAM)?.key {
            let data = candidate
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?;
            let matches = position_owner(&data, outcome_count)
                .map(|value| value == owner)
                .unwrap_or(false);
            drop(data);
            if matches {
                return Ok(candidate);
            }
        }
    }
    Err(TradingSbfError::Transition.into())
}

fn claims_revision(
    account: &AccountInfo<'_>,
    outcome_count: u32,
    present: bool,
) -> Result<u64, ProgramError> {
    if !present {
        return Ok(NO_POSITION_REVISION);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    position_revision(&data, outcome_count).map_err(|_| TradingSbfError::Transition.into())
}

fn claims_resource_digest(
    market: &[u8],
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    source_present: bool,
    destination_present: bool,
) -> Result<[u8; 32], ProgramError> {
    match (source_present, destination_present) {
        (true, true) => {
            let source = source
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?;
            let destination = destination
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?;
            Ok(hashv(&[market, &source, &destination]).to_bytes())
        }
        (true, false) => {
            let source = source
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?;
            Ok(hashv(&[market, &source]).to_bytes())
        }
        (false, true) => {
            let destination = destination
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?;
            Ok(hashv(&[market, &destination]).to_bytes())
        }
        (false, false) => Err(TradingSbfError::Transition.into()),
    }
}

fn token_amount(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    TokenAccount::parse(&data)
        .map(|value| value.amount)
        .map_err(|_| TradingSbfError::Transition.into())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn readonly(key: &Pubkey) -> AccountMeta {
    AccountMeta::new_readonly(*key, false)
}

fn writable(key: &Pubkey) -> AccountMeta {
    AccountMeta::new(*key, false)
}

fn signer_readonly(key: &Pubkey) -> AccountMeta {
    AccountMeta::new_readonly(*key, true)
}

fn position_meta(account: &AccountInfo<'_>, owner: [u8; 32]) -> AccountMeta {
    if owner == [0; 32] {
        readonly(account.key)
    } else {
        writable(account.key)
    }
}
