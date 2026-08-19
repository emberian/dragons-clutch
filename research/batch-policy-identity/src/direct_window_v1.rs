//! Fixed-layout model of a closed direct-candidate window.
//!
//! This module deliberately stops at an offline transition model.  In
//! particular, [`crate::direct_window_v1::DirectWindowBindingV1`] assumes its
//! opening and closing slots
//! came from an immutable, authenticated Epoch schema.  The current live Epoch
//! account has no such fields, so a live adapter must refuse rather than let
//! the first submitter choose either boundary.
//!
//! Capacity is not a semantic candidate-set cap.  The window retains the
//! canonical best three candidates seen so far.  Every admitted fresh
//! submission advances a full-width transcript commitment and count; once the
//! top-three array is full, a strictly better candidate replaces the worst and
//! the displaced account becomes `SUPERSEDED`.  Therefore a poor early trio
//! cannot crowd out a later better candidate.  The transcript commitment is
//! intentionally order-sensitive audit evidence.  The top-three result and
//! selected winner are order-independent under the frozen total score order.

use core::cmp::Ordering;

use clutch_batch::relation_v1::{
    AllocationPolicyV1, AonPolicyV1, FeeBaseV1, FrozenPolicyV1, PairingWitnessPolicyV1,
    PortfolioLotPolicyV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1,
    SelfCrossPolicyV1, TransferPhaseV1, MAX_OUTCOMES,
};
use clutch_batch::{DustPolicy, MAX_ORDERS};
use sha2::{Digest, Sha256};

use super::{
    FullRelationDomainV1, FullScoreV1, FullSubmittedCandidateV1, Identity32V1,
    PolicyIdentityErrorV1, VerifiedSubmittedCandidateV1, ACCOUNT_CANDIDATE_DIGEST_DOMAIN,
    FULL_RELATION_CANDIDATE_DIGEST_DOMAIN,
};

/// Maximum number of candidate accounts retained for final re-verification.
pub const MAX_DIRECT_CANDIDATES: usize = 3;
/// Candidate account bytes after a future two-byte layout tag/version envelope.
pub const DIRECT_CANDIDATE_ACCOUNT_BYTES: usize = 440;
/// Candidate bytes modeled here, excluding that unallocated envelope.
pub const DIRECT_CANDIDATE_BODY_BYTES: usize = DIRECT_CANDIDATE_ACCOUNT_BYTES - 2;
/// Window account bytes after a future two-byte layout tag/version envelope.
pub const DIRECT_WINDOW_ACCOUNT_BYTES: usize = 456;
/// Window bytes modeled here, excluding that unallocated envelope.
pub const DIRECT_WINDOW_BODY_BYTES: usize = DIRECT_WINDOW_ACCOUNT_BYTES - 2;

/// A candidate whose full relation was recomputed but which is not selected.
pub const DIRECT_CANDIDATE_STATUS_VERIFIED: u8 = 1;
/// The unique selected candidate of a closed window.
pub const DIRECT_CANDIDATE_STATUS_SELECTED: u8 = 2;
/// A valid candidate which did not win the closed submitted set.
pub const DIRECT_CANDIDATE_STATUS_SUPERSEDED: u8 = 4;
/// Candidate window still accepting submissions before its frozen close slot.
pub const DIRECT_WINDOW_PHASE_OPEN: u8 = 0;
/// Candidate window closed and one retained candidate selected.
pub const DIRECT_WINDOW_PHASE_SELECTED: u8 = 1;

/// The one complete policy profile this compact account can currently carry.
///
/// This is an equality gate, not an adapter default: every other authenticated
/// policy is refused before projection because the compact account has no fee,
/// remainder, cumulative-residual, Active-only, or explicit-witness fields.
pub const DIRECT_POLICY_V1: FrozenPolicyV1 = FrozenPolicyV1 {
    allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
    self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
    aon: AonPolicyV1::RefuseAdmission,
    rounding: RoundingBoundaryV1::None,
    residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
    transfer_phase: TransferPhaseV1::ActiveOrResolved,
    portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
    pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
    dust: DustPolicy::Reject,
    score: ScorePolicyV1::LexicographicDispersionV1,
    fee_base: FeeBaseV1::None,
};

/// Domain separator for the ordered admission transcript.
pub const DIRECT_ADMISSION_TRANSCRIPT_DOMAIN: &[u8] =
    b"dragons-clutch/direct-admission-transcript/v1\0";

const CANDIDATE_RESERVED_BYTES: usize = 12;
const WINDOW_RESERVED_BYTES: usize = 2;

const _: () = assert!(
    DIRECT_CANDIDATE_BODY_BYTES
        == (7 * 32)
            + (MAX_OUTCOMES * 8)
            + (2 * 8)
            + 16
            + 16
            + 8
            + 8
            + 1
            + 1
            + 1
            + 2
            + 1
            + 1
            + 1
            + 1
            + 1
            + CANDIDATE_RESERVED_BYTES
);
const _: () = assert!(
    DIRECT_WINDOW_BODY_BYTES
        == (7 * 32) + (MAX_DIRECT_CANDIDATES * 2 * 32) + (3 * 8) + 8 + 4 + WINDOW_RESERVED_BYTES
);

/// Refusals owned by the bounded window model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectWindowErrorV1 {
    /// Exact account-body length was not supplied.
    WrongLength,
    /// A required identity is zero.
    ZeroIdentity,
    /// A count, selector, status, or reserved byte is non-canonical.
    NonCanonical,
    /// An identity or domain binding differs.
    MismatchedBinding,
    /// The immutable opening boundary has not arrived.
    BeforeOpen,
    /// Submission was attempted at or after the immutable close boundary.
    SubmissionClosed,
    /// Selection was attempted before the immutable close boundary.
    SelectionEarly,
    /// A selected window was replayed.
    AlreadySelected,
    /// The same content-derived candidate was submitted twice.
    Replay,
    /// An increment or score-coordinate conversion overflowed.
    ArithmeticOverflow,
    /// A relation-verified candidate cannot inhabit the direct specialization.
    NotDirect,
    /// Full relation verification refused.
    Relation(PolicyIdentityErrorV1),
}

impl From<PolicyIdentityErrorV1> for DirectWindowErrorV1 {
    fn from(value: PolicyIdentityErrorV1) -> Self {
        Self::Relation(value)
    }
}

fn nonzero(value: Identity32V1) -> Result<(), DirectWindowErrorV1> {
    if value.is_zero() {
        Err(DirectWindowErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

/// Recompute the existing account-plane candidate identity without truncating
/// either parent identity. This is public so the hostile-byte layout owner can
/// pin byte-for-byte parity at the dependency boundary.
pub fn canonical_account_candidate_id(
    epoch: Identity32V1,
    market: Identity32V1,
    prices: &[u64; MAX_OUTCOMES],
) -> Identity32V1 {
    let mut h = Sha256::new();
    h.update(ACCOUNT_CANDIDATE_DIGEST_DOMAIN);
    h.update(epoch.0);
    h.update(market.0);
    h.update([2]);
    h.update([2]);
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        h.update(prices[i].to_le_bytes());
        i += 1;
    }
    // Direct V1 fixes sigma, mu, and the honored-AON mask to zero.
    h.update(0u64.to_le_bytes());
    h.update(0u64.to_le_bytes());
    h.update(0u64.to_le_bytes());
    Identity32V1(h.finalize().into())
}

/// Exact retained identity pair.  Score coordinates live in the referenced
/// candidate account and must be presented again on every registry mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateEntryV1 {
    /// Existing account-plane candidate identity.
    pub candidate_id: Identity32V1,
    /// Full SHA-256 relation-candidate digest and final score component.
    pub relation_candidate_digest: Identity32V1,
}

impl DirectCandidateEntryV1 {
    /// Canonical padding for an unused top-three slot.
    pub const ZERO: Self = Self {
        candidate_id: Identity32V1::ZERO,
        relation_candidate_digest: Identity32V1::ZERO,
    };

    fn validate_active(&self) -> Result<(), DirectWindowErrorV1> {
        nonzero(self.candidate_id)?;
        nonzero(self.relation_candidate_digest)
    }
}

/// Adapter-authenticated coordinates which are not outputs of relation
/// verification itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateCoordinatesV1 {
    /// Authenticated submission slot.
    pub submitted_slot: u64,
    /// Relation index of the buy order.
    pub buy_index: u8,
    /// Relation index of the sell order.
    pub sell_index: u8,
    /// Shared Egg outcome.
    pub outcome: u8,
    /// Future PDA bump.
    pub stored_bump: u8,
}

