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
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1;
use clutch_failure_policy_runtime::market_runtime_v1::{
    admit_failure_market_runtime_v1, AuthenticatedFailureMarketRuntimeAdmissionV1,
    FailureMarketRuntimeAdmissionReceiptV1, FailureMarketRuntimeRootFundingFactsV1,
    FailureMarketRuntimeStateCommitmentV1, FailureMarketRuntimeTerminalPlanV2,
    FailureMarketRuntimeV1, FailureMarketSessionTransitionPlanV1,
    FailureMarketSessionTransitionReceiptIdV1, FAILURE_MARKET_RUNTIME_BYTES_V1,
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
const FAILURE_MARKET_RUNTIME_SESSION_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/failure-market-runtime-session-postwrite/v1\0";

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

/// Exact shared-runtime session transition admitted by one outer composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketRuntimeSessionWriteFactsV1 {
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    pub runtime_after: FailureMarketRuntimeStateCommitmentV1,
    pub transition_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

/// Default-refusing authority for one exact subordinate-session postwrite.
pub(crate) trait AuthenticatedFailureMarketRuntimeSessionWriteV1 {
    fn authenticate_failure_market_runtime_session_write_v1(
        &self,
        _expected: FailureMarketRuntimeSessionWriteFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        Err(clutch_failure_policy_runtime::Error::BindingMismatch)
    }
}

/// Hostile-reopened mutable runtime postimage for one session transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketRuntimeSessionPostwriteV1 {
    id: ProductContentId,
    root: AuthenticatedFailureMarketRuntimeRootV1,
    transition_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
    runtime_before: FailureMarketRuntimeStateCommitmentV1,
}

impl AuthenticatedFailureMarketRuntimeSessionPostwriteV1 {
    pub(crate) const fn id(self) -> ProductContentId {
        self.id
    }

    pub(crate) const fn root(self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.root
    }

    pub(crate) const fn transition_receipt_id(self) -> FailureMarketSessionTransitionReceiptIdV1 {
        self.transition_receipt_id
    }

    pub(crate) const fn runtime_before(self) -> FailureMarketRuntimeStateCommitmentV1 {
        self.runtime_before
    }
}

/// Authenticate an existing `0xa0/v3` runtime against immutable `0xa0/v2`.
pub fn authenticate_failure_market_runtime_root_v1<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    writable: bool,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(
        live_admission == admission_root,
        ClutchError::MismatchedState,
    )?;
    require(
        *runtime_root.key != *admission_root_account.key
            && *runtime_root.key != admission_root.account(),
        ClutchError::AccountAlias,
    )?;
    let admission_root = live_admission;
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
    admission_root_account: &AccountInfo<'a>,
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
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(
        live_admission == admission_root,
        ClutchError::MismatchedState,
    )?;
    let admission_root = live_admission;
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
        admission_root_account,
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
    admission_root_account: &AccountInfo<'a>,
    runtime_root: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    state: FailureMarketRuntimeV1,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    require_system_program(system_program)?;
    require_distinct(&[
        admission_root_account.clone(),
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
    persist_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        state,
    )
}

fn persist_failure_market_runtime_root_v1(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
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
    drop(data);
    let reopened = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        true,
    )?;
    require(reopened.state == state, ClutchError::MismatchedState)?;
    Ok(reopened)
}

/// Persist one exact Begin/advance/resolution/archive transcript transition.
///
/// The pure plan is already guarded by the outer operation's private semantic
/// authority. This second default-refusing boundary binds that plan to the
/// outer operation's exact physical postwrites before the mutable Market root
/// is changed. The account is hostile-reopened both before and after the write,
/// and its separately owned rent principal and donations never move here.
pub(crate) fn write_failure_market_runtime_session_plan_v1<
    A: AuthenticatedFailureMarketRuntimeSessionWriteV1 + ?Sized,
