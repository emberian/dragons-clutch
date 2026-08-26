//! Runtime-width General action controller behind authenticated Trading.
//!
//! The future common hot outer supplies a descriptor/config/root-authenticated
//! [`TradingFamilyContextV1`]. This family module owns no Program identity and
//! introduces no private General authority. It executes the exact General
//! suffix, stages every mutation, and only writes after the complete action
//! accepts.

extern crate alloc;

use alloc::boxed::Box;

use dclutch_general_adapter_contract::{
    consider_verified_input, freeze_selection, initialize_settlement, CandidateVerifierV1,
    ConsiderVerifiedInputV1, VerifiedCandidateV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1,
    GENERAL_CERTIFICATE_PDA_DOMAIN_V1, GENERAL_PAGE_PDA_DOMAIN_V1,
    GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1,
};
use dclutch_general_codec::{
    Action, CandidateV1, ControllerRequestV1, PageViewV1, SelectionCursorV1, SelectionPolicyV1,
    CANDIDATE_BYTES, CONTROLLER_REQUEST_BYTES, PAGE_BYTES, SELECTION_CURSOR_BYTES,
    SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES,
};
use dclutch_general_config_contract::GeneralConfigV2;
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    dispatch::TradingFamilyContextV1, general::route::process_settlement_v2, TradingSbfError,
};

/// Exact General suffix widths before the settlement phase.
pub const GENERAL_CONSIDER_ACCOUNT_COUNT_V2: usize = 12;
/// Exact General Freeze suffix width.
pub const GENERAL_FREEZE_ACCOUNT_COUNT_V2: usize = 6;
/// Exact General InitializeSettlement suffix width.
pub const GENERAL_INITIALIZE_ACCOUNT_COUNT_V2: usize = 9;

const MARKET: usize = 0;
const TRADING_PROGRAM: usize = 3;
const SELECTION: usize = 5;
const VERIFICATION: usize = 6;
const CERTIFICATE: usize = 7;
const CANDIDATE: usize = 8;
const POLICY: usize = 9;
const PAGE: usize = 10;
const INCUMBENT: usize = 11;

struct ConsiderAccountsV2<'a, 'info> {
    selection: &'a AccountInfo<'info>,
    verification: &'a AccountInfo<'info>,
    certificate: &'a AccountInfo<'info>,
    candidate: &'a AccountInfo<'info>,
    policy: &'a AccountInfo<'info>,
    page: &'a AccountInfo<'info>,
    incumbent: &'a AccountInfo<'info>,
}

struct ConsiderSemanticV2<'a> {
    context: TradingFamilyContextV1,
    request: ControllerRequestV1,
    config: GeneralConfigV2,
    candidate: &'a CandidateV1,
    policy: &'a SelectionPolicyV1,
}

struct VerificationStageV2 {
    bytes: [u8; VERIFICATION_CURSOR_BYTES_V1],
    complete: bool,
}

struct SelectionStageV2 {
    selection: [u8; SELECTION_CURSOR_BYTES],
    certificate: [u8; VERIFIED_CANDIDATE_BYTES_V1],
}

/// Execute one exact decoded General-family suffix.
#[inline(never)]
pub fn process_general_action_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if instruction_data.len() != CONTROLLER_REQUEST_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let request =
        ControllerRequestV1::decode(instruction_data).map_err(|_| TradingSbfError::Content)?;
    authenticate_common(program_id, context, accounts, config)?;
    match request.action {
        Action::Consider => process_consider(program_id, context, accounts, request, config),
        Action::Freeze => process_freeze(program_id, context, accounts, request, config),
        Action::InitializeSettlement => {
            process_initialize(program_id, context, accounts, request, config)
        }
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            process_settlement_v2(program_id, context, accounts, request, config)
        }
    }
}

