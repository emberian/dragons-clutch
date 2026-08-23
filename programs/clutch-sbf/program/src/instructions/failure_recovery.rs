//! Disabled SBF account and atomic-mutation adapter for Recovery78/v1.
//!
//! The nine registry actions remain capability-disabled. The public helpers in
//! this module are the complete account-facing mutation seam a future atomic
//! router must call only after Source, relation, Product/Series, and terminal
//! owners have constructed their private typed receipts.

use crate::accounts::{expect_pda, require, require_count, require_distinct, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::{CLOCK_SYSVAR_ID, CLOCK_SYSVAR_LEN};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::instructions::series_failure_funding::{
    fund_series_failure_accounts_v1, SeriesMarketCoreFundingReceiptV1,
};
use crate::instructions_sysvar::SYSVAR_OWNER_ID;
use crate::seeds;
use crate::source_plane_v3_actions::SourcePolicyHandoffJoinV1;
use clutch_evidence_recovery::Identity as RecoveryIdentity;
use clutch_failure_policy_adapter::external_v2::{
    authenticate_external_root_readonly_v2, authenticate_external_root_v2,
    initialize_external_root_v2, project_external_recovery_close_v2,
    project_external_semantic_transition_v2, project_external_work_transition_v2,
    AuthenticatedExternalRootV2, ExternalAdapterErrorV2, ExternalRecoveryCloseV2,
    ExternalRootFundingObservationV2, ExternalRootInitializationV2, ExternalSemanticMutationV2,
    ExternalWorkMutationV2,
};
use clutch_failure_policy_adapter::{AccountId, AccountView};
use clutch_failure_policy_runtime::external_v2::{
    FailureExternalAdmissionReceiptV2, FailureExternalTransitionPlanV2,
    FailureRecoveryTerminalReceiptV2, FailureRuntimeExternalV2,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_compartment_account_v1, decode_runtime_policy_account_v1,
    RuntimeAdmissionAccountPlanV1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeTransferRoleV1,
};
use clutch_liveness::runtime_v1::{RuntimeCompartmentKindV1, RuntimeCompartmentV1};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::failure_recovery::{
    account_metas_v1, decode_failure_account_body_v1, decode_payload_v1,
    encode_failure_account_header_v1, AcceptRecoveryWorkV1, AdvanceRecoveryScheduleV1,
    CloseRecoveryFundingV1, FailureRecoveryPayloadV1, FailureReplayTombstonePhaseV1,
    FailureReplayTombstoneV1, RecoveryAccountRoleV1, RecoveryCommonV1, TriggerSourceFailureV1,
    ACCEPT_RECOVERY_WORK_METAS_V1, CLOSE_RECOVERY_FUNDING_METAS_V1,
    FAILURE_ACCOUNT_HEADER_BYTES_V1, FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1, FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1,
    INITIALIZE_FAILURE_ROOT_METAS_V1,
};
use clutch_solana_layout::registry::{self, RecoveryAction};
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceReleaseV1, ClockSnapshotV1, FailurePolicySourceHandoffV1,
    SuccessfulEvaluationHandoffV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Capability guard for the disabled family.
///
/// It intentionally returns before decoding payload bytes or touching account
/// metadata. Mutation is available only through the explicit typed helpers in
/// this module until a release promotes the whole family.
pub fn process(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo<'_>],
    action: RecoveryAction,
    _payload: &[u8],
) -> Outcome<()> {
    require(
        capabilities::extension_intent_action_enabled(
            registry::RECOVERY_FAMILY_TAG,
            registry::RECOVERY_FAMILY_VERSION,
            action.tag(),
        ),
        ClutchError::UnsupportedInstruction,
    )?;
    Err(ClutchError::UnsupportedInstruction.into())
}

// Relation-refusal, both resolution actions, and semantic-root close
// intentionally have no mutation helpers here. Their allocated wire shapes do
// not grant authority: the repository has no persisted relation-execution
// result owner and no per-occurrence zero-liability retirement owner yet.
// Keeping these paths absent prevents caller-built relation or retirement DTOs
// from becoming executable authority while those separate owners are built.

/// Source-owned maturity-failure join. Private fields prevent ID-only use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceFailureJoinV1 {
    release: AuthenticatedSourceReleaseV1,
    handoff: FailurePolicySourceHandoffV1,
    source: SourcePolicyHandoffJoinV1,
}

impl AuthenticatedSourceFailureJoinV1 {
    /// Join an already authenticated Source release and Source-owned handoff to
    /// the exact payload commitment.
    pub fn from_source_adapter(
        release: AuthenticatedSourceReleaseV1,
        handoff: FailurePolicySourceHandoffV1,
        source: SourcePolicyHandoffJoinV1,
    ) -> Outcome<Self> {
        let occurrence = handoff.occurrence();
        let clock_policy_id = release
            .clock_policy()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            source.handoff_id() == handoff.id()
                && source.release_authentication_id() == release.id()
                && source.route_id() == occurrence.route_id()
                && source.occurrence_account() == occurrence.occurrence_account()
                && source.source_fact_authentication_id() == handoff.source_fact_receipt_id()
                && source.clock_policy_id() == clock_policy_id
                && source.clock_policy_id() == occurrence.clock_policy_id()
                && source.clock() == handoff.clock()
                && source.failure_policy_binding_id() == handoff.failure_policy_binding_id()
                && source.source_spec_id() == occurrence.source_spec_id()
                && source.window_id() == occurrence.window_id()
                && source.statistic_key_id() == occurrence.statistic_key_id(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            release,
            handoff,
            source,
        })
    }
}