/// Complete small input to the SBF-safe direct relation specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTwoOrderInputV1 {
    /// Candidate simplex vector; exactly two entries are active.
    pub prices: [u64; MAX_OUTCOMES],
    /// Buy order's frozen limit.
    pub buy_limit: u64,
    /// Sell order's frozen limit.
    pub sell_limit: u64,
    /// Exact common full-fill quantity.
    pub quantity: u64,
    /// Authenticated Clock slot of admission.
    pub submitted_slot: u64,
    /// Relation index of the buy order.
    pub buy_index: u8,
    /// Relation index of the sell order.
    pub sell_index: u8,
    /// Shared Egg outcome.
    pub outcome: u8,
    /// Canonical future PDA bump.
    pub stored_bump: u8,
}

/// Verify the exact two-order direct specialization without materializing the
/// 64-order host relation.
///
/// The arithmetic and digest are byte-for-byte the same V0--V9 result as the
/// full relation for two opposite, distinct-owner, full-fill single-Egg orders
/// under [`DIRECT_POLICY_V1`]. Tests below compare this bounded implementation
/// against the full verifier across prices and book orderings. No identity is
/// projected to the host relation's legacy `u64` tags.
pub fn verify_direct_two_order_candidate(
    domain: &FullRelationDomainV1,
    input: DirectTwoOrderInputV1,
) -> Result<DirectCandidateV2, DirectWindowErrorV1> {
    domain.validate()?;
    if domain.policy != DIRECT_POLICY_V1
        || domain.outcome_count != 2
        || domain.owner_count != 2
        || domain.price_scale == 0
        || input.quantity == 0
        || input.buy_index > 1
        || input.sell_index > 1
        || input.buy_index == input.sell_index
        || input.outcome >= 2
        || input.prices[2..].iter().any(|price| *price != 0)
    {
        return Err(DirectWindowErrorV1::NotDirect);
    }
    let price = input.prices[usize::from(input.outcome)];
    let other = input.prices[1usize - usize::from(input.outcome)];
    if price == 0
        || other == 0
        || price.checked_add(other) != Some(domain.price_scale)
        || input.sell_limit > price
        || price > input.buy_limit
    {
        return Err(DirectWindowErrorV1::NotDirect);
    }
    let weighted = u128::from(input.quantity)
        .checked_mul(u128::from(price))
        .and_then(|value| value.checked_mul(u128::from(other)))
        .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
    let consideration = u128::from(input.quantity)
        .checked_mul(u128::from(price))
        .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
    if consideration % u128::from(domain.price_scale) != 0 {
        return Err(DirectWindowErrorV1::NotDirect);
    }
    let weighted_direct_volume =
        i128::try_from(weighted).map_err(|_| DirectWindowErrorV1::ArithmeticOverflow)?;
    let spread = input
        .buy_limit
        .checked_sub(input.sell_limit)
        .ok_or(DirectWindowErrorV1::NotDirect)?;
    let limit_surplus_price_units = u128::from(input.quantity)
        .checked_mul(u128::from(spread))
        .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
    let candidate_id =
        canonical_account_candidate_id(domain.epoch_id, domain.market_id, &input.prices);
    let relation_candidate_digest =
        direct_relation_candidate_digest(domain, candidate_id, input.quantity)?;
    let value = DirectCandidateV2 {
        candidate_id,
        epoch_id: domain.epoch_id,
        market_id: domain.market_id,
        order_set_id: domain.order_set_id,
        policy_id: domain.policy_id,
        relation_domain_digest: domain.digest()?,
        relation_candidate_digest,
        prices: input.prices,
        fills: [input.quantity, input.quantity],
        weighted_direct_volume,
        limit_surplus_price_units,
        submitted_slot: input.submitted_slot,
        quantity: input.quantity,
        buy_index: input.buy_index,
        sell_index: input.sell_index,
        outcome: input.outcome,
        distinct_owners: 2,
        order_len: 2,
        outcome_count: 2,
        status: DIRECT_CANDIDATE_STATUS_VERIFIED,
        stored_bump: input.stored_bump,
        flags: 0,
        reserved: [0; CANDIDATE_RESERVED_BYTES],
    };
    value.validate()?;
    Ok(value)
}

fn direct_relation_candidate_digest(
    domain: &FullRelationDomainV1,
    candidate_id: Identity32V1,
    quantity: u64,
) -> Result<Identity32V1, DirectWindowErrorV1> {
    let mut h = Sha256::new();
    h.update(FULL_RELATION_CANDIDATE_DIGEST_DOMAIN);
    h.update(domain.digest()?.0);
    h.update(candidate_id.0);
    h.update(quantity.to_le_bytes());
    h.update(quantity.to_le_bytes());
    let mut index = 2usize;
    while index < MAX_ORDERS {
        h.update(0u64.to_le_bytes());
        index += 1;
    }
    h.update(0u64.to_le_bytes()); // honored AON mask
    h.update([0]); // no explicit pairing witness under DIRECT_POLICY_V1
    Ok(Identity32V1(h.finalize().into()))
}

/// Compact, fixed-layout verified candidate for the bounded direct profile.
///
/// It persists the exact two fills because the live V1 CandidateRecord omits
/// them and its CandidateFeed truncates the relation digest.  It cannot encode
/// fees, partials, portfolios, virtual legs, or more than one direct slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateV2 {
    /// Existing canonical free-coordinate identity.
    pub candidate_id: Identity32V1,
    /// Full Epoch identity.
    pub epoch_id: Identity32V1,
    /// Full Market identity.
    pub market_id: Identity32V1,
    /// Complete frozen order-set identity.
    pub order_set_id: Identity32V1,
    /// Immutable BatchPolicy digest.
    pub policy_id: Identity32V1,
    /// Full relation-domain digest.
    pub relation_domain_digest: Identity32V1,
    /// Full relation-candidate digest; also the final score coordinate.
    pub relation_candidate_digest: Identity32V1,
    /// Full simplex vector; only two outcomes are active.
    pub prices: [u64; MAX_OUTCOMES],
    /// Exact two full fills in frozen-book order.
    pub fills: [u64; 2],
    /// Score coordinate 1.
    pub weighted_direct_volume: i128,
    /// Score coordinate 2.
    pub limit_surplus_price_units: u128,
    /// Authenticated Clock slot of admission.
    pub submitted_slot: u64,
    /// The one direct slice quantity; equals both fills.
    pub quantity: u64,
    /// Relation index of the buy order.
    pub buy_index: u8,
    /// Relation index of the sell order.
    pub sell_index: u8,
    /// Shared Egg outcome.
    pub outcome: u8,
    /// Always two in this profile.
    pub distinct_owners: u16,
    /// Always two in this profile.
    pub order_len: u8,
    /// Always two in this profile.
    pub outcome_count: u8,
    /// `VERIFIED`, `SELECTED`, or `SUPERSEDED`.
    pub status: u8,
    /// Future PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero.
    pub flags: u8,
    /// Reserved bytes; zero.
    pub reserved: [u8; CANDIDATE_RESERVED_BYTES],
}

