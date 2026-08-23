// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Account, authentication, intent, and mutation contract for the successor
//! failure-policy runtime.

use clutch_evidence_recovery::{Identity as RecoveryIdentity, RecoveryClock, TransferPlan};
use clutch_failure_policy_runtime::{
    AcceptedResolutionV1, AdapterAuthenticatedRelationRefusalV1, FailurePolicyBindingId,
    FailureRecoveryTerminalReceiptV1, FailureRuntimeV1, FailureTerminalJoinV1,
    FailureTransitionPlanV1, LivenessWorkReceiptJoinV1, FAILURE_RUNTIME_V1_BYTES,
};
use clutch_product_series::MarketInstanceV2Id;
use clutch_source_plane_v3::{
    StatisticKeyV3, StatisticResultV3, SummaryProgramV3, WindowSealV3, WindowSpecV3,
};
use sha2::{Digest, Sha256};

const ROOT_MAGIC: [u8; 8] = *b"DCFAILA1";
const ROOT_SCHEMA: u16 = 1;
const ROOT_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/failure-root-account/v1";
const INTENT_MAGIC: [u8; 8] = *b"DCFAILI1";
const INTENT_SCHEMA: u16 = 1;

/// Exact canonical durable failure-root width.
pub const FAILURE_ROOT_ACCOUNT_V1_BYTES: usize = 1_820;
/// Exact canonical failure intent width.
pub const FAILURE_INTENT_V1_BYTES: usize = 344;

/// Adapter result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Deterministic refusal from the standalone adapter contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The semantic runtime refused a value or transition.
    Runtime(clutch_failure_policy_runtime::Error),
    /// Input or output did not have the one exact width.
    WrongLength,
    /// A discriminator did not match.
    BadMagic,
    /// A schema version did not match exactly.
    BadVersion,
    /// Reserved or inactive bytes were nonzero.
    NonCanonicalPadding,
    /// An enum discriminant or action-specific field shape was invalid.
    InvalidParameter,
    /// An otherwise valid intent named another action.
    WrongAction,
    /// An authenticated source, relation, work, or terminal artifact did not
    /// match the exact identity committed by the intent.
    ArtifactMismatch,
    /// A required identity was the all-zero sentinel.
    ZeroIdentity,
    /// An account key did not match the exact expected identity.
    WrongKey,
    /// An account owner did not match the exact program owner.
    WrongOwner,
    /// A required account was not writable.
    NotWritable,
    /// Durable root and expendable reserve aliases were presented.
    AccountAlias,
    /// The reserve carried data even though its sole role is lamport custody.
    ReserveDataNotEmpty,
    /// The stored root digest did not match canonical runtime bytes.
    DigestMismatch,
    /// The intent replay nonce did not equal the decoded runtime nonce.
    ReplayMismatch,
    /// A transition expected another reserve pre-balance.
    ReserveBalanceMismatch,
}

impl From<clutch_failure_policy_runtime::Error> for Error {
    fn from(value: clutch_failure_policy_runtime::Error) -> Self {
        Self::Runtime(value)
    }
}

impl From<clutch_source_plane_v3::Error> for Error {
    fn from(value: clutch_source_plane_v3::Error) -> Self {
        Self::Runtime(clutch_failure_policy_runtime::Error::Source(value))
    }
}

/// Exact opaque account identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct AccountId([u8; 32]);

impl AccountId {
    /// Construct from exact bytes without claiming account authentication.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

/// Durable canonical failure-root body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRootAccountV1 {
    bump: u8,
    runtime_digest: [u8; 32],
    runtime: FailureRuntimeV1,
}

impl FailureRootAccountV1 {
    /// Construct a durable root from one fully checked runtime.
    pub fn new(bump: u8, runtime: FailureRuntimeV1) -> Result<Self> {
        runtime.check()?;
        let runtime_digest = runtime_digest(bump, &runtime)?;
        Ok(Self {
            bump,
            runtime_digest,
            runtime,
        })
    }

    /// Stored address-derivation bump; the live adapter must verify its PDA.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    /// Digest of exact canonical runtime bytes and bump.
    pub const fn runtime_digest(&self) -> [u8; 32] {
        self.runtime_digest
    }

    /// Complete semantic runtime.
    pub const fn runtime(&self) -> FailureRuntimeV1 {
        self.runtime
    }