/// Source-owned successful-evaluation join. Relation semantics remain absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceSuccessJoinV1 {
    release: AuthenticatedSourceReleaseV1,
    handoff: SuccessfulEvaluationHandoffV1,
    source: SourcePolicyHandoffJoinV1,
}

impl AuthenticatedSourceSuccessJoinV1 {
    /// Join an already authenticated Source release and success handoff to the
    /// exact payload commitment.
    pub fn from_source_adapter(
        release: AuthenticatedSourceReleaseV1,
        handoff: SuccessfulEvaluationHandoffV1,
        source: SourcePolicyHandoffJoinV1,
    ) -> Outcome<Self> {
        let occurrence = handoff.occurrence();
        let clock_policy_id = release
            .clock_policy()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            source.handoff_id() == handoff.id()
                && source.release_authentication_id() == release.id()
                && source.route_id() == occurrence.route_id()
                && source.occurrence_account() == occurrence.occurrence_account()
                && source.source_fact_authentication_id()
                    == handoff.result_account_authentication_id()
                && source.clock_policy_id() == clock_policy_id
                && source.clock_policy_id() == occurrence.clock_policy_id()
                && source.clock() == handoff.clock()
                && source.failure_policy_binding_id() == handoff.failure_policy_binding_id()
                && source.source_spec_id() == occurrence.source_spec_id()
                && source.window_id() == occurrence.window_id()
                && source.statistic_key_id() == occurrence.statistic_key_id(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            release,
            handoff,
            source,
        })
    }
}

/// Decode the hostile payload and enforce the exact account count/privileges.
pub fn authenticate_action_envelope_v1<'a>(
    action: RecoveryAction,
    payload: &[u8],
    accounts: &'a [AccountInfo<'a>],
) -> Outcome<FailureRecoveryPayloadV1> {
    let decoded = decode_payload_v1(action, payload)?;
    authenticate_ordered_metas_v1(action, accounts)?;
    Ok(decoded)
}

/// Authenticate exact ordered roles without interpreting semantic bytes.
pub fn authenticate_ordered_metas_v1(
    action: RecoveryAction,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    let metas = account_metas_v1(action);
    require_count(accounts, metas.len())?;
    require_distinct(accounts)?;
    let mut index = 0usize;
    while index < metas.len() {
        let expected = metas[index];
        let account = &accounts[index];
        require(
            account.is_writable == expected.writable,
            if expected.writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            account.is_signer == expected.signer,
            if expected.signer {
                ClutchError::MissingSignature
            } else {
                ClutchError::NonCanonical
            },
        )?;
        let executable = matches!(
            expected.role,
            RecoveryAccountRoleV1::SourceAdapterProgram
                | RecoveryAccountRoleV1::ParserProgram
                | RecoveryAccountRoleV1::SystemProgram
        );
        require(
            account.executable == executable,
            ClutchError::ExecutableAccount,
        )?;
        index += 1;
    }
    Ok(())
}

/// Read the exact canonical Clock sysvar and refuse negative Unix time.
pub fn authenticate_clock_snapshot_v1(account: &AccountInfo<'_>) -> Outcome<ClockSnapshotV1> {
    require(
        *account.key == CLOCK_SYSVAR_ID,
        ClutchError::WrongClockSysvar,
    )?;
    require(
        account.owner.to_bytes() == SYSVAR_OWNER_ID,
        ClutchError::WrongClockSysvar,
    )?;
    require(
        !account.is_writable && !account.is_signer && !account.executable,
        ClutchError::WrongClockSysvar,
    )?;
    require(
        account.data_len() == CLOCK_SYSVAR_LEN,
        ClutchError::WrongClockSysvar,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let slot = u64::from_le_bytes(array_at::<8>(&data, 0)?);
    let signed = i64::from_le_bytes(array_at::<8>(&data, 32)?);
    let unix_timestamp =
        u64::try_from(signed).map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    Ok(ClockSnapshotV1 {
        slot,
        unix_timestamp,
    })
}

/// Persist the main-program frame around one fully decoded immutable liveness
/// policy body. Allocation and present rent funding must already be complete.
pub fn persist_liveness_policy_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    stored_bump: u8,
    policy_body: &[u8],
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
            && policy_body.len() == FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let policy_view = RuntimePersistedAccountViewV1 {
        account_id: liveness_id(account.key),
        owner_program_id: liveness_id(program_id),
        lamports: account.lamports(),
        data: policy_body,
        writable: false,
    };
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(account.key),
        policy_view,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(stored_bump),
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    encode_failure_account_header_v1(
        &mut data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        stored_bump,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(policy_body);
    Ok(())
}

/// Persist the sole Recovery compartment from a checked liveness admission
/// plan. The observed account must already hold its exact planned balance.
pub fn persist_recovery_admission_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    stored_bump: u8,
    plan: RuntimeAdmissionAccountPlanV1,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        account.data_len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
            && plan.kind == RuntimeCompartmentKindV1::Recovery
            && plan.account_id == liveness_id(account.key)
            && plan.owner_program_id_after == liveness_id(program_id)
            && plan.balance_after == account.lamports(),
        ClutchError::MismatchedState,
    )?;
    let raw_view = RuntimePersistedAccountViewV1 {
        account_id: plan.account_id,
        owner_program_id: plan.owner_program_id_after,
        lamports: plan.balance_after,
        data: &plan.post_account_data,
        writable: true,
    };
    let state = decode_runtime_compartment_account_v1(liveness_id(program_id), raw_view)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        state.kind == RuntimeCompartmentKindV1::Recovery
            && state
                .expected_account_balance_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == account.lamports(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &state.identity.lifecycle_id.bytes(),
            state.identity.generation,
        ),
        Some(stored_bump),
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    encode_failure_account_header_v1(
        &mut data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        stored_bump,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&plan.post_account_data);
    Ok(())
}