impl DirectCandidateV2 {
    /// Construct the compact persisted projection of one already reverified
    /// full-width candidate.  This method does not perform relation execution;
    /// the unforgeable input type is the result of that execution.
    pub fn from_verified(
        domain: &FullRelationDomainV1,
        submitted: &FullSubmittedCandidateV1,
        verified: &VerifiedSubmittedCandidateV1,
        coordinates: DirectCandidateCoordinatesV1,
    ) -> Result<Self, DirectWindowErrorV1> {
        domain.validate()?;
        if domain.policy != DIRECT_POLICY_V1
            || submitted.candidate_id != verified.candidate_id
            || submitted.claimed_score != verified.score
            || submitted.order_len != 2
            || submitted.virtual_split != 0
            || submitted.virtual_merge != 0
            || submitted.honored_aon_mask != 0
            || coordinates.buy_index > 1
            || coordinates.sell_index > 1
            || coordinates.buy_index == coordinates.sell_index
            || coordinates.outcome >= 2
        {
            return Err(DirectWindowErrorV1::NotDirect);
        }
        let quantity = submitted.fills[0];
        if quantity == 0 || submitted.fills[1] != quantity {
            return Err(DirectWindowErrorV1::NotDirect);
        }
        let mut i = 2usize;
        while i < MAX_ORDERS {
            if submitted.fills[i] != 0 {
                return Err(DirectWindowErrorV1::NotDirect);
            }
            i += 1;
        }
        let value = Self {
            candidate_id: submitted.candidate_id,
            epoch_id: domain.epoch_id,
            market_id: domain.market_id,
            order_set_id: domain.order_set_id,
            policy_id: domain.policy_id,
            relation_domain_digest: domain.digest()?,
            relation_candidate_digest: verified.score.digest,
            prices: submitted.prices,
            fills: [submitted.fills[0], submitted.fills[1]],
            weighted_direct_volume: verified.score.weighted_direct_volume,
            limit_surplus_price_units: verified.score.limit_surplus_price_units,
            submitted_slot: coordinates.submitted_slot,
            quantity,
            buy_index: coordinates.buy_index,
            sell_index: coordinates.sell_index,
            outcome: coordinates.outcome,
            distinct_owners: verified.score.distinct_owners,
            order_len: 2,
            outcome_count: 2,
            status: DIRECT_CANDIDATE_STATUS_VERIFIED,
            stored_bump: coordinates.stored_bump,
            flags: 0,
            reserved: [0; CANDIDATE_RESERVED_BYTES],
        };
        value.validate()?;
        Ok(value)
    }

    /// Frozen full score used by the window's total order.
    pub const fn score(&self) -> FullScoreV1 {
        FullScoreV1 {
            weighted_direct_volume: self.weighted_direct_volume,
            limit_surplus_price_units: self.limit_surplus_price_units,
            distinct_owners: self.distinct_owners,
            churn: 0,
            digest: self.relation_candidate_digest,
        }
    }

    /// Retained identity pair.
    pub const fn entry(&self) -> DirectCandidateEntryV1 {
        DirectCandidateEntryV1 {
            candidate_id: self.candidate_id,
            relation_candidate_digest: self.relation_candidate_digest,
        }
    }

