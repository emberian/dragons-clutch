// SPDX-License-Identifier: AGPL-3.0-or-later

//! Authoritative Dealer root successor.
//!
//! V1 remains decodable evidence, but its fee-budget and liveness-budget child
//! classes are not authoritative for a funded facility. V2 counts one typed
//! funded-dependency child instead. External liveness accounts and owner-netted
//! fee records remain counted and retired by their own semantic owners.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    validate_padding_i64, DealerPolicyV1, Error, FixedCodec, Id, Result,
    RootRentOwnerV1, SponsorCapitalDispositionV1, DEALER_STATE_CONTENT_DOMAIN_V2, MAX_ATOMS,
    MAX_LP_PAGES, MAX_OUTCOMES, ROOT_RENT_OWNER_BYTES,
};

/// Exhaustive authoritative V2 facility phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerPhaseV2 {
    /// LP funding may be assembled.
    Funding = 0,
    /// Fully funded two-sided trading.
    Trading = 1,
    /// Only exposure-reducing trading.
    UnwindOnly = 2,
    /// Authenticated payout is being folded into page allocations.
    Resolving = 3,
    /// Every page allocation is fixed; permissionless claims are live.
    Resolved = 4,
    /// Activation failed or became stale.
    Cancelled = 5,
    /// Economic state is terminal and children are closing.
    Retiring = 6,
    /// Every counted child is closed.
    Closed = 7,
}

impl DealerPhaseV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Funding),
            1 => Ok(Self::Trading),
            2 => Ok(Self::UnwindOnly),
            3 => Ok(Self::Resolving),
            4 => Ok(Self::Resolved),
            5 => Ok(Self::Cancelled),
            6 => Ok(Self::Retiring),
            7 => Ok(Self::Closed),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Local semantic-body magic; this is not a global account discriminator.
pub const DEALER_STATE_MAGIC_V2: [u8; 8] = *b"DCDSTAT2";
/// Exact local semantic-body version.
pub const DEALER_STATE_VERSION_V2: u16 = 2;
/// Exact bytes in one canonical `DealerStateV2` body.
pub const DEALER_STATE_BYTES_V2: usize =
    HEADER_BYTES + (16 * 32) + 8 + (6 * 8) + (MAX_OUTCOMES * 8) + 48 + ROOT_RENT_OWNER_BYTES;

/// Exhaustive disjoint children owned by the authoritative V2 root.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerChildCountsV2 {
    /// Facility Position account; zero or one.
    pub facility_positions: u32,
    /// Facility Replay companion; zero or one.
    pub facility_replays: u32,
    /// LP ownership pages.
    pub lp_pages: u32,
    /// Live LP entries nested in the page set.
    pub live_lp_positions: u32,
    /// Owner-scoped mutable exit tickets.
    pub exit_tickets: u32,
    /// Terminal LP entries whose claims remain undelivered.
    pub unclaimed_lp_positions: u32,
    /// The exact rent-owned funded-dependency child; zero or one.
    pub funded_dependencies: u32,
    /// Active Epoch binding; zero or one.
    pub epoch_bindings: u32,
    /// One-generation V2 lease; zero or one.
    pub leases: u32,
    /// Three-stage V2 pot; zero or one.
    pub settlement_pots: u32,
    /// Terminal allocations, exactly one per still-live LP page after resolution.
    pub terminal_allocations: u32,
    /// Singleton terminal claim/page-closure work owner; zero or one.
    pub claim_work: u32,
}

impl DealerChildCountsV2 {
    /// Validate the exhaustive V2 classes and fixed cardinality caps.
    pub fn validate(&self) -> Result<()> {
        let lp_capacity = self
            .lp_pages
            .checked_mul(crate::LP_ENTRIES_PER_PAGE as u32)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.facility_positions > 1
            || self.facility_replays > 1
            || self.lp_pages > MAX_LP_PAGES
            || self.live_lp_positions > lp_capacity
            || self.exit_tickets > self.live_lp_positions
            || self.unclaimed_lp_positions > self.live_lp_positions
            || self.funded_dependencies > 1
            || self.epoch_bindings > 1
            || self.leases > 1
            || self.settlement_pots > 1
            || self.terminal_allocations > self.lp_pages
            || self.claim_work > 1
        {
            return Err(Error::InvalidChildGraph);
        }
        Ok(())
    }

