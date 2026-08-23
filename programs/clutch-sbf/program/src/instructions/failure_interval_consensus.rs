// SPDX-License-Identifier: AGPL-3.0-or-later
//! Disabled authenticated account boundary for Failure interval consensus.
//!
//! This module does not route an instruction. It authenticates the dedicated
//! `0xab/v1` mutable work account and permanent `0xac/v1` replay account, then
//! mints only the private authorities required to restore Failure state and
//! Product's verified interval capability. Begin remains unavailable until a
//! shared MarketLifecycle owner supplies exact market-scoped capitalization.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_failure_policy_runtime::external_v2::FailureRecoveryWorkReceiptIdV2;
use clutch_failure_policy_runtime::interval_consensus_v1::{
    project_failure_interval_consensus_replay_id_v1, restore_failure_interval_consensus_state_v1,
    AuthenticatedFailureIntervalConsensusStateV1, FailureIntervalConsensusAccountIdV1,
    FailureIntervalConsensusBindingIdV1, FailureIntervalConsensusCloseAuthorizationIdV1,
    FailureIntervalConsensusFundingReceiptIdV1, FailureIntervalConsensusPersistedFactsV1,
    FailureIntervalConsensusPhaseV1, FailureIntervalConsensusReplayReceiptIdV1,
    FailureIntervalConsensusReplayV1, FailureIntervalConsensusResolutionReceiptIdV1,
    FailureIntervalConsensusStateV1, FailureIntervalConsensusTransitionReceiptIdV1,
};
use clutch_failure_policy_runtime::{Error as FailureError, FailurePolicyBindingId};
use clutch_product_series::{
    AuthenticatedQuantizedIntervalConsensusHistoryV1, FixedCodec,
    QuantizedIntervalConsensusCertificateV1Id, QuantizedIntervalConsensusRestorationV1,
    QuantizedIntervalConsensusWorkV1, QuantizedIntervalConsensusWorkV1Id,
};
use clutch_solana_layout::failure_interval_consensus::{
    FailureIntervalConsensusPhaseV1 as AccountPhaseV1, FailureIntervalConsensusReplayAccountV1,
    FailureIntervalConsensusWorkAccountV1, PRODUCT_INTERVAL_WORK_BODY_BYTES_V1,
};
use clutch_solana_layout::registry;
use clutch_source_plane_v3::ContentId as SourceContentId;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Private account-authenticated authority for one exact `0xab`/`0xac` pair.
///
/// There is no public constructor and no raw-ID restoration path. Every value
/// is minted by [`authenticate_failure_interval_consensus_accounts_v1`] after
/// owner, PDA, privilege, full-body, cross-account, balance, and Product work
/// authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFailureIntervalConsensusAccountsV1 {
    facts: FailureIntervalConsensusPersistedFactsV1,
    replay_receipt_id: FailureIntervalConsensusReplayReceiptIdV1,
}

impl AuthenticatedFailureIntervalConsensusAccountsV1 {
    /// Exact authenticated semantic facts.
    pub const fn facts(self) -> FailureIntervalConsensusPersistedFactsV1 {
        self.facts
    }

    /// Exact permanent replay postimage identity.
    pub const fn replay_receipt_id(self) -> FailureIntervalConsensusReplayReceiptIdV1 {
        self.replay_receipt_id
    }
}

impl AuthenticatedFailureIntervalConsensusStateV1
    for AuthenticatedFailureIntervalConsensusAccountsV1
{
    fn authenticate_interval_consensus_state(
        &self,
        expected: FailureIntervalConsensusPersistedFactsV1,
        replay_receipt_id: FailureIntervalConsensusReplayReceiptIdV1,
    ) -> core::result::Result<(), FailureError> {
        if expected == self.facts && replay_receipt_id == self.replay_receipt_id {
            Ok(())
        } else {
            Err(FailureError::BindingMismatch)
        }
    }
}