    /// Validate fixed direct shape, canonical padding, and account identity.
    pub fn validate(&self) -> Result<(), DirectWindowErrorV1> {
        for identity in [
            self.candidate_id,
            self.epoch_id,
            self.market_id,
            self.order_set_id,
            self.policy_id,
            self.relation_domain_digest,
            self.relation_candidate_digest,
        ] {
            nonzero(identity)?;
        }
        if self.quantity == 0
            || self.fills != [self.quantity, self.quantity]
            || self.buy_index > 1
            || self.sell_index > 1
            || self.buy_index == self.sell_index
            || self.outcome >= 2
            || self.distinct_owners != 2
            || self.order_len != 2
            || self.outcome_count != 2
            || !matches!(
                self.status,
                DIRECT_CANDIDATE_STATUS_VERIFIED
                    | DIRECT_CANDIDATE_STATUS_SELECTED
                    | DIRECT_CANDIDATE_STATUS_SUPERSEDED
            )
            || self.flags != 0
            || self.reserved.iter().any(|byte| *byte != 0)
            || self.prices[2..].iter().any(|price| *price != 0)
        {
            return Err(DirectWindowErrorV1::NonCanonical);
        }
        if self.candidate_id
            != canonical_account_candidate_id(self.epoch_id, self.market_id, &self.prices)
        {
            return Err(DirectWindowErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode the exact modeled body.  A future live layout must prepend its
    /// independently allocated two-byte tag/version envelope.
    pub fn encode_body(&self, out: &mut [u8]) -> Result<usize, DirectWindowErrorV1> {
        self.validate()?;
        if out.len() != DIRECT_CANDIDATE_BODY_BYTES {
            return Err(DirectWindowErrorV1::WrongLength);
        }
        let mut w = Writer::new(out);
        for value in [
            self.candidate_id,
            self.epoch_id,
            self.market_id,
            self.order_set_id,
            self.policy_id,
            self.relation_domain_digest,
            self.relation_candidate_digest,
        ] {
            w.identity(value)?;
        }
        for value in self.prices {
            w.u64(value)?;
        }
        for value in self.fills {
            w.u64(value)?;
        }
        w.i128(self.weighted_direct_volume)?;
        w.u128(self.limit_surplus_price_units)?;
        w.u64(self.submitted_slot)?;
        w.u64(self.quantity)?;
        w.u8(self.buy_index)?;
        w.u8(self.sell_index)?;
        w.u8(self.outcome)?;
        w.u16(self.distinct_owners)?;
        w.u8(self.order_len)?;
        w.u8(self.outcome_count)?;
        w.u8(self.status)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.bytes(&self.reserved)?;
        w.finish()
    }

    /// Decode exactly one modeled body.
    pub fn decode_body(input: &[u8]) -> Result<Self, DirectWindowErrorV1> {
        if input.len() != DIRECT_CANDIDATE_BODY_BYTES {
            return Err(DirectWindowErrorV1::WrongLength);
        }
        let mut r = Reader::new(input);
        let candidate_id = r.identity()?;
        let epoch_id = r.identity()?;
        let market_id = r.identity()?;
        let order_set_id = r.identity()?;
        let policy_id = r.identity()?;
        let relation_domain_digest = r.identity()?;
        let relation_candidate_digest = r.identity()?;
        let mut prices = [0u64; MAX_OUTCOMES];
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            prices[i] = r.u64()?;
            i += 1;
        }
        let fills = [r.u64()?, r.u64()?];
        let value = Self {
            candidate_id,
            epoch_id,
            market_id,
            order_set_id,
            policy_id,
            relation_domain_digest,
            relation_candidate_digest,
            prices,
            fills,
            weighted_direct_volume: r.i128()?,
            limit_surplus_price_units: r.u128()?,
            submitted_slot: r.u64()?,
            quantity: r.u64()?,
            buy_index: r.u8()?,
            sell_index: r.u8()?,
            outcome: r.u8()?,
            distinct_owners: r.u16()?,
            order_len: r.u8()?,
            outcome_count: r.u8()?,
            status: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
            reserved: r.array()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Frozen inputs which the current live Epoch schema cannot yet transport.
///
/// A future version must bind both slots into the immutable Epoch identity.
/// Neither slot is supplied by the first submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectWindowBindingV1 {
    /// Full Epoch identity.
    pub epoch_id: Identity32V1,
    /// Full Market identity.
    pub market_id: Identity32V1,
    /// Complete frozen order set.
    pub order_set_id: Identity32V1,
    /// Immutable policy identity.
    pub policy_id: Identity32V1,
    /// Full relation-domain digest.
    pub relation_domain_digest: Identity32V1,
    /// Immutable first admissible submission slot.
    pub opens_slot: u64,
    /// Immutable first inadmissible submission slot.
    pub closes_slot: u64,
}

impl DirectWindowBindingV1 {
    /// Validate exact identities and a non-empty half-open window.
    pub fn validate(&self) -> Result<(), DirectWindowErrorV1> {
        for value in [
            self.epoch_id,
            self.market_id,
            self.order_set_id,
            self.policy_id,
            self.relation_domain_digest,
        ] {
            nonzero(value)?;
        }
        if self.opens_slot >= self.closes_slot {
            return Err(DirectWindowErrorV1::NonCanonical);
        }
        Ok(())
    }
}

/// Exact fixed-layout closed-set owner for the bounded direct profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateWindowV1 {
    /// Full Epoch identity.
    pub epoch_id: Identity32V1,
    /// Full Market identity.
    pub market_id: Identity32V1,
    /// Complete frozen order set.
    pub order_set_id: Identity32V1,
    /// Immutable policy identity.
    pub policy_id: Identity32V1,
    /// Full relation-domain digest.
    pub relation_domain_digest: Identity32V1,
    /// Order-sensitive commitment over every fresh admitted submission.
    pub admission_transcript: Identity32V1,
    /// Zero while open; the unique winner after selection.
    pub selected_candidate: Identity32V1,
    /// Canonical best-to-worst retained identities.
    pub top: [DirectCandidateEntryV1; MAX_DIRECT_CANDIDATES],
    /// Immutable opening slot from the Epoch.
    pub opens_slot: u64,
    /// Immutable closing slot from the Epoch.
    pub closes_slot: u64,
    /// Authenticated selection slot, zero while open.
    pub selected_slot: u64,
    /// Count of every fresh valid candidate admitted, including superseded.
    pub admitted_count: u64,
    /// Active prefix length of `top`.
    pub top_count: u8,
    /// `OPEN` or `SELECTED`.
    pub phase: u8,
    /// Future PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero.
    pub flags: u8,
    /// Reserved bytes; zero.
    pub reserved: [u8; WINDOW_RESERVED_BYTES],
}

impl DirectCandidateWindowV1 {
    /// Atomically construct a non-empty window from its first fresh verified
    /// candidate.  The caller cannot choose either time boundary.
    pub fn first(
        binding: DirectWindowBindingV1,
        candidate: &DirectCandidateV2,
        now: u64,
        stored_bump: u8,
    ) -> Result<Self, DirectWindowErrorV1> {
        binding.validate()?;
        candidate.validate()?;
        bind_candidate(&binding, candidate)?;
        require_submission_time(binding.opens_slot, binding.closes_slot, now)?;
        if candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED || candidate.submitted_slot != now {
            return Err(DirectWindowErrorV1::MismatchedBinding);
        }
        let mut value = Self {
            epoch_id: binding.epoch_id,
            market_id: binding.market_id,
            order_set_id: binding.order_set_id,
            policy_id: binding.policy_id,
            relation_domain_digest: binding.relation_domain_digest,
            admission_transcript: Identity32V1::ZERO,
            selected_candidate: Identity32V1::ZERO,
            top: [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES],
            opens_slot: binding.opens_slot,
            closes_slot: binding.closes_slot,
            selected_slot: 0,
            admitted_count: 1,
            top_count: 1,
            phase: DIRECT_WINDOW_PHASE_OPEN,
            stored_bump,
            flags: 0,
            reserved: [0; WINDOW_RESERVED_BYTES],
        };
        value.top[0] = candidate.entry();
        value.admission_transcript =
            next_transcript(Identity32V1::ZERO, &value, 1, candidate.entry());
        value.validate()?;
        Ok(value)
    }

    /// Frozen binding repeated by the window.
    pub const fn binding(&self) -> DirectWindowBindingV1 {
        DirectWindowBindingV1 {
            epoch_id: self.epoch_id,
            market_id: self.market_id,
            order_set_id: self.order_set_id,
            policy_id: self.policy_id,
            relation_domain_digest: self.relation_domain_digest,
            opens_slot: self.opens_slot,
            closes_slot: self.closes_slot,
        }
    }

    /// Validate local shape and canonical padding.  Score ordering is checked
    /// separately against the exact referenced candidate accounts.
    pub fn validate(&self) -> Result<(), DirectWindowErrorV1> {
        self.binding().validate()?;
        nonzero(self.admission_transcript)?;
        if self.top_count == 0
            || usize::from(self.top_count) > MAX_DIRECT_CANDIDATES
            || self.admitted_count < u64::from(self.top_count)
            || self.flags != 0
            || self.reserved.iter().any(|byte| *byte != 0)
        {
            return Err(DirectWindowErrorV1::NonCanonical);
        }
        match self.phase {
            DIRECT_WINDOW_PHASE_OPEN => {
                if !self.selected_candidate.is_zero() || self.selected_slot != 0 {
                    return Err(DirectWindowErrorV1::NonCanonical);
                }
            }
            DIRECT_WINDOW_PHASE_SELECTED => {
                nonzero(self.selected_candidate)?;
                if self.selected_slot < self.closes_slot
                    || self.selected_candidate != self.top[0].candidate_id
                {
                    return Err(DirectWindowErrorV1::NonCanonical);
                }
            }
            _ => return Err(DirectWindowErrorV1::NonCanonical),
        }
        let mut i = 0usize;
        while i < MAX_DIRECT_CANDIDATES {
            if i < usize::from(self.top_count) {
                self.top[i].validate_active()?;
                let mut j = 0usize;
                while j < i {
                    if self.top[j].candidate_id == self.top[i].candidate_id {
                        return Err(DirectWindowErrorV1::Replay);
                    }
                    j += 1;
                }
            } else if self.top[i] != DirectCandidateEntryV1::ZERO {
                return Err(DirectWindowErrorV1::NonCanonical);
            }
            i += 1;
        }
        Ok(())
    }

    /// Encode the exact modeled body.
    pub fn encode_body(&self, out: &mut [u8]) -> Result<usize, DirectWindowErrorV1> {
        self.validate()?;
        if out.len() != DIRECT_WINDOW_BODY_BYTES {
            return Err(DirectWindowErrorV1::WrongLength);
        }
        let mut w = Writer::new(out);
        for value in [
            self.epoch_id,
            self.market_id,
            self.order_set_id,
            self.policy_id,
            self.relation_domain_digest,
            self.admission_transcript,
            self.selected_candidate,
        ] {
            w.identity(value)?;
        }
        for entry in self.top {
            w.identity(entry.candidate_id)?;
            w.identity(entry.relation_candidate_digest)?;
        }
        w.u64(self.opens_slot)?;
        w.u64(self.closes_slot)?;
        w.u64(self.selected_slot)?;
        w.u64(self.admitted_count)?;
        w.u8(self.top_count)?;
        w.u8(self.phase)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        w.bytes(&self.reserved)?;
        w.finish()
    }

    /// Decode exactly one modeled body.
    pub fn decode_body(input: &[u8]) -> Result<Self, DirectWindowErrorV1> {
        if input.len() != DIRECT_WINDOW_BODY_BYTES {
            return Err(DirectWindowErrorV1::WrongLength);
        }
        let mut r = Reader::new(input);
        let epoch_id = r.identity()?;
        let market_id = r.identity()?;
        let order_set_id = r.identity()?;
        let policy_id = r.identity()?;
        let relation_domain_digest = r.identity()?;
        let admission_transcript = r.identity()?;
        let selected_candidate = r.identity()?;
        let mut top = [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES];
        let mut i = 0usize;
        while i < MAX_DIRECT_CANDIDATES {
            top[i] = DirectCandidateEntryV1 {
                candidate_id: r.identity()?,
                relation_candidate_digest: r.identity()?,
            };
            i += 1;
        }
        let value = Self {
            epoch_id,
            market_id,
            order_set_id,
            policy_id,
            relation_domain_digest,
            admission_transcript,
            selected_candidate,
            top,
            opens_slot: r.u64()?,
            closes_slot: r.u64()?,
            selected_slot: r.u64()?,
            admitted_count: r.u64()?,
            top_count: r.u8()?,
            phase: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
            reserved: r.array()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Complete post-state of one later submission.  The runtime can preflight
/// every byte before writing any account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAdmissionPlanV1 {
    /// Complete next window.
    pub post_window: DirectCandidateWindowV1,
    /// Status to write to the newly admitted candidate.
    pub submitted_status: u8,
    /// Existing retained candidate to mark superseded, or zero.
    pub displaced_candidate: Identity32V1,
}

/// Stack-bounded retained-candidate view used by the live SBF registry walk.
///
/// Construction validates the complete candidate and its full-width window
/// binding before discarding fields which do not participate in ranking.  The
/// compact value therefore cannot be manufactured from an unauthenticated
/// score by the runtime adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRetainedCandidateV1 {
    /// Exact two-digest registry entry frozen in the window.
    pub entry: DirectCandidateEntryV1,
    /// Complete total-order score, including its full digest tie-break.
    pub score: FullScoreV1,
}

impl DirectRetainedCandidateV1 {
    /// Validate and project one complete retained candidate.
    pub fn from_candidate(
        window: &DirectCandidateWindowV1,
        candidate: &DirectCandidateV2,
    ) -> Result<Self, DirectWindowErrorV1> {
        window.validate()?;
        candidate.validate()?;
        bind_candidate(&window.binding(), candidate)?;
        if candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED {
            return Err(DirectWindowErrorV1::MismatchedBinding);
        }
        Ok(Self {
            entry: candidate.entry(),
            score: candidate.score(),
        })
    }

    /// Zero padding for a fixed-capacity stack array.  It is never accepted in
    /// the active prefix because registry validation requires exact entries.
    pub const ZERO: Self = Self {
        entry: DirectCandidateEntryV1::ZERO,
        score: FullScoreV1::ZERO,
    };
}

/// Complete post-state facts of once-only selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionPlanV1 {
    /// Complete selected window.
    pub post_window: DirectCandidateWindowV1,
    /// Unique winner to mark selected and freeze into the receipt.
    pub selected_candidate: Identity32V1,
    /// Retained losers to mark superseded.
    pub superseded: [Identity32V1; MAX_DIRECT_CANDIDATES - 1],
    /// Active prefix length of `superseded`.
    pub superseded_count: u8,
}

/// Admit one fresh verified candidate into an existing open window.
///
/// `current_top` must be the exact account list named by the retained registry,
/// in best-to-worst order.  Freshness remains the future adapter's
/// content-PDA creation obligation; candidates already retained are also
/// rejected here, while displaced/rejected candidates carry `SUPERSEDED` and
/// cannot be resubmitted.
pub fn plan_admission(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectCandidateV2],
    submitted: &DirectCandidateV2,
    now: u64,
) -> Result<DirectAdmissionPlanV1, DirectWindowErrorV1> {
    validate_registry(window, current_top)?;
    let mut retained = [DirectRetainedCandidateV1::ZERO; MAX_DIRECT_CANDIDATES];
    let mut i = 0usize;
    while i < current_top.len() {
        retained[i] = DirectRetainedCandidateV1 {
            entry: current_top[i].entry(),
            score: current_top[i].score(),
        };
        i += 1;
    }
    plan_admission_retained(window, &retained[..current_top.len()], submitted, now)
}

/// Stack-bounded admission over already authenticated retained projections.
///
/// This is byte-for-byte the same transition as [`plan_admission`].  It lets a
/// constrained adapter decode and authenticate one 440-byte candidate account
/// at a time, retain only its exact entry and score, and avoid keeping three
/// complete candidates in one 4 KiB SBF frame.
pub fn plan_admission_retained(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectRetainedCandidateV1],
    submitted: &DirectCandidateV2,
    now: u64,
) -> Result<DirectAdmissionPlanV1, DirectWindowErrorV1> {
    window.validate()?;
    if window.phase != DIRECT_WINDOW_PHASE_OPEN {
        return Err(DirectWindowErrorV1::AlreadySelected);
    }
    require_submission_time(window.opens_slot, window.closes_slot, now)?;
    validate_retained_registry(window, current_top)?;
    submitted.validate()?;
    bind_candidate(&window.binding(), submitted)?;
    if submitted.status != DIRECT_CANDIDATE_STATUS_VERIFIED || submitted.submitted_slot != now {
        return Err(DirectWindowErrorV1::MismatchedBinding);
    }
    let count = usize::from(window.top_count);
    let mut i = 0usize;
    while i < count {
        if current_top[i].entry.candidate_id == submitted.candidate_id {
            return Err(DirectWindowErrorV1::Replay);
        }
        i += 1;
    }

    let next_count = window
        .admitted_count
        .checked_add(1)
        .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
    let mut post = *window;
    post.admitted_count = next_count;
    post.admission_transcript = next_transcript(
        window.admission_transcript,
        window,
        next_count,
        submitted.entry(),
    );

    let mut rank = count;
    i = 0;
    while i < count {
        match submitted.score().total_order(&current_top[i].score) {
            Ordering::Greater => {
                rank = i;
                break;
            }
            Ordering::Equal => return Err(DirectWindowErrorV1::NonCanonical),
            Ordering::Less => {}
        }
        i += 1;
    }

    let mut submitted_status = DIRECT_CANDIDATE_STATUS_VERIFIED;
    let mut displaced = Identity32V1::ZERO;
    if count < MAX_DIRECT_CANDIDATES {
        let mut at = count;
        while at > rank {
            post.top[at] = post.top[at - 1];
            at -= 1;
        }
        post.top[rank] = submitted.entry();
        post.top_count += 1;
    } else if rank < MAX_DIRECT_CANDIDATES {
        displaced = post.top[MAX_DIRECT_CANDIDATES - 1].candidate_id;
        let mut at = MAX_DIRECT_CANDIDATES - 1;
        while at > rank {
            post.top[at] = post.top[at - 1];
            at -= 1;
        }
        post.top[rank] = submitted.entry();
    } else {
        submitted_status = DIRECT_CANDIDATE_STATUS_SUPERSEDED;
    }
    post.validate()?;
    Ok(DirectAdmissionPlanV1 {
        post_window: post,
        submitted_status,
        displaced_candidate: displaced,
    })
}

/// Close an expired non-empty window and select its exact best retained
/// candidate.  A late call is valid; an early call and every replay refuse.
pub fn plan_selection(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectCandidateV2],
    now: u64,
) -> Result<DirectSelectionPlanV1, DirectWindowErrorV1> {
    validate_registry(window, current_top)?;
    let mut retained = [DirectRetainedCandidateV1::ZERO; MAX_DIRECT_CANDIDATES];
    let mut i = 0usize;
    while i < current_top.len() {
        retained[i] = DirectRetainedCandidateV1 {
            entry: current_top[i].entry(),
            score: current_top[i].score(),
        };
        i += 1;
    }
    plan_selection_retained(window, &retained[..current_top.len()], now)
}

