// SPDX-License-Identifier: AGPL-3.0-or-later

//! Streamed terminal allocation, claims, and independently closable LP pages.
//!
//! Allocation uses one named rounding boundary, `OwnerPrefixFloorV1`. Across
//! the globally owner-sorted page chain, entry `i` receives
//! `floor(C*S_i/T) - floor(C*S_{i-1}/T)`. The telescoping construction is
//! allocation-free, deterministic, exact in aggregate, and never requires all
//! LPs in one transaction.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerActionLivenessAuthorizationV1, DealerChildKindV2, DealerLivenessScheduleV1,
    DealerPhaseV2, DealerPolicyV1, DealerPositionObservationV3, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DeletableRentOwnerV1, Error,
    FacilityPositionBindingV2, FixedCodec, Id, LpPageV2, Result,
    DEALER_CLAIM_WORK_CONTENT_DOMAIN_V1, DEALER_TERMINAL_ALLOCATION_CONTENT_DOMAIN_V1,
    DELETABLE_RENT_OWNER_BYTES, LP_ENTRIES_PER_PAGE, MAX_ATOMS, MAX_LP_PAGES,
};
use clutch_retirement::PositionLifecycleV3;

/// Bytes in the page-closure bitmap for the maximum 4,096 pages.
pub const DEALER_PAGE_BITMAP_BYTES_V1: usize = (MAX_LP_PAGES as usize) / 8;
/// Local semantic magic for a page terminal allocation.
pub const DEALER_TERMINAL_ALLOCATION_MAGIC_V1: [u8; 8] = *b"DCTALCV1";
/// Exact local semantic version.
pub const DEALER_TERMINAL_ALLOCATION_VERSION_V1: u16 = 1;
/// Exact bytes in one page allocation.
pub const DEALER_TERMINAL_ALLOCATION_BYTES_V1: usize =
    HEADER_BYTES + (16 * 32) + 16 + (LP_ENTRIES_PER_PAGE * 8) + DELETABLE_RENT_OWNER_BYTES;
/// Local semantic magic for singleton claim work.
pub const DEALER_CLAIM_WORK_MAGIC_V1: [u8; 8] = *b"DCCLWMV1";
/// Exact local semantic version.
pub const DEALER_CLAIM_WORK_VERSION_V1: u16 = 1;
/// Exact bytes in the streamed claim-work owner.
pub const DEALER_CLAIM_WORK_BYTES_V1: usize = HEADER_BYTES
    + (15 * 32)
    + 56
    + DEALER_PAGE_BITMAP_BYTES_V1
    + DELETABLE_RENT_OWNER_BYTES;

/// Frozen terminal-allocation rounding policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerTerminalRoundingPolicyV1 {
    /// Difference of adjacent exact cumulative floor values in owner order.
    OwnerPrefixFloorV1 = 1,
}

