//! Authenticated General settlement routing behind the common Trading hot outer.
//!
//! The common outer supplies the immutable [`TradingFamilyContextV1`]. This
//! module independently hostile-decodes the 28-account General suffix, derives
//! every child resource coordinate from those accounts, prepares one atomic
//! transition, and delegates CPI plus commit-last handling to `controller`.

extern crate alloc;

use dclutch_custody_contract::{CallerRoleV1, CustodyReplayV1, CUSTODY_REPLAY_PDA_DOMAIN_V1};
use dclutch_economic_slice_kernel::{market_revision, position_owner, position_revision};
use dclutch_general_adapter_contract::{
    child_packets::{ClaimsResourcesV2, CustodyResourcesV2},
    CompleteSetMoveV1, VerifiedCandidateV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1,
    GENERAL_CERTIFICATE_PDA_DOMAIN_V1, GENERAL_PAGE_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
};
use dclutch_general_codec::{
    Action, CandidateV1, ControllerRequestV1, PageViewV1, SettlementCursorV1, CANDIDATE_BYTES,
    PAGE_BYTES, SETTLEMENT_CURSOR_BYTES,
};
use dclutch_general_config_contract::GeneralConfigV2;
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    dispatch::TradingFamilyContextV1,
    general::{
        controller::{
            apply_prepared_settlement_v2, CANDIDATE, CLAIMS_MARKET, COLLATERAL_DESTINATION,
            COLLATERAL_MINT, COLLATERAL_SOURCE, CORE_MARKET, CUSTODY_PROGRAM, CUSTODY_REPLAY,
            GENERAL_SETTLEMENT_ACCOUNT_COUNT_V2, PAGE_OR_MARKET, ROW_OWNER_POSITION,
            SETTLEMENT_CURSOR, SETTLEMENT_POSITION, TOKEN_PROGRAM, VERIFIED_CERTIFICATE,
        },
        settlement::{
            prepare_close_v2, prepare_collect_v2, prepare_distribute_v2, prepare_materialize_v2,
        },
    },
    TradingSbfError,
};

/// Execute one permissionless physical General continuation.
///
/// The selected raw config is reauthenticated inside the commit controller.
/// `config` is the same decoded value supplied by the common content boundary;
/// this route uses it only to derive immutable lifecycle and surplus facts.
#[inline(never)]
pub fn process_settlement_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if accounts.len() != GENERAL_SETTLEMENT_ACCOUNT_COUNT_V2
        || context.program_id() != program_id.to_bytes()
        || config.generation() != context.generation()
        || !matches!(
            request.action,
            Action::Collect | Action::Materialize | Action::Distribute | Action::Close
        )
    {
        return Err(TradingSbfError::Content.into());
    }
    let candidate =
        alloc::boxed::Box::new(decode_candidate(program_id, context, accounts, request)?);
    let verified = alloc::boxed::Box::new(decode_certificate(
        program_id,
        context,
        accounts,
        candidate.as_ref(),
    )?);
    let cursor = alloc::boxed::Box::new(decode_cursor(
        program_id,
        context,
        accounts,
        verified.as_ref(),
        request,
    )?);
    let claims = claims_resources(accounts, verified.as_ref(), cursor.as_ref(), request)?;
    let custody = custody_resources(
        program_id,
        context,
        accounts,
        verified.as_ref(),
        cursor.as_ref(),
        request,
        config,
    )?;
    let cursor_data = account(accounts, SETTLEMENT_CURSOR)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let page_data = if matches!(request.action, Action::Collect | Action::Distribute) {
        Some(
            account(accounts, PAGE_OR_MARKET)?
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?,
        )
    } else {
        None
    };
    let prepared = match request.action {
        Action::Collect => prepare_collect_v2(
            &cursor_data,
            execution_context(context),
            verified.as_ref(),
            page_data.as_deref().ok_or(TradingSbfError::Transition)?,
            request.expected_revision,
            &claims,
            custody.as_ref(),
        ),
        Action::Materialize => prepare_materialize_v2(
            &cursor_data,
            execution_context(context),
            verified.as_ref(),
            request.expected_revision,
            &claims,
            custody.as_ref(),
        ),
        Action::Distribute => prepare_distribute_v2(
            &cursor_data,
            execution_context(context),
            verified.as_ref(),
            page_data.as_deref().ok_or(TradingSbfError::Transition)?,
            request.expected_revision,
            &claims,
            custody.as_ref(),
        ),
        Action::Close => prepare_close_v2(
            &cursor_data,
            execution_context(context),
            verified.as_ref(),
            request.expected_revision,
            &claims,
            custody.as_ref(),
            (cursor.quote_inventory != 0).then_some(
                dclutch_general_adapter_contract::QuoteSurplusRouteV2 {
                    destination_account: account(accounts, COLLATERAL_DESTINATION)?.key.to_bytes(),
                    beneficiary: config.quote_surplus_beneficiary(),
                },
            ),
        ),
        _ => return Err(TradingSbfError::UnsupportedContent.into()),
    }
    .map_err(|_| TradingSbfError::Transition)?;
    drop(page_data);
    drop(cursor_data);
    apply_prepared_settlement_v2(program_id, context, accounts, prepared.as_ref())
}

