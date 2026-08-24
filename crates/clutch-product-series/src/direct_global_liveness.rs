//! Product-owned authority over one complete seven-compartment liveness bundle.
//!
//! The account is a manifest and allocation lifecycle, not a second balance
//! ledger. Each physical [`clutch_liveness::runtime_v1::RuntimeCompartmentV1`]
//! remains the sole owner of work, rent, donation, and per-call accounting.
//! This owner binds the seven canonical accounts and their non-detachable
//! capitalization receipts, reserves disjoint Candidate call ranges for
//! Direct Markets, and refuses Product terminality until every allocation and
//! every physical compartment has terminal evidence.

use crate::codec::{Reader, Writer};
use crate::{content_id, ContentId, Error, FixedCodec, MarketInstanceV2Id, Result};
use clutch_liveness::runtime_v1::RUNTIME_COMPARTMENT_COUNT_V1;

const MAGIC_V1: [u8; 8] = *b"DCDGLIV1";
const SCHEMA_V1: u16 = 1;

/// Exact canonical number of Source/Candidate/Clearing/Settlement/Resolution/
/// Retirement/Recovery rows.
pub const DIRECT_GLOBAL_LIVENESS_COUNT_V1: usize = RUNTIME_COMPARTMENT_COUNT_V1;
/// Exact consecutive Candidate call range reserved for one Direct V5 occurrence.
pub const DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V1: u32 = 8;
/// Exact hostile-codec width of [`DirectGlobalLivenessV1`].
pub const DIRECT_GLOBAL_LIVENESS_BYTES_V1: usize = 1_008;
/// Stable semantic identity of the complete current state.
pub const DIRECT_GLOBAL_LIVENESS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/direct-global-liveness/v1";
/// Stable identity of the immutable seven-account binding.
pub const DIRECT_GLOBAL_LIVENESS_BINDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/direct-global-liveness-binding/v1";
/// Stable identity of the full-payer capitalization transcript.
pub const DIRECT_GLOBAL_LIVENESS_CAPITALIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/direct-global-liveness-capitalization/v1";
/// Stable identity of one Product-owned Candidate range allocation.
pub const DIRECT_GLOBAL_LIVENESS_ALLOCATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/direct-global-liveness-allocation/v1";
/// Stable identity of complete physical retirement.
pub const DIRECT_GLOBAL_LIVENESS_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/direct-global-liveness-terminal/v1";

const _: () = {
    assert!(DIRECT_GLOBAL_LIVENESS_COUNT_V1 == 7);
    assert!(DIRECT_GLOBAL_LIVENESS_BYTES_V1 == 16 + 14 * 32 + 2 * 7 * 32 + 96);
};

/// Product lifecycle phase for the separate `0xba/v1` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DirectGlobalLivenessPhaseV1 {
    /// Capitalized, but not yet consumed by the atomic Product founder.
    Founding = 1,
    /// Founder/root join is complete and Direct allocations may be admitted.
    Active = 2,
    /// No allocation remains live and physical terminal close may proceed.
    Retiring = 3,
}