    /// Encode the exact durable root body.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.runtime.check()?;
        if self.runtime_digest != runtime_digest(self.bump, &self.runtime)? {
            return Err(Error::DigestMismatch);
        }
        if output.len() != FAILURE_ROOT_ACCOUNT_V1_BYTES {
            return Err(Error::WrongLength);
        }
        output.fill(0);
        output[..8].copy_from_slice(&ROOT_MAGIC);
        output[8..10].copy_from_slice(&ROOT_SCHEMA.to_le_bytes());
        output[10] = self.bump;
        output[12..44].copy_from_slice(&self.runtime_digest);
        self.runtime
            .encode_into(&mut output[44..FAILURE_ROOT_ACCOUNT_V1_BYTES])?;
        Ok(())
    }

    /// Decode and fully validate one exact durable root body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != FAILURE_ROOT_ACCOUNT_V1_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != ROOT_MAGIC {
            return Err(Error::BadMagic);
        }
        if u16::from_le_bytes([input[8], input[9]]) != ROOT_SCHEMA {
            return Err(Error::BadVersion);
        }
        if input[11] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        let bump = input[10];
        let mut stored = [0; 32];
        stored.copy_from_slice(&input[12..44]);
        let runtime = FailureRuntimeV1::decode(&input[44..])?;
        let value = Self {
            bump,
            runtime_digest: stored,
            runtime,
        };
        if value.runtime_digest != runtime_digest(value.bump, &value.runtime)? {
            return Err(Error::DigestMismatch);
        }
        Ok(value)
    }
}

/// Read-only adapter view over one runtime account.
#[derive(Clone, Copy, Debug)]
pub struct AccountView<'a> {
    /// Account key.
    pub key: AccountId,
    /// Runtime owner.
    pub owner: AccountId,
    /// Current lamport balance.
    pub lamports: u64,
    /// Exact data bytes.
    pub data: &'a [u8],
    /// Whether this transaction presented the account writable.
    pub is_writable: bool,
}

/// Private-field capability for one authenticated root and reserve pair.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedFailureAccountsV1 {
    root_key: AccountId,
    reserve_key: AccountId,
    root: FailureRootAccountV1,
    root_lamports: u64,
    reserve_lamports: u64,
}

impl AuthenticatedFailureAccountsV1 {
    /// Exact decoded root.
    pub const fn root(&self) -> FailureRootAccountV1 {
        self.root
    }

    /// Exact reserve balance used by transition planning.
    pub const fn reserve_lamports(&self) -> u64 {
        self.reserve_lamports
    }

    /// Durable root lamports, never recovery work/rent principal.
    pub const fn root_lamports(&self) -> u64 {
        self.root_lamports
    }
}

/// Authenticate exact root/reserve keys, owners, mutability, and reserve shape.
pub fn authenticate_failure_accounts<'a>(
    root: AccountView<'a>,
    reserve: AccountView<'a>,
    expected_root_key: AccountId,
    program_id: AccountId,
) -> Result<AuthenticatedFailureAccountsV1> {
    if expected_root_key.is_zero() || program_id.is_zero() {
        return Err(Error::ZeroIdentity);
    }
    if root.key != expected_root_key {
        return Err(Error::WrongKey);
    }
    if root.owner != program_id || reserve.owner != program_id {
        return Err(Error::WrongOwner);
    }
    if !root.is_writable || !reserve.is_writable {
        return Err(Error::NotWritable);
    }
    if root.key == reserve.key {
        return Err(Error::AccountAlias);
    }
    if !reserve.data.is_empty() {
        return Err(Error::ReserveDataNotEmpty);
    }
    let decoded = FailureRootAccountV1::decode(root.data)?;
    let expected_reserve = AccountId::from_bytes(
        decoded.runtime().binding().recovery_state_id().bytes(),
    );
    if reserve.key != expected_reserve {
        return Err(Error::WrongKey);
    }
    Ok(AuthenticatedFailureAccountsV1 {
        root_key: root.key,
        reserve_key: reserve.key,
        root: decoded,
        root_lamports: root.lamports,
        reserve_lamports: reserve.lamports,
    })
}

