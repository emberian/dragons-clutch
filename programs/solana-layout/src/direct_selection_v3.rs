//! Versioned hostile-byte ownership for the bounded direct-selection V3 cut.
//!
//! These codecs allocate no runtime authority. They freeze the exact account
//! shapes required by the executable V3 lifecycle model: immutable schedules,
//! full-width verifier release identity, bounded retained candidates, staged
//! verification, and payer-principal/donation separation. Earlier direct and
//! reservation versions remain distinct and fail closed by version and length.

use clutch_batch_policy_identity::{
    batch_policy_digest, canonical_batch_policy_bytes, decode_batch_policy,
    direct_lifecycle_v3::{
        DirectCandidateLeaseV3 as ModelCandidateLeaseV3,
        DirectCandidateStageV3 as ModelCandidateStageV3,
        DirectLifecyclePhaseV3 as ModelLifecyclePhaseV3,
        DirectTerminalReasonV3 as ModelTerminalReasonV3,
        DirectTerminalReceiptV3 as ModelTerminalReceiptV3, MAX_DIRECT_TICKS_V3,
        MAX_SELECTION_SPAN_V3, MAX_SETTLEMENT_SPAN_V3, MAX_SUBMISSION_SPAN_V3,
        MIN_SELECTION_SPAN_V3, MIN_SETTLEMENT_SPAN_V3, MIN_SUBMISSION_SPAN_V3,
    },
    direct_window_v1::{
        DirectCandidateEntryV1, DirectCandidateV2, DirectCandidateWindowV1, DirectWindowErrorV1,
        DIRECT_CANDIDATE_ACCOUNT_BYTES, DIRECT_CANDIDATE_STATUS_SELECTED,
        DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_WINDOW_ACCOUNT_BYTES, DIRECT_WINDOW_PHASE_OPEN,
        DIRECT_WINDOW_PHASE_SELECTED, MAX_DIRECT_CANDIDATES,
    },
    Identity32V1, BATCH_POLICY_BYTES,
};

use super::{
    digest, put_header,
    reservation::{
        ReservationAccount, RESERVATION_ACCOUNT_BYTES, RESERVATION_ACCOUNT_TAG,
        RESERVATION_ACCOUNT_VERSION,
    },
    CodecError, Hash32, Reader, Result, Writer, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN, EPOCH_PHASE_SETTLED, EPOCH_TAG, MAX_OUTCOMES,
};

use super::direct_selection::{
    DirectEpochV3Account, DIRECT_CANDIDATE_TAG, DIRECT_EPOCH_BYTES, DIRECT_WINDOW_TAG,
};

/// Direct Epoch schema carrying the complete V3 lifecycle schedule and receipt.
pub const DIRECT_EPOCH_V4_VERSION: u8 = 4;
/// Exact Direct Epoch V4 byte length.
pub const DIRECT_EPOCH_V4_BYTES: usize = 673;
/// Direct Candidate schema with exact rent/donation ownership.
pub const DIRECT_CANDIDATE_V3_VERSION: u8 = 2;
/// Exact Direct Candidate V3 byte length.
pub const DIRECT_CANDIDATE_V3_BYTES: usize = 488;
/// Direct Window schema with staged-work and exact funding ownership.
pub const DIRECT_WINDOW_V3_VERSION: u8 = 2;
/// Exact Direct Window V3 byte length.
pub const DIRECT_WINDOW_V3_BYTES: usize = 632;
/// Direct WorkBudget account discriminator.
pub const DIRECT_WORK_BUDGET_TAG: u8 = 23;
/// First Direct WorkBudget schema.
pub const DIRECT_WORK_BUDGET_VERSION: u8 = 1;
/// Exact Direct WorkBudget byte length.
pub const DIRECT_WORK_BUDGET_BYTES: usize = 248;
/// Reservation schema with exact payer principal and neutral donation fields.
pub const DIRECT_RESERVATION_V2_VERSION: u8 = 2;
/// Exact Reservation V2 byte length: the direct plane's frozen 570-byte
/// reservation body plus its 48-byte funding ownership tail.
///
/// Deliberately a literal, *not* derived from [`RESERVATION_ACCOUNT_BYTES`].
/// The general clearing plane's reservation grew to 610 bytes when it took on
/// the per-order partial-fill ledger; the direct plane fills whole orders in
/// one shot, carries no ledger, and stays byte-frozen at 618.  The two
/// schemas share the tag and are separated by version and length.
pub const DIRECT_RESERVATION_V2_BYTES: usize = 570 + 32 + 8 + 8;
/// Exact DirectBatchPolicy V3 artifact body length.
pub const DIRECT_BATCH_POLICY_V3_BYTES: usize = BATCH_POLICY_BYTES + 32;

/// Init Direct Epoch V4 intent tag.
pub const INIT_DIRECT_EPOCH_V4_TAG: u8 = 36;
/// Freeze Direct Epoch V4 and its prepaid work intent tag.
pub const FREEZE_DIRECT_EPOCH_V4_TAG: u8 = 37;
/// Abort an unfrozen Direct Epoch V4 intent tag.
pub const ABORT_UNFROZEN_DIRECT_V4_TAG: u8 = 38;
/// Submit one fully verified direct Candidate V3 intent tag.
pub const SUBMIT_DIRECT_CANDIDATE_V3_TAG: u8 = 39;
/// Begin staged direct verification intent tag.
pub const BEGIN_DIRECT_VERIFICATION_V3_TAG: u8 = 40;
/// Verify one retained direct Candidate intent tag.
pub const VERIFY_DIRECT_CANDIDATE_V3_TAG: u8 = 41;
/// Finalize staged direct selection intent tag.
pub const FINALIZE_DIRECT_SELECTION_V3_TAG: u8 = 42;
/// Settle exact selected direct authority intent tag.
pub const SETTLE_DIRECT_V3_TAG: u8 = 43;
/// Lapse a frozen-empty direct epoch intent tag.
pub const LAPSE_EMPTY_DIRECT_V3_TAG: u8 = 44;
/// Lapse an unselected nonempty direct window intent tag.
pub const LAPSE_UNSELECTED_DIRECT_V3_TAG: u8 = 45;
/// Lapse selected direct authority intent tag.
pub const LAPSE_SELECTED_DIRECT_V3_TAG: u8 = 46;
/// Last allocated common intent tag in this codec revision.
pub const LAST_DIRECT_V3_INTENT_TAG: u8 = LAPSE_SELECTED_DIRECT_V3_TAG;

/// Init Direct Epoch V4 wire bytes.
pub const INIT_DIRECT_EPOCH_V4_BYTES: usize = 138;
/// Freeze Direct Epoch V4 wire bytes.
pub const FREEZE_DIRECT_EPOCH_V4_BYTES: usize = 114;
/// Common two-identity V3 action wire bytes.
pub const DIRECT_V3_COMMON_ACTION_BYTES: usize = 66;
/// Submit Direct Candidate V3 wire bytes.
pub const SUBMIT_DIRECT_CANDIDATE_V3_BYTES: usize = 74;
/// Verify one retained Candidate wire bytes.
pub const VERIFY_DIRECT_CANDIDATE_V3_BYTES: usize = 67;

/// Direct Epoch exists before its book is frozen.
pub const DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN: u8 = 0;
/// Frozen two-order authority exists without a competitive candidate.
pub const DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY: u8 = 1;
/// A bounded candidate window is accepting submissions.
pub const DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN: u8 = 2;
/// Retained candidates are being reverified one transaction at a time.
pub const DIRECT_LIFECYCLE_PHASE_VERIFYING: u8 = 3;
/// One candidate is selected and both reservations are entitled.
pub const DIRECT_LIFECYCLE_PHASE_SELECTED: u8 = 4;
/// Transient authority is gone and the Epoch carries the durable receipt.
pub const DIRECT_LIFECYCLE_PHASE_TERMINAL: u8 = 5;

/// No competitive candidate existed at the selection deadline.
///
/// Zero is also the preterminal placeholder; the lifecycle phase owns that
/// distinction, so the same bytes never claim terminality by themselves.
pub const DIRECT_TERMINAL_REASON_EMPTY_LAPSE: u8 = 0;
/// Preterminal placeholder alias. It is valid only outside `TERMINAL`.
pub const DIRECT_TERMINAL_REASON_NONE: u8 = DIRECT_TERMINAL_REASON_EMPTY_LAPSE;
/// A nonempty window did not finish selection before its deadline.
pub const DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE: u8 = 1;
/// Selected authority did not settle before its deadline.
pub const DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE: u8 = 2;
/// The exact selected pair settled atomically.
pub const DIRECT_TERMINAL_REASON_SETTLED: u8 = 3;
/// The Epoch reached submission open without ever freezing.
pub const DIRECT_TERMINAL_REASON_PREFREEZE_ABORT: u8 = 4;

/// Project the model lifecycle enum to its frozen wire value. Never cast the
/// model enum: its Rust discriminants are deliberately not protocol bytes.
pub const fn direct_lifecycle_phase_wire(phase: ModelLifecyclePhaseV3) -> u8 {
    match phase {
        ModelLifecyclePhaseV3::FrozenEmpty => DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        ModelLifecyclePhaseV3::WindowOpen => DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
        ModelLifecyclePhaseV3::Verifying => DIRECT_LIFECYCLE_PHASE_VERIFYING,
        ModelLifecyclePhaseV3::Selected => DIRECT_LIFECYCLE_PHASE_SELECTED,
        ModelLifecyclePhaseV3::Terminal => DIRECT_LIFECYCLE_PHASE_TERMINAL,
    }
}

/// Project a model terminal reason to its frozen wire value.
pub const fn direct_terminal_reason_wire(reason: ModelTerminalReasonV3) -> u8 {
    match reason {
        ModelTerminalReasonV3::EmptyLapse => DIRECT_TERMINAL_REASON_EMPTY_LAPSE,
        ModelTerminalReasonV3::PreSelectionLapse => DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE,
        ModelTerminalReasonV3::PostSelectionLapse => DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE,
        ModelTerminalReasonV3::Settled => DIRECT_TERMINAL_REASON_SETTLED,
        ModelTerminalReasonV3::PrefreezeAbort => DIRECT_TERMINAL_REASON_PREFREEZE_ABORT,
    }
}

/// Candidate status after its independent staged reexecution.
pub const DIRECT_CANDIDATE_STATUS_REVERIFIED: u8 = 5;
/// Window phase while retained candidates are independently reexecuted.
pub const DIRECT_WINDOW_PHASE_VERIFYING: u8 = 2;
/// Active WorkBudget phase. A terminal budget account is closed, not persisted.
pub const DIRECT_WORK_BUDGET_PHASE_ACTIVE: u8 = 1;

/// Domain separating the epoch-context-bound DirectBatchPolicy V3 digest.
pub const DIRECT_BATCH_POLICY_V3_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/direct-batch-policy/v3\0";

const CANDIDATE_STATUS_ACCOUNT_OFFSET: usize = 425;
const CANDIDATE_EXTENSION_OFFSET: usize = DIRECT_CANDIDATE_ACCOUNT_BYTES;
const WINDOW_EXTENSION_OFFSET: usize = DIRECT_WINDOW_ACCOUNT_BYTES;

const _: () = assert!(DIRECT_EPOCH_V4_BYTES == DIRECT_EPOCH_BYTES + 328);
const _: () = assert!(DIRECT_CANDIDATE_V3_BYTES == DIRECT_CANDIDATE_ACCOUNT_BYTES + 48);
const _: () = assert!(DIRECT_WINDOW_V3_BYTES == DIRECT_WINDOW_ACCOUNT_BYTES + 176);
const _: () = assert!(RESERVATION_ACCOUNT_BYTES == 610);
const _: () = assert!(DIRECT_RESERVATION_V2_BYTES == 618);
const _: () = assert!(DIRECT_RESERVATION_V2_VERSION != RESERVATION_ACCOUNT_VERSION);
const _: () = assert!(DIRECT_BATCH_POLICY_V3_BYTES == 96);

fn map_direct_error(error: DirectWindowErrorV1) -> CodecError {
    match error {
        DirectWindowErrorV1::WrongLength => CodecError::Truncated,
        DirectWindowErrorV1::ZeroIdentity => CodecError::ZeroIdentity,
        DirectWindowErrorV1::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        DirectWindowErrorV1::MismatchedBinding
        | DirectWindowErrorV1::NotDirect
        | DirectWindowErrorV1::Relation(_) => CodecError::MismatchedBinding,
        DirectWindowErrorV1::NonCanonical => CodecError::NonCanonicalPadding,
        DirectWindowErrorV1::BeforeOpen
        | DirectWindowErrorV1::SubmissionClosed
        | DirectWindowErrorV1::SelectionEarly
        | DirectWindowErrorV1::AlreadySelected
        | DirectWindowErrorV1::Replay => CodecError::InvalidEnum,
    }
}

fn identity(value: Hash32) -> Identity32V1 {
    Identity32V1(value.bytes())
}

fn hash(value: Identity32V1) -> Hash32 {
    Hash32::from_bytes(value.0)
}

fn nonzero(value: Hash32) -> Result<()> {
    Hash32::new(value.bytes()).map(|_| ())
}

fn prefix_mask(count: u8) -> Result<u8> {
    if usize::from(count) > MAX_DIRECT_CANDIDATES {
        return Err(CodecError::InvalidCount);
    }
    Ok(if count == 0 { 0 } else { (1u8 << count) - 1 })
}

fn checked_span(start: u64, end: u64, min: u64, max: u64) -> Result<()> {
    let span = end.checked_sub(start).ok_or(CodecError::InvalidCount)?;
    if !(min..=max).contains(&span) {
        return Err(CodecError::InvalidCount);
    }
    Ok(())
}

/// Persisted exact payer contribution plus the neutral-donation lower bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFundingLedgerV3 {
    /// Authenticated payer that receives exactly its recorded principal.
    pub payer: Hash32,
    /// Exact payer-funded principal; prefunds never reduce it.
    pub payer_principal_lamports: u64,
    /// Previously authenticated neutral donation lower bound.
    pub prior_donation_lamports: u64,
}

impl DirectFundingLedgerV3 {
    /// Canonical absent funding ledger.
    pub const ZERO: Self = Self {
        payer: Hash32::ZERO,
        payer_principal_lamports: 0,
        prior_donation_lamports: 0,
    };

    /// Whether every field is canonical absent padding.
    pub fn is_zero(self) -> bool {
        self.payer_principal_lamports == 0
            && self.prior_donation_lamports == 0
            && self.payer.0 == [0; 32]
    }

