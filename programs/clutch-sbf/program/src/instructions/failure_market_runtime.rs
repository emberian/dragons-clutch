// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled market-scoped Failure runtime account seam.
//!
//! The immutable `0xa0/v2` admission root and mutable `0xa0/v3` runtime root
//! are distinct accounts and semantic owners. This module owns hostile runtime
//! decoding, the existing market/generation PDA, canonical Rent, prefunded
//! allocation, and exact first write. It neither routes an instruction nor
//! accepts a caller-built Product foundation DTO.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1;
use clutch_failure_policy_runtime::market_runtime_v1::{
    admit_failure_market_runtime_v1, AuthenticatedFailureMarketRuntimeAdmissionV1,
    FailureMarketRuntimeAdmissionReceiptV1, FailureMarketRuntimeRootFundingFactsV1,
    FailureMarketRuntimeStateCommitmentV1, FailureMarketRuntimeV1, FAILURE_MARKET_RUNTIME_BYTES_V1,
};
use clutch_product_series::ContentId as ProductContentId;
use clutch_solana_layout::failure_recovery::{
    FailureMarketRuntimeRootAccountV1, FAILURE_MARKET_RUNTIME_BODY_BYTES_V1,
    FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const _: () = assert!(FAILURE_MARKET_RUNTIME_BODY_BYTES_V1 == FAILURE_MARKET_RUNTIME_BYTES_V1);

/// Exact authenticated mutable market-scoped Failure runtime root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFailureMarketRuntimeRootV1 {
    account: Pubkey,
    bump: u8,
    state: FailureMarketRuntimeV1,
    state_commitment: FailureMarketRuntimeStateCommitmentV1,
}

impl AuthenticatedFailureMarketRuntimeRootV1 {
    /// Exact physical runtime root.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Stored canonical PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }

    /// Complete authenticated semantic state.
    pub const fn state(self) -> FailureMarketRuntimeV1 {
        self.state
    }

    /// Commitment to the complete canonical semantic body.
    pub const fn state_commitment(self) -> FailureMarketRuntimeStateCommitmentV1 {
        self.state_commitment
    }
}

/// Atomic postimage of one Product-authorized runtime foundation step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimePostimageV1 {
    root: AuthenticatedFailureMarketRuntimeRootV1,
    admission_receipt: FailureMarketRuntimeAdmissionReceiptV1,
}

impl FailureMarketRuntimePostimageV1 {
    /// Newly persisted mutable runtime root.
    pub const fn root(self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.root
    }

    /// Exact semantic admission receipt consumed by the Product lifecycle.
    pub const fn admission_receipt(self) -> FailureMarketRuntimeAdmissionReceiptV1 {
        self.admission_receipt
    }
}