/// Atomic account mutation projection for one semantic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAccountMutationV1 {
    root_key: AccountId,
    reserve_key: AccountId,
    before_root_digest: [u8; 32],
    after_root: FailureRootAccountV1,
    root_lamports: u64,
    expected_reserve_pre_balance: u64,
    expected_reserve_post_balance: u64,
    transfers: TransferPlan,
}

impl FailureAccountMutationV1 {
    /// Root key whose data must be atomically replaced.
    pub const fn root_key(&self) -> AccountId {
        self.root_key
    }

    /// Reserve key whose lamports must match the exact plan.
    pub const fn reserve_key(&self) -> AccountId {
        self.reserve_key
    }

    /// Digest of the authenticated root prestate.
    pub const fn before_root_digest(&self) -> [u8; 32] {
        self.before_root_digest
    }

    /// Canonical complete root poststate.
    pub const fn after_root(&self) -> FailureRootAccountV1 {
        self.after_root
    }

    /// Unchanged durable root lamports.
    pub const fn root_lamports(&self) -> u64 {
        self.root_lamports
    }

    /// Exact reserve pre-balance.
    pub const fn expected_reserve_pre_balance(&self) -> u64 {
        self.expected_reserve_pre_balance
    }

    /// Exact reserve post-balance.
    pub const fn expected_reserve_post_balance(&self) -> u64 {
        self.expected_reserve_post_balance
    }

    /// Exact four semantic transfer compartments.
    pub const fn transfers(&self) -> TransferPlan {
        self.transfers
    }
}

/// Apply a semantic plan to the authenticated pure account projection.
///
/// The external adapter must perform transfers, verify every coalesced
/// recipient delta and reserve post-balance, then encode `after_root` into the
/// writable root in the same transaction.
pub fn project_failure_transition(
    accounts: AuthenticatedFailureAccountsV1,
    plan: FailureTransitionPlanV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    if accounts.reserve_lamports != plan.expected_pre_balance() {
        return Err(Error::ReserveBalanceMismatch);
    }
    let mut runtime = accounts.root.runtime();
    runtime.commit_plan(plan, actual_reserve_post_balance)?;
    let after_root = FailureRootAccountV1::new(accounts.root.bump(), runtime)?;
    Ok(FailureAccountMutationV1 {
        root_key: accounts.root_key,
        reserve_key: accounts.reserve_key,
        before_root_digest: accounts.root.runtime_digest(),
        after_root,
        root_lamports: accounts.root_lamports,
        expected_reserve_pre_balance: plan.expected_pre_balance(),
        expected_reserve_post_balance: plan.expected_post_balance(),
        transfers: plan.transfers(),
    })
}

/// Closed failure-runtime instruction family for a future central allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureActionV1 {
    /// Enter degraded recovery at immutable maturity.
    TriggerMaturity = 1,
    /// Enter degraded recovery with an exact SourcePlane refusal.
    TriggerSourceRefusal = 2,
    /// Enter degraded recovery with a frozen relation refusal.
    TriggerRelationRefusal = 3,
    /// Advance/close the finite repair schedule.
    AdvanceSchedule = 4,
    /// Accept and pay one authenticated liveness work receipt.
    AcceptWork = 5,
    /// Resolve from caller-funded accepted evidence.
    ResolveCallerFunded = 6,
    /// Resolve with one final paid work receipt.
    ResolvePaidWork = 7,
    /// Bind separately authenticated terminal-owner receipts.
    BindTerminal = 8,
    /// Bind the finite recovery campaign's success/failure close receipt.
    BindRecoveryFundingClose = 9,
}

/// Exact fixed intent preimage; inactive action fields are canonical zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntentV1 {
    /// Closed action.
    pub action: FailureActionV1,
    /// Immutable failure-policy binding.
    pub binding_id: FailurePolicyBindingId,
    /// Full-width V2 market occurrence.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact recovery generation.
    pub generation: u64,
    /// Expected runtime transition nonce.
    pub expected_transition_nonce: u64,
    /// Exact adapter-authenticated Clock projection, or zero for terminal bind.
    pub clock: RecoveryClock,
    /// Exact SourcePlane Window ID where the action uses evidence/work.
    pub window_id: [u8; 32],
    /// Exact source/result/relation accepted evidence identity.
    pub evidence_id: [u8; 32],
    /// Exact liveness work receipt identity.
    pub work_receipt_id: [u8; 32],
    /// Exact recovery-compartment quote schedule authenticated by liveness.
    pub quote_schedule_id: [u8; 32],
    /// Exact per-call lamport ceiling authenticated by liveness.
    pub scheduled_ceiling_lamports: u64,
    /// Exact work reward recipient.
    pub reward_recipient: RecoveryIdentity,
    /// Exact cumulative accepted progress.
    pub accepted_progress_total: u64,
    /// Exact terminal join identity.
    pub terminal_join_id: [u8; 32],
    /// Stable source/relation refusal code.
    pub refusal_code: u32,
}