    /// Validate a live account ledger against its immutable neutral sink.
    pub fn validate_for_sink(self, neutral_sink: Hash32) -> Result<()> {
        nonzero(self.payer)?;
        nonzero(neutral_sink)?;
        if self.payer == neutral_sink || self.payer_principal_lamports == 0 {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
}

fn write_ledger(writer: &mut Writer<'_>, ledger: DirectFundingLedgerV3) -> Result<()> {
    writer.hash(ledger.payer)?;
    writer.u64(ledger.payer_principal_lamports)?;
    writer.u64(ledger.prior_donation_lamports)
}

fn read_ledger(reader: &mut Reader<'_>) -> Result<DirectFundingLedgerV3> {
    Ok(DirectFundingLedgerV3 {
        payer: reader.hash()?,
        payer_principal_lamports: reader.u64()?,
        prior_donation_lamports: reader.u64()?,
    })
}

/// Durable terminal fields embedded in Direct Epoch V4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTerminalReceiptV3 {
    /// Terminal reason, or `NONE` before terminality.
    pub reason: u8,
    /// Settled outcome; zero for lapse and before terminality.
    pub outcome: u8,
    /// Exact Reservation prefix archived by the terminal transition.
    pub terminal_reservation_count: u8,
    /// Selection slot; zero if no candidate was selected.
    pub selected_slot: u64,
    /// Selected Candidate identity; zero before selection.
    pub candidate: Hash32,
    /// Full relation-candidate digest; zero before selection.
    pub relation_candidate_digest: Hash32,
    /// Settled quantity; zero for lapse.
    pub quantity: u64,
    /// Settled price units; zero for lapse.
    pub price: u64,
    /// Exact quantity times price units; zero for lapse.
    pub consideration_price_units: u128,
    /// Slot of terminal transition; zero before terminality.
    pub terminal_slot: u64,
}

impl DirectTerminalReceiptV3 {
    /// Canonical preterminal placeholder.
    pub const EMPTY: Self = Self {
        reason: DIRECT_TERMINAL_REASON_NONE,
        outcome: 0,
        terminal_reservation_count: 0,
        selected_slot: 0,
        candidate: Hash32::ZERO,
        relation_candidate_digest: Hash32::ZERO,
        quantity: 0,
        price: 0,
        consideration_price_units: 0,
        terminal_slot: 0,
    };
}

/// Byte-exact projection of the executable model receipt.
pub const fn project_model_terminal_receipt(
    receipt: ModelTerminalReceiptV3,
) -> DirectTerminalReceiptV3 {
    DirectTerminalReceiptV3 {
        reason: direct_terminal_reason_wire(receipt.reason),
        outcome: receipt.outcome,
        terminal_reservation_count: receipt.terminal_reservation_count,
        selected_slot: receipt.selected_slot,
        candidate: Hash32::from_bytes(receipt.candidate_id.0),
        relation_candidate_digest: Hash32::from_bytes(receipt.relation_candidate_digest.0),
        quantity: receipt.quantity,
        price: receipt.price,
        consideration_price_units: receipt.consideration_price_units,
        terminal_slot: receipt.terminal_slot,
    }
}

/// Version-four direct Epoch with complete schedule and durable terminal audit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEpochV4Account {
    /// Exact Direct Epoch V3 prefix.
    pub direct: DirectEpochV3Account,
    /// Exclusive staged-selection deadline.
    pub selection_deadline_slot: u64,
    /// Exclusive post-selection settlement deadline.
    pub settlement_deadline_slot: u64,
    /// Fine-grained direct lifecycle phase.
    pub lifecycle_phase: u8,
    /// Durable terminal fields.
    pub terminal: DirectTerminalReceiptV3,
    /// Realm-authenticated destination for all unsolicited lamports.
    pub neutral_lamport_sink: Hash32,
    /// Compile-time semantic verifier release identifier.
    ///
    /// This is not an ELF, ProgramData, deployment, or source hash.
    pub verifier_release_id: Hash32,
    /// Epoch-bound identity of the exact 96-byte DirectBatchPolicy artifact.
    pub direct_policy_v3_id: Hash32,
    /// Durable Epoch rent principal and prefund donation ownership.
    pub epoch_funding: DirectFundingLedgerV3,
    /// Sole page-zero rent principal and prefund donation ownership.
    pub page_funding: DirectFundingLedgerV3,
    /// Canonical zero reserve.
    pub reserved: [u8; 4],
}

