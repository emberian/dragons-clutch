// SPDX-License-Identifier: AGPL-3.0-or-later
//! SBF-neutral persisted-account and atomic-movement adapter for runtime V1.
//!
//! Protocol-family adapters authenticate their own receipt codecs and project
//! the exact checked facts into [`RuntimeReceiptObservationV1`]. This module
//! then joins those facts to the persisted liveness policy/account, applies one
//! pure transition, and emits a fixed-capacity all-or-nothing write/transfer
//! bundle. It performs no CPI and has no Solana SDK dependency.

use super::runtime_v1::{
    RuntimeBalanceTransitionV1, RuntimeCallAuthorizationV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessBundleV1, RuntimeLivenessErrorV1, RuntimeLivenessPolicyV1,
    RuntimeTerminalAuthorizationV1, RUNTIME_COMPARTMENT_COUNT_V1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use super::Id;

/// Local intent-body magic. This is not a global instruction discriminator.
pub const RUNTIME_TRANSITION_INTENT_MAGIC_V1: [u8; 8] = *b"DCLINT01";
/// Exact local intent semantic version.
pub const RUNTIME_TRANSITION_INTENT_VERSION_V1: u16 = 1;
/// Exact bytes in one transition intent.
pub const RUNTIME_TRANSITION_INTENT_BYTES_V1: usize = 272;
/// Maximum distinct outgoing movements from one compartment transition.
pub const RUNTIME_MAX_TRANSFERS_V1: usize = 2;

/// Adapter refusal. Runtime arithmetic errors retain their exact typed cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAdapterErrorV1 {
    Runtime(RuntimeLivenessErrorV1),
    WrongProgramOwner,
    WrongPolicyAccount,
    WrongCompartmentAccount,
    AccountNotWritable,
    NoncanonicalIntent,
    MissingReceipt,
    UnexpectedReceipt,
    ReceiptMismatch,
    BalanceMismatch,
    TransferOverflow,
    TransferConservation,
    EmptyBatch,
    BatchTooWide,
    DuplicateAccount,
    DuplicateCompartment,
    DuplicateReceipt,
    BatchBindingMismatch,
    CodecLength,
    CodecMagic,
    CodecVersion,
    CodecReserved,
    CodecEnum,
}

impl From<RuntimeLivenessErrorV1> for RuntimeAdapterErrorV1 {
    fn from(value: RuntimeLivenessErrorV1) -> Self {
        Self::Runtime(value)
    }
}

pub type RuntimeAdapterResultV1<T> = Result<T, RuntimeAdapterErrorV1>;

fn live(id: Id) -> RuntimeAdapterResultV1<()> {
    if id.is_zero() {
        Err(RuntimeAdapterErrorV1::NoncanonicalIntent)
    } else {
        Ok(())
    }
}

fn add(left: u64, right: u64) -> RuntimeAdapterResultV1<u64> {
    left.checked_add(right)
        .ok_or(RuntimeAdapterErrorV1::TransferOverflow)
}

/// Read-only projection of an account supplied by the concrete runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePersistedAccountViewV1<'a> {
    pub account_id: Id,
    pub owner_program_id: Id,
    pub lamports: u64,
    pub data: &'a [u8],
    pub writable: bool,
}

/// Decode and authenticate one immutable policy account.
pub fn decode_runtime_policy_account_v1(
    expected_runtime_program_id: Id,
    expected_policy_account_id: Id,
    view: RuntimePersistedAccountViewV1<'_>,
) -> RuntimeAdapterResultV1<RuntimeLivenessPolicyV1> {
    live(expected_runtime_program_id)?;
    live(expected_policy_account_id)?;
    if view.owner_program_id != expected_runtime_program_id {
        return Err(RuntimeAdapterErrorV1::WrongProgramOwner);
    }
    if view.account_id != expected_policy_account_id {
        return Err(RuntimeAdapterErrorV1::WrongPolicyAccount);
    }
    if view.data.len() != RUNTIME_LIVENESS_POLICY_BYTES_V1 {
        return Err(RuntimeAdapterErrorV1::CodecLength);
    }
    Ok(RuntimeLivenessPolicyV1::decode(view.data)?)
}