impl FailureIntentV1 {
    /// Validate action-specific identity presence and zero padding.
    pub fn validate(&self) -> Result<()> {
        if self.binding_id.bytes().iter().all(|byte| *byte == 0)
            || self.market_instance_id.bytes().iter().all(|byte| *byte == 0)
            || self.generation == 0
        {
            return Err(Error::ZeroIdentity);
        }
        let has_window = !is_zero(&self.window_id);
        let has_evidence = !is_zero(&self.evidence_id);
        let has_work = !is_zero(&self.work_receipt_id);
        let has_quote = !is_zero(&self.quote_schedule_id);
        let has_reward = !is_zero(&self.reward_recipient.bytes());
        let has_terminal = !is_zero(&self.terminal_join_id);
        let clock_is_zero = self.clock
            == (RecoveryClock {
                slot: 0,
                unix_timestamp: 0,
                current_bucket: 0,
            });
        let valid = match self.action {
            FailureActionV1::TriggerMaturity => {
                !clock_is_zero
                    && !has_window
                    && !has_evidence
                    && !has_work
                    && !has_quote
                    && self.scheduled_ceiling_lamports == 0
                    && !has_reward
                    && !has_terminal
                    && self.accepted_progress_total == 0
                    && self.refusal_code == 0
            }
            FailureActionV1::TriggerSourceRefusal
            | FailureActionV1::TriggerRelationRefusal => {
                !clock_is_zero
                    && has_window
                    && has_evidence
                    && !has_work
                    && !has_quote
                    && self.scheduled_ceiling_lamports == 0
                    && !has_reward
                    && !has_terminal
                    && self.accepted_progress_total == 0
                    && self.refusal_code != 0
            }
            FailureActionV1::AdvanceSchedule => {
                !clock_is_zero
                    && !has_window
                    && !has_evidence
                    && !has_work
                    && !has_quote
                    && self.scheduled_ceiling_lamports == 0
                    && !has_reward
                    && !has_terminal
                    && self.accepted_progress_total == 0
                    && self.refusal_code == 0
            }
            FailureActionV1::AcceptWork => {
                !clock_is_zero
                    && has_window
                    && !has_evidence
                    && has_work
                    && has_quote
                    && self.scheduled_ceiling_lamports != 0
                    && has_reward
                    && !has_terminal
                    && self.accepted_progress_total != 0
                    && self.refusal_code == 0
            }
            FailureActionV1::ResolveCallerFunded => {
                !clock_is_zero
                    && has_window
                    && has_evidence
                    && !has_work
                    && !has_quote
                    && self.scheduled_ceiling_lamports == 0
                    && !has_reward
                    && !has_terminal
                    && self.accepted_progress_total == 0
                    && self.refusal_code == 0
            }
            FailureActionV1::ResolvePaidWork => {
                !clock_is_zero
                    && has_window
                    && has_evidence
                    && has_work
                    && has_quote
                    && self.scheduled_ceiling_lamports != 0
                    && has_reward
                    && !has_terminal
                    && self.accepted_progress_total != 0
                    && self.refusal_code == 0
            }
            FailureActionV1::BindTerminal | FailureActionV1::BindRecoveryFundingClose => {
                clock_is_zero
                    && !has_window
                    && !has_evidence
                    && !has_work
                    && !has_quote
                    && self.scheduled_ceiling_lamports == 0
                    && !has_reward
                    && has_terminal
                    && self.accepted_progress_total == 0
                    && self.refusal_code == 0
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::InvalidParameter)
        }
    }

