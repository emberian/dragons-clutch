#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::indexing_slicing)]

//! Registry-authenticated, verifier-oriented General SBF adapter.
//!
//! The currently executable slice streams candidate verification, freezes the
//! best valid submitted candidate, and initializes settlement. Physical Claims
//! and Custody actions refuse until their separately owned canonical child
//! adapters are linked; no provisional child wire exists here.

extern crate std;

use dclutch_general_adapter_contract::{
    CandidateVerifierV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
    GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1, VerifiedCandidateV1,
    consider_verified, freeze_selection, initialize_settlement,
};
use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CONTROLLER_REQUEST_BYTES, CandidateV1, ControllerRequestV1,
    PAGE_BYTES, SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES,
    SelectionCursorV1, SelectionPolicyV1,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::vec::Vec;

/// Exact account count for streamed candidate consideration.
pub const CONSIDER_ACCOUNT_COUNT_V1: usize = 12;
/// Exact account count for selection freeze.
pub const FREEZE_ACCOUNT_COUNT_V1: usize = 6;
/// Exact account count for settlement initialization.
pub const INITIALIZE_ACCOUNT_COUNT_V1: usize = 9;
/// Exact reserved account count for physical settlement actions.
pub const SETTLEMENT_ACCOUNT_COUNT_V1: usize = 24;

const MARKET: usize = 0;
const ACTIVATION_CACHE: usize = 1;
const REGISTRY_PROGRAM: usize = 2;
const TRADING_PROGRAM: usize = 3;
const TRADING_PROGRAMDATA: usize = 4;

/// Stable physical-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSbfError {
    /// The exact 64-byte controller request refused.
    Instruction = 0,
    /// Account count, order, aliasing, owner, or privilege refused.
    AccountFrame = 1,
    /// Registry activation cache, Trading deployment, or receipt refused.
    ReleaseAdmission = 2,
    /// Candidate, policy, or page bytes/PDA refused.
    ImmutableInput = 3,
    /// Verification cursor bytes/PDA or page progression refused.
    Verification = 4,
    /// Selection cursor bytes/PDA or transition refused.
    Selection = 5,
    /// Verified certificate bytes/PDA refused.
    Certificate = 6,
    /// Settlement cursor bytes/PDA or initialization refused.
    Settlement = 7,
    /// An account-data borrow refused.
    Borrow = 8,
    /// Canonical Claims/Custody child integration is not linked yet.
    ChildUnavailable = 9,
}

