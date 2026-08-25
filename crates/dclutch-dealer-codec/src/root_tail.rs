//! Inventory-free mutable tail for one canonical composite Trading root.
//!
//! The tail persists only Dealer-owned facts. Canonical Claims Position
//! quantities and Custody token balances are projected into the semantic
//! interpreter for one transaction and are never encoded here.

use super::{
    fee_due, is_zero, put, put_byte, put_u64, put_u64_array, require_zero, u16_at, u64_array_at,
    u64_at, CandidateView, Error, Identity, Phase, Policy, Result, State, MAX_OUTCOMES,
};
use crate::generated_dealer_trading_profile as generated;

/// Exact mutable Dealer tail width inside a composite Trading root.
pub const ROOT_TAIL_BYTES: usize = generated::ROOT_TAIL_BYTES;

/// Dealer-external authoritative observations used for one transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityObservation<'a> {
    /// Exact release set from the authenticated immutable Trading root header.
    pub release_set_id: Identity,
    /// Runtime-width quantities from the canonical Dealer Claims Position.
    pub inventory: &'a [u64],
    /// Current canonical Dealer TradingPrincipal token balance.
    pub quote_custody: u64,
    /// Current canonical FeeVault token balance.
    pub fee_custody: u64,
    /// Current canonical LivenessVault token balance.
    pub liveness_custody: u64,
    /// Canonical Core terminal winner, present only while Dealer is terminal.
    pub terminal_winner: Option<u8>,
}

/// Dealer-owned state stored after the immutable Trading root header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootTail {
    /// Dealer lifecycle phase.
    pub phase: Phase,
    /// Current immutable Candidate content identity.
    pub active_candidate_id: Identity,
    /// Pending Candidate content identity or all zero.
    pub pending_candidate_id: Identity,
    /// Active Candidate revision.
    pub active_revision: u64,
    /// Pending Candidate revision or zero.
    pub pending_revision: u64,
    /// Optimistic Dealer transition revision.
    pub state_revision: u64,
    /// Cumulative ask-curve quantities for the active Candidate.
    pub buy_used: [u64; MAX_OUTCOMES],
    /// Cumulative bid-curve quantities for the active Candidate.
    pub sell_used: [u64; MAX_OUTCOMES],
    /// Cumulative fragmentation-independent fee base.
    pub fee_base: u64,
    /// Active Candidate prepaid work remaining.
    pub active_work_remaining: u64,
    /// Pending Candidate prepaid work funding or zero.
    pub pending_work_funding: u64,
}

impl RootTail {
    /// Construct the initial Dealer tail for one authenticated active Candidate.
    pub fn initialize(active: CandidateView<'_>) -> Self {
        Self {
            phase: Phase::Open,
            active_candidate_id: active.candidate_id,
            pending_candidate_id: [0; 32],
            active_revision: active.revision,
            pending_revision: 0,
            state_revision: 1,
            buy_used: [0; MAX_OUTCOMES],
            sell_used: [0; MAX_OUTCOMES],
            fee_base: 0,
            active_work_remaining: active.work_funding,
            pending_work_funding: 0,
        }
    }

