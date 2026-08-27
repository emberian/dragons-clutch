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
    CandidateVerifierV1, ConsiderVerifiedInputV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1,
    GENERAL_CERTIFICATE_PDA_DOMAIN_V1, GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1,
    GENERAL_SELECTION_PDA_DOMAIN_V1, GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
    GENERAL_VERIFICATION_PDA_DOMAIN_V1, VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1,
    VerifiedCandidateV1, consider_verified_input, freeze_selection, initialize_settlement,
};
use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CONTROLLER_REQUEST_BYTES, CandidateV1, ControllerRequestV1,
    PAGE_BYTES, PageViewV1, SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES,
    SETTLEMENT_CURSOR_BYTES, SelectionCursorV1, SelectionPolicyV1,
};
use dclutch_general_config_contract::{GeneralConfigV2, GeneralLifecycleV2, GeneralRootV2};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    TradingSbfError, dispatch::TradingFamilyContextV1, general::route::process_settlement_v2,
};

/// Exact General suffix widths before the settlement phase.
pub const GENERAL_CONSIDER_ACCOUNT_COUNT_V2: usize = 12;
/// Exact General Freeze suffix width.
pub const GENERAL_FREEZE_ACCOUNT_COUNT_V2: usize = 6;
/// Exact General InitializeSettlement suffix width.
pub const GENERAL_INITIALIZE_ACCOUNT_COUNT_V2: usize = 9;

