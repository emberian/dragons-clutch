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
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_policy_v1::{
    admit_failure_market_recovery_funding_v1, project_initial_market_recovery_funding_v1,
    AuthenticatedFailureMarketRecoveryFundingV1, FailureMarketAdmissionStateV1,
    FailureMarketPolicyBindingV1, FailureMarketPrepaidDebitReceiptIdV1,
    FailureMarketRecoveryFundingFactsV1, FailureMarketRecoveryFundingReceiptV1,
    FailureMarketRootBalanceDispositionV1, FailureMarketRootFundingReceiptV1,
    FAILURE_MARKET_ADMISSION_STATE_BYTES_V1,
};
use clutch_failure_policy_runtime::market_quote_v1::{
    FailureMarketRecoveryQuoteScheduleV1, FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::{RuntimeCompartmentKindV1, RuntimeCompartmentV1};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FailureMarketRootAccountV3,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    FAILURE_MARKET_ADMISSION_BODY_BYTES_V1,
    FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1,
    FAILURE_MARKET_RECOVERY_QUOTE_BODY_BYTES_V1, FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const _: () =
    assert!(FAILURE_MARKET_ADMISSION_BODY_BYTES_V1 == FAILURE_MARKET_ADMISSION_STATE_BYTES_V1);
const _: () = assert!(
    FAILURE_MARKET_RECOVERY_QUOTE_BODY_BYTES_V1
        == FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1
);

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

/// Existing authenticated current `0xa0/v4` shared-Market root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFailureMarketRootV3 {
    account: Pubkey,
    bump: u8,
    state: FailureMarketAdmissionStateV1,
    recovery_quote: FailureMarketRecoveryQuoteScheduleV1,
    interval_funding_preimage:
        [u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1],
}

impl AuthenticatedFailureMarketRootV3 {
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

    /// Exact persisted per-attempt Recovery price schedule. This is chain
    /// state, never a caller payload or static-client projection.
    pub const fn recovery_quote(self) -> FailureMarketRecoveryQuoteScheduleV1 {
        self.recovery_quote
    }

    /// Exact retained Product capitalization preimage for the reusable
    /// interval pair. It is decoded again by the Failure owner before use.
    pub const fn interval_funding_preimage(
        self,
    ) -> [u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1] {
        self.interval_funding_preimage
    }
}

/// Exact writable account prestate for eventual terminal root disposition.
///
/// This is deliberately not terminal authority. It only authenticates the
/// persisted root and the two immutable lamport destinations, then projects
/// the sole valid principal/donation split. A Product-owned private terminal
/// receipt must still be consumed before any mutation may use this value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketClosePrestateV2 {
    root: AuthenticatedFailureMarketRootV3,
    disposition: FailureMarketRootBalanceDispositionV1,
    refund_owner_post_balance: u64,
    neutral_sink_post_balance: u64,
}

impl AuthenticatedFailureMarketClosePrestateV2 {
    /// Exact authenticated root and immutable policy/funding state.
    pub(crate) const fn root(self) -> AuthenticatedFailureMarketRootV3 {
        self.root
    }

    /// Sole exact close-time principal/donation split.
    pub(crate) const fn disposition(self) -> FailureMarketRootBalanceDispositionV1 {
        self.disposition
    }

    /// Checked refund-recipient postbalance.
    pub(crate) const fn refund_owner_post_balance(self) -> u64 {
        self.refund_owner_post_balance
    }

    /// Checked neutral-sink postbalance.
    pub(crate) const fn neutral_sink_post_balance(self) -> u64 {
        self.neutral_sink_post_balance
    }
}

/// Atomic semantic postimage of one fully joined Market Failure admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketAdmissionPostimageV1 {
    root: AuthenticatedFailureMarketRootV3,
    liveness: AuthenticatedFailureMarketLivenessV1,
    recovery_funding: FailureMarketRecoveryFundingReceiptV1,
}