/// Atomic postimages created by one Series-funded Failure activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRootActivationV1 {
    /// Initialized semantic root and immutable rent ownership.
    pub root: ExternalRootInitializationV2,
    /// Pre-funded permanent replay record in its pending phase.
    pub replay: FailureReplayTombstoneV1,
}

/// Execute root and replay initialization after the caller has constructed
/// the typed successor runtime and Series funding receipt from the exact
/// accounts in the frozen 32-role list.
pub fn handle_initialize_failure_root_v1<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    payload: &clutch_solana_layout::failure_recovery::InitializeFailureRootV1,
    source_release: AuthenticatedSourceReleaseV1,
    runtime: FailureRuntimeExternalV2,
    receipt: FailureExternalAdmissionReceiptV2,
    market_core_funding: SeriesMarketCoreFundingReceiptV1,
) -> Outcome<FailureRootActivationV1> {
    authenticate_ordered_metas_v1(RecoveryAction::InitializeFailureRoot, accounts)?;
    require_source_release_account(source_release, &accounts[18])?;
    runtime
        .authenticate_source_release(source_release)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    authenticate_present_recovery_admission(
        program_id,
        &accounts[2],
        &accounts[3],
        runtime,
        receipt,
    )?;
    initialize_failure_root_v1(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[6],
        &accounts[4],
        &accounts[29],
        &accounts[31],
        &accounts[30],
        payload,
        runtime,
        receipt,
        market_core_funding,
    )
}