/// Authenticate an existing `0xa0/v3` runtime against immutable `0xa0/v2`.
pub fn authenticate_failure_market_runtime_root_v1<'a>(
    program_id: &Pubkey,
    runtime_root: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    writable: bool,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    require(
        *runtime_root.key != admission_root.account(),
        ClutchError::AccountAlias,
    )?;
    require(
        runtime_root.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!runtime_root.is_signer, ClutchError::NonCanonical)?;
    require(!runtime_root.executable, ClutchError::ExecutableAccount)?;
    require(
        runtime_root.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    let data = runtime_root
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let input: &[u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let record = FailureMarketRuntimeRootAccountV1::decode(input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let state =
        FailureMarketRuntimeV1::decode_for_admission(&record.runtime_body, admission_root.state())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy = admission_root.state().binding().facts();
    let funding = state.root_funding();
    require(
        state.runtime_account_id().bytes() == runtime_root.key.to_bytes()
            && policy.recovery_state_id.bytes() == runtime_root.key.to_bytes()
            && runtime_root.lamports() >= funding.observed_balance_lamports
            && funding.rent_refund_owner.bytes() != runtime_root.key.to_bytes()
            && funding.neutral_sink.bytes() != runtime_root.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        runtime_root.key,
        seeds::failure_external_root_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(record.bump),
    )?;
    let state_commitment = state
        .commitment()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(AuthenticatedFailureMarketRuntimeRootV1 {
        account: *runtime_root.key,
        bump: record.bump,
        state,
        state_commitment,
    })
}

/// Execute a complete non-routable runtime foundation step.
///
/// The authority must be a Product-private accepted foundation-step receipt.
/// Its default-refusing pure trait binds the slot-6 account, graph, principal,
/// prior donation, refund owner, neutral sink, and resulting state commitment.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_failure_market_runtime_v1<'a, A>(
    program_id: &Pubkey,
    runtime_root: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    product_foundation_authority: &A,
    foundation_receipt_id: ProductContentId,
    root_funding: FailureMarketRuntimeRootFundingFactsV1,
) -> Outcome<FailureMarketRuntimePostimageV1>
where
    A: AuthenticatedFailureMarketRuntimeAdmissionV1 + ?Sized,
{
    require(
        *runtime_root.key != admission_root.account(),
        ClutchError::AccountAlias,
    )?;
    let runtime_account_id = FailureMarketAccountIdV1::from_bytes(runtime_root.key.to_bytes());
    let (state, admission_receipt) = admit_failure_market_runtime_v1(
        product_foundation_authority,
        admission_root.state(),
        runtime_account_id,
        foundation_receipt_id,
        root_funding,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root = initialize_prefunded_failure_market_runtime_root_v1(
        program_id,
        runtime_root,
        rent_sysvar,
        system_program,
        admission_root,
        state,
    )?;
    Ok(FailureMarketRuntimePostimageV1 {
        root,
        admission_receipt,
    })
}

fn initialize_prefunded_failure_market_runtime_root_v1<'a>(
    program_id: &Pubkey,
    runtime_root: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    state: FailureMarketRuntimeV1,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    require_system_program(system_program)?;
    require_distinct(&[
        runtime_root.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    require(
        *runtime_root.key != admission_root.account(),
        ClutchError::AccountAlias,
    )?;
    let rent = read_rent(rent_sysvar)?;
    let policy = admission_root.state().binding().facts();
    let funding = state.root_funding();
    let expected_balance = funding
        .rent_principal_lamports
        .checked_add(funding.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        runtime_root.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && runtime_root.is_writable
            && !runtime_root.is_signer
            && !runtime_root.executable
            && runtime_root.data_len() == 0
            && runtime_root.lamports() == expected_balance
            && funding.observed_balance_lamports == expected_balance
            && funding.rent_principal_lamports
                == rent.minimum_balance(FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1)?
            && state.runtime_account_id().bytes() == runtime_root.key.to_bytes()
            && policy.recovery_state_id.bytes() == runtime_root.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (expected_root, bump) = seeds::failure_external_root_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(runtime_root.key, (expected_root, bump), None)?;
    let market_instance = policy.market_instance_id.bytes();
    let generation = policy.generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_FAILURE_EXTERNAL_ROOT,
        &market_instance,
        &generation,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1),
        vec![AccountMeta::new(*runtime_root.key, true)],
    );
    invoke_signed(
        &allocate,
        &[runtime_root.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*runtime_root.key, true)],
    );
    invoke_signed(
        &assign,
        &[runtime_root.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        runtime_root.owner == program_id
            && runtime_root.data_len() == FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1
            && runtime_root.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    persist_failure_market_runtime_root_v1(program_id, runtime_root, admission_root, state)
}

fn persist_failure_market_runtime_root_v1(
    program_id: &Pubkey,
    runtime_root: &AccountInfo<'_>,
    admission_root: AuthenticatedFailureMarketRootV2,
    state: FailureMarketRuntimeV1,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    let policy = admission_root.state().binding().facts();
    let funding = state.root_funding();
    let expected_balance = funding
        .rent_principal_lamports
        .checked_add(funding.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        runtime_root.owner == program_id
            && runtime_root.is_writable
            && !runtime_root.is_signer
            && !runtime_root.executable
            && runtime_root.data_len() == FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1
            && runtime_root.lamports() == expected_balance
            && funding.observed_balance_lamports == expected_balance
            && state.runtime_account_id().bytes() == runtime_root.key.to_bytes()
            && policy.recovery_state_id.bytes() == runtime_root.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (expected_root, bump) = seeds::failure_external_root_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(runtime_root.key, (expected_root, bump), None)?;
    let mut runtime_body = [0u8; FAILURE_MARKET_RUNTIME_BYTES_V1];
    state
        .encode_into(&mut runtime_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let record = FailureMarketRuntimeRootAccountV1 { bump, runtime_body };
    let mut data = runtime_root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    let output: &mut [u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = data
        .as_mut()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    record
        .encode_into(output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let state_commitment = state
        .commitment()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(AuthenticatedFailureMarketRuntimeRootV1 {
        account: *runtime_root.key,
        bump,
        state,
        state_commitment,
    })
}