    /// Exact outstanding count for a V2 child class.
    pub const fn count(self, kind: DealerChildKindV2) -> u32 {
        match kind {
            DealerChildKindV2::FacilityPosition => self.facility_positions,
            DealerChildKindV2::FacilityReplay => self.facility_replays,
            DealerChildKindV2::LpPage => self.lp_pages,
            DealerChildKindV2::LpPosition => self.live_lp_positions,
            DealerChildKindV2::ExitTicket => self.exit_tickets,
            DealerChildKindV2::UnclaimedLpPosition => self.unclaimed_lp_positions,
            DealerChildKindV2::FundedDependencies => self.funded_dependencies,
            DealerChildKindV2::EpochBinding => self.epoch_bindings,
            DealerChildKindV2::Lease => self.leases,
            DealerChildKindV2::SettlementPot => self.settlement_pots,
            DealerChildKindV2::TerminalAllocation => self.terminal_allocations,
            DealerChildKindV2::ClaimWork => self.claim_work,
        }
    }

    fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.u32(self.facility_positions);
        writer.u32(self.facility_replays);
        writer.u32(self.lp_pages);
        writer.u32(self.live_lp_positions);
        writer.u32(self.exit_tickets);
        writer.u32(self.unclaimed_lp_positions);
        writer.u32(self.funded_dependencies);
        writer.u32(self.epoch_bindings);
        writer.u32(self.leases);
        writer.u32(self.settlement_pots);
        writer.u32(self.terminal_allocations);
        writer.u32(self.claim_work);
    }

    fn decode_body(reader: &mut Reader<'_>) -> Self {
        Self {
            facility_positions: reader.u32(),
            facility_replays: reader.u32(),
            lp_pages: reader.u32(),
            live_lp_positions: reader.u32(),
            exit_tickets: reader.u32(),
            unclaimed_lp_positions: reader.u32(),
            funded_dependencies: reader.u32(),
            epoch_bindings: reader.u32(),
            leases: reader.u32(),
            settlement_pots: reader.u32(),
            terminal_allocations: reader.u32(),
            claim_work: reader.u32(),
        }
    }
}

/// Exhaustive child classes counted by `DealerStateV2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerChildKindV2 {
    /// Facility Position.
    FacilityPosition = 0,
    /// Facility Replay companion.
    FacilityReplay = 1,
    /// LP page.
    LpPage = 2,
    /// Live nested LP entry.
    LpPosition = 3,
    /// Owner-scoped mutable exit ticket.
    ExitTicket = 11,
    /// Unclaimed terminal LP entry.
    UnclaimedLpPosition = 4,
    /// Rent-owned funded-dependency child.
    FundedDependencies = 5,
    /// Active Epoch binding.
    EpochBinding = 6,
    /// V2 lease.
    Lease = 7,
    /// V2 settlement pot.
    SettlementPot = 8,
    /// One page-scoped immutable terminal allocation.
    TerminalAllocation = 9,
    /// Singleton terminal claim/page-closure work owner.
    ClaimWork = 10,
}

/// Canonical counted-child edge embedded in every V2 child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedDealerChildV2 {
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable Position-authority binding identity authenticated at initialization.
    pub facility_position_binding_id: Id,
    /// Exhaustive V2 class.
    pub kind: DealerChildKindV2,
    /// Parent generation at admission.
    pub counted_generation: u64,
}

/// Allocation-free exhaustive V2 child fold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerChildGraphFoldV2 {
    facility_id: Id,
    facility_position_binding_id: Id,
    generation: u64,
    observed: DealerChildCountsV2,
}

impl DealerChildGraphFoldV2 {
    /// Start an empty fold for one authoritative root.
    pub fn new(
        facility_id: Id,
        facility_position_binding_id: Id,
        generation: u64,
    ) -> Result<Self> {
        facility_id.validate_live()?;
        facility_position_binding_id.validate_live()?;
        Ok(Self {
            facility_id,
            facility_position_binding_id,
            generation,
            observed: DealerChildCountsV2::default(),
        })
    }

