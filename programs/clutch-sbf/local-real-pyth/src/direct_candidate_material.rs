//! Chain-derived unsigned material for current Direct actions 5 through 7.
//!
//! The browser never supplies candidate fills, semantic identities, account
//! roles, refund recipients, liveness ordinals, or postimages. This module
//! hostile-decodes one finalized b1/v2+b2+b3 snapshot, derives one deterministic
//! exact valid pair candidate or the next verification transition, replays the same
//! pure owners used by SBF, and only then emits one release-bound unsigned
//! draft. Signing, blockhash acquisition, and submission are outside this
//! crate.

use crate::action_material::{
    chain_derived_direct_role_v2, finish_chain_derived_direct_material_v2,
    ActionFreshnessBoundaryV1, CanonicalActionMaterialErrorV1,
    CanonicalActionMaterialV1,
};
use crate::operatord::KeeperActionSelection;
use crate::rpc_index::{IndexedProgramRelease, ObservedRpcAccount, RpcCommitment};
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, ProtocolTransactionBuilder,
};
use crate::workflow_graph::ExplicitOperatorReleaseManifest;
use crate::workflow_graph::WorkflowLane;
use clutch_batch::direct_pair_v1::{
    verify_compact_direct_candidate_v1, DirectEconomicCandidateV1,
};
use clutch_batch::PartialPolicy;
use clutch_client_contract::direct_market::DirectMarketClientPayloadV1;
use clutch_direct_market_runtime::codec_v2::{
    authenticate_direct_root_transition_body_v2,
    decode_direct_action_replay_body_for_transition_v2,
    decode_direct_selection_body_for_transition_v2,
    encode_direct_action_replay_body_into_transition_v2,
    encode_direct_selection_body_into_transition_v2,
    write_direct_root_transition_body_v2, AuthenticatedDirectRootTransitionV2,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    begin_direct_candidate_verification_v2, bind_direct_candidate_work_batch_v2,
    prepare_direct_candidate_work_batch_v2, submit_direct_candidate_v2,
    verify_next_direct_candidate_v2, DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::{
    DirectHashBackendV1, DirectMarketActionV1, DirectRootPhaseV1,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1,
    RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
    RuntimeTransferRoleV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::direct_market_v1::{
    DirectActionReplayAccountV1, DirectSelectionAccountV1,
};
use clutch_solana_layout::direct_market_v2::DirectMarketRootAccountV2;
use clutch_solana_layout::direct_market_v1::DirectSubmitCandidatePayloadV1;
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, DIRECT_SELECTION_ACCOUNT_BYTES,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use std::str::FromStr;

const DIRECT_ROOT_SEED_V2: &[u8] = b"dc:direct-market-root:v2";
const DIRECT_REPLAY_SEED_V1: &[u8] = b"dc:direct-action-replay:v1";
const DIRECT_SELECTION_SEED_V1: &[u8] = b"dc:direct-selection:v1";

/// Exact finalized observations required by action 5.
#[derive(Clone, Copy, Debug)]
pub struct DirectCandidateSubmissionSnapshotV1<'a> {
    pub root: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub selection: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    /// Ordinary signer account selected by the operator deployment, not by a
    /// browser request. The returned transaction remains unsigned.
    pub submitter: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    /// Present only when the derived top-three transition evicts a bond owner.
    pub evicted_refund_owner: Option<&'a ObservedRpcAccount>,
}

/// Exact finalized observations required by actions 6 and 7.
#[derive(Clone, Copy, Debug)]
pub struct DirectCandidateVerificationSnapshotV1<'a> {
    pub root: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub selection: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    pub liveness_policy: &'a ObservedRpcAccount,
    pub candidate_compartment: &'a ObservedRpcAccount,
    /// Ordinary signer selected by the operator deployment. It may equal the
    /// immutable Candidate payer, as admitted by the onchain account grammar.
    pub keeper: &'a ObservedRpcAccount,
    pub candidate_payer: &'a ObservedRpcAccount,
}

/// Exact postimages and balance movements predicted before creating a draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCandidatePostimagesV1 {
    action: DirectMarketAction,
    prestate_id: [u8; 32],
    root_data_id: [u8; 32],
    replay_data_id: [u8; 32],
    selection_data_id: [u8; 32],
    candidate_data_id: Option<[u8; 32]>,
    selection_lamports_after: u64,
    candidate_lamports_after: Option<u64>,
    retained_bond_incoming_lamports: u64,
    evicted_bond_refund_owner: Option<Address>,
    evicted_bond_refund_lamports: u64,
    keeper_payment_lamports: u64,
    candidate_payer_refund_lamports: u64,
}