/// Stack-bounded once-only selection over authenticated retained projections.
pub fn plan_selection_retained(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectRetainedCandidateV1],
    now: u64,
) -> Result<DirectSelectionPlanV1, DirectWindowErrorV1> {
    window.validate()?;
    if window.phase != DIRECT_WINDOW_PHASE_OPEN {
        return Err(DirectWindowErrorV1::AlreadySelected);
    }
    if now < window.closes_slot {
        return Err(DirectWindowErrorV1::SelectionEarly);
    }
    validate_retained_registry(window, current_top)?;
    let mut post = *window;
    post.phase = DIRECT_WINDOW_PHASE_SELECTED;
    post.selected_candidate = current_top[0].entry.candidate_id;
    post.selected_slot = now;
    post.validate()?;

    let mut superseded = [Identity32V1::ZERO; MAX_DIRECT_CANDIDATES - 1];
    let mut i = 1usize;
    while i < current_top.len() {
        superseded[i - 1] = current_top[i].entry.candidate_id;
        i += 1;
    }
    Ok(DirectSelectionPlanV1 {
        post_window: post,
        selected_candidate: current_top[0].entry.candidate_id,
        superseded,
        superseded_count: window.top_count - 1,
    })
}

fn require_submission_time(
    opens_slot: u64,
    closes_slot: u64,
    now: u64,
) -> Result<(), DirectWindowErrorV1> {
    if now < opens_slot {
        Err(DirectWindowErrorV1::BeforeOpen)
    } else if now >= closes_slot {
        Err(DirectWindowErrorV1::SubmissionClosed)
    } else {
        Ok(())
    }
}