/// Decode and authenticate one writable compartment account.
pub fn decode_runtime_compartment_account_v1(
    expected_runtime_program_id: Id,
    view: RuntimePersistedAccountViewV1<'_>,
) -> RuntimeAdapterResultV1<RuntimeCompartmentV1> {
    live(expected_runtime_program_id)?;
    if view.owner_program_id != expected_runtime_program_id {
        return Err(RuntimeAdapterErrorV1::WrongProgramOwner);
    }
    if !view.writable {
        return Err(RuntimeAdapterErrorV1::AccountNotWritable);
    }
    if view.data.len() != RUNTIME_LIVENESS_ACCOUNT_BYTES_V1 {
        return Err(RuntimeAdapterErrorV1::CodecLength);
    }
    let state = RuntimeCompartmentV1::decode(view.data)?;
    if state.identity.account_id != view.account_id {
        return Err(RuntimeAdapterErrorV1::WrongCompartmentAccount);
    }
    Ok(state)
}

/// Fresh predictable-account observation at bundle admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionAccountObservationV1 {
    pub account_id: Id,
    pub owner_program_id_before: Id,
    pub balance_before: u64,
    pub balance_after: u64,
    pub data_len_before: usize,
    pub writable: bool,
    pub executable: bool,
}

/// Exact allocate/assign/fund/write plan for one compartment account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionAccountPlanV1 {
    pub kind: RuntimeCompartmentKindV1,
    pub account_id: Id,
    pub owner_program_id_after: Id,
    pub payer: Id,
    pub payer_debit_lamports: u64,
    pub balance_before: u64,
    pub balance_after: u64,
    pub post_account_data: [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
}

/// All seven account creations which must succeed or roll back together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAtomicAdmissionV1 {
    pub policy_id: Id,
    pub lifecycle_id: Id,
    pub total_payer_debit_lamports: u64,
    pub bundle: RuntimeLivenessBundleV1,
    pub accounts: [RuntimeAdmissionAccountPlanV1; RUNTIME_COMPARTMENT_COUNT_V1],
}

/// Plan fresh predictable accounts from exact present payer debits.
///
/// Prefunded lamports remain donations: the payer still supplies the complete
/// work-plus-rent debit, and assignment/write of all seven accounts is atomic.
#[allow(clippy::too_many_arguments)]
pub fn plan_runtime_bundle_admission_v1(
    expected_runtime_program_id: Id,
    expected_uninitialized_program_id: Id,
    expected_policy_account_id: Id,
    policy_view: RuntimePersistedAccountViewV1<'_>,
    lifecycle_id: Id,
    admissions: [RuntimeCompartmentAdmissionV1; RUNTIME_COMPARTMENT_COUNT_V1],
    observations: [RuntimeAdmissionAccountObservationV1; RUNTIME_COMPARTMENT_COUNT_V1],
) -> RuntimeAdapterResultV1<RuntimeAtomicAdmissionV1> {
    live(expected_uninitialized_program_id)?;
    let policy = decode_runtime_policy_account_v1(
        expected_runtime_program_id,
        expected_policy_account_id,
        policy_view,
    )?;
    let bundle = RuntimeLivenessBundleV1::admit(policy, lifecycle_id, admissions)?;
    let first = admission_account_plan(
        expected_runtime_program_id,
        expected_uninitialized_program_id,
        bundle.compartments[0],
        admissions[0],
        observations[0],
    )?;
    let mut accounts = [first; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut total_payer_debit_lamports = 0u64;
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        accounts[index] = admission_account_plan(
            expected_runtime_program_id,
            expected_uninitialized_program_id,
            bundle.compartments[index],
            admissions[index],
            observations[index],
        )?;
        total_payer_debit_lamports = add(
            total_payer_debit_lamports,
            accounts[index].payer_debit_lamports,
        )?;
        index += 1;
    }
    if total_payer_debit_lamports != policy.total_payer_debit_lamports()? {
        return Err(RuntimeAdapterErrorV1::TransferConservation);
    }
    Ok(RuntimeAtomicAdmissionV1 {
        policy_id: policy.policy_id,
        lifecycle_id,
        total_payer_debit_lamports,
        bundle,
        accounts,
    })
}