impl From<GeneralSbfError> for ProgramError {
    fn from(value: GeneralSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Reauthenticate Trading and execute one exact General action.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != CONTROLLER_REQUEST_BYTES {
        return Err(GeneralSbfError::Instruction.into());
    }
    let request =
        ControllerRequestV1::decode(instruction_data).map_err(|_| GeneralSbfError::Instruction)?;
    validate_common(program_id, accounts, request.action)?;
    let release_set_id = reauthenticate_trading(program_id, accounts)?;
    process_authenticated(program_id, accounts, request, release_set_id)
}

/// Execute after exact Registry Trading reauthentication.
///
/// This function is public so host tests can exercise state rollback without
/// replacing Solana's Registry CPI runtime.
#[inline(never)]
pub fn process_authenticated(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
    release_set_id: [u8; 32],
) -> ProgramResult {
    if release_set_id.iter().all(|byte| *byte == 0) {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    match request.action {
        Action::Consider => process_consider(program_id, accounts, request),
        Action::Freeze => process_freeze(program_id, accounts, request),
        Action::InitializeSettlement => process_initialize(program_id, accounts, request),
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            if accounts.len() != SETTLEMENT_ACCOUNT_COUNT_V1 {
                Err(GeneralSbfError::AccountFrame.into())
            } else {
                Err(GeneralSbfError::ChildUnavailable.into())
            }
        }
    }
}

fn validate_common(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: Action,
) -> ProgramResult {
    let expected = match action {
        Action::Consider => CONSIDER_ACCOUNT_COUNT_V1,
        Action::Freeze => FREEZE_ACCOUNT_COUNT_V1,
        Action::InitializeSettlement => INITIALIZE_ACCOUNT_COUNT_V1,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            SETTLEMENT_ACCOUNT_COUNT_V1
        }
    };
    if accounts.len() != expected
        || accounts.iter().any(|account| account.is_signer)
        || accounts[MARKET].executable
        || accounts[MARKET].is_writable
        || accounts[ACTIVATION_CACHE].executable
        || accounts[ACTIVATION_CACHE].is_writable
        || !accounts[REGISTRY_PROGRAM].executable
        || accounts[REGISTRY_PROGRAM].is_writable
        || !accounts[TRADING_PROGRAM].executable
        || accounts[TRADING_PROGRAM].is_writable
        || accounts[TRADING_PROGRAM].key != program_id
        || accounts[TRADING_PROGRAMDATA].executable
        || accounts[TRADING_PROGRAMDATA].is_writable
        || accounts[REGISTRY_PROGRAM].key == program_id
    {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    Ok(())
}

#[inline(never)]
fn reauthenticate_trading(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    let registry = &accounts[REGISTRY_PROGRAM];
    let cache = &accounts[ACTIVATION_CACHE];
    if cache.owner != registry.key {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    let (release_set_id, trading) = {
        let bytes = cache
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        let view = ActivatedExecutionReleaseSetViewV1::decode(&bytes)
            .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
        let release_set = view
            .execution_release_set_id()
            .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
        let role = view
            .role(ExecutionRoleV1::Trading)
            .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
        (release_set.to_bytes(), role)
    };
    require_pda(
        registry.key,
        cache,
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        GeneralSbfError::ReleaseAdmission,
    )?;
    let release = trading.release();
    if release.program().as_bytes() != &program_id.to_bytes()
        || release.programdata() != accounts[TRADING_PROGRAMDATA].key.to_bytes()
    {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAMDATA].key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Trading)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            accounts[TRADING_PROGRAM].clone(),
            accounts[TRADING_PROGRAMDATA].clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
    let (producer, bytes) = get_return_data().ok_or(GeneralSbfError::ReleaseAdmission)?;
    if producer != *registry.key {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    let receipt = AuthenticatedRoleReceiptV1::decode(&bytes)
        .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
    if receipt.role() != ExecutionRoleV1::Trading
        || receipt.execution_release_set_id().as_bytes() != &release_set_id
        || receipt.program().as_bytes() != &program_id.to_bytes()
        || receipt.artifact_release_id() != trading.artifact_release_id()
        || receipt.semantic_release_id() != release.semantic_release_id()
    {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    Ok(release_set_id)
}

#[inline(never)]
fn process_consider(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
) -> ProgramResult {
    if accounts.len() != CONSIDER_ACCOUNT_COUNT_V1 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[5];
    let verification = &accounts[6];
    let certificate = &accounts[7];
    let candidate_account = &accounts[8];
    let policy_account = &accounts[9];
    let page_account = &accounts[10];
    let incumbent_account = &accounts[11];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, true)?;
    require_owned_state(program_id, verification, VERIFICATION_CURSOR_BYTES_V1, true)?;
    require_owned_state(program_id, certificate, VERIFIED_CANDIDATE_BYTES_V1, true)?;
    require_owned_state(program_id, candidate_account, CANDIDATE_BYTES, false)?;
    require_owned_state(program_id, policy_account, SELECTION_POLICY_BYTES, false)?;
    require_owned_state(program_id, page_account, PAGE_BYTES, false)?;

    let candidate = decode_candidate(candidate_account)?;
    let policy = decode_policy(policy_account)?;
    let candidate_id = request.candidate_id.ok_or(GeneralSbfError::Instruction)?;
    if candidate.candidate_id != candidate_id {
        return Err(GeneralSbfError::ImmutableInput.into());
    }
    require_general_data_pdas(
        program_id,
        accounts[MARKET].key,
        candidate_account,
        policy_account,
        page_account,
        candidate,
        policy,
        request.page_index,
    )?;
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    require_pda(
        program_id,
        verification,
        &[
            GENERAL_VERIFICATION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Verification,
    )?;
    require_pda(
        program_id,
        certificate,
        &[
            GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Certificate,
    )?;

    let mut verifier = load_verifier(verification, candidate, request)?;
    ingest_verifier(&mut verifier, page_account, request.expected_revision)?;

    if verifier.is_complete() {
        let verified = finish_verifier(verifier)?;
        finalize_consider(
            program_id,
            accounts,
            selection,
            certificate,
            incumbent_account,
            candidate,
            policy,
            verified,
        )?;
    } else {
        require_zero_account(certificate, GeneralSbfError::Certificate)?;
    }
    store_verifier(verification, verifier)
}

#[inline(never)]
fn load_verifier(
    verification: &AccountInfo<'_>,
    candidate: CandidateV1,
    request: ControllerRequestV1,
) -> Result<CandidateVerifierV1, ProgramError> {
    let bytes = verification
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if bytes.iter().all(|byte| *byte == 0) {
        if request.page_index != 0 || request.expected_revision != 0 {
            return Err(GeneralSbfError::Verification.into());
        }
        return Ok(CandidateVerifierV1::begin(candidate));
    }
    let verifier =
        CandidateVerifierV1::decode(&bytes).map_err(|_| GeneralSbfError::Verification)?;
    if verifier.candidate() != candidate || verifier.next_page() != request.page_index {
        return Err(GeneralSbfError::Verification.into());
    }
    Ok(verifier)
}

#[inline(never)]
fn ingest_verifier(
    verifier: &mut CandidateVerifierV1,
    page_account: &AccountInfo<'_>,
    expected_revision: u64,
) -> ProgramResult {
    let page = page_account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    verifier
        .ingest_page_at(&page, expected_revision)
        .map_err(|_| GeneralSbfError::Verification.into())
}

#[inline(never)]
fn finish_verifier(verifier: CandidateVerifierV1) -> Result<VerifiedCandidateV1, ProgramError> {
    verifier
        .finish()
        .map_err(|_| GeneralSbfError::Verification.into())
}

#[inline(never)]
fn store_verifier(verification: &AccountInfo<'_>, verifier: CandidateVerifierV1) -> ProgramResult {
    let bytes = verifier
        .to_bytes()
        .map_err(|_| GeneralSbfError::Verification)?;
    verification
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finalize_consider(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selection: &AccountInfo<'_>,
    certificate: &AccountInfo<'_>,
    incumbent_account: &AccountInfo<'_>,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
    verified: VerifiedCandidateV1,
) -> ProgramResult {
    let mut selection_bytes = [0_u8; SELECTION_CURSOR_BYTES];
    {
        let source = selection
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        selection_bytes.copy_from_slice(&source);
    }
    let (selection_revision, best) = if selection_bytes.iter().all(|byte| *byte == 0) {
        (0, None)
    } else {
        let cursor =
            SelectionCursorV1::decode(&selection_bytes).map_err(|_| GeneralSbfError::Selection)?;
        (cursor.revision, cursor.best_candidate_id)
    };
    let incumbent = match best {
        None => {
            if incumbent_account.key != accounts[MARKET].key {
                return Err(GeneralSbfError::AccountFrame.into());
            }
            None
        }
        Some(best_id) => {
            require_owned_state(
                program_id,
                incumbent_account,
                VERIFIED_CANDIDATE_BYTES_V1,
                false,
            )?;
            require_pda(
                program_id,
                incumbent_account,
                &[
                    GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                    accounts[MARKET].key.as_ref(),
                    &best_id,
                ],
                GeneralSbfError::Certificate,
            )?;
            let bytes = incumbent_account
                .try_borrow_data()
                .map_err(|_| GeneralSbfError::Borrow)?;
            Some(VerifiedCandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::Certificate)?)
        }
    };
    let mut certificate_bytes = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
    {
        let source = certificate
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        certificate_bytes.copy_from_slice(&source);
    }
    consider_verified(
        &mut selection_bytes,
        &mut certificate_bytes,
        &candidate,
        &policy,
        verified,
        incumbent.as_ref(),
        selection_revision,
    )
    .map_err(|_| GeneralSbfError::Selection)?;
    selection
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&selection_bytes);
    certificate
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&certificate_bytes);
    Ok(())
}

#[inline(never)]
fn process_freeze(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
) -> ProgramResult {
    if accounts.len() != FREEZE_ACCOUNT_COUNT_V1 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[5];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, true)?;
    let mut bytes = [0_u8; SELECTION_CURSOR_BYTES];
    {
        let source = selection
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        bytes.copy_from_slice(&source);
    }
    let cursor = SelectionCursorV1::decode(&bytes).map_err(|_| GeneralSbfError::Selection)?;
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &cursor.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    freeze_selection(&mut bytes, request.expected_revision)
        .map_err(|_| GeneralSbfError::Selection)?;
    selection
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&bytes);
    Ok(())
}

