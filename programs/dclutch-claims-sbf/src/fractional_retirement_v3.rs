//! One-coordinate ordered Fractional Position and Mint retirement.
//!
//! Claims closes the zero native Position/admission pair, then invokes
//! Token-2022 to close the matching zero-supply Mint, and commits the cursor
//! last. The Trading-owned root is authenticated and borrowed only through a
//! readonly Claims view; its propagated signature is used solely as the
//! Token-2022 MintCloseAuthority. Any late CPI or cursor failure rolls every
//! earlier mutation back at the SVM instruction boundary.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use dclutch_claims_svm::{
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    },
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3,
    FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3, FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3,
    FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3,
    FractionalRetireCoordinateObservationV3, FractionalRetirementActionV3,
    FractionalRetirementCoordinateReceiptV3, FractionalRetirementCursorV3,
    FractionalRetirementRequestV3, decode_fractional_capability_root_v4,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsV2,
};
use dclutch_sha256_adapter::digest;
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2, TokenBehaviorSelectionV2,
};
use solana_program::{
    account_info::AccountInfo,
    program::{invoke, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use spl_token_2022_interface::instruction as token_instruction;

use crate::{
    ClaimsSbfError,
    protocol_position_v2::{
        AuthenticatedProtocolPositionCloseParentV2, PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2,
        execute_parent_authenticated_close,
    },
    rational_representation_v2::authenticate_finalized_rational_record,
};

const AUTHORITY: usize = 0;
const MARKET: usize = 1;
const POSITION: usize = 2;
const ADMISSION: usize = 3;
const RENT: usize = 4;
const REGISTRY: usize = 7;
const TRADING_PROGRAM: usize = 8;
const CLAIMS_PROGRAM: usize = 10;
const ROOT: usize = 12;
const RENT_CREDIT: usize = 13;
const RENT_PROGRAM: usize = 14;
const CURSOR: usize = 15;
const TERMS_RAW: usize = 16;
const TERMS_STAGING: usize = 17;
const TOKEN_BEHAVIOR_RAW: usize = 18;
const TOKEN_BEHAVIOR_STAGING: usize = 19;
const SHARD_MINT: usize = 20;
const TOKEN_PROGRAM: usize = 21;

/// Execute one exact next-coordinate retirement.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if instruction_data.len() != FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3 {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let request = FractionalRetirementRequestV3::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    if request.action() != FractionalRetirementActionV3::RetireCoordinate {
        return Err(ClaimsSbfError::Instruction.into());
    }
    authenticate_frame(program_id, accounts)?;
    let prepared = prepare_retirement(program_id, accounts, &request, instruction_data)?;
    execute_retirement(program_id, accounts, &request, &prepared)
}

struct StaticRetirementFactsV3 {
    expected_mint: [u8; 32],
    market_generation: u64,
    market_revision: u64,
}

struct PreparedRetirementV3 {
    request_digest: [u8; 32],
    close_request: Vec<u8>,
    pre_cursor: Vec<u8>,
    post_cursor: Vec<u8>,
    pre_cursor_digest: [u8; 32],
    post_cursor_digest: [u8; 32],
    expected_mint: [u8; 32],
    post_revision: u64,
}

#[inline(never)]
fn prepare_retirement(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
) -> Result<Box<PreparedRetirementV3>, ProgramError> {
    let facts = authenticate_terms_market_and_mint(accounts, request)?;
    authenticate_behavior(accounts, request)?;
    authenticate_root(accounts, request)?;
    let (pre_cursor, post_cursor, pre_cursor_digest, post_cursor_digest, post_revision) =
        prepare_cursor(program_id, accounts, request, facts.expected_mint)?;
    let close_request = prepare_close_request(accounts, request, &facts)?;
    Ok(Box::new(PreparedRetirementV3 {
        request_digest: digest(instruction_data),
        close_request,
        pre_cursor,
        post_cursor,
        pre_cursor_digest,
        post_cursor_digest,
        expected_mint: facts.expected_mint,
        post_revision,
    }))
}