fn admission_account_plan(
    expected_runtime_program_id: Id,
    expected_uninitialized_program_id: Id,
    state: RuntimeCompartmentV1,
    admission: RuntimeCompartmentAdmissionV1,
    observation: RuntimeAdmissionAccountObservationV1,
) -> RuntimeAdapterResultV1<RuntimeAdmissionAccountPlanV1> {
    if observation.account_id != state.identity.account_id
        || observation.account_id != admission.identity.account_id
        || observation.owner_program_id_before != expected_uninitialized_program_id
        || observation.balance_before != admission.funding.account_balance_before
        || observation.balance_after != admission.funding.account_balance_after
        || observation.data_len_before != 0
        || !observation.writable
        || observation.executable
    {
        return Err(RuntimeAdapterErrorV1::WrongCompartmentAccount);
    }
    let mut post_account_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    state.encode(&mut post_account_data)?;
    Ok(RuntimeAdmissionAccountPlanV1 {
        kind: state.kind,
        account_id: state.identity.account_id,
        owner_program_id_after: expected_runtime_program_id,
        payer: state.identity.payer,
        payer_debit_lamports: admission.funding.payer_debit_lamports,
        balance_before: observation.balance_before,
        balance_after: observation.balance_after,
        post_account_data,
    })
}

/// One persisted-account transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeTransitionActionV1 {
    ObserveDonation = 0,
    SpendWork = 1,
    CloseSuccess = 2,
    CloseFailure = 3,
}

impl RuntimeTransitionActionV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::ObserveDonation => 0,
            Self::SpendWork => 1,
            Self::CloseSuccess => 2,
            Self::CloseFailure => 3,
        }
    }

    fn decode(value: u8) -> RuntimeAdapterResultV1<Self> {
        match value {
            0 => Ok(Self::ObserveDonation),
            1 => Ok(Self::SpendWork),
            2 => Ok(Self::CloseSuccess),
            3 => Ok(Self::CloseFailure),
            _ => Err(RuntimeAdapterErrorV1::CodecEnum),
        }
    }
}

/// Fixed intent shared by General, Source, Series, recovery, and retirement.
///
/// Balance observations are deliberately not user-controlled intent bytes;
/// the concrete adapter obtains them from the real account immediately before
/// and after applying the emitted movements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTransitionIntentV1 {
    pub action: RuntimeTransitionActionV1,
    pub kind: RuntimeCompartmentKindV1,
    pub policy_id: Id,
    pub lifecycle_id: Id,
    pub account_id: Id,
    pub semantic_owner: Id,
    pub quote_schedule_id: Id,
    pub receipt_id: Id,
    pub keeper: Id,
    pub generation: u64,
    pub call_ordinal: u32,
    pub call_ceiling_lamports: u64,
    pub keeper_payment_lamports: u64,
    pub flags: u16,
}

impl RuntimeTransitionIntentV1 {
    pub fn validate(self) -> RuntimeAdapterResultV1<()> {
        for identity in [
            self.policy_id,
            self.lifecycle_id,
            self.account_id,
            self.semantic_owner,
            self.quote_schedule_id,
        ] {
            live(identity)?;
        }
        if self.flags != 0 {
            return Err(RuntimeAdapterErrorV1::NoncanonicalIntent);
        }
        match self.action {
            RuntimeTransitionActionV1::ObserveDonation => {
                if !self.receipt_id.is_zero()
                    || !self.keeper.is_zero()
                    || self.call_ordinal != 0
                    || self.call_ceiling_lamports != 0
                    || self.keeper_payment_lamports != 0
                {
                    return Err(RuntimeAdapterErrorV1::NoncanonicalIntent);
                }
            }
            RuntimeTransitionActionV1::SpendWork => {
                live(self.receipt_id)?;
                live(self.keeper)?;
                if self.call_ordinal == 0
                    || self.call_ceiling_lamports == 0
                    || self.keeper_payment_lamports > self.call_ceiling_lamports
                {
                    return Err(RuntimeAdapterErrorV1::NoncanonicalIntent);
                }
            }
            RuntimeTransitionActionV1::CloseSuccess
            | RuntimeTransitionActionV1::CloseFailure => {
                live(self.receipt_id)?;
                if !self.keeper.is_zero()
                    || self.call_ordinal != 0
                    || self.call_ceiling_lamports != 0
                    || self.keeper_payment_lamports != 0
                {
                    return Err(RuntimeAdapterErrorV1::NoncanonicalIntent);
                }
            }
        }
        Ok(())
    }

