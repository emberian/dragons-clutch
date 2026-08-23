// SPDX-License-Identifier: AGPL-3.0-or-later
//! Root-only account contract for the single-custody failure runtime.
//!
//! The failure root persists semantics and its own rent. It never authenticates
//! or debits a zero-data work reserve. Every work/rent movement is planned by
//! the separately persisted liveness Recovery compartment.

use clutch_failure_policy_runtime::external_v2::{
    FailureExternalAdmissionReceiptV2, FailureExternalTransitionPlanV2,
    FailureRecoveryTerminalReceiptV2, FailureRuntimeExternalV2, FAILURE_RUNTIME_EXTERNAL_V2_BYTES,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeReceiptObservationV1,
};
use clutch_liveness::Id as LivenessId;
use sha2::{Digest, Sha256};

use crate::{AccountId, AccountView};

const MAGIC: [u8; 8] = *b"DCFAILE2";
const VERSION: u16 = 2;
const ROOT_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/failure-external-root/v2";

/// Exact width of the root-only external-custody account.
pub const FAILURE_EXTERNAL_ROOT_V2_BYTES: usize = 2_168;

/// Refusal from the single-custody account/liveness bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalAdapterErrorV2 {
    /// Failure semantic owner refused the operation.
    Failure(clutch_failure_policy_runtime::Error),
    /// Liveness owner refused the funding transition.
    Liveness(clutch_liveness::runtime_adapter_v1::RuntimeAdapterErrorV1),
    /// Input or output did not use the one exact width.
    WrongLength,
    /// Root discriminator did not match.
    BadMagic,
    /// Root schema did not match exactly.
    BadVersion,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// Root account key was wrong.
    WrongRoot,
    /// Program owner was wrong.
    WrongOwner,
    /// Writable privilege was absent.
    NotWritable,
    /// A freshly allocated root contained nonzero data.
    RootNotZero,
    /// Root-rent funding was absent or did not match the exact payer debit.
    RootRentMismatch,
    /// An admission or terminal receipt did not match the decoded root.
    ReceiptMismatch,
    /// Root digest did not match its complete semantic bytes.
    DigestMismatch,
    /// Work and non-work projections were crossed.
    WrongTransitionKind,
    /// Root rent is no longer present.
    RootRentUnderfunded,
}

impl From<clutch_failure_policy_runtime::Error> for ExternalAdapterErrorV2 {
    fn from(value: clutch_failure_policy_runtime::Error) -> Self {
        Self::Failure(value)
    }
}

impl From<clutch_liveness::runtime_adapter_v1::RuntimeAdapterErrorV1> for ExternalAdapterErrorV2 {
    fn from(value: clutch_liveness::runtime_adapter_v1::RuntimeAdapterErrorV1) -> Self {
        Self::Liveness(value)
    }
}

/// Result alias for the single-custody adapter.
pub type ExternalResultV2<T> = core::result::Result<T, ExternalAdapterErrorV2>;

/// Exact present root-rent funding. Preexisting lamports are sink-owned
/// donations and never reduce the payer debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRootFundingObservationV2 {
    /// Root balance before exact payer funding.
    pub balance_before: u64,
    /// Root balance after exact payer funding.
    pub balance_after: u64,
    /// Exact payer debit for root rent.
    pub payer_debit_lamports: u64,
}

/// Initialization plan for one durable semantic root and no work reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRootInitializationV2 {
    /// Exact root account.
    pub root: AccountId,
    /// Exact root-rent payer.
    pub root_rent_payer: AccountId,
    /// Exact root rent principal.
    pub root_rent_principal_lamports: u64,
    /// Complete post-initialization bytes.
    pub post_root_data: [u8; FAILURE_EXTERNAL_ROOT_V2_BYTES],
}