/// Create the semantic root and pending permanent replay record from the sole
/// typed Series MarketCore receipt. The caller must have obtained `runtime`
/// and `receipt` from `FailureRuntimeExternalV2::admit_successor` over
/// authenticated accounts.
#[allow(clippy::too_many_arguments)]
pub fn initialize_failure_root_v1<'a>(
    program_id: &Pubkey,
    market_core_vault: &AccountInfo<'a>,
    root: &AccountInfo<'a>,
    funding_state: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    replay_tombstone: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    payload: &clutch_solana_layout::failure_recovery::InitializeFailureRootV1,
    runtime: FailureRuntimeExternalV2,
    receipt: FailureExternalAdmissionReceiptV2,
    market_core_funding: SeriesMarketCoreFundingReceiptV1,
) -> Outcome<FailureRootActivationV1> {
    payload.common.validate_for_runtime(runtime)?;
    require(
        payload.common.expected_transition_nonce == 0,
        ClutchError::Replay,
    )?;
    require(
        receipt.binding_id().bytes() == payload.common.binding_id
            && receipt.market_instance_id().bytes() == payload.common.market_instance_v2_id
            && receipt.generation() == payload.common.generation
            && receipt.series_plan_id() == payload.series_plan_v5_id
            && receipt.ordinal() == payload.ordinal
            && receipt.funding_quote_id().bytes() == payload.series_funding_quote_id,
        ClutchError::MismatchedState,
    )?;
    let admitted_rent = market_core_funding
        .failure_root_rent_principal_lamports()
        .checked_add(market_core_funding.replay_tombstone_rent_principal_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let expected_intermediate = market_core_funding
        .vault_balance_before()
        .checked_sub(admitted_rent)
        .ok_or(ClutchError::Arithmetic)?;
    let expected_final = market_core_funding
        .vault_balance_before()
        .checked_sub(market_core_funding.market_core_debit_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    require(
        market_core_funding.id().bytes() == payload.market_core_funding_receipt_id
            && market_core_funding.series_plan_id().bytes() == payload.series_plan_v5_id
            && market_core_funding.ordinal() == payload.ordinal
            && market_core_funding.market_instance_id().bytes()
                == payload.common.market_instance_v2_id
            && market_core_funding.funding_quote_id().bytes() == payload.series_funding_quote_id
            && market_core_funding.generation() == payload.common.generation
            && market_core_funding.funding_state_account() == *funding_state.key
            && market_core_funding.market_core_lamport_vault() == *market_core_vault.key
            && market_core_funding.neutral_lamport_sink().bytes() == neutral_sink.key.to_bytes()
            && market_core_funding.failure_root_rent_principal_lamports()
                == payload.root_rent_principal_lamports
            && market_core_funding.replay_tombstone_rent_principal_lamports()
                == payload.replay_rent_principal_lamports
            && market_core_funding.vault_balance_after_failure_accounts() == expected_intermediate
            && market_core_funding.vault_balance_after() == expected_final
            && market_core_funding.market_core_debit_lamports() >= admitted_rent,
        ClutchError::MismatchedState,
    )?;
    require(!neutral_sink.is_writable, ClutchError::UnexpectedWritable)?;
    require_creatable(root)?;
    require_creatable(replay_tombstone)?;
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    let root_minimum = rent.minimum_balance(FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1)?;
    let replay_minimum = rent.minimum_balance(FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1)?;
    require(
        root_minimum == payload.root_rent_principal_lamports
            && replay_minimum == payload.replay_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected_root, root_bump) = seeds::failure_external_root_pda(
        program_id,
        &payload.common.market_instance_v2_id,
        payload.common.generation,
    );
    let (expected_replay, replay_bump) = seeds::failure_replay_tombstone_pda(
        program_id,
        &payload.common.market_instance_v2_id,
        payload.common.generation,
    );
    expect_pda(root.key, (expected_root, root_bump), None)?;
    expect_pda(replay_tombstone.key, (expected_replay, replay_bump), None)?;
    let root_balance_before = root.lamports();
    let replay_balance_before = replay_tombstone.lamports();
    let root_balance_after = root_balance_before
        .checked_add(root_minimum)
        .ok_or(ClutchError::Arithmetic)?;
    let replay_balance_after = replay_balance_before
        .checked_add(replay_minimum)
        .ok_or(ClutchError::Arithmetic)?;
    fund_series_failure_accounts_v1(
        program_id,
        market_core_funding,
        market_core_vault,
        root,
        replay_tombstone,
        system_program,
    )?;
    require(
        root.lamports() == root_balance_after
            && replay_tombstone.lamports() == replay_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    allocate_assign_failure_root(program_id, root, system_program, &payload.common, root_bump)?;
    allocate_assign_failure_tombstone(
        program_id,
        replay_tombstone,
        system_program,
        &payload.common.market_instance_v2_id,
        payload.common.generation,
        replay_bump,
    )?;

    let root_plan = {
        let data = root
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.len() == FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1
                && data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
        initialize_external_root_v2(
            id(program_id),
            AccountView {
                key: id(root.key),
                owner: id(root.owner),
                lamports: root.lamports(),
                data: &data[FAILURE_ACCOUNT_HEADER_BYTES_V1..],
                is_writable: root.is_writable,
            },
            root_bump,
            AccountId::from_bytes(market_core_funding.lamport_principal_refund().bytes()),
            root_minimum,
            id(neutral_sink.key),
            ExternalRootFundingObservationV2 {
                balance_before: root_balance_before,
                balance_after: root_balance_after,
                payer_debit_lamports: root_minimum,
            },
            runtime,
            receipt,
        )
        .map_err(map_external_error)?
    };
    require(root_plan.root == id(root.key), ClutchError::MismatchedState)?;
    let mut data = root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode_failure_account_header_v1(
        &mut data,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
        root_bump,
        FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
    )?;
    data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&root_plan.post_root_data);
    drop(data);

    let replay = FailureReplayTombstoneV1 {
        stored_bump: replay_bump,
        phase: FailureReplayTombstonePhaseV1::Pending,
        permanent_rent_lamports: replay_minimum,
        prior_donation_lamports: replay_balance_before,
        permanent_rent_funder: market_core_funding.lamport_principal_refund().bytes(),
        funding_admission_receipt_id: market_core_funding.id().bytes(),
        binding_id: payload.common.binding_id,
        market_instance_v2_id: payload.common.market_instance_v2_id,
        generation: payload.common.generation,
        failure_terminal_join_id: [0; 32],
        retirement_root_id: [0; 32],
        source_release_receipt_id: [0; 32],
    };
    let mut replay_data = replay_tombstone
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        replay_data.len() == FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1
            && replay_data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    replay.encode(&mut replay_data)?;
    Ok(FailureRootActivationV1 {
        root: root_plan,
        replay,
    })
}

/// Authenticate the root frame, PDA, complete owner body, and common replay
/// join. Source or relation authority is deliberately not inferred here.
pub fn authenticate_failure_root_v1(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    common: RecoveryCommonV1,
) -> Outcome<AuthenticatedExternalRootV2> {
    authenticate_failure_root_access_v1(program_id, root, common, true)
}

fn authenticate_failure_root_readonly_v1(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    common: RecoveryCommonV1,
) -> Outcome<AuthenticatedExternalRootV2> {
    authenticate_failure_root_access_v1(program_id, root, common, false)
}

fn authenticate_failure_root_access_v1(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    common: RecoveryCommonV1,
    require_writable: bool,
) -> Outcome<AuthenticatedExternalRootV2> {
    let authenticated = {
        let data = root
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let frame = decode_failure_account_body_v1(
            &data,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
        )?;
        let (expected, bump) = seeds::failure_external_root_pda(
            program_id,
            &common.market_instance_v2_id,
            common.generation,
        );
        expect_pda(root.key, (expected, bump), Some(frame.stored_bump))?;
        let view = AccountView {
            key: id(root.key),
            owner: id(root.owner),
            lamports: root.lamports(),
            data: frame.body,
            is_writable: root.is_writable,
        };
        if require_writable {
            authenticate_external_root_v2(id(program_id), view)
        } else {
            authenticate_external_root_readonly_v2(id(program_id), view)
        }
        .map_err(map_external_error)?
    };
    common.validate_for_runtime(authenticated.runtime())?;
    Ok(authenticated)
}

/// Plan a Source-owned failure trigger against an authenticated root.
pub fn plan_source_failure_v1(
    root: AuthenticatedExternalRootV2,
    source: AuthenticatedSourceFailureJoinV1,
) -> Outcome<FailureExternalTransitionPlanV2> {
    root.runtime()
        .plan_trigger_source_handoff(source.handoff, source.release)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Plan one Clock-driven schedule advance using only the release-embedded
/// Clock policy and the canonical sysvar snapshot.
pub fn plan_schedule_advance_v1(
    root: AuthenticatedExternalRootV2,
    source_release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
    expected_attempt_index: u8,
) -> Outcome<FailureExternalTransitionPlanV2> {
    require(
        root.runtime().next_attempt_index() == expected_attempt_index,
        ClutchError::Replay,
    )?;
    let snapshot = authenticate_clock_snapshot_v1(clock_account)?;
    root.runtime()
        .plan_advance_schedule_from_source_release(source_release, snapshot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Plan one accepted recovery work unit. Liveness is the sole payer.
pub fn plan_accept_recovery_work_v1(
    root: AuthenticatedExternalRootV2,
    source: AuthenticatedSourceSuccessJoinV1,
    reward_recipient: &Pubkey,
    scheduled_ceiling_lamports: u64,
) -> Outcome<FailureExternalTransitionPlanV2> {
    root.runtime()
        .plan_accept_repair_work(
            source.handoff,
            source.release,
            RecoveryIdentity::from_bytes(reward_recipient.to_bytes()),
            scheduled_ceiling_lamports,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Execute the complete typed Source-failure trigger handler.
pub fn handle_source_failure_v1<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    payload: TriggerSourceFailureV1,
    source: AuthenticatedSourceFailureJoinV1,
) -> Outcome<ExternalSemanticMutationV2> {
    authenticate_ordered_metas_v1(RecoveryAction::TriggerSourceFailure, accounts)?;
    require_source_action(
        source.source,
        payload.common,
        payload.source_failure_handoff_id,
        source.handoff.occurrence().market_instance_id().bytes(),
    )?;
    require_failure_source_accounts(source, &accounts[1..])?;
    require_current_after(accounts, 5, source.handoff.clock())?;
    let root = authenticate_failure_root_v1(program_id, &accounts[0], payload.common)?;
    let plan = plan_source_failure_v1(root, source)?;
    apply_semantic_transition_v1(program_id, &accounts[0], payload.common, plan)
}

/// Execute one immutable schedule advance using the canonical Clock.
pub fn handle_schedule_advance_v1<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    payload: AdvanceRecoveryScheduleV1,
    source_release: AuthenticatedSourceReleaseV1,
) -> Outcome<ExternalSemanticMutationV2> {
    authenticate_ordered_metas_v1(RecoveryAction::AdvanceRecoverySchedule, accounts)?;
    require_source_release_account(source_release, &accounts[1])?;
    let root = authenticate_failure_root_v1(program_id, &accounts[0], payload.common)?;
    let plan = plan_schedule_advance_v1(
        root,
        source_release,
        &accounts[2],
        payload.expected_attempt_index,
    )?;
    apply_semantic_transition_v1(program_id, &accounts[0], payload.common, plan)
}

/// Execute one Source-authenticated repair unit and its sole liveness debit.
pub fn handle_accept_recovery_work_v1<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    payload: AcceptRecoveryWorkV1,
    source: AuthenticatedSourceSuccessJoinV1,
) -> Outcome<ExternalWorkMutationV2> {
    authenticate_ordered_metas_v1(RecoveryAction::AcceptRecoveryWork, accounts)?;
    require_source_action(
        source.source,
        payload.common,
        payload.source_success_handoff_id,
        source.handoff.occurrence().market_instance_id().bytes(),
    )?;
    require_success_source_accounts(source, &accounts[3..])?;
    require_current_after(accounts, 9, source.handoff.clock())?;
    require(
        accounts[7].key.to_bytes() == payload.reward_recipient,
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_failure_root_v1(program_id, &accounts[0], payload.common)?;
    let plan = plan_accept_recovery_work_v1(
        root,
        source,
        accounts[7].key,
        payload.scheduled_ceiling_lamports,
    )?;
    apply_work_transition_v1(
        program_id,
        RecoveryAction::AcceptRecoveryWork,
        accounts,
        &FailureRecoveryPayloadV1::AcceptRecoveryWork(payload),
        plan,
    )
}

/// Apply a Source failure trigger or schedule advance to the root only.
fn apply_semantic_transition_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    common: RecoveryCommonV1,
    plan: FailureExternalTransitionPlanV2,
) -> Outcome<ExternalSemanticMutationV2> {
    let root = authenticate_failure_root_v1(program_id, root_account, common)?;
    let mutation =
        project_external_semantic_transition_v2(root, plan).map_err(map_external_error)?;
    require(
        mutation.root == id(root_account.key),
        ClutchError::MismatchedState,
    )?;
    write_root_poststate(root_account, &mutation)?;
    Ok(mutation)
}

/// Atomically apply the failure-root and sole liveness Recovery work mutation.
/// `accounts` must use the accepted-work ordered contract.
fn apply_work_transition_v1<'a>(
    program_id: &Pubkey,
    action: RecoveryAction,
    accounts: &'a [AccountInfo<'a>],
    payload: &FailureRecoveryPayloadV1,
    plan: FailureExternalTransitionPlanV2,
) -> Outcome<ExternalWorkMutationV2> {
    let (metas, keeper_index, payer_index, common, source_id, recipient, ceiling) =
        match (action, payload) {
            (
                RecoveryAction::AcceptRecoveryWork,
                FailureRecoveryPayloadV1::AcceptRecoveryWork(value),
            ) => (
                ACCEPT_RECOVERY_WORK_METAS_V1,
                7usize,
                8usize,
                value.common,
                value.source_success_handoff_id,
                value.reward_recipient,
                value.scheduled_ceiling_lamports,
            ),
            _ => return Err(ClutchError::UnsupportedInstruction.into()),
        };
    require_count(accounts, metas.len())?;
    authenticate_ordered_metas_v1(action, accounts)?;
    let root_account = &accounts[0];
    let policy_account = &accounts[1];
    let recovery_account = &accounts[2];
    let keeper = &accounts[keeper_index];
    let payer = &accounts[payer_index];
    let root = authenticate_failure_root_v1(program_id, root_account, common)?;
    let work = plan
        .work()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        work.source_success_handoff_id().bytes() == source_id
            && work.reward_recipient().bytes() == recipient
            && work.reward_recipient().bytes() == keeper.key.to_bytes()
            && work.scheduled_ceiling_lamports() == ceiling,
        ClutchError::MismatchedState,
    )?;
    let expected_after = recovery_account
        .lamports()
        .checked_sub(work.scheduled_ceiling_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let mutation = project_work_with_framed_accounts(
        program_id,
        policy_account,
        recovery_account,
        expected_after,
        root,
        plan,
    )?;
    apply_work_mutation(
        root_account,
        recovery_account,
        keeper,
        payer,
        &mutation,
        work,
    )?;
    Ok(mutation)
}

/// Close only the liveness Recovery compartment from the current failure
/// terminal receipt. The semantic root remains readable and funded.
pub fn apply_recovery_close_v1<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    payload: CloseRecoveryFundingV1,
    receipt: FailureRecoveryTerminalReceiptV2,
) -> Outcome<ExternalRecoveryCloseV2> {
    require_count(accounts, CLOSE_RECOVERY_FUNDING_METAS_V1.len())?;
    authenticate_ordered_metas_v1(RecoveryAction::CloseRecoveryFunding, accounts)?;
    let root_account = &accounts[0];
    let policy_account = &accounts[1];
    let recovery_account = &accounts[2];
    let payer = &accounts[3];
    let sink = &accounts[4];
    require(
        receipt.id().bytes() == payload.recovery_terminal_receipt_id
            && receipt.transition_nonce() == payload.common.expected_transition_nonce,
        ClutchError::Replay,
    )?;
    let root = authenticate_failure_root_readonly_v1(program_id, root_account, payload.common)?;
    let close = project_close_with_framed_accounts(
        program_id,
        policy_account,
        recovery_account,
        root,
        receipt,
    )?;
    require(
        close.preserved_root == id(root_account.key),
        ClutchError::MismatchedState,
    )?;
    apply_liveness_close(recovery_account, payer, sink, &close.liveness)?;
    Ok(close)
}

fn authenticate_present_recovery_admission(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    runtime: FailureRuntimeExternalV2,
    receipt: FailureExternalAdmissionReceiptV2,
) -> Outcome<()> {
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy_view = liveness_view(policy_account, policy.body, false);
    let decoded_policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        policy_view,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        recovery_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    let decoded_recovery = RuntimeCompartmentV1::decode(recovery.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        decoded_recovery.identity.account_id == liveness_id(recovery_account.key),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &decoded_policy.policy_id.bytes()),
        Some(policy.stored_bump),
    )?;
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &decoded_recovery.identity.lifecycle_id.bytes(),
            decoded_recovery.identity.generation,
        ),
        Some(recovery.stored_bump),
    )?;
    require(
        decoded_recovery.kind == RuntimeCompartmentKindV1::Recovery
            && decoded_recovery
                .expected_account_balance_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == recovery_account.lamports()
            && receipt.liveness_policy_id() == decoded_policy.policy_id
            && receipt.liveness_lifecycle_id() == decoded_recovery.identity.lifecycle_id
            && receipt.recovery_compartment_account_id() == decoded_recovery.identity.account_id
            && runtime.recovery_compartment_account_id() == decoded_recovery.identity.account_id,
        ClutchError::MismatchedState,
    )
}