    pub fn encode(self, output: &mut [u8]) -> RuntimeAdapterResultV1<()> {
        self.validate()?;
        let mut writer = Writer::exact(output, RUNTIME_TRANSITION_INTENT_BYTES_V1)?;
        writer.array(RUNTIME_TRANSITION_INTENT_MAGIC_V1)?;
        writer.u16(RUNTIME_TRANSITION_INTENT_VERSION_V1)?;
        writer.u8(self.action.byte())?;
        writer.u8(kind_byte(self.kind))?;
        writer.u16(self.flags)?;
        writer.reserved(2)?;
        for identity in [
            self.policy_id,
            self.lifecycle_id,
            self.account_id,
            self.semantic_owner,
            self.quote_schedule_id,
            self.receipt_id,
            self.keeper,
        ] {
            writer.id(identity)?;
        }
        writer.u64(self.generation)?;
        writer.u32(self.call_ordinal)?;
        writer.reserved(4)?;
        writer.u64(self.call_ceiling_lamports)?;
        writer.u64(self.keeper_payment_lamports)?;
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> RuntimeAdapterResultV1<Self> {
        let mut reader = Reader::exact(input, RUNTIME_TRANSITION_INTENT_BYTES_V1)?;
        if reader.array::<8>()? != RUNTIME_TRANSITION_INTENT_MAGIC_V1 {
            return Err(RuntimeAdapterErrorV1::CodecMagic);
        }
        if reader.u16()? != RUNTIME_TRANSITION_INTENT_VERSION_V1 {
            return Err(RuntimeAdapterErrorV1::CodecVersion);
        }
        let action = RuntimeTransitionActionV1::decode(reader.u8()?)?;
        let kind = decode_kind(reader.u8()?)?;
        let flags = reader.u16()?;
        reader.reserved(2)?;
        let value = Self {
            action,
            kind,
            policy_id: reader.id()?,
            lifecycle_id: reader.id()?,
            account_id: reader.id()?,
            semantic_owner: reader.id()?,
            quote_schedule_id: reader.id()?,
            receipt_id: reader.id()?,
            keeper: reader.id()?,
            generation: reader.u64()?,
            call_ordinal: reader.u32()?,
            call_ceiling_lamports: {
                reader.reserved(4)?;
                reader.u64()?
            },
            keeper_payment_lamports: reader.u64()?,
            flags,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Upstream receipt type after its family-specific owner/PDA/codec checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeReceiptKindV1 {
    WorkCompleted = 0,
    TerminalSuccess = 1,
    TerminalFailure = 2,
}

/// SBF-neutral projection of a family-specific authenticated receipt.
///
/// This module checks every field against policy, state, and intent. The
/// calling family adapter remains responsible for producing these facts only
/// after checking the actual receipt account owner, PDA, codec, and status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReceiptObservationV1 {
    pub receipt_account_id: Id,
    pub receipt_account_owner_program_id: Id,
    pub receipt_id: Id,
    pub receipt_kind: RuntimeReceiptKindV1,
    pub compartment_kind: RuntimeCompartmentKindV1,
    pub semantic_owner: Id,
    pub lifecycle_id: Id,
    pub quote_schedule_id: Id,
    pub generation: u64,
    pub call_ordinal: u32,
    pub call_ceiling_lamports: u64,
}

impl RuntimeReceiptObservationV1 {
    fn validate_for(
        self,
        state: RuntimeCompartmentV1,
        intent: RuntimeTransitionIntentV1,
    ) -> RuntimeAdapterResultV1<()> {
        for identity in [
            self.receipt_account_id,
            self.receipt_account_owner_program_id,
            self.receipt_id,
            self.semantic_owner,
            self.lifecycle_id,
            self.quote_schedule_id,
        ] {
            live(identity)?;
        }
        if self.receipt_account_owner_program_id != state.receipt_program_id
            || self.receipt_id != intent.receipt_id
            || self.compartment_kind != state.kind
            || self.compartment_kind != intent.kind
            || self.semantic_owner != state.identity.owner
            || self.semantic_owner != intent.semantic_owner
            || self.lifecycle_id != state.identity.lifecycle_id
            || self.lifecycle_id != intent.lifecycle_id
            || self.quote_schedule_id != state.quote_schedule_id
            || self.quote_schedule_id != intent.quote_schedule_id
            || self.generation != state.identity.generation
            || self.generation != intent.generation
        {
            return Err(RuntimeAdapterErrorV1::ReceiptMismatch);
        }
        match intent.action {
            RuntimeTransitionActionV1::ObserveDonation => {
                return Err(RuntimeAdapterErrorV1::UnexpectedReceipt)
            }
            RuntimeTransitionActionV1::SpendWork => {
                if self.receipt_kind != RuntimeReceiptKindV1::WorkCompleted
                    || self.call_ordinal != intent.call_ordinal
                    || self.call_ceiling_lamports != intent.call_ceiling_lamports
                {
                    return Err(RuntimeAdapterErrorV1::ReceiptMismatch);
                }
            }
            RuntimeTransitionActionV1::CloseSuccess => {
                if self.receipt_kind != RuntimeReceiptKindV1::TerminalSuccess
                    || self.call_ordinal != 0
                    || self.call_ceiling_lamports != 0
                {
                    return Err(RuntimeAdapterErrorV1::ReceiptMismatch);
                }
            }
            RuntimeTransitionActionV1::CloseFailure => {
                if self.receipt_kind != RuntimeReceiptKindV1::TerminalFailure
                    || self.call_ordinal != 0
                    || self.call_ceiling_lamports != 0
                {
                    return Err(RuntimeAdapterErrorV1::ReceiptMismatch);
                }
            }
        }
        Ok(())
    }
}

/// Economic destination of one emitted lamport movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeTransferRoleV1 {
    KeeperPayment = 0,
    PayerWorkRefund = 1,
    PayerTerminalRefund = 2,
    NeutralTerminalSink = 3,
}

/// One exact movement which the concrete adapter applies atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLamportTransferV1 {
    pub destination: Id,
    pub lamports: u64,
    pub role: RuntimeTransferRoleV1,
}

impl RuntimeLamportTransferV1 {
    const EMPTY: Self = Self {
        destination: Id::ZERO,
        lamports: 0,
        role: RuntimeTransferRoleV1::KeeperPayment,
    };
}

/// Fixed-capacity result that must be applied in one atomic transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAtomicTransitionV1 {
    pub action: RuntimeTransitionActionV1,
    pub kind: RuntimeCompartmentKindV1,
    pub account_id: Id,
    pub account_balance_before: u64,
    pub account_balance_after: u64,
    pub state_before: RuntimeCompartmentV1,
    pub state_after: RuntimeCompartmentV1,
    pub write_account_data: bool,
    pub close_account: bool,
    pub post_account_data: [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
    transfers: [RuntimeLamportTransferV1; RUNTIME_MAX_TRANSFERS_V1],
    transfer_count: usize,
}

impl RuntimeAtomicTransitionV1 {
    pub fn transfers(&self) -> &[RuntimeLamportTransferV1] {
        &self.transfers[..self.transfer_count]
    }
}

/// Multiple compartment plans which the concrete adapter applies or rolls
/// back as one instruction. `N` is bounded by the seven canonical accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAtomicTransitionBatchV1<const N: usize> {
    pub policy_id: Id,
    pub lifecycle_id: Id,
    pub total_account_outflow_lamports: u64,
    pub transitions: [RuntimeAtomicTransitionV1; N],
}