fn bind_candidate(
    binding: &DirectWindowBindingV1,
    candidate: &DirectCandidateV2,
) -> Result<(), DirectWindowErrorV1> {
    if candidate.epoch_id != binding.epoch_id
        || candidate.market_id != binding.market_id
        || candidate.order_set_id != binding.order_set_id
        || candidate.policy_id != binding.policy_id
        || candidate.relation_domain_digest != binding.relation_domain_digest
    {
        return Err(DirectWindowErrorV1::MismatchedBinding);
    }
    Ok(())
}

fn validate_registry(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectCandidateV2],
) -> Result<(), DirectWindowErrorV1> {
    if current_top.len() != usize::from(window.top_count) {
        return Err(DirectWindowErrorV1::MismatchedBinding);
    }
    let mut i = 0usize;
    while i < current_top.len() {
        let candidate = current_top[i];
        candidate.validate()?;
        bind_candidate(&window.binding(), &candidate)?;
        if candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED
            || candidate.entry() != window.top[i]
        {
            return Err(DirectWindowErrorV1::MismatchedBinding);
        }
        if i > 0 {
            match current_top[i - 1].score().total_order(&candidate.score()) {
                Ordering::Greater => {}
                _ => return Err(DirectWindowErrorV1::NonCanonical),
            }
        }
        i += 1;
    }
    Ok(())
}

fn validate_retained_registry(
    window: &DirectCandidateWindowV1,
    current_top: &[DirectRetainedCandidateV1],
) -> Result<(), DirectWindowErrorV1> {
    if current_top.len() != usize::from(window.top_count) {
        return Err(DirectWindowErrorV1::MismatchedBinding);
    }
    let mut i = 0usize;
    while i < current_top.len() {
        if current_top[i].entry != window.top[i] {
            return Err(DirectWindowErrorV1::MismatchedBinding);
        }
        if i > 0 {
            match current_top[i - 1].score.total_order(&current_top[i].score) {
                Ordering::Greater => {}
                _ => return Err(DirectWindowErrorV1::NonCanonical),
            }
        }
        i += 1;
    }
    Ok(())
}

fn next_transcript(
    previous: Identity32V1,
    window: &DirectCandidateWindowV1,
    next_count: u64,
    admitted: DirectCandidateEntryV1,
) -> Identity32V1 {
    let mut h = Sha256::new();
    h.update(DIRECT_ADMISSION_TRANSCRIPT_DOMAIN);
    h.update(window.epoch_id.0);
    h.update(window.order_set_id.0);
    h.update(window.policy_id.0);
    h.update(window.relation_domain_digest.0);
    h.update(window.opens_slot.to_le_bytes());
    h.update(window.closes_slot.to_le_bytes());
    h.update(next_count.to_le_bytes());
    h.update(previous.0);
    h.update(admitted.candidate_id.0);
    h.update(admitted.relation_candidate_digest.0);
    Identity32V1(h.finalize().into())
}

struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, at: 0 }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), DirectWindowErrorV1> {
        let end = self
            .at
            .checked_add(bytes.len())
            .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
        let target = self
            .out
            .get_mut(self.at..end)
            .ok_or(DirectWindowErrorV1::WrongLength)?;
        target.copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }

    fn identity(&mut self, value: Identity32V1) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&value.0)
    }

    fn u8(&mut self, value: u8) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn i128(&mut self, value: i128) -> Result<(), DirectWindowErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn finish(self) -> Result<usize, DirectWindowErrorV1> {
        if self.at == self.out.len() {
            Ok(self.at)
        } else {
            Err(DirectWindowErrorV1::WrongLength)
        }
    }
}

struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, at: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DirectWindowErrorV1> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(DirectWindowErrorV1::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.at..end)
            .ok_or(DirectWindowErrorV1::WrongLength)?;
        let mut out = [0u8; N];
        out.copy_from_slice(source);
        self.at = end;
        Ok(out)
    }

    fn identity(&mut self) -> Result<Identity32V1, DirectWindowErrorV1> {
        Ok(Identity32V1(self.array()?))
    }

    fn u8(&mut self) -> Result<u8, DirectWindowErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DirectWindowErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, DirectWindowErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn u128(&mut self) -> Result<u128, DirectWindowErrorV1> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    fn i128(&mut self) -> Result<i128, DirectWindowErrorV1> {
        Ok(i128::from_le_bytes(self.array()?))
    }

    fn finish(self) -> Result<(), DirectWindowErrorV1> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(DirectWindowErrorV1::WrongLength)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        batch_policy_digest, complete_submitted_candidate, verify_submitted_candidate,
        FullRelationDomainV1,
    };
    use clutch_batch::relation_v1::{
        canonical_candidate, BookV1, FrozenPolicyV1, OrderV1, SingleEggOrderV1, PRICE_SCALE,
        RELATION_VERSION_V1,
    };
    use clutch_batch::{PartialPolicy, Side};

    fn id(seed: u8) -> Identity32V1 {
        let mut out = [seed; 32];
        out[31] = seed.wrapping_add(1);
        Identity32V1(out)
    }

    fn policy() -> FrozenPolicyV1 {
        DIRECT_POLICY_V1
    }

    fn domain() -> FullRelationDomainV1 {
        let policy = policy();
        FullRelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: id(1),
            book_id: id(2),
            epoch_id: id(3),
            policy_id: batch_policy_digest(&policy).unwrap(),
            order_set_id: id(5),
            epoch_index: 7,
            outcome_count: 2,
            owner_count: 2,
            price_scale: PRICE_SCALE,
            remainder_seed: 9,
            policy,
        }
    }

    fn book() -> BookV1 {
        let mut book = BookV1::empty();
        book.orders[0] = OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: 1,
            owner: 0,
            outcome: 0,
            side: Side::Buy,
            quantity: 4,
            limit_price: 7_500,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        });
        book.orders[1] = OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: 2,
            owner: 1,
            outcome: 0,
            side: Side::Sell,
            quantity: 4,
            limit_price: 2_500,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        });
        book.len = 2;
        book
    }

    fn verified_at(price: u64, slot: u64) -> DirectCandidateV2 {
        let domain = domain();
        let book = book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = price;
        prices[1] = PRICE_SCALE - price;
        let legacy =
            canonical_candidate(&domain.arithmetic_domain(), &book, &prices, 0, 0).unwrap();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&domain, &legacy).unwrap();
        let (submitted, feed) = complete_submitted_candidate(&domain, &book, &raw, None).unwrap();
        let verified = verify_submitted_candidate(&domain, &book, &submitted, &feed, None).unwrap();
        DirectCandidateV2::from_verified(
            &domain,
            &submitted,
            &verified,
            DirectCandidateCoordinatesV1 {
                submitted_slot: slot,
                buy_index: 0,
                sell_index: 1,
                outcome: 0,
                stored_bump: 7,
            },
        )
        .unwrap()
    }

    #[test]
    fn bounded_two_order_verifier_is_byte_exact_with_full_relation() {
        for (price, slot) in [(2_500, 100), (5_000, 101), (7_500, 102)] {
            let full = verified_at(price, slot);
            let mut prices = [0u64; MAX_OUTCOMES];
            prices[0] = price;
            prices[1] = PRICE_SCALE - price;
            let bounded = verify_direct_two_order_candidate(
                &domain(),
                DirectTwoOrderInputV1 {
                    prices,
                    buy_limit: 7_500,
                    sell_limit: 2_500,
                    quantity: 4,
                    submitted_slot: slot,
                    buy_index: 0,
                    sell_index: 1,
                    outcome: 0,
                    stored_bump: 7,
                },
            )
            .unwrap();
            assert_eq!(bounded, full);

            let reversed = verify_direct_two_order_candidate(
                &domain(),
                DirectTwoOrderInputV1 {
                    buy_index: 1,
                    sell_index: 0,
                    ..DirectTwoOrderInputV1 {
                        prices,
                        buy_limit: 7_500,
                        sell_limit: 2_500,
                        quantity: 4,
                        submitted_slot: slot,
                        buy_index: 0,
                        sell_index: 1,
                        outcome: 0,
                        stored_bump: 7,
                    }
                },
            )
            .unwrap();
            assert_eq!(reversed.score(), full.score());
            assert_eq!(reversed.candidate_id, full.candidate_id);
            assert_eq!(reversed.relation_domain_digest, full.relation_domain_digest);
        }
    }

    fn binding() -> DirectWindowBindingV1 {
        let domain = domain();
        DirectWindowBindingV1 {
            epoch_id: domain.epoch_id,
            market_id: domain.market_id,
            order_set_id: domain.order_set_id,
            policy_id: domain.policy_id,
            relation_domain_digest: domain.digest().unwrap(),
            opens_slot: 100,
            closes_slot: 120,
        }
    }

    fn synthetic(price: u64, slot: u64, primary: i128, digest: Identity32V1) -> DirectCandidateV2 {
        let binding = binding();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = price;
        prices[1] = PRICE_SCALE - price;
        DirectCandidateV2 {
            candidate_id: canonical_account_candidate_id(
                binding.epoch_id,
                binding.market_id,
                &prices,
            ),
            epoch_id: binding.epoch_id,
            market_id: binding.market_id,
            order_set_id: binding.order_set_id,
            policy_id: binding.policy_id,
            relation_domain_digest: binding.relation_domain_digest,
            relation_candidate_digest: digest,
            prices,
            fills: [4, 4],
            weighted_direct_volume: primary,
            limit_surplus_price_units: 20_000,
            submitted_slot: slot,
            quantity: 4,
            buy_index: 0,
            sell_index: 1,
            outcome: 0,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            status: DIRECT_CANDIDATE_STATUS_VERIFIED,
            stored_bump: 7,
            flags: 0,
            reserved: [0; CANDIDATE_RESERVED_BYTES],
        }
    }

    fn apply_plan(
        plan: DirectAdmissionPlanV1,
        top: &mut [DirectCandidateV2; MAX_DIRECT_CANDIDATES],
        top_len: &mut usize,
        mut submitted: DirectCandidateV2,
    ) {
        submitted.status = plan.submitted_status;
        let mut next = [submitted; MAX_DIRECT_CANDIDATES];
        let mut n = 0usize;
        for entry in plan.post_window.top {
            if entry == DirectCandidateEntryV1::ZERO {
                break;
            }
            if entry.candidate_id == submitted.candidate_id {
                next[n] = submitted;
            } else {
                let existing = top[..*top_len]
                    .iter()
                    .find(|candidate| candidate.candidate_id == entry.candidate_id)
                    .copied()
                    .unwrap();
                next[n] = existing;
            }
            n += 1;
        }
        *top = next;
        *top_len = n;
    }

    #[test]
    fn exact_candidate_and_window_bodies_round_trip_and_refuse_padding() {
        let candidate = verified_at(2_500, 100);
        let mut candidate_bytes = [0u8; DIRECT_CANDIDATE_BODY_BYTES];
        assert_eq!(
            candidate.encode_body(&mut candidate_bytes),
            Ok(DIRECT_CANDIDATE_BODY_BYTES)
        );
        assert_eq!(
            DirectCandidateV2::decode_body(&candidate_bytes),
            Ok(candidate)
        );
        assert_eq!(
            DirectCandidateV2::decode_body(&candidate_bytes[..candidate_bytes.len() - 1]),
            Err(DirectWindowErrorV1::WrongLength)
        );
        candidate_bytes[DIRECT_CANDIDATE_BODY_BYTES - 1] = 1;
        assert_eq!(
            DirectCandidateV2::decode_body(&candidate_bytes),
            Err(DirectWindowErrorV1::NonCanonical)
        );

        let window = DirectCandidateWindowV1::first(binding(), &candidate, 100, 8).unwrap();
        let mut window_bytes = [0u8; DIRECT_WINDOW_BODY_BYTES];
        assert_eq!(
            window.encode_body(&mut window_bytes),
            Ok(DIRECT_WINDOW_BODY_BYTES)
        );
        assert_eq!(
            DirectCandidateWindowV1::decode_body(&window_bytes),
            Ok(window)
        );
        window_bytes[DIRECT_WINDOW_BODY_BYTES - 1] = 1;
        assert_eq!(
            DirectCandidateWindowV1::decode_body(&window_bytes),
            Err(DirectWindowErrorV1::NonCanonical)
        );
    }

    #[test]
    fn immutable_boundaries_replay_capacity_and_replacement_are_fail_closed() {
        let first = synthetic(1_000, 100, 10, id(30));
        assert_eq!(
            DirectCandidateWindowV1::first(binding(), &first, 99, 8),
            Err(DirectWindowErrorV1::BeforeOpen)
        );
        let mut window = DirectCandidateWindowV1::first(binding(), &first, 100, 8).unwrap();
        assert_eq!(
            plan_admission(&window, &[first], &first, 100),
            Err(DirectWindowErrorV1::Replay)
        );

        let mut top = [first; MAX_DIRECT_CANDIDATES];
        let mut top_len = 1usize;
        for (price, primary, digest_seed, slot) in [
            (2_000, 20, 31, 101),
            (3_000, 30, 32, 102),
            (4_000, 5, 33, 103),
        ] {
            let candidate = synthetic(price, slot, primary, id(digest_seed));
            let plan = plan_admission(&window, &top[..top_len], &candidate, slot).unwrap();
            window = plan.post_window;
            apply_plan(plan, &mut top, &mut top_len, candidate);
        }
        assert_eq!(window.admitted_count, 4);
        assert_eq!(top_len, 3);
        // The fourth, worse candidate was admitted into the transcript and
        // immediately superseded, without occupying retained capacity.
        assert_eq!(top[0].weighted_direct_volume, 30);
        assert_eq!(top[1].weighted_direct_volume, 20);
        assert_eq!(top[2].weighted_direct_volume, 10);

        let better = synthetic(5_000, 104, 40, id(34));
        let replacement = plan_admission(&window, &top[..top_len], &better, 104).unwrap();
        assert_eq!(replacement.displaced_candidate, first.candidate_id);
        window = replacement.post_window;
        apply_plan(replacement, &mut top, &mut top_len, better);
        assert_eq!(top[0].weighted_direct_volume, 40);
        assert_eq!(window.admitted_count, 5);

        let before = window;
        let late = synthetic(6_000, 120, 50, id(35));
        assert_eq!(
            plan_admission(&window, &top[..top_len], &late, 120),
            Err(DirectWindowErrorV1::SubmissionClosed)
        );
        assert_eq!(window, before);
        assert_eq!(
            plan_selection(&window, &top[..top_len], 119),
            Err(DirectWindowErrorV1::SelectionEarly)
        );
        assert_eq!(window, before);

        let selected = plan_selection(&window, &top[..top_len], 150).unwrap();
        assert_eq!(selected.selected_candidate, top[0].candidate_id);
        assert_eq!(selected.post_window.selected_slot, 150);
        assert_eq!(
            plan_selection(&selected.post_window, &top[..top_len], 151),
            Err(DirectWindowErrorV1::AlreadySelected)
        );
    }

    #[test]
    fn crossed_book_tie_and_streaming_top_three_are_order_independent() {
        let low_price = verified_at(2_500, 100);
        let high_price = verified_at(7_500, 101);
        assert_eq!(
            low_price.weighted_direct_volume,
            high_price.weighted_direct_volume
        );
        assert_eq!(
            low_price.limit_surplus_price_units,
            high_price.limit_surplus_price_units
        );
        assert_eq!(low_price.distinct_owners, high_price.distinct_owners);
        assert_ne!(
            low_price.relation_candidate_digest,
            high_price.relation_candidate_digest
        );
        let expected = if low_price.score().is_better_than(&high_price.score()) {
            low_price.candidate_id
        } else {
            high_price.candidate_id
        };

        let first = DirectCandidateWindowV1::first(binding(), &low_price, 100, 8).unwrap();
        let added = plan_admission(&first, &[low_price], &high_price, 101).unwrap();
        let forward_top = if added.post_window.top[0].candidate_id == low_price.candidate_id {
            [low_price, high_price]
        } else {
            [high_price, low_price]
        };
        let forward = plan_selection(&added.post_window, &forward_top, 120).unwrap();

        let high_first = DirectCandidateV2 {
            submitted_slot: 100,
            ..high_price
        };
        let low_second = DirectCandidateV2 {
            submitted_slot: 101,
            ..low_price
        };
        let reverse_window =
            DirectCandidateWindowV1::first(binding(), &high_first, 100, 8).unwrap();
        let reverse_added =
            plan_admission(&reverse_window, &[high_first], &low_second, 101).unwrap();
        let reverse_top =
            if reverse_added.post_window.top[0].candidate_id == low_second.candidate_id {
                [low_second, high_first]
            } else {
                [high_first, low_second]
            };
        let reverse = plan_selection(&reverse_added.post_window, &reverse_top, 120).unwrap();
        assert_eq!(forward.selected_candidate, expected);
        assert_eq!(reverse.selected_candidate, expected);
        assert_eq!(added.post_window.top, reverse_added.post_window.top);
        // The audit transcript records arrival order; it is not falsely called
        // a commutative set commitment.
        assert_ne!(
            added.post_window.admission_transcript,
            reverse_added.post_window.admission_transcript
        );

        // Pin that the total order reads digest bytes outside a 128-bit prefix.
        let mut left_digest = [9u8; 32];
        let mut right_digest = left_digest;
        left_digest[20] = 1;
        right_digest[20] = 2;
        let left = synthetic(4_000, 105, 77, Identity32V1(left_digest));
        let right = synthetic(6_000, 106, 77, Identity32V1(right_digest));
        assert!(left.score().is_better_than(&right.score()));
    }

    #[test]
    fn every_five_candidate_arrival_permutation_retains_the_same_top_three() {
        let prototypes = [
            synthetic(1_000, 100, 10, id(40)),
            synthetic(2_000, 100, 50, id(41)),
            synthetic(3_000, 100, 30, id(42)),
            synthetic(4_000, 100, 20, id(43)),
            synthetic(5_000, 100, 40, id(44)),
        ];
        let mut permutations = 0usize;
        for a in 0..5 {
            for b in 0..5 {
                for c in 0..5 {
                    for d in 0..5 {
                        for e in 0..5 {
                            let order = [a, b, c, d, e];
                            let mut distinct = true;
                            let mut i = 0usize;
                            while i < order.len() {
                                let mut j = 0usize;
                                while j < i {
                                    distinct &= order[i] != order[j];
                                    j += 1;
                                }
                                i += 1;
                            }
                            if !distinct {
                                continue;
                            }
                            permutations += 1;

                            let first = DirectCandidateV2 {
                                submitted_slot: 100,
                                ..prototypes[order[0]]
                            };
                            let mut window =
                                DirectCandidateWindowV1::first(binding(), &first, 100, 8).unwrap();
                            let mut top = [first; MAX_DIRECT_CANDIDATES];
                            let mut top_len = 1usize;
                            let mut arrival = 1usize;
                            while arrival < order.len() {
                                let slot = 100 + arrival as u64;
                                let candidate = DirectCandidateV2 {
                                    submitted_slot: slot,
                                    ..prototypes[order[arrival]]
                                };
                                let plan =
                                    plan_admission(&window, &top[..top_len], &candidate, slot)
                                        .unwrap();
                                window = plan.post_window;
                                apply_plan(plan, &mut top, &mut top_len, candidate);
                                arrival += 1;
                            }
                            assert_eq!(window.admitted_count, 5);
                            assert_eq!(top_len, 3);
                            assert_eq!(top[0].weighted_direct_volume, 50);
                            assert_eq!(top[1].weighted_direct_volume, 40);
                            assert_eq!(top[2].weighted_direct_volume, 30);
                            let selected = plan_selection(&window, &top[..top_len], 120).unwrap();
                            assert_eq!(selected.selected_candidate, prototypes[1].candidate_id);
                        }
                    }
                }
            }
        }
        assert_eq!(permutations, 120);
    }

    #[test]
    fn substitution_and_registry_omission_leave_the_window_unchanged() {
        let first = verified_at(2_500, 100);
        let window = DirectCandidateWindowV1::first(binding(), &first, 100, 8).unwrap();
        let second = verified_at(7_500, 101);
        let before = window;
        assert_eq!(
            plan_admission(&window, &[], &second, 101),
            Err(DirectWindowErrorV1::MismatchedBinding)
        );
        assert_eq!(window, before);

        let mut substituted = second;
        substituted.policy_id.0[31] ^= 1;
        assert_eq!(
            plan_admission(&window, &[first], &substituted, 101),
            Err(DirectWindowErrorV1::MismatchedBinding)
        );
        assert_eq!(window, before);
    }

    #[test]
    fn compact_projection_refuses_every_unrepresented_policy_family() {
        let baseline = domain();
        let book = book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = 2_500;
        prices[1] = 7_500;
        let variants = [
            FrozenPolicyV1 {
                allocation: AllocationPolicyV1::FullProRata,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                self_cross: SelfCrossPolicyV1::RefuseOverlap,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                aon: AonPolicyV1::FullSizeCounting,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                rounding: RoundingBoundaryV1::ReceiptFloor,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::FullPairOnly,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                transfer_phase: TransferPhaseV1::ActiveOnly,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                dust: DustPolicy::AssignCanonical,
                ..DIRECT_POLICY_V1
            },
            FrozenPolicyV1 {
                fee_base: FeeBaseV1::FlatNotional { bps: 1 },
                ..DIRECT_POLICY_V1
            },
        ];
        for policy in variants {
            let mut changed = baseline;
            changed.policy = policy;
            changed.policy_id = batch_policy_digest(&policy).unwrap();
            if let Ok(legacy) =
                canonical_candidate(&changed.arithmetic_domain(), &book, &prices, 0, 0)
            {
                let raw =
                    FullSubmittedCandidateV1::from_relation_candidate(&changed, &legacy).unwrap();
                if let Ok((submitted, feed)) =
                    complete_submitted_candidate(&changed, &book, &raw, None)
                {
                    if let Ok(verified) =
                        verify_submitted_candidate(&changed, &book, &submitted, &feed, None)
                    {
                        assert_eq!(
                            DirectCandidateV2::from_verified(
                                &changed,
                                &submitted,
                                &verified,
                                DirectCandidateCoordinatesV1 {
                                    submitted_slot: 100,
                                    buy_index: 0,
                                    sell_index: 1,
                                    outcome: 0,
                                    stored_bump: 7,
                                }
                            ),
                            Err(DirectWindowErrorV1::NotDirect)
                        );
                    }
                }
            }
        }
    }
}