#[inline(never)]
fn process_consider(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if accounts.len() != GENERAL_CONSIDER_ACCOUNT_COUNT_V2 {
        return Err(TradingSbfError::Content.into());
    }
    let observed = ConsiderAccountsV2 {
        selection: account(accounts, SELECTION)?,
        verification: account(accounts, VERIFICATION)?,
        certificate: account(accounts, CERTIFICATE)?,
        candidate: account(accounts, CANDIDATE)?,
        policy: account(accounts, POLICY)?,
        page: account(accounts, PAGE)?,
        incumbent: account(accounts, INCUMBENT)?,
    };
    require_owned(observed.selection, program_id, SELECTION_CURSOR_BYTES, true)?;
    require_owned(
        observed.verification,
        program_id,
        VERIFICATION_CURSOR_BYTES_V1,
        true,
    )?;
    require_owned(
        observed.certificate,
        program_id,
        VERIFIED_CANDIDATE_BYTES_V1,
        true,
    )?;
    require_owned(observed.candidate, program_id, CANDIDATE_BYTES, false)?;
    require_owned(observed.policy, program_id, SELECTION_POLICY_BYTES, false)?;
    require_owned(observed.page, program_id, PAGE_BYTES, false)?;
    require_distinct(
        accounts,
        &[
            SELECTION,
            VERIFICATION,
            CERTIFICATE,
            CANDIDATE,
            POLICY,
            PAGE,
        ],
    )?;

    let candidate = Box::new(decode_candidate(observed.candidate)?);
    let policy = Box::new(decode_policy(observed.policy)?);
    config
        .require_selection_policy(policy.policy_id)
        .map_err(|_| TradingSbfError::Content)?;
    if request.candidate_id != Some(candidate.candidate_id) {
        return Err(TradingSbfError::Content.into());
    }
    let semantic = ConsiderSemanticV2 {
        context,
        request,
        config,
        candidate: candidate.as_ref(),
        policy: policy.as_ref(),
    };
    authenticate_immutable_pdas(program_id, &observed, &semantic)?;
    require_pda(
        program_id,
        observed.selection.key,
        context.market(),
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &candidate.batch_id],
    )?;
    require_pda(
        program_id,
        observed.verification.key,
        context.market(),
        &[GENERAL_VERIFICATION_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    require_pda(
        program_id,
        observed.certificate.key,
        context.market(),
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;

    let verification_after = prepare_verification_page(&observed, &semantic)?;
    let selection_after = if verification_after.complete {
        Some(prepare_best_valid_submitted_candidate(
            program_id,
            accounts,
            &observed,
            &semantic,
            &verification_after.bytes,
        )?)
    } else if account_has_data(observed.certificate)? {
        return Err(TradingSbfError::Content.into());
    } else {
        None
    };

    if let Some(after) = selection_after {
        observed
            .selection
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?
            .copy_from_slice(&after.selection);
        observed
            .certificate
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?
            .copy_from_slice(&after.certificate);
    }
    observed
        .verification
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?
        .copy_from_slice(&verification_after.bytes);
    Ok(())
}

#[inline(never)]
fn prepare_verification_page(
    observed: &ConsiderAccountsV2<'_, '_>,
    semantic: &ConsiderSemanticV2<'_>,
) -> Result<Box<VerificationStageV2>, ProgramError> {
    let mut verifier = Box::new(load_verifier(
        observed.verification,
        *semantic.candidate,
        semantic.request,
    )?);
    {
        let page = observed
            .page
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        verifier
            .ingest_page_at(&page, semantic.request.expected_revision)
            .map_err(|_| TradingSbfError::Transition)?;
    }
    semantic
        .config
        .require_candidate_envelope(
            semantic.candidate.outcome_count,
            semantic.candidate.page_count,
            semantic.candidate.price_scale,
            verifier.order_count(),
        )
        .map_err(|_| TradingSbfError::Content)?;
    let mut stage = Box::new(VerificationStageV2 {
        bytes: [0; VERIFICATION_CURSOR_BYTES_V1],
        complete: verifier.is_complete(),
    });
    verifier
        .encode_into(&mut stage.bytes)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(stage)
}

#[inline(never)]
fn prepare_best_valid_submitted_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    observed: &ConsiderAccountsV2<'_, '_>,
    semantic: &ConsiderSemanticV2<'_>,
    verification_after: &[u8; VERIFICATION_CURSOR_BYTES_V1],
) -> Result<Box<SelectionStageV2>, ProgramError> {
    let verified = CandidateVerifierV1::decode(verification_after)
        .map_err(|_| TradingSbfError::Transition)?
        .finish()
        .map_err(|_| TradingSbfError::Transition)?;
    let mut output = Box::new(SelectionStageV2 {
        selection: copy_exact::<SELECTION_CURSOR_BYTES>(observed.selection)?,
        certificate: copy_exact::<VERIFIED_CANDIDATE_BYTES_V1>(observed.certificate)?,
    });
    let incumbent = incumbent(
        program_id,
        semantic.context,
        accounts,
        &output.selection,
        observed.incumbent,
    )?;
    let selection_revision = if output.selection.iter().all(|byte| *byte == 0) {
        0
    } else {
        SelectionCursorV1::decode(&output.selection)
            .map_err(|_| TradingSbfError::Transition)?
            .revision
    };
    consider_verified_input(
        &mut output.selection,
        &mut output.certificate,
        ConsiderVerifiedInputV1 {
            candidate: semantic.candidate,
            policy: semantic.policy,
            verified: &verified,
            incumbent: incumbent.as_ref(),
            expected_revision: selection_revision,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(output)
}

#[inline(never)]
fn process_freeze(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if accounts.len() != GENERAL_FREEZE_ACCOUNT_COUNT_V2
        || request.candidate_id.is_some()
        || request.page_index != 0
        || request.execution_index != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let selection = account(accounts, SELECTION)?;
    require_owned(selection, program_id, SELECTION_CURSOR_BYTES, true)?;
    let mut output = copy_exact::<SELECTION_CURSOR_BYTES>(selection)?;
    let cursor = SelectionCursorV1::decode(&output).map_err(|_| TradingSbfError::Transition)?;
    config
        .require_selection_policy(cursor.policy_id)
        .map_err(|_| TradingSbfError::Content)?;
    require_pda(
        program_id,
        selection.key,
        context.market(),
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &cursor.batch_id],
    )?;
    freeze_selection(&mut output, request.expected_revision)
        .map_err(|_| TradingSbfError::Transition)?;
    selection
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?
        .copy_from_slice(&output);
    Ok(())
}

#[inline(never)]
fn process_initialize(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if accounts.len() != GENERAL_INITIALIZE_ACCOUNT_COUNT_V2
        || request.page_index != 0
        || request.execution_index != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let selection = account(accounts, SELECTION)?;
    let settlement = account(accounts, VERIFICATION)?;
    let certificate = account(accounts, CERTIFICATE)?;
    let candidate_account = account(accounts, CANDIDATE)?;
    require_owned(selection, program_id, SELECTION_CURSOR_BYTES, false)?;
    require_owned(settlement, program_id, SETTLEMENT_CURSOR_BYTES, true)?;
    require_owned(certificate, program_id, VERIFIED_CANDIDATE_BYTES_V1, false)?;
    require_owned(candidate_account, program_id, CANDIDATE_BYTES, false)?;
    require_distinct(accounts, &[SELECTION, VERIFICATION, CERTIFICATE, CANDIDATE])?;
    let candidate = decode_candidate(candidate_account)?;
    if request.candidate_id != Some(candidate.candidate_id) {
        return Err(TradingSbfError::Content.into());
    }
    let certificate_bytes = certificate
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let verified =
        VerifiedCandidateV1::decode(&certificate_bytes).map_err(|_| TradingSbfError::Content)?;
    if verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        return Err(TradingSbfError::Content.into());
    }
    config
        .require_candidate_envelope(
            candidate.outcome_count,
            candidate.page_count,
            candidate.price_scale,
            0,
        )
        .map_err(|_| TradingSbfError::Content)?;
    require_pda(
        program_id,
        selection.key,
        context.market(),
        &[GENERAL_SELECTION_PDA_DOMAIN_V1, &candidate.batch_id],
    )?;
    require_pda(
        program_id,
        settlement.key,
        context.market(),
        &[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    require_pda(
        program_id,
        certificate.key,
        context.market(),
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    require_pda(
        program_id,
        candidate_account.key,
        context.market(),
        &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
    )?;
    let selection_bytes = selection
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let selection_cursor =
        SelectionCursorV1::decode(&selection_bytes).map_err(|_| TradingSbfError::Transition)?;
    config
        .require_selection_policy(selection_cursor.policy_id)
        .map_err(|_| TradingSbfError::Content)?;
    let mut settlement_after = copy_exact::<SETTLEMENT_CURSOR_BYTES>(settlement)?;
    initialize_settlement(
        &mut settlement_after,
        &selection_bytes,
        &verified,
        request.expected_revision,
    )
    .map_err(|_| TradingSbfError::Transition)?;
    drop(selection_bytes);
    drop(certificate_bytes);
    settlement
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?
        .copy_from_slice(&settlement_after);
    Ok(())
}

fn authenticate_common(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    config: GeneralConfigV2,
) -> Result<(), ProgramError> {
    if accounts.len() <= TRADING_PROGRAM
        || context.program_id() != program_id.to_bytes()
        || context.market() != account(accounts, MARKET)?.key.to_bytes()
        || context.generation() != config.generation()
        || account(accounts, MARKET)?.is_signer
        || account(accounts, MARKET)?.is_writable
        || account(accounts, MARKET)?.executable
        || account(accounts, TRADING_PROGRAM)?.key != program_id
        || !account(accounts, TRADING_PROGRAM)?.executable
        || account(accounts, TRADING_PROGRAM)?.is_signer
        || account(accounts, TRADING_PROGRAM)?.is_writable
        || accounts.iter().any(|value| value.is_signer)
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn load_verifier(
    verification: &AccountInfo<'_>,
    candidate: CandidateV1,
    request: ControllerRequestV1,
) -> Result<CandidateVerifierV1, ProgramError> {
    let bytes = verification
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    if bytes.iter().all(|byte| *byte == 0) {
        if request.page_index != 0 || request.expected_revision != 0 {
            return Err(TradingSbfError::Transition.into());
        }
        return Ok(CandidateVerifierV1::begin(candidate));
    }
    let verifier = CandidateVerifierV1::decode(&bytes).map_err(|_| TradingSbfError::Transition)?;
    if verifier.candidate() != candidate || verifier.next_page() != request.page_index {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(verifier)
}

fn incumbent(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    selection_bytes: &[u8; SELECTION_CURSOR_BYTES],
    observed: &AccountInfo<'_>,
) -> Result<Option<VerifiedCandidateV1>, ProgramError> {
    if selection_bytes.iter().all(|byte| *byte == 0) {
        if observed.key != account(accounts, MARKET)?.key {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(None);
    }
    let selection =
        SelectionCursorV1::decode(selection_bytes).map_err(|_| TradingSbfError::Transition)?;
    let Some(best) = selection.best_candidate_id else {
        if observed.key != account(accounts, MARKET)?.key {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(None);
    };
    require_owned(observed, program_id, VERIFIED_CANDIDATE_BYTES_V1, false)?;
    require_pda(
        program_id,
        observed.key,
        context.market(),
        &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &best],
    )?;
    let bytes = observed
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let value = VerifiedCandidateV1::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    if value.candidate_id != best {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Some(value))
}

fn authenticate_immutable_pdas(
    program_id: &Pubkey,
    observed: &ConsiderAccountsV2<'_, '_>,
    semantic: &ConsiderSemanticV2<'_>,
) -> Result<(), ProgramError> {
    require_pda(
        program_id,
        observed.candidate.key,
        semantic.context.market(),
        &[
            GENERAL_CANDIDATE_PDA_DOMAIN_V1,
            &semantic.candidate.candidate_id,
        ],
    )?;
    require_pda(
        program_id,
        observed.policy.key,
        semantic.context.market(),
        &[GENERAL_POLICY_PDA_DOMAIN_V1, &semantic.policy.policy_id],
    )?;
    let page_bytes = observed
        .page
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let page = PageViewV1::decode(&page_bytes).map_err(|_| TradingSbfError::Content)?;
    if page.candidate_id() != semantic.candidate.candidate_id
        || page.outcome_count() != semantic.candidate.outcome_count
        || page.page_count() != semantic.candidate.page_count
        || page.page_index() != semantic.request.page_index
    {
        return Err(TradingSbfError::Content.into());
    }
    require_pda(
        program_id,
        observed.page.key,
        semantic.context.market(),
        &[
            GENERAL_PAGE_PDA_DOMAIN_V1,
            &semantic.candidate.candidate_id,
            &semantic.request.page_index.to_le_bytes(),
        ],
    )
}

fn account_has_data(account: &AccountInfo<'_>) -> Result<bool, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(bytes.iter().any(|byte| *byte != 0))
}

fn decode_candidate(account: &AccountInfo<'_>) -> Result<CandidateV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    CandidateV1::decode(&bytes).map_err(|_| TradingSbfError::Content.into())
}

fn decode_policy(account: &AccountInfo<'_>) -> Result<SelectionPolicyV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    SelectionPolicyV1::decode(&bytes).map_err(|_| TradingSbfError::Content.into())
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

fn require_pda(
    program_id: &Pubkey,
    actual: &Pubkey,
    market: [u8; 32],
    suffix: &[&[u8]],
) -> Result<(), ProgramError> {
    let domain = suffix.first().copied().ok_or(TradingSbfError::Content)?;
    let mut seeds = alloc::vec::Vec::with_capacity(suffix.len().saturating_add(1));
    seeds.push(domain);
    seeds.push(market.as_slice());
    seeds.extend(suffix.iter().skip(1).copied());
    if Pubkey::find_program_address(&seeds, program_id).0 != *actual {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountInfo<'_>], indices: &[usize]) -> Result<(), ProgramError> {
    for (position, left) in indices.iter().enumerate() {
        for right in indices.iter().skip(position.saturating_add(1)) {
            if account(accounts, *left)?.key == account(accounts, *right)?.key {
                return Err(TradingSbfError::Content.into());
            }
        }
    }
    Ok(())
}

fn copy_exact<const N: usize>(account: &AccountInfo<'_>) -> Result<[u8; N], ProgramError> {
    let source = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    source
        .as_ref()
        .try_into()
        .map_err(|_| TradingSbfError::Content.into())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}
