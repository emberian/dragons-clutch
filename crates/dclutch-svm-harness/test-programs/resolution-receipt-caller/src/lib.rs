#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for funded Resolution receipt and rollback evidence.
//!
//! The caller owns no protocol state or semantic authority. It forwards the
//! exact production funded request, immediately authenticates the return-data
//! producer and every caller-observable receipt join, then optionally refuses
//! to prove whole-transaction rollback after the production child committed.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_capability_contract::FundingStateV1;
use dclutch_resolution_codec::{
    FUNDED_POSTSTATE_DIGEST_DOMAIN_V1, FUNDED_TRANSITION_RECEIPT_BYTES,
    FUNDED_TRANSITION_REQUEST_BYTES, FundedReceiptPostPhaseV1, FundedTerminalRefundPhaseV1,
    FundedTransitionActionV3, FundedTransitionReceiptV1, FundedTransitionRequestV3,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
    ResolutionCertificateKindV1, ResolutionCertificateV1,
};
use dclutch_source_contract::{SourceResolutionPhaseV1, SourceResolutionStateV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact wrapper wire: fail-after-validation flag plus production funded request.
pub const TEST_FUNDED_WIRE_BYTES_V1: usize = 1 + FUNDED_TRANSITION_REQUEST_BYTES;
/// Exact forwarded production Resolution account count.
pub const TEST_FUNDED_ACCOUNT_COUNT_V1: usize = 19;

/// Stable test-only caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TestReceiptCallerError {
    /// Wrapper bytes or fail flag were malformed.
    Instruction = 0x10_A000,
    /// Resolution program or forwarded account frame was malformed.
    AccountFrame = 0x10_A001,
    /// Production Resolution CPI refused.
    ResolutionCpi = 0x10_A002,
    /// Return data was missing, malformed, or produced by another program.
    ReturnData = 0x10_A003,
    /// Receipt did not bind the exact request, accounts, or poststate.
    ReceiptMismatch = 0x10_A004,
    /// Deliberate refusal after complete receipt validation.
    DeliberateLateFailure = 0x10_A005,
    /// Checked payout arithmetic failed.
    Arithmetic = 0x10_A006,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    TestReceiptCallerError::Instruction as u32
        == dclutch_refusal_registry::TEST_RESOLUTION_RECEIPT_CALLER_BASE,
    "TestReceiptCallerError must start at its registered refusal band base"
);
const _: () = assert!(
    (TestReceiptCallerError::Arithmetic as u32)
        < dclutch_refusal_registry::TEST_RESOLUTION_RECEIPT_CALLER_BASE
            + dclutch_refusal_registry::BAND_SPAN,
    "TestReceiptCallerError must not run past its registered refusal band"
);