fn project_work_with_framed_accounts(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    recovery_balance_after: u64,
    root: AuthenticatedExternalRootV2,
    plan: FailureExternalTransitionPlanV2,
) -> Outcome<ExternalWorkMutationV2> {
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy_view = liveness_view(policy_account, policy.body, false);
    let recovery_view = liveness_view(recovery_account, recovery.body, true);
    authenticate_liveness_pdas(
        program_id,
        policy_account,
        policy.stored_bump,
        policy_view,
        recovery_account,
        recovery.stored_bump,
        recovery_view,
    )?;
    project_external_work_transition_v2(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        policy_view,
        recovery_view,
        recovery_balance_after,
        root,
        plan,
    )
    .map_err(map_external_error)
}

fn project_close_with_framed_accounts(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    root: AuthenticatedExternalRootV2,
    receipt: FailureRecoveryTerminalReceiptV2,
) -> Outcome<ExternalRecoveryCloseV2> {
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy_view = liveness_view(policy_account, policy.body, false);
    let recovery_view = liveness_view(recovery_account, recovery.body, true);
    authenticate_liveness_pdas(
        program_id,
        policy_account,
        policy.stored_bump,
        policy_view,
        recovery_account,
        recovery.stored_bump,
        recovery_view,
    )?;
    project_external_recovery_close_v2(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        policy_view,
        recovery_view,
        0,
        root,
        receipt,
    )
    .map_err(map_external_error)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_liveness_pdas(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    policy_bump: u8,
    policy_view: RuntimePersistedAccountViewV1<'_>,
    recovery_account: &AccountInfo<'_>,
    recovery_bump: u8,
    recovery_view: RuntimePersistedAccountViewV1<'_>,
) -> Outcome<()> {
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        policy_view,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let compartment = decode_runtime_compartment_account_v1(liveness_id(program_id), recovery_view)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        compartment.kind == RuntimeCompartmentKindV1::Recovery,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(policy_bump),
    )?;
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &compartment.identity.lifecycle_id.bytes(),
            compartment.identity.generation,
        ),
        Some(recovery_bump),
    )
}