/// Authenticate and initialize one root after liveness funding is already
/// present. No reserve account is accepted by this API.
pub fn initialize_external_root_v2(
    program_id: AccountId,
    root: AccountView<'_>,
    bump: u8,
    root_rent_payer: AccountId,
    root_rent_principal_lamports: u64,
    neutral_sink: AccountId,
    funding: ExternalRootFundingObservationV2,
    runtime: FailureRuntimeExternalV2,
    receipt: FailureExternalAdmissionReceiptV2,
) -> ExternalResultV2<ExternalRootInitializationV2> {
    if root.owner != program_id {
        return Err(ExternalAdapterErrorV2::WrongOwner);
    }
    if !root.is_writable {
        return Err(ExternalAdapterErrorV2::NotWritable);
    }
    if root.data.len() != FAILURE_EXTERNAL_ROOT_V2_BYTES || root.data.iter().any(|byte| *byte != 0)
    {
        return Err(ExternalAdapterErrorV2::RootNotZero);
    }
    runtime.check()?;
    if root.key.bytes() != runtime.semantic_state_id().bytes()
        || receipt.binding_id() != runtime.binding_id()
        || receipt.market_instance_id() != runtime.binding().market_instance_id()
        || receipt.funding_quote_id() != runtime.binding().funding_quote_id()
        || receipt.semantic_state_id() != runtime.semantic_state_id()
        || receipt.recovery_compartment_account_id() != runtime.recovery_compartment_account_id()
        || receipt.generation() != runtime.binding().generation()
        || root_rent_payer.bytes() != runtime.recovery_payer().bytes()
        || neutral_sink.bytes() != runtime.recovery_neutral_sink().bytes()
        || root_rent_payer == root.key
        || neutral_sink == root.key
        || neutral_sink == root_rent_payer
    {
        return Err(ExternalAdapterErrorV2::ReceiptMismatch);
    }
    if root_rent_principal_lamports == 0
        || funding.payer_debit_lamports != root_rent_principal_lamports
        || funding.balance_after != root.lamports
        || funding
            .balance_before
            .checked_add(root_rent_principal_lamports)
            != Some(funding.balance_after)
    {
        return Err(ExternalAdapterErrorV2::RootRentMismatch);
    }
    let mut post_root_data = [0u8; FAILURE_EXTERNAL_ROOT_V2_BYTES];
    encode_root(
        &mut post_root_data,
        bump,
        root_rent_payer,
        root_rent_principal_lamports,
        neutral_sink,
        runtime,
    )?;
    Ok(ExternalRootInitializationV2 {
        root: root.key,
        root_rent_payer,
        root_rent_principal_lamports,
        post_root_data,
    })
}

/// Private authenticated root capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedExternalRootV2 {
    root: AccountId,
    owner_program: AccountId,
    lamports: u64,
    bump: u8,
    root_rent_payer: AccountId,
    root_rent_principal_lamports: u64,
    neutral_sink: AccountId,
    runtime: FailureRuntimeExternalV2,
}

impl AuthenticatedExternalRootV2 {
    /// Exact decoded semantic runtime.
    pub const fn runtime(self) -> FailureRuntimeExternalV2 {
        self.runtime
    }

    /// Exact root account identity.
    pub const fn root(self) -> AccountId {
        self.root
    }

    /// Current root lamports, containing only root rent plus donations.
    pub const fn lamports(self) -> u64 {
        self.lamports
    }
}

/// Authenticate owner, root key, mutability, complete codec, and digest.
pub fn authenticate_external_root_v2(
    expected_program_id: AccountId,
    root: AccountView<'_>,
) -> ExternalResultV2<AuthenticatedExternalRootV2> {
    if !root.is_writable {
        return Err(ExternalAdapterErrorV2::NotWritable);
    }
    authenticate_external_root_contents_v2(expected_program_id, root)
}

/// Authenticate owner, root key, complete codec, and digest without claiming
/// write authority. The account-facing adapter separately enforces a read-only
/// meta for transitions which preserve the semantic root byte-for-byte.
pub fn authenticate_external_root_readonly_v2(
    expected_program_id: AccountId,
    root: AccountView<'_>,
) -> ExternalResultV2<AuthenticatedExternalRootV2> {
    authenticate_external_root_contents_v2(expected_program_id, root)
}

fn authenticate_external_root_contents_v2(
    expected_program_id: AccountId,
    root: AccountView<'_>,
) -> ExternalResultV2<AuthenticatedExternalRootV2> {
    if root.owner != expected_program_id {
        return Err(ExternalAdapterErrorV2::WrongOwner);
    }
    let decoded = decode_root(root.data)?;
    if root.key.bytes() != decoded.runtime.semantic_state_id().bytes() {
        return Err(ExternalAdapterErrorV2::WrongRoot);
    }
    if root.lamports < decoded.root_rent_principal_lamports {
        return Err(ExternalAdapterErrorV2::RootRentUnderfunded);
    }
    Ok(AuthenticatedExternalRootV2 {
        root: root.key,
        owner_program: expected_program_id,
        lamports: root.lamports,
        bump: decoded.bump,
        root_rent_payer: decoded.root_rent_payer,
        root_rent_principal_lamports: decoded.root_rent_principal_lamports,
        neutral_sink: decoded.neutral_sink,
        runtime: decoded.runtime,
    })
}