#[inline(never)]
fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ControllerRequestV1,
) -> ProgramResult {
    if accounts.len() != INITIALIZE_ACCOUNT_COUNT_V1 {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let selection = &accounts[5];
    let settlement = &accounts[6];
    let certificate = &accounts[7];
    let candidate_account = &accounts[8];
    require_owned_state(program_id, selection, SELECTION_CURSOR_BYTES, false)?;
    require_owned_state(program_id, settlement, SETTLEMENT_CURSOR_BYTES, true)?;
    require_owned_state(program_id, certificate, VERIFIED_CANDIDATE_BYTES_V1, false)?;
    require_owned_state(program_id, candidate_account, CANDIDATE_BYTES, false)?;
    let candidate = decode_candidate(candidate_account)?;
    if request.candidate_id != Some(candidate.candidate_id) {
        return Err(GeneralSbfError::Instruction.into());
    }
    require_pda(
        program_id,
        selection,
        &[
            GENERAL_SELECTION_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.batch_id,
        ],
        GeneralSbfError::Selection,
    )?;
    require_pda(
        program_id,
        certificate,
        &[
            GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Certificate,
    )?;
    require_pda(
        program_id,
        settlement,
        &[
            GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
            accounts[MARKET].key.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::Settlement,
    )?;
    let verified = {
        let bytes = certificate
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        VerifiedCandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::Certificate)?
    };
    if verified.candidate_id != candidate.candidate_id {
        return Err(GeneralSbfError::Certificate.into());
    }
    let selection_bytes = selection
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    let mut settlement_bytes = [0_u8; SETTLEMENT_CURSOR_BYTES];
    {
        let source = settlement
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        settlement_bytes.copy_from_slice(&source);
    }
    initialize_settlement(
        &mut settlement_bytes,
        &selection_bytes,
        &verified,
        request.expected_revision,
    )
    .map_err(|_| GeneralSbfError::Settlement)?;
    drop(selection_bytes);
    settlement
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&settlement_bytes);
    Ok(())
}

fn decode_candidate(account: &AccountInfo<'_>) -> Result<CandidateV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    CandidateV1::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput.into())
}

fn decode_policy(account: &AccountInfo<'_>) -> Result<SelectionPolicyV1, ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    SelectionPolicyV1::decode(&bytes).map_err(|_| GeneralSbfError::ImmutableInput.into())
}

#[allow(clippy::too_many_arguments)]
fn require_general_data_pdas(
    program_id: &Pubkey,
    market: &Pubkey,
    candidate_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    page_account: &AccountInfo<'_>,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
    page_index: u32,
) -> ProgramResult {
    let page_bytes = page_index.to_le_bytes();
    require_pda(
        program_id,
        candidate_account,
        &[
            GENERAL_CANDIDATE_PDA_DOMAIN_V1,
            market.as_ref(),
            &candidate.candidate_id,
        ],
        GeneralSbfError::ImmutableInput,
    )?;
    require_pda(
        program_id,
        policy_account,
        &[
            GENERAL_POLICY_PDA_DOMAIN_V1,
            market.as_ref(),
            &policy.policy_id,
        ],
        GeneralSbfError::ImmutableInput,
    )?;
    require_pda(
        program_id,
        page_account,
        &[
            GENERAL_PAGE_PDA_DOMAIN_V1,
            market.as_ref(),
            &candidate.candidate_id,
            &page_bytes,
        ],
        GeneralSbfError::ImmutableInput,
    )
}

fn require_owned_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    width: usize,
    writable: bool,
) -> ProgramResult {
    if account.owner != program_id
        || account.data_len() != width
        || account.is_writable != writable
        || account.is_signer
        || account.executable
    {
        Err(GeneralSbfError::AccountFrame.into())
    } else {
        Ok(())
    }
}