impl DirectEpochV4Account {
    /// Validate exact schedules, phase mirroring, and terminal receipt shape.
    pub fn validate(&self) -> Result<()> {
        let prefreeze_shape = self.direct.common.phase == EPOCH_PHASE_OPEN
            && (self.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
                || (self.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_TERMINAL
                    && self.terminal.reason == DIRECT_TERMINAL_REASON_PREFREEZE_ABORT));
        if prefreeze_shape {
            if self.direct.common.page_count > 1 {
                return Err(CodecError::InvalidCount);
            }
            let mut normalized = self.direct;
            normalized.common.page_count = 0;
            normalized.validate()?;
        } else {
            self.direct.validate()?;
        }
        nonzero(self.neutral_lamport_sink)?;
        nonzero(self.verifier_release_id)?;
        nonzero(self.direct_policy_v3_id)?;
        self.epoch_funding
            .validate_for_sink(self.neutral_lamport_sink)?;
        if self.direct.common.page_count == 0 {
            if !self.page_funding.is_zero() {
                return Err(CodecError::MismatchedBinding);
            }
        } else {
            self.page_funding
                .validate_for_sink(self.neutral_lamport_sink)?;
        }
        let exact_policy = DirectBatchPolicyV3::direct(self.verifier_release_id)?;
        if self.direct.common.policy.0
            != batch_policy_digest(
                &clutch_batch_policy_identity::direct_window_v1::DIRECT_POLICY_V1,
            )
            .map_err(|_| CodecError::MismatchedBinding)?
            .0
            || exact_policy.digest_for_epoch(self.direct.common.epoch)? != self.direct_policy_v3_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        checked_span(
            self.direct.submission_opens_slot,
            self.direct.submission_closes_slot,
            MIN_SUBMISSION_SPAN_V3,
            MAX_SUBMISSION_SPAN_V3,
        )?;
        checked_span(
            self.direct.submission_closes_slot,
            self.selection_deadline_slot,
            MIN_SELECTION_SPAN_V3,
            MAX_SELECTION_SPAN_V3,
        )?;
        checked_span(
            self.selection_deadline_slot,
            self.settlement_deadline_slot,
            MIN_SETTLEMENT_SPAN_V3,
            MAX_SETTLEMENT_SPAN_V3,
        )?;
        if self.reserved.iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        match self.lifecycle_phase {
            DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN => {
                if self.direct.common.phase != EPOCH_PHASE_OPEN
                    || self.terminal != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY
            | DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN
            | DIRECT_LIFECYCLE_PHASE_VERIFYING => {
                if self.direct.common.phase != EPOCH_PHASE_FROZEN
                    || self.terminal != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_LIFECYCLE_PHASE_SELECTED => {
                if self.direct.common.phase != EPOCH_PHASE_CLEARED
                    || self.terminal.reason != DIRECT_TERMINAL_REASON_NONE
                    || self.terminal.terminal_reservation_count != 0
                    || self.terminal.selected_slot < self.direct.submission_closes_slot
                    || self.terminal.selected_slot >= self.selection_deadline_slot
                    || self.terminal.candidate == Hash32::ZERO
                    || self.terminal.relation_candidate_digest == Hash32::ZERO
                    || self.terminal.outcome != 0
                    || self.terminal.quantity != 0
                    || self.terminal.price != 0
                    || self.terminal.consideration_price_units != 0
                    || self.terminal.terminal_slot != 0
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_LIFECYCLE_PHASE_TERMINAL => self.validate_terminal()?,
            _ => return Err(CodecError::InvalidEnum),
        }
        Ok(())
    }

    /// Validate the generic byte shape and bind it to one verifier release.
    ///
    /// Every semantics-bearing runtime handler must use this boundary rather
    /// than treating a self-consistent caller-selected release as authority.
    pub fn validate_for_release(&self, expected_release: Hash32) -> Result<()> {
        self.validate()?;
        nonzero(expected_release)?;
        if self.verifier_release_id != expected_release {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Require the sole V4 phase in which the existing PlaceOrder wire may
    /// create a Reservation V2. Coarse `OPEN` alone is insufficient because a
    /// durable pre-freeze abort intentionally retains that coarse phase.
    pub fn require_prefreeze_placement(&self) -> Result<()> {
        self.validate()?;
        if self.lifecycle_phase != DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
            || self.direct.common.phase != EPOCH_PHASE_OPEN
            || self.terminal != DirectTerminalReceiptV3::EMPTY
        {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    fn validate_terminal(&self) -> Result<()> {
        let terminal = self.terminal;
        match terminal.reason {
            DIRECT_TERMINAL_REASON_PREFREEZE_ABORT => {
                if self.direct.common.phase != EPOCH_PHASE_OPEN
                    || terminal.selected_slot != 0
                    || terminal.terminal_reservation_count > 2
                    || (terminal.terminal_reservation_count == 0
                        && (terminal.candidate != Hash32::ZERO
                            || terminal.relation_candidate_digest != Hash32::ZERO))
                    || (terminal.terminal_reservation_count == 1
                        && (terminal.candidate == Hash32::ZERO
                            || terminal.relation_candidate_digest != Hash32::ZERO))
                    || (terminal.terminal_reservation_count == 2
                        && (terminal.candidate == Hash32::ZERO
                            || terminal.relation_candidate_digest == Hash32::ZERO
                            || terminal.candidate == terminal.relation_candidate_digest))
                    || terminal.outcome != 0
                    || terminal.quantity != 0
                    || terminal.price != 0
                    || terminal.consideration_price_units != 0
                    || terminal.terminal_slot < self.direct.submission_opens_slot
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_TERMINAL_REASON_EMPTY_LAPSE | DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE => {
                if self.direct.common.phase != EPOCH_PHASE_LAPSED
                    || terminal.terminal_reservation_count != 2
                    || terminal.selected_slot != 0
                    || terminal.candidate != Hash32::ZERO
                    || terminal.relation_candidate_digest != Hash32::ZERO
                    || terminal.outcome != 0
                    || terminal.quantity != 0
                    || terminal.price != 0
                    || terminal.consideration_price_units != 0
                    || terminal.terminal_slot < self.selection_deadline_slot
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE => {
                if self.direct.common.phase != EPOCH_PHASE_LAPSED
                    || terminal.terminal_reservation_count != 2
                    || terminal.selected_slot < self.direct.submission_closes_slot
                    || terminal.selected_slot >= self.selection_deadline_slot
                    || terminal.candidate == Hash32::ZERO
                    || terminal.relation_candidate_digest == Hash32::ZERO
                    || terminal.outcome != 0
                    || terminal.quantity != 0
                    || terminal.price != 0
                    || terminal.consideration_price_units != 0
                    || terminal.terminal_slot < self.settlement_deadline_slot
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_TERMINAL_REASON_SETTLED => {
                nonzero(terminal.candidate)?;
                nonzero(terminal.relation_candidate_digest)?;
                let expected = u128::from(terminal.quantity)
                    .checked_mul(u128::from(terminal.price))
                    .ok_or(CodecError::ArithmeticOverflow)?;
                if self.direct.common.phase != EPOCH_PHASE_SETTLED
                    || terminal.terminal_reservation_count != 2
                    || terminal.selected_slot < self.direct.submission_closes_slot
                    || terminal.selected_slot >= self.selection_deadline_slot
                    || terminal.outcome >= 2
                    || terminal.quantity == 0
                    || terminal.price == 0
                    || terminal.consideration_price_units != expected
                    || terminal.terminal_slot < terminal.selected_slot
                    || terminal.terminal_slot >= self.settlement_deadline_slot
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            _ => return Err(CodecError::InvalidEnum),
        }
        Ok(())
    }

    /// Encode exactly [`DIRECT_EPOCH_V4_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < DIRECT_EPOCH_V4_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let e = &self.direct.common;
        let mut writer = Writer::new(out);
        put_header(&mut writer, EPOCH_TAG, DIRECT_EPOCH_V4_VERSION)?;
        for value in [
            e.epoch,
            e.market,
            e.book,
            e.terms,
            e.price_grid,
            e.policy,
            e.order_set,
            e.first_order_id,
            e.last_order_id,
        ] {
            writer.hash(value)?;
        }
        writer.u64(e.epoch_index)?;
        writer.u32(e.relation_version)?;
        writer.u64(e.price_scale)?;
        writer.u64(e.remainder_seed)?;
        writer.u16(e.owner_count)?;
        writer.u16(e.page_count)?;
        writer.u16(e.order_count)?;
        writer.u8(e.outcome_count)?;
        writer.u8(e.basis_degree)?;
        writer.u8(e.phase)?;
        writer.u64(self.direct.submission_opens_slot)?;
        writer.u64(self.direct.submission_closes_slot)?;
        writer.u8(e.stored_bump)?;
        writer.u8(e.flags)?;
        writer.u64(self.selection_deadline_slot)?;
        writer.u64(self.settlement_deadline_slot)?;
        writer.u8(self.lifecycle_phase)?;
        writer.u8(self.terminal.reason)?;
        writer.u8(self.terminal.outcome)?;
        writer.u8(self.terminal.terminal_reservation_count)?;
        writer.u64(self.terminal.selected_slot)?;
        writer.hash(self.terminal.candidate)?;
        writer.hash(self.terminal.relation_candidate_digest)?;
        writer.u64(self.terminal.quantity)?;
        writer.u64(self.terminal.price)?;
        writer.u128(self.terminal.consideration_price_units)?;
        writer.u64(self.terminal.terminal_slot)?;
        writer.hash(self.neutral_lamport_sink)?;
        writer.hash(self.verifier_release_id)?;
        writer.hash(self.direct_policy_v3_id)?;
        write_ledger(&mut writer, self.epoch_funding)?;
        write_ledger(&mut writer, self.page_funding)?;
        writer.bytes(&self.reserved)?;
        Ok(writer.at)
    }

    /// Decode exactly Epoch tag/version 4. Every earlier Epoch refuses.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            EPOCH_TAG,
            DIRECT_EPOCH_V4_VERSION,
            DIRECT_EPOCH_V4_BYTES,
        )?;
        let common = super::EpochAccount {
            epoch: reader.hash()?,
            market: reader.hash()?,
            book: reader.hash()?,
            terms: reader.hash()?,
            price_grid: reader.hash()?,
            policy: reader.hash()?,
            order_set: reader.hash()?,
            first_order_id: reader.hash()?,
            last_order_id: reader.hash()?,
            epoch_index: reader.u64()?,
            relation_version: reader.u32()?,
            price_scale: reader.u64()?,
            remainder_seed: reader.u64()?,
            owner_count: reader.u16()?,
            page_count: reader.u16()?,
            order_count: reader.u16()?,
            outcome_count: reader.u8()?,
            basis_degree: reader.u8()?,
            phase: reader.u8()?,
            stored_bump: 0,
            flags: 0,
        };
        let submission_opens_slot = reader.u64()?;
        let submission_closes_slot = reader.u64()?;
        let mut direct = DirectEpochV3Account {
            common,
            submission_opens_slot,
            submission_closes_slot,
        };
        direct.common.stored_bump = reader.u8()?;
        direct.common.flags = reader.u8()?;
        let value = Self {
            direct,
            selection_deadline_slot: reader.u64()?,
            settlement_deadline_slot: reader.u64()?,
            lifecycle_phase: reader.u8()?,
            terminal: DirectTerminalReceiptV3 {
                reason: reader.u8()?,
                outcome: reader.u8()?,
                terminal_reservation_count: reader.u8()?,
                selected_slot: reader.u64()?,
                candidate: reader.hash()?,
                relation_candidate_digest: reader.hash()?,
                quantity: reader.u64()?,
                price: reader.u64()?,
                consideration_price_units: reader.u128()?,
                terminal_slot: reader.u64()?,
            },
            neutral_lamport_sink: reader.hash()?,
            verifier_release_id: reader.hash()?,
            direct_policy_v3_id: reader.hash()?,
            epoch_funding: read_ledger(&mut reader)?,
            page_funding: read_ledger(&mut reader)?,
            reserved: reader.bytes()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

fn validate_candidate_common(candidate: &DirectCandidateV2) -> Result<()> {
    if !matches!(
        candidate.status,
        DIRECT_CANDIDATE_STATUS_VERIFIED
            | DIRECT_CANDIDATE_STATUS_REVERIFIED
            | DIRECT_CANDIDATE_STATUS_SELECTED
    ) {
        return Err(CodecError::InvalidEnum);
    }
    let mut structural = *candidate;
    structural.status = DIRECT_CANDIDATE_STATUS_VERIFIED;
    structural.validate().map_err(map_direct_error)
}

/// Direct Candidate V3 with exact refund and neutral-donation ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateV3Account {
    /// Full relation-owned Candidate body.
    pub candidate: DirectCandidateV2,
    /// Exact transient-account funding ownership.
    pub funding: DirectFundingLedgerV3,
}

impl DirectCandidateV3Account {
    /// Validate structural Candidate bytes and funding ownership.
    pub fn validate(&self, neutral_sink: Hash32) -> Result<()> {
        validate_candidate_common(&self.candidate)?;
        self.funding.validate_for_sink(neutral_sink)
    }

    /// Encode exactly [`DIRECT_CANDIDATE_V3_BYTES`] bytes.
    pub fn encode(&self, neutral_sink: Hash32, out: &mut [u8]) -> Result<usize> {
        self.validate(neutral_sink)?;
        if out.len() < DIRECT_CANDIDATE_V3_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            DIRECT_CANDIDATE_TAG,
            DIRECT_CANDIDATE_V3_VERSION,
        )?;
        write_candidate_body(&mut writer, &self.candidate)?;
        write_ledger(&mut writer, self.funding)?;
        Ok(writer.at)
    }

    /// Decode exactly Candidate tag/version 2 and validate against the sink.
    pub fn decode(input: &[u8], neutral_sink: Hash32) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            DIRECT_CANDIDATE_TAG,
            DIRECT_CANDIDATE_V3_VERSION,
            DIRECT_CANDIDATE_V3_BYTES,
        )?;
        let value = Self {
            candidate: read_candidate_body(&mut reader)?,
            funding: read_ledger(&mut reader)?,
        };
        reader.done()?;
        value.validate(neutral_sink)?;
        Ok(value)
    }
}

fn write_candidate_body(writer: &mut Writer<'_>, value: &DirectCandidateV2) -> Result<()> {
    for item in [
        value.candidate_id,
        value.epoch_id,
        value.market_id,
        value.order_set_id,
        value.policy_id,
        value.relation_domain_digest,
        value.relation_candidate_digest,
    ] {
        writer.hash(hash(item))?;
    }
    for price in value.prices {
        writer.u64(price)?;
    }
    for fill in value.fills {
        writer.u64(fill)?;
    }
    writer.i128(value.weighted_direct_volume)?;
    writer.u128(value.limit_surplus_price_units)?;
    writer.u64(value.submitted_slot)?;
    writer.u64(value.quantity)?;
    writer.u8(value.buy_index)?;
    writer.u8(value.sell_index)?;
    writer.u8(value.outcome)?;
    writer.u16(value.distinct_owners)?;
    writer.u8(value.order_len)?;
    writer.u8(value.outcome_count)?;
    writer.u8(value.status)?;
    writer.u8(value.stored_bump)?;
    writer.u8(value.flags)?;
    writer.bytes(&value.reserved)
}

fn read_candidate_body(reader: &mut Reader<'_>) -> Result<DirectCandidateV2> {
    let candidate_id = identity(reader.hash()?);
    let epoch_id = identity(reader.hash()?);
    let market_id = identity(reader.hash()?);
    let order_set_id = identity(reader.hash()?);
    let policy_id = identity(reader.hash()?);
    let relation_domain_digest = identity(reader.hash()?);
    let relation_candidate_digest = identity(reader.hash()?);
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        prices[index] = reader.u64()?;
        index += 1;
    }
    Ok(DirectCandidateV2 {
        candidate_id,
        epoch_id,
        market_id,
        order_set_id,
        policy_id,
        relation_domain_digest,
        relation_candidate_digest,
        prices,
        fills: [reader.u64()?, reader.u64()?],
        weighted_direct_volume: reader.i128()?,
        limit_surplus_price_units: reader.u128()?,
        submitted_slot: reader.u64()?,
        quantity: reader.u64()?,
        buy_index: reader.u8()?,
        sell_index: reader.u8()?,
        outcome: reader.u8()?,
        distinct_owners: reader.u16()?,
        order_len: reader.u8()?,
        outcome_count: reader.u8()?,
        status: reader.u8()?,
        stored_bump: reader.u8()?,
        flags: reader.u8()?,
        reserved: reader.bytes()?,
    })
}

fn validate_window_common(window: &DirectCandidateWindowV1) -> Result<()> {
    let mut structural = *window;
    if structural.phase == DIRECT_WINDOW_PHASE_VERIFYING {
        structural.phase = DIRECT_WINDOW_PHASE_OPEN;
    }
    structural.validate().map_err(map_direct_error)?;
    let mut index = 0usize;
    while index < usize::from(window.top_count) {
        let mut prior = 0usize;
        while prior < index {
            if window.top[prior].relation_candidate_digest
                == window.top[index].relation_candidate_digest
            {
                return Err(CodecError::MismatchedBinding);
            }
            prior += 1;
        }
        index += 1;
    }
    Ok(())
}

/// Direct Window V3 with bounded replay, staged work, and funding ledgers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectWindowV3Account {
    /// Full Window V1 identity/top prefix with V3 phase semantics.
    pub window: DirectCandidateWindowV1,
    /// Exact Window account funding ownership.
    pub funding: DirectFundingLedgerV3,
    /// Ticks ever competitively admitted.
    pub seen_competitive_ticks: u64,
    /// Candidates already reexecuted in the current staged pass.
    pub verification_mask: u8,
    /// Candidate accounts which remain physically live.
    pub live_candidate_mask: u8,
    /// Reserved extension flags; zero.
    pub extension_flags: u16,
    /// Exclusive staged selection deadline.
    pub selection_deadline_slot: u64,
    /// Exclusive settlement deadline.
    pub settlement_deadline_slot: u64,
    /// Receipt funding, absent until selection.
    pub receipt_funding: DirectFundingLedgerV3,
    /// Pot funding, absent until selection.
    pub pot_funding: DirectFundingLedgerV3,
    /// Canonical zero reserve.
    pub reserved: [u8; 4],
}

impl DirectWindowV3Account {
    /// Validate all local hostile fields against the immutable neutral sink.
    pub fn validate(&self, neutral_sink: Hash32) -> Result<()> {
        validate_window_common(&self.window)?;
        self.funding.validate_for_sink(neutral_sink)?;
        checked_span(
            self.window.opens_slot,
            self.window.closes_slot,
            MIN_SUBMISSION_SPAN_V3,
            MAX_SUBMISSION_SPAN_V3,
        )?;
        checked_span(
            self.window.closes_slot,
            self.selection_deadline_slot,
            MIN_SELECTION_SPAN_V3,
            MAX_SELECTION_SPAN_V3,
        )?;
        checked_span(
            self.selection_deadline_slot,
            self.settlement_deadline_slot,
            MIN_SETTLEMENT_SPAN_V3,
            MAX_SETTLEMENT_SPAN_V3,
        )?;
        if self.window.admitted_count > u64::from(MAX_DIRECT_TICKS_V3)
            || self.window.admitted_count != u64::from(self.seen_competitive_ticks.count_ones())
            || self.extension_flags != 0
            || self.reserved.iter().any(|byte| *byte != 0)
        {
            return Err(CodecError::NonCanonicalPadding);
        }
        let prefix = prefix_mask(self.window.top_count)?;
        if self.verification_mask & !prefix != 0 || self.live_candidate_mask & !prefix != 0 {
            return Err(CodecError::InvalidCount);
        }
        match self.window.phase {
            DIRECT_WINDOW_PHASE_OPEN => {
                if self.verification_mask != 0
                    || self.live_candidate_mask != prefix
                    || !self.receipt_funding.is_zero()
                    || !self.pot_funding.is_zero()
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_WINDOW_PHASE_VERIFYING => {
                if self.live_candidate_mask != prefix
                    || !self.receipt_funding.is_zero()
                    || !self.pot_funding.is_zero()
                {
                    return Err(CodecError::MismatchedBinding);
                }
            }
            DIRECT_WINDOW_PHASE_SELECTED => {
                if self.verification_mask != prefix || self.live_candidate_mask != 1 {
                    return Err(CodecError::MismatchedBinding);
                }
                self.receipt_funding.validate_for_sink(neutral_sink)?;
                self.pot_funding.validate_for_sink(neutral_sink)?;
            }
            _ => return Err(CodecError::InvalidEnum),
        }
        Ok(())
    }

    /// Encode exactly [`DIRECT_WINDOW_V3_BYTES`] bytes.
    pub fn encode(&self, neutral_sink: Hash32, out: &mut [u8]) -> Result<usize> {
        self.validate(neutral_sink)?;
        if out.len() < DIRECT_WINDOW_V3_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(&mut writer, DIRECT_WINDOW_TAG, DIRECT_WINDOW_V3_VERSION)?;
        write_window_body(&mut writer, &self.window)?;
        write_ledger(&mut writer, self.funding)?;
        writer.u64(self.seen_competitive_ticks)?;
        writer.u8(self.verification_mask)?;
        writer.u8(self.live_candidate_mask)?;
        writer.u16(self.extension_flags)?;
        writer.u64(self.selection_deadline_slot)?;
        writer.u64(self.settlement_deadline_slot)?;
        write_ledger(&mut writer, self.receipt_funding)?;
        write_ledger(&mut writer, self.pot_funding)?;
        writer.bytes(&self.reserved)?;
        Ok(writer.at)
    }

    /// Decode exactly Window tag/version 2 and validate against the sink.
    pub fn decode(input: &[u8], neutral_sink: Hash32) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            DIRECT_WINDOW_TAG,
            DIRECT_WINDOW_V3_VERSION,
            DIRECT_WINDOW_V3_BYTES,
        )?;
        let value = Self {
            window: read_window_body(&mut reader)?,
            funding: read_ledger(&mut reader)?,
            seen_competitive_ticks: reader.u64()?,
            verification_mask: reader.u8()?,
            live_candidate_mask: reader.u8()?,
            extension_flags: reader.u16()?,
            selection_deadline_slot: reader.u64()?,
            settlement_deadline_slot: reader.u64()?,
            receipt_funding: read_ledger(&mut reader)?,
            pot_funding: read_ledger(&mut reader)?,
            reserved: reader.bytes()?,
        };
        reader.done()?;
        value.validate(neutral_sink)?;
        Ok(value)
    }
}

fn write_window_body(writer: &mut Writer<'_>, value: &DirectCandidateWindowV1) -> Result<()> {
    for item in [
        value.epoch_id,
        value.market_id,
        value.order_set_id,
        value.policy_id,
        value.relation_domain_digest,
        value.admission_transcript,
        value.selected_candidate,
    ] {
        writer.hash(hash(item))?;
    }
    for entry in value.top {
        writer.hash(hash(entry.candidate_id))?;
        writer.hash(hash(entry.relation_candidate_digest))?;
    }
    writer.u64(value.opens_slot)?;
    writer.u64(value.closes_slot)?;
    writer.u64(value.selected_slot)?;
    writer.u64(value.admitted_count)?;
    writer.u8(value.top_count)?;
    writer.u8(value.phase)?;
    writer.u8(value.stored_bump)?;
    writer.u8(value.flags)?;
    writer.bytes(&value.reserved)
}

fn read_window_body(reader: &mut Reader<'_>) -> Result<DirectCandidateWindowV1> {
    let epoch_id = identity(reader.hash()?);
    let market_id = identity(reader.hash()?);
    let order_set_id = identity(reader.hash()?);
    let policy_id = identity(reader.hash()?);
    let relation_domain_digest = identity(reader.hash()?);
    let admission_transcript = identity(reader.hash()?);
    let selected_candidate = identity(reader.hash()?);
    let mut top = [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES];
    let mut index = 0usize;
    while index < MAX_DIRECT_CANDIDATES {
        top[index] = DirectCandidateEntryV1 {
            candidate_id: identity(reader.hash()?),
            relation_candidate_digest: identity(reader.hash()?),
        };
        index += 1;
    }
    Ok(DirectCandidateWindowV1 {
        epoch_id,
        market_id,
        order_set_id,
        policy_id,
        relation_domain_digest,
        admission_transcript,
        selected_candidate,
        top,
        opens_slot: reader.u64()?,
        closes_slot: reader.u64()?,
        selected_slot: reader.u64()?,
        admitted_count: reader.u64()?,
        top_count: reader.u8()?,
        phase: reader.u8()?,
        stored_bump: reader.u8()?,
        flags: reader.u8()?,
        reserved: reader.bytes()?,
    })
}

/// Frozen strictly-positive keeper reward schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectKeeperRewardsV3 {
    /// Begin staged verification.
    pub begin_verification: u64,
    /// Reexecute one exact Candidate.
    pub verify_candidate: u64,
    /// Finalize selection and entitlement.
    pub finalize_selection: u64,
    /// Execute exact settlement.
    pub settle: u64,
    /// Execute any permissionless lapse.
    pub lapse: u64,
}

impl DirectKeeperRewardsV3 {
    /// Compute the exact maximum frozen reward obligation.
    pub fn worst_case(self) -> Result<u64> {
        if self.begin_verification == 0
            || self.verify_candidate == 0
            || self.finalize_selection == 0
            || self.settle == 0
            || self.lapse == 0
        {
            return Err(CodecError::ZeroValue);
        }
        self.verify_candidate
            .checked_mul(MAX_DIRECT_CANDIDATES as u64)
            .and_then(|value| value.checked_add(self.begin_verification))
            .and_then(|value| value.checked_add(self.finalize_selection))
            .and_then(|value| value.checked_add(self.settle.max(self.lapse)))
            .ok_or(CodecError::ArithmeticOverflow)
    }
}

/// Prepaid WorkBudget for finite staged selection and terminal work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectWorkBudgetV1Account {
    /// Canonical Epoch identity.
    pub epoch: Hash32,
    /// Exact DirectBatchPolicy V3 artifact identity.
    pub policy: Hash32,
    /// Compile-time verifier release identity.
    pub verifier_release_id: Hash32,
    /// Sponsor receiving unused reward lamports.
    pub reward_sponsor: Hash32,
    /// Account rent funding and donation ownership.
    pub funding: DirectFundingLedgerV3,
    /// Remaining spendable reward lamports.
    pub reward_balance: u64,
    /// Initial spendable reward lamports.
    pub initial_reward_balance: u64,
    /// Rewards paid so far.
    pub rewards_paid: u64,
    /// Frozen per-action rewards.
    pub rewards: DirectKeeperRewardsV3,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Active phase.
    pub phase: u8,
    /// Reserved flags; zero.
    pub flags: u16,
    /// Canonical zero reserve.
    pub reserved: [u8; 2],
}

impl DirectWorkBudgetV1Account {
    /// Validate exact ownership and finite reward solvency.
    pub fn validate(&self, neutral_sink: Hash32) -> Result<()> {
        for value in [
            self.epoch,
            self.policy,
            self.verifier_release_id,
            self.reward_sponsor,
        ] {
            nonzero(value)?;
        }
        self.funding.validate_for_sink(neutral_sink)?;
        let accounted = self
            .reward_balance
            .checked_add(self.rewards_paid)
            .ok_or(CodecError::ArithmeticOverflow)?;
        if self.reward_sponsor != self.funding.payer
            || accounted != self.initial_reward_balance
            || self.initial_reward_balance < self.rewards.worst_case()?
            || self.phase != DIRECT_WORK_BUDGET_PHASE_ACTIVE
            || self.flags != 0
            || self.reserved != [0; 2]
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly [`DIRECT_WORK_BUDGET_BYTES`] bytes.
    pub fn encode(&self, neutral_sink: Hash32, out: &mut [u8]) -> Result<usize> {
        self.validate(neutral_sink)?;
        if out.len() < DIRECT_WORK_BUDGET_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            DIRECT_WORK_BUDGET_TAG,
            DIRECT_WORK_BUDGET_VERSION,
        )?;
        writer.hash(self.epoch)?;
        writer.hash(self.policy)?;
        writer.hash(self.verifier_release_id)?;
        writer.hash(self.reward_sponsor)?;
        write_ledger(&mut writer, self.funding)?;
        writer.u64(self.reward_balance)?;
        writer.u64(self.initial_reward_balance)?;
        writer.u64(self.rewards_paid)?;
        writer.u64(self.rewards.begin_verification)?;
        writer.u64(self.rewards.verify_candidate)?;
        writer.u64(self.rewards.finalize_selection)?;
        writer.u64(self.rewards.settle)?;
        writer.u64(self.rewards.lapse)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.phase)?;
        writer.u16(self.flags)?;
        writer.bytes(&self.reserved)?;
        Ok(writer.at)
    }

    /// Decode exactly WorkBudget tag/version 1 and validate against the sink.
    pub fn decode(input: &[u8], neutral_sink: Hash32) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            DIRECT_WORK_BUDGET_TAG,
            DIRECT_WORK_BUDGET_VERSION,
            DIRECT_WORK_BUDGET_BYTES,
        )?;
        let value = Self {
            epoch: reader.hash()?,
            policy: reader.hash()?,
            verifier_release_id: reader.hash()?,
            reward_sponsor: reader.hash()?,
            funding: read_ledger(&mut reader)?,
            reward_balance: reader.u64()?,
            initial_reward_balance: reader.u64()?,
            rewards_paid: reader.u64()?,
            rewards: DirectKeeperRewardsV3 {
                begin_verification: reader.u64()?,
                verify_candidate: reader.u64()?,
                finalize_selection: reader.u64()?,
                settle: reader.u64()?,
                lapse: reader.u64()?,
            },
            stored_bump: reader.u8()?,
            phase: reader.u8()?,
            flags: reader.u16()?,
            reserved: reader.bytes()?,
        };
        reader.done()?;
        value.validate(neutral_sink)?;
        Ok(value)
    }
}

