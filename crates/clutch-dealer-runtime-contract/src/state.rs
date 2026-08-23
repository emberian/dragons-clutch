// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    validate_padding_i64, DealerPolicyV1, Error, FixedCodec, Id, Result, RootRentOwnerV1,
    DEALER_STATE_CONTENT_DOMAIN_V1, MAX_ATOMS, MAX_LP_PAGES, MAX_OUTCOMES, ROOT_RENT_OWNER_BYTES,
};

/// Local semantic-body magic; this is not a global account discriminator.
pub const DEALER_STATE_MAGIC_V1: [u8; 8] = *b"DCDSTAT1";
/// Exact local semantic-body version.
pub const DEALER_STATE_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `DealerStateV1` body.
pub const DEALER_STATE_BYTES_V1: usize =
    HEADER_BYTES + (11 * 32) + 8 + (6 * 8) + (MAX_OUTCOMES * 8) + 44 + ROOT_RENT_OWNER_BYTES;

/// Covered-dealer runtime phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerPhaseV1 {
    /// LP unit-basket funding may be assembled.
    Funding = 0,
    /// Fully funded two-sided aggregate trading.
    Trading = 1,
    /// Only componentwise exposure-reducing trades.
    UnwindOnly = 2,
    /// Authenticated payout has made LP terminal claims final.
    Resolved = 3,
    /// Activation failed or became stale.
    Cancelled = 4,
    /// Economic state is terminal and counted children are closing.
    Retiring = 5,
    /// Every counted child is closed; only permanent evidence remains.
    Closed = 6,
}

impl DealerPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Funding),
            1 => Ok(Self::Trading),
            2 => Ok(Self::UnwindOnly),
            3 => Ok(Self::Resolved),
            4 => Ok(Self::Cancelled),
            5 => Ok(Self::Retiring),
            6 => Ok(Self::Closed),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Exact disposition of the separately funded sponsor capital.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SponsorCapitalDispositionV1 {
    /// Still refundable because activation has not happened.
    Refundable = 0,
    /// Irrevocably donated to the LP pool on activation.
    Donated = 1,
    /// Returned after cancellation without activation.
    Refunded = 2,
}

impl SponsorCapitalDispositionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Refundable),
            1 => Ok(Self::Donated),
            2 => Ok(Self::Refunded),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Exhaustive disjoint outstanding child counts owned by one DealerState.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerChildCountsV1 {
    /// Facility Position account; canonically zero or one.
    pub facility_positions: u32,
    /// Facility Position Replay companion; canonically zero or one.
    pub facility_replays: u32,
    /// LP ownership pages.
    pub lp_pages: u32,
    /// Exact live LP entries nested in the page set.
    pub live_lp_positions: u32,
    /// Exact terminal LP entries not yet claimed.
    pub unclaimed_lp_positions: u32,
    /// Active Epoch binding; canonically zero or one.
    pub epoch_bindings: u32,
    /// One-generation leases; canonically zero or one outstanding.
    pub leases: u32,
    /// Two-phase settlement pots; canonically zero or one outstanding.
    pub settlement_pots: u32,
    /// Facility fee budgets; canonically zero or one.
    pub fee_budgets: u32,
    /// Facility liveness budgets; canonically zero or one.
    pub liveness_budgets: u32,
    /// Open resolution or claim work; canonically zero or one.
    pub resolution_claim_work: u32,
}

impl DealerChildCountsV1 {
    /// Validate the exhaustive fixed child classes and their cardinality caps.
    pub fn validate(&self) -> Result<()> {
        let lp_capacity = self
            .lp_pages
            .checked_mul(crate::LP_ENTRIES_PER_PAGE as u32)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.facility_positions > 1
            || self.facility_replays > 1
            || self.lp_pages > MAX_LP_PAGES
            || self.live_lp_positions > lp_capacity
            || self.unclaimed_lp_positions > self.live_lp_positions
            || self.epoch_bindings > 1
            || self.leases > 1
            || self.settlement_pots > 1
            || self.fee_budgets > 1
            || self.liveness_budgets > 1
            || self.resolution_claim_work > 1
        {
            return Err(Error::InvalidChildGraph);
        }
        Ok(())
    }