/// Join independently planned compartment mutations into one atomic bundle.
///
/// Account and semantic receipt identities are disjoint. A family action which
/// spans, for example, Candidate and Settlement cannot apply only one debit.
pub fn join_runtime_atomic_transition_batch_v1<const N: usize>(
    policy_id: Id,
    lifecycle_id: Id,
    transitions: [RuntimeAtomicTransitionV1; N],
) -> RuntimeAdapterResultV1<RuntimeAtomicTransitionBatchV1<N>> {
    live(policy_id)?;
    live(lifecycle_id)?;
    if N == 0 {
        return Err(RuntimeAdapterErrorV1::EmptyBatch);
    }
    if N > RUNTIME_COMPARTMENT_COUNT_V1 {
        return Err(RuntimeAdapterErrorV1::BatchTooWide);
    }
    let mut total_account_outflow_lamports = 0u64;
    let mut index = 0usize;
    while index < N {
        let transition = transitions[index];
        if transition.state_before.identity.policy_id != policy_id
            || transition.state_after.identity.policy_id != policy_id
            || transition.state_before.identity.lifecycle_id != lifecycle_id
            || transition.state_after.identity.lifecycle_id != lifecycle_id
            || transition.state_before.identity.account_id != transition.account_id
            || transition.state_after.identity.account_id != transition.account_id
        {
            return Err(RuntimeAdapterErrorV1::BatchBindingMismatch);
        }
        let outflow = transition
            .account_balance_before
            .checked_sub(transition.account_balance_after)
            .ok_or(RuntimeAdapterErrorV1::BalanceMismatch)?;
        total_account_outflow_lamports = add(total_account_outflow_lamports, outflow)?;
        let receipt_id = transition_receipt_id(transition);
        let mut prior = 0usize;
        while prior < index {
            if transitions[prior].account_id == transition.account_id {
                return Err(RuntimeAdapterErrorV1::DuplicateAccount);
            }
            if transitions[prior].kind == transition.kind {
                return Err(RuntimeAdapterErrorV1::DuplicateCompartment);
            }
            let prior_receipt_id = transition_receipt_id(transitions[prior]);
            if !receipt_id.is_zero() && receipt_id == prior_receipt_id {
                return Err(RuntimeAdapterErrorV1::DuplicateReceipt);
            }
            prior += 1;
        }
        index += 1;
    }
    Ok(RuntimeAtomicTransitionBatchV1 {
        policy_id,
        lifecycle_id,
        total_account_outflow_lamports,
        transitions,
    })
}