    /// Bind this request to the exact decoded runtime and replay nonce.
    pub fn validate_runtime(&self, runtime: &FailureRuntimeV1) -> Result<()> {
        self.validate()?;
        runtime.check()?;
        if self.binding_id != runtime.binding_id()
            || self.market_instance_id != runtime.binding().market_instance_id()
            || self.generation != runtime.binding().generation()
        {
            return Err(Error::WrongKey);
        }
        if self.expected_transition_nonce != runtime.transition_nonce() {
            return Err(Error::ReplayMismatch);
        }
        Ok(())
    }

    /// Encode the exact fixed intent preimage.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        if output.len() != FAILURE_INTENT_V1_BYTES {
            return Err(Error::WrongLength);
        }
        output.fill(0);
        output[..8].copy_from_slice(&INTENT_MAGIC);
        output[8..10].copy_from_slice(&INTENT_SCHEMA.to_le_bytes());
        output[10] = self.action as u8;
        let mut at = 16;
        put(output, &mut at, &self.binding_id.bytes())?;
        put(output, &mut at, &self.market_instance_id.bytes())?;
        put(output, &mut at, &self.generation.to_le_bytes())?;
        put(output, &mut at, &self.expected_transition_nonce.to_le_bytes())?;
        put(output, &mut at, &self.clock.slot.to_le_bytes())?;
        put(output, &mut at, &self.clock.unix_timestamp.to_le_bytes())?;
        put(output, &mut at, &self.clock.current_bucket.to_le_bytes())?;
        put(output, &mut at, &self.window_id)?;
        put(output, &mut at, &self.evidence_id)?;
        put(output, &mut at, &self.work_receipt_id)?;
        put(output, &mut at, &self.quote_schedule_id)?;
        put(
            output,
            &mut at,
            &self.scheduled_ceiling_lamports.to_le_bytes(),
        )?;
        put(output, &mut at, &self.reward_recipient.bytes())?;
        put(output, &mut at, &self.accepted_progress_total.to_le_bytes())?;
        put(output, &mut at, &self.terminal_join_id)?;
        put(output, &mut at, &self.refusal_code.to_le_bytes())?;
        if at != 332 {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Decode and fully validate one exact intent preimage.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != FAILURE_INTENT_V1_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != INTENT_MAGIC {
            return Err(Error::BadMagic);
        }
        if u16::from_le_bytes([input[8], input[9]]) != INTENT_SCHEMA {
            return Err(Error::BadVersion);
        }
        if input[11..16].iter().any(|byte| *byte != 0)
            || input[332..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalPadding);
        }
        let action = decode_action(input[10])?;
        let mut at = 16;
        let value = Self {
            action,
            binding_id: FailurePolicyBindingId::from_bytes(take(input, &mut at)?),
            market_instance_id: MarketInstanceV2Id::from_bytes(take(input, &mut at)?),
            generation: u64::from_le_bytes(take(input, &mut at)?),
            expected_transition_nonce: u64::from_le_bytes(take(input, &mut at)?),
            clock: RecoveryClock {
                slot: u64::from_le_bytes(take(input, &mut at)?),
                unix_timestamp: i64::from_le_bytes(take(input, &mut at)?),
                current_bucket: u64::from_le_bytes(take(input, &mut at)?),
            },
            window_id: take(input, &mut at)?,
            evidence_id: take(input, &mut at)?,
            work_receipt_id: take(input, &mut at)?,
            quote_schedule_id: take(input, &mut at)?,
            scheduled_ceiling_lamports: u64::from_le_bytes(take(input, &mut at)?),
            reward_recipient: RecoveryIdentity::from_bytes(take(input, &mut at)?),
            accepted_progress_total: u64::from_le_bytes(take(input, &mut at)?),
            terminal_join_id: take(input, &mut at)?,
            refusal_code: u32::from_le_bytes(take(input, &mut at)?),
        };
        if at != 332 {
            return Err(Error::WrongLength);
        }
        value.validate()?;
        Ok(value)
    }
}

/// Project an immutable-maturity failure trigger from one authenticated intent.
pub fn project_maturity_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(&accounts, &intent, FailureActionV1::TriggerMaturity)?;
    let plan = runtime.plan_trigger_maturity(intent.clock, accounts.reserve_lamports)?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project an immutable-maturity trigger classified by one exact refused