impl DealerTerminalRoundingPolicyV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::OwnerPrefixFloorV1),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Mutable page-scoped claims; LP ownership and shares remain solely in `LpPageV2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTerminalAllocationV1 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Authoritative State account.
    pub dealer_state_account_id: Id,
    /// Canonical Position V3 purpose binding.
    pub facility_position_binding_id: Id,
    /// Physical LP page account.
    pub lp_page_account_id: Id,
    /// Immutable sealed LP page content identity.
    pub lp_page_content_id: Id,
    /// Singleton claim-work account.
    pub claim_work_account_id: Id,
    /// Authenticated terminal settlement identity.
    pub terminal_settlement_id: Id,
    /// Authenticated payout identity.
    pub payout_id: Id,
    /// Counted funded-dependency semantic identity.
    pub funded_dependencies_id: Id,
    /// External liveness policy.
    pub runtime_liveness_policy_id: Id,
    /// Seven-account liveness binding digest.
    pub runtime_liveness_binding_digest: Id,
    /// Fine-grained Dealer schedule.
    pub dealer_liveness_schedule_id: Id,
    /// Exact successful allocation receipt account.
    pub allocation_receipt_account_id: Id,
    /// Exact successful allocation receipt semantic identity.
    pub allocation_receipt_semantic_id: Id,
    /// Program admitted to own that receipt.
    pub allocation_receipt_program_id: Id,
    /// Parent generation at allocation.
    pub counted_generation: u64,
    /// Page ordinal.
    pub page_ordinal: u32,
    /// Active entry count copied only as a fixed-width codec bound.
    pub entry_count: u8,
    /// Claimed active-entry bitmap.
    pub claimed_bitmap: u16,
    /// Exact terminal claim atoms followed by zero padding.
    pub claim_atoms: [u64; LP_ENTRIES_PER_PAGE],
    /// Independently funded allocation rent.
    pub rent: DeletableRentOwnerV1,
}

impl DealerTerminalAllocationV1 {
    /// Validate identities, active bitmap, claim padding, and rent.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.lp_page_account_id,
            self.lp_page_content_id,
            self.claim_work_account_id,
            self.terminal_settlement_id,
            self.payout_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.allocation_receipt_account_id,
            self.allocation_receipt_semantic_id,
            self.allocation_receipt_program_id,
        ] {
            identity.validate_live()?;
        }
        let count = usize::from(self.entry_count);
        if count == 0 || count > LP_ENTRIES_PER_PAGE || self.page_ordinal >= MAX_LP_PAGES {
            return Err(Error::InvalidParameter);
        }
        let active_mask = if count == LP_ENTRIES_PER_PAGE {
            u16::MAX
        } else {
            (1u16 << count) - 1
        };
        if self.claimed_bitmap & !active_mask != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        let mut index = count;
        while index < LP_ENTRIES_PER_PAGE {
            if self.claim_atoms[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.rent.validate()
    }

    /// Whether every active entry, including zero-atom claims, was delivered.
    pub fn fully_claimed(&self) -> Result<bool> {
        self.validate()?;
        let count = usize::from(self.entry_count);
        let active_mask = if count == LP_ENTRIES_PER_PAGE {
            u16::MAX
        } else {
            (1u16 << count) - 1
        };
        Ok(self.claimed_bitmap == active_mask)
    }

    /// Exact current allocation identity.
    pub fn allocation_id(&self) -> Result<Id> {
        self.content_id(DEALER_TERMINAL_ALLOCATION_CONTENT_DOMAIN_V1)
    }

    /// Counted V2 child edge.
    pub const fn counted_child(&self) -> crate::CountedDealerChildV2 {
        crate::CountedDealerChildV2 {
            facility_id: self.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: DealerChildKindV2::TerminalAllocation,
            counted_generation: self.counted_generation,
        }
    }
}

impl FixedCodec for DealerTerminalAllocationV1 {
    const ENCODED_LEN: usize = DEALER_TERMINAL_ALLOCATION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_TERMINAL_ALLOCATION_MAGIC_V1,
            DEALER_TERMINAL_ALLOCATION_VERSION_V1,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.lp_page_account_id,
            self.lp_page_content_id,
            self.claim_work_account_id,
            self.terminal_settlement_id,
            self.payout_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.allocation_receipt_account_id,
            self.allocation_receipt_semantic_id,
            self.allocation_receipt_program_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.counted_generation);
        writer.u32(self.page_ordinal);
        writer.u8(self.entry_count);
        writer.u16(self.claimed_bitmap);
        writer.reserved(1);
        let mut index = 0usize;
        while index < LP_ENTRIES_PER_PAGE {
            writer.u64(self.claim_atoms[index]);
            index += 1;
        }
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_TERMINAL_ALLOCATION_MAGIC_V1,
            DEALER_TERMINAL_ALLOCATION_VERSION_V1,
        )?;
        let mut identities = [Id::ZERO; 16];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index] = reader.id();
            index += 1;
        }
        let counted_generation = reader.u64();
        let page_ordinal = reader.u32();
        let entry_count = reader.u8();
        let claimed_bitmap = reader.u16();
        reader.reserved(1)?;
        let mut claim_atoms = [0u64; LP_ENTRIES_PER_PAGE];
        index = 0;
        while index < LP_ENTRIES_PER_PAGE {
            claim_atoms[index] = reader.u64();
            index += 1;
        }
        let value = Self {
            policy_id: identities[0],
            facility_id: identities[1],
            dealer_state_account_id: identities[2],
            facility_position_binding_id: identities[3],
            lp_page_account_id: identities[4],
            lp_page_content_id: identities[5],
            claim_work_account_id: identities[6],
            terminal_settlement_id: identities[7],
            payout_id: identities[8],
            funded_dependencies_id: identities[9],
            runtime_liveness_policy_id: identities[10],
            runtime_liveness_binding_digest: identities[11],
            dealer_liveness_schedule_id: identities[12],
            allocation_receipt_account_id: identities[13],
            allocation_receipt_semantic_id: identities[14],
            allocation_receipt_program_id: identities[15],
            counted_generation,
            page_ordinal,
            entry_count,
            claimed_bitmap,
            claim_atoms,
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Singleton owner of bounded allocation progress and page-close membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerClaimWorkV1 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Authoritative State account.
    pub dealer_state_account_id: Id,
    /// Canonical Position V3 purpose binding.
    pub facility_position_binding_id: Id,
    /// Physical claim-work account.
    pub claim_work_account_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Authenticated terminal settlement identity.
    pub terminal_settlement_id: Id,
    /// Authenticated payout identity.
    pub payout_id: Id,
    /// Counted funded-dependency identity.
    pub funded_dependencies_id: Id,
    /// External liveness policy.
    pub runtime_liveness_policy_id: Id,
    /// Seven-account liveness binding digest.
    pub runtime_liveness_binding_digest: Id,
    /// Dealer liveness schedule.
    pub dealer_liveness_schedule_id: Id,
    /// Opening Resolve receipt account.
    pub resolve_receipt_account_id: Id,
    /// Opening Resolve receipt semantic identity.
    pub resolve_receipt_semantic_id: Id,
    /// Program admitted to own that receipt.
    pub resolve_receipt_program_id: Id,
    /// Frozen terminal allocation rounding boundary.
    pub rounding_policy: DealerTerminalRoundingPolicyV1,
    /// Parent generation at opening.
    pub counted_generation: u64,
    /// Immutable number of pages in the terminal roster.
    pub original_page_count: u32,
    /// Next page ordinal that may receive an allocation.
    pub next_allocation_page_ordinal: u32,
    /// Immutable aggregate LP shares.
    pub original_total_shares: u64,
    /// Exact terminal cash distributed to LPs.
    pub terminal_cash_atoms: u64,
    /// Share prefix consumed by completed allocations.
    pub allocated_share_prefix: u64,
    /// Cash atoms assigned by completed allocations.
    pub allocated_cash_atoms: u64,
    /// Bit set exactly when a page/allocation pair has closed.
    pub closed_pages: [u8; DEALER_PAGE_BITMAP_BYTES_V1],
    /// Independently funded claim-work rent.
    pub rent: DeletableRentOwnerV1,
}

impl DealerClaimWorkV1 {
    /// Validate bounded cursors, totals, bitmap padding, and rent.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.claim_work_account_id,
            self.market_instance_v2_id,
            self.terminal_settlement_id,
            self.payout_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.resolve_receipt_account_id,
            self.resolve_receipt_semantic_id,
            self.resolve_receipt_program_id,
        ] {
            identity.validate_live()?;
        }
        if self.original_page_count == 0
            || self.original_page_count > MAX_LP_PAGES
            || self.next_allocation_page_ordinal > self.original_page_count
            || self.original_total_shares == 0
            || self.original_total_shares > MAX_ATOMS
            || self.terminal_cash_atoms > 4 * MAX_ATOMS
            || self.allocated_share_prefix > self.original_total_shares
            || self.allocated_cash_atoms > self.terminal_cash_atoms
        {
            return Err(Error::InvalidParameter);
        }
        let active_bits = usize::try_from(self.original_page_count)
            .map_err(|_| Error::InvalidParameter)?;
        let mut bit = active_bits;
        while bit < DEALER_PAGE_BITMAP_BYTES_V1 * 8 {
            if self.closed_pages[bit / 8] & (1u8 << (bit % 8)) != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            bit += 1;
        }
        self.rent.validate()
    }

    /// Whether one original page is already closed.
    pub fn page_closed(&self, ordinal: u32) -> Result<bool> {
        self.validate()?;
        if ordinal >= self.original_page_count {
            return Err(Error::InvalidParameter);
        }
        let index = usize::try_from(ordinal).map_err(|_| Error::InvalidParameter)?;
        Ok(self.closed_pages[index / 8] & (1u8 << (index % 8)) != 0)
    }

    fn mark_page_closed(&mut self, ordinal: u32) -> Result<()> {
        if self.page_closed(ordinal)? {
            return Err(Error::InvalidChildGraph);
        }
        let index = usize::try_from(ordinal).map_err(|_| Error::InvalidParameter)?;
        self.closed_pages[index / 8] |= 1u8 << (index % 8);
        self.validate()
    }

    /// Number of page pairs already closed, derived from the owned bitmap.
    pub fn closed_page_count(&self) -> Result<u32> {
        self.validate()?;
        let mut count = 0u32;
        let mut index = 0usize;
        while index < self.closed_pages.len() {
            count = count
                .checked_add(self.closed_pages[index].count_ones())
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(count)
    }

    /// Exact current work identity.
    pub fn work_id(&self) -> Result<Id> {
        self.content_id(DEALER_CLAIM_WORK_CONTENT_DOMAIN_V1)
    }

    /// Counted V2 child edge.
    pub const fn counted_child(&self) -> crate::CountedDealerChildV2 {
        crate::CountedDealerChildV2 {
            facility_id: self.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: DealerChildKindV2::ClaimWork,
            counted_generation: self.counted_generation,
        }
    }
}

impl FixedCodec for DealerClaimWorkV1 {
    const ENCODED_LEN: usize = DEALER_CLAIM_WORK_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_CLAIM_WORK_MAGIC_V1, DEALER_CLAIM_WORK_VERSION_V1);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.facility_position_binding_id,
            self.claim_work_account_id,
            self.market_instance_v2_id,
            self.terminal_settlement_id,
            self.payout_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.resolve_receipt_account_id,
            self.resolve_receipt_semantic_id,
            self.resolve_receipt_program_id,
        ] {
            writer.id(identity);
        }
        writer.u8(self.rounding_policy as u8);
        writer.reserved(7);
        writer.u64(self.counted_generation);
        writer.u32(self.original_page_count);
        writer.u32(self.next_allocation_page_ordinal);
        writer.u64(self.original_total_shares);
        writer.u64(self.terminal_cash_atoms);
        writer.u64(self.allocated_share_prefix);
        writer.u64(self.allocated_cash_atoms);
        writer.bytes(&self.closed_pages);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_CLAIM_WORK_MAGIC_V1, DEALER_CLAIM_WORK_VERSION_V1)?;
        let mut identities = [Id::ZERO; 15];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index] = reader.id();
            index += 1;
        }
        let rounding_policy = DealerTerminalRoundingPolicyV1::decode(reader.u8())?;
        reader.reserved(7)?;
        let counted_generation = reader.u64();
        let original_page_count = reader.u32();
        let next_allocation_page_ordinal = reader.u32();
        let original_total_shares = reader.u64();
        let terminal_cash_atoms = reader.u64();
        let allocated_share_prefix = reader.u64();
        let allocated_cash_atoms = reader.u64();
        let mut closed_pages = [0u8; DEALER_PAGE_BITMAP_BYTES_V1];
        reader.copy_bytes(&mut closed_pages);
        let value = Self {
            policy_id: identities[0],
            facility_id: identities[1],
            dealer_state_account_id: identities[2],
            facility_position_binding_id: identities[3],
            claim_work_account_id: identities[4],
            market_instance_v2_id: identities[5],
            terminal_settlement_id: identities[6],
            payout_id: identities[7],
            funded_dependencies_id: identities[8],
            runtime_liveness_policy_id: identities[9],
            runtime_liveness_binding_digest: identities[10],
            dealer_liveness_schedule_id: identities[11],
            resolve_receipt_account_id: identities[12],
            resolve_receipt_semantic_id: identities[13],
            resolve_receipt_program_id: identities[14],
            rounding_policy,
            counted_generation,
            original_page_count,
            next_allocation_page_ordinal,
            original_total_shares,
            terminal_cash_atoms,
            allocated_share_prefix,
            allocated_cash_atoms,
            closed_pages,
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Begin bounded terminal allocation after canonical Position resolution.
#[allow(clippy::too_many_arguments)]
pub fn begin_terminal_resolution_v1(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    work: &DealerClaimWorkV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    resolve: &DealerActionLivenessAuthorizationV1,
    position_before: &DealerPositionObservationV3,
    position_resolved: &DealerPositionObservationV3,
    current_slot: u64,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    work.validate()?;
    resolve.validate_against(schedule, runtime)?;
    let binding_id = binding.binding_id()?;
    position_before.validate_current(state, binding, policy)?;
    position_resolved.validate_against(binding, binding_id, policy)?;
    let before = position_before.projection.position();
    let after = position_resolved.projection.position();
    if !matches!(state.phase, DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly)
        || current_slot < policy.maturity_slot
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.claim_work != 0
        || state.children.terminal_allocations != 0
        || work.policy_id != policy.policy_id()?
        || work.facility_id != state.facility_id
        || work.dealer_state_account_id != state_account_id
        || work.facility_position_binding_id != binding_id
        || work.market_instance_v2_id != policy.market_instance_v2_id
        || work.rounding_policy != DealerTerminalRoundingPolicyV1::OwnerPrefixFloorV1
        || work.funded_dependencies_id != state.funded_dependencies_id
        || work.original_page_count != state.children.lp_pages
        || work.original_total_shares != state.total_shares
        || work.counted_generation != state.generation
        || work.next_allocation_page_ordinal != 0
        || work.allocated_share_prefix != 0
        || work.allocated_cash_atoms != 0
        || work.closed_pages != [0; DEALER_PAGE_BITMAP_BYTES_V1]
        || work.terminal_cash_atoms != after.cash_atoms()
        || after.reserved_cash_atoms() != 0
        || after.native_eggs() != [0; crate::MAX_OUTCOMES]
        || after.generation() != state.generation.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        || after.lifecycle() != PositionLifecycleV3::Open
        || Id::from_bytes(after.replay_account().bytes()) == state.facility_replay_account_id
        || resolve.action != DealerRuntimeActionV1::Resolve
        || resolve.owner != state_account_id
        || resolve.lifecycle_id != state.facility_id
        || resolve.facility_generation != state.generation
        || work.resolve_receipt_account_id != resolve.receipt_account_id
        || work.resolve_receipt_semantic_id != resolve.receipt_semantic_id
        || work.resolve_receipt_program_id != resolve.receipt_program_id
        || before.outstanding_reservations() != 0
    {
        return Err(Error::MismatchedBinding);
    }
    let mut next = *state;
    next.phase = DealerPhaseV2::Resolving;
    next.net_sold = [0; crate::MAX_OUTCOMES];
    next.facility_position_id = position_resolved.semantic_id;
    next.facility_replay_account_id = Id::from_bytes(after.replay_account().bytes());
    next.generation = after.generation();
    next.children.claim_work = 1;
    next.children.unclaimed_lp_positions = next.children.live_lp_positions;
    next.child_sequence = next
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next.validate_against_policy(policy)?;
    Ok(next)
}

/// Result of allocating the next owner-sorted LP page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAllocateTerminalPageV1 {
    /// Updated State counter.
    pub state_after: DealerStateV2,
    /// Updated prefix work.
    pub work_after: DealerClaimWorkV1,
    /// Exact page allocation initialized with an empty claimed bitmap.
    pub allocation: DealerTerminalAllocationV1,
}

/// Allocate the next page under `OwnerPrefixFloorV1`.
#[allow(clippy::too_many_arguments)]
pub fn allocate_terminal_page_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    work: &DealerClaimWorkV1,
    page_account_id: Id,
    page: &LpPageV2,
    mut allocation: DealerTerminalAllocationV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    resolve: &DealerActionLivenessAuthorizationV1,
) -> Result<DealerAllocateTerminalPageV1> {
    state.validate_against_policy(policy)?;
    work.validate()?;
    page.validate_against(policy, state, state_account_id)?;
    allocation.validate()?;
    resolve.validate_against(schedule, runtime)?;
    if state.phase != DealerPhaseV2::Resolving
        || state.children.claim_work != 1
        || page.page_ordinal != work.next_allocation_page_ordinal
        || page.page_ordinal != allocation.page_ordinal
        || page.entry_count != allocation.entry_count
        || !page.sealed
        || page_account_id != allocation.lp_page_account_id
        || page.page_content_id()? != allocation.lp_page_content_id
        || allocation.claim_work_account_id != work.claim_work_account_id
        || allocation.policy_id != work.policy_id
        || allocation.facility_id != work.facility_id
        || allocation.dealer_state_account_id != state_account_id
        || allocation.facility_position_binding_id != work.facility_position_binding_id
        || allocation.terminal_settlement_id != work.terminal_settlement_id
        || allocation.payout_id != work.payout_id
        || allocation.funded_dependencies_id != work.funded_dependencies_id
        || allocation.runtime_liveness_policy_id != work.runtime_liveness_policy_id
        || allocation.runtime_liveness_binding_digest != work.runtime_liveness_binding_digest
        || allocation.dealer_liveness_schedule_id != work.dealer_liveness_schedule_id
        || allocation.counted_generation != state.generation
        || allocation.claimed_bitmap != 0
        || resolve.action != DealerRuntimeActionV1::Resolve
        || resolve.owner != state_account_id
        || resolve.lifecycle_id != state.facility_id
        || resolve.facility_generation != state.generation
        || allocation.allocation_receipt_account_id != resolve.receipt_account_id
        || allocation.allocation_receipt_semantic_id != resolve.receipt_semantic_id
        || allocation.allocation_receipt_program_id != resolve.receipt_program_id
    {
        return Err(Error::MismatchedBinding);
    }
    let mut share_prefix = work.allocated_share_prefix;
    let mut cash_prefix = work.allocated_cash_atoms;
    let mut index = 0usize;
    while index < usize::from(page.entry_count) {
        let next_share = share_prefix
            .checked_add(page.entries[index].shares)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_cash = prefix_floor(
            work.terminal_cash_atoms,
            next_share,
            work.original_total_shares,
        )?;
        allocation.claim_atoms[index] = next_cash
            .checked_sub(cash_prefix)
            .ok_or(Error::ConservationFailure)?;
        share_prefix = next_share;
        cash_prefix = next_cash;
        index += 1;
    }
    while index < LP_ENTRIES_PER_PAGE {
        allocation.claim_atoms[index] = 0;
        index += 1;
    }
    allocation.validate()?;
    let mut work_after = *work;
    work_after.next_allocation_page_ordinal = work_after
        .next_allocation_page_ordinal
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    work_after.allocated_share_prefix = share_prefix;
    work_after.allocated_cash_atoms = cash_prefix;
    work_after.validate()?;
    let mut state_after = *state;
    state_after.children.terminal_allocations = state_after
        .children
        .terminal_allocations
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if work_after.next_allocation_page_ordinal == work_after.original_page_count {
        if work_after.allocated_share_prefix != work_after.original_total_shares
            || work_after.allocated_cash_atoms != work_after.terminal_cash_atoms
            || state_after.children.terminal_allocations != state_after.children.lp_pages
        {
            return Err(Error::ConservationFailure);
        }
        state_after.phase = DealerPhaseV2::Resolved;
    }
    state_after.validate_against_policy(policy)?;
    Ok(DealerAllocateTerminalPageV1 {
        state_after,
        work_after,
        allocation,
    })
}

fn prefix_floor(cash: u64, shares: u64, total: u64) -> Result<u64> {
    if total == 0 || shares > total {
        return Err(Error::InvalidParameter);
    }
    let value = u128::from(cash)
        .checked_mul(u128::from(shares))
        .ok_or(Error::ArithmeticOverflow)?
        / u128::from(total);
    u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

#[allow(clippy::too_many_arguments)]
fn validate_terminal_page_binding_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    work: &DealerClaimWorkV1,
    page_account_id: Id,
    page: &LpPageV2,
    allocation: &DealerTerminalAllocationV1,
) -> Result<()> {
    work.validate()?;
    page.validate_against(policy, state, state_account_id)?;
    allocation.validate()?;
    if !page.sealed
        || work.policy_id != state.policy_id
        || work.facility_id != state.facility_id
        || work.dealer_state_account_id != state_account_id
        || work.facility_position_binding_id != state.facility_position_binding_id
        || work.market_instance_v2_id != policy.market_instance_v2_id
        || work.funded_dependencies_id != state.funded_dependencies_id
        || work.rounding_policy != DealerTerminalRoundingPolicyV1::OwnerPrefixFloorV1
        || allocation.policy_id != work.policy_id
        || allocation.facility_id != work.facility_id
        || allocation.dealer_state_account_id != work.dealer_state_account_id
        || allocation.facility_position_binding_id != work.facility_position_binding_id
        || allocation.lp_page_account_id != page_account_id
        || allocation.lp_page_content_id != page.page_content_id()?
        || allocation.claim_work_account_id != work.claim_work_account_id
        || allocation.terminal_settlement_id != work.terminal_settlement_id
        || allocation.payout_id != work.payout_id
        || allocation.funded_dependencies_id != work.funded_dependencies_id
        || allocation.runtime_liveness_policy_id != work.runtime_liveness_policy_id
        || allocation.runtime_liveness_binding_digest != work.runtime_liveness_binding_digest
        || allocation.dealer_liveness_schedule_id != work.dealer_liveness_schedule_id
        || allocation.page_ordinal != page.page_ordinal
        || allocation.entry_count != page.entry_count
        || allocation.rent.neutral_sink != policy.neutral_sink
        || work.rent.neutral_sink != policy.neutral_sink
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

/// Claim one immutable LP entry and advance the shared Position/Replay generation.
#[allow(clippy::too_many_arguments)]
pub fn claim_terminal_entry_v1(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    work: &DealerClaimWorkV1,
    page_account_id: Id,
    page: &LpPageV2,
    allocation: &DealerTerminalAllocationV1,
    entry_index: u8,
    owner: Id,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    claim: &DealerActionLivenessAuthorizationV1,
    position_before: &DealerPositionObservationV3,
    position_after: &DealerPositionObservationV3,
) -> Result<(DealerStateV2, DealerTerminalAllocationV1)> {
    state.validate_against_policy(policy)?;
    work.validate()?;
    validate_terminal_page_binding_v1(
        policy,
        state,
        state_account_id,
        work,
        page_account_id,
        page,
        allocation,
    )?;
    claim.validate_against(schedule, runtime)?;
    position_before.validate_current(state, binding, policy)?;
    let binding_id = binding.binding_id()?;
    position_after.validate_against(binding, binding_id, policy)?;
    let index = usize::from(entry_index);
    if state.phase != DealerPhaseV2::Resolved
        || index >= usize::from(page.entry_count)
        || allocation.page_ordinal != page.page_ordinal
        || allocation.lp_page_content_id != page.page_content_id()?
        || allocation.claim_work_account_id != work.claim_work_account_id
        || allocation.claimed_bitmap & (1u16 << index) != 0
        || page.entries[index].owner != owner
        || claim.action != DealerRuntimeActionV1::Claim
        || claim.owner != state_account_id
        || claim.lifecycle_id != state.facility_id
        || claim.facility_generation != state.generation
    {
        return Err(Error::MismatchedBinding);
    }
    let before = position_before.projection.position();
    let after = position_after.projection.position();
    let amount = allocation.claim_atoms[index];
    if after.cash_atoms()
        != before
            .cash_atoms()
            .checked_sub(amount)
            .ok_or(Error::ConservationFailure)?
        || after.reserved_cash_atoms() != before.reserved_cash_atoms()
        || after.native_eggs() != before.native_eggs()
        || after.generation() != state.generation.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        || after.lifecycle() != PositionLifecycleV3::Open
        || Id::from_bytes(after.replay_account().bytes()) == state.facility_replay_account_id
    {
        return Err(Error::ConservationFailure);
    }
    let mut allocation_after = *allocation;
    allocation_after.claimed_bitmap |= 1u16 << index;
    allocation_after.validate()?;
    let mut state_after = *state;
    state_after.children.unclaimed_lp_positions = state_after
        .children
        .unclaimed_lp_positions
        .checked_sub(1)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.terminal_claimed_shares = state_after
        .terminal_claimed_shares
        .checked_add(page.entries[index].shares)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.facility_position_id = position_after.semantic_id;
    state_after.facility_replay_account_id = Id::from_bytes(after.replay_account().bytes());
    state_after.generation = after.generation();
    state_after.validate_against_policy(policy)?;
    Ok((state_after, allocation_after))
}

/// Exact close observations for a claimed page, allocation, and optional final work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTerminalPageCloseRentV1 {
    /// Exact payer identities for Page, Allocation, and optional Work.
    pub payers: [Id; 3],
    /// Exact neutral-sink identities for Page, Allocation, and optional Work.
    pub neutral_sinks: [Id; 3],
    /// Page lamports before close.
    pub page_lamports_before: u64,
    /// Allocation lamports before close.
    pub allocation_lamports_before: u64,
    /// Claim-work lamports before close; zero unless this is the last page.
    pub work_lamports_before: u64,
    /// All closed accounts must reload at zero lamports.
    pub closed_lamports_after: [u64; 3],
    /// Exact independently credited refundable principals.
    pub payer_refund_lamports: [u64; 3],
    /// Exact independently credited donation floors and surplus.
    pub sink_lamports: [u64; 3],
}

/// Close a fully claimed page/allocation pair; the final pair also closes work.
pub fn close_claimed_lp_page_v1(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    work: &DealerClaimWorkV1,
    page_account_id: Id,
    page: &LpPageV2,
    allocation: &DealerTerminalAllocationV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    retire: &DealerActionLivenessAuthorizationV1,
    rent: DealerTerminalPageCloseRentV1,
) -> Result<(DealerStateV2, Option<DealerClaimWorkV1>)> {
    state.validate_against_policy(policy)?;
    validate_terminal_page_binding_v1(
        policy,
        state,
        state_account_id,
        work,
        page_account_id,
        page,
        allocation,
    )?;
    retire.validate_against(schedule, runtime)?;
    if state.phase != DealerPhaseV2::Resolved
        || !allocation.fully_claimed()?
        || allocation.page_ordinal != page.page_ordinal
        || allocation.lp_page_content_id != page.page_content_id()?
        || allocation.claim_work_account_id != work.claim_work_account_id
        || work.page_closed(page.page_ordinal)?
        || retire.action != DealerRuntimeActionV1::Retire
        || retire.owner != state_account_id
        || retire.lifecycle_id != state.facility_id
        || retire.facility_generation != state.generation
    {
        return Err(Error::MismatchedBinding);
    }
    let (page_shares, page_queued) = page.share_totals()?;
    let last = state.children.lp_pages == 1;
    if rent.closed_lamports_after[0] != 0
        || rent.closed_lamports_after[1] != 0
        || rent.closed_lamports_after[2] != 0
        || (!last && rent.work_lamports_before != 0)
    {
        return Err(Error::ConservationFailure);
    }
    let page_protected = page
        .rent
        .refundable_principal
        .checked_add(page.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    let allocation_protected = allocation
        .rent
        .refundable_principal
        .checked_add(allocation.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    let work_protected = if last {
        work.rent
            .refundable_principal
            .checked_add(work.rent.donation_floor)
            .ok_or(Error::ArithmeticOverflow)?
    } else {
        0
    };
    if rent.page_lamports_before < page_protected
        || rent.allocation_lamports_before < allocation_protected
        || rent.work_lamports_before < work_protected
    {
        return Err(Error::ConservationFailure);
    }
    let expected_payers = if last {
        [page.rent.payer, allocation.rent.payer, work.rent.payer]
    } else {
        [page.rent.payer, allocation.rent.payer, Id::ZERO]
    };
    let expected_sinks = if last {
        [
            page.rent.neutral_sink,
            allocation.rent.neutral_sink,
            work.rent.neutral_sink,
        ]
    } else {
        [page.rent.neutral_sink, allocation.rent.neutral_sink, Id::ZERO]
    };
    let expected_refunds = if last {
        [
            page.rent.refundable_principal,
            allocation.rent.refundable_principal,
            work.rent.refundable_principal,
        ]
    } else {
        [
            page.rent.refundable_principal,
            allocation.rent.refundable_principal,
            0,
        ]
    };
    let expected_sinks_lamports = [
        rent.page_lamports_before
            .checked_sub(page.rent.refundable_principal)
            .ok_or(Error::ConservationFailure)?,
        rent.allocation_lamports_before
            .checked_sub(allocation.rent.refundable_principal)
            .ok_or(Error::ConservationFailure)?,
        if last {
            rent.work_lamports_before
                .checked_sub(work.rent.refundable_principal)
                .ok_or(Error::ConservationFailure)?
        } else {
            0
        },
    ];
    if rent.payers != expected_payers
        || rent.neutral_sinks != expected_sinks
        || rent.payer_refund_lamports != expected_refunds
        || rent.sink_lamports != expected_sinks_lamports
    {
        return Err(Error::ConservationFailure);
    }
    let mut work_after = *work;
    work_after.mark_page_closed(page.page_ordinal)?;
    let mut state_after = *state;
    state_after.children.lp_pages = state_after.children.lp_pages.checked_sub(1).ok_or(Error::InvalidChildGraph)?;
    state_after.children.terminal_allocations = state_after
        .children
        .terminal_allocations
        .checked_sub(1)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.children.live_lp_positions = state_after
        .children
        .live_lp_positions
        .checked_sub(u32::from(page.entry_count))
        .ok_or(Error::InvalidChildGraph)?;
    state_after.total_shares = state_after
        .total_shares
        .checked_sub(page_shares)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.queued_shares = state_after
        .queued_shares
        .checked_sub(page_queued)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.terminal_claimed_shares = state_after
        .terminal_claimed_shares
        .checked_sub(page_shares)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(if last { 3 } else { 2 })
        .ok_or(Error::ArithmeticOverflow)?;
    if last {
        if state_after.children.unclaimed_lp_positions != 0
            || work_after.closed_page_count()? != work_after.original_page_count
            || work_after.next_allocation_page_ordinal != work_after.original_page_count
        {
            return Err(Error::InvalidChildGraph);
        }
        state_after.children.claim_work = 0;
        state_after.lp_page_head_id = Id::ZERO;
        state_after.lp_page_set_root = Id::ZERO;
        state_after.phase = DealerPhaseV2::Retiring;
        state_after.validate_against_policy(policy)?;
        Ok((state_after, None))
    } else {
        state_after.validate_against_policy(policy)?;
        Ok((state_after, Some(work_after)))
    }
}

const _: () = assert!(DEALER_PAGE_BITMAP_BYTES_V1 == 512);
const _: () = assert!(DEALER_TERMINAL_ALLOCATION_BYTES_V1 == 748);
const _: () = assert!(DEALER_CLAIM_WORK_BYTES_V1 == 1_140);
const _: () = assert!(DEALER_CLAIM_WORK_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