const MARKET: usize = 0;
const TRADING_PROGRAM: usize = 3;
pub(crate) const SELECTION: usize = 5;
pub(crate) const VERIFICATION: usize = 6;
pub(crate) const CERTIFICATE: usize = 7;
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
///
/// `root_state` is the mutable General tail the common layer split off the
/// authenticated composite root account. The family owns its lifecycle
/// refusal; the common header proves only identity, never that this capability
/// is still accepting work.
#[inline(never)]
pub fn process_general_action_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    config: GeneralConfigV2,
    root_state: GeneralRootV2,
) -> Result<(), ProgramError> {
    if instruction_data.len() != CONTROLLER_REQUEST_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let request =
        ControllerRequestV1::decode(instruction_data).map_err(|_| TradingSbfError::Content)?;
    authenticate_common(program_id, context, accounts, config, root_state)?;
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
    root_state: GeneralRootV2,
) -> Result<(), ProgramError> {
    if accounts.len() <= TRADING_PROGRAM
        || context.program_id() != program_id.to_bytes()
        || context.market() != account(accounts, MARKET)?.key.to_bytes()
        || context.generation() != config.generation()
        || root_state.lifecycle() != GeneralLifecycleV2::Active
        || root_state.market() != context.market()
        || root_state.generation() != context.generation()
        || root_state.config_id() != context.selection().config().to_bytes()
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

#[cfg(test)]
pub(crate) mod tests {
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_capability_program_contract::{
        CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    };
    use dclutch_core_contract::ContentId;
    use dclutch_general_adapter_contract::CandidateVerifierV1;
    use dclutch_general_codec::{
        ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES, MAX_SELECTION_CRITERIA, PageV1, Phase,
        SelectionCriterion, SettlementCursorV1,
    };
    use dclutch_general_config_contract::{
        GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GeneralConfigV2Input,
    };
    use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1, ProgramIdentityV1,
    };
    use solana_program::hash::hash;

    use super::*;

    const GENERATION: u64 = 7;

    pub(crate) fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        *value.get_mut(0).expect("identity byte") = low;
        value
    }

    fn cid(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero content identity")
    }

    fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        *values.get_mut(0).expect("first outcome") = first;
        *values.get_mut(1).expect("second outcome") = second;
        values
    }

    pub(crate) fn at<'a>(
        frame: &'a [AccountInfo<'static>],
        index: usize,
    ) -> &'a AccountInfo<'static> {
        frame.get(index).expect("frame account")
    }

    pub(crate) fn borrowed(account: &AccountInfo<'_>) -> Vec<u8> {
        account.try_borrow_data().expect("account data").to_vec()
    }

    fn flip_byte(account: &AccountInfo<'_>, offset: usize) {
        let mut data = account.try_borrow_mut_data().expect("account data");
        let byte = data.get_mut(offset).expect("byte within the record");
        *byte ^= 1;
    }

    pub(crate) fn account(
        key: Pubkey,
        writable: bool,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            writable,
            Box::leak(Box::new(1_u64)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn readonly(owner: Pubkey, executable: bool) -> AccountInfo<'static> {
        account(Pubkey::new_unique(), false, Vec::new(), owner, executable)
    }

    fn candidate() -> CandidateV1 {
        CandidateV1 {
            outcome_count: 2,
            candidate_id: id(21),
            product_id: id(31),
            batch_id: id(41),
            page_count: 1,
            price_scale: 2,
            prices: vector(1, 1),
        }
    }

    fn policy() -> SelectionPolicyV1 {
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        *criteria.get_mut(1).expect("second criterion") = SelectionCriterion::MinimizeQuoteSurplus;
        *criteria.get_mut(2).expect("third criterion") = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: id(51),
            criterion_count: 3,
            criteria,
        }
    }

    fn config_with_policy(selection_policy_id: [u8; 32]) -> GeneralConfigV2 {
        GeneralConfigV2::new(GeneralConfigV2Input {
            capacity_profile_id: id(61),
            claim_basis_id: id(62),
            capability_program_id: id(64),
            generation: GENERATION,
            price_scale: 2,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: 32,
            max_pages_per_candidate: 1,
            continuation_reward_lamports: 5,
            selection_policy_id,
            outcome_count: 2,
            quote_surplus_beneficiary: id(63),
        })
        .expect("General config")
    }

    pub(crate) fn config() -> GeneralConfigV2 {
        config_with_policy(policy().policy_id)
    }

    fn execution(order: u8, owner: u8, receive: [u64; MAX_OUTCOMES]) -> ExecutionV1 {
        ExecutionV1 {
            order_id: id(order),
            owner_id: id(owner),
            nonce: 1,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: receive,
            deliver_per_lot: [0; MAX_OUTCOMES],
        }
    }

    fn page() -> [u8; PAGE_BYTES] {
        let mut rows = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        *rows.get_mut(0).expect("first row") = execution(1, 11, vector(1, 0));
        *rows.get_mut(1).expect("second row") = execution(2, 12, vector(0, 1));
        PageV1 {
            outcome_count: 2,
            candidate_id: candidate().candidate_id,
            page_index: 0,
            page_count: 1,
            execution_count: 2,
            executions: rows,
        }
        .to_bytes()
        .expect("page")
    }

    /// The composite root the common Trading layer authenticates, with the
    /// exact account bytes behind it.
    pub(crate) struct CompositeRootV2 {
        /// Context the common layer derives from the immutable header.
        pub(crate) context: TradingFamilyContextV1,
        /// Authenticated composite root-account address.
        pub(crate) root_key: Pubkey,
        /// Initial active General tail.
        pub(crate) root_state: GeneralRootV2,
        /// Exact `header || tail` account bytes.
        pub(crate) account_bytes: Vec<u8>,
    }

    impl CompositeRootV2 {
        /// Rebuild the same authenticated account carrying another tail.
        ///
        /// The immutable header is byte-identical, so the common layer still
        /// authenticates exactly this capability; only the family lifecycle
        /// the header says nothing about has moved.
        pub(crate) fn with_state(&self, state: GeneralRootV2) -> Vec<u8> {
            let mut bytes = self.account_bytes.clone();
            bytes
                .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .expect("General tail")
                .copy_from_slice(&state.to_bytes());
            bytes
        }
    }

    /// Build the composite root the common Trading layer authenticates.
    pub(crate) fn composite_root_v2(
        program_id: Pubkey,
        market: Pubkey,
        config: GeneralConfigV2,
    ) -> CompositeRootV2 {
        let config_id = hash(&config.to_bytes()).to_bytes();
        let release_set = cid(id(70));
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            cid(id(71)),
            cid(GENERAL_CAPABILITY_KIND_ID_V1),
            cid(id(64)),
            cid(config_id),
        )
        .expect("selection");
        let header =
            CapabilityRootHeaderV1::new(release_set, market.to_bytes(), GENERATION, selection)
                .expect("root header");
        let root_key = Pubkey::find_program_address(&header.seeds().as_slices(), &program_id).0;
        let root_state = GeneralRootV2::active(market.to_bytes(), config_id, GENERATION)
            .expect("General root tail");
        let mut root_account = Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1);
        root_account.extend_from_slice(&header.to_bytes());
        root_account.extend_from_slice(&root_state.to_bytes());
        assert_eq!(
            root_account.len(),
            CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2
        );
        let receipt = AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            release_set,
            ProgramIdentityV1::new(program_id.to_bytes()).expect("Trading program"),
            ArtifactReleaseIdV1::new(id(72)).expect("artifact release"),
            cid(id(73)),
        );
        let context = TradingFamilyContextV1::authenticate(
            &program_id,
            &root_key,
            &program_id,
            &root_account,
            receipt,
        )
        .expect("authenticated family context");
        CompositeRootV2 {
            context,
            root_key,
            root_state,
            account_bytes: root_account,
        }
    }

    /// Return only the two values the family transition itself consumes.
    fn composite_root(
        program_id: Pubkey,
        market: Pubkey,
        config: GeneralConfigV2,
    ) -> (TradingFamilyContextV1, GeneralRootV2) {
        let root = composite_root_v2(program_id, market, config);
        (root.context, root.root_state)
    }

    /// The five readonly accounts every General route starts with.
    fn common(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        vec![
            account(market, false, Vec::new(), Pubkey::new_unique(), false),
            readonly(Pubkey::new_unique(), false),
            readonly(Pubkey::new_unique(), true),
            account(program_id, false, Vec::new(), Pubkey::new_unique(), true),
            readonly(Pubkey::new_unique(), false),
        ]
    }

    fn family_pda(program_id: Pubkey, market: Pubkey, suffix: &[&[u8]]) -> Pubkey {
        let mut seeds: Vec<&[u8]> = Vec::with_capacity(suffix.len().saturating_add(1));
        seeds.push(suffix.first().copied().expect("seed domain"));
        seeds.push(market.as_ref());
        seeds.extend(suffix.iter().skip(1).copied());
        Pubkey::find_program_address(&seeds, &program_id).0
    }

    fn owned(
        program_id: Pubkey,
        key: Pubkey,
        writable: bool,
        data: Vec<u8>,
    ) -> AccountInfo<'static> {
        account(key, writable, data, program_id, false)
    }

    pub(crate) fn consider_frame(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        let candidate = candidate();
        let policy = policy();
        let mut frame = common(program_id, market);
        let market_account = at(&frame, MARKET).clone();
        let page_index = 0_u32.to_le_bytes();
        frame.extend([
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_SELECTION_PDA_DOMAIN_V1, &candidate.batch_id],
                ),
                true,
                vec![0; SELECTION_CURSOR_BYTES],
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_VERIFICATION_PDA_DOMAIN_V1, &candidate.candidate_id],
                ),
                true,
                vec![0; VERIFICATION_CURSOR_BYTES_V1],
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, &candidate.candidate_id],
                ),
                true,
                vec![0; VERIFIED_CANDIDATE_BYTES_V1],
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_CANDIDATE_PDA_DOMAIN_V1, &candidate.candidate_id],
                ),
                false,
                candidate.to_bytes().expect("candidate").to_vec(),
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_POLICY_PDA_DOMAIN_V1, &policy.policy_id],
                ),
                false,
                policy.to_bytes().expect("policy").to_vec(),
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[
                        GENERAL_PAGE_PDA_DOMAIN_V1,
                        &candidate.candidate_id,
                        &page_index,
                    ],
                ),
                false,
                page().to_vec(),
            ),
            market_account,
        ]);
        frame
    }

    pub(crate) fn consider_request() -> ControllerRequestV1 {
        ControllerRequestV1 {
            action: Action::Consider,
            expected_revision: 0,
            candidate_id: Some(candidate().candidate_id),
            page_index: 0,
            execution_index: 0,
        }
    }

    fn execute(
        program_id: &Pubkey,
        context: TradingFamilyContextV1,
        accounts: &[AccountInfo<'_>],
        request: ControllerRequestV1,
        config: GeneralConfigV2,
        root_state: GeneralRootV2,
    ) -> Result<(), ProgramError> {
        process_general_action_v2(
            program_id,
            context,
            accounts,
            &request.to_bytes().expect("request bytes"),
            config,
            root_state,
        )
    }

    #[test]
    fn authenticated_consider_streams_and_commits_exact_certificate() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let config = config();
        let (context, root_state) = composite_root(program_id, market, config);
        let frame = consider_frame(program_id, market);
        execute(
            &program_id,
            context,
            &frame,
            consider_request(),
            config,
            root_state,
        )
        .expect("consider");
        let selection =
            SelectionCursorV1::decode(&borrowed(at(&frame, SELECTION))).expect("selection");
        assert_eq!(selection.best_candidate_id, Some(candidate().candidate_id));
        assert_eq!(selection.revision, 1);
        let certificate =
            VerifiedCandidateV1::decode(&borrowed(at(&frame, CERTIFICATE))).expect("certificate");
        assert_eq!(certificate.complete_set_quantity, 1);
        assert_eq!(certificate.quote_surplus, 1);
        let verifier =
            CandidateVerifierV1::decode(&borrowed(at(&frame, VERIFICATION))).expect("verification");
        assert!(verifier.is_complete());
    }

    #[test]
    fn substituted_config_policy_and_inactive_root_refuse_before_state_change() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let config = config();

        // A capability whose immutable config names another selection policy:
        // the authenticated policy record no longer joins and nothing is
        // written. The substituted config is carried coherently through the
        // composite root, so this is a real alternative capability rather than
        // a torn one.
        let substituted_config = config_with_policy(id(52));
        let (substituted_context, substituted_root) =
            composite_root(program_id, market, substituted_config);
        let frame = consider_frame(program_id, market);
        let selection_before = borrowed(at(&frame, SELECTION));
        let verification_before = borrowed(at(&frame, VERIFICATION));
        let certificate_before = borrowed(at(&frame, CERTIFICATE));
        assert_eq!(
            execute(
                &program_id,
                substituted_context,
                &frame,
                consider_request(),
                substituted_config,
                substituted_root,
            ),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(borrowed(at(&frame, SELECTION)), selection_before);
        assert_eq!(borrowed(at(&frame, VERIFICATION)), verification_before);
        assert_eq!(borrowed(at(&frame, CERTIFICATE)), certificate_before);

        // A substituted policy record supplied at its own derived address.
        let (context, root_state) = composite_root(program_id, market, config);
        let mut policy_substitution = consider_frame(program_id, market);
        let mut substituted_policy = policy();
        substituted_policy.policy_id = id(52);
        *policy_substitution
            .get_mut(POLICY)
            .expect("policy coordinate") = owned(
            program_id,
            family_pda(
                program_id,
                market,
                &[GENERAL_POLICY_PDA_DOMAIN_V1, &substituted_policy.policy_id],
            ),
            false,
            substituted_policy
                .to_bytes()
                .expect("substituted policy")
                .to_vec(),
        );
        let policy_verification_before = borrowed(at(&policy_substitution, VERIFICATION));
        assert_eq!(
            execute(
                &program_id,
                context,
                &policy_substitution,
                consider_request(),
                config,
                root_state,
            ),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            borrowed(at(&policy_substitution, VERIFICATION)),
            policy_verification_before
        );

        // A capability whose General tail has left Active. The common root
        // header is exactly right and proves only identity, so the family's own
        // lifecycle is the sole refusal.
        let inactive = consider_frame(program_id, market);
        let mut retiring = root_state;
        retiring.begin_retiring(1).expect("retiring root");
        assert_eq!(
            execute(
                &program_id,
                context,
                &inactive,
                consider_request(),
                config,
                retiring,
            ),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            borrowed(at(&inactive, VERIFICATION)),
            vec![0; VERIFICATION_CURSOR_BYTES_V1]
        );
    }

    #[test]
    fn hostile_page_and_stale_replay_preserve_all_general_state() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let config = config();
        let (context, root_state) = composite_root(program_id, market, config);
        let frame = consider_frame(program_id, market);
        let selection_before = borrowed(at(&frame, SELECTION));
        let verification_before = borrowed(at(&frame, VERIFICATION));
        let certificate_before = borrowed(at(&frame, CERTIFICATE));
        flip_byte(at(&frame, PAGE), 16);
        assert_eq!(
            execute(
                &program_id,
                context,
                &frame,
                consider_request(),
                config,
                root_state,
            ),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(borrowed(at(&frame, SELECTION)), selection_before);
        assert_eq!(borrowed(at(&frame, VERIFICATION)), verification_before);
        assert_eq!(borrowed(at(&frame, CERTIFICATE)), certificate_before);

        flip_byte(at(&frame, PAGE), 16);
        execute(
            &program_id,
            context,
            &frame,
            consider_request(),
            config,
            root_state,
        )
        .expect("first consider");
        let snapshot = borrowed(at(&frame, SELECTION));
        assert_eq!(
            execute(
                &program_id,
                context,
                &frame,
                consider_request(),
                config,
                root_state,
            ),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(borrowed(at(&frame, SELECTION)), snapshot);
    }

    #[test]
    fn freeze_then_initialize_enters_zero_inventory_collecting_phase() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let config = config();
        let (context, root_state) = composite_root(program_id, market, config);
        let consider = consider_frame(program_id, market);
        execute(
            &program_id,
            context,
            &consider,
            consider_request(),
            config,
            root_state,
        )
        .expect("consider");

        let selection_key = *at(&consider, SELECTION).key;
        let mut freeze = common(program_id, market);
        freeze.push(owned(
            program_id,
            selection_key,
            true,
            borrowed(at(&consider, SELECTION)),
        ));
        execute(
            &program_id,
            context,
            &freeze,
            ControllerRequestV1 {
                action: Action::Freeze,
                expected_revision: 1,
                candidate_id: None,
                page_index: 0,
                execution_index: 0,
            },
            config,
            root_state,
        )
        .expect("freeze");

        let candidate = candidate();
        let mut initialize = common(program_id, market);
        initialize.extend([
            owned(
                program_id,
                selection_key,
                false,
                borrowed(at(&freeze, SELECTION)),
            ),
            owned(
                program_id,
                family_pda(
                    program_id,
                    market,
                    &[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, &candidate.candidate_id],
                ),
                true,
                vec![0; SETTLEMENT_CURSOR_BYTES],
            ),
            owned(
                program_id,
                *at(&consider, CERTIFICATE).key,
                false,
                borrowed(at(&consider, CERTIFICATE)),
            ),
            owned(
                program_id,
                *at(&consider, CANDIDATE).key,
                false,
                borrowed(at(&consider, CANDIDATE)),
            ),
        ]);
        execute(
            &program_id,
            context,
            &initialize,
            ControllerRequestV1 {
                action: Action::InitializeSettlement,
                expected_revision: 0,
                candidate_id: Some(candidate.candidate_id),
                page_index: 0,
                execution_index: 0,
            },
            config,
            root_state,
        )
        .expect("initialize");
        let settlement = SettlementCursorV1::decode(&borrowed(at(&initialize, VERIFICATION)))
            .expect("settlement cursor");
        assert_eq!(settlement.phase, Phase::Collecting);
        assert_eq!(settlement.next_page, 0);
        assert_eq!(settlement.next_execution, 0);
        assert_eq!(settlement.claim_inventory, [0; MAX_OUTCOMES]);
        assert_eq!(settlement.quote_inventory, 0);
    }
}