fn require_zero_account(account: &AccountInfo<'_>, error: GeneralSbfError) -> ProgramResult {
    let data = account
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if data.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(error.into())
    }
}

fn require_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    seeds: &[&[u8]],
    error: GeneralSbfError,
) -> ProgramResult {
    let (expected, _) = Pubkey::find_program_address(seeds, program_id);
    if account.key == &expected {
        Ok(())
    } else {
        Err(error.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_general_codec::{
        ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES, MAX_SELECTION_CRITERIA, PageV1, Phase,
        SelectionCriterion, SettlementCursorV1,
    };
    use std::{boxed::Box, vec};

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        values[0] = first;
        values[1] = second;
        values
    }

    fn account(
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
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
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
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: id(51),
            criterion_count: 3,
            criteria,
        }
    }

    fn page() -> [u8; PAGE_BYTES] {
        let mut rows = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        rows[0] = ExecutionV1 {
            order_id: id(1),
            owner_id: id(11),
            nonce: 1,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(1, 0),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
        rows[1] = ExecutionV1 {
            order_id: id(2),
            owner_id: id(12),
            nonce: 1,
            max_lots: 1,
            max_quote_debit_per_lot: 1,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(0, 1),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
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

    fn common(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        vec![
            account(market, false, Vec::new(), Pubkey::new_unique(), false),
            account(
                Pubkey::new_unique(),
                false,
                Vec::new(),
                Pubkey::new_unique(),
                false,
            ),
            account(
                Pubkey::new_unique(),
                false,
                Vec::new(),
                Pubkey::new_unique(),
                true,
            ),
            account(program_id, false, Vec::new(), Pubkey::new_unique(), true),
            account(
                Pubkey::new_unique(),
                false,
                Vec::new(),
                Pubkey::new_unique(),
                false,
            ),
        ]
    }

    fn consider_frame(program_id: Pubkey, market: Pubkey) -> Vec<AccountInfo<'static>> {
        let candidate = candidate();
        let policy = policy();
        let mut frame = common(program_id, market);
        let selection = Pubkey::find_program_address(
            &[
                GENERAL_SELECTION_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.batch_id,
            ],
            &program_id,
        )
        .0;
        let verification = Pubkey::find_program_address(
            &[
                GENERAL_VERIFICATION_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let certificate = Pubkey::find_program_address(
            &[
                GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let candidate_key = Pubkey::find_program_address(
            &[
                GENERAL_CANDIDATE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let policy_key = Pubkey::find_program_address(
            &[
                GENERAL_POLICY_PDA_DOMAIN_V1,
                market.as_ref(),
                &policy.policy_id,
            ],
            &program_id,
        )
        .0;
        let page_key = Pubkey::find_program_address(
            &[
                GENERAL_PAGE_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
                &0_u32.to_le_bytes(),
            ],
            &program_id,
        )
        .0;
        frame.extend([
            account(
                selection,
                true,
                vec![0; SELECTION_CURSOR_BYTES],
                program_id,
                false,
            ),
            account(
                verification,
                true,
                vec![0; VERIFICATION_CURSOR_BYTES_V1],
                program_id,
                false,
            ),
            account(
                certificate,
                true,
                vec![0; VERIFIED_CANDIDATE_BYTES_V1],
                program_id,
                false,
            ),
            account(
                candidate_key,
                false,
                candidate.to_bytes().expect("candidate").to_vec(),
                program_id,
                false,
            ),
            account(
                policy_key,
                false,
                policy.to_bytes().expect("policy").to_vec(),
                program_id,
                false,
            ),
            account(page_key, false, page().to_vec(), program_id, false),
            frame[MARKET].clone(),
        ]);
        frame
    }

    fn consider_request() -> ControllerRequestV1 {
        ControllerRequestV1 {
            action: Action::Consider,
            expected_revision: 0,
            candidate_id: Some(candidate().candidate_id),
            page_index: 0,
            execution_index: 0,
        }
    }

    #[test]
    fn authenticated_consider_streams_and_commits_exact_certificate() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let frame = consider_frame(program_id, market);
        process_authenticated(&program_id, &frame, consider_request(), id(90)).expect("consider");
        let selection =
            SelectionCursorV1::decode(&frame[5].try_borrow_data().expect("selection borrow"))
                .expect("selection");
        assert_eq!(selection.best_candidate_id, Some(candidate().candidate_id));
        assert_eq!(selection.revision, 1);
        let certificate =
            VerifiedCandidateV1::decode(&frame[7].try_borrow_data().expect("certificate borrow"))
                .expect("certificate");
        assert_eq!(certificate.complete_set_quantity, 1);
        assert_eq!(certificate.quote_surplus, 1);
        let verifier =
            CandidateVerifierV1::decode(&frame[6].try_borrow_data().expect("verification borrow"))
                .expect("verification");
        assert!(verifier.is_complete());
    }

    #[test]
    fn hostile_page_and_stale_replay_preserve_all_general_state() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let frame = consider_frame(program_id, market);
        let selection_before = frame[5].try_borrow_data().expect("selection").to_vec();
        let verification_before = frame[6].try_borrow_data().expect("verification").to_vec();
        let certificate_before = frame[7].try_borrow_data().expect("certificate").to_vec();
        frame[10].try_borrow_mut_data().expect("page")[16] ^= 1;
        assert_eq!(
            process_authenticated(&program_id, &frame, consider_request(), id(90)),
            Err(GeneralSbfError::Verification.into())
        );
        assert_eq!(
            frame[5].try_borrow_data().expect("selection").as_ref(),
            selection_before.as_slice()
        );
        assert_eq!(
            frame[6].try_borrow_data().expect("verification").as_ref(),
            verification_before.as_slice()
        );
        assert_eq!(
            frame[7].try_borrow_data().expect("certificate").as_ref(),
            certificate_before.as_slice()
        );

        frame[10].try_borrow_mut_data().expect("page")[16] ^= 1;
        process_authenticated(&program_id, &frame, consider_request(), id(90))
            .expect("first consider");
        let snapshot = frame[5].try_borrow_data().expect("selection").to_vec();
        assert_eq!(
            process_authenticated(&program_id, &frame, consider_request(), id(90)),
            Err(GeneralSbfError::Verification.into())
        );
        assert_eq!(
            frame[5].try_borrow_data().expect("selection").as_ref(),
            snapshot.as_slice()
        );
    }

    #[test]
    fn freeze_then_initialize_enters_zero_inventory_collecting_phase() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let consider = consider_frame(program_id, market);
        process_authenticated(&program_id, &consider, consider_request(), id(90))
            .expect("consider");

        let selection_bytes = consider[5].try_borrow_data().expect("selection").to_vec();
        let selection_key = *consider[5].key;
        let mut freeze = common(program_id, market);
        freeze.push(account(
            selection_key,
            true,
            selection_bytes,
            program_id,
            false,
        ));
        let freeze_request = ControllerRequestV1 {
            action: Action::Freeze,
            expected_revision: 1,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
        };
        process_authenticated(&program_id, &freeze, freeze_request, id(90)).expect("freeze");

        let candidate = candidate();
        let settlement_key = Pubkey::find_program_address(
            &[
                GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
                market.as_ref(),
                &candidate.candidate_id,
            ],
            &program_id,
        )
        .0;
        let mut initialize = common(program_id, market);
        initialize.extend([
            account(
                selection_key,
                false,
                freeze[5].try_borrow_data().expect("selection").to_vec(),
                program_id,
                false,
            ),
            account(
                settlement_key,
                true,
                vec![0; SETTLEMENT_CURSOR_BYTES],
                program_id,
                false,
            ),
            account(
                *consider[7].key,
                false,
                consider[7].try_borrow_data().expect("certificate").to_vec(),
                program_id,
                false,
            ),
            account(
                *consider[8].key,
                false,
                consider[8].try_borrow_data().expect("candidate").to_vec(),
                program_id,
                false,
            ),
        ]);
        let request = ControllerRequestV1 {
            action: Action::InitializeSettlement,
            expected_revision: 0,
            candidate_id: Some(candidate.candidate_id),
            page_index: 0,
            execution_index: 0,
        };
        process_authenticated(&program_id, &initialize, request, id(90)).expect("initialize");
        let settlement =
            SettlementCursorV1::decode(&initialize[6].try_borrow_data().expect("settlement"))
                .expect("settlement cursor");
        assert_eq!(settlement.phase, Phase::Collecting);
        assert_eq!(settlement.next_page, 0);
        assert_eq!(settlement.next_execution, 0);
        assert_eq!(settlement.claim_inventory, [0; MAX_OUTCOMES]);
        assert_eq!(settlement.quote_inventory, 0);
    }
}