    /// Return the exact outstanding count for a named child class.
    pub const fn count(self, kind: DealerChildKindV1) -> u32 {
        match kind {
            DealerChildKindV1::FacilityPosition => self.facility_positions,
            DealerChildKindV1::FacilityReplay => self.facility_replays,
            DealerChildKindV1::LpPage => self.lp_pages,
            DealerChildKindV1::LpPosition => self.live_lp_positions,
            DealerChildKindV1::UnclaimedLpPosition => self.unclaimed_lp_positions,
            DealerChildKindV1::EpochBinding => self.epoch_bindings,
            DealerChildKindV1::Lease => self.leases,
            DealerChildKindV1::SettlementPot => self.settlement_pots,
            DealerChildKindV1::FeeBudget => self.fee_budgets,
            DealerChildKindV1::LivenessBudget => self.liveness_budgets,
            DealerChildKindV1::ResolutionClaimWork => self.resolution_claim_work,
        }
    }

    fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.u32(self.facility_positions);
        writer.u32(self.facility_replays);
        writer.u32(self.lp_pages);
        writer.u32(self.live_lp_positions);
        writer.u32(self.unclaimed_lp_positions);
        writer.u32(self.epoch_bindings);
        writer.u32(self.leases);
        writer.u32(self.settlement_pots);
        writer.u32(self.fee_budgets);
        writer.u32(self.liveness_budgets);
        writer.u32(self.resolution_claim_work);
    }

    fn decode_body(reader: &mut Reader<'_>) -> Self {
        Self {
            facility_positions: reader.u32(),
            facility_replays: reader.u32(),
            lp_pages: reader.u32(),
            live_lp_positions: reader.u32(),
            unclaimed_lp_positions: reader.u32(),
            epoch_bindings: reader.u32(),
            leases: reader.u32(),
            settlement_pots: reader.u32(),
            fee_budgets: reader.u32(),
            liveness_budgets: reader.u32(),
            resolution_claim_work: reader.u32(),
        }
    }
}

/// Exhaustive child classes counted by `DealerStateV1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerChildKindV1 {
    /// The external Facility Position account.
    FacilityPosition = 0,
    /// The Facility Position Replay companion.
    FacilityReplay = 1,
    /// One LP page.
    LpPage = 2,
    /// One live LP entry nested in a page.
    LpPosition = 3,
    /// One terminal LP entry whose claim is not delivered.
    UnclaimedLpPosition = 4,
    /// One active Epoch binding.
    EpochBinding = 5,
    /// One one-generation lease.
    Lease = 6,
    /// One three-stage settlement pot.
    SettlementPot = 7,
    /// One fee budget.
    FeeBudget = 8,
    /// One liveness budget.
    LivenessBudget = 9,
    /// One open resolution or terminal-claim work item.
    ResolutionClaimWork = 10,
}

/// Canonical counted-child edge embedded in every DealerState-owned child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedDealerChildV1 {
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Exhaustive child class used by the parent's counter.
    pub kind: DealerChildKindV1,
    /// Parent generation at which this child was admitted.
    pub counted_generation: u64,
}

/// Allocation-free fold used by an adapter while authenticating the complete
/// child set. Account uniqueness and PDA ownership remain adapter checks; this
/// fold owns the exact exhaustive class totals and parent binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerChildGraphFoldV1 {
    facility_id: Id,
    generation: u64,
    observed: DealerChildCountsV1,
}

impl DealerChildGraphFoldV1 {
    /// Start an empty fold for one live facility.
    pub fn new(facility_id: Id, generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Ok(Self {
            facility_id,
            generation,
            observed: DealerChildCountsV1::default(),
        })
    }

    /// Observe one adapter-authenticated counted child.
    pub fn observe(&mut self, child: CountedDealerChildV1) -> Result<()> {
        if child.facility_id != self.facility_id {
            return Err(Error::MismatchedBinding);
        }
        let must_match_generation = matches!(
            child.kind,
            DealerChildKindV1::EpochBinding
                | DealerChildKindV1::Lease
                | DealerChildKindV1::SettlementPot
        );
        if child.counted_generation > self.generation
            || (must_match_generation && child.counted_generation != self.generation)
        {
            return Err(Error::MismatchedBinding);
        }
        let mut next = self.observed;
        let slot = match child.kind {
            DealerChildKindV1::FacilityPosition => &mut next.facility_positions,
            DealerChildKindV1::FacilityReplay => &mut next.facility_replays,
            DealerChildKindV1::LpPage => &mut next.lp_pages,
            DealerChildKindV1::LpPosition => &mut next.live_lp_positions,
            DealerChildKindV1::UnclaimedLpPosition => &mut next.unclaimed_lp_positions,
            DealerChildKindV1::EpochBinding => &mut next.epoch_bindings,
            DealerChildKindV1::Lease => &mut next.leases,
            DealerChildKindV1::SettlementPot => &mut next.settlement_pots,
            DealerChildKindV1::FeeBudget => &mut next.fee_budgets,
            DealerChildKindV1::LivenessBudget => &mut next.liveness_budgets,
            DealerChildKindV1::ResolutionClaimWork => &mut next.resolution_claim_work,
        };
        *slot = slot.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        self.observed = next;
        Ok(())
    }