fn transition_receipt_id(transition: RuntimeAtomicTransitionV1) -> Id {
    match transition.action {
        RuntimeTransitionActionV1::ObserveDonation => Id::ZERO,
        RuntimeTransitionActionV1::SpendWork => transition.state_after.last_work_receipt_id,
        RuntimeTransitionActionV1::CloseSuccess | RuntimeTransitionActionV1::CloseFailure => {
            transition.state_after.terminal_receipt_id
        }
    }
}

/// Authenticate and plan one persisted transition without CPI or allocation.
#[allow(clippy::too_many_arguments)]
pub fn plan_runtime_transition_v1(
    expected_runtime_program_id: Id,
    expected_policy_account_id: Id,
    policy_view: RuntimePersistedAccountViewV1<'_>,
    compartment_view: RuntimePersistedAccountViewV1<'_>,
    intent: RuntimeTransitionIntentV1,
    receipt: Option<RuntimeReceiptObservationV1>,
    account_balance_after: u64,
) -> RuntimeAdapterResultV1<RuntimeAtomicTransitionV1> {
    intent.validate()?;
    let policy = decode_runtime_policy_account_v1(
        expected_runtime_program_id,
        expected_policy_account_id,
        policy_view,
    )?;
    let state_before =
        decode_runtime_compartment_account_v1(expected_runtime_program_id, compartment_view)?;
    state_before.validate_against_policy(policy)?;
    if intent.policy_id != policy.policy_id
        || intent.policy_id != state_before.identity.policy_id
        || intent.lifecycle_id != state_before.identity.lifecycle_id
        || intent.account_id != state_before.identity.account_id
        || intent.semantic_owner != state_before.identity.owner
        || intent.quote_schedule_id != state_before.quote_schedule_id
        || intent.generation != state_before.identity.generation
        || intent.kind != state_before.kind
    {
        return Err(RuntimeAdapterErrorV1::NoncanonicalIntent);
    }
    if compartment_view.lamports < state_before.expected_account_balance_lamports()? {
        return Err(RuntimeAdapterErrorV1::BalanceMismatch);
    }

    let mut transfers = [RuntimeLamportTransferV1::EMPTY; RUNTIME_MAX_TRANSFERS_V1];
    let mut transfer_count = 0usize;
    let balances = RuntimeBalanceTransitionV1 {
        account_balance_before: compartment_view.lamports,
        account_balance_after,
    };
    let state_after = match intent.action {
        RuntimeTransitionActionV1::ObserveDonation => {
            if receipt.is_some() {
                return Err(RuntimeAdapterErrorV1::UnexpectedReceipt);
            }
            if account_balance_after != compartment_view.lamports {
                return Err(RuntimeAdapterErrorV1::BalanceMismatch);
            }
            state_before.observe_balance(compartment_view.lamports)?
        }
        RuntimeTransitionActionV1::SpendWork => {
            let receipt = receipt.ok_or(RuntimeAdapterErrorV1::MissingReceipt)?;
            receipt.validate_for(state_before, intent)?;
            let (state_after, movement) = state_before.spend_call(
                RuntimeCallAuthorizationV1 {
                    kind: intent.kind,
                    account: intent.account_id,
                    owner: intent.semantic_owner,
                    generation: intent.generation,
                    quote_schedule_id: intent.quote_schedule_id,
                    call_ordinal: intent.call_ordinal,
                    call_ceiling_lamports: intent.call_ceiling_lamports,
                    work_receipt_id: intent.receipt_id,
                },
                intent.keeper,
                intent.keeper_payment_lamports,
                balances,
            )?;
            push_transfer(
                &mut transfers,
                &mut transfer_count,
                RuntimeLamportTransferV1 {
                    destination: movement.keeper,
                    lamports: movement.keeper_lamports,
                    role: RuntimeTransferRoleV1::KeeperPayment,
                },
            )?;
            push_transfer(
                &mut transfers,
                &mut transfer_count,
                RuntimeLamportTransferV1 {
                    destination: movement.payer,
                    lamports: movement.payer_refund_lamports,
                    role: RuntimeTransferRoleV1::PayerWorkRefund,
                },
            )?;
            state_after
        }
        RuntimeTransitionActionV1::CloseSuccess => {
            let receipt = receipt.ok_or(RuntimeAdapterErrorV1::MissingReceipt)?;
            receipt.validate_for(state_before, intent)?;
            let (state_after, movement) = state_before.close_success(
                RuntimeTerminalAuthorizationV1 {
                    kind: intent.kind,
                    account: intent.account_id,
                    owner: intent.semantic_owner,
                    generation: intent.generation,
                    terminal_receipt_id: intent.receipt_id,
                },
                balances,
            )?;
            push_terminal_transfers(
                &mut transfers,
                &mut transfer_count,
                movement.payer,
                movement.payer_refund_lamports,
                movement.neutral_sink,
                movement.neutral_lamports,
            )?;
            state_after
        }
        RuntimeTransitionActionV1::CloseFailure => {
            let receipt = receipt.ok_or(RuntimeAdapterErrorV1::MissingReceipt)?;
            receipt.validate_for(state_before, intent)?;
            let (state_after, movement) = state_before.close_failure(
                RuntimeTerminalAuthorizationV1 {
                    kind: intent.kind,
                    account: intent.account_id,
                    owner: intent.semantic_owner,
                    generation: intent.generation,
                    terminal_receipt_id: intent.receipt_id,
                },
                balances,
            )?;
            push_terminal_transfers(
                &mut transfers,
                &mut transfer_count,
                movement.payer,
                movement.payer_refund_lamports,
                movement.neutral_sink,
                movement.neutral_lamports,
            )?;
            state_after
        }
    };

    let close_account = state_after.phase != RuntimeCompartmentPhaseV1::Active;
    let write_account_data = !close_account;
    let mut post_account_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    if write_account_data {
        state_after.encode(&mut post_account_data)?;
    }
    let mut transfer_total = 0u64;
    let mut index = 0usize;
    while index < transfer_count {
        transfer_total = add(transfer_total, transfers[index].lamports)?;
        index += 1;
    }
    let observed_outflow = compartment_view
        .lamports
        .checked_sub(account_balance_after)
        .ok_or(RuntimeAdapterErrorV1::BalanceMismatch)?;
    if transfer_total != observed_outflow {
        return Err(RuntimeAdapterErrorV1::TransferConservation);
    }
    Ok(RuntimeAtomicTransitionV1 {
        action: intent.action,
        kind: intent.kind,
        account_id: intent.account_id,
        account_balance_before: compartment_view.lamports,
        account_balance_after,
        state_before,
        state_after,
        write_account_data,
        close_account,
        post_account_data,
        transfers,
        transfer_count,
    })
}