>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root: &AccountInfo<'_>,
    admission_root: AuthenticatedFailureMarketRootV2,
    authenticated: AuthenticatedFailureMarketRuntimeRootV1,
    plan: FailureMarketSessionTransitionPlanV1,
    authority: &A,
) -> Outcome<AuthenticatedFailureMarketRuntimeSessionPostwriteV1> {
    require(
        authenticated.account == *runtime_root.key && runtime_root.is_writable,
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        true,
    )?;
    require(live == authenticated, ClutchError::MismatchedState)?;
    let runtime_before = live.state_commitment;
    let resulting = plan.resulting_runtime();
    let runtime_after = resulting
        .commitment()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = FailureMarketRuntimeSessionWriteFactsV1 {
        runtime_before,
        runtime_after,
        transition_receipt_id: plan.receipt_id(),
    };
    authority
        .authenticate_failure_market_runtime_session_write_v1(facts)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut after = live.state;
    after
        .commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(after == resulting, ClutchError::MismatchedState)?;
    let balance_before = runtime_root.lamports();
    let mut runtime_body = [0; FAILURE_MARKET_RUNTIME_BYTES_V1];
    after
        .encode_into(&mut runtime_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let record = FailureMarketRuntimeRootAccountV1 {
        bump: live.bump,
        runtime_body,
    };
    {
        let mut data = runtime_root
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let output: &mut [u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        record
            .encode_into(output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    require(
        runtime_root.lamports() == balance_before,
        ClutchError::MismatchedState,
    )?;
    let reopened = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        true,
    )?;
    require(
        reopened.state == after
            && reopened.state_commitment == runtime_after
            && reopened.state_commitment != runtime_before,
        ClutchError::MismatchedState,
    )?;
    let id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RUNTIME_SESSION_POSTWRITE_DOMAIN_V1,
            runtime_root.key.as_ref(),
            &runtime_before.bytes(),
            &runtime_after.bytes(),
            &facts.transition_receipt_id.bytes(),
            &balance_before.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(id != ProductContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketRuntimeSessionPostwriteV1 {
        id,
        root: reopened,
        transition_receipt_id: facts.transition_receipt_id,
        runtime_before,
    })
}

/// Persist one exact Recovery-close or final-family runtime transition.
///
/// The pure plan has private prestates and can only follow the typed terminal
/// chain. This writer reopens the live account immediately before mutation,
/// commits the stale-checked plan, preserves every lamport, and hostile-decodes
/// the postimage again. It does not route an instruction or construct any
/// Product, replay, history, or liveness authority.
pub(crate) fn write_failure_market_runtime_terminal_plan_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root: &AccountInfo<'a>,
    admission_root: AuthenticatedFailureMarketRootV2,
    authenticated: AuthenticatedFailureMarketRuntimeRootV1,
    plan: FailureMarketRuntimeTerminalPlanV2,
) -> Outcome<AuthenticatedFailureMarketRuntimeRootV1> {
    require(
        authenticated.account == *runtime_root.key && runtime_root.is_writable,
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        true,
    )?;
    require(live == authenticated, ClutchError::MismatchedState)?;
    let mut after = live.state;
    after
        .commit_terminal_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let balance_before = runtime_root.lamports();
    let mut runtime_body = [0; FAILURE_MARKET_RUNTIME_BYTES_V1];
    after
        .encode_into(&mut runtime_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let record = FailureMarketRuntimeRootAccountV1 {
        bump: live.bump,
        runtime_body,
    };
    {
        let mut data = runtime_root
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let output: &mut [u8; FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_BYTES_V1] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        record
            .encode_into(output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    require(
        runtime_root.lamports() == balance_before,
        ClutchError::MismatchedState,
    )?;
    let reopened = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root,
        admission_root,
        true,
    )?;
    require(
        reopened.state == after && reopened.state_commitment != live.state_commitment,
        ClutchError::MismatchedState,
    )?;
    Ok(reopened)
}

#[cfg(test)]
mod adversarial_runtime_writer_tests {
    #[test]
    fn session_writer_hostile_reopens_both_sides_and_never_moves_rent_or_donations() {
        let source = include_str!("failure_market_runtime.rs");
        let writer = source
            .split("pub(crate) fn write_failure_market_runtime_session_plan_v1")
            .nth(1)
            .expect("single private session writer");
        let preopen = writer
            .find("let live = authenticate_failure_market_runtime_root_v1")
            .expect("live prestate");
        let write = writer
            .find("record.encode_into(output)")
            .expect("canonical body write");
        let balance = writer
            .find("runtime_root.lamports() == balance_before")
            .expect("exact native-lamport preservation");
        let postopen = writer[write..]
            .find("let reopened = authenticate_failure_market_runtime_root_v1")
            .expect("hostile poststate reopen")
            + write;
        assert!(preopen < write && write < balance && balance < postopen);
        assert!(!writer[..postopen].contains("try_borrow_mut_lamports"));
    }
}