/// Exact Reservation V2: byte-preserved V1 semantics plus funding ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationV2Account {
    /// Exact existing Reservation fields and typed ownership phase.
    pub reservation: ReservationAccount,
    /// Exact transient-account funding ownership.
    pub funding: DirectFundingLedgerV3,
}

impl DirectReservationV2Account {
    /// Validate reservation semantics and funding ownership.
    pub fn validate(&self, neutral_sink: Hash32) -> Result<()> {
        self.reservation.validate()?;
        self.funding.validate_for_sink(neutral_sink)
    }

    /// Encode exactly 618 bytes under Reservation tag/version 2.
    pub fn encode(&self, neutral_sink: Hash32, out: &mut [u8]) -> Result<usize> {
        self.validate(neutral_sink)?;
        if out.len() < DIRECT_RESERVATION_V2_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            RESERVATION_ACCOUNT_TAG,
            DIRECT_RESERVATION_V2_VERSION,
        )?;
        write_reservation_body(&mut writer, &self.reservation)?;
        write_ledger(&mut writer, self.funding)?;
        Ok(writer.at)
    }

    /// Decode exactly Reservation tag/version 2 and validate against the sink.
    pub fn decode(input: &[u8], neutral_sink: Hash32) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            RESERVATION_ACCOUNT_TAG,
            DIRECT_RESERVATION_V2_VERSION,
            DIRECT_RESERVATION_V2_BYTES,
        )?;
        let value = Self {
            reservation: read_reservation_body(&mut reader)?,
            funding: read_ledger(&mut reader)?,
        };
        reader.done()?;
        value.validate(neutral_sink)?;
        Ok(value)
    }
}

fn write_reservation_body(writer: &mut Writer<'_>, value: &ReservationAccount) -> Result<()> {
    for item in [
        value.reservation,
        value.market,
        value.epoch,
        value.owner,
        value.order_id,
        value.price_grid,
        value.terms,
        value.policy,
    ] {
        writer.hash(item)?;
    }
    writer.u64(value.position_generation)?;
    writer.u64(value.order_generation)?;
    writer.u64(value.initial_cash_atoms)?;
    writer.u64(value.remaining_cash_atoms)?;
    writer.u64(value.max_fee_atoms)?;
    writer.u64(value.release_generation)?;
    writer.u16(value.page_index)?;
    writer.u8(value.outcome_count)?;
    writer.u8(value.order_kind)?;
    writer.u8(value.side)?;
    writer.u8(value.state)?;
    writer.u8(value.stored_bump)?;
    writer.u8(value.flags)?;
    writer.amounts(&value.initial_internal)?;
    writer.amounts(&value.remaining_internal)?;
    /* The direct plane's body is the frozen 570-byte v1 shape.  It carries no
     * partial-fill ledger — a direct order fills whole or not at all — so a
     * stamped or consumed ledger cannot be persisted through this writer, and
     * a caller holding one is refused rather than silently truncated. */
    if value.entitled_units != 0
        || value.consumed_units != 0
        || value.fee_debited_atoms != 0
        || value.fee_carry_numerator != 0
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

fn read_reservation_body(reader: &mut Reader<'_>) -> Result<ReservationAccount> {
    Ok(ReservationAccount {
        reservation: reader.hash()?,
        market: reader.hash()?,
        epoch: reader.hash()?,
        owner: reader.hash()?,
        order_id: reader.hash()?,
        price_grid: reader.hash()?,
        terms: reader.hash()?,
        policy: reader.hash()?,
        position_generation: reader.u64()?,
        order_generation: reader.u64()?,
        initial_cash_atoms: reader.u64()?,
        remaining_cash_atoms: reader.u64()?,
        max_fee_atoms: reader.u64()?,
        release_generation: reader.u64()?,
        page_index: reader.u16()?,
        outcome_count: reader.u8()?,
        order_kind: reader.u8()?,
        side: reader.u8()?,
        state: reader.u8()?,
        stored_bump: reader.u8()?,
        flags: reader.u8()?,
        initial_internal: reader.amounts()?,
        remaining_internal: reader.amounts()?,
        // Unstamped: the frozen direct body has no ledger words to read.
        entitled_units: 0,
        consumed_units: 0,
        fee_debited_atoms: 0,
        fee_carry_numerator: 0,
    })
}

/// Exact 96-byte policy plus compile-time verifier release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBatchPolicyV3 {
    /// Exact canonical 64-byte FrozenPolicy V1 preimage.
    pub policy_bytes: [u8; BATCH_POLICY_BYTES],
    /// Release identifier owned by the exact verifier implementation.
    pub verifier_release_id: Hash32,
}

impl DirectBatchPolicyV3 {
    /// Validate the registered policy and nonzero release identity.
    pub fn validate(&self) -> Result<()> {
        decode_batch_policy(&self.policy_bytes).map_err(|_| CodecError::MismatchedBinding)?;
        nonzero(self.verifier_release_id)
    }