impl FailureMarketAdmissionPostimageV1 {
    /// Newly persisted authenticated shared root.
    pub const fn root(self) -> AuthenticatedFailureMarketRootV3 {
        self.root
    }

    /// Exact full-body liveness authentication used by admission.
    pub const fn liveness(self) -> AuthenticatedFailureMarketLivenessV1 {
        self.liveness
    }

    /// Exact sole-custody Recovery funding receipt persisted in the root.
    pub const fn recovery_funding(self) -> FailureMarketRecoveryFundingReceiptV1 {
        self.recovery_funding
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
fn persist_failure_market_root_v3(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    state: FailureMarketAdmissionStateV1,
    recovery_quote: FailureMarketRecoveryQuoteScheduleV1,
    interval_funding_preimage:
        [u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1],
) -> Outcome<AuthenticatedFailureMarketRootV3> {
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
            && root.data_len() == FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    require(
        funding.root_account_id.bytes() == root.key.to_bytes()
            && funding.rent_principal_lamports != 0
            && funding.observed_balance_lamports == expected_balance
            && root.lamports() == expected_balance
            && policy.recovery_state_id.bytes() != root.key.to_bytes(),
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
    let quote_id = recovery_quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        quote_id.bytes() == policy.recovery_quote_schedule_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        interval_funding_preimage.iter().any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )?;
    let mut recovery_quote_body = [0u8; FAILURE_MARKET_RECOVERY_QUOTE_BODY_BYTES_V1];
    recovery_quote
        .encode_into(&mut recovery_quote_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let record = FailureMarketRootAccountV3 {
        bump,
        admission_body,
        recovery_quote_body,
        interval_funding_preimage,
    };
    let mut data = root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    let output: &mut [u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3] = data
        .as_mut()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    record
        .encode_into(output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(AuthenticatedFailureMarketRootV3 {
        account: *root.key,
        bump,
        state,
        recovery_quote,
        interval_funding_preimage,
    })
}

/// Allocate, assign, and persist one exactly prefunded shared-Market root.
///
/// Product must have applied the authenticated FoundationVault debit first in
/// the same outer instruction. This helper performs no transfer and therefore
/// cannot source rent from a signer, Recovery custody, Hoard, or future fees.
/// The supplied canonical Rent sysvar fixes the exact refundable principal for
/// the current 2,172-byte account width; prior lamports remain donations.
fn initialize_prefunded_failure_market_root_v3<'a>(
    program_id: &Pubkey,
    root: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    state: FailureMarketAdmissionStateV1,
    recovery_quote: FailureMarketRecoveryQuoteScheduleV1,
    interval_funding_preimage:
        [u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1],
) -> Outcome<AuthenticatedFailureMarketRootV3> {
    require_system_program(system_program)?;
    require_distinct(&[root.clone(), rent_sysvar.clone(), system_program.clone()])?;
    let rent = read_rent(rent_sysvar)?;
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
            && funding.rent_principal_lamports
                == rent.minimum_balance(FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3)?
            && funding.root_account_id.bytes() == root.key.to_bytes()
            && policy.recovery_state_id.bytes() != root.key.to_bytes(),
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
        &allocate_data(FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3),
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
            && root.data_len() == FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3
            && root.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    persist_failure_market_root_v3(
        program_id,
        root,
        state,
        recovery_quote,
        interval_funding_preimage,
    )
}

/// Execute the complete non-routable shared-Market admission join.
///
/// This remains crate-private until Product's account adapter supplies both
/// private debit authorities. The root write cannot be reached through this
/// seam without authenticating the full liveness bodies and exact current
/// Recovery balance first.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_failure_market_admission_v3<'a>(
    program_id: &Pubkey,
    root: &AccountInfo<'a>,
    liveness_policy_account: &AccountInfo<'a>,
    recovery_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    binding: FailureMarketPolicyBindingV1,
    root_funding: FailureMarketRootFundingReceiptV1,
    recovery_prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1,
    recovery_quote: FailureMarketRecoveryQuoteScheduleV1,
    interval_funding_preimage:
        [u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BODY_BYTES_V1],
) -> Outcome<FailureMarketAdmissionPostimageV1> {
    require_distinct(&[
        root.clone(),
        liveness_policy_account.clone(),
        recovery_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    let (liveness, recovery_funding) = authenticate_initial_market_recovery_funding_v1(
        program_id,
        liveness_policy_account,
        recovery_account,
        binding,
        recovery_prepaid_debit_receipt_id,
    )?;
    let state =
        FailureMarketAdmissionStateV1::from_receipts(binding, recovery_funding, root_funding)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root = initialize_prefunded_failure_market_root_v3(
        program_id,
        root,
        rent_sysvar,
        system_program,
        state,
        recovery_quote,
        interval_funding_preimage,
    )?;
    Ok(FailureMarketAdmissionPostimageV1 {
        root,
        liveness,
        recovery_funding,
    })
}

/// Authenticate an existing shared-Market policy/funding root.
pub fn authenticate_failure_market_root_v3(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedFailureMarketRootV3> {
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
    let input: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V3] = data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let record = FailureMarketRootAccountV3::decode(input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let state = FailureMarketAdmissionStateV1::decode(&record.admission_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let recovery_quote = FailureMarketRecoveryQuoteScheduleV1::decode(&record.recovery_quote_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        record
            .interval_funding_preimage
            .iter()
            .any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )?;
    let policy = state.binding().facts();
    let root_funding = state.root_funding().facts();
    require(
        policy.recovery_state_id.bytes() != root.key.to_bytes()
            && policy.recovery_receipt_program_id == liveness_id(program_id)
            && recovery_quote
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
                .bytes()
                == policy.recovery_quote_schedule_id.bytes()
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
    Ok(AuthenticatedFailureMarketRootV3 {
        account: *root.key,
        bump: record.bump,
        state,
        recovery_quote,
        interval_funding_preimage: record.interval_funding_preimage,
    })
}

/// Authenticate the exact close-time accounts and project their postbalances.
///
/// This performs no write and accepts no terminal ID or caller-provided
/// disposition. The eventual close wrapper must additionally consume
/// Product's private whole-Market terminal receipt in the same instruction.
pub(crate) fn authenticate_failure_market_close_prestate_v2<'a>(
    program_id: &Pubkey,
    root: &AccountInfo<'a>,
    refund_owner: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
) -> Outcome<AuthenticatedFailureMarketClosePrestateV2> {
    require_distinct(&[root.clone(), refund_owner.clone(), neutral_sink.clone()])?;
    for recipient in [refund_owner, neutral_sink] {
        require(recipient.is_writable, ClutchError::NotWritable)?;
        require(!recipient.is_signer, ClutchError::NonCanonical)?;
        require(!recipient.executable, ClutchError::ExecutableAccount)?;
    }
    let authenticated = authenticate_failure_market_root_v3(program_id, root, true)?;
    let disposition = authenticated
        .state()
        .project_root_balance_disposition(root.lamports())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        disposition.root_account_id().bytes() == root.key.to_bytes()
            && disposition.rent_refund_owner().bytes() == refund_owner.key.to_bytes()
            && disposition.neutral_sink().bytes() == neutral_sink.key.to_bytes()
            && disposition.expected_root_pre_balance() == root.lamports()
            && disposition
                .rent_refund_lamports()
                .checked_add(disposition.donation_neutral_lamports())
                == Some(root.lamports()),
        ClutchError::MismatchedState,
    )?;
    let refund_owner_post_balance = refund_owner
        .lamports()
        .checked_add(disposition.rent_refund_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let neutral_sink_post_balance = neutral_sink
        .lamports()
        .checked_add(disposition.donation_neutral_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    Ok(AuthenticatedFailureMarketClosePrestateV2 {
        root: authenticated,
        disposition,
        refund_owner_post_balance,
        neutral_sink_post_balance,
    })
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}