fn apply_work_mutation(
    root: &AccountInfo<'_>,
    recovery: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    mutation: &ExternalWorkMutationV2,
    receipt: clutch_failure_policy_runtime::external_v2::FailureRecoveryWorkReceiptV2,
) -> Outcome<()> {
    let liveness = &mutation.liveness;
    require(
        !liveness.close_account
            && liveness.write_account_data
            && liveness.account_id == liveness_id(recovery.key)
            && liveness.account_balance_before == recovery.lamports()
            && liveness.account_balance_after
                == recovery
                    .lamports()
                    .checked_sub(receipt.scheduled_ceiling_lamports())
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let reward = receipt.exact_reward_lamports();
    let refund = receipt
        .scheduled_ceiling_lamports()
        .checked_sub(reward)
        .ok_or(ClutchError::Arithmetic)?;
    require_transfer(
        liveness,
        RuntimeTransferRoleV1::KeeperPayment,
        keeper,
        reward,
    )?;
    require_transfer(
        liveness,
        RuntimeTransferRoleV1::PayerWorkRefund,
        payer,
        refund,
    )?;
    require(
        liveness.transfers().len() == usize::from(reward != 0) + usize::from(refund != 0),
        ClutchError::MismatchedState,
    )?;
    require(
        mutation.semantic.root == id(root.key),
        ClutchError::MismatchedState,
    )?;
    let keeper_after = keeper
        .lamports()
        .checked_add(reward)
        .ok_or(ClutchError::Arithmetic)?;
    let payer_after = payer
        .lamports()
        .checked_add(refund)
        .ok_or(ClutchError::Arithmetic)?;
    let mut root_data = root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut recovery_data = recovery
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut recovery_lamports = recovery
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut keeper_lamports = keeper
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut payer_lamports = payer
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    root_data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&mutation.semantic.post_root_data);
    recovery_data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&liveness.post_account_data);
    **recovery_lamports = liveness.account_balance_after;
    **keeper_lamports = keeper_after;
    **payer_lamports = payer_after;
    Ok(())
}