impl AuthenticatedQuantizedIntervalConsensusHistoryV1
    for AuthenticatedFailureIntervalConsensusAccountsV1
{
    fn authenticate_complete_history(
        &self,
        expected: QuantizedIntervalConsensusRestorationV1,
    ) -> clutch_product_series::Result<()> {
        if self.facts.phase == FailureIntervalConsensusPhaseV1::Active
            && self.facts.checked_coordinates == self.facts.total_coordinates
            && self.facts.transition_nonce != 0
            && expected.work_id == self.facts.current_work_id
            && expected.market_instance_id == self.facts.market_instance_id
            && expected.source_interval_id.bytes() == self.facts.source_interval_id.bytes()
            && expected.interval_profile_id == self.facts.interval_profile_id
            && expected.checked_coordinates == self.facts.checked_coordinates
            && expected.transcript.bytes() == self.facts.current_transcript.bytes()
            && expected
                .certificate_id
                .bytes()
                .iter()
                .any(|byte| *byte != 0)
        {
            Ok(())
        } else {
            Err(clutch_product_series::Error::UnauthenticatedAuthority)
        }
    }
}

/// Persist an exact pure poststate into the canonical mutable work and
/// permanent replay accounts. Account creation/funding and any liveness or
/// Failure-root write remain separate operations in the same outer batch.
pub fn persist_failure_interval_consensus_accounts_v1(
    program_id: &Pubkey,
    work_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    state: FailureIntervalConsensusStateV1,
    replay: FailureIntervalConsensusReplayV1,
    product_work: QuantizedIntervalConsensusWorkV1,
) -> Outcome<()> {
    require_distinct(&[work_account.clone(), replay_account.clone()])?;
    authenticate_metadata(
        program_id,
        work_account,
        registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        true,
    )?;
    authenticate_metadata(
        program_id,
        replay_account,
        registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        true,
    )?;
    let facts = state.persisted_facts();
    let derived_replay_id = project_failure_interval_consensus_replay_id_v1(facts)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let product_work_id = product_work
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        facts.work_account.bytes() == work_account.key.to_bytes()
            && facts.replay_account.bytes() == replay_account.key.to_bytes()
            && facts.current_work_id == product_work_id
            && facts.current_transcript.bytes() == product_work.transcript().bytes()
            && facts.checked_coordinates == product_work.checked_coordinates()
            && facts.total_coordinates
                == product_work
                    .total_coordinates()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && facts.market_instance_id == product_work.market_instance_id()
            && facts.source_interval_id.bytes() == product_work.source_interval_id().bytes()
            && facts.interval_profile_id == product_work.interval_profile_id()
            && replay.id() == derived_replay_id
            && replay.binding_id() == facts.binding_id
            && replay.transition_nonce() == facts.transition_nonce
            && replay.phase() == facts.phase
            && replay_account.lamports() == facts.replay_preserved_lamports
            && work_account.lamports() >= facts.work_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let work_pda = seeds::failure_interval_consensus_work_pda(
        program_id,
        &facts.market_instance_id.bytes(),
        facts.generation,
    );
    let replay_pda = seeds::failure_interval_consensus_replay_pda(
        program_id,
        &facts.market_instance_id.bytes(),
        facts.generation,
    );
    expect_pda(work_account.key, work_pda, None)?;
    expect_pda(replay_account.key, replay_pda, None)?;

    let mut product_work_body = [0; PRODUCT_INTERVAL_WORK_BODY_BYTES_V1];
    product_work
        .encode_into(&mut product_work_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let work_record = FailureIntervalConsensusWorkAccountV1 {
        bump: work_pda.1,
        phase: account_phase(facts.phase),
        generation: facts.generation,
        transition_nonce: facts.transition_nonce,
        accepted_recovery_progress_total: facts.accepted_recovery_progress_total,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        replay_rent_principal_lamports: facts.replay_rent_principal_lamports,
        interval_binding_id: facts.binding_id.bytes(),
        failure_policy_binding_id: facts.failure_policy_binding_id.bytes(),
        funding_receipt_id: facts.funding_receipt_id.bytes(),
        replay_account: facts.replay_account.bytes(),
        rent_refund_owner: facts.rent_refund_owner.bytes(),
        neutral_sink: facts.neutral_sink.bytes(),
        last_transition_receipt_id: facts.last_transition_receipt_id.bytes(),
        last_liveness_receipt_id: facts.last_liveness_receipt_id.bytes(),
        resolution_receipt_id: facts.resolution_receipt_id.bytes(),
        close_authorization_id: facts.close_authorization_id.bytes(),
        product_work_body,
    };
    let replay_record = FailureIntervalConsensusReplayAccountV1 {
        bump: replay_pda.1,
        phase: account_phase(facts.phase),
        generation: facts.generation,
        transition_nonce: facts.transition_nonce,
        preserved_lamports: facts.replay_preserved_lamports,
        interval_binding_id: facts.binding_id.bytes(),
        failure_policy_binding_id: facts.failure_policy_binding_id.bytes(),
        source_success_handoff_id: facts.source_success_handoff_id.bytes(),
        work_account: facts.work_account.bytes(),
        initial_work_id: facts.initial_work_id.bytes(),
        current_work_id: facts.current_work_id.bytes(),
        current_transcript: facts.current_transcript.bytes(),
        last_transition_receipt_id: facts.last_transition_receipt_id.bytes(),
        last_liveness_receipt_id: facts.last_liveness_receipt_id.bytes(),
        certificate_id: facts.certificate_id.bytes(),
        resolution_receipt_id: facts.resolution_receipt_id.bytes(),
        close_authorization_id: facts.close_authorization_id.bytes(),
    };
    {
        let mut data = work_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        work_record
            .encode_into(output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        replay_record
            .encode_into(output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    Ok(())
}

/// Authenticate one existing dedicated work/replay pair and restore its pure
/// Failure state. This never mutates either account.
pub fn authenticate_failure_interval_consensus_accounts_v1(
    program_id: &Pubkey,
    work_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    require_writable: bool,
) -> Outcome<(
    AuthenticatedFailureIntervalConsensusAccountsV1,
    FailureIntervalConsensusStateV1,
    FailureIntervalConsensusReplayV1,
    QuantizedIntervalConsensusWorkV1,
)> {
    require_distinct(&[work_account.clone(), replay_account.clone()])?;
    authenticate_metadata(
        program_id,
        work_account,
        registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        require_writable,
    )?;
    authenticate_metadata(
        program_id,
        replay_account,
        registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        require_writable,
    )?;

    let work_data = work_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let work_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = work_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let work_record = FailureIntervalConsensusWorkAccountV1::decode(work_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let product_work = QuantizedIntervalConsensusWorkV1::decode(&work_record.product_work_body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;

    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay_bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] =
        replay_data
            .as_ref()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let replay_record = FailureIntervalConsensusReplayAccountV1::decode(replay_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let product_work_id = product_work
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let product_total_coordinates = product_work
        .total_coordinates()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require(
        work_record.generation == replay_record.generation
            && work_record.transition_nonce == replay_record.transition_nonce
            && work_record.interval_binding_id == replay_record.interval_binding_id
            && work_record.failure_policy_binding_id == replay_record.failure_policy_binding_id
            && work_record.replay_account == replay_account.key.to_bytes()
            && replay_record.work_account == work_account.key.to_bytes()
            && work_record.last_transition_receipt_id == replay_record.last_transition_receipt_id
            && work_record.last_liveness_receipt_id == replay_record.last_liveness_receipt_id
            && work_record.resolution_receipt_id == replay_record.resolution_receipt_id
            && work_record.close_authorization_id == replay_record.close_authorization_id
            && map_phase(work_record.phase) == map_phase(replay_record.phase)
            && product_work_id.bytes() == replay_record.current_work_id
            && product_work.transcript().bytes() == replay_record.current_transcript
            && replay_record.preserved_lamports == replay_account.lamports()
            && work_account.lamports() >= work_record.work_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        work_account.key,
        seeds::failure_interval_consensus_work_pda(
            program_id,
            &product_work.market_instance_id().bytes(),
            work_record.generation,
        ),
        Some(work_record.bump),
    )?;
    expect_pda(
        replay_account.key,
        seeds::failure_interval_consensus_replay_pda(
            program_id,
            &product_work.market_instance_id().bytes(),
            work_record.generation,
        ),
        Some(replay_record.bump),
    )?;

    let phase = map_phase(work_record.phase);
    let facts = FailureIntervalConsensusPersistedFactsV1 {
        phase,
        binding_id: FailureIntervalConsensusBindingIdV1::from_bytes(
            work_record.interval_binding_id,
        ),
        failure_policy_binding_id: FailurePolicyBindingId::from_bytes(
            work_record.failure_policy_binding_id,
        ),
        market_instance_id: product_work.market_instance_id(),
        generation: work_record.generation,
        source_success_handoff_id: SourceContentId::from_bytes(
            replay_record.source_success_handoff_id,
        ),
        source_interval_id: SourceContentId::from_bytes(product_work.source_interval_id().bytes()),
        interval_profile_id: product_work.interval_profile_id(),
        funding_receipt_id: FailureIntervalConsensusFundingReceiptIdV1::from_bytes(
            work_record.funding_receipt_id,
        ),
        work_account: FailureIntervalConsensusAccountIdV1::from_bytes(work_account.key.to_bytes()),
        replay_account: FailureIntervalConsensusAccountIdV1::from_bytes(
            replay_account.key.to_bytes(),
        ),
        rent_refund_owner: FailureIntervalConsensusAccountIdV1::from_bytes(
            work_record.rent_refund_owner,
        ),
        neutral_sink: FailureIntervalConsensusAccountIdV1::from_bytes(work_record.neutral_sink),
        work_rent_principal_lamports: work_record.work_rent_principal_lamports,
        replay_rent_principal_lamports: work_record.replay_rent_principal_lamports,
        replay_preserved_lamports: replay_record.preserved_lamports,
        initial_work_id: QuantizedIntervalConsensusWorkV1Id::from_bytes(
            replay_record.initial_work_id,
        ),
        current_work_id: QuantizedIntervalConsensusWorkV1Id::from_bytes(
            replay_record.current_work_id,
        ),
        current_transcript: SourceContentId::from_bytes(replay_record.current_transcript),
        checked_coordinates: product_work.checked_coordinates(),
        total_coordinates: product_total_coordinates,
        accepted_recovery_progress_total: work_record.accepted_recovery_progress_total,
        transition_nonce: work_record.transition_nonce,
        last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1::from_bytes(
            work_record.last_transition_receipt_id,
        ),
        last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2::from_bytes(
            work_record.last_liveness_receipt_id,
        ),
        certificate_id: QuantizedIntervalConsensusCertificateV1Id::from_bytes(
            replay_record.certificate_id,
        ),
        resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1::from_bytes(
            replay_record.resolution_receipt_id,
        ),
        close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1::from_bytes(
            replay_record.close_authorization_id,
        ),
    };
    let replay_receipt_id = project_failure_interval_consensus_replay_id_v1(facts)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authenticated = AuthenticatedFailureIntervalConsensusAccountsV1 {
        facts,
        replay_receipt_id,
    };
    let (state, replay) = restore_failure_interval_consensus_state_v1(&authenticated, facts)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((authenticated, state, replay, product_work))
}

fn authenticate_metadata(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == expected_len,
        ClutchError::WrongDataLength,
    )
}

const fn map_phase(phase: AccountPhaseV1) -> FailureIntervalConsensusPhaseV1 {
    match phase {
        AccountPhaseV1::Active => FailureIntervalConsensusPhaseV1::Active,
        AccountPhaseV1::Resolved => FailureIntervalConsensusPhaseV1::Resolved,
        AccountPhaseV1::Closed => FailureIntervalConsensusPhaseV1::Closed,
    }
}

const fn account_phase(phase: FailureIntervalConsensusPhaseV1) -> AccountPhaseV1 {
    match phase {
        FailureIntervalConsensusPhaseV1::Active => AccountPhaseV1::Active,
        FailureIntervalConsensusPhaseV1::Resolved => AccountPhaseV1::Resolved,
        FailureIntervalConsensusPhaseV1::Closed => AccountPhaseV1::Closed,
    }
}

const _: () = assert!(
    PRODUCT_INTERVAL_WORK_BODY_BYTES_V1
        == clutch_product_series::QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1
);