fn push_terminal_transfers(
    transfers: &mut [RuntimeLamportTransferV1; RUNTIME_MAX_TRANSFERS_V1],
    transfer_count: &mut usize,
    payer: Id,
    payer_lamports: u64,
    neutral_sink: Id,
    neutral_lamports: u64,
) -> RuntimeAdapterResultV1<()> {
    push_transfer(
        transfers,
        transfer_count,
        RuntimeLamportTransferV1 {
            destination: payer,
            lamports: payer_lamports,
            role: RuntimeTransferRoleV1::PayerTerminalRefund,
        },
    )?;
    push_transfer(
        transfers,
        transfer_count,
        RuntimeLamportTransferV1 {
            destination: neutral_sink,
            lamports: neutral_lamports,
            role: RuntimeTransferRoleV1::NeutralTerminalSink,
        },
    )
}

fn push_transfer(
    transfers: &mut [RuntimeLamportTransferV1; RUNTIME_MAX_TRANSFERS_V1],
    transfer_count: &mut usize,
    transfer: RuntimeLamportTransferV1,
) -> RuntimeAdapterResultV1<()> {
    if transfer.lamports == 0 {
        return Ok(());
    }
    live(transfer.destination)?;
    if *transfer_count == RUNTIME_MAX_TRANSFERS_V1 {
        return Err(RuntimeAdapterErrorV1::TransferOverflow);
    }
    transfers[*transfer_count] = transfer;
    *transfer_count += 1;
    Ok(())
}