    /// Hostile-decode one exact canonical mutable Dealer tail.
    pub fn decode(input: &[u8]) -> Result<Self> {
        tail_header(input)?;
        require_zero(input, generated::ROOT_TAIL_RESERVED_OFFSET, 4)?;
        let has_pending = super::bool_at(input, generated::ROOT_TAIL_HAS_PENDING_OFFSET)?;
        let value = Self {
            phase: Phase::decode(super::byte_at(input, generated::ROOT_TAIL_PHASE_OFFSET)?)?,
            active_candidate_id: super::array_at(
                input,
                generated::ROOT_TAIL_ACTIVE_CANDIDATE_ID_OFFSET,
            )?,
            pending_candidate_id: super::array_at(
                input,
                generated::ROOT_TAIL_PENDING_CANDIDATE_ID_OFFSET,
            )?,
            active_revision: u64_at(input, generated::ROOT_TAIL_ACTIVE_REVISION_OFFSET)?,
            pending_revision: u64_at(input, generated::ROOT_TAIL_PENDING_REVISION_OFFSET)?,
            state_revision: u64_at(input, generated::ROOT_TAIL_STATE_REVISION_OFFSET)?,
            buy_used: u64_array_at(input, generated::ROOT_TAIL_BUY_USED_OFFSET)?,
            sell_used: u64_array_at(input, generated::ROOT_TAIL_SELL_USED_OFFSET)?,
            fee_base: u64_at(input, generated::ROOT_TAIL_FEE_BASE_OFFSET)?,
            active_work_remaining: u64_at(
                input,
                generated::ROOT_TAIL_ACTIVE_WORK_REMAINING_OFFSET,
            )?,
            pending_work_funding: u64_at(input, generated::ROOT_TAIL_PENDING_WORK_FUNDING_OFFSET)?,
        };
        if is_zero(&value.active_candidate_id)
            || value.active_revision == 0
            || value.state_revision == 0
            || has_pending != !is_zero(&value.pending_candidate_id)
            || (has_pending && (value.pending_revision == 0 || value.pending_work_funding == 0))
            || (!has_pending && (value.pending_revision != 0 || value.pending_work_funding != 0))
        {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Encode one exact canonical mutable Dealer tail.
    pub fn to_bytes(self) -> Result<[u8; ROOT_TAIL_BYTES]> {
        if is_zero(&self.active_candidate_id)
            || self.active_revision == 0
            || self.state_revision == 0
            || (is_zero(&self.pending_candidate_id)
                && (self.pending_revision != 0 || self.pending_work_funding != 0))
            || (!is_zero(&self.pending_candidate_id)
                && (self.pending_revision == 0 || self.pending_work_funding == 0))
        {
            return Err(Error::NonCanonicalPadding);
        }
        let mut output = [0_u8; ROOT_TAIL_BYTES];
        put(&mut output, 0, &generated::ROOT_TAIL_MAGIC)?;
        put(
            &mut output,
            generated::ROOT_TAIL_VERSION_OFFSET,
            &generated::ROOT_TAIL_ABI_VERSION.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated::ROOT_TAIL_PHASE_OFFSET,
            self.phase.tag(),
        )?;
        put_byte(
            &mut output,
            generated::ROOT_TAIL_HAS_PENDING_OFFSET,
            u8::from(!is_zero(&self.pending_candidate_id)),
        )?;
        put(
            &mut output,
            generated::ROOT_TAIL_ACTIVE_CANDIDATE_ID_OFFSET,
            &self.active_candidate_id,
        )?;
        put(
            &mut output,
            generated::ROOT_TAIL_PENDING_CANDIDATE_ID_OFFSET,
            &self.pending_candidate_id,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_ACTIVE_REVISION_OFFSET,
            self.active_revision,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_PENDING_REVISION_OFFSET,
            self.pending_revision,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_STATE_REVISION_OFFSET,
            self.state_revision,
        )?;
        put_u64_array(
            &mut output,
            generated::ROOT_TAIL_BUY_USED_OFFSET,
            &self.buy_used,
        )?;
        put_u64_array(
            &mut output,
            generated::ROOT_TAIL_SELL_USED_OFFSET,
            &self.sell_used,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_FEE_BASE_OFFSET,
            self.fee_base,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_ACTIVE_WORK_REMAINING_OFFSET,
            self.active_work_remaining,
        )?;
        put_u64(
            &mut output,
            generated::ROOT_TAIL_PENDING_WORK_FUNDING_OFFSET,
            self.pending_work_funding,
        )?;
        Ok(output)
    }

    /// Materialize the previous semantic state from sole-owner observations.
    ///
    /// This value is transaction-local. The adapter must not persist its
    /// inventory or Custody fields after interpreting the transition.
    pub fn materialize(
        self,
        policy: Policy,
        active: CandidateView<'_>,
        pending: Option<CandidateView<'_>>,
        authority: AuthorityObservation<'_>,
    ) -> Result<State> {
        policy.validate()?;
        active.validate_against(policy)?;
        if authority.release_set_id != policy.release_set_id
            || self.active_candidate_id != active.candidate_id
            || self.active_revision != active.revision
        {
            return Err(Error::IdentityMismatch);
        }
        match (is_zero(&self.pending_candidate_id), pending) {
            (true, None) => {}
            (false, Some(candidate)) => {
                candidate.validate_against(policy)?;
                if self.pending_candidate_id != candidate.candidate_id
                    || self.pending_revision != candidate.revision
                    || self.pending_work_funding != candidate.work_funding
                {
                    return Err(Error::IdentityMismatch);
                }
            }
            _ => return Err(Error::IdentityMismatch),
        }

        let count = usize::from(policy.outcome_count);
        if authority.inventory.len() != count {
            return Err(Error::InvalidLength);
        }
        let winner = match (self.phase, authority.terminal_winner) {
            (Phase::Open | Phase::Retired, None) => 0,
            (Phase::Terminal, Some(winner)) if usize::from(winner) < count => winner,
            _ => return Err(Error::InvalidPhase),
        };
        let mut inventory = [0_u64; MAX_OUTCOMES];
        inventory
            .get_mut(..count)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(authority.inventory);
        let mut buy_quote_paid = [0_u64; MAX_OUTCOMES];
        let mut sell_quote_paid = [0_u64; MAX_OUTCOMES];
        for outcome in 0..count {
            buy_quote_paid[outcome] = active.cumulative_quote(
                policy,
                outcome,
                super::Side::TakerBuys,
                self.buy_used[outcome],
            )?;
            sell_quote_paid[outcome] = active.cumulative_quote(
                policy,
                outcome,
                super::Side::TakerSells,
                self.sell_used[outcome],
            )?;
        }
        let state = State {
            phase: self.phase,
            outcome_count: policy.outcome_count,
            winner,
            active_candidate_id: self.active_candidate_id,
            pending_candidate_id: self.pending_candidate_id,
            release_set_id: authority.release_set_id,
            active_revision: self.active_revision,
            pending_revision: self.pending_revision,
            state_revision: self.state_revision,
            inventory,
            buy_used: self.buy_used,
            sell_used: self.sell_used,
            buy_quote_paid,
            sell_quote_paid,
            fee_base: self.fee_base,
            fee_paid: fee_due(policy, self.fee_base)?,
            quote_custody: authority.quote_custody,
            fee_custody: authority.fee_custody,
            liveness_custody: authority.liveness_custody,
            active_work_remaining: self.active_work_remaining,
            pending_work_funding: self.pending_work_funding,
        };
        super::validate_state(policy, active, pending, state)?;
        Ok(state)
    }

    /// Retain only Dealer-owned coordinates from a validated semantic post-state.
    pub const fn from_validated_post(post: State) -> Self {
        Self {
            phase: post.phase,
            active_candidate_id: post.active_candidate_id,
            pending_candidate_id: post.pending_candidate_id,
            active_revision: post.active_revision,
            pending_revision: post.pending_revision,
            state_revision: post.state_revision,
            buy_used: post.buy_used,
            sell_used: post.sell_used,
            fee_base: post.fee_base,
            active_work_remaining: post.active_work_remaining,
            pending_work_funding: post.pending_work_funding,
        }
    }
}

fn tail_header(input: &[u8]) -> Result<()> {
    if input.len() != ROOT_TAIL_BYTES {
        return Err(Error::InvalidLength);
    }
    if input.get(..generated::ROOT_TAIL_MAGIC.len()) != Some(&generated::ROOT_TAIL_MAGIC) {
        return Err(Error::InvalidMagic);
    }
    if u16_at(input, generated::ROOT_TAIL_VERSION_OFFSET)? != generated::ROOT_TAIL_ABI_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_candidate, CandidateInput, CurveBand, CurveInput};

    const MARKET: Identity = [1; 32];
    const RELEASE: Identity = [2; 32];
    const DEALER: Identity = [3; 32];

    fn policy() -> Policy {
        Policy {
            market_id: MARKET,
            release_set_id: RELEASE,
            dealer_id: DEALER,
            fee_recipient_id: [4; 32],
            unwind_recipient_id: [5; 32],
            outcome_count: 2,
            quote_scale: 100,
            fee_numerator: 1,
            fee_denominator: 10,
            minimum_work_funding: 20,
            replacement_delay: 5,
        }
    }

    fn candidate_bytes() -> [u8; super::super::CANDIDATE_BYTES] {
        let bids = [CurveBand {
            capacity: 100,
            price_numerator: 40,
        }];
        let asks = [CurveBand {
            capacity: 100,
            price_numerator: 60,
        }];
        let curves = [
            CurveInput {
                bids: &bids,
                asks: &asks,
            },
            CurveInput {
                bids: &bids,
                asks: &asks,
            },
        ];
        let mut output = [0_u8; super::super::CANDIDATE_BYTES];
        encode_candidate(
            &mut output,
            CandidateInput {
                candidate_id: [6; 32],
                revision: 7,
                valid_from: 10,
                expires_at: 100,
                quote_reserve_floor: 50,
                work_funding: 20,
                work_reward: 2,
                minimum_inventory: &[0, 0],
                maximum_inventory: &[100, 100],
                curves: &curves,
            },
        )
        .expect("canonical candidate");
        output
    }

    #[test]
    fn tail_is_exact_and_round_trips() {
        let candidate = candidate_bytes();
        let active = CandidateView::decode(&candidate).expect("candidate");
        let tail = RootTail::initialize(active);
        let bytes = tail.to_bytes().expect("tail bytes");
        assert_eq!(bytes.len(), 384);
        assert_eq!(RootTail::decode(&bytes), Ok(tail));
    }

    #[test]
    fn canonical_claims_and_custody_are_ephemeral_not_persisted() {
        let candidate = candidate_bytes();
        let active = CandidateView::decode(&candidate).expect("candidate");
        let tail = RootTail::initialize(active);
        let before = tail.to_bytes().expect("tail bytes");
        let first = tail
            .materialize(
                policy(),
                active,
                None,
                AuthorityObservation {
                    release_set_id: RELEASE,
                    inventory: &[10, 20],
                    quote_custody: 50,
                    fee_custody: 0,
                    liveness_custody: 20,
                    terminal_winner: None,
                },
            )
            .expect("authority projection");
        let second = tail
            .materialize(
                policy(),
                active,
                None,
                AuthorityObservation {
                    release_set_id: RELEASE,
                    inventory: &[30, 40],
                    quote_custody: 90,
                    fee_custody: 0,
                    liveness_custody: 20,
                    terminal_winner: None,
                },
            )
            .expect("different authority projection");
        assert_eq!(&first.inventory[..2], &[10, 20]);
        assert_eq!(&second.inventory[..2], &[30, 40]);
        assert_eq!(first.quote_custody, 50);
        assert_eq!(second.quote_custody, 90);
        assert_eq!(tail.to_bytes().expect("same tail"), before);
    }

    #[test]
    fn hostile_stale_candidate_width_and_noncanonical_tail_refuse() {
        let candidate = candidate_bytes();
        let active = CandidateView::decode(&candidate).expect("candidate");
        let mut tail = RootTail::initialize(active);
        tail.active_revision += 1;
        assert_eq!(
            tail.materialize(
                policy(),
                active,
                None,
                AuthorityObservation {
                    release_set_id: RELEASE,
                    inventory: &[10, 20],
                    quote_custody: 50,
                    fee_custody: 0,
                    liveness_custody: 20,
                    terminal_winner: None,
                },
            ),
            Err(Error::IdentityMismatch)
        );

        let canonical = RootTail::initialize(active);
        assert_eq!(
            canonical.materialize(
                policy(),
                active,
                None,
                AuthorityObservation {
                    release_set_id: [9; 32],
                    inventory: &[10, 20],
                    quote_custody: 50,
                    fee_custody: 0,
                    liveness_custody: 20,
                    terminal_winner: None,
                },
            ),
            Err(Error::IdentityMismatch)
        );

        assert_eq!(
            canonical.materialize(
                policy(),
                active,
                None,
                AuthorityObservation {
                    release_set_id: RELEASE,
                    inventory: &[10],
                    quote_custody: 50,
                    fee_custody: 0,
                    liveness_custody: 20,
                    terminal_winner: None,
                },
            ),
            Err(Error::InvalidLength)
        );

        let mut bytes = canonical.to_bytes().expect("canonical tail");
        bytes[generated::ROOT_TAIL_RESERVED_OFFSET] = 1;
        assert_eq!(RootTail::decode(&bytes), Err(Error::NonCanonicalPadding));
    }
}