fn apply_liveness_close(
    recovery: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
    transition: &RuntimeAtomicTransitionV1,
) -> Outcome<()> {
    require(
        transition.close_account
            && !transition.write_account_data
            && transition.account_id == liveness_id(recovery.key)
            && transition.account_balance_before == recovery.lamports()
            && transition.account_balance_after == 0,
        ClutchError::MismatchedState,
    )?;
    let payer_amount = transfer_amount(
        transition,
        RuntimeTransferRoleV1::PayerTerminalRefund,
        payer,
    )?;
    let sink_amount =
        transfer_amount(transition, RuntimeTransferRoleV1::NeutralTerminalSink, sink)?;
    require(
        transition.transfers().len()
            == usize::from(payer_amount != 0) + usize::from(sink_amount != 0),
        ClutchError::MismatchedState,
    )?;
    require(
        payer_amount
            .checked_add(sink_amount)
            .ok_or(ClutchError::Arithmetic)?
            == recovery.lamports(),
        ClutchError::MismatchedState,
    )?;
    let payer_after = payer
        .lamports()
        .checked_add(payer_amount)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after = sink
        .lamports()
        .checked_add(sink_amount)
        .ok_or(ClutchError::Arithmetic)?;
    {
        let mut recovery_lamports = recovery
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_lamports = payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **recovery_lamports = 0;
        **payer_lamports = payer_after;
        **sink_lamports = sink_after;
    }
    recovery
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    recovery.assign(&SYSTEM_PROGRAM_ID);
    Ok(())
}

fn write_root_poststate(
    root: &AccountInfo<'_>,
    mutation: &ExternalSemanticMutationV2,
) -> Outcome<()> {
    let mut data = root
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        data.len() == FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&mutation.post_root_data);
    Ok(())
}

fn require_failure_source_accounts(
    source: AuthenticatedSourceFailureJoinV1,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    require(accounts.len() >= 4, ClutchError::AccountCount)?;
    require_source_release_account(source.release, &accounts[0])?;
    require(
        accounts[1].key.to_bytes() == source.source.occurrence_account().bytes()
            && accounts[2].key.to_bytes() == source.source.result_or_absence_account().bytes()
            && accounts[3].key.to_bytes() == source.source.work_receipt_account().bytes(),
        ClutchError::MismatchedState,
    )
}

fn require_source_action(
    source: SourcePolicyHandoffJoinV1,
    common: RecoveryCommonV1,
    expected_handoff_id: [u8; 32],
    source_market_instance_v2_id: [u8; 32],
) -> Outcome<()> {
    require(
        source.handoff_id().bytes() == expected_handoff_id
            && source.failure_policy_binding_id().bytes() == common.binding_id
            && source.generation() == common.generation
            && source_market_instance_v2_id == common.market_instance_v2_id,
        ClutchError::MismatchedState,
    )
}

fn require_success_source_accounts(
    source: AuthenticatedSourceSuccessJoinV1,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    require(accounts.len() >= 4, ClutchError::AccountCount)?;
    require_source_release_account(source.release, &accounts[0])?;
    require(
        accounts[1].key.to_bytes() == source.source.occurrence_account().bytes()
            && accounts[2].key.to_bytes() == source.source.result_or_absence_account().bytes()
            && accounts[3].key.to_bytes() == source.source.work_receipt_account().bytes(),
        ClutchError::MismatchedState,
    )
}

fn require_source_release_account(
    release: AuthenticatedSourceReleaseV1,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    require(
        release.account().bytes() == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )
}