/// SourcePlane result.
pub fn project_source_refusal_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    result: &StatisticResultV3,
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    seal: &WindowSealV3,
    window: &WindowSpecV3,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(
        &accounts,
        &intent,
        FailureActionV1::TriggerSourceRefusal,
    )?;
    if intent.window_id != window.id()?.bytes()
        || intent.evidence_id != result.id()?.bytes()
        || intent.refusal_code != result.refusal_code()
    {
        return Err(Error::ArtifactMismatch);
    }
    let plan = runtime.plan_trigger_source_refusal(
        intent.clock,
        accounts.reserve_lamports,
        result,
        key,
        summary,
        seal,
        window,
    )?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project an immutable-maturity trigger classified by an adapter-authenticated
/// frozen-relation refusal over one successful SourcePlane result.
pub fn project_relation_refusal_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    refusal: AdapterAuthenticatedRelationRefusalV1,
    result: &StatisticResultV3,
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    seal: &WindowSealV3,
    window: &WindowSpecV3,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(
        &accounts,
        &intent,
        FailureActionV1::TriggerRelationRefusal,
    )?;
    if intent.window_id != window.id()?.bytes()
        || intent.evidence_id != result.id()?.bytes()
        || intent.refusal_code != refusal.refusal.code()
    {
        return Err(Error::ArtifactMismatch);
    }
    let plan = runtime.plan_trigger_relation_refusal(
        intent.clock,
        accounts.reserve_lamports,
        refusal,
        result,
        key,
        summary,
        seal,
        window,
    )?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project the deterministic finite repair-schedule advance or close.
pub fn project_schedule_advance_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(&accounts, &intent, FailureActionV1::AdvanceSchedule)?;
    let plan = runtime.plan_advance_schedule(intent.clock, accounts.reserve_lamports)?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project exact paid work from an independently authenticated liveness receipt.
///
/// The live adapter must authenticate the receipt under the liveness runtime
/// before obtaining `receipt`; this function then rechecks every receipt field
/// against the intent and current failure/recovery cursor.
pub fn project_accept_work_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    window: &WindowSpecV3,
    receipt: LivenessWorkReceiptJoinV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(&accounts, &intent, FailureActionV1::AcceptWork)?;
    validate_work_artifacts(&intent, window, receipt)?;
    let plan = runtime.plan_accept_liveness_work_progress(
        intent.clock,
        accounts.reserve_lamports,
        window,
        intent.reward_recipient,
        receipt,
    )?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project caller-funded resolution from one previously authenticated accepted
/// resolution capability. Caller funding never debits recovery principal.
pub fn project_caller_funded_resolution_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    accepted: AcceptedResolutionV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(
        &accounts,
        &intent,
        FailureActionV1::ResolveCallerFunded,
    )?;
    if intent.window_id != accepted.window_id().bytes()
        || intent.evidence_id != accepted.id().bytes()
    {
        return Err(Error::ArtifactMismatch);
    }
    let plan = runtime.plan_resolve_caller_funded(
        intent.clock,
        accounts.reserve_lamports,
        accepted,
    )?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Project resolution with one final independently authenticated paid-work
/// receipt and accepted evidence from that same repair Window.
pub fn project_paid_resolution_transition(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    window: &WindowSpecV3,
    receipt: LivenessWorkReceiptJoinV1,
    accepted: AcceptedResolutionV1,
    actual_reserve_post_balance: u64,
) -> Result<FailureAccountMutationV1> {
    let runtime = intent_runtime(&accounts, &intent, FailureActionV1::ResolvePaidWork)?;
    validate_work_artifacts(&intent, window, receipt)?;
    if intent.evidence_id != accepted.id().bytes()
        || intent.window_id != accepted.window_id().bytes()
    {
        return Err(Error::ArtifactMismatch);
    }
    let plan = runtime.plan_resolve_paid_liveness_progress(
        intent.clock,
        accounts.reserve_lamports,
        window,
        intent.reward_recipient,
        receipt,
        accepted,
    )?;
    project_failure_transition(accounts, plan, actual_reserve_post_balance)
}