fn kind_byte(kind: RuntimeCompartmentKindV1) -> u8 {
    match kind {
        RuntimeCompartmentKindV1::Source => 0,
        RuntimeCompartmentKindV1::Candidate => 1,
        RuntimeCompartmentKindV1::Clearing => 2,
        RuntimeCompartmentKindV1::Settlement => 3,
        RuntimeCompartmentKindV1::Resolution => 4,
        RuntimeCompartmentKindV1::Retirement => 5,
        RuntimeCompartmentKindV1::Recovery => 6,
    }
}

fn decode_kind(value: u8) -> RuntimeAdapterResultV1<RuntimeCompartmentKindV1> {
    match value {
        0 => Ok(RuntimeCompartmentKindV1::Source),
        1 => Ok(RuntimeCompartmentKindV1::Candidate),
        2 => Ok(RuntimeCompartmentKindV1::Clearing),
        3 => Ok(RuntimeCompartmentKindV1::Settlement),
        4 => Ok(RuntimeCompartmentKindV1::Resolution),
        5 => Ok(RuntimeCompartmentKindV1::Retirement),
        6 => Ok(RuntimeCompartmentKindV1::Recovery),
        _ => Err(RuntimeAdapterErrorV1::CodecEnum),
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
}

impl<'a> Writer<'a> {
    fn exact(output: &'a mut [u8], expected: usize) -> RuntimeAdapterResultV1<Self> {
        if output.len() != expected {
            return Err(RuntimeAdapterErrorV1::CodecLength);
        }
        output.fill(0);
        Ok(Self { output, cursor: 0 })
    }

    fn array<const N: usize>(&mut self, value: [u8; N]) -> RuntimeAdapterResultV1<()> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        self.output
            .get_mut(self.cursor..end)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?
            .copy_from_slice(&value);
        self.cursor = end;
        Ok(())
    }

    fn id(&mut self, value: Id) -> RuntimeAdapterResultV1<()> {
        self.array(value.bytes())
    }

    fn u8(&mut self, value: u8) -> RuntimeAdapterResultV1<()> {
        self.array([value])
    }

    fn u16(&mut self, value: u16) -> RuntimeAdapterResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> RuntimeAdapterResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> RuntimeAdapterResultV1<()> {
        self.array(value.to_le_bytes())
    }

    fn reserved(&mut self, count: usize) -> RuntimeAdapterResultV1<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        if self.output.get(self.cursor..end).is_none() {
            return Err(RuntimeAdapterErrorV1::CodecLength);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> RuntimeAdapterResultV1<()> {
        if self.cursor != self.output.len() {
            return Err(RuntimeAdapterErrorV1::CodecLength);
        }
        Ok(())
    }
}

struct Reader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn exact(input: &'a [u8], expected: usize) -> RuntimeAdapterResultV1<Self> {
        if input.len() != expected {
            return Err(RuntimeAdapterErrorV1::CodecLength);
        }
        Ok(Self { input, cursor: 0 })
    }

    fn array<const N: usize>(&mut self) -> RuntimeAdapterResultV1<[u8; N]> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        let source = self
            .input
            .get(self.cursor..end)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        let mut output = [0u8; N];
        output.copy_from_slice(source);
        self.cursor = end;
        Ok(output)
    }

    fn id(&mut self) -> RuntimeAdapterResultV1<Id> {
        Ok(Id::from_bytes(self.array()?))
    }

    fn u8(&mut self) -> RuntimeAdapterResultV1<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> RuntimeAdapterResultV1<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> RuntimeAdapterResultV1<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> RuntimeAdapterResultV1<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn reserved(&mut self, count: usize) -> RuntimeAdapterResultV1<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        let reserved = self
            .input
            .get(self.cursor..end)
            .ok_or(RuntimeAdapterErrorV1::CodecLength)?;
        if reserved.iter().any(|byte| *byte != 0) {
            return Err(RuntimeAdapterErrorV1::CodecReserved);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> RuntimeAdapterResultV1<()> {
        if self.cursor != self.input.len() {
            return Err(RuntimeAdapterErrorV1::CodecLength);
        }
        Ok(())
    }
}