fn require_current_after(
    accounts: &[AccountInfo<'_>],
    clock_index: usize,
    source_clock: ClockSnapshotV1,
) -> Outcome<()> {
    let current = authenticate_clock_snapshot_v1(
        accounts
            .get(clock_index)
            .ok_or(Refusal::Adapter(ClutchError::AccountCount))?,
    )?;
    require(
        current.slot >= source_clock.slot && current.unix_timestamp >= source_clock.unix_timestamp,
        ClutchError::MismatchedState,
    )
}

fn require_transfer(
    transition: &RuntimeAtomicTransitionV1,
    role: RuntimeTransferRoleV1,
    destination: &AccountInfo<'_>,
    expected_lamports: u64,
) -> Outcome<()> {
    let actual = transfer_amount(transition, role, destination)?;
    require(actual == expected_lamports, ClutchError::MismatchedState)
}

fn transfer_amount(
    transition: &RuntimeAtomicTransitionV1,
    role: RuntimeTransferRoleV1,
    destination: &AccountInfo<'_>,
) -> Outcome<u64> {
    let mut found = None;
    for transfer in transition.transfers() {
        if transfer.role == role {
            require(found.is_none(), ClutchError::MismatchedState)?;
            require(
                transfer.destination == liveness_id(destination.key),
                ClutchError::MismatchedState,
            )?;
            found = Some(transfer.lamports);
        }
    }
    Ok(found.unwrap_or(0))
}

fn transfer_from_signer<'a>(
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
    expected_destination_after: u64,
) -> Outcome<()> {
    let expected_payer_after = payer
        .lamports()
        .checked_sub(lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke(
        &transfer,
        &[payer.clone(), destination.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        destination.lamports() == expected_destination_after
            && payer.lamports() == expected_payer_after,
        ClutchError::AccountCreationFailed,
    )
}

fn allocate_assign_failure_root<'a>(
    program_id: &Pubkey,
    root: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    common: &RecoveryCommonV1,
    bump: u8,
) -> Outcome<()> {
    let generation = common.generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds = [
        seeds::SEED_FAILURE_EXTERNAL_ROOT,
        common.market_instance_v2_id.as_slice(),
        generation.as_slice(),
        bump_seed.as_slice(),
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1),
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
        root.data_len() == FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1 && root.owner == program_id,
        ClutchError::AccountCreationFailed,
    )
}

fn allocate_assign_failure_tombstone<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
    bump: u8,
) -> Outcome<()> {
    let generation_bytes = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds = [
        seeds::SEED_FAILURE_REPLAY_TOMBSTONE,
        market_instance_v2_id.as_slice(),
        generation_bytes.as_slice(),
        bump_seed.as_slice(),
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        account.data_len() == FAILURE_REPLAY_TOMBSTONE_ACCOUNT_BYTES_V1
            && account.owner == program_id,
        ClutchError::AccountCreationFailed,
    )
}

fn liveness_view<'a>(
    account: &AccountInfo<'_>,
    body: &'a [u8],
    writable: bool,
) -> RuntimePersistedAccountViewV1<'a> {
    RuntimePersistedAccountViewV1 {
        account_id: liveness_id(account.key),
        owner_program_id: liveness_id(account.owner),
        lamports: account.lamports(),
        data: body,
        writable,
    }
}

fn map_external_error(error: ExternalAdapterErrorV2) -> Refusal {
    let code = match error {
        ExternalAdapterErrorV2::WrongLength => ClutchError::WrongDataLength,
        ExternalAdapterErrorV2::WrongOwner => ClutchError::WrongProgramOwner,
        ExternalAdapterErrorV2::NotWritable => ClutchError::NotWritable,
        ExternalAdapterErrorV2::RootNotZero => ClutchError::AlreadyInitialized,
        ExternalAdapterErrorV2::WrongRoot => ClutchError::WrongPda,
        ExternalAdapterErrorV2::BadMagic
        | ExternalAdapterErrorV2::BadVersion
        | ExternalAdapterErrorV2::NonCanonicalReserved => ClutchError::NonCanonical,
        ExternalAdapterErrorV2::Failure(_)
        | ExternalAdapterErrorV2::Liveness(_)
        | ExternalAdapterErrorV2::RootRentMismatch
        | ExternalAdapterErrorV2::ReceiptMismatch
        | ExternalAdapterErrorV2::DigestMismatch
        | ExternalAdapterErrorV2::WrongTransitionKind
        | ExternalAdapterErrorV2::RootRentUnderfunded => ClutchError::MismatchedState,
    };
    Refusal::Adapter(code)
}

fn id(key: &Pubkey) -> AccountId {
    AccountId::from_bytes(key.to_bytes())
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Outcome<[u8; N]> {
    let end = offset.checked_add(N).ok_or(ClutchError::Arithmetic)?;
    let source = input
        .get(offset..end)
        .ok_or(Refusal::Adapter(ClutchError::WrongDataLength))?;
    let mut output = [0; N];
    output.copy_from_slice(source);
    Ok(output)
}

trait CommonRuntimeJoin {
    fn validate_for_runtime(self, runtime: FailureRuntimeExternalV2) -> Outcome<()>;
}

impl CommonRuntimeJoin for RecoveryCommonV1 {
    fn validate_for_runtime(self, runtime: FailureRuntimeExternalV2) -> Outcome<()> {
        runtime
            .check()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            runtime.binding_id().bytes() == self.binding_id
                && runtime.binding().market_instance_id.bytes() == self.market_instance_v2_id
                && runtime.binding().generation == self.generation
                && runtime.transition_nonce() == self.expected_transition_nonce,
            if runtime.transition_nonce() != self.expected_transition_nonce {
                ClutchError::Replay
            } else {
                ClutchError::MismatchedState
            },
        )
    }
}

const _: () = assert!(FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1 > FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2);
const _: () =
    assert!(FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1 > FAILURE_LIVENESS_POLICY_BODY_BYTES_V1);
const _: () =
    assert!(FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1 > FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1);
const _: () = assert!(INITIALIZE_FAILURE_ROOT_METAS_V1.len() <= 32);
