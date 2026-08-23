// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled shared-Market Failure admission account seam.
//!
//! This module authenticates the complete persisted liveness policy and sole
//! Recovery body, derives the exact pure funding facts, and owns the versioned
//! `0xa0/v2` policy/funding record. It does not route an instruction. Product's
//! forthcoming private FoundationVault receipt must mint the pure root-funding
//! receipt joined into [`FailureMarketAdmissionStateV1`]; until that join
//! lands, no routed instruction can construct a writable postimage.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_policy_v1::{
    admit_failure_market_recovery_funding_v1, project_initial_market_recovery_funding_v1,
    AuthenticatedFailureMarketRecoveryFundingV1, FailureMarketAdmissionStateV1,
    FailureMarketPolicyBindingV1, FailureMarketPrepaidDebitReceiptIdV1,
    FailureMarketRecoveryFundingFactsV1, FailureMarketRecoveryFundingReceiptV1,
    FAILURE_MARKET_ADMISSION_STATE_BYTES_V1,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::{RuntimeCompartmentKindV1, RuntimeCompartmentV1};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FailureMarketRootAccountV2,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    FAILURE_MARKET_ADMISSION_BODY_BYTES_V1, FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const _: () =
    assert!(FAILURE_MARKET_ADMISSION_BODY_BYTES_V1 == FAILURE_MARKET_ADMISSION_STATE_BYTES_V1);

/// Private full-body authentication of one initial Recovery custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFailureMarketLivenessV1 {
    facts: FailureMarketRecoveryFundingFactsV1,
}

impl AuthenticatedFailureMarketLivenessV1 {
    /// Exact facts derived from both persisted liveness bodies.
    pub const fn facts(self) -> FailureMarketRecoveryFundingFactsV1 {
        self.facts
    }
}

impl AuthenticatedFailureMarketRecoveryFundingV1 for AuthenticatedFailureMarketLivenessV1 {
    fn authenticate_failure_market_recovery_funding(
        &self,
        expected: FailureMarketRecoveryFundingFactsV1,
    ) -> core::result::Result<(), clutch_failure_policy_runtime::Error> {
        if expected == self.facts {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

/// Existing authenticated `0xa0/v2` shared-Market root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFailureMarketRootV2 {
    account: Pubkey,
    bump: u8,
    state: FailureMarketAdmissionStateV1,
}

impl AuthenticatedFailureMarketRootV2 {
    /// Exact physical root account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Canonical stored PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }

    /// Complete policy and initial funding semantic state.
    pub const fn state(self) -> FailureMarketAdmissionStateV1 {
        self.state
    }
}

/// Authenticate full initial liveness bodies and admit their funding receipt.
///
/// This function is crate-private because the prepaid debit ID is not itself
/// authority. The future Product adapter must call it only after authenticating
/// the private FoundationVault debit receipt in the same instruction.
pub(crate) fn authenticate_initial_market_recovery_funding_v1(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    binding: FailureMarketPolicyBindingV1,
    prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1,
) -> Outcome<(
    AuthenticatedFailureMarketLivenessV1,
    FailureMarketRecoveryFundingReceiptV1,
)> {
    require_distinct(&[policy_account.clone(), recovery_account.clone()])?;
    for account in [policy_account, recovery_account] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        require(!account.is_signer, ClutchError::NonCanonical)?;
        require(!account.executable, ClutchError::ExecutableAccount)?;
    }
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_frame = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(policy_account.key),
            owner_program_id: liveness_id(policy_account.owner),
            lamports: policy_account.lamports(),
            data: policy_frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        recovery.kind == RuntimeCompartmentKindV1::Recovery
            && recovery.identity.account_id == liveness_id(recovery_account.key)
            && recovery.identity.owner == liveness_id(program_id)
            && binding.facts().recovery_receipt_program_id == liveness_id(program_id),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(policy_frame.stored_bump),
    )?;
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &recovery.identity.lifecycle_id.bytes(),
            recovery.identity.generation,
        ),
        Some(recovery_frame.stored_bump),
    )?;
    let facts = project_initial_market_recovery_funding_v1(
        binding,
        prepaid_debit_receipt_id,
        policy,
        recovery,
        recovery_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authority = AuthenticatedFailureMarketLivenessV1 { facts };
    let receipt = admit_failure_market_recovery_funding_v1(&authority, binding, facts)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((authority, receipt))
}