#[inline(never)]
fn authenticate_terms_market_and_mint(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
) -> Result<StaticRetirementFactsV3, ProgramError> {
    let input = request.input();
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| ClaimsSbfError::Accounts)?;
    let registry = account(accounts, REGISTRY)?;
    let terms_raw = account(accounts, TERMS_RAW)?;
    let terms_staging = account(accounts, TERMS_STAGING)?;
    let terms_data = terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        terms_raw,
        terms_staging,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        input.terms,
        &terms_data,
    )?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: input.terms,
            finalized_terms_id: input.terms,
            recomputed_terms_digest: input.terms,
            finalized_terms_digest: input.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    request
        .bind_terms(terms)
        .map_err(|_| ClaimsSbfError::Representation)?;

    let market_data = account(accounts, MARKET)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market =
        LiabilityBasisMarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    if market.logical_market != input.market
        || market.release_set != input.release_set
        || market.registry_program != registry.key.to_bytes()
    {
        return Err(ClaimsSbfError::Identity.into());
    }

    let mint_account = account(accounts, SHARD_MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let expected_mint = terms
        .shard_mint(input.representation_coordinate)
        .map_err(|_| ClaimsSbfError::Token)?;
    if mint_account.key.to_bytes() != expected_mint
        || mint_account.owner != token_program.key
        || token_program.key.to_bytes() != input.token_program
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        input.token_program,
        expected_mint,
        &mint_data,
        input.root,
        0,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if mint_facts.base_supply() != 0 {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(StaticRetirementFactsV3 {
        expected_mint,
        market_generation: market.generation,
        market_revision: market.revision,
    })
}

#[inline(never)]
fn authenticate_behavior(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
) -> Result<(), ProgramError> {
    let input = request.input();
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| ClaimsSbfError::Accounts)?;
    let registry = account(accounts, REGISTRY)?;
    let behavior_raw = account(accounts, TOKEN_BEHAVIOR_RAW)?;
    let behavior_staging = account(accounts, TOKEN_BEHAVIOR_STAGING)?;
    let behavior_data = behavior_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        behavior_raw,
        behavior_staging,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        input.token_behavior,
        &behavior_data,
    )?;
    let market_data = account(accounts, MARKET)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market =
        LiabilityBasisMarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    let behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        &behavior_data,
        market.realm_id,
        input.release_set,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if behavior.token_program() != input.token_program {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_root(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
) -> Result<(), ProgramError> {
    let input = request.input();
    let trading_program = account(accounts, TRADING_PROGRAM)?;
    let root_account = account(accounts, ROOT)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let composite_root =
        decode_fractional_capability_root_v4(&root_data).ok_or(ClaimsSbfError::Representation)?;
    let header = composite_root.header();
    let root = composite_root.state();
    let root_input = root.input();
    let (expected_root, expected_root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key);
    if root_account.key != &expected_root
        || root_account.key.to_bytes() != input.root
        || root_account.owner != trading_program.key
        || header.release_set().to_bytes() != input.release_set
        || header.market() != input.market
        || header.selection().config().to_bytes() != input.terms
        || root_input.bump != expected_root_bump
        || root_input.terms != input.terms
        || root_input.market != input.market
        || root_input.rent_beneficiary != input.rent_credit
        || root_input.revision != input.expected_revision
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
#[inline(never)]
fn prepare_cursor(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    expected_mint: [u8; 32],
) -> Result<(Vec<u8>, Vec<u8>, [u8; 32], [u8; 32], u64), ProgramError> {
    let input = request.input();
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| ClaimsSbfError::Accounts)?;
    let terms_data = account(accounts, TERMS_RAW)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: input.terms,
            finalized_terms_id: input.terms,
            recomputed_terms_digest: input.terms,
            finalized_terms_digest: input.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    let cursor_account = account(accounts, CURSOR)?;
    let cursor_data = cursor_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let pre_cursor = cursor_data.to_vec();
    let pre_cursor_digest = digest(&pre_cursor);
    let cursor = FractionalRetirementCursorV3::decode(&cursor_data)
        .map_err(|_| ClaimsSbfError::Representation)?;
    let bump = [cursor.bump()];
    let expected_cursor = Pubkey::create_program_address(
        &[
            FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3,
            account(accounts, ROOT)?.key.as_ref(),
            &bump,
        ],
        program_id,
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    if cursor_account.key != &expected_cursor
        || cursor_account.owner != program_id
        || cursor_account.lamports() != cursor.historical_rent_principal()
        || !rent.is_exempt(
            cursor_account.lamports(),
            FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3,
        )
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    drop(cursor_data);

    let position_data = account(accounts, POSITION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let reserve_claims = position
        .balance(&position_data, input.representation_coordinate)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(position_data);
    let cursor_candidate = cursor
        .advance(
            terms,
            *request,
            FractionalRetireCoordinateObservationV3 {
                shard_mint: expected_mint,
                shard_supply: 0,
                reserve_claims,
                mint_authenticated: true,
                reserve_authenticated: position.owner == input.root
                    && position.market_account == account(accounts, MARKET)?.key.to_bytes(),
            },
        )
        .map_err(|_| ClaimsSbfError::Representation)?;
    let post_cursor = cursor_candidate
        .to_bytes()
        .map_err(|_| ClaimsSbfError::Representation)?
        .to_vec();
    let post_cursor_digest = digest(&post_cursor);
    Ok((
        pre_cursor,
        post_cursor,
        pre_cursor_digest,
        post_cursor_digest,
        cursor_candidate.revision(),
    ))
}

#[inline(never)]
fn prepare_close_request(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    facts: &StaticRetirementFactsV3,
) -> Result<Vec<u8>, ProgramError> {
    let input = request.input();
    let position_data = account(accounts, POSITION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(position_data);
    let admission_data = account(accounts, ADMISSION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let admission = ProtocolPositionAdmissionV2::decode(&admission_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(admission_data);
    let close_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Close,
        owner_kind: admission.owner_kind(),
        presence: ProtocolPositionPresenceV2::Existing,
        release_set: input.release_set,
        market: input.market,
        position_owner: input.root,
        parent_request_digest: admission.parent_request_digest(),
        rent_credit: input.rent_credit,
        rent_program: admission.rent_program(),
        generation: facts.market_generation,
        expected_market_revision: facts.market_revision,
        expected_position_revision: position.revision,
        observed_position_lamports: account(accounts, POSITION)?.lamports(),
        observed_admission_lamports: account(accounts, ADMISSION)?.lamports(),
        position_rent_principal: admission.position_rent_principal(),
        admission_rent_principal: admission.admission_rent_principal(),
        capability_descriptor: admission.capability_descriptor(),
        capability_outcome: admission.capability_outcome(),
    }
    .new()
    .map_err(|_| ClaimsSbfError::Identity)?;
    if admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
        || admission.position_owner() != input.root
        || admission.release_set() != input.release_set
        || admission.market() != input.market
        || admission.rent_credit() != input.rent_credit
        || admission.generation() != facts.market_generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(close_request
        .to_bytes()
        .map_err(|_| ClaimsSbfError::Identity)?
        .to_vec())
}

#[inline(never)]
fn execute_retirement(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
) -> Result<(), ProgramError> {
    let close_receipt_digest = execute_position_close(program_id, accounts, request, prepared)?;
    execute_mint_close(accounts)?;
    commit_cursor_and_emit(accounts, request, prepared, close_receipt_digest)
}

#[inline(never)]
fn execute_position_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
) -> Result<[u8; 32], ProgramError> {
    let input = request.input();
    let close_accounts = accounts
        .get(..PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2)
        .ok_or(ClaimsSbfError::Accounts)?;
    let close_receipt = execute_parent_authenticated_close(
        program_id,
        close_accounts,
        &prepared.close_request,
        AuthenticatedProtocolPositionCloseParentV2 {
            release_set: input.release_set,
            market: input.market,
            parent_context: input.terms,
            parent_request_digest: prepared.request_digest,
            trading_root: input.root,
        },
    )?;
    Ok(digest(
        &close_receipt
            .to_bytes()
            .map_err(|_| ClaimsSbfError::Receipt)?,
    ))
}

#[inline(never)]
fn execute_mint_close(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let mint_account = account(accounts, SHARD_MINT)?;
    let root_account = account(accounts, ROOT)?;
    let close_mint = token_instruction::close_account(
        token_program.key,
        mint_account.key,
        account(accounts, RENT_CREDIT)?.key,
        root_account.key,
        &[],
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    invoke(
        &close_mint,
        &[
            mint_account.clone(),
            account(accounts, RENT_CREDIT)?.clone(),
            root_account.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if mint_account.owner != &system_program::ID
        || !mint_account.data_is_empty()
        || mint_account.lamports() != 0
    {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

#[inline(never)]
fn commit_cursor_and_emit(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
    close_receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let cursor_account = account(accounts, CURSOR)?;
    {
        let mut observed = cursor_account
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if observed.as_ref() != prepared.pre_cursor.as_slice()
            || observed.len() != prepared.post_cursor.len()
        {
            return Err(ClaimsSbfError::Receipt.into());
        }
        observed.copy_from_slice(&prepared.post_cursor);
    }
    let receipt = FractionalRetirementCoordinateReceiptV3::new(
        *request,
        prepared.request_digest,
        close_receipt_digest,
        prepared.pre_cursor_digest,
        prepared.post_cursor_digest,
        prepared.expected_mint,
        prepared.post_revision,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let receipt_bytes = receipt.to_bytes();
    if receipt_bytes.len() != FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if accounts.len() != FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3
        || account(accounts, CLAIMS_PROGRAM)?.key != program_id
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for (index, observed) in accounts
        .iter()
        .enumerate()
        .skip(PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2)
    {
        let (signer, writable, executable) = match index {
            CURSOR | SHARD_MINT => (false, true, false),
            TOKEN_PROGRAM => (false, false, true),
            TERMS_RAW | TERMS_STAGING | TOKEN_BEHAVIOR_RAW | TOKEN_BEHAVIOR_STAGING => {
                (false, false, false)
            }
            _ => return Err(ClaimsSbfError::Accounts.into()),
        };
        if observed.is_signer != signer
            || observed.is_writable != writable
            || observed.executable != executable
        {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    let distinct = [
        AUTHORITY,
        MARKET,
        POSITION,
        ADMISSION,
        REGISTRY,
        TRADING_PROGRAM,
        CLAIMS_PROGRAM,
        ROOT,
        RENT_CREDIT,
        RENT_PROGRAM,
        CURSOR,
        TERMS_RAW,
        TERMS_STAGING,
        TOKEN_BEHAVIOR_RAW,
        TOKEN_BEHAVIOR_STAGING,
        SHARD_MINT,
        TOKEN_PROGRAM,
    ];
    for (offset, left) in distinct.iter().copied().enumerate() {
        if distinct.get(offset.saturating_add(1)..).is_none_or(|tail| {
            tail.iter().any(|right| {
                accounts
                    .get(*right)
                    .zip(accounts.get(left))
                    .is_some_and(|(right, left)| right.key == left.key)
            })
        }) {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_frame_is_exact_and_below_both_lock_boundaries() {
        assert_eq!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3, 22);
        assert!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3 <= 64);
        assert!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3 < 65);
        assert_eq!(FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3, 288);
        assert!(FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3 <= 1_232);
    }
}