    /// Require exact equality with the root's authoritative child counters.
    pub fn finish(self, state: &DealerStateV1) -> Result<()> {
        state.validate()?;
        if state.facility_id != self.facility_id
            || state.generation != self.generation
            || state.children != self.observed
        {
            return Err(Error::InvalidChildGraph);
        }
        Ok(())
    }

    /// Current exact observed counts.
    pub const fn observed(&self) -> DealerChildCountsV1 {
        self.observed
    }
}

/// Exact semantic state of one dealer, excluding all asset balances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerStateV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable semantic identity of this facility.
    pub facility_id: Id,
    /// Current semantic identity of the external Facility Position.
    ///
    /// The Facility Position is the sole long-lived cash and Egg owner while
    /// idle. During a lease, the authenticated Position/Pot aggregate refines
    /// this identity without adding balance fields to DealerState.
    pub facility_position_id: Id,
    /// Exact external Facility Position account identity.
    pub facility_position_account_id: Id,
    /// Exact Replay companion account identity.
    pub facility_replay_account_id: Id,
    /// Sponsor identity.
    pub sponsor: Id,
    /// Exact sponsor-capital refund recipient before activation.
    pub sponsor_refund_recipient: Id,
    /// First LP page account identity, or zero when `children.lp_pages == 0`.
    pub lp_page_head_id: Id,
    /// Canonical root of the exact ordered LP-page set, or zero when empty.
    pub lp_page_set_root: Id,
    /// Exact active Epoch identity, or zero when no Epoch is bound.
    pub active_epoch_id: Id,
    /// Exact active Lease account identity, or zero when no Lease exists.
    pub active_lease_id: Id,
    /// Current lifecycle phase.
    pub phase: DealerPhaseV1,
    /// Exact sponsor-capital disposition.
    pub sponsor_capital_disposition: SponsorCapitalDispositionV1,
    /// Active native outcome width, copied from the bound policy.
    pub outcome_count: u8,
    /// Monotone facility economic/lifecycle generation.
    pub generation: u64,
    /// Independent monotone child/account graph sequence.
    pub child_sequence: u64,
    /// Exact outstanding LP shares summarized from the page set.
    pub total_shares: u64,
    /// Exact irrevocably queued shares summarized from the page set.
    pub queued_shares: u64,
    /// Exact shares whose terminal page claim has been delivered.
    pub terminal_claimed_shares: u64,
    /// Present sponsor capital fixed at initialization.
    pub sponsor_capital_atoms: u64,
    /// Signed cumulative net Eggs sold; positive is sold, negative is bought.
    pub net_sold: [i64; MAX_OUTCOMES],
    /// Exhaustive outstanding child graph.
    pub children: DealerChildCountsV1,
    /// Exact root shrink-to-tombstone rent owner.
    pub rent: RootRentOwnerV1,
}

impl DealerStateV1 {
    /// Validate local state, canonical padding, sponsor facts, and child graph.
    pub fn validate(&self) -> Result<()> {
        let required = [
            self.policy_id,
            self.facility_id,
            self.facility_position_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.sponsor,
            self.sponsor_refund_recipient,
        ];
        let mut index = 0usize;
        while index < required.len() {
            required[index].validate_live()?;
            index += 1;
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::InvalidParameter);
        }
        validate_padding_i64(self.outcome_count, &self.net_sold)?;
        index = 0;
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

        if (self.children.epoch_bindings == 0) != self.active_epoch_id.is_zero()
            || (self.children.leases == 0) != self.active_lease_id.is_zero()
            || self.children.leases != self.children.settlement_pots
            || self.children.leases > self.children.epoch_bindings
        {
            return Err(Error::InvalidChildGraph);
        }

        if self.children.lp_pages == 0 {
            if !self.lp_page_head_id.is_zero() || !self.lp_page_set_root.is_zero() {
                return Err(Error::InvalidChildGraph);
            }
            if self.total_shares != 0
                || self.queued_shares != 0
                || self.terminal_claimed_shares != 0
            {
                return Err(Error::InvalidChildGraph);
            }
        } else {
            self.lp_page_head_id.validate_live()?;
            self.lp_page_set_root.validate_live()?;
        }
        if (self.total_shares == 0) != (self.children.live_lp_positions == 0) {
            return Err(Error::InvalidChildGraph);
        }