impl DirectGlobalLivenessPhaseV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Founding => 1,
            Self::Active => 2,
            Self::Retiring => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Founding),
            2 => Ok(Self::Active),
            3 => Ok(Self::Retiring),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact immutable and monetary facts authenticated by the private SBF creator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessCapitalizationV1 {
    /// Canonical `0xba/v1` PDA.
    pub account_id: ContentId,
    /// Full-width Product Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Canonical Product `0xaa` expected in the same founder transaction.
    pub lifecycle_root_account: ContentId,
    /// Exact immutable Product root binding.
    pub market_binding_id: ContentId,
    /// Immutable Realm selected by Product Genesis.
    pub realm_id: ContentId,
    /// Exact immutable generic-liveness policy account.
    pub policy_account: ContentId,
    /// Policy semantic identity.
    pub policy_id: ContentId,
    /// Hostile account-data identity of the policy postimage.
    pub policy_data_id: ContentId,
    /// Market/generation-scoped global runtime lifecycle.
    pub global_lifecycle_id: ContentId,
    /// Stable binding of the policy and seven physical accounts.
    pub global_bundle_binding_id: ContentId,
    /// Exact atomic payer-debit/postwrite receipt for the complete bundle.
    pub global_capitalization_receipt_id: ContentId,
    /// Immutable destination for unused work and all rent principal.
    pub principal_refund_owner: ContentId,
    /// Immutable destination for donations and failure-path work residue.
    pub neutral_lamport_sink: ContentId,
    /// Source..Recovery physical accounts in canonical liveness order.
    pub compartment_accounts: [ContentId; DIRECT_GLOBAL_LIVENESS_COUNT_V1],
    /// Per-account full-payer capitalization receipts in the same order.
    pub compartment_capitalization_receipt_ids:
        [ContentId; DIRECT_GLOBAL_LIVENESS_COUNT_V1],
    /// Nonzero physical generation shared by every row.
    pub generation: u64,
    /// Exact work principal already present across the seven accounts.
    pub total_work_principal_lamports: u64,
    /// Exact refundable rent principal already present across the seven accounts.
    pub total_rent_principal_lamports: u64,
    /// Exact pre-capitalization bundle balance, owned only by the neutral sink.
    pub initial_bundle_donation_lamports: u64,
    /// Full payer-funded rent principal for the `0xba/v1` account itself.
    pub manifest_rent_principal_lamports: u64,
    /// Exact `0xba` prebalance, owned only by the neutral sink.
    pub manifest_initial_donation_lamports: u64,
    /// Candidate account's exact finite call capacity.
    pub candidate_maximum_calls: u32,
    /// Candidate account's exact prepaid work principal.
    pub candidate_work_principal_lamports: u64,
    /// Frozen number of consecutive calls owned by one Direct occurrence.
    pub allocation_call_width: u32,
}

/// Exact Product allocation offered to one Direct root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessAllocationV1 {
    /// Exact Direct root which consumes this allocation once.
    pub direct_root_account: ContentId,
    /// Exact Direct action-replay owner paired with that root.
    pub direct_action_replay_account: ContentId,
    /// One-based Product family admission sequence.
    pub family_admission_sequence: u32,
    /// First one-based Candidate call ordinal in this disjoint range.
    pub first_call_ordinal: u32,
    /// Exact consecutive call count; must equal the frozen account width.
    pub reserved_calls: u32,
    /// Exact sum of the Direct work schedule's call ceilings.
    pub reserved_work_lamports: u64,
    /// Direct-owned work-schedule identity.
    pub work_schedule_id: ContentId,
    /// One-way private Product allocation receipt.
    pub allocation_receipt_id: ContentId,
}

/// Exact terminal money partition aggregated from all seven physical owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessTerminalAccountingV1 {
    /// Per-row terminal receipt IDs in canonical order.
    pub compartment_terminal_receipt_ids: [ContentId; DIRECT_GLOBAL_LIVENESS_COUNT_V1],
    /// Work paid to keepers on accepted calls.
    pub keeper_paid_work_principal_lamports: u64,
    /// Unspent/per-call headroom work returned to the principal owner.
    pub refunded_work_principal_lamports: u64,
    /// Failure-path work residue sent to the neutral sink.
    pub sinked_work_principal_lamports: u64,
    /// All seven physical account rents returned to the principal owner.
    pub refunded_bundle_rent_principal_lamports: u64,
    /// Total bundle donations observed and sent to the neutral sink.
    pub sinked_bundle_donation_lamports: u64,
    /// `0xba` rent returned on physical close.
    pub refunded_manifest_rent_principal_lamports: u64,
    /// All `0xba` donations observed and sent to the neutral sink.
    pub sinked_manifest_donation_lamports: u64,
    /// Exact aggregate received by the immutable principal-refund owner.
    pub refundable_surplus_lamports: u64,
    /// Exact aggregate received by the immutable neutral sink.
    pub neutral_sink_lamports: u64,
    /// Product root transition receipt which makes this close inseparable.
    pub product_terminal_receipt_id: ContentId,
}

/// Private-consumer projection of complete physical retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessTerminalProjectionV1 {
    id: ContentId,
    account_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    market_binding_id: ContentId,
    global_bundle_binding_id: ContentId,
    global_capitalization_receipt_id: ContentId,
    principal_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    refundable_surplus_lamports: u64,
    neutral_sink_lamports: u64,
    final_transition_sequence: u64,
}