/// Authenticate a terminal intent against the current resolved runtime and all
/// separately authenticated terminal-owner receipts embedded in `terminal`.
///
/// This emits no transfer and performs no state mutation. The terminal owner
/// must consume the returned join under its own replay tombstone.
pub fn authenticate_terminal_join_intent(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    terminal: FailureTerminalJoinV1,
) -> Result<FailureTerminalJoinV1> {
    let runtime = intent_runtime(&accounts, &intent, FailureActionV1::BindTerminal)?;
    let expected = FailureTerminalJoinV1::from_adapter(
        &runtime,
        terminal.generation(),
        terminal.retirement_root_id(),
        terminal.replay_tombstone_id(),
        terminal.source_release_receipt_id(),
    )?;
    if expected != terminal
        || intent.terminal_join_id != terminal.id().bytes()
        || terminal.binding_id() != intent.binding_id
        || terminal.market_instance_id() != intent.market_instance_id
        || terminal.generation() != intent.generation
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(terminal)
}

/// Authenticate the current finite recovery campaign's success/failure close
/// receipt for projection into the separately owned liveness runtime.
///
/// A dormant receipt closes only finite Recovery funding; it is not a market
/// settlement or retirement fact, and caller-funded recovery remains live.
pub fn authenticate_recovery_funding_close_intent(
    accounts: AuthenticatedFailureAccountsV1,
    intent: FailureIntentV1,
    receipt: FailureRecoveryTerminalReceiptV1,
) -> Result<FailureRecoveryTerminalReceiptV1> {
    let runtime = intent_runtime(
        &accounts,
        &intent,
        FailureActionV1::BindRecoveryFundingClose,
    )?;
    let expected = FailureRecoveryTerminalReceiptV1::from_runtime(&runtime)?;
    if expected != receipt
        || intent.terminal_join_id != receipt.id().bytes()
        || intent.binding_id != receipt.binding_id()
        || intent.market_instance_id != receipt.market_instance_id()
        || intent.generation != receipt.generation()
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(receipt)
}

fn intent_runtime(
    accounts: &AuthenticatedFailureAccountsV1,
    intent: &FailureIntentV1,
    expected_action: FailureActionV1,
) -> Result<FailureRuntimeV1> {
    if intent.action != expected_action {
        return Err(Error::WrongAction);
    }
    let runtime = accounts.root.runtime();
    intent.validate_runtime(&runtime)?;
    Ok(runtime)
}

fn validate_work_artifacts(
    intent: &FailureIntentV1,
    window: &WindowSpecV3,
    receipt: LivenessWorkReceiptJoinV1,
) -> Result<()> {
    if intent.window_id != window.id()?.bytes()
        || intent.work_receipt_id != receipt.work_receipt_id()
        || intent.quote_schedule_id != receipt.quote_schedule_id()
        || intent.scheduled_ceiling_lamports != receipt.scheduled_ceiling_lamports()
        || intent.accepted_progress_total != receipt.accepted_progress_total()
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn runtime_digest(bump: u8, runtime: &FailureRuntimeV1) -> Result<[u8; 32]> {
    let mut bytes = [0; FAILURE_RUNTIME_V1_BYTES];
    runtime.encode_into(&mut bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(ROOT_DIGEST_DOMAIN);
    hasher.update([bump]);
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn decode_action(value: u8) -> Result<FailureActionV1> {
    match value {
        1 => Ok(FailureActionV1::TriggerMaturity),
        2 => Ok(FailureActionV1::TriggerSourceRefusal),
        3 => Ok(FailureActionV1::TriggerRelationRefusal),
        4 => Ok(FailureActionV1::AdvanceSchedule),
        5 => Ok(FailureActionV1::AcceptWork),
        6 => Ok(FailureActionV1::ResolveCallerFunded),
        7 => Ok(FailureActionV1::ResolvePaidWork),
        8 => Ok(FailureActionV1::BindTerminal),
        9 => Ok(FailureActionV1::BindRecoveryFundingClose),
        _ => Err(Error::InvalidParameter),
    }
}

fn put(output: &mut [u8], at: &mut usize, value: &[u8]) -> Result<()> {
    let end = at.checked_add(value.len()).ok_or(Error::WrongLength)?;
    let target = output.get_mut(*at..end).ok_or(Error::WrongLength)?;
    target.copy_from_slice(value);
    *at = end;
    Ok(())
}

fn take<const N: usize>(input: &[u8], at: &mut usize) -> Result<[u8; N]> {
    let end = at.checked_add(N).ok_or(Error::WrongLength)?;
    let source = input.get(*at..end).ok_or(Error::WrongLength)?;
    let mut value = [0; N];
    value.copy_from_slice(source);
    *at = end;
    Ok(value)
}

fn is_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}