        let inventory_is_zero = self.net_sold == [0; MAX_OUTCOMES];
        match self.phase {
            DealerPhaseV1::Funding => {
                if !inventory_is_zero
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV1::Trading | DealerPhaseV1::UnwindOnly => {
                if self.total_shares == 0
                    || self.terminal_claimed_shares != 0
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV1::Resolved => {
                if self.total_shares == 0
                    || self.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV1::Cancelled => {
                if !inventory_is_zero
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.epoch_bindings != 0
                    || self.sponsor_capital_disposition == SponsorCapitalDispositionV1::Donated
                    || self.children.facility_positions != 1
                    || self.children.facility_replays != 1
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV1::Retiring => {
                if self.total_shares != 0
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.children.live_lp_positions != 0
                    || self.children.unclaimed_lp_positions != 0
                    || self.children.epoch_bindings != 0
                    || self.children.leases != 0
                    || self.children.settlement_pots != 0
                    || self.children.resolution_claim_work != 0
                    || self.sponsor_capital_disposition == SponsorCapitalDispositionV1::Refundable
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerPhaseV1::Closed => {
                if self.children != DealerChildCountsV1::default()
                    || self.total_shares != 0
                    || self.queued_shares != 0
                    || self.terminal_claimed_shares != 0
                    || self.sponsor_capital_disposition == SponsorCapitalDispositionV1::Refundable
                {
                    return Err(Error::InvalidPhase);
                }
            }
        }
        Ok(())
    }

    /// Join local state to the exact immutable policy content identity.
    pub fn validate_against_policy(&self, policy: &DealerPolicyV1) -> Result<()> {
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
        if self.phase == DealerPhaseV1::Trading
            && policy
                .shutdown_queue_threshold_met_validated(self.queued_shares, self.total_shares)?
        {
            return Err(Error::InvalidPhase);
        }
        if matches!(
            self.phase,
            DealerPhaseV1::Trading | DealerPhaseV1::UnwindOnly | DealerPhaseV1::Resolved
        ) && self.total_shares < policy.minimum_lp_shares
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical mutable-state content identity.
    pub fn state_content_id(&self) -> Result<Id> {
        self.content_id(DEALER_STATE_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerStateV1 {
    const ENCODED_LEN: usize = DEALER_STATE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_STATE_MAGIC_V1, DEALER_STATE_VERSION_V1);
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.facility_position_id);
        writer.id(self.facility_position_account_id);
        writer.id(self.facility_replay_account_id);
        writer.id(self.sponsor);
        writer.id(self.sponsor_refund_recipient);
        writer.id(self.lp_page_head_id);
        writer.id(self.lp_page_set_root);
        writer.id(self.active_epoch_id);
        writer.id(self.active_lease_id);
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
        reader.header(&DEALER_STATE_MAGIC_V1, DEALER_STATE_VERSION_V1)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let facility_position_id = reader.id();
        let facility_position_account_id = reader.id();
        let facility_replay_account_id = reader.id();
        let sponsor = reader.id();
        let sponsor_refund_recipient = reader.id();
        let lp_page_head_id = reader.id();
        let lp_page_set_root = reader.id();
        let active_epoch_id = reader.id();
        let active_lease_id = reader.id();
        let phase = DealerPhaseV1::decode(reader.u8())?;
        let sponsor_capital_disposition = SponsorCapitalDispositionV1::decode(reader.u8())?;
        let outcome_count = reader.u8();
        reader.reserved(5)?;
        let generation = reader.u64();
        let child_sequence = reader.u64();
        let total_shares = reader.u64();
        let queued_shares = reader.u64();
        let terminal_claimed_shares = reader.u64();
        let sponsor_capital_atoms = reader.u64();
        let mut net_sold = [0i64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            net_sold[index] = reader.i64();
            index += 1;
        }
        let value = Self {
            policy_id,
            facility_id,
            facility_position_id,
            facility_position_account_id,
            facility_replay_account_id,
            sponsor,
            sponsor_refund_recipient,
            lp_page_head_id,
            lp_page_set_root,
            active_epoch_id,
            active_lease_id,
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
            children: DealerChildCountsV1::decode_body(&mut reader),
            rent: RootRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_STATE_BYTES_V1 == 680);
const _: () = assert!(DEALER_STATE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