impl From<TestReceiptCallerError> for ProgramError {
    fn from(value: TestReceiptCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one production funded transition and authenticate its immediate receipt.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != TEST_FUNDED_WIRE_BYTES_V1
        || accounts.len() != TEST_FUNDED_ACCOUNT_COUNT_V1 + 1
    {
        return Err(TestReceiptCallerError::Instruction.into());
    }
    let fail_after = *instruction_data
        .first()
        .ok_or(TestReceiptCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(TestReceiptCallerError::Instruction.into());
    }
    let resolution_program = accounts
        .first()
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    if !resolution_program.executable
        || resolution_program.is_signer
        || resolution_program.is_writable
        || forwarded
            .get(8)
            .ok_or(TestReceiptCallerError::AccountFrame)?
            .key
            != resolution_program.key
    {
        return Err(TestReceiptCallerError::AccountFrame.into());
    }
    let request_bytes = instruction_data
        .get(1..)
        .ok_or(TestReceiptCallerError::Instruction)?;
    let request = FundedTransitionRequestV3::decode(request_bytes)
        .map_err(|_| TestReceiptCallerError::Instruction)?;
    let source = forwarded
        .first()
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    let certificate = forwarded
        .get(1)
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    let funding = forwarded
        .get(2)
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    let worker = forwarded
        .get(3)
        .ok_or(TestReceiptCallerError::AccountFrame)?;
    let pre_source_digest = {
        let data = source
            .try_borrow_data()
            .map_err(|_| TestReceiptCallerError::AccountFrame)?;
        hash(&data).to_bytes()
    };
    let funding_lamports_before = funding.lamports();
    let worker_lamports_before = worker.lamports();

    let mut metas = Vec::with_capacity(forwarded.len());
    for account in forwarded {
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, account.is_signer)
        } else {
            AccountMeta::new_readonly(*account.key, account.is_signer)
        });
    }
    let instruction = Instruction {
        program_id: *resolution_program.key,
        accounts: metas,
        data: request_bytes.into(),
    };
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(resolution_program.clone());
    invoke(&instruction, &infos).map_err(|_| TestReceiptCallerError::ResolutionCpi)?;

    let (producer, receipt_bytes) = get_return_data().ok_or(TestReceiptCallerError::ReturnData)?;
    if producer != *resolution_program.key || receipt_bytes.len() != FUNDED_TRANSITION_RECEIPT_BYTES
    {
        return Err(TestReceiptCallerError::ReturnData.into());
    }
    let receipt = FundedTransitionReceiptV1::decode(&receipt_bytes)
        .map_err(|_| TestReceiptCallerError::ReturnData)?;
    authenticate_receipt(
        resolution_program,
        source,
        certificate,
        funding,
        worker,
        request,
        request_bytes,
        pre_source_digest,
        funding_lamports_before,
        worker_lamports_before,
        receipt,
    )?;
    if fail_after == 1 {
        return Err(TestReceiptCallerError::DeliberateLateFailure.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_receipt(
    resolution_program: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    certificate: &AccountInfo<'_>,
    funding: &AccountInfo<'_>,
    worker: &AccountInfo<'_>,
    request: FundedTransitionRequestV3,
    request_bytes: &[u8],
    pre_source_digest: [u8; 32],
    funding_lamports_before: u64,
    worker_lamports_before: u64,
    receipt: FundedTransitionReceiptV1,
) -> ProgramResult {
    let source_data = source
        .try_borrow_data()
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let source_state = SourceResolutionStateV1::decode(&source_data)
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let certificate_data = certificate
        .try_borrow_data()
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let certificate_value = ResolutionCertificateV1::decode(&certificate_data)
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let funding_data = funding
        .try_borrow_data()
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let funding_value = FundingStateV1::decode(&funding_data)
        .map_err(|_| TestReceiptCallerError::ReceiptMismatch)?;
    let funding_lamports_after = funding.lamports();
    let worker_lamports_after = worker.lamports();
    let work_from_funding = funding_lamports_before
        .checked_sub(funding_lamports_after)
        .ok_or(TestReceiptCallerError::Arithmetic)?;
    let work_to_worker = worker_lamports_after
        .checked_sub(worker_lamports_before)
        .ok_or(TestReceiptCallerError::Arithmetic)?;
    let funding_lamports_after_bytes = funding_lamports_after.to_le_bytes();
    let expected_funding_digest = hashv(&[
        FUNDED_POSTSTATE_DIGEST_DOMAIN_V1,
        &funding_data,
        &funding_lamports_after_bytes,
    ])
    .to_bytes();
    let certificate_tag = match request.action {
        FundedTransitionActionV3::FailNext => 2_u8,
        FundedTransitionActionV3::Exhaust => 3_u8,
        FundedTransitionActionV3::CommitFailure => 4_u8,
    };
    let sequence = receipt.replay_sequence.to_le_bytes();
    let expected_certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.key.as_ref(),
            &[certificate_tag],
            &sequence,
        ],
        resolution_program.key,
    )
    .0;
    let partition_matches = match request.action {
        FundedTransitionActionV3::FailNext => {
            source_state.phase() == SourceResolutionPhaseV1::Recovery
                && receipt.post_phase == FundedReceiptPostPhaseV1::Recovery
                && receipt.certificate_kind == ResolutionCertificateKindV1::RecoveryAdvanced
                && receipt.terminal_refund_phase == FundedTerminalRefundPhaseV1::Continuing
        }
        FundedTransitionActionV3::Exhaust => {
            source_state.phase() == SourceResolutionPhaseV1::Exhausted
                && receipt.post_phase == FundedReceiptPostPhaseV1::Exhausted
                && receipt.certificate_kind == ResolutionCertificateKindV1::Exhausted
                && receipt.terminal_refund_phase == FundedTerminalRefundPhaseV1::AwaitingFailure
        }
        FundedTransitionActionV3::CommitFailure => {
            source_state.phase() == SourceResolutionPhaseV1::FailureCommitted
                && receipt.post_phase == FundedReceiptPostPhaseV1::FailureCommitted
                && receipt.certificate_kind == ResolutionCertificateKindV1::ResolutionFailure
                && receipt.terminal_refund_phase
                    == FundedTerminalRefundPhaseV1::TerminalRefundPending
        }
    };
    if !partition_matches
        || receipt.action != request.action
        || receipt.producer_program != resolution_program.key.to_bytes()
        || receipt.producer_release != RESOLUTION_CONTROLLER_RELEASE_ID_V4
        || receipt.request_digest != hash(request_bytes).to_bytes()
        || receipt.source_state != source.key.to_bytes()
        || receipt.funding_state != funding.key.to_bytes()
        || receipt.worker != worker.key.to_bytes()
        || receipt.certificate != certificate.key.to_bytes()
        || receipt.pre_source_digest != pre_source_digest
        || receipt.post_source_digest != hash(&source_data).to_bytes()
        || receipt.funding_post_digest != expected_funding_digest
        || receipt.generation != request.expected_generation
        || receipt.work_paid != work_from_funding
        || receipt.work_paid != work_to_worker
        || receipt.funding_remaining != funding_value.remaining().bounty().amount()
        || expected_certificate != *certificate.key
        || certificate_value.kind != receipt.certificate_kind
        || certificate_value.receipt_account != receipt.certificate
        || certificate_value.generation != receipt.generation
        || certificate_value.work_paid != receipt.work_paid
        || certificate_value.funding_remaining != receipt.funding_remaining
        || certificate_value.selector != receipt.selector
    {
        return Err(TestReceiptCallerError::ReceiptMismatch.into());
    }
    Ok(())
}
