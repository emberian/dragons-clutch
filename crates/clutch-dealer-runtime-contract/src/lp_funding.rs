// SPDX-License-Identifier: AGPL-3.0-or-later

use sha2::{Digest, Sha256};

use crate::{
    DealerPolicyV1, DealerStateV1, Error, Id, LpPageV1, Result, MAX_LP_PAGES, NO_NEXT_LP_PAGE,
};

/// Exact result of folding the complete, ordered, sealed LP page set.
///
/// This is an ephemeral authenticated fact, not a second persisted owner. The
/// adapter must supply every page under its canonical PDA and program owner in
/// ordinal order. The fold then owns cross-page order, chain closure, exact
/// shares, queue totals, live-entry count, and the state root equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpFundingFactsV1 {
    /// Exact Dealer policy identity shared by every page.
    pub policy_id: Id,
    /// Exact facility identity shared by every page.
    pub facility_id: Id,
    /// Parent generation shared by every funding page.
    pub counted_generation: u64,
    /// First canonical LP page account key.
    pub first_page_account_id: Id,
    /// Canonical root over page account keys and semantic content identities.
    pub page_set_root: Id,
    /// Exact number of observed pages.
    pub page_count: u32,
    /// Exact number of live entries across all pages.
    pub live_lp_positions: u32,
    /// Exact checked sum of live LP shares.
    pub total_shares: u64,
    /// Exact checked sum of irrevocably queued shares.
    pub queued_shares: u64,
}

impl DealerLpFundingFactsV1 {
    /// Require exact equality to the authoritative root summaries.
    pub fn validate_against_state(&self, state: &DealerStateV1) -> Result<()> {
        state.validate()?;
        if self.policy_id != state.policy_id
            || self.facility_id != state.facility_id
            || self.counted_generation > state.generation
            || self.first_page_account_id != state.lp_page_head_id
            || self.page_set_root != state.lp_page_set_root
            || self.page_count != state.children.lp_pages
            || self.live_lp_positions != state.children.live_lp_positions
            || self.total_shares != state.total_shares
            || self.queued_shares != state.queued_shares
        {
            return Err(Error::InvalidChildGraph);
        }
        Ok(())
    }
}

/// Allocation-free canonical fold over one sealed activation page set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpFundingFoldV1 {
    policy_id: Id,
    facility_id: Id,
    counted_generation: u64,
    next_page_ordinal: u32,
    first_page_account_id: Id,
    previous_page_account_id: Id,
    previous_owner: Id,
    rolling_root: Id,
    live_lp_positions: u32,
    total_shares: u64,
    queued_shares: u64,
    previous_next_page_ordinal: u32,
}

impl DealerLpFundingFoldV1 {
    /// Start an empty fold for one exact Policy/facility/generation tuple.
    pub fn new(policy: &DealerPolicyV1, facility_id: Id, counted_generation: u64) -> Result<Self> {
        policy.validate()?;
        facility_id.validate_live()?;
        let policy_id = policy.policy_id()?;
        let rolling_root = digest_parts(
            crate::DEALER_LP_PAGE_SET_INIT_DOMAIN_V1,
            &[
                &policy_id.bytes(),
                &facility_id.bytes(),
                &counted_generation.to_le_bytes(),
            ],
        );
        Ok(Self {
            policy_id,
            facility_id,
            counted_generation,
            next_page_ordinal: 0,
            first_page_account_id: Id::ZERO,
            previous_page_account_id: Id::ZERO,
            previous_owner: Id::ZERO,
            rolling_root,
            live_lp_positions: 0,
            total_shares: 0,
            queued_shares: 0,
            previous_next_page_ordinal: NO_NEXT_LP_PAGE,
        })
    }

    /// Observe the next adapter-authenticated canonical page.
    pub fn observe(
        &mut self,
        page_account_id: Id,
        page: &LpPageV1,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        page_account_id.validate_live()?;
        page.validate_against_policy(policy)?;
        if policy.policy_id()? != self.policy_id
            || page.policy_id != self.policy_id
            || page.facility_id != self.facility_id
            || page.counted_generation != self.counted_generation
            || page.page_ordinal != self.next_page_ordinal
            || page.entry_count == 0
            || !page.sealed
            || page.terminal_allocated
            || (self.next_page_ordinal != 0 && self.previous_next_page_ordinal != page.page_ordinal)
            || page_account_id == self.previous_page_account_id
        {
            return Err(Error::InvalidLpPage);
        }
        let count = usize::from(page.entry_count);
        let mut index = 0usize;
        let mut previous_owner = self.previous_owner;
        let mut next_live = self.live_lp_positions;
        let mut next_total = self.total_shares;
        let mut next_queued = self.queued_shares;
        while index < count {
            let entry = page.entries[index];
            if !previous_owner.is_zero() && previous_owner >= entry.owner {
                return Err(Error::InvalidLpPage);
            }
            previous_owner = entry.owner;
            next_live = next_live.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            next_total = next_total
                .checked_add(entry.shares)
                .ok_or(Error::ArithmeticOverflow)?;
            next_queued = next_queued
                .checked_add(entry.queued_shares)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        if next_total > policy.maximum_lp_shares || next_queued > next_total {
            return Err(Error::InvalidParameter);
        }
        let page_id = page.page_content_id()?;
        let next_root = digest_parts(
            crate::DEALER_LP_PAGE_SET_STEP_DOMAIN_V1,
            &[
                &self.rolling_root.bytes(),
                &page_account_id.bytes(),
                &page.page_ordinal.to_le_bytes(),
                &page_id.bytes(),
            ],
        );
        if self.next_page_ordinal == 0 {
            self.first_page_account_id = page_account_id;
        }
        self.previous_page_account_id = page_account_id;
        self.previous_owner = previous_owner;
        self.previous_next_page_ordinal = page.next_page_ordinal;
        self.rolling_root = next_root;
        self.live_lp_positions = next_live;
        self.total_shares = next_total;
        self.queued_shares = next_queued;
        self.next_page_ordinal = self
            .next_page_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.next_page_ordinal > MAX_LP_PAGES {
            return Err(Error::InvalidLpPage);
        }
        Ok(())
    }

    /// Close the chain and require exact equality to the State summaries.
    pub fn finish(self, state: &DealerStateV1) -> Result<DealerLpFundingFactsV1> {
        if self.next_page_ordinal == 0
            || self.previous_next_page_ordinal != NO_NEXT_LP_PAGE
            || self.total_shares == 0
        {
            return Err(Error::InvalidLpPage);
        }
        let page_set_root = digest_parts(
            crate::DEALER_LP_PAGE_SET_FINAL_DOMAIN_V1,
            &[
                &self.rolling_root.bytes(),
                &self.next_page_ordinal.to_le_bytes(),
                &self.live_lp_positions.to_le_bytes(),
                &self.total_shares.to_le_bytes(),
                &self.queued_shares.to_le_bytes(),
            ],
        );
        let facts = DealerLpFundingFactsV1 {
            policy_id: self.policy_id,
            facility_id: self.facility_id,
            counted_generation: self.counted_generation,
            first_page_account_id: self.first_page_account_id,
            page_set_root,
            page_count: self.next_page_ordinal,
            live_lp_positions: self.live_lp_positions,
            total_shares: self.total_shares,
            queued_shares: self.queued_shares,
        };
        facts.validate_against_state(state)?;
        Ok(facts)
    }
}

fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    let mut index = 0usize;
    while index < parts.len() {
        hasher.update(parts[index]);
        index += 1;
    }
    Id::from_bytes(hasher.finalize().into())
}