impl DirectCandidatePostimagesV1 {
    pub const fn action(&self) -> DirectMarketAction { self.action }
    pub const fn prestate_id(&self) -> [u8; 32] { self.prestate_id }
    pub const fn root_data_id(&self) -> [u8; 32] { self.root_data_id }
    pub const fn replay_data_id(&self) -> [u8; 32] { self.replay_data_id }
    pub const fn selection_data_id(&self) -> [u8; 32] { self.selection_data_id }
    pub const fn candidate_data_id(&self) -> Option<[u8; 32]> {
        self.candidate_data_id
    }
    pub const fn selection_lamports_after(&self) -> u64 {
        self.selection_lamports_after
    }
    pub const fn candidate_lamports_after(&self) -> Option<u64> {
        self.candidate_lamports_after
    }
    pub const fn retained_bond_incoming_lamports(&self) -> u64 {
        self.retained_bond_incoming_lamports
    }
    pub const fn evicted_bond_refund_owner(&self) -> Option<Address> {
        self.evicted_bond_refund_owner
    }
    pub const fn evicted_bond_refund_lamports(&self) -> u64 {
        self.evicted_bond_refund_lamports
    }
    pub const fn keeper_payment_lamports(&self) -> u64 {
        self.keeper_payment_lamports
    }
    pub const fn candidate_payer_refund_lamports(&self) -> u64 {
        self.candidate_payer_refund_lamports
    }
}

/// Opaque release-bound material plus independently inspectable postimages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCandidateActionMaterialV1 {
    canonical: CanonicalActionMaterialV1,
    postimages: DirectCandidatePostimagesV1,
}

impl DirectCandidateActionMaterialV1 {
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    pub const fn postimages(&self) -> &DirectCandidatePostimagesV1 { &self.postimages }
}

#[derive(Debug)]
pub(crate) struct DecodedDirectSnapshotV1 {
    pub(crate) root_bump: u8,
    pub(crate) replay_bump: u8,
    pub(crate) selection_bump: u8,
    pub(crate) state: DirectRootReplayTransitionV2,
    pub(crate) selection: clutch_direct_market_runtime::selection_v1::DirectSelectionV1,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OperatorSha256V1;

impl DirectHashBackendV1 for OperatorSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update(part);
        }
        hash.finalize().into()
    }
}

impl clutch_retirement::PositionV3Sha256Backend for OperatorSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, &[domain, body])
    }
}

impl clutch_retirement::ReplayV3HashBackend for OperatorSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