fn decode_candidate(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
) -> Result<CandidateV1, ProgramError> {
    let observed = account(accounts, CANDIDATE)?;
    require_owned(observed, program_id, CANDIDATE_BYTES, false)?;
    let data = observed
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let candidate = CandidateV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let expected = family_pda(
        program_id,
        context.market(),
        &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    if observed.key != &expected || request.candidate_id != Some(candidate.candidate_id) {
        return Err(TradingSbfError::Content.into());
    }
    Ok(candidate)
}

fn decode_certificate(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    candidate: &CandidateV1,
) -> Result<VerifiedCandidateV1, ProgramError> {
    let observed = account(accounts, VERIFIED_CERTIFICATE)?;
    require_owned(
        observed,
        program_id,
        dclutch_general_adapter_contract::VERIFIED_CANDIDATE_BYTES_V1,
        false,
    )?;
    let data = observed
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let verified = VerifiedCandidateV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let expected = family_pda(
        program_id,
        context.market(),
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    if observed.key != &expected
        || verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(verified)
}

fn decode_cursor(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    verified: &VerifiedCandidateV1,
    request: ControllerRequestV1,
) -> Result<SettlementCursorV1, ProgramError> {
    let observed = account(accounts, SETTLEMENT_CURSOR)?;
    require_owned(observed, program_id, SETTLEMENT_CURSOR_BYTES, true)?;
    let data = observed
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let cursor = SettlementCursorV1::decode(&data).map_err(|_| TradingSbfError::Transition)?;
    let expected = family_pda(
        program_id,
        context.market(),
        &[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, &verified.candidate_id],
    )?;
    let row = matches!(request.action, Action::Collect | Action::Distribute);
    if observed.key != &expected
        || cursor.candidate_id != verified.candidate_id
        || cursor.outcome_count != verified.outcome_count
        || cursor.page_count != verified.page_count
        || cursor.revision != request.expected_revision
        || (row
            && (cursor.next_page != request.page_index
                || cursor.next_execution != request.execution_index))
        || (!row && (request.page_index != 0 || request.execution_index != 0))
    {
        return Err(TradingSbfError::Transition.into());
    }
    if row {
        let page = account(accounts, PAGE_OR_MARKET)?;
        require_owned(page, program_id, PAGE_BYTES, false)?;
        let data = page
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        let decoded = PageViewV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
        let page_index = cursor.next_page.to_le_bytes();
        let expected_page = family_pda(
            program_id,
            context.market(),
            &[
                GENERAL_PAGE_PDA_DOMAIN_V1,
                &verified.candidate_id,
                &page_index,
            ],
        )?;
        if page.key != &expected_page
            || decoded.candidate_id() != verified.candidate_id
            || decoded.outcome_count() != verified.outcome_count
            || decoded.page_count() != verified.page_count
            || decoded.page_index() != cursor.next_page
            || decoded
                .execution(usize::from(cursor.next_execution))
                .is_err()
        {
            return Err(TradingSbfError::Content.into());
        }
    } else if account(accounts, PAGE_OR_MARKET)?.key != account(accounts, CORE_MARKET)?.key {
        return Err(TradingSbfError::Content.into());
    }
    Ok(cursor)
}

fn claims_resources(
    accounts: &[AccountInfo<'_>],
    verified: &VerifiedCandidateV1,
    cursor: &SettlementCursorV1,
    request: ControllerRequestV1,
) -> Result<ClaimsResourcesV2, ProgramError> {
    let row = row_execution(accounts, cursor, request)?;
    let count = usize::from(verified.outcome_count);
    let required = match request.action {
        Action::Collect => row.is_some_and(|value| {
            value.lots != 0
                && value
                    .deliver_per_lot
                    .get(..count)
                    .is_some_and(|active| active.iter().any(|quantity| *quantity != 0))
        }),
        Action::Distribute => row.is_some_and(|value| {
            value.lots != 0
                && value
                    .receive_per_lot
                    .get(..count)
                    .is_some_and(|active| active.iter().any(|quantity| *quantity != 0))
        }),
        Action::Materialize => verified.complete_set_move != CompleteSetMoveV1::None,
        Action::Close => false,
        _ => return Err(TradingSbfError::UnsupportedContent.into()),
    };
    if !required {
        return Ok(ClaimsResourcesV2 {
            settlement_owner: verified.candidate_id,
            market_revision: 0,
            owner_position_revision: 0,
            settlement_position_revision: 0,
        });
    }
    let count = u32::from(verified.outcome_count);
    let market = account(accounts, CLAIMS_MARKET)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let market_revision = market_revision(&market).map_err(|_| TradingSbfError::Content)?;
    let settlement = account(accounts, SETTLEMENT_POSITION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let settlement_owner =
        position_owner(&settlement, count).map_err(|_| TradingSbfError::Content)?;
    let settlement_position_revision =
        position_revision(&settlement, count).map_err(|_| TradingSbfError::Content)?;
    if settlement_owner != verified.candidate_id {
        return Err(TradingSbfError::Content.into());
    }
    let owner_position_revision = if let Some(row) = row {
        let owner = account(accounts, ROW_OWNER_POSITION)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        if position_owner(&owner, count).map_err(|_| TradingSbfError::Content)? != row.owner_id {
            return Err(TradingSbfError::Content.into());
        }
        position_revision(&owner, count).map_err(|_| TradingSbfError::Content)?
    } else {
        0
    };
    Ok(ClaimsResourcesV2 {
        settlement_owner,
        market_revision,
        owner_position_revision,
        settlement_position_revision,
    })
}

fn custody_resources(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    verified: &VerifiedCandidateV1,
    cursor: &SettlementCursorV1,
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> Result<Option<CustodyResourcesV2>, ProgramError> {
    let row = row_execution(accounts, cursor, request)?;
    let required = match request.action {
        Action::Collect => row.is_some_and(|value| value.quote_debit != 0),
        Action::Distribute => row.is_some_and(|value| value.quote_credit != 0),
        Action::Materialize => verified.complete_set_move != CompleteSetMoveV1::None,
        Action::Close => cursor.quote_inventory != 0,
        _ => return Err(TradingSbfError::UnsupportedContent.into()),
    };
    if !required {
        return Ok(None);
    }
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let replay_account = account(accounts, CUSTODY_REPLAY)?;
    if replay_account.owner != custody_program.key
        || replay_account.executable
        || !replay_account.is_writable
        || replay_account.is_signer
    {
        return Err(TradingSbfError::Content.into());
    }
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| TradingSbfError::Content)?;
    let market = context.market();
    let release_set = context.release_set().to_bytes();
    let replay_seeds = [
        CUSTODY_REPLAY_PDA_DOMAIN_V1,
        market.as_slice(),
        release_set.as_slice(),
        verified.candidate_id.as_slice(),
    ];
    if replay_account.key != &Pubkey::find_program_address(&replay_seeds, custody_program.key).0
        || replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != context.release_set().to_bytes()
        || replay.market != context.market()
        || replay.context != verified.candidate_id
        || replay.caller_program != program_id.to_bytes()
        || replay.generation != context.generation()
    {
        return Err(TradingSbfError::Content.into());
    }
    let row_owner = row.map_or([0; 32], |value| value.owner_id);
    let (source_owner, destination_owner, source_vault_context, destination_vault_context) =
        match request.action {
            Action::Collect => (row_owner, [0; 32], [0; 32], verified.candidate_id),
            Action::Distribute => ([0; 32], row_owner, verified.candidate_id, [0; 32]),
            Action::Materialize if verified.complete_set_move == CompleteSetMoveV1::Mint => {
                ([0; 32], [0; 32], verified.candidate_id, context.market())
            }
            Action::Materialize if verified.complete_set_move == CompleteSetMoveV1::Merge => {
                ([0; 32], [0; 32], context.market(), verified.candidate_id)
            }
            Action::Close => (
                [0; 32],
                config.quote_surplus_beneficiary(),
                verified.candidate_id,
                [0; 32],
            ),
            _ => return Ok(None),
        };
    Ok(Some(CustodyResourcesV2 {
        realm: replay.realm,
        trading_program: program_id.to_bytes(),
        generation: replay.generation,
        source: account(accounts, COLLATERAL_SOURCE)?.key.to_bytes(),
        destination: account(accounts, COLLATERAL_DESTINATION)?.key.to_bytes(),
        source_owner,
        destination_owner,
        source_vault_context,
        destination_vault_context,
        mint: account(accounts, COLLATERAL_MINT)?.key.to_bytes(),
        token_program: account(accounts, TOKEN_PROGRAM)?.key.to_bytes(),
        replay_revision: replay.next_revision,
        transfer_index: 0,
    }))
}

fn row_execution(
    accounts: &[AccountInfo<'_>],
    cursor: &SettlementCursorV1,
    request: ControllerRequestV1,
) -> Result<Option<dclutch_general_codec::ExecutionV1>, ProgramError> {
    if !matches!(request.action, Action::Collect | Action::Distribute) {
        return Ok(None);
    }
    let page = account(accounts, PAGE_OR_MARKET)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    PageViewV1::decode(&page)
        .and_then(|value| value.execution(usize::from(cursor.next_execution)))
        .map(Some)
        .map_err(|_| TradingSbfError::Content.into())
}

const fn execution_context(
    context: TradingFamilyContextV1,
) -> dclutch_general_adapter_contract::ExecutionContextV1 {
    dclutch_general_adapter_contract::ExecutionContextV1 {
        market_id: context.market(),
        release_set_id: context.release_set().to_bytes(),
    }
}

fn require_owned(
    account: &AccountInfo<'_>,
    owner: &Pubkey,
    width: usize,
    writable: bool,
) -> Result<(), ProgramError> {
    if account.owner != owner
        || account.data_len() != width
        || account.executable
        || account.is_signer
        || account.is_writable != writable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn family_pda(
    program_id: &Pubkey,
    market: [u8; 32],
    suffix: &[&[u8]],
) -> Result<Pubkey, ProgramError> {
    let domain = suffix.first().copied().ok_or(TradingSbfError::Content)?;
    let mut seeds = alloc::vec::Vec::with_capacity(suffix.len().saturating_add(1));
    seeds.push(domain);
    seeds.push(market.as_slice());
    seeds.extend(suffix.iter().skip(1).copied());
    Ok(Pubkey::find_program_address(&seeds, program_id).0)
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}