/// Root-only semantic mutation with no liveness movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalSemanticMutationV2 {
    /// Exact root account to rewrite.
    pub root: AccountId,
    /// Complete poststate bytes.
    pub post_root_data: [u8; FAILURE_EXTERNAL_ROOT_V2_BYTES],
}

/// Project a trigger, schedule advance, or caller-funded resolution. A plan
/// carrying a work receipt is refused so payment cannot be accidentally lost.
pub fn project_external_semantic_transition_v2(
    root: AuthenticatedExternalRootV2,
    plan: FailureExternalTransitionPlanV2,
) -> ExternalResultV2<ExternalSemanticMutationV2> {
    if plan.work().is_some() {
        return Err(ExternalAdapterErrorV2::WrongTransitionKind);
    }
    let mut runtime = root.runtime;
    runtime.commit_plan(plan)?;
    let mut post_root_data = [0u8; FAILURE_EXTERNAL_ROOT_V2_BYTES];
    encode_root(
        &mut post_root_data,
        root.bump,
        root.root_rent_payer,
        root.root_rent_principal_lamports,
        root.neutral_sink,
        runtime,
    )?;
    Ok(ExternalSemanticMutationV2 {
        root: root.root,
        post_root_data,
    })
}

/// Atomic failure-state plus liveness Recovery work mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalWorkMutationV2 {
    /// Exact semantic root rewrite.
    pub semantic: ExternalSemanticMutationV2,
    /// Sole liveness custody write and keeper/refund movements.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Join one semantic work receipt to the exact liveness `SpendWork` plan.
/// Failure-root lamports remain unchanged; the keeper movement exists only in
/// `liveness`.
#[allow(clippy::too_many_arguments)]
pub fn project_external_work_transition_v2(
    expected_liveness_program_id: LivenessId,
    expected_liveness_policy_account_id: LivenessId,
    liveness_policy_view: RuntimePersistedAccountViewV1<'_>,
    liveness_recovery_view: RuntimePersistedAccountViewV1<'_>,
    liveness_account_balance_after: u64,
    root: AuthenticatedExternalRootV2,
    plan: FailureExternalTransitionPlanV2,
) -> ExternalResultV2<ExternalWorkMutationV2> {
    let work = plan
        .work()
        .ok_or(ExternalAdapterErrorV2::WrongTransitionKind)?;
    if liveness_recovery_view.account_id != root.runtime.recovery_compartment_account_id() {
        return Err(ExternalAdapterErrorV2::ReceiptMismatch);
    }
    let intent = work.runtime_transition_intent();
    let receipt = work.runtime_receipt_observation(
        LivenessId::from_bytes(root.root.bytes()),
        LivenessId::from_bytes(root.owner_program.bytes()),
    )?;
    let liveness = plan_runtime_transition_v1(
        expected_liveness_program_id,
        expected_liveness_policy_account_id,
        liveness_policy_view,
        liveness_recovery_view,
        intent,
        Some(receipt),
        liveness_account_balance_after,
    )?;
    let semantic = project_external_work_semantic(root, plan)?;
    Ok(ExternalWorkMutationV2 { semantic, liveness })
}

fn project_external_work_semantic(
    root: AuthenticatedExternalRootV2,
    plan: FailureExternalTransitionPlanV2,
) -> ExternalResultV2<ExternalSemanticMutationV2> {
    if plan.work().is_none() {
        return Err(ExternalAdapterErrorV2::WrongTransitionKind);
    }
    let mut runtime = root.runtime;
    runtime.commit_plan(plan)?;
    let mut post_root_data = [0u8; FAILURE_EXTERNAL_ROOT_V2_BYTES];
    encode_root(
        &mut post_root_data,
        root.bump,
        root.root_rent_payer,
        root.root_rent_principal_lamports,
        root.neutral_sink,
        runtime,
    )?;
    Ok(ExternalSemanticMutationV2 {
        root: root.root,
        post_root_data,
    })
}

/// Recovery-only liveness close. The semantic root remains readable and may
/// later accept caller-funded evidence after dormancy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryCloseV2 {
    /// Root deliberately preserved by this close.
    pub preserved_root: AccountId,
    /// Liveness Recovery terminal close and its exact dispositions.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Close only the external Recovery compartment from a current terminal