/// Persist a fresh shared-Market admission after Product funding authority.
///
/// Allocation/assignment and the Product FoundationVault debit occur outside
/// this helper but in the same atomic instruction. This function verifies the
/// exact authenticated postfund balance before the first and only data write.
pub fn persist_failure_market_root_v2(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    state: FailureMarketAdmissionStateV1,
) -> Outcome<AuthenticatedFailureMarketRootV2> {
    let policy = state.binding().facts();
    let funding = state.root_funding().facts();
    let expected_balance = funding
        .rent_principal_lamports
        .checked_add(funding.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        root.owner == program_id
            && root.is_writable
            && !root.is_signer
            && !root.executable
            && root.data_len() == FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    require(
        funding.root_account_id.bytes() == root.key.to_bytes()
            && funding.rent_principal_lamports != 0
            && funding.observed_balance_lamports == expected_balance
            && root.lamports() == expected_balance
            && policy.recovery_state_id.bytes() == root.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (expected_root, bump) = seeds::failure_market_root_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(root.key, (expected_root, bump), None)?;
    let mut admission_body = [0u8; FAILURE_MARKET_ADMISSION_STATE_BYTES_V1];
    state
        .encode_into(&mut admission_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let record = FailureMarketRootAccountV2 {
        bump,
        admission_body,
    };
    let mut data = root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    let output: &mut [u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2] = data
        .as_mut()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    record
        .encode_into(output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(AuthenticatedFailureMarketRootV2 {
        account: *root.key,
        bump,
        state,
    })
}

/// Allocate, assign, and persist one exactly prefunded shared-Market root.
///
/// Product must have applied the authenticated FoundationVault debit first in
/// the same outer instruction. This helper performs no transfer and therefore
/// cannot source rent from a signer, Recovery custody, Hoard, or future fees.
pub fn initialize_prefunded_failure_market_root_v2<'a>(
    program_id: &Pubkey,
    root: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    state: FailureMarketAdmissionStateV1,
) -> Outcome<AuthenticatedFailureMarketRootV2> {
    require_system_program(system_program)?;
    require_distinct(&[root.clone(), system_program.clone()])?;
    let policy = state.binding().facts();
    let funding = state.root_funding().facts();
    let expected_balance = funding
        .rent_principal_lamports
        .checked_add(funding.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        root.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && root.is_writable
            && !root.is_signer
            && !root.executable
            && root.data_len() == 0
            && root.lamports() == expected_balance
            && funding.observed_balance_lamports == expected_balance
            && funding.root_account_id.bytes() == root.key.to_bytes()
            && policy.recovery_state_id.bytes() == root.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (expected_root, bump) = seeds::failure_market_root_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(root.key, (expected_root, bump), None)?;
    let market_instance = policy.market_instance_id.bytes();
    let generation = policy.generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_FAILURE_MARKET_ROOT_V2,
        &market_instance,
        &generation,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2),
        vec![AccountMeta::new(*root.key, true)],
    );
    invoke_signed(
        &allocate,
        &[root.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*root.key, true)],
    );
    invoke_signed(
        &assign,
        &[root.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        root.owner == program_id
            && root.data_len() == FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2
            && root.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    persist_failure_market_root_v2(program_id, root, state)
}

/// Authenticate an existing shared-Market policy/funding root.
pub fn authenticate_failure_market_root_v2(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedFailureMarketRootV2> {
    require(root.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!root.is_signer, ClutchError::NonCanonical)?;
    require(!root.executable, ClutchError::ExecutableAccount)?;
    require(
        root.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    let data = root
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let input: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2] = data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let record = FailureMarketRootAccountV2::decode(input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let state = FailureMarketAdmissionStateV1::decode(&record.admission_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let policy = state.binding().facts();
    let root_funding = state.root_funding().facts();
    require(
        policy.recovery_state_id.bytes() == root.key.to_bytes()
            && policy.recovery_receipt_program_id == liveness_id(program_id)
            && root_funding.root_account_id.bytes() == root.key.to_bytes()
            && root.lamports() >= root_funding.observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        root.key,
        seeds::failure_market_root_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(record.bump),
    )?;
    Ok(AuthenticatedFailureMarketRootV2 {
        account: *root.key,
        bump: record.bump,
        state,
    })
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}