impl DirectGlobalLivenessTerminalProjectionV1 {
    /// Projection identity consumed by the Product root terminal composer.
    pub const fn id(self) -> ContentId { self.id }
    /// Canonical `0xba/v1` account being closed.
    pub const fn account_id(self) -> ContentId { self.account_id }
    /// Exact refundable principal and unused-work aggregate.
    pub const fn refundable_surplus_lamports(self) -> u64 {
        self.refundable_surplus_lamports
    }
    /// Exact donation/failure-residue aggregate.
    pub const fn neutral_sink_lamports(self) -> u64 { self.neutral_sink_lamports }
}

/// Default-refusing adapter seam. Live SBF implements it only for private,
/// non-detachable founder, Direct-root, and terminal postwrite receipts.
pub trait ProductDirectGlobalLivenessAuthorityV1 {
    /// Authenticate complete present capitalization before state creation.
    fn authenticate_capitalization(
        &self,
        _capitalization: &DirectGlobalLivenessCapitalizationV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the atomic Product founder/root join.
    fn authenticate_founder_activation(
        &self,
        _state: &DirectGlobalLivenessV1,
        _founder_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate one exact Direct root/replay allocation.
    fn authenticate_candidate_allocation(
        &self,
        _state: &DirectGlobalLivenessV1,
        _allocation: DirectGlobalLivenessAllocationV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the exact private Direct terminal receipt once.
    fn authenticate_candidate_retirement(
        &self,
        _state: &DirectGlobalLivenessV1,
        _direct_terminal_receipt_id: ContentId,
        _family_terminal_sequence: u32,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the Product root's narrow Active-to-Retiring transition.
    fn authenticate_root_retirement(
        &self,
        _state: &DirectGlobalLivenessV1,
        _root_retirement_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate exact seven-row terminal poststates and physical movements.
    fn authenticate_terminal_accounting(
        &self,
        _state: &DirectGlobalLivenessV1,
        _accounting: &DirectGlobalLivenessTerminalAccountingV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Sole semantic owner persisted in Product `0xba/v1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessV1 {
    phase: DirectGlobalLivenessPhaseV1,
    account_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    lifecycle_root_account: ContentId,
    market_binding_id: ContentId,
    realm_id: ContentId,
    policy_account: ContentId,
    policy_id: ContentId,
    policy_data_id: ContentId,
    global_lifecycle_id: ContentId,
    global_bundle_binding_id: ContentId,
    global_capitalization_receipt_id: ContentId,
    principal_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    lifecycle_transcript_id: ContentId,
    compartment_accounts: [ContentId; DIRECT_GLOBAL_LIVENESS_COUNT_V1],
    compartment_capitalization_receipt_ids: [ContentId; DIRECT_GLOBAL_LIVENESS_COUNT_V1],
    generation: u64,
    transition_sequence: u64,
    total_work_principal_lamports: u64,
    total_rent_principal_lamports: u64,
    initial_bundle_donation_lamports: u64,
    manifest_rent_principal_lamports: u64,
    manifest_initial_donation_lamports: u64,
    candidate_maximum_calls: u32,
    candidate_reserved_calls: u32,
    candidate_work_principal_lamports: u64,
    candidate_reserved_work_lamports: u64,
    admitted_allocations: u32,
    live_allocations: u32,
    retired_allocations: u32,
    allocation_call_width: u32,
}

impl DirectGlobalLivenessV1 {
    /// Construct only from an adapter-authenticated complete capitalization.
    pub fn initialize<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        authority: &A,
        capitalization: DirectGlobalLivenessCapitalizationV1,
    ) -> Result<Self> {
        authority.authenticate_capitalization(&capitalization)?;
        let value = Self {
            phase: DirectGlobalLivenessPhaseV1::Founding,
            account_id: capitalization.account_id,
            market_instance_id: capitalization.market_instance_id,
            lifecycle_root_account: capitalization.lifecycle_root_account,
            market_binding_id: capitalization.market_binding_id,
            realm_id: capitalization.realm_id,
            policy_account: capitalization.policy_account,
            policy_id: capitalization.policy_id,
            policy_data_id: capitalization.policy_data_id,
            global_lifecycle_id: capitalization.global_lifecycle_id,
            global_bundle_binding_id: capitalization.global_bundle_binding_id,
            global_capitalization_receipt_id: capitalization.global_capitalization_receipt_id,
            principal_refund_owner: capitalization.principal_refund_owner,
            neutral_lamport_sink: capitalization.neutral_lamport_sink,
            lifecycle_transcript_id: capitalization.global_capitalization_receipt_id,
            compartment_accounts: capitalization.compartment_accounts,
            compartment_capitalization_receipt_ids:
                capitalization.compartment_capitalization_receipt_ids,
            generation: capitalization.generation,
            transition_sequence: 1,
            total_work_principal_lamports: capitalization.total_work_principal_lamports,
            total_rent_principal_lamports: capitalization.total_rent_principal_lamports,
            initial_bundle_donation_lamports: capitalization.initial_bundle_donation_lamports,
            manifest_rent_principal_lamports: capitalization.manifest_rent_principal_lamports,
            manifest_initial_donation_lamports: capitalization
                .manifest_initial_donation_lamports,
            candidate_maximum_calls: capitalization.candidate_maximum_calls,
            candidate_reserved_calls: 0,
            candidate_work_principal_lamports: capitalization
                .candidate_work_principal_lamports,
            candidate_reserved_work_lamports: 0,
            admitted_allocations: 0,
            live_allocations: 0,
            retired_allocations: 0,
            allocation_call_width: capitalization.allocation_call_width,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate canonical state, exact counters, and money bounds.
    pub fn validate(&self) -> Result<()> {
        self.market_instance_id.validate()?;
        for id in [
            self.account_id,
            self.lifecycle_root_account,
            self.market_binding_id,
            self.realm_id,
            self.policy_account,
            self.policy_id,
            self.policy_data_id,
            self.global_lifecycle_id,
            self.global_bundle_binding_id,
            self.global_capitalization_receipt_id,
            self.principal_refund_owner,
            self.neutral_lamport_sink,
            self.lifecycle_transcript_id,
        ] {
            id.validate()?;
        }
        require_distinct(&[
            self.account_id,
            self.lifecycle_root_account,
            self.policy_account,
            self.principal_refund_owner,
            self.neutral_lamport_sink,
        ])?;
        require_distinct(&self.compartment_accounts)?;
        require_distinct(&self.compartment_capitalization_receipt_ids)?;
        let mut index = 0usize;
        while index < DIRECT_GLOBAL_LIVENESS_COUNT_V1 {
            self.compartment_accounts[index].validate()?;
            self.compartment_capitalization_receipt_ids[index].validate()?;
            for role in [
                self.account_id,
                self.lifecycle_root_account,
                self.policy_account,
                self.principal_refund_owner,
                self.neutral_lamport_sink,
            ] {
                if self.compartment_accounts[index] == role {
                    return Err(Error::MismatchedArtifact);
                }
            }
            index += 1;
        }
        let expected_reserved_calls = self
            .admitted_allocations
            .checked_mul(self.allocation_call_width)
            .ok_or(Error::ArithmeticOverflow)?;
        let expected_admitted = self
            .live_allocations
            .checked_add(self.retired_allocations)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.generation == 0
            || self.transition_sequence == 0
            || self.total_work_principal_lamports == 0
            || self.total_rent_principal_lamports == 0
            || self.manifest_rent_principal_lamports == 0
            || self.candidate_maximum_calls == 0
            || self.candidate_work_principal_lamports == 0
            || self.allocation_call_width != DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V1
            || self.allocation_call_width > self.candidate_maximum_calls
            || self.candidate_reserved_calls != expected_reserved_calls
            || self.candidate_reserved_calls > self.candidate_maximum_calls
            || self.candidate_reserved_work_lamports > self.candidate_work_principal_lamports
            || self.admitted_allocations != expected_admitted
            || (self.phase == DirectGlobalLivenessPhaseV1::Founding
                && self.admitted_allocations != 0)
            || (self.phase == DirectGlobalLivenessPhaseV1::Retiring
                && self.live_allocations != 0)
        {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }

    /// Stable identity of the complete mutable state.
    pub fn semantic_id(&self) -> Result<ContentId> {
        let mut body = [0u8; DIRECT_GLOBAL_LIVENESS_BYTES_V1];
        self.encode_into(&mut body)?;
        Ok(content_id(DIRECT_GLOBAL_LIVENESS_DOMAIN_V1, &body))
    }

    /// Consume the non-detachable founder receipt and enable allocations.
    pub fn activate_founder<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        founder_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        founder_receipt_id.validate()?;
        if self.phase != DirectGlobalLivenessPhaseV1::Founding {
            return Err(Error::WorkStateMismatch);
        }
        authority.authenticate_founder_activation(self, founder_receipt_id)?;
        let mut next = *self;
        next.phase = DirectGlobalLivenessPhaseV1::Active;
        next.advance_transcript(
            b"dragons-clutch/product/direct-global-liveness-founder/v1",
            founder_receipt_id,
        )?;
        next.validate()?;
        Ok(next)
    }

    /// Reserve the next exact, never-reused Candidate call range.
    pub fn allocate_candidate<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        allocation: DirectGlobalLivenessAllocationV1,
    ) -> Result<Self> {
        self.validate()?;
        for id in [
            allocation.direct_root_account,
            allocation.direct_action_replay_account,
            allocation.work_schedule_id,
            allocation.allocation_receipt_id,
        ] {
            id.validate()?;
        }
        require_distinct(&[
            allocation.direct_root_account,
            allocation.direct_action_replay_account,
            allocation.work_schedule_id,
            allocation.allocation_receipt_id,
        ])?;
        let expected_first = self
            .candidate_reserved_calls
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.phase != DirectGlobalLivenessPhaseV1::Active
            || allocation.family_admission_sequence == 0
            || allocation.first_call_ordinal != expected_first
            || allocation.reserved_calls != self.allocation_call_width
            || allocation.reserved_work_lamports == 0
        {
            return Err(Error::WorkStateMismatch);
        }
        let next_calls = self
            .candidate_reserved_calls
            .checked_add(allocation.reserved_calls)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_work = self
            .candidate_reserved_work_lamports
            .checked_add(allocation.reserved_work_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if next_calls > self.candidate_maximum_calls
            || next_work > self.candidate_work_principal_lamports
        {
            return Err(Error::InsufficientPrepayment);
        }
        authority.authenticate_candidate_allocation(self, allocation)?;
        let mut next = *self;
        next.candidate_reserved_calls = next_calls;
        next.candidate_reserved_work_lamports = next_work;
        next.admitted_allocations = next
            .admitted_allocations
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.live_allocations = next
            .live_allocations
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.advance_transcript(
            DIRECT_GLOBAL_LIVENESS_ALLOCATION_DOMAIN_V1,
            allocation.allocation_receipt_id,
        )?;
        next.validate()?;
        Ok(next)
    }

    /// Retire one allocation only from Direct's private terminal postwrite.
    pub fn retire_candidate<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        direct_terminal_receipt_id: ContentId,
        family_terminal_sequence: u32,
    ) -> Result<Self> {
        self.validate()?;
        direct_terminal_receipt_id.validate()?;
        if self.phase != DirectGlobalLivenessPhaseV1::Active
            || self.live_allocations == 0
            || family_terminal_sequence == 0
        {
            return Err(Error::WorkStateMismatch);
        }
        authority.authenticate_candidate_retirement(
            self,
            direct_terminal_receipt_id,
            family_terminal_sequence,
        )?;
        let mut next = *self;
        next.live_allocations -= 1;
        next.retired_allocations = next
            .retired_allocations
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.advance_transcript(
            b"dragons-clutch/product/direct-global-liveness-allocation-retired/v1",
            direct_terminal_receipt_id,
        )?;
        next.validate()?;
        Ok(next)
    }

    /// Seal allocations when Product enters Market retirement.
    pub fn begin_retirement<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        root_retirement_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        root_retirement_receipt_id.validate()?;
        if self.phase != DirectGlobalLivenessPhaseV1::Active || self.live_allocations != 0 {
            return Err(Error::WorkStateMismatch);
        }
        authority.authenticate_root_retirement(self, root_retirement_receipt_id)?;
        let mut next = *self;
        next.phase = DirectGlobalLivenessPhaseV1::Retiring;
        next.advance_transcript(
            b"dragons-clutch/product/direct-global-liveness-retiring/v1",
            root_retirement_receipt_id,
        )?;
        next.validate()?;
        Ok(next)
    }

    /// Project complete exact terminal movements. The SBF caller must consume
    /// this projection while closing `0xba` and advancing the Product root.
    pub fn terminal_projection<A: ProductDirectGlobalLivenessAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        accounting: DirectGlobalLivenessTerminalAccountingV1,
    ) -> Result<DirectGlobalLivenessTerminalProjectionV1> {
        self.validate()?;
        accounting.product_terminal_receipt_id.validate()?;
        require_distinct(&accounting.compartment_terminal_receipt_ids)?;
        for receipt in accounting.compartment_terminal_receipt_ids {
            receipt.validate()?;
        }
        if self.phase != DirectGlobalLivenessPhaseV1::Retiring {
            return Err(Error::WorkStateMismatch);
        }
        let work_partition = accounting
            .keeper_paid_work_principal_lamports
            .checked_add(accounting.refunded_work_principal_lamports)
            .and_then(|value| value.checked_add(accounting.sinked_work_principal_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        let refundable_surplus = accounting
            .refunded_work_principal_lamports
            .checked_add(accounting.refunded_bundle_rent_principal_lamports)
            .and_then(|value| value.checked_add(accounting.refunded_manifest_rent_principal_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        let neutral_sink = accounting
            .sinked_work_principal_lamports
            .checked_add(accounting.sinked_bundle_donation_lamports)
            .and_then(|value| value.checked_add(accounting.sinked_manifest_donation_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        if work_partition != self.total_work_principal_lamports
            || accounting.refunded_bundle_rent_principal_lamports
                != self.total_rent_principal_lamports
            || accounting.sinked_bundle_donation_lamports
                < self.initial_bundle_donation_lamports
            || accounting.refunded_manifest_rent_principal_lamports
                != self.manifest_rent_principal_lamports
            || accounting.sinked_manifest_donation_lamports
                < self.manifest_initial_donation_lamports
            || accounting.refundable_surplus_lamports != refundable_surplus
            || accounting.neutral_sink_lamports != neutral_sink
        {
            return Err(Error::WorkStateMismatch);
        }
        authority.authenticate_terminal_accounting(self, &accounting)?;
        let final_transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut body = [0u8; 32 * 15 + 8 * 4];
        let mut at = 0usize;
        for id in [
            self.account_id,
            self.market_instance_id.content_id(),
            self.market_binding_id,
            self.global_bundle_binding_id,
            self.global_capitalization_receipt_id,
            self.principal_refund_owner,
            self.neutral_lamport_sink,
            accounting.product_terminal_receipt_id,
        ] {
            body[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        for receipt in accounting.compartment_terminal_receipt_ids {
            // The fixed body has room for the complete seven-row evidence.
            if at + 32 > body.len() {
                return Err(Error::ArithmeticOverflow);
            }
            body[at..at + 32].copy_from_slice(&receipt.bytes());
            at += 32;
        }
        // The body size above deliberately covers 15 IDs and four scalars.
        for value in [
            self.generation,
            refundable_surplus,
            neutral_sink,
            final_transition_sequence,
        ] {
            body[at..at + 8].copy_from_slice(&value.to_le_bytes());
            at += 8;
        }
        let id = content_id(DIRECT_GLOBAL_LIVENESS_TERMINAL_DOMAIN_V1, &body);
        Ok(DirectGlobalLivenessTerminalProjectionV1 {
            id,
            account_id: self.account_id,
            market_instance_id: self.market_instance_id,
            generation: self.generation,
            market_binding_id: self.market_binding_id,
            global_bundle_binding_id: self.global_bundle_binding_id,
            global_capitalization_receipt_id: self.global_capitalization_receipt_id,
            principal_refund_owner: self.principal_refund_owner,
            neutral_lamport_sink: self.neutral_lamport_sink,
            refundable_surplus_lamports: refundable_surplus,
            neutral_sink_lamports: neutral_sink,
            final_transition_sequence,
        })
    }

    /// Exact current phase.
    pub const fn phase(&self) -> DirectGlobalLivenessPhaseV1 { self.phase }
    /// Canonical persisted account.
    pub const fn account_id(&self) -> ContentId { self.account_id }
    /// Product Market.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id { self.market_instance_id }
    /// Shared generation.
    pub const fn generation(&self) -> u64 { self.generation }
    /// Product root account.
    pub const fn lifecycle_root_account(&self) -> ContentId { self.lifecycle_root_account }
    /// Product root binding.
    pub const fn market_binding_id(&self) -> ContentId { self.market_binding_id }
    /// Immutable Realm.
    pub const fn realm_id(&self) -> ContentId { self.realm_id }
    /// Runtime policy account.
    pub const fn policy_account(&self) -> ContentId { self.policy_account }
    /// Runtime policy semantic ID.
    pub const fn policy_id(&self) -> ContentId { self.policy_id }
    /// Runtime policy data ID.
    pub const fn policy_data_id(&self) -> ContentId { self.policy_data_id }
    /// Shared runtime lifecycle.
    pub const fn global_lifecycle_id(&self) -> ContentId { self.global_lifecycle_id }
    /// Immutable seven-row bundle binding.
    pub const fn global_bundle_binding_id(&self) -> ContentId { self.global_bundle_binding_id }
    /// Complete capitalization receipt.
    pub const fn global_capitalization_receipt_id(&self) -> ContentId {
        self.global_capitalization_receipt_id
    }
    /// Immutable principal-refund owner.
    pub const fn principal_refund_owner(&self) -> ContentId { self.principal_refund_owner }
    /// Immutable neutral sink.
    pub const fn neutral_lamport_sink(&self) -> ContentId { self.neutral_lamport_sink }
    /// Exact refundable rent principal locked in `0xba/v1` itself.
    pub const fn manifest_rent_principal_lamports(&self) -> u64 {
        self.manifest_rent_principal_lamports
    }
    /// Initial unsolicited `0xba/v1` balance owned only by the neutral sink.
    pub const fn manifest_initial_donation_lamports(&self) -> u64 {
        self.manifest_initial_donation_lamports
    }
    /// Canonical physical account for one row.
    pub const fn compartment_account(&self, index: usize) -> Option<ContentId> {
        if index < DIRECT_GLOBAL_LIVENESS_COUNT_V1 {
            Some(self.compartment_accounts[index])
        } else {
            None
        }
    }
    /// Static capitalization receipt for one row.
    pub const fn compartment_capitalization_receipt_id(
        &self,
        index: usize,
    ) -> Option<ContentId> {
        if index < DIRECT_GLOBAL_LIVENESS_COUNT_V1 {
            Some(self.compartment_capitalization_receipt_ids[index])
        } else {
            None
        }
    }
    /// Next one-based Candidate call ordinal available for allocation.
    pub fn next_candidate_call_ordinal(&self) -> Result<u32> {
        self.candidate_reserved_calls
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)
    }
    /// Exact frozen calls per Direct allocation.
    pub const fn allocation_call_width(&self) -> u32 { self.allocation_call_width }
    /// Exact remaining unallocated Candidate work principal.
    pub fn unallocated_candidate_work_lamports(&self) -> Result<u64> {
        self.candidate_work_principal_lamports
            .checked_sub(self.candidate_reserved_work_lamports)
            .ok_or(Error::ArithmeticOverflow)
    }

    fn advance_transcript(&mut self, domain: &[u8], receipt: ContentId) -> Result<()> {
        receipt.validate()?;
        self.transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut body = [0u8; 72];
        body[..32].copy_from_slice(&self.lifecycle_transcript_id.bytes());
        body[32..64].copy_from_slice(&receipt.bytes());
        body[64..72].copy_from_slice(&self.transition_sequence.to_le_bytes());
        self.lifecycle_transcript_id = content_id(domain, &body);
        Ok(())
    }
}

impl FixedCodec for DirectGlobalLivenessV1 {
    const ENCODED_LEN: usize = DIRECT_GLOBAL_LIVENESS_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC_V1);
        writer.u16(SCHEMA_V1);
        writer.u8(self.phase.byte());
        writer.reserved(5);
        for id in [
            self.account_id,
            self.market_instance_id.content_id(),
            self.lifecycle_root_account,
            self.market_binding_id,
            self.realm_id,
            self.policy_account,
            self.policy_id,
            self.policy_data_id,
            self.global_lifecycle_id,
            self.global_bundle_binding_id,
            self.global_capitalization_receipt_id,
            self.principal_refund_owner,
            self.neutral_lamport_sink,
            self.lifecycle_transcript_id,
        ] {
            writer.id(id);
        }
        for account in self.compartment_accounts { writer.id(account); }
        for receipt in self.compartment_capitalization_receipt_ids { writer.id(receipt); }
        writer.u64(self.generation);
        writer.u64(self.transition_sequence);
        writer.u64(self.total_work_principal_lamports);
        writer.u64(self.total_rent_principal_lamports);
        writer.u64(self.initial_bundle_donation_lamports);
        writer.u64(self.manifest_rent_principal_lamports);
        writer.u64(self.manifest_initial_donation_lamports);
        writer.u32(self.candidate_maximum_calls);
        writer.u32(self.candidate_reserved_calls);
        writer.u64(self.candidate_work_principal_lamports);
        writer.u64(self.candidate_reserved_work_lamports);
        writer.u32(self.admitted_allocations);
        writer.u32(self.live_allocations);
        writer.u32(self.retired_allocations);
        writer.u32(self.allocation_call_width);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MAGIC_V1)?;
        if reader.u16() != SCHEMA_V1 { return Err(Error::BadVersion); }
        let phase = DirectGlobalLivenessPhaseV1::decode(reader.u8())?;
        reader.reserved(5)?;
        let account_id = reader.id();
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let lifecycle_root_account = reader.id();
        let market_binding_id = reader.id();
        let realm_id = reader.id();
        let policy_account = reader.id();
        let policy_id = reader.id();
        let policy_data_id = reader.id();
        let global_lifecycle_id = reader.id();
        let global_bundle_binding_id = reader.id();
        let global_capitalization_receipt_id = reader.id();
        let principal_refund_owner = reader.id();
        let neutral_lamport_sink = reader.id();
        let lifecycle_transcript_id = reader.id();
        let mut compartment_accounts = [ContentId::ZERO; DIRECT_GLOBAL_LIVENESS_COUNT_V1];
        for account in &mut compartment_accounts { *account = reader.id(); }
        let mut compartment_capitalization_receipt_ids =
            [ContentId::ZERO; DIRECT_GLOBAL_LIVENESS_COUNT_V1];
        for receipt in &mut compartment_capitalization_receipt_ids { *receipt = reader.id(); }
        let value = Self {
            phase,
            account_id,
            market_instance_id,
            lifecycle_root_account,
            market_binding_id,
            realm_id,
            policy_account,
            policy_id,
            policy_data_id,
            global_lifecycle_id,
            global_bundle_binding_id,
            global_capitalization_receipt_id,
            principal_refund_owner,
            neutral_lamport_sink,
            lifecycle_transcript_id,
            compartment_accounts,
            compartment_capitalization_receipt_ids,
            generation: reader.u64(),
            transition_sequence: reader.u64(),
            total_work_principal_lamports: reader.u64(),
            total_rent_principal_lamports: reader.u64(),
            initial_bundle_donation_lamports: reader.u64(),
            manifest_rent_principal_lamports: reader.u64(),
            manifest_initial_donation_lamports: reader.u64(),
            candidate_maximum_calls: reader.u32(),
            candidate_reserved_calls: reader.u32(),
            candidate_work_principal_lamports: reader.u64(),
            candidate_reserved_work_lamports: reader.u64(),
            admitted_allocations: reader.u32(),
            live_allocations: reader.u32(),
            retired_allocations: reader.u32(),
            allocation_call_width: reader.u32(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn require_distinct<const N: usize>(ids: &[ContentId; N]) -> Result<()> {
    let mut left = 0usize;
    while left < N {
        let mut right = left + 1;
        while right < N {
            if ids[left] == ids[right] { return Err(Error::MismatchedArtifact); }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}