/// Derive action 5's only operator candidate from the frozen pair and price.
/// No fill, AON bit, refund recipient, sequence, or semantic ID is accepted.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_candidate_submission_v1(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectCandidateSubmissionSnapshotV1<'_>,
) -> Result<DirectCandidateActionMaterialV1, CanonicalActionMaterialErrorV1> {
    let accounts = [
        snapshot.root,
        snapshot.replay,
        snapshot.selection,
        snapshot.clock,
        snapshot.submitter,
        snapshot.system_program,
    ];
    authenticate_snapshot_set(release, freshness, &accounts)?;
    if let Some(refund) = snapshot.evicted_refund_owner {
        authenticate_snapshot_set(release, freshness, &[refund])?;
    }
    require_operator_signer(snapshot.submitter)?;
    require_system_program(snapshot.system_program)?;
    let observed_slot = decode_clock(snapshot.clock)?;
    let mut decoded = decode_direct_state(
        release,
        snapshot.root,
        snapshot.replay,
        snapshot.selection,
    )?;
    let sequence = decoded.state.replay().next_action_sequence();
    let mut prestate_accounts = accounts.to_vec();
    if let Some(refund) = snapshot.evicted_refund_owner {
        prestate_accounts.push(refund);
    }
    let prestate_id = direct_snapshot_id(
        release,
        DirectMarketAction::SubmitCandidate,
        sequence,
        &prestate_accounts,
    );
    authenticate_operator_cursor(
        operator_selection,
        decoded.state.root().generation(),
        DirectMarketAction::SubmitCandidate,
        sequence,
        prestate_id,
        [
            snapshot.root.address,
            snapshot.replay.address,
            snapshot.selection.address,
            Address::new_from_array(
                decoded.state.root().candidate_liveness().policy_account,
            ),
            Address::new_from_array(
                decoded.state.root().candidate_liveness().candidate_account,
            ),
        ],
    )?;
    let candidate = derive_deterministic_valid_candidate(&decoded.selection)?;
    let bond_before = decoded
        .state
        .root()
        .outstanding_candidate_bond_lamports(decoded.selection)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    require_selection_balance(decoded.selection, snapshot.selection.lamports, bond_before)?;
    let effects = submit_direct_candidate_v2(
        &mut decoded.state,
        &mut decoded.selection,
        sequence,
        observed_slot,
        candidate,
        snapshot.submitter.address.to_bytes(),
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let movement = effects.candidate_bond_movement;
    let (incoming, evicted, expected_refund) = match movement {
        Some(value) => (
            value.incoming_lamports,
            value.evicted_refund_lamports,
            if value.evicted_refund_lamports == 0 {
                None
            } else {
                Some(value.evicted_refund_recipient)
            },
        ),
        None => (0, 0, None),
    };
    if movement.is_some_and(|value| {
        value.incoming_payer != snapshot.submitter.address.to_bytes()
            || value.principal_before_lamports != bond_before
    }) || snapshot.submitter.lamports < incoming
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    match (expected_refund, snapshot.evicted_refund_owner) {
        (None, None) => {}
        (Some(expected), Some(observed))
            if observed.address.to_bytes() == expected && !observed.executable => {}
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    }
    require_submission_aliases(snapshot, expected_refund)?;
    let selection_lamports_after = snapshot
        .selection
        .lamports
        .checked_add(incoming)
        .and_then(|value| value.checked_sub(evicted))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let bond_after = decoded
        .state
        .root()
        .outstanding_candidate_bond_lamports(decoded.selection)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if movement.is_some_and(|value| value.principal_after_lamports != bond_after) {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_selection_balance(decoded.selection, selection_lamports_after, bond_after)?;

    let post = encode_direct_postimages(snapshot.root, snapshot.replay, snapshot.selection, &decoded)?;
    let payload = DirectMarketClientPayloadV1::submit_candidate(
        DirectSubmitCandidatePayloadV1 {
            candidate,
        },
    );
    let mut metas = vec![
        meta(snapshot.root, false, true),
        meta(snapshot.replay, false, true),
        meta(snapshot.selection, false, true),
        meta(snapshot.clock, false, false),
        meta(snapshot.submitter, true, true),
        meta(snapshot.system_program, false, false),
    ];
    let mut roles = vec![
        role("direct-root", snapshot.root, true, false),
        role("direct-replay", snapshot.replay, true, false),
        role("direct-selection", snapshot.selection, true, false),
        role("clock-sysvar", snapshot.clock, false, false),
        role("candidate-submitter", snapshot.submitter, true, true),
        role("system-program", snapshot.system_program, false, false),
    ];
    if let Some(refund) = snapshot.evicted_refund_owner {
        metas.push(meta(refund, false, true));
        roles.push(role("evicted-bond-refund-owner", refund, true, false));
    }
    let equation = ExactEquation {
        name: "selection retained-bond principal conservation".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(snapshot.selection.lamports) + u128::from(incoming),
        right: u128::from(selection_lamports_after) + u128::from(evicted),
    };
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::SubmitCandidate,
        sequence,
        metas,
        vec![snapshot.submitter.address],
        roles,
        vec![equation],
        payload,
    )?;
    Ok(DirectCandidateActionMaterialV1 {
        canonical,
        postimages: DirectCandidatePostimagesV1 {
            action: DirectMarketAction::SubmitCandidate,
            prestate_id,
            root_data_id: post.0,
            replay_data_id: post.1,
            selection_data_id: post.2,
            candidate_data_id: None,
            selection_lamports_after,
            candidate_lamports_after: None,
            retained_bond_incoming_lamports: incoming,
            evicted_bond_refund_owner: snapshot
                .evicted_refund_owner
                .map(|account| account.address),
            evicted_bond_refund_lamports: evicted,
            keeper_payment_lamports: 0,
            candidate_payer_refund_lamports: 0,
        },
    })
}

/// Derive exactly the next action 6 or 7 from persisted phase/cursor state and
/// stream its complete shared-Candidate work batch.
#[allow(clippy::too_many_arguments)]
pub fn construct_next_direct_candidate_verification_v1(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectCandidateVerificationSnapshotV1<'_>,
) -> Result<DirectCandidateActionMaterialV1, CanonicalActionMaterialErrorV1> {
    let accounts = [
        snapshot.root,
        snapshot.replay,
        snapshot.selection,
        snapshot.clock,
        snapshot.liveness_policy,
        snapshot.candidate_compartment,
        snapshot.keeper,
        snapshot.candidate_payer,
    ];
    authenticate_snapshot_set(release, freshness, &accounts)?;
    require_operator_signer(snapshot.keeper)?;
    let observed_slot = decode_clock(snapshot.clock)?;
    let mut decoded = decode_direct_state(
        release,
        snapshot.root,
        snapshot.replay,
        snapshot.selection,
    )?;
    let binding = decoded.state.root().candidate_liveness();
    authenticate_candidate_accounts(release, snapshot, binding)?;
    let sequence = decoded.state.replay().next_action_sequence();
    let (wire_action, semantic_action) = match decoded.state.root().phase() {
        DirectRootPhaseV1::SubmissionOpen => {
            begin_direct_candidate_verification_v2(
                &mut decoded.state,
                &mut decoded.selection,
                sequence,
                observed_slot,
                &OperatorSha256V1,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            (
                DirectMarketAction::BeginVerification,
                DirectMarketActionV1::BeginVerification,
            )
        }
        DirectRootPhaseV1::Verifying => {
            verify_next_direct_candidate_v2(
                &mut decoded.state,
                &mut decoded.selection,
                sequence,
                observed_slot,
                &OperatorSha256V1,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            (
                DirectMarketAction::VerifyCandidate,
                DirectMarketActionV1::VerifyCandidate,
            )
        }
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };
    let prestate_id = direct_snapshot_id(release, wire_action, sequence, &accounts);
    authenticate_operator_cursor(
        operator_selection,
        decoded.state.root().generation(),
        wire_action,
        sequence,
        prestate_id,
        [
            snapshot.root.address,
            snapshot.replay.address,
            snapshot.selection.address,
            snapshot.liveness_policy.address,
            snapshot.candidate_compartment.address,
        ],
    )?;
    let policy_id = sha256(&snapshot.liveness_policy.data);
    if policy_id != binding.policy_data_id {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let mut candidate_data = exact_candidate_bytes(snapshot.candidate_compartment)?;
    let candidate_pre_data_id = sha256(&candidate_data);
    let candidate_state = RuntimeCompartmentV1::decode(&candidate_data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if candidate_state.kind != RuntimeCompartmentKindV1::Candidate
        || candidate_state.identity.policy_id.bytes()
            != decoded.state.root().candidate_liveness_policy_id()
        || candidate_state.identity.lifecycle_id.bytes() != binding.global_lifecycle_id
        || candidate_state.identity.account_id.bytes() != binding.candidate_account
        || candidate_state.identity.owner.bytes() != binding.candidate_semantic_owner
        || candidate_state.identity.payer.bytes() != snapshot.candidate_payer.address.to_bytes()
        || candidate_state.identity.neutral_sink.bytes()
            != decoded.state.root().neutral_lamport_sink()
        || candidate_state.identity.generation != binding.candidate_generation
        || candidate_state.quote_schedule_id.bytes() != binding.candidate_quote_schedule_id
        || candidate_state.receipt_program_id.bytes() != binding.candidate_receipt_program_id
        || candidate_state.receipt_program_id.bytes() != release.program_id.to_bytes()
        || (decoded.state.replay().candidate_liveness_completed_calls() == 0
            && candidate_pre_data_id != binding.candidate_data_id)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let batch = prepare_direct_candidate_work_batch_v2(
        &decoded.state,
        Some(&decoded.selection),
        semantic_action,
        candidate_state.completed_calls,
        candidate_state.last_work_receipt_id.bytes(),
        candidate_pre_data_id,
        snapshot.keeper.address.to_bytes(),
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate_lamports_after = apply_candidate_batch(
        release,
        snapshot,
        binding,
        batch,
        &mut candidate_data,
    )?;
    bind_direct_candidate_work_batch_v2(&mut decoded.state, batch, &OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let post = encode_direct_postimages(snapshot.root, snapshot.replay, snapshot.selection, &decoded)?;
    let candidate_post_id = sha256(&candidate_data);
    let payload = DirectMarketClientPayloadV1::empty(wire_action)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let metas = vec![
        meta(snapshot.root, false, true),
        meta(snapshot.replay, false, true),
        meta(snapshot.selection, false, true),
        meta(snapshot.clock, false, false),
        meta(snapshot.liveness_policy, false, false),
        meta(snapshot.candidate_compartment, false, true),
        meta(snapshot.keeper, true, true),
        meta(snapshot.candidate_payer, false, true),
    ];
    let roles = vec![
        role("direct-root", snapshot.root, true, false),
        role("direct-replay", snapshot.replay, true, false),
        role("direct-selection", snapshot.selection, true, false),
        role("clock-sysvar", snapshot.clock, false, false),
        role("candidate-liveness-policy", snapshot.liveness_policy, false, false),
        role("candidate-compartment", snapshot.candidate_compartment, true, false),
        role("keeper", snapshot.keeper, true, true),
        role("candidate-immutable-payer", snapshot.candidate_payer, true, false),
    ];
    let equation = ExactEquation {
        name: "candidate work principal conservation".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(snapshot.candidate_compartment.lamports),
        right: u128::from(candidate_lamports_after)
            + u128::from(batch.total_keeper_payment_lamports())
            + u128::from(batch.total_payer_refund_lamports()),
    };
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        wire_action,
        sequence,
        metas,
        vec![snapshot.keeper.address],
        roles,
        vec![equation],
        payload,
    )?;
    Ok(DirectCandidateActionMaterialV1 {
        canonical,
        postimages: DirectCandidatePostimagesV1 {
            action: wire_action,
            prestate_id,
            root_data_id: post.0,
            replay_data_id: post.1,
            selection_data_id: post.2,
            candidate_data_id: Some(candidate_post_id),
            selection_lamports_after: snapshot.selection.lamports,
            candidate_lamports_after: Some(candidate_lamports_after),
            retained_bond_incoming_lamports: 0,
            evicted_bond_refund_owner: None,
            evicted_bond_refund_lamports: 0,
            keeper_payment_lamports: batch.total_keeper_payment_lamports(),
            candidate_payer_refund_lamports: batch.total_payer_refund_lamports(),
        },
    })
}

pub(crate) fn decode_direct_state(
    release: &IndexedProgramRelease,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
    selection_account: &ObservedRpcAccount,
) -> Result<DecodedDirectSnapshotV1, CanonicalActionMaterialErrorV1> {
    for account in [root_account, replay_account, selection_account] {
        if account.owner != release.program_id || account.executable {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    let root_frame = DirectMarketRootAccountV2::decode(&root_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let root = authenticate_direct_root_transition_body_v2(
        root_frame.semantic_body(),
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    authenticate_root_address(release.program_id, root_account, root_frame.bump(), &root)?;
    let replay_bytes = <&[u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES]>::try_from(
        replay_account.data.as_slice(),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_frame = DirectActionReplayAccountV1::decode(replay_bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay = decode_direct_action_replay_body_for_transition_v2(
        replay_frame.semantic_body(),
        &root,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected_replay, replay_bump) = Address::find_program_address(
        &[DIRECT_REPLAY_SEED_V1, root_account.address.as_ref()],
        &release.program_id,
    );
    if replay_account.address != expected_replay
        || replay_frame.bump() != replay_bump
        || replay_account.address.to_bytes() != root.action_replay_account()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(replay.rent(), replay_account.lamports)?;
    let selection_bytes = <&[u8; DIRECT_SELECTION_ACCOUNT_BYTES]>::try_from(
        selection_account.data.as_slice(),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let selection_frame = DirectSelectionAccountV1::decode(selection_bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let selection = decode_direct_selection_body_for_transition_v2(
        selection_frame.semantic_body(),
        &root,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected_selection, selection_bump) = Address::find_program_address(
        &[DIRECT_SELECTION_SEED_V1, root_account.address.as_ref()],
        &release.program_id,
    );
    if selection_account.address != expected_selection
        || selection_frame.bump() != selection_bump
        || selection_account.address.to_bytes() != root.selection_account()
        || selection.account() != selection_account.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(root.root_rent(), root_account.lamports)?;
    require_rent(selection.rent(), selection_account.lamports)?;
    let state = DirectRootReplayTransitionV2::authenticate(root, replay)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(DecodedDirectSnapshotV1 {
        root_bump: root_frame.bump(),
        replay_bump,
        selection_bump,
        state,
        selection,
    })
}

fn authenticate_root_address(
    program: Address,
    account: &ObservedRpcAccount,
    stored_bump: u8,
    root: &AuthenticatedDirectRootTransitionV2,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let generation = root.generation().to_le_bytes();
    let (expected, bump) = Address::find_program_address(
        &[
            DIRECT_ROOT_SEED_V2,
            &root.market_instance_id(),
            &generation,
        ],
        &program,
    );
    if account.address != expected
        || bump != stored_bump
        || account.address.to_bytes() != root.direct_root_account()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn derive_deterministic_valid_candidate(
    selection: &clutch_direct_market_runtime::selection_v1::DirectSelectionV1,
) -> Result<DirectEconomicCandidateV1, CanonicalActionMaterialErrorV1> {
    let book = selection.book();
    if book.len != 2 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let first = book.orders[0];
    let second = book.orders[1];
    let maximum = first.quantity.min(second.quantity);
    let fixed = match (first.partial_policy, second.partial_policy) {
        (PartialPolicy::AllOrNone, PartialPolicy::AllOrNone) => {
            if first.quantity != second.quantity {
                return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
            }
            Some(first.quantity)
        }
        (PartialPolicy::AllOrNone, PartialPolicy::Allow) => Some(first.quantity),
        (PartialPolicy::Allow, PartialPolicy::AllOrNone) => Some(second.quantity),
        (PartialPolicy::Allow, PartialPolicy::Allow) => None,
    };
    let outcome = selected_single_egg_outcome(selection.domain(), &first.coefficients)?;
    if selected_single_egg_outcome(selection.domain(), &second.coefficients)? != outcome {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let price = selection.price().prices[usize::from(outcome)];
    let scale = selection.domain().price_scale;
    let divisor = greatest_common_divisor(price, scale);
    let step = scale
        .checked_div(divisor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let fill = fixed.unwrap_or_else(|| maximum - (maximum % step));
    if fill == 0
        || fill > maximum
        || fill < first.minimum_fill
        || fill < second.minimum_fill
        || fill % step != 0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let honored_aon_mask = (if first.partial_policy == PartialPolicy::AllOrNone { 1 } else { 0 })
        | (if second.partial_policy == PartialPolicy::AllOrNone { 2 } else { 0 });
    let candidate = DirectEconomicCandidateV1 {
        fills: [fill, fill],
        honored_aon_mask,
    };
    verify_compact_direct_candidate_v1(
        selection.domain(),
        selection.book(),
        selection.price(),
        candidate,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(candidate)
}

fn selected_single_egg_outcome(
    domain: &clutch_batch::relation_v2::EconomicDomainV2,
    coefficients: &[u64; 16],
) -> Result<u8, CanonicalActionMaterialErrorV1> {
    let mut selected = None;
    for (index, coefficient) in coefficients.iter().enumerate() {
        if index < usize::from(domain.outcome_count) {
            match *coefficient {
                0 => {}
                1 if selected.is_none() => {
                    selected = Some(
                        u8::try_from(index)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
                    );
                }
                _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
            }
        } else if *coefficient != 0 {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    selected.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)
}

const fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn authenticate_candidate_accounts(
    release: &IndexedProgramRelease,
    snapshot: DirectCandidateVerificationSnapshotV1<'_>,
    binding: clutch_direct_market_runtime::liveness_v1::DirectCandidateLivenessBindingV1,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if snapshot.liveness_policy.address.to_bytes() != binding.policy_account
        || snapshot.liveness_policy.owner != release.program_id
        || snapshot.liveness_policy.executable
        || snapshot.liveness_policy.data.len() != RUNTIME_LIVENESS_POLICY_BYTES_V1
        || snapshot.candidate_compartment.address.to_bytes() != binding.candidate_account
        || snapshot.candidate_compartment.owner != release.program_id
        || snapshot.candidate_compartment.executable
        || snapshot.candidate_compartment.data.len() != RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
        || snapshot.keeper.executable
        || snapshot.candidate_payer.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let fixed = [
        snapshot.root.address,
        snapshot.replay.address,
        snapshot.selection.address,
        snapshot.clock.address,
        snapshot.liveness_policy.address,
        snapshot.candidate_compartment.address,
    ];
    for (index, address) in fixed.iter().enumerate() {
        if fixed[..index].contains(address)
            || *address == snapshot.keeper.address
            || *address == snapshot.candidate_payer.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn apply_candidate_batch(
    release: &IndexedProgramRelease,
    snapshot: DirectCandidateVerificationSnapshotV1<'_>,
    binding: clutch_direct_market_runtime::liveness_v1::DirectCandidateLivenessBindingV1,
    batch: clutch_direct_market_runtime::liveness_v1::DirectCandidateWorkBatchV1,
    candidate_data: &mut [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let expected_program = LivenessId::from_bytes(release.program_id.to_bytes());
    let expected_policy = LivenessId::from_bytes(snapshot.liveness_policy.address.to_bytes());
    let mut balance = snapshot.candidate_compartment.lamports;
    let mut keeper_total = 0u64;
    let mut payer_total = 0u64;
    for index in 0..batch.receipt_count() {
        let receipt = batch
            .receipt(index, binding, &OperatorSha256V1)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let after = balance
            .checked_sub(receipt.call_ceiling_lamports())
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(
                binding_policy_id(snapshot.candidate_compartment)?,
            ),
            lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
            quote_schedule_id: LivenessId::from_bytes(binding.candidate_quote_schedule_id),
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            keeper: LivenessId::from_bytes(snapshot.keeper.address.to_bytes()),
            generation: binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
            keeper_payment_lamports: receipt.keeper_payment_lamports(),
            flags: 0,
        };
        let transition = plan_runtime_transition_v1(
            expected_program,
            expected_policy,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy,
                owner_program_id: expected_program,
                lamports: snapshot.liveness_policy.lamports,
                data: &snapshot.liveness_policy.data,
                writable: false,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(
                    snapshot.candidate_compartment.address.to_bytes(),
                ),
                owner_program_id: expected_program,
                lamports: balance,
                data: candidate_data,
                writable: true,
            },
            intent,
            Some(RuntimeReceiptObservationV1 {
                receipt_account_id: LivenessId::from_bytes(snapshot.replay.address.to_bytes()),
                receipt_account_owner_program_id: expected_program,
                receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
                receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
                compartment_kind: RuntimeCompartmentKindV1::Candidate,
                semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
                lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
                quote_schedule_id: LivenessId::from_bytes(
                    binding.candidate_quote_schedule_id,
                ),
                generation: binding.candidate_generation,
                call_ordinal: receipt.call_ordinal(),
                call_ceiling_lamports: receipt.call_ceiling_lamports(),
            }),
            after,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if !transition.write_account_data
            || transition.close_account
            || transition.account_balance_before != balance
            || transition.account_balance_after != after
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        for transfer in transition.transfers() {
            match transfer.role {
                RuntimeTransferRoleV1::KeeperPayment
                    if transfer.destination.bytes() == snapshot.keeper.address.to_bytes() => {
                    keeper_total = keeper_total
                        .checked_add(transfer.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                RuntimeTransferRoleV1::PayerWorkRefund
                    if transfer.destination.bytes()
                        == snapshot.candidate_payer.address.to_bytes() => {
                    payer_total = payer_total
                        .checked_add(transfer.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
            }
        }
        candidate_data.copy_from_slice(&transition.post_account_data);
        balance = after;
    }
    if keeper_total != batch.total_keeper_payment_lamports()
        || payer_total != batch.total_payer_refund_lamports()
        || snapshot
            .candidate_compartment
            .lamports
            .checked_sub(balance)
            != Some(batch.total_call_ceiling_lamports())
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(balance)
}

fn binding_policy_id(
    candidate: &ObservedRpcAccount,
) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
    RuntimeCompartmentV1::decode(&candidate.data)
        .map(|state| state.identity.policy_id.bytes())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn encode_direct_postimages(
    root_pre: &ObservedRpcAccount,
    replay_pre: &ObservedRpcAccount,
    selection_pre: &ObservedRpcAccount,
    decoded: &DecodedDirectSnapshotV1,
) -> Result<([u8; 32], [u8; 32], [u8; 32]), CanonicalActionMaterialErrorV1> {
    let mut root = root_pre.data.clone();
    let mut replay = replay_pre.data.clone();
    let mut selection = selection_pre.data.clone();
    if root.len() != DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2
        || replay.len() != DIRECT_ACTION_REPLAY_ACCOUNT_BYTES
        || selection.len() != DIRECT_SELECTION_ACCOUNT_BYTES
        || root[2] != decoded.root_bump
        || replay[2] != decoded.replay_bump
        || selection[2] != decoded.selection_bump
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    write_direct_root_transition_body_v2(
        decoded.state.root(),
        &mut root[4..],
        &OperatorSha256V1,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    encode_direct_action_replay_body_into_transition_v2(
        decoded.state.replay(),
        decoded.state.root(),
        &mut replay[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    encode_direct_selection_body_into_transition_v2(
        decoded.selection,
        decoded.state.root(),
        &mut selection[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok((sha256(&root), sha256(&replay), sha256(&selection)))
}

fn require_selection_balance(
    selection: clutch_direct_market_runtime::selection_v1::DirectSelectionV1,
    observed: u64,
    bond_principal: u64,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let rent = selection.rent();
    let minimum = rent
        .principal_lamports
        .checked_add(rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if observed < minimum {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

pub(crate) fn require_rent(
    rent: clutch_direct_market_runtime::DirectRentOwnerV1,
    observed: u64,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let minimum = rent
        .principal_lamports
        .checked_add(rent.donation_floor_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if observed < minimum {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

fn require_submission_aliases(
    snapshot: DirectCandidateSubmissionSnapshotV1<'_>,
    expected_refund: Option<[u8; 32]>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let fixed = [
        snapshot.root.address,
        snapshot.replay.address,
        snapshot.selection.address,
        snapshot.clock.address,
    ];
    for (index, address) in fixed.iter().enumerate() {
        if fixed[..index].contains(address)
            || *address == snapshot.submitter.address
            || *address == snapshot.system_program.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    if let (Some(expected), Some(refund)) = (expected_refund, snapshot.evicted_refund_owner) {
        if fixed.iter().any(|address| address.to_bytes() == expected)
            || refund.address == snapshot.system_program.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

pub(crate) fn authenticate_snapshot_set(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    accounts: &[&ObservedRpcAccount],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for account in accounts {
        if account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.release_key != release.key()
            || account.provenance.slot != freshness.observed_slot
        {
            return Err(CanonicalActionMaterialErrorV1::WrongSelection);
        }
    }
    Ok(())
}

fn authenticate_operator_cursor(
    selection: &KeeperActionSelection,
    generation: u64,
    action: DirectMarketAction,
    sequence: u64,
    prestate_id: [u8; 32],
    expected_dependencies: [Address; 5],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if selection.cursor.workflow_id == [0; 32]
        || selection.cursor.lane != WorkflowLane::Candidate
        || selection.cursor.generation != generation
        || selection.cursor.position.phase != u16::from(action.tag())
        || selection.cursor.position.item != sequence
        || selection.cursor.observed_state_sha256 == [0; 32]
        || prestate_id == [0; 32]
        || selection.dependencies.len() != expected_dependencies.len()
        || expected_dependencies
            .iter()
            .any(|address| !selection.dependencies.contains(address))
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(())
}

fn direct_snapshot_id(
    release: &IndexedProgramRelease,
    action: DirectMarketAction,
    sequence: u64,
    accounts: &[&ObservedRpcAccount],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/direct-candidate-snapshot/v1\0");
    hash.update(release.release_manifest_sha256);
    hash.update(release.capability_profile_id);
    hash.update([action.tag()]);
    hash.update(sequence.to_le_bytes());
    for account in accounts {
        hash.update(account.address.as_ref());
        hash.update(account.owner.as_ref());
        hash.update(account.lamports.to_le_bytes());
        hash.update([u8::from(account.executable)]);
        hash.update(account.provenance.slot.to_le_bytes());
        hash.update(sha256(&account.data));
    }
    hash.finalize().into()
}

fn require_operator_signer(
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address == Address::default()
        || account.owner != Address::default()
        || account.executable
        || !account.data.is_empty()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn require_system_program(
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address != Address::default() || !account.executable {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

pub(crate) fn decode_clock(
    account: &ObservedRpcAccount,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let expected = Address::from_str("SysvarC1ock11111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let sysvar_owner = Address::from_str("Sysvar1111111111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != expected
        || account.owner != sysvar_owner
        || account.executable
        || account.data.len() != 40
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&account.data[..8]);
    let slot = u64::from_le_bytes(bytes);
    if slot == 0 || slot != account.provenance.slot {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(slot)
}

fn exact_candidate_bytes(
    account: &ObservedRpcAccount,
) -> Result<[u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1], CanonicalActionMaterialErrorV1> {
    <[u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1]>::try_from(account.data.as_slice())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn meta(account: &ObservedRpcAccount, signer: bool, writable: bool) -> AccountMeta {
    AccountMeta {
        pubkey: account.address,
        is_signer: signer,
        is_writable: writable,
    }
}

fn role(
    label: &'static str,
    account: &ObservedRpcAccount,
    writable: bool,
    signer: bool,
) -> crate::action_material::CanonicalAccountRoleV1 {
    chain_derived_direct_role_v2(label, account.address, writable, signer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v2::{EconomicDomainV2, EconomicOrderV2, PricePreconditionV2};
    use clutch_batch::Side;

    // This source-only adversarial test pins the operator's exact-cash step:
    // A quantity bound of five at price 2/3 deterministically derives the
    // exact-cash fill three, never the inexact fill five.
    #[test]
    fn exact_cash_step_is_not_rounded() {
        assert_eq!(greatest_common_divisor(2, 3), 1);
        let maximum = 5u64;
        let step = 3u64;
        assert_eq!(maximum - maximum % step, 3);
    }

    #[test]
    fn hostile_non_single_egg_projection_is_refused() {
        let domain = EconomicDomainV2 {
            relation_version: 2,
            market_semantics_digest: [1; 32],
            epoch_semantics_digest: [2; 32],
            relation_policy_digest: [3; 32],
            price_policy_digest: [4; 32],
            epoch_index: 1,
            outcome_count: 2,
            price_scale: 10,
        };
        let mut coefficients = [0u64; 16];
        coefficients[0] = 1;
        coefficients[1] = 1;
        assert_eq!(
            selected_single_egg_outcome(&domain, &coefficients),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
        let _unused_types = (
            core::mem::size_of::<EconomicOrderV2>(),
            core::mem::size_of::<PricePreconditionV2>(),
            Side::Buy,
        );
    }
}