    /// Observe one adapter-authenticated V2 child.
    pub fn observe(&mut self, child: CountedDealerChildV2) -> Result<()> {
        if child.facility_id != self.facility_id
            || child.facility_position_binding_id != self.facility_position_binding_id
        {
            return Err(Error::MismatchedBinding);
        }
        let generation_exact = matches!(
            child.kind,
            DealerChildKindV2::EpochBinding
                | DealerChildKindV2::ExitTicket
                | DealerChildKindV2::Lease
                | DealerChildKindV2::SettlementPot
                | DealerChildKindV2::TerminalAllocation
                | DealerChildKindV2::ClaimWork
        );
        if child.counted_generation > self.generation
            || (generation_exact && child.counted_generation != self.generation)
        {
            return Err(Error::MismatchedBinding);
        }
        let mut next = self.observed;
        let slot = match child.kind {
            DealerChildKindV2::FacilityPosition => &mut next.facility_positions,
            DealerChildKindV2::FacilityReplay => &mut next.facility_replays,
            DealerChildKindV2::LpPage => &mut next.lp_pages,
            DealerChildKindV2::LpPosition => &mut next.live_lp_positions,
            DealerChildKindV2::ExitTicket => &mut next.exit_tickets,
            DealerChildKindV2::UnclaimedLpPosition => &mut next.unclaimed_lp_positions,
            DealerChildKindV2::FundedDependencies => &mut next.funded_dependencies,
            DealerChildKindV2::EpochBinding => &mut next.epoch_bindings,
            DealerChildKindV2::Lease => &mut next.leases,
            DealerChildKindV2::SettlementPot => &mut next.settlement_pots,
            DealerChildKindV2::TerminalAllocation => &mut next.terminal_allocations,
            DealerChildKindV2::ClaimWork => &mut next.claim_work,
        };
        *slot = slot.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        self.observed = next;
        Ok(())
    }

    /// Require equality with the authoritative counters.
    pub fn finish(self, state: &DealerStateV2) -> Result<()> {
        state.validate()?;
        if state.facility_id != self.facility_id
            || state.facility_position_binding_id != self.facility_position_binding_id
            || state.generation != self.generation
            || state.children != self.observed
        {
            return Err(Error::InvalidChildGraph);
        }
        Ok(())
    }
}

/// Exact V2 semantic state, excluding every asset and external-runtime balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerStateV2 {
    /// Dealer policy content identity.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable purpose binding for the canonical shared Position V3 account.
    pub facility_position_binding_id: Id,
    /// Current Facility Position semantic identity.
    pub facility_position_id: Id,
    /// Facility Position account identity.
    pub facility_position_account_id: Id,
    /// Replay companion account identity.
    pub facility_replay_account_id: Id,
    /// Sponsor identity.
    pub sponsor: Id,
    /// Sponsor-capital refund recipient.
    pub sponsor_refund_recipient: Id,
    /// LP page head or zero.
    pub lp_page_head_id: Id,
    /// Exact current tail content identity; its prefix transitively commits the page set.
    pub lp_page_set_root: Id,
    /// Greatest owner admitted to the globally sorted LP page chain, or zero.
    pub last_lp_owner: Id,
    /// Active Epoch identity or zero.
    pub active_epoch_id: Id,
    /// Active counted Dealer Epoch-binding account or zero.
    pub active_epoch_binding_account_id: Id,
    /// Active V2 Lease account or zero.
    pub active_lease_id: Id,
    /// Immutable semantic identity of the admitted dependency child.
    ///
    /// This remains live after the child account closes so the tombstone can
    /// preserve the exact funded provenance without keeping refundable rent.
    pub funded_dependencies_id: Id,
    /// Live dependency child account, or zero after its counted close.
    pub funded_dependencies_account_id: Id,
    /// Current lifecycle phase.
    pub phase: DealerPhaseV2,
    /// Sponsor-capital disposition.
    pub sponsor_capital_disposition: SponsorCapitalDispositionV1,
    /// Active outcome width.
    pub outcome_count: u8,
    /// Monotone economic generation.
    pub generation: u64,
    /// Monotone child-graph sequence.
    pub child_sequence: u64,
    /// Outstanding LP shares.
    pub total_shares: u64,
    /// Irrevocably queued shares.
    pub queued_shares: u64,
    /// Delivered terminal shares.
    pub terminal_claimed_shares: u64,
    /// Present sponsor collateral fixed at initialization.
    pub sponsor_capital_atoms: u64,
    /// Signed cumulative net Eggs sold.
    pub net_sold: [i64; MAX_OUTCOMES],
    /// Exhaustive V2 child graph.
    pub children: DealerChildCountsV2,
    /// Root shrink-to-tombstone rent owner.
    pub rent: RootRentOwnerV1,
}