/// semantic receipt. No Retirement success is inferred from dormancy.
#[allow(clippy::too_many_arguments)]
pub fn project_external_recovery_close_v2(
    expected_liveness_program_id: LivenessId,
    expected_liveness_policy_account_id: LivenessId,
    liveness_policy_view: RuntimePersistedAccountViewV1<'_>,
    liveness_recovery_view: RuntimePersistedAccountViewV1<'_>,
    liveness_account_balance_after: u64,
    root: AuthenticatedExternalRootV2,
    receipt: FailureRecoveryTerminalReceiptV2,
) -> ExternalResultV2<ExternalRecoveryCloseV2> {
    let intent = receipt.runtime_transition_intent();
    let observation: RuntimeReceiptObservationV1 = receipt.runtime_receipt_observation(
        LivenessId::from_bytes(root.root.bytes()),
        LivenessId::from_bytes(root.owner_program.bytes()),
    )?;
    let liveness = plan_runtime_transition_v1(
        expected_liveness_program_id,
        expected_liveness_policy_account_id,
        liveness_policy_view,
        liveness_recovery_view,
        intent,
        Some(observation),
        liveness_account_balance_after,
    )?;
    Ok(ExternalRecoveryCloseV2 {
        preserved_root: root.root,
        liveness,
    })
}

struct DecodedRootV2 {
    bump: u8,
    root_rent_payer: AccountId,
    root_rent_principal_lamports: u64,
    neutral_sink: AccountId,
    runtime: FailureRuntimeExternalV2,
}

fn encode_root(
    output: &mut [u8],
    bump: u8,
    root_rent_payer: AccountId,
    root_rent_principal_lamports: u64,
    neutral_sink: AccountId,
    runtime: FailureRuntimeExternalV2,
) -> ExternalResultV2<()> {
    if output.len() != FAILURE_EXTERNAL_ROOT_V2_BYTES {
        return Err(ExternalAdapterErrorV2::WrongLength);
    }
    let mut runtime_bytes = [0u8; FAILURE_RUNTIME_EXTERNAL_V2_BYTES];
    runtime.encode_into(&mut runtime_bytes)?;
    output.fill(0);
    output[..8].copy_from_slice(&MAGIC);
    output[8..10].copy_from_slice(&VERSION.to_le_bytes());
    output[10] = bump;
    output[16..48].copy_from_slice(&root_rent_payer.bytes());
    output[48..80].copy_from_slice(&neutral_sink.bytes());
    output[80..88].copy_from_slice(&root_rent_principal_lamports.to_le_bytes());
    output[120..].copy_from_slice(&runtime_bytes);
    let digest = root_digest(&output[..88], &runtime_bytes);
    output[88..120].copy_from_slice(&digest);
    Ok(())
}

fn decode_root(input: &[u8]) -> ExternalResultV2<DecodedRootV2> {
    if input.len() != FAILURE_EXTERNAL_ROOT_V2_BYTES {
        return Err(ExternalAdapterErrorV2::WrongLength);
    }
    if input[..8] != MAGIC {
        return Err(ExternalAdapterErrorV2::BadMagic);
    }
    if u16::from_le_bytes([input[8], input[9]]) != VERSION {
        return Err(ExternalAdapterErrorV2::BadVersion);
    }
    if input[11..16].iter().any(|byte| *byte != 0) {
        return Err(ExternalAdapterErrorV2::NonCanonicalReserved);
    }
    let root_rent_payer = AccountId::from_bytes(array_at(input, 16)?);
    let neutral_sink = AccountId::from_bytes(array_at(input, 48)?);
    let root_rent_principal_lamports = u64::from_le_bytes(array_at(input, 80)?);
    let stored_digest = array_at::<32>(input, 88)?;
    let runtime_bytes = array_at::<FAILURE_RUNTIME_EXTERNAL_V2_BYTES>(input, 120)?;
    if stored_digest != root_digest(&input[..88], &runtime_bytes) {
        return Err(ExternalAdapterErrorV2::DigestMismatch);
    }
    let runtime = FailureRuntimeExternalV2::decode(&runtime_bytes)?;
    Ok(DecodedRootV2 {
        bump: input[10],
        root_rent_payer,
        root_rent_principal_lamports,
        neutral_sink,
        runtime,
    })
}

fn root_digest(prefix: &[u8], runtime: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ROOT_DIGEST_DOMAIN);
    hasher.update(prefix);
    hasher.update(runtime);
    hasher.finalize().into()
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> ExternalResultV2<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(ExternalAdapterErrorV2::WrongLength)?;
    let source = input
        .get(offset..end)
        .ok_or(ExternalAdapterErrorV2::WrongLength)?;
    let mut output = [0u8; N];
    output.copy_from_slice(source);
    Ok(output)
}