    /// Construct from the canonical direct policy and a release identity.
    pub fn direct(verifier_release_id: Hash32) -> Result<Self> {
        let policy_bytes = canonical_batch_policy_bytes(
            &clutch_batch_policy_identity::direct_window_v1::DIRECT_POLICY_V1,
        )
        .map_err(|_| CodecError::MismatchedBinding)?;
        let value = Self {
            policy_bytes,
            verifier_release_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode exactly [`DIRECT_BATCH_POLICY_V3_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < DIRECT_BATCH_POLICY_V3_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        out[..BATCH_POLICY_BYTES].copy_from_slice(&self.policy_bytes);
        out[BATCH_POLICY_BYTES..DIRECT_BATCH_POLICY_V3_BYTES]
            .copy_from_slice(&self.verifier_release_id.0);
        Ok(DIRECT_BATCH_POLICY_V3_BYTES)
    }

    /// Decode an exact 96-byte DirectBatchPolicy V3 body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < DIRECT_BATCH_POLICY_V3_BYTES {
            return Err(CodecError::Truncated);
        }
        if input.len() > DIRECT_BATCH_POLICY_V3_BYTES {
            return Err(CodecError::TrailingBytes);
        }
        let mut policy_bytes = [0u8; BATCH_POLICY_BYTES];
        policy_bytes.copy_from_slice(&input[..BATCH_POLICY_BYTES]);
        let mut release = [0u8; 32];
        release.copy_from_slice(&input[BATCH_POLICY_BYTES..]);
        let value = Self {
            policy_bytes,
            verifier_release_id: Hash32::from_bytes(release),
        };
        value.validate()?;
        Ok(value)
    }

    /// Digest all 96 bytes under the exact canonical Epoch context.
    pub fn digest_for_epoch(&self, epoch: Hash32) -> Result<Hash32> {
        self.validate()?;
        nonzero(epoch)?;
        let mut bytes = [0u8; DIRECT_BATCH_POLICY_V3_BYTES];
        self.encode(&mut bytes)?;
        Ok(digest(
            DIRECT_BATCH_POLICY_V3_DIGEST_DOMAIN,
            &[&epoch.0, &bytes],
        ))
    }
}

/// Compile-time tripwire proving the candidate status byte remains byte 425.
pub const fn direct_candidate_status_offset() -> usize {
    CANDIDATE_STATUS_ACCOUNT_OFFSET
}

/// Project the semantic model stage into the sole persisted Candidate status
/// byte. This explicit match prevents Rust enum discriminants from becoming an
/// accidental wire format.
pub const fn direct_candidate_stage_wire(stage: ModelCandidateStageV3) -> u8 {
    match stage {
        ModelCandidateStageV3::Verified => DIRECT_CANDIDATE_STATUS_VERIFIED,
        ModelCandidateStageV3::Reverified => DIRECT_CANDIDATE_STATUS_REVERIFIED,
        ModelCandidateStageV3::Selected => DIRECT_CANDIDATE_STATUS_SELECTED,
    }
}

/// Byte-exact Candidate account projection from the executable lifecycle
/// lease. The model deliberately keeps the relation-issued body VERIFIED and
/// owns later status in `stage`; this is the sole join that writes the stage
/// into the persisted status byte.
pub fn project_model_candidate(lease: ModelCandidateLeaseV3) -> Result<DirectCandidateV3Account> {
    if lease.candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED {
        return Err(CodecError::MismatchedBinding);
    }
    lease.candidate.validate().map_err(map_direct_error)?;
    let mut candidate = lease.candidate;
    candidate.status = direct_candidate_stage_wire(lease.stage);
    let funding = DirectFundingLedgerV3 {
        payer: hash(lease.account.rent.payer),
        payer_principal_lamports: lease.account.rent.lamports,
        prior_donation_lamports: lease
            .account
            .donation_lamports()
            .map_err(|_| CodecError::MismatchedBinding)?,
    };
    Ok(DirectCandidateV3Account { candidate, funding })
}

/// Compile-time tripwire for the first Candidate V3 extension byte.
pub const fn direct_candidate_extension_offset() -> usize {
    CANDIDATE_EXTENSION_OFFSET
}

/// Compile-time tripwire for the first Window V3 extension byte.
pub const fn direct_window_extension_offset() -> usize {
    WINDOW_EXTENSION_OFFSET
}

/// Complete version-three direct lifecycle instruction family.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectV3Intent {
    /// Create an unfrozen Direct Epoch V4 with immutable deadlines and sink.
    InitEpoch {
        /// Market identity.
        market: Hash32,
        /// Epoch index within the Market.
        epoch_index: u64,
        /// DirectBatchPolicy V3 artifact digest.
        policy: Hash32,
        /// First accepted Candidate submission slot.
        submission_opens_slot: u64,
        /// Exclusive Candidate submission close.
        submission_closes_slot: u64,
        /// Exclusive staged-selection deadline.
        selection_deadline_slot: u64,
        /// Exclusive selected-settlement deadline.
        settlement_deadline_slot: u64,
        /// Immutable destination for unsolicited lamports.
        neutral_lamport_sink: Hash32,
    },
    /// Freeze exact two-order authority and capitalize finite work.
    FreezeEpoch {
        /// Market identity.
        market: Hash32,
        /// Epoch identity.
        epoch: Hash32,
        /// Reward-only lamports deposited by the authenticated sponsor.
        reward_deposit: u64,
        /// Frozen per-action rewards.
        rewards: DirectKeeperRewardsV3,
    },
    /// Abort an unfrozen Direct Epoch V4.
    AbortUnfrozen { market: Hash32, epoch: Hash32 },
    /// Verify and competitively admit one price tick.
    SubmitCandidate {
        market: Hash32,
        epoch: Hash32,
        /// Price of the candidate's selected outcome.
        outcome_price: u64,
    },
    /// Close submissions and begin staged verification.
    BeginVerification { market: Hash32, epoch: Hash32 },
    /// Reexecute one retained candidate by canonical top index.
    VerifyCandidate {
        market: Hash32,
        epoch: Hash32,
        /// Retained top index in `0..3`.
        retained_index: u8,
    },
    /// Select the exact fully reverified top candidate.
    FinalizeSelection { market: Hash32, epoch: Hash32 },
    /// Consume the selected exact entitlement.
    Settle { market: Hash32, epoch: Hash32 },
    /// Lapse a frozen epoch with no competitive Candidate.
    LapseEmpty { market: Hash32, epoch: Hash32 },
    /// Lapse an open or partially verified nonempty Window.
    LapseUnselected { market: Hash32, epoch: Hash32 },
    /// Lapse selected authority after the settlement deadline.
    LapseSelected { market: Hash32, epoch: Hash32 },
}

impl DirectV3Intent {
    /// Exact stable wire tag.
    pub const fn tag(self) -> u8 {
        match self {
            Self::InitEpoch { .. } => INIT_DIRECT_EPOCH_V4_TAG,
            Self::FreezeEpoch { .. } => FREEZE_DIRECT_EPOCH_V4_TAG,
            Self::AbortUnfrozen { .. } => ABORT_UNFROZEN_DIRECT_V4_TAG,
            Self::SubmitCandidate { .. } => SUBMIT_DIRECT_CANDIDATE_V3_TAG,
            Self::BeginVerification { .. } => BEGIN_DIRECT_VERIFICATION_V3_TAG,
            Self::VerifyCandidate { .. } => VERIFY_DIRECT_CANDIDATE_V3_TAG,
            Self::FinalizeSelection { .. } => FINALIZE_DIRECT_SELECTION_V3_TAG,
            Self::Settle { .. } => SETTLE_DIRECT_V3_TAG,
            Self::LapseEmpty { .. } => LAPSE_EMPTY_DIRECT_V3_TAG,
            Self::LapseUnselected { .. } => LAPSE_UNSELECTED_DIRECT_V3_TAG,
            Self::LapseSelected { .. } => LAPSE_SELECTED_DIRECT_V3_TAG,
        }
    }

    /// Exact stable wire length.
    pub const fn encoded_len(self) -> usize {
        match self {
            Self::InitEpoch { .. } => INIT_DIRECT_EPOCH_V4_BYTES,
            Self::FreezeEpoch { .. } => FREEZE_DIRECT_EPOCH_V4_BYTES,
            Self::SubmitCandidate { .. } => SUBMIT_DIRECT_CANDIDATE_V3_BYTES,
            Self::VerifyCandidate { .. } => VERIFY_DIRECT_CANDIDATE_V3_BYTES,
            Self::AbortUnfrozen { .. }
            | Self::BeginVerification { .. }
            | Self::FinalizeSelection { .. }
            | Self::Settle { .. }
            | Self::LapseEmpty { .. }
            | Self::LapseUnselected { .. }
            | Self::LapseSelected { .. } => DIRECT_V3_COMMON_ACTION_BYTES,
        }
    }

    /// Validate and encode without touching `out` on semantic refusal.
    pub fn encode(self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        let exact = self.encoded_len();
        if out.len() < exact {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(&mut writer, self.tag(), super::INTENT_VERSION)?;
        match self {
            Self::InitEpoch {
                market,
                epoch_index,
                policy,
                submission_opens_slot,
                submission_closes_slot,
                selection_deadline_slot,
                settlement_deadline_slot,
                neutral_lamport_sink,
            } => {
                writer.hash(market)?;
                writer.u64(epoch_index)?;
                writer.hash(policy)?;
                writer.u64(submission_opens_slot)?;
                writer.u64(submission_closes_slot)?;
                writer.u64(selection_deadline_slot)?;
                writer.u64(settlement_deadline_slot)?;
                writer.hash(neutral_lamport_sink)?;
            }
            Self::FreezeEpoch {
                market,
                epoch,
                reward_deposit,
                rewards,
            } => {
                writer.hash(market)?;
                writer.hash(epoch)?;
                writer.u64(reward_deposit)?;
                writer.u64(rewards.begin_verification)?;
                writer.u64(rewards.verify_candidate)?;
                writer.u64(rewards.finalize_selection)?;
                writer.u64(rewards.settle)?;
                writer.u64(rewards.lapse)?;
            }
            Self::SubmitCandidate {
                market,
                epoch,
                outcome_price,
            } => {
                writer.hash(market)?;
                writer.hash(epoch)?;
                writer.u64(outcome_price)?;
            }
            Self::VerifyCandidate {
                market,
                epoch,
                retained_index,
            } => {
                writer.hash(market)?;
                writer.hash(epoch)?;
                writer.u8(retained_index)?;
            }
            Self::AbortUnfrozen { market, epoch }
            | Self::BeginVerification { market, epoch }
            | Self::FinalizeSelection { market, epoch }
            | Self::Settle { market, epoch }
            | Self::LapseEmpty { market, epoch }
            | Self::LapseUnselected { market, epoch }
            | Self::LapseSelected { market, epoch } => {
                writer.hash(market)?;
                writer.hash(epoch)?;
            }
        }
        if writer.at != exact {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(writer.at)
    }

    fn validate(self) -> Result<()> {
        match self {
            Self::InitEpoch {
                market,
                policy,
                submission_opens_slot,
                submission_closes_slot,
                selection_deadline_slot,
                settlement_deadline_slot,
                neutral_lamport_sink,
                ..
            } => {
                nonzero(market)?;
                nonzero(policy)?;
                nonzero(neutral_lamport_sink)?;
                checked_span(
                    submission_opens_slot,
                    submission_closes_slot,
                    MIN_SUBMISSION_SPAN_V3,
                    MAX_SUBMISSION_SPAN_V3,
                )?;
                checked_span(
                    submission_closes_slot,
                    selection_deadline_slot,
                    MIN_SELECTION_SPAN_V3,
                    MAX_SELECTION_SPAN_V3,
                )?;
                checked_span(
                    selection_deadline_slot,
                    settlement_deadline_slot,
                    MIN_SETTLEMENT_SPAN_V3,
                    MAX_SETTLEMENT_SPAN_V3,
                )
            }
            Self::FreezeEpoch {
                market,
                epoch,
                reward_deposit,
                rewards,
            } => {
                nonzero(market)?;
                nonzero(epoch)?;
                if reward_deposit < rewards.worst_case()? {
                    return Err(CodecError::ZeroValue);
                }
                Ok(())
            }
            Self::SubmitCandidate {
                market,
                epoch,
                outcome_price,
            } => {
                nonzero(market)?;
                nonzero(epoch)?;
                if outcome_price == 0 {
                    return Err(CodecError::ZeroValue);
                }
                Ok(())
            }
            Self::VerifyCandidate {
                market,
                epoch,
                retained_index,
            } => {
                nonzero(market)?;
                nonzero(epoch)?;
                if usize::from(retained_index) >= MAX_DIRECT_CANDIDATES {
                    return Err(CodecError::InvalidCount);
                }
                Ok(())
            }
            Self::AbortUnfrozen { market, epoch }
            | Self::BeginVerification { market, epoch }
            | Self::FinalizeSelection { market, epoch }
            | Self::Settle { market, epoch }
            | Self::LapseEmpty { market, epoch }
            | Self::LapseUnselected { market, epoch }
            | Self::LapseSelected { market, epoch } => {
                nonzero(market)?;
                nonzero(epoch)
            }
        }
    }

    /// Decode one exact V3 lifecycle wire. Other tags fail closed.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < 2 {
            return Err(CodecError::Truncated);
        }
        let tag = input[0];
        let exact = match tag {
            INIT_DIRECT_EPOCH_V4_TAG => INIT_DIRECT_EPOCH_V4_BYTES,
            FREEZE_DIRECT_EPOCH_V4_TAG => FREEZE_DIRECT_EPOCH_V4_BYTES,
            SUBMIT_DIRECT_CANDIDATE_V3_TAG => SUBMIT_DIRECT_CANDIDATE_V3_BYTES,
            VERIFY_DIRECT_CANDIDATE_V3_TAG => VERIFY_DIRECT_CANDIDATE_V3_BYTES,
            ABORT_UNFROZEN_DIRECT_V4_TAG
            | BEGIN_DIRECT_VERIFICATION_V3_TAG
            | FINALIZE_DIRECT_SELECTION_V3_TAG
            | SETTLE_DIRECT_V3_TAG
            | LAPSE_EMPTY_DIRECT_V3_TAG
            | LAPSE_UNSELECTED_DIRECT_V3_TAG
            | LAPSE_SELECTED_DIRECT_V3_TAG => DIRECT_V3_COMMON_ACTION_BYTES,
            _ => return Err(CodecError::WrongTag),
        };
        let mut reader = Reader::new(input, tag, super::INTENT_VERSION, exact)?;
        let value = match tag {
            INIT_DIRECT_EPOCH_V4_TAG => Self::InitEpoch {
                market: reader.hash()?,
                epoch_index: reader.u64()?,
                policy: reader.hash()?,
                submission_opens_slot: reader.u64()?,
                submission_closes_slot: reader.u64()?,
                selection_deadline_slot: reader.u64()?,
                settlement_deadline_slot: reader.u64()?,
                neutral_lamport_sink: reader.hash()?,
            },
            FREEZE_DIRECT_EPOCH_V4_TAG => Self::FreezeEpoch {
                market: reader.hash()?,
                epoch: reader.hash()?,
                reward_deposit: reader.u64()?,
                rewards: DirectKeeperRewardsV3 {
                    begin_verification: reader.u64()?,
                    verify_candidate: reader.u64()?,
                    finalize_selection: reader.u64()?,
                    settle: reader.u64()?,
                    lapse: reader.u64()?,
                },
            },
            ABORT_UNFROZEN_DIRECT_V4_TAG => Self::AbortUnfrozen {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            SUBMIT_DIRECT_CANDIDATE_V3_TAG => Self::SubmitCandidate {
                market: reader.hash()?,
                epoch: reader.hash()?,
                outcome_price: reader.u64()?,
            },
            BEGIN_DIRECT_VERIFICATION_V3_TAG => Self::BeginVerification {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            VERIFY_DIRECT_CANDIDATE_V3_TAG => Self::VerifyCandidate {
                market: reader.hash()?,
                epoch: reader.hash()?,
                retained_index: reader.u8()?,
            },
            FINALIZE_DIRECT_SELECTION_V3_TAG => Self::FinalizeSelection {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            SETTLE_DIRECT_V3_TAG => Self::Settle {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            LAPSE_EMPTY_DIRECT_V3_TAG => Self::LapseEmpty {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            LAPSE_UNSELECTED_DIRECT_V3_TAG => Self::LapseUnselected {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            LAPSE_SELECTED_DIRECT_V3_TAG => Self::LapseSelected {
                market: reader.hash()?,
                epoch: reader.hash()?,
            },
            _ => return Err(CodecError::WrongTag),
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canonical_order_id,
        direct_selection::{canonical_direct_book_id, canonical_direct_remainder_seed},
        reservation::{
            ReservationPlan, RESERVATION_ACCOUNT_VERSION, RESERVATION_STATE_ACTIVE,
            RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED, RESERVATION_STATE_RELEASED,
        },
        EpochAccount, OrderRecord, OrderSlot, RELATION_VERSION,
    };
    use clutch_batch_policy_identity::direct_lifecycle_v3::{
        direct_policy_v3_digest, DirectAccountLedgerV3 as ModelAccountLedgerV3,
        DirectCreationFundingV3 as ModelCreationFundingV3, DirectGridV3 as ModelGridV3,
        DirectKeeperRewardsV3 as ModelKeeperRewardsV3,
        DirectLifecycleAuthorityV3 as ModelLifecycleAuthorityV3,
        DirectObservedBalancesV3 as ModelObservedBalancesV3, DirectPositionV3 as ModelPositionV3,
        DirectPrefreezeOrderV3 as ModelPrefreezeOrderV3, DirectPrefreezeV3 as ModelPrefreezeV3,
        DirectRentPrincipalV3 as ModelRentPrincipalV3,
        DirectReservationDomainV3 as ModelReservationDomainV3,
        DirectReservationPlacementV3 as ModelReservationPlacementV3,
        DirectScheduleV3 as ModelScheduleV3, DirectTransitionContextV3 as ModelTransitionContextV3,
        DirectWorkBudgetFundingV3 as ModelWorkBudgetFundingV3,
    };
    use clutch_batch_policy_identity::direct_window_v1::{
        canonical_account_candidate_id, DirectCandidateEntryV1, DIRECT_POLICY_V1,
    };
    use clutch_batch_policy_identity::{batch_policy_digest, Identity32V1};
    extern crate std;

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn ledger(byte: u8) -> DirectFundingLedgerV3 {
        DirectFundingLedgerV3 {
            payer: h(byte),
            payer_principal_lamports: 1_000 + u64::from(byte),
            prior_donation_lamports: u64::from(byte),
        }
    }

    fn epoch(phase: u8, lifecycle_phase: u8) -> DirectEpochV4Account {
        let market = h(1);
        let epoch_id = crate::canonical_epoch_id(market, 7);
        let verifier_release_id = h(80);
        let direct_policy = DirectBatchPolicyV3::direct(verifier_release_id).unwrap();
        let common = EpochAccount {
            epoch: epoch_id,
            market,
            book: canonical_direct_book_id(epoch_id),
            terms: h(2),
            price_grid: h(3),
            policy: Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0),
            order_set: if phase == EPOCH_PHASE_OPEN {
                Hash32::ZERO
            } else {
                h(5)
            },
            first_order_id: if phase == EPOCH_PHASE_OPEN {
                Hash32::ZERO
            } else {
                canonical_order_id(1)
            },
            last_order_id: if phase == EPOCH_PHASE_OPEN {
                Hash32::ZERO
            } else {
                canonical_order_id(2)
            },
            epoch_index: 7,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: canonical_direct_remainder_seed(epoch_id),
            owner_count: 2,
            page_count: if phase == EPOCH_PHASE_OPEN { 0 } else { 1 },
            order_count: if phase == EPOCH_PHASE_OPEN { 0 } else { 2 },
            outcome_count: 2,
            basis_degree: 1,
            phase,
            stored_bump: 9,
            flags: 0,
        };
        DirectEpochV4Account {
            direct: DirectEpochV3Account {
                common,
                submission_opens_slot: 100,
                submission_closes_slot: 110,
            },
            selection_deadline_slot: 120,
            settlement_deadline_slot: 140,
            lifecycle_phase,
            terminal: DirectTerminalReceiptV3::EMPTY,
            neutral_lamport_sink: h(90),
            verifier_release_id,
            direct_policy_v3_id: direct_policy.digest_for_epoch(epoch_id).unwrap(),
            epoch_funding: ledger(24),
            page_funding: if common.page_count == 0 {
                DirectFundingLedgerV3::ZERO
            } else {
                ledger(25)
            },
            reserved: [0; 4],
        }
    }

    fn candidate(status: u8) -> DirectCandidateV3Account {
        let epoch = h(10);
        let market = h(11);
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 2_500;
        prices[1] = 7_500;
        DirectCandidateV3Account {
            candidate: DirectCandidateV2 {
                candidate_id: canonical_account_candidate_id(
                    identity(epoch),
                    identity(market),
                    &prices,
                ),
                epoch_id: identity(epoch),
                market_id: identity(market),
                order_set_id: Identity32V1([12; 32]),
                policy_id: Identity32V1([13; 32]),
                relation_domain_digest: Identity32V1([14; 32]),
                relation_candidate_digest: Identity32V1([15; 32]),
                prices,
                fills: [4, 4],
                weighted_direct_volume: 75_000_000,
                limit_surplus_price_units: 20_000,
                submitted_slot: 105,
                quantity: 4,
                buy_index: 0,
                sell_index: 1,
                outcome: 0,
                distinct_owners: 2,
                order_len: 2,
                outcome_count: 2,
                status,
                stored_bump: 8,
                flags: 0,
                reserved: [0; 12],
            },
            funding: ledger(20),
        }
    }

    fn window(phase: u8) -> DirectWindowV3Account {
        let c = candidate(if phase == DIRECT_WINDOW_PHASE_SELECTED {
            DIRECT_CANDIDATE_STATUS_SELECTED
        } else {
            DIRECT_CANDIDATE_STATUS_VERIFIED
        });
        let entry = c.candidate.entry();
        DirectWindowV3Account {
            window: DirectCandidateWindowV1 {
                epoch_id: c.candidate.epoch_id,
                market_id: c.candidate.market_id,
                order_set_id: c.candidate.order_set_id,
                policy_id: c.candidate.policy_id,
                relation_domain_digest: c.candidate.relation_domain_digest,
                admission_transcript: Identity32V1([30; 32]),
                selected_candidate: if phase == DIRECT_WINDOW_PHASE_SELECTED {
                    entry.candidate_id
                } else {
                    Identity32V1::ZERO
                },
                top: [
                    entry,
                    DirectCandidateEntryV1::ZERO,
                    DirectCandidateEntryV1::ZERO,
                ],
                opens_slot: 100,
                closes_slot: 110,
                selected_slot: if phase == DIRECT_WINDOW_PHASE_SELECTED {
                    115
                } else {
                    0
                },
                admitted_count: 1,
                top_count: 1,
                phase,
                stored_bump: 7,
                flags: 0,
                reserved: [0; 2],
            },
            funding: ledger(21),
            seen_competitive_ticks: 1 << 3,
            verification_mask: if phase == DIRECT_WINDOW_PHASE_SELECTED {
                1
            } else {
                0
            },
            live_candidate_mask: 1,
            extension_flags: 0,
            selection_deadline_slot: 120,
            settlement_deadline_slot: 140,
            receipt_funding: if phase == DIRECT_WINDOW_PHASE_SELECTED {
                ledger(22)
            } else {
                DirectFundingLedgerV3::ZERO
            },
            pot_funding: if phase == DIRECT_WINDOW_PHASE_SELECTED {
                ledger(23)
            } else {
                DirectFundingLedgerV3::ZERO
            },
            reserved: [0; 4],
        }
    }

    fn reservation() -> DirectReservationV2Account {
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = 4;
        let plan = ReservationPlan {
            cash_atoms: 0,
            internal,
            max_fee_atoms: 0,
            outcome_count: 2,
            order_kind: crate::ORDER_KIND_SINGLE,
            side: 1,
        };
        DirectReservationV2Account {
            reservation: ReservationAccount::active(
                h(1),
                h(2),
                h(3),
                canonical_order_id(1),
                h(4),
                h(5),
                h(6),
                7,
                8,
                0,
                9,
                plan,
            )
            .unwrap(),
            funding: ledger(24),
        }
    }

    #[test]
    fn exact_widths_derive_from_live_constants_and_tags_do_not_collide() {
        assert_eq!(DIRECT_EPOCH_V4_BYTES, 673);
        assert_eq!(DIRECT_CANDIDATE_V3_BYTES, 488);
        assert_eq!(DIRECT_WINDOW_V3_BYTES, 632);
        assert_eq!(DIRECT_WORK_BUDGET_BYTES, 248);
        // The general reservation grew to 610 with the partial-fill ledger;
        // the direct plane's own body stays byte-frozen at 618 and is told
        // apart from it by both version and length.
        assert_eq!(RESERVATION_ACCOUNT_BYTES, 610);
        assert_eq!(DIRECT_RESERVATION_V2_BYTES, 618);
        assert_ne!(DIRECT_RESERVATION_V2_BYTES, RESERVATION_ACCOUNT_BYTES);
        assert_ne!(DIRECT_RESERVATION_V2_VERSION, RESERVATION_ACCOUNT_VERSION);
        assert_eq!(DIRECT_BATCH_POLICY_V3_BYTES, 96);
        assert_eq!(DIRECT_WORK_BUDGET_TAG, 23);
        assert_ne!(DIRECT_WORK_BUDGET_TAG, DIRECT_CANDIDATE_TAG);
        assert_ne!(DIRECT_WORK_BUDGET_TAG, DIRECT_WINDOW_TAG);
        assert_eq!(direct_candidate_status_offset(), 425);
        assert_eq!(direct_candidate_extension_offset(), 440);
        assert_eq!(direct_window_extension_offset(), 456);
    }

    #[test]
    fn epoch_v4_roundtrips_and_versions_refuse_each_other() {
        for value in [
            epoch(EPOCH_PHASE_OPEN, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN),
            epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY),
        ] {
            let mut bytes = [0u8; DIRECT_EPOCH_V4_BYTES];
            assert_eq!(value.encode(&mut bytes), Ok(DIRECT_EPOCH_V4_BYTES));
            assert_eq!(DirectEpochV4Account::decode(&bytes), Ok(value));
            assert_eq!(&bytes[509..541], &value.verifier_release_id.bytes());
            assert_eq!(&bytes[541..573], &value.direct_policy_v3_id.bytes());
            assert_eq!(&bytes[573..605], &value.epoch_funding.payer.bytes());
            assert_eq!(
                &bytes[605..613],
                &value.epoch_funding.payer_principal_lamports.to_le_bytes()
            );
            assert_eq!(
                &bytes[613..621],
                &value.epoch_funding.prior_donation_lamports.to_le_bytes()
            );
            assert_eq!(&bytes[621..653], &value.page_funding.payer.bytes());
            assert_eq!(
                &bytes[653..661],
                &value.page_funding.payer_principal_lamports.to_le_bytes()
            );
            assert_eq!(
                &bytes[661..669],
                &value.page_funding.prior_donation_lamports.to_le_bytes()
            );
            assert_eq!(&bytes[669..673], &[0; 4]);
            assert_eq!(
                DirectEpochV3Account::decode(&bytes),
                Err(CodecError::TrailingBytes)
            );
            let mut wrong = bytes;
            wrong[1] = super::super::direct_selection::DIRECT_EPOCH_VERSION;
            assert_eq!(
                DirectEpochV4Account::decode(&wrong),
                Err(CodecError::WrongVersion)
            );
        }
    }

    #[test]
    fn epoch_schedule_phase_and_terminal_receipt_fail_closed() {
        let mut value = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        value.selection_deadline_slot = value.direct.submission_closes_slot + 4;
        assert_eq!(value.validate(), Err(CodecError::InvalidCount));
        value.selection_deadline_slot = 120;
        value.direct.common.phase = EPOCH_PHASE_CLEARED;
        assert_eq!(value.validate(), Err(CodecError::MismatchedBinding));
        value.direct.common.phase = EPOCH_PHASE_FROZEN;
        value.reserved[0] = 1;
        assert_eq!(value.validate(), Err(CodecError::NonCanonicalPadding));

        let mut wrong_release = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        wrong_release.verifier_release_id = h(81);
        assert_eq!(wrong_release.validate(), Err(CodecError::MismatchedBinding));
        let mut wrong_policy = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        wrong_policy.direct_policy_v3_id = h(82);
        assert_eq!(wrong_policy.validate(), Err(CodecError::MismatchedBinding));
        let mut wrong_relation = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        wrong_relation.direct.common.policy = h(83);
        assert_eq!(
            wrong_relation.validate(),
            Err(CodecError::MismatchedBinding)
        );
        let mut wrong_funding = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        wrong_funding.epoch_funding.payer_principal_lamports = 0;
        assert_eq!(wrong_funding.validate(), Err(CodecError::MismatchedBinding));

        let mut coherent_alternate = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        coherent_alternate.verifier_release_id = h(81);
        coherent_alternate.direct_policy_v3_id = DirectBatchPolicyV3::direct(h(81))
            .unwrap()
            .digest_for_epoch(coherent_alternate.direct.common.epoch)
            .unwrap();
        assert_eq!(coherent_alternate.validate(), Ok(()));
        assert_eq!(
            coherent_alternate.validate_for_release(h(80)),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn v4_placement_requires_prefreeze_lifecycle_not_coarse_open() {
        let open = epoch(EPOCH_PHASE_OPEN, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN);
        assert_eq!(open.require_prefreeze_placement(), Ok(()));

        let mut aborted = epoch(EPOCH_PHASE_OPEN, DIRECT_LIFECYCLE_PHASE_TERMINAL);
        aborted.terminal.reason = DIRECT_TERMINAL_REASON_PREFREEZE_ABORT;
        aborted.terminal.terminal_slot = aborted.direct.submission_opens_slot;
        assert_eq!(aborted.validate(), Ok(()));
        assert_eq!(
            aborted.require_prefreeze_placement(),
            Err(CodecError::InvalidEnum)
        );

        let frozen = epoch(EPOCH_PHASE_FROZEN, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY);
        assert_eq!(
            frozen.require_prefreeze_placement(),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn every_terminal_reason_has_one_exact_coarse_phase_and_shape() {
        let mut prefreeze = epoch(EPOCH_PHASE_OPEN, DIRECT_LIFECYCLE_PHASE_TERMINAL);
        prefreeze.terminal.reason = DIRECT_TERMINAL_REASON_PREFREEZE_ABORT;
        prefreeze.terminal.terminal_slot = 100;
        assert_eq!(prefreeze.validate(), Ok(()));
        prefreeze.direct.common.phase = EPOCH_PHASE_LAPSED;
        assert!(prefreeze.validate().is_err());

        let mut empty = epoch(EPOCH_PHASE_LAPSED, DIRECT_LIFECYCLE_PHASE_TERMINAL);
        empty.terminal.reason = DIRECT_TERMINAL_REASON_EMPTY_LAPSE;
        empty.terminal.terminal_reservation_count = 2;
        empty.terminal.terminal_slot = 120;
        assert_eq!(empty.validate(), Ok(()));

        let mut pre = empty;
        pre.terminal.reason = DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE;
        assert_eq!(pre.validate(), Ok(()));

        let mut post = epoch(EPOCH_PHASE_LAPSED, DIRECT_LIFECYCLE_PHASE_TERMINAL);
        post.terminal = DirectTerminalReceiptV3 {
            reason: DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE,
            terminal_reservation_count: 2,
            selected_slot: 115,
            candidate: h(40),
            relation_candidate_digest: h(41),
            terminal_slot: 140,
            ..DirectTerminalReceiptV3::EMPTY
        };
        assert_eq!(post.validate(), Ok(()));

        let mut settled = epoch(EPOCH_PHASE_SETTLED, DIRECT_LIFECYCLE_PHASE_TERMINAL);
        settled.terminal = DirectTerminalReceiptV3 {
            reason: DIRECT_TERMINAL_REASON_SETTLED,
            outcome: 1,
            terminal_reservation_count: 2,
            selected_slot: 115,
            candidate: h(40),
            relation_candidate_digest: h(41),
            quantity: 4,
            price: 7_500,
            consideration_price_units: 30_000,
            terminal_slot: 130,
        };
        assert_eq!(settled.validate(), Ok(()));
        settled.terminal.consideration_price_units += 1;
        assert_eq!(settled.validate(), Err(CodecError::MismatchedBinding));
        settled.terminal.consideration_price_units = 30_000;
        settled.direct.common.phase = EPOCH_PHASE_LAPSED;
        assert_eq!(settled.validate(), Err(CodecError::MismatchedBinding));

        let mut unknown = empty;
        unknown.terminal.reason = DIRECT_TERMINAL_REASON_PREFREEZE_ABORT + 1;
        assert_eq!(unknown.validate(), Err(CodecError::InvalidEnum));
    }

    #[test]
    fn model_enums_and_terminal_receipts_project_without_discriminant_casts() {
        let phases = [
            (
                ModelLifecyclePhaseV3::FrozenEmpty,
                DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
            ),
            (
                ModelLifecyclePhaseV3::WindowOpen,
                DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
            ),
            (
                ModelLifecyclePhaseV3::Verifying,
                DIRECT_LIFECYCLE_PHASE_VERIFYING,
            ),
            (
                ModelLifecyclePhaseV3::Selected,
                DIRECT_LIFECYCLE_PHASE_SELECTED,
            ),
            (
                ModelLifecyclePhaseV3::Terminal,
                DIRECT_LIFECYCLE_PHASE_TERMINAL,
            ),
        ];
        for (model, wire) in phases {
            assert_eq!(direct_lifecycle_phase_wire(model), wire);
        }
        let reasons = [
            (
                ModelTerminalReasonV3::EmptyLapse,
                DIRECT_TERMINAL_REASON_EMPTY_LAPSE,
            ),
            (
                ModelTerminalReasonV3::PreSelectionLapse,
                DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE,
            ),
            (
                ModelTerminalReasonV3::PostSelectionLapse,
                DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE,
            ),
            (
                ModelTerminalReasonV3::Settled,
                DIRECT_TERMINAL_REASON_SETTLED,
            ),
            (
                ModelTerminalReasonV3::PrefreezeAbort,
                DIRECT_TERMINAL_REASON_PREFREEZE_ABORT,
            ),
        ];
        for (model, wire) in reasons {
            assert_eq!(direct_terminal_reason_wire(model), wire);
        }

        let model = ModelTerminalReceiptV3 {
            reason: ModelTerminalReasonV3::Settled,
            terminal_reservation_count: 2,
            selected_slot: 115,
            candidate_id: Identity32V1([40; 32]),
            relation_candidate_digest: Identity32V1([41; 32]),
            outcome: 1,
            quantity: 4,
            price: 7_500,
            consideration_price_units: 30_000,
            terminal_slot: 130,
        };
        assert_eq!(
            project_model_terminal_receipt(model),
            DirectTerminalReceiptV3 {
                reason: DIRECT_TERMINAL_REASON_SETTLED,
                outcome: 1,
                terminal_reservation_count: 2,
                selected_slot: 115,
                candidate: h(40),
                relation_candidate_digest: h(41),
                quantity: 4,
                price: 7_500,
                consideration_price_units: 30_000,
                terminal_slot: 130,
            }
        );

        let model_receipts = [
            (
                EPOCH_PHASE_OPEN,
                ModelTerminalReceiptV3 {
                    reason: ModelTerminalReasonV3::PrefreezeAbort,
                    terminal_reservation_count: 2,
                    selected_slot: 0,
                    candidate_id: Identity32V1([42; 32]),
                    relation_candidate_digest: Identity32V1([43; 32]),
                    outcome: 0,
                    quantity: 0,
                    price: 0,
                    consideration_price_units: 0,
                    terminal_slot: 100,
                },
            ),
            (
                EPOCH_PHASE_LAPSED,
                ModelTerminalReceiptV3 {
                    reason: ModelTerminalReasonV3::EmptyLapse,
                    terminal_reservation_count: 2,
                    selected_slot: 0,
                    candidate_id: Identity32V1::ZERO,
                    relation_candidate_digest: Identity32V1::ZERO,
                    outcome: 0,
                    quantity: 0,
                    price: 0,
                    consideration_price_units: 0,
                    terminal_slot: 120,
                },
            ),
            (
                EPOCH_PHASE_LAPSED,
                ModelTerminalReceiptV3 {
                    reason: ModelTerminalReasonV3::PreSelectionLapse,
                    terminal_reservation_count: 2,
                    selected_slot: 0,
                    candidate_id: Identity32V1::ZERO,
                    relation_candidate_digest: Identity32V1::ZERO,
                    outcome: 0,
                    quantity: 0,
                    price: 0,
                    consideration_price_units: 0,
                    terminal_slot: 120,
                },
            ),
            (
                EPOCH_PHASE_LAPSED,
                ModelTerminalReceiptV3 {
                    reason: ModelTerminalReasonV3::PostSelectionLapse,
                    terminal_reservation_count: 2,
                    selected_slot: 115,
                    candidate_id: Identity32V1([40; 32]),
                    relation_candidate_digest: Identity32V1([41; 32]),
                    outcome: 0,
                    quantity: 0,
                    price: 0,
                    consideration_price_units: 0,
                    terminal_slot: 140,
                },
            ),
            (EPOCH_PHASE_SETTLED, model),
        ];
        for (coarse_phase, model_receipt) in model_receipts {
            let mut archived = epoch(coarse_phase, DIRECT_LIFECYCLE_PHASE_TERMINAL);
            archived.terminal = project_model_terminal_receipt(model_receipt);
            assert_eq!(archived.validate(), Ok(()));
        }
    }

    #[test]
    fn model_candidate_stage_is_the_persisted_status_owner() {
        let source = candidate(DIRECT_CANDIDATE_STATUS_VERIFIED);
        let account = ModelAccountLedgerV3::restore(
            ModelRentPrincipalV3 {
                payer: identity(source.funding.payer),
                lamports: source.funding.payer_principal_lamports,
            },
            identity(h(90)),
            source.funding.prior_donation_lamports,
        )
        .unwrap();
        for (stage, status) in [
            (
                ModelCandidateStageV3::Verified,
                DIRECT_CANDIDATE_STATUS_VERIFIED,
            ),
            (
                ModelCandidateStageV3::Reverified,
                DIRECT_CANDIDATE_STATUS_REVERIFIED,
            ),
            (
                ModelCandidateStageV3::Selected,
                DIRECT_CANDIDATE_STATUS_SELECTED,
            ),
        ] {
            let projected = project_model_candidate(ModelCandidateLeaseV3 {
                candidate: source.candidate,
                tick: 3,
                stage,
                account,
            })
            .unwrap();
            assert_eq!(projected.candidate.status, status);
            let mut bytes = [0u8; DIRECT_CANDIDATE_V3_BYTES];
            projected.encode(h(90), &mut bytes).unwrap();
            assert_eq!(
                DirectCandidateV3Account::decode(&bytes, h(90)),
                Ok(projected)
            );
        }

        let mut hostile = source.candidate;
        hostile.status = DIRECT_CANDIDATE_STATUS_SELECTED;
        assert_eq!(
            project_model_candidate(ModelCandidateLeaseV3 {
                candidate: hostile,
                tick: 3,
                stage: ModelCandidateStageV3::Selected,
                account,
            }),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn candidate_v3_roundtrips_all_staged_statuses_and_refuses_old_version() {
        for status in [
            DIRECT_CANDIDATE_STATUS_VERIFIED,
            DIRECT_CANDIDATE_STATUS_REVERIFIED,
            DIRECT_CANDIDATE_STATUS_SELECTED,
        ] {
            let value = candidate(status);
            let mut bytes = [0u8; DIRECT_CANDIDATE_V3_BYTES];
            assert_eq!(
                value.encode(h(90), &mut bytes),
                Ok(DIRECT_CANDIDATE_V3_BYTES)
            );
            assert_eq!(bytes[CANDIDATE_STATUS_ACCOUNT_OFFSET], status);
            assert_eq!(DirectCandidateV3Account::decode(&bytes, h(90)), Ok(value));
            assert_eq!(
                super::super::direct_selection::decode_direct_candidate(&bytes),
                Err(CodecError::TrailingBytes)
            );
            let mut wrong = bytes;
            wrong[1] = super::super::direct_selection::DIRECT_CANDIDATE_VERSION;
            assert_eq!(
                DirectCandidateV3Account::decode(&wrong, h(90)),
                Err(CodecError::WrongVersion)
            );
        }
        let mut raw = 0u16;
        loop {
            let status = raw as u8;
            assert_eq!(
                candidate(status).validate(h(90)).is_ok(),
                matches!(
                    status,
                    DIRECT_CANDIDATE_STATUS_VERIFIED
                        | DIRECT_CANDIDATE_STATUS_REVERIFIED
                        | DIRECT_CANDIDATE_STATUS_SELECTED
                )
            );
            if raw == u16::from(u8::MAX) {
                break;
            }
            raw += 1;
        }
    }

    #[test]
    fn candidate_refusal_never_mutates_destination() {
        let mut value = candidate(DIRECT_CANDIDATE_STATUS_REVERIFIED);
        value.funding.payer = h(90);
        let mut bytes = [0xa5; DIRECT_CANDIDATE_V3_BYTES];
        assert_eq!(
            value.encode(h(90), &mut bytes),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(bytes, [0xa5; DIRECT_CANDIDATE_V3_BYTES]);
    }

    #[test]
    fn window_v3_roundtrips_open_verifying_and_selected() {
        for phase in [
            DIRECT_WINDOW_PHASE_OPEN,
            DIRECT_WINDOW_PHASE_VERIFYING,
            DIRECT_WINDOW_PHASE_SELECTED,
        ] {
            let mut value = window(phase);
            if phase == DIRECT_WINDOW_PHASE_VERIFYING {
                value.verification_mask = 1;
            }
            let mut bytes = [0u8; DIRECT_WINDOW_V3_BYTES];
            assert_eq!(value.encode(h(90), &mut bytes), Ok(DIRECT_WINDOW_V3_BYTES));
            assert_eq!(DirectWindowV3Account::decode(&bytes, h(90)), Ok(value));
            assert_eq!(
                super::super::direct_selection::decode_direct_window(&bytes),
                Err(CodecError::TrailingBytes)
            );
        }
    }

    #[test]
    fn window_masks_bitmap_digest_and_funding_are_canonical() {
        let mut value = window(DIRECT_WINDOW_PHASE_OPEN);
        value.seen_competitive_ticks = 0;
        assert_eq!(value.validate(h(90)), Err(CodecError::NonCanonicalPadding));
        value.seen_competitive_ticks = 1 << 3;
        value.verification_mask = 2;
        assert_eq!(value.validate(h(90)), Err(CodecError::InvalidCount));
        value.verification_mask = 0;
        value.receipt_funding = ledger(22);
        assert_eq!(value.validate(h(90)), Err(CodecError::MismatchedBinding));

        let mut duplicate = window(DIRECT_WINDOW_PHASE_OPEN);
        duplicate.window.top_count = 2;
        duplicate.window.top[1] = DirectCandidateEntryV1 {
            candidate_id: Identity32V1([70; 32]),
            relation_candidate_digest: duplicate.window.top[0].relation_candidate_digest,
        };
        duplicate.window.admitted_count = 2;
        duplicate.seen_competitive_ticks |= 1 << 4;
        duplicate.live_candidate_mask = 3;
        assert_eq!(
            duplicate.validate(h(90)),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn work_budget_roundtrips_and_never_spends_rent_or_donation() {
        let rewards = DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 2,
            finalize_selection: 3,
            settle: 4,
            lapse: 5,
        };
        let initial = rewards.worst_case().unwrap();
        let value = DirectWorkBudgetV1Account {
            epoch: h(1),
            policy: h(2),
            verifier_release_id: h(3),
            reward_sponsor: h(24),
            funding: ledger(24),
            reward_balance: initial - 1,
            initial_reward_balance: initial,
            rewards_paid: 1,
            rewards,
            stored_bump: 9,
            phase: DIRECT_WORK_BUDGET_PHASE_ACTIVE,
            flags: 0,
            reserved: [0; 2],
        };
        let mut bytes = [0u8; DIRECT_WORK_BUDGET_BYTES];
        assert_eq!(
            value.encode(h(90), &mut bytes),
            Ok(DIRECT_WORK_BUDGET_BYTES)
        );
        assert_eq!(DirectWorkBudgetV1Account::decode(&bytes, h(90)), Ok(value));
        let mut underfunded = value;
        underfunded.initial_reward_balance -= 1;
        underfunded.reward_balance -= 1;
        assert_eq!(
            underfunded.validate(h(90)),
            Err(CodecError::MismatchedBinding)
        );
        let mut index = 0usize;
        while index < 5 {
            let mut zero = value;
            match index {
                0 => zero.rewards.begin_verification = 0,
                1 => zero.rewards.verify_candidate = 0,
                2 => zero.rewards.finalize_selection = 0,
                3 => zero.rewards.settle = 0,
                4 => zero.rewards.lapse = 0,
                _ => unreachable!(),
            }
            assert_eq!(zero.validate(h(90)), Err(CodecError::ZeroValue));
            index += 1;
        }
    }

    #[test]
    fn reservation_v2_is_exact_618_and_v1_v2_refuse_each_other() {
        let value = reservation();
        let mut bytes = [0u8; DIRECT_RESERVATION_V2_BYTES];
        assert_eq!(
            value.encode(h(90), &mut bytes),
            Ok(DIRECT_RESERVATION_V2_BYTES)
        );
        assert_eq!(DirectReservationV2Account::decode(&bytes, h(90)), Ok(value));
        assert_eq!(
            ReservationAccount::decode(&bytes),
            Err(CodecError::TrailingBytes)
        );
        let mut v1 = [0u8; RESERVATION_ACCOUNT_BYTES];
        value.reservation.encode(&mut v1).unwrap();
        assert_eq!(v1[1], RESERVATION_ACCOUNT_VERSION);
        assert_eq!(
            DirectReservationV2Account::decode(&v1, h(90)),
            Err(CodecError::Truncated)
        );
        let mut wrong = bytes;
        wrong[1] = RESERVATION_ACCOUNT_VERSION;
        assert_eq!(
            DirectReservationV2Account::decode(&wrong, h(90)),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(&bytes[314..322], &4u64.to_le_bytes());
        assert_eq!(&bytes[322..442], &[0u8; 120]);
        assert_eq!(&bytes[442..450], &4u64.to_le_bytes());
        assert_eq!(&bytes[450..570], &[0u8; 120]);
        assert_eq!(&bytes[570..602], &h(24).0);
        assert_eq!(&bytes[602..610], &1_024u64.to_le_bytes());
        assert_eq!(&bytes[610..618], &24u64.to_le_bytes());
    }

    #[test]
    fn reservation_typed_phases_roundtrip_and_hostile_tail_refuses() {
        let mut value = reservation();
        for state in [
            RESERVATION_STATE_ACTIVE,
            RESERVATION_STATE_ENTITLED,
            RESERVATION_STATE_CONSUMED,
            RESERVATION_STATE_RELEASED,
        ] {
            value.reservation.state = state;
            match state {
                RESERVATION_STATE_ACTIVE | RESERVATION_STATE_ENTITLED => {
                    value.reservation.release_generation = 0;
                    value.reservation.remaining_internal = value.reservation.initial_internal;
                }
                RESERVATION_STATE_CONSUMED => {
                    value.reservation.release_generation = 0;
                    value.reservation.remaining_internal = [0; MAX_OUTCOMES];
                }
                RESERVATION_STATE_RELEASED => {
                    value.reservation.release_generation = 9;
                    value.reservation.remaining_internal = [0; MAX_OUTCOMES];
                }
                _ => unreachable!(),
            }
            let mut bytes = [0u8; DIRECT_RESERVATION_V2_BYTES];
            value.encode(h(90), &mut bytes).unwrap();
            assert_eq!(DirectReservationV2Account::decode(&bytes, h(90)), Ok(value));
        }
    }

    #[test]
    fn direct_batch_policy_binds_all_bytes_and_epoch_context() {
        let value = DirectBatchPolicyV3::direct(h(77)).unwrap();
        let mut bytes = [0u8; DIRECT_BATCH_POLICY_V3_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(DIRECT_BATCH_POLICY_V3_BYTES));
        assert_eq!(DirectBatchPolicyV3::decode(&bytes), Ok(value));
        let digest = value.digest_for_epoch(h(1)).unwrap();
        assert_ne!(digest, value.digest_for_epoch(h(2)).unwrap());
        bytes[DIRECT_BATCH_POLICY_V3_BYTES - 1] ^= 1;
        let changed = DirectBatchPolicyV3::decode(&bytes).unwrap();
        assert_ne!(digest, changed.digest_for_epoch(h(1)).unwrap());
        assert_eq!(
            DirectBatchPolicyV3::decode(&bytes[..DIRECT_BATCH_POLICY_V3_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        let mut zero_release = bytes;
        zero_release[BATCH_POLICY_BYTES..].fill(0);
        assert_eq!(
            DirectBatchPolicyV3::decode(&zero_release),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn every_codec_refuses_short_trailing_wrong_version_and_padding() {
        let value = candidate(DIRECT_CANDIDATE_STATUS_VERIFIED);
        let mut candidate_bytes = [0u8; DIRECT_CANDIDATE_V3_BYTES];
        value.encode(h(90), &mut candidate_bytes).unwrap();
        assert_eq!(
            DirectCandidateV3Account::decode(
                &candidate_bytes[..DIRECT_CANDIDATE_V3_BYTES - 1],
                h(90)
            ),
            Err(CodecError::Truncated)
        );
        let mut long = std::vec::Vec::from(candidate_bytes);
        long.push(0);
        assert_eq!(
            DirectCandidateV3Account::decode(&long, h(90)),
            Err(CodecError::TrailingBytes)
        );
        candidate_bytes[0] = DIRECT_WINDOW_TAG;
        assert_eq!(
            DirectCandidateV3Account::decode(&candidate_bytes, h(90)),
            Err(CodecError::WrongTag)
        );

        let mut epoch_bytes = [0u8; DIRECT_EPOCH_V4_BYTES];
        epoch(EPOCH_PHASE_OPEN, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN)
            .encode(&mut epoch_bytes)
            .unwrap();
        epoch_bytes[DIRECT_EPOCH_V4_BYTES - 1] = 1;
        assert_eq!(
            DirectEpochV4Account::decode(&epoch_bytes),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut policy = [0u8; DIRECT_BATCH_POLICY_V3_BYTES + 1];
        DirectBatchPolicyV3::direct(h(77))
            .unwrap()
            .encode(&mut policy)
            .unwrap();
        assert_eq!(
            DirectBatchPolicyV3::decode(&policy),
            Err(CodecError::TrailingBytes)
        );
    }

    fn intent_fixtures() -> [DirectV3Intent; 11] {
        let rewards = DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 2,
            finalize_selection: 3,
            settle: 4,
            lapse: 5,
        };
        [
            DirectV3Intent::InitEpoch {
                market: h(1),
                epoch_index: 7,
                policy: h(2),
                submission_opens_slot: 100,
                submission_closes_slot: 110,
                selection_deadline_slot: 120,
                settlement_deadline_slot: 140,
                neutral_lamport_sink: h(90),
            },
            DirectV3Intent::FreezeEpoch {
                market: h(1),
                epoch: h(2),
                reward_deposit: rewards.worst_case().unwrap(),
                rewards,
            },
            DirectV3Intent::AbortUnfrozen {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::SubmitCandidate {
                market: h(1),
                epoch: h(2),
                outcome_price: 2_500,
            },
            DirectV3Intent::BeginVerification {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::VerifyCandidate {
                market: h(1),
                epoch: h(2),
                retained_index: 2,
            },
            DirectV3Intent::FinalizeSelection {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::Settle {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::LapseEmpty {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::LapseUnselected {
                market: h(1),
                epoch: h(2),
            },
            DirectV3Intent::LapseSelected {
                market: h(1),
                epoch: h(2),
            },
        ]
    }

    #[test]
    fn direct_v3_intent_registry_and_every_exact_wire_are_frozen() {
        let intents = intent_fixtures();
        let expected_tags = [36u8, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46];
        let expected_lengths = [138usize, 114, 66, 74, 66, 67, 66, 66, 66, 66, 66];
        assert_eq!(
            INIT_DIRECT_EPOCH_V4_TAG,
            crate::resolution_work::ABORT_RESOLUTION_WORK_TAG + 1
        );
        assert_eq!(LAST_DIRECT_V3_INTENT_TAG, 46);
        let mut index = 0usize;
        while index < intents.len() {
            let intent = intents[index];
            assert_eq!(intent.tag(), expected_tags[index]);
            assert_eq!(intent.encoded_len(), expected_lengths[index]);
            let mut bytes = [0u8; INIT_DIRECT_EPOCH_V4_BYTES];
            let written = intent.encode(&mut bytes).unwrap();
            assert_eq!(written, expected_lengths[index]);
            assert_eq!(&bytes[..2], &[expected_tags[index], crate::INTENT_VERSION]);
            assert_eq!(DirectV3Intent::decode(&bytes[..written]), Ok(intent));
            index += 1;
        }
    }

    #[test]
    fn direct_v3_intents_refuse_hostile_versions_lengths_and_values() {
        for intent in intent_fixtures() {
            let mut bytes = [0u8; INIT_DIRECT_EPOCH_V4_BYTES + 1];
            let written = intent.encode(&mut bytes).unwrap();
            assert_eq!(
                DirectV3Intent::decode(&bytes[..written - 1]),
                Err(CodecError::Truncated)
            );
            bytes[written] = 0;
            assert_eq!(
                DirectV3Intent::decode(&bytes[..written + 1]),
                Err(CodecError::TrailingBytes)
            );
            bytes[1] = crate::INTENT_VERSION - 1;
            assert_eq!(
                DirectV3Intent::decode(&bytes[..written]),
                Err(CodecError::WrongVersion)
            );
        }

        let mut refused = [0xa5; INIT_DIRECT_EPOCH_V4_BYTES];
        let bad_schedule = DirectV3Intent::InitEpoch {
            market: h(1),
            epoch_index: 7,
            policy: h(2),
            submission_opens_slot: 100,
            submission_closes_slot: 101,
            selection_deadline_slot: 106,
            settlement_deadline_slot: 108,
            neutral_lamport_sink: h(90),
        };
        assert_eq!(
            bad_schedule.encode(&mut refused),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(refused, [0xa5; INIT_DIRECT_EPOCH_V4_BYTES]);

        let bad_price = DirectV3Intent::SubmitCandidate {
            market: h(1),
            epoch: h(2),
            outcome_price: 0,
        };
        assert_eq!(bad_price.validate(), Err(CodecError::ZeroValue));
        let bad_index = DirectV3Intent::VerifyCandidate {
            market: h(1),
            epoch: h(2),
            retained_index: 3,
        };
        assert_eq!(bad_index.validate(), Err(CodecError::InvalidCount));
        let zero_market = DirectV3Intent::LapseEmpty {
            market: Hash32::ZERO,
            epoch: h(2),
        };
        assert_eq!(zero_market.validate(), Err(CodecError::ZeroIdentity));
    }

    #[test]
    fn direct_v3_freeze_budget_is_checked_and_overflow_refuses() {
        let mut rewards = DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 2,
            finalize_selection: 3,
            settle: 4,
            lapse: 5,
        };
        let exact = rewards.worst_case().unwrap();
        assert_eq!(
            DirectV3Intent::FreezeEpoch {
                market: h(1),
                epoch: h(2),
                reward_deposit: exact - 1,
                rewards,
            }
            .validate(),
            Err(CodecError::ZeroValue)
        );
        rewards.verify_candidate = u64::MAX;
        assert_eq!(rewards.worst_case(), Err(CodecError::ArithmeticOverflow));
    }

    #[test]
    fn fixture_order_record_remains_single_egg_only() {
        let order = OrderRecord {
            owner: h(1),
            order_id: canonical_order_id(1),
            outcome: 0,
            side: 0,
            quantity: 4,
            limit: 7_500,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        };
        assert!(matches!(OrderSlot::Single(order), OrderSlot::Single(_)));
    }

    /// Byte identity between the two representations.
    fn im(byte: u8) -> Identity32V1 {
        Identity32V1([byte; 32])
    }

    /// Cross-crate tripwire: the executable model's recomputed frozen-page
    /// digest and order-set fold must byte-match the live layout's page fold
    /// over the same two records. This is the check that catches a model
    /// preimage drifting from `ORDER_RECORD_BYTES`/`ORDER_SLOT_BYTES`.
    #[test]
    fn model_frozen_page_digest_matches_live_page_fold() {
        const SCALE: u64 = 10_000;
        let mut grid = ModelGridV3 {
            grid_id: Identity32V1::ZERO,
            realm_id: im(8),
            price_scale: SCALE,
            tick_count: 2,
            ticks: {
                let mut ticks = [0u64; MAX_DIRECT_TICKS_V3 as usize];
                ticks[1] = SCALE;
                ticks
            },
            stored_bump: 4,
            flags: 0,
        };
        grid.grid_id = grid.recomputed_grid_id();
        let domain = ModelReservationDomainV3 {
            market_id: im(1),
            epoch_id: im(3),
            book_id: im(2),
            price_grid_id: grid.grid_id,
            terms_id: im(6),
            policy_id: batch_policy_digest(&DIRECT_POLICY_V1).unwrap(),
            epoch_index: 7,
            price_scale: SCALE,
            outcome_count: 2,
            remainder_seed: 9,
        };
        let schedule = ModelScheduleV3 {
            submission_opens_slot: 10,
            submission_closes_slot: 20,
            selection_deadline_slot: 30,
            settlement_deadline_slot: 40,
        };
        let authority = ModelLifecycleAuthorityV3 {
            verifier_release_id: im(80),
            direct_policy_v3_id: direct_policy_v3_digest(im(3), im(80)).unwrap(),
            neutral_lamport_sink: im(81),
        };
        let context = ModelTransitionContextV3 {
            now: 5,
            verifier_release_id: im(80),
        };
        let model_order = |rank: u64, owner: u8, side: u8, limit: u64| ModelPrefreezeOrderV3 {
            owner: im(owner),
            order_id: Identity32V1(canonical_order_id(rank).0),
            outcome: 0,
            side,
            quantity: 5,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        };
        let creation = |payer: u8, lamports: u64| ModelCreationFundingV3 {
            rent: ModelRentPrincipalV3 {
                payer: im(payer),
                lamports,
            },
            balance_before: 0,
            balance_after: lamports,
        };
        let buy_position = ModelPositionV3 {
            market_id: im(1),
            owner: im(70),
            generation: 1,
            internal: [0; 16],
            cash_atoms: 5,
            reserved_cash_atoms: 0,
            stored_bump: 3,
            close_state: 0,
        };
        let sell_position = ModelPositionV3 {
            internal: {
                let mut internal = [0u64; 16];
                internal[0] = 5;
                internal
            },
            cash_atoms: 0,
            owner: im(71),
            stored_bump: 4,
            ..buy_position
        };
        let one = ModelPrefreezeV3::initialize(schedule, authority, domain, grid, 4)
            .unwrap()
            .place_reservation(
                context,
                ModelReservationPlacementV3 {
                    order: model_order(1, 70, 0, SCALE),
                    position: buy_position,
                    max_fee_atoms: 0,
                    stored_bump: 5,
                    creation: creation(92, 600),
                },
                ModelObservedBalancesV3::ZERO,
            )
            .unwrap()
            .post;
        let two = one
            .place_reservation(
                context,
                ModelReservationPlacementV3 {
                    order: model_order(2, 71, 1, 0),
                    position: sell_position,
                    max_fee_atoms: 0,
                    stored_bump: 6,
                    creation: creation(93, 700),
                },
                ModelObservedBalancesV3 {
                    reservations: [600, 0],
                    ..ModelObservedBalancesV3::ZERO
                },
            )
            .unwrap()
            .post;
        let frozen = two
            .freeze(
                ModelTransitionContextV3 {
                    now: 9,
                    verifier_release_id: im(80),
                },
                ModelKeeperRewardsV3 {
                    begin_verification: 1,
                    verify_candidate: 1,
                    finalize_selection: 1,
                    settle: 1,
                    lapse: 1,
                },
                ModelWorkBudgetFundingV3 {
                    reward_sponsor: im(90),
                    creation: ModelCreationFundingV3 {
                        rent: ModelRentPrincipalV3 {
                            payer: im(90),
                            lamports: 500,
                        },
                        balance_before: 0,
                        balance_after: 506,
                    },
                    reward_lamports: 6,
                },
                ModelObservedBalancesV3 {
                    reservations: [600, 700],
                    ..ModelObservedBalancesV3::ZERO
                },
            )
            .unwrap()
            .post;

        let mut page = [0u8; crate::account_len::ORDER_PAGE];
        crate::stream::init_page(&mut page, h(1), h(3), 0, 1, 4).unwrap();
        let live_order = |rank: u64, owner: u8, side: u8, limit: u64| OrderRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            outcome: 0,
            side,
            quantity: 5,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        };
        crate::stream::write_single_slot(&mut page, &live_order(1, 70, 0, SCALE)).unwrap();
        let open = crate::stream::write_single_slot(&mut page, &live_order(2, 71, 1, 0)).unwrap();
        let (live_set, live_count) = crate::stream::frozen_set_commitment(&[&page]).unwrap();
        let sealed = crate::stream::seal_page(&mut page, live_set, live_count).unwrap();
        assert_eq!(sealed.page_digest, open.page_digest);
        assert_eq!(frozen.frozen_page.page_digest.0, sealed.page_digest.0);
        assert_eq!(frozen.frozen_page.order_set_id.0, live_set.0);
        assert_eq!(
            crate::stream::streamed_page_digest(&page).unwrap(),
            sealed.page_digest
        );
    }

    /// The model's epoch-bound DirectBatchPolicy V3 identity is byte-identical
    /// to the codec artifact digest, so the release identifier the model
    /// anchors is exactly the artifact identity the account plane binds.
    #[test]
    fn model_direct_policy_v3_digest_matches_codec_epoch_digest() {
        let artifact = DirectBatchPolicyV3::direct(h(80)).unwrap();
        let codec_digest = artifact.digest_for_epoch(h(3)).unwrap();
        let model_digest = direct_policy_v3_digest(im(3), im(80)).unwrap();
        assert_eq!(codec_digest.0, model_digest.0);
        assert_ne!(artifact.digest_for_epoch(h(4)).unwrap().0, model_digest.0);
    }
}