impl DealerStateV2 {
    /// Validate V2 identity, exact children, and phase partition.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.facility_position_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.sponsor,
            self.sponsor_refund_recipient,
            self.funded_dependencies_id,
        ] {
            identity.validate_live()?;
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::InvalidParameter);
        }
        validate_padding_i64(self.outcome_count, &self.net_sold)?;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            if i128::from(self.net_sold[index]).unsigned_abs() > u128::from(MAX_ATOMS) {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        if self.sponsor_capital_atoms == 0
            || self.sponsor_capital_atoms > MAX_ATOMS
            || self.sponsor == self.facility_id
            || self.facility_position_account_id == self.facility_replay_account_id
            || self.queued_shares > self.total_shares
            || self.terminal_claimed_shares > self.total_shares
        {
            return Err(Error::InvalidParameter);
        }
        self.children.validate()?;
        self.rent.validate()?;
        if (self.children.funded_dependencies == 0)
            != self.funded_dependencies_account_id.is_zero()
            || (self.children.epoch_bindings == 0) != self.active_epoch_id.is_zero()
            || (self.children.epoch_bindings == 0)
                != self.active_epoch_binding_account_id.is_zero()
            || (self.children.leases == 0) != self.active_lease_id.is_zero()
            || self.children.leases != self.children.settlement_pots
            || self.children.leases > self.children.epoch_bindings
        {
            return Err(Error::InvalidChildGraph);
        }
        if self.children.lp_pages == 0 {
            if !self.lp_page_head_id.is_zero()
                || !self.lp_page_set_root.is_zero()
                || self.total_shares != 0
                || self.queued_shares != 0
                || self.terminal_claimed_shares != 0
            {
                return Err(Error::InvalidChildGraph);
            }
        } else {
            self.lp_page_head_id.validate_live()?;
            self.lp_page_set_root.validate_live()?;
        }
        if self.children.live_lp_positions != 0 {
            self.last_lp_owner.validate_live()?;
        }
        if (self.total_shares == 0) != (self.children.live_lp_positions == 0) {
            return Err(Error::InvalidChildGraph);
        }
        if (self.queued_shares == 0) != (self.children.exit_tickets == 0) {
            return Err(Error::InvalidChildGraph);
        }

        let inventory_zero = self.net_sold == [0; MAX_OUTCOMES];
        match self.phase {
            DealerPhaseV2::Funding => {
                if !inventory_zero
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.sponsor_capital_disposition
                        != SponsorCapitalDispositionV1::Refundable
                    || self.children.funded_dependencies != 1
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.terminal_allocations != 0
                    || self.children.claim_work != 0
                    || self.children.exit_tickets != 0
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly => {
                if self.total_shares == 0
                    || self.terminal_claimed_shares != 0
                    || self.children.unclaimed_lp_positions != 0
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
                    || self.children.funded_dependencies != 1
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                    || self.children.terminal_allocations != 0
                    || self.children.claim_work != 0
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Resolving => {
                if self.total_shares == 0
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.children.unclaimed_lp_positions
                        != self.children.live_lp_positions
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
                    || self.children.funded_dependencies != 1
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.terminal_allocations > self.children.lp_pages
                    || self.children.claim_work != 1
                    || self.children.exit_tickets != 0
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Resolved => {
                if self.total_shares == 0
                    || self.queued_shares != 0
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
                    || self.children.funded_dependencies != 1
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.terminal_allocations != self.children.lp_pages
                    || self.children.claim_work != 1
                    || self.children.exit_tickets != 0
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Cancelled => {
                if !inventory_zero
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.children.funded_dependencies != 1
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.terminal_allocations != 0
                    || self.children.claim_work != 0
                    || self.children.exit_tickets != 0
                    || self.sponsor_capital_disposition == SponsorCapitalDispositionV1::Donated
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Retiring => {
                if self.total_shares != 0
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.children.live_lp_positions != 0
                    || self.children.unclaimed_lp_positions != 0
                    || self.children.epoch_bindings != 0
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.terminal_allocations != 0
                    || self.children.claim_work != 0
                    || self.children.exit_tickets != 0
                    || self.sponsor_capital_disposition
                        == SponsorCapitalDispositionV1::Refundable
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV2::Closed => {
                if self.children != DealerChildCountsV2::default()
                    || self.total_shares != 0
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.sponsor_capital_disposition
                        == SponsorCapitalDispositionV1::Refundable
                {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        Ok(())
    }

    /// Join the root to its exact immutable policy.
    pub fn validate_against_policy(&self, policy: &DealerPolicyV1) -> Result<()> {
        self.validate_policy_bindings(policy)?;
        if self.phase == DealerPhaseV2::Trading
            && policy
                .shutdown_queue_threshold_met_validated(self.queued_shares, self.total_shares)?
        {
            return Err(Error::InvalidPhase);
        }
        Ok(())
    }

    /// Validate immutable policy facts while admitting the atomic queue-quorum transition.
    pub(crate) fn validate_policy_bindings(&self, policy: &DealerPolicyV1) -> Result<()> {
        self.validate()?;
        policy.validate()?;
        if self.policy_id != policy.policy_id()?
            || self.outcome_count != policy.outcome_count
            || self.total_shares > policy.maximum_lp_shares
            || self.children.lp_pages > policy.maximum_lp_pages
            || self.sponsor_capital_atoms < policy.minimum_sponsor_capital()?
            || self.rent.neutral_sink != policy.neutral_sink
            || self.sponsor == policy.neutral_sink
            || self.sponsor_refund_recipient == policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        policy.validate_net_sold(&self.net_sold)?;
        if matches!(
            self.phase,
            DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly
        ) && self.total_shares < policy.minimum_lp_shares
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical V2 mutable-state content identity.
    pub fn state_content_id(&self) -> Result<Id> {
        self.content_id(DEALER_STATE_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for DealerStateV2 {
    const ENCODED_LEN: usize = DEALER_STATE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_STATE_MAGIC_V2, DEALER_STATE_VERSION_V2);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.facility_position_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.sponsor,
            self.sponsor_refund_recipient,
            self.lp_page_head_id,
            self.lp_page_set_root,
            self.last_lp_owner,
            self.active_epoch_id,
            self.active_epoch_binding_account_id,
            self.active_lease_id,
            self.funded_dependencies_id,
            self.funded_dependencies_account_id,
        ] {
            writer.id(identity);
        }
        writer.u8(self.phase as u8);
        writer.u8(self.sponsor_capital_disposition as u8);
        writer.u8(self.outcome_count);
        writer.reserved(5);
        writer.u64(self.generation);
        writer.u64(self.child_sequence);
        writer.u64(self.total_shares);
        writer.u64(self.queued_shares);
        writer.u64(self.terminal_claimed_shares);
        writer.u64(self.sponsor_capital_atoms);
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            writer.i64(self.net_sold[index]);
            index += 1;
        }
        self.children.encode_body(&mut writer);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_STATE_MAGIC_V2, DEALER_STATE_VERSION_V2)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let facility_position_binding_id = reader.id();
        let facility_position_id = reader.id();
        let facility_position_account_id = reader.id();
        let facility_replay_account_id = reader.id();
        let sponsor = reader.id();
        let sponsor_refund_recipient = reader.id();
        let lp_page_head_id = reader.id();
        let lp_page_set_root = reader.id();
        let last_lp_owner = reader.id();
        let active_epoch_id = reader.id();
        let active_epoch_binding_account_id = reader.id();
        let active_lease_id = reader.id();
        let funded_dependencies_id = reader.id();
        let funded_dependencies_account_id = reader.id();
        let phase = DealerPhaseV2::decode(reader.u8())?;
        let sponsor_capital_disposition = SponsorCapitalDispositionV1::decode(reader.u8())?;
        let outcome_count = reader.u8();
        reader.reserved(5)?;
        let generation = reader.u64();
        let child_sequence = reader.u64();
        let total_shares = reader.u64();
        let queued_shares = reader.u64();
        let terminal_claimed_shares = reader.u64();
        let sponsor_capital_atoms = reader.u64();
        let mut net_sold = [0; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            net_sold[index] = reader.i64();
            index += 1;
        }
        let value = Self {
            policy_id,
            facility_id,
            facility_position_binding_id,
            facility_position_id,
            facility_position_account_id,
            facility_replay_account_id,
            sponsor,
            sponsor_refund_recipient,
            lp_page_head_id,
            lp_page_set_root,
            last_lp_owner,
            active_epoch_id,
            active_epoch_binding_account_id,
            active_lease_id,
            funded_dependencies_id,
            funded_dependencies_account_id,
            phase,
            sponsor_capital_disposition,
            outcome_count,
            generation,
            child_sequence,
            total_shares,
            queued_shares,
            terminal_claimed_shares,
            sponsor_capital_atoms,
            net_sold,
            children: DealerChildCountsV2::decode_body(&mut reader),
            rent: RootRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_STATE_BYTES_V2 == 844);
const _: () = assert!(DEALER_STATE_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
