//! Executable authority and conservation model for bounded direct selection V3.
//!
//! V3 begins as an unfrozen Epoch that may own zero, one, or two Reservation V2
//! accounts. Full candidate verification is staged only after an atomic Freeze,
//! live Candidate accounts are bounded to three, every transient account
//! records its rent payer and exact payer-funded principal, unsolicited lamports
//! go to an immutable neutral sink, and every pre-freeze or frozen phase has a
//! bounded terminal path.

use clutch_batch::relation_v1::MAX_OUTCOMES;
use clutch_liveness::{DonationLedger, Id as LivenessId};
use sha2::{Digest, Sha256};

use crate::{
    batch_policy_digest, canonical_batch_policy_bytes,
    direct_window_v1::{
        DirectCandidateEntryV1, DirectCandidateV2, DirectTwoOrderInputV1, DirectWindowErrorV1,
        ValidatedDirectDomainV1, DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_POLICY_V1,
        MAX_DIRECT_CANDIDATES,
    },
    FullRelationDomainV1, FullScoreV1, Identity32V1, RELATION_VERSION_V1,
};

/// Maximum frozen ticks admitted by the replay bitmap.
pub const MAX_DIRECT_TICKS_V3: u8 = 64;
/// Minimum half-open submission span in slots.
pub const MIN_SUBMISSION_SPAN_V3: u64 = 2;
/// Minimum selection span: Begin + three Verify + Finalize slot opportunities.
pub const MIN_SELECTION_SPAN_V3: u64 = 5;
/// Minimum post-selection settlement span in slots.
pub const MIN_SETTLEMENT_SPAN_V3: u64 = 2;
/// Maximum research-policy submission span (roughly one day at 400 ms/slot).
pub const MAX_SUBMISSION_SPAN_V3: u64 = 216_000;
/// Maximum research-policy staged-selection span.
pub const MAX_SELECTION_SPAN_V3: u64 = 21_600;
/// Maximum research-policy post-selection span.
pub const MAX_SETTLEMENT_SPAN_V3: u64 = 216_000;

const TRANSCRIPT_DOMAIN_V3: &[u8] = b"dragons-clutch/direct-lifecycle-v3/admission\0";
const RESERVATION_DOMAIN_V1: &[u8] = b"dragons-clutch/reservation/v1";
const PRICE_GRID_DOMAIN_V1: &[u8] = b"dragons-clutch/price-grid/v1";
const ORDER_PAGE_DOMAIN_V3: &[u8] = b"dragons-clutch/order-page/v3";
const ORDER_SET_DOMAIN_V1: &[u8] = b"dragons-clutch/order-set/v1";
const DIRECT_BATCH_POLICY_V3_DOMAIN: &[u8] = b"dragons-clutch/direct-batch-policy/v3\0";
const DIRECT_ORDER_SLOT_BYTES_V3: usize = 236;
/// Exact live single-order record body width: the layout's `ORDER_RECORD_BYTES`
/// (owner 32, order id 32, outcome 1, side 1, quantity 8, limit 8, minimum
/// fill 8, flags 1, generation 8, expiry epoch 8). A cross-crate tripwire in
/// `clutch-solana-layout` asserts the resulting page digest byte-matches the
/// live page fold.
const DIRECT_ORDER_BODY_BYTES_V3: usize = 107;
const DIRECT_PAGE_SLOT_COUNT_V3: usize = 16;

/// Lifecycle phase of one successfully frozen direct epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLifecyclePhaseV3 {
    /// Frozen reservations exist, but no competitive Candidate exists.
    FrozenEmpty,
    /// A bounded Window accepts competitive submissions.
    WindowOpen,
    /// The exact retained prefix is being re-executed across transactions.
    Verifying,
    /// Candidate zero is selected and reservations are ENTITLED.
    Selected,
    /// Transient authority is closed; the Epoch receipt remains.
    Terminal,
}

/// Candidate account status owned by the staged V3 lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCandidateStageV3 {
    /// Submit performed the complete relation verification.
    Verified,
    /// A later staged Verify transaction independently re-executed it.
    Reverified,
    /// The exact winner.
    Selected,
}

/// Durable terminal reason stored in the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectTerminalReasonV3 {
    /// The Epoch never froze; every present OPEN reservation was released.
    PrefreezeAbort,
    /// No competitive Candidate existed at the selection deadline.
    EmptyLapse,
    /// A non-empty Window was not selected before its deadline.
    PreSelectionLapse,
    /// Selected authority was not settled before its deadline.
    PostSelectionLapse,
    /// The selected pair settled atomically.
    Settled,
}

/// Reservation ownership transition authorized by one lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReservationTransitionV3 {
    /// No reservation mutation.
    None,
    /// Selection freezes both ACTIVE envelopes as settlement entitlements.
    ActiveToEntitled,
    /// Settlement consumes both ENTITLED envelopes.
    EntitledToConsumed,
    /// Empty or pre-selection lapse returns the active reservation prefix.
    ActiveToReleased,
    /// Post-selection lapse returns both ENTITLED envelopes.
    EntitledToReleased,
}

/// Reservation state tracked by the executable lifecycle model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReservationStateV3 {
    /// Both exact reservations are ACTIVE.
    Active,
    /// Both exact reservations are ENTITLED to the selected slice.
    Entitled,
    /// Both exact reservations reached CONSUMED or RELEASED terminality.
    Terminal,
}

/// Explicit outcome of a valid submission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAdmissionDispositionV3 {
    /// First competitive Candidate created Candidate and Window authority.
    First,
    /// Candidate joined a not-yet-full retained prefix.
    Retained,
    /// Candidate displaced and closed the former worst.
    Replaced,
    /// Valid Candidate could not beat a full top three; no authority was made.
    RejectedNonCompetitive,
}

/// Refusal from the bounded lifecycle model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLifecycleErrorV3 {
    /// A zero identity, zero amount, malformed padding, or invalid phase shape.
    NonCanonical,
    /// The requested transition is unavailable in the current phase.
    WrongPhase,
    /// Submission lies outside the immutable half-open window.
    SubmissionClosed,
    /// Begin, Verify, or Finalize lies outside the selection window.
    SelectionWindow,
    /// Settlement lies before selection or at/after its immutable deadline.
    SettlementClosed,
    /// Pre-freeze placement/Freeze or bounded abort lies on the wrong side of
    /// immutable submission-open.
    PrefreezeWindow,
    /// The canonical price tick was already competitively admitted.
    Replay,
    /// A verification bit was replayed or the exact mask is incomplete.
    VerificationIncomplete,
    /// An identity, candidate, balance, status, or receipt does not bind.
    MismatchedBinding,
    /// The prepaid work budget cannot cover its frozen obligation.
    WorkBudgetInsufficient,
    /// A bounded counter or lamport sum overflowed.
    ArithmeticOverflow,
    /// Full direct relation or frozen-grid verification refused.
    RelationRefused,
    /// Canonical liveness donation accounting refused.
    LivenessRefused,
}

impl From<DirectWindowErrorV1> for DirectLifecycleErrorV3 {
    fn from(_: DirectWindowErrorV1) -> Self {
        Self::RelationRefused
    }
}

impl From<clutch_liveness::Error> for DirectLifecycleErrorV3 {
    fn from(_: clutch_liveness::Error) -> Self {
        Self::LivenessRefused
    }
}

/// Immutable lifecycle deadlines supplied by the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectScheduleV3 {
    /// First accepted submission slot.
    pub submission_opens_slot: u64,
    /// Exclusive submission close and first selection slot.
    pub submission_closes_slot: u64,
    /// Exclusive selection deadline and first unselected lapse slot.
    pub selection_deadline_slot: u64,
    /// Exclusive settlement deadline and first selected lapse slot.
    pub settlement_deadline_slot: u64,
}

impl DirectScheduleV3 {
    /// Validate ordering plus versioned minimum and maximum horizons.
    pub fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        let submission = self
            .submission_closes_slot
            .checked_sub(self.submission_opens_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        let selection = self
            .selection_deadline_slot
            .checked_sub(self.submission_closes_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        let settlement = self
            .settlement_deadline_slot
            .checked_sub(self.selection_deadline_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        if !(MIN_SUBMISSION_SPAN_V3..=MAX_SUBMISSION_SPAN_V3).contains(&submission)
            || !(MIN_SELECTION_SPAN_V3..=MAX_SELECTION_SPAN_V3).contains(&selection)
            || !(MIN_SETTLEMENT_SPAN_V3..=MAX_SETTLEMENT_SPAN_V3).contains(&settlement)
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Frozen keeper rewards paid from WorkBudget, never from rent principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectKeeperRewardsV3 {
    /// Reward for closing submissions.
    pub begin_verification: u64,
    /// Reward for each successful retained-candidate re-execution.
    pub verify_candidate: u64,
    /// Reward for final selection and entitlement creation.
    pub finalize_selection: u64,
    /// Reward for completed atomic settlement.
    pub settle: u64,
    /// Reward for any permissionless lapse transition.
    pub lapse: u64,
}

impl DirectKeeperRewardsV3 {
    /// Canonical absence before successful Freeze.
    pub const ZERO: Self = Self {
        begin_verification: 0,
        verify_candidate: 0,
        finalize_selection: 0,
        settle: 0,
        lapse: 0,
    };

    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.begin_verification == 0
            || self.verify_candidate == 0
            || self.finalize_selection == 0
            || self.settle == 0
            || self.lapse == 0
        {
            return Err(DirectLifecycleErrorV3::WorkBudgetInsufficient);
        }
        self.worst_case().map(|_| ())
    }

    fn worst_case(self) -> Result<u64, DirectLifecycleErrorV3> {
        let verification = self
            .verify_candidate
            .checked_mul(MAX_DIRECT_CANDIDATES as u64)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let selected_path = self
            .begin_verification
            .checked_add(verification)
            .and_then(|value| value.checked_add(self.finalize_selection))
            .and_then(|value| value.checked_add(self.settle.max(self.lapse)))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(selected_path.max(self.lapse))
    }
}

/// Exact fixed grid view authenticated by the live PriceGrid account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGridV3 {
    /// Self-certifying identity of the exact 553-byte grid body.
    pub grid_id: Identity32V1,
    /// Realm namespace bound into that body.
    pub realm_id: Identity32V1,
    /// Exact simplex scale.
    pub price_scale: u64,
    /// Active prefix in `ticks`.
    pub tick_count: u8,
    /// Strictly increasing active ticks and canonical zero padding.
    pub ticks: [u64; MAX_DIRECT_TICKS_V3 as usize],
    /// Stored PDA bump; deliberately outside the grid digest.
    pub stored_bump: u8,
    /// Reserved flags; exactly zero.
    pub flags: u8,
}

impl DirectGridV3 {
    /// Validate the bounded exact-membership grid.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        if self.realm_id.is_zero()
            || self.price_scale == 0
            || self.tick_count < 2
            || self.tick_count > MAX_DIRECT_TICKS_V3
            || self.flags != 0
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < self.ticks.len() {
            if index < usize::from(self.tick_count) {
                if self.ticks[index] > self.price_scale
                    || (index > 0 && self.ticks[index] <= self.ticks[index - 1])
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
            } else if self.ticks[index] != 0 {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        if self.grid_id != self.recomputed_grid_id() {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    /// Recompute the exact live PriceGrid body identity without an adapter
    /// projection or variable-width preimage.
    pub fn recomputed_grid_id(&self) -> Identity32V1 {
        let mut hasher = Sha256::new();
        hasher.update(PRICE_GRID_DOMAIN_V1);
        hasher.update(self.realm_id.0);
        hasher.update(self.price_scale.to_le_bytes());
        hasher.update([self.tick_count]);
        let mut index = 0usize;
        while index < self.ticks.len() {
            hasher.update(self.ticks[index].to_le_bytes());
            index += 1;
        }
        Identity32V1(hasher.finalize().into())
    }

    fn tick_of(&self, price: u64) -> Result<u8, DirectLifecycleErrorV3> {
        self.validate()?;
        let mut index = 0usize;
        while index < usize::from(self.tick_count) {
            if self.ticks[index] == price {
                return u8::try_from(index).map_err(|_| DirectLifecycleErrorV3::ArithmeticOverflow);
            }
            index += 1;
        }
        Err(DirectLifecycleErrorV3::RelationRefused)
    }
}

/// Exact payer-funded rent principal for one transient account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRentPrincipalV3 {
    /// Authenticated payer receiving only its exact contribution.
    pub payer: Identity32V1,
    /// Exact lamports debited from that payer after prefund normalization.
    pub lamports: u64,
}

impl DirectRentPrincipalV3 {
    /// Canonical absent account.
    pub const ZERO: Self = Self {
        payer: Identity32V1::ZERO,
        lamports: 0,
    };

    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.payer.is_zero() || self.lamports == 0 {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Canonical liveness ledger plus the separately owned payer rent principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAccountLedgerV3 {
    /// Exact payer-funded principal. Donations never reduce this amount.
    pub rent: DirectRentPrincipalV3,
    /// Canonical semantic owner for prebalance and later unsolicited lamports.
    pub donation: Option<DonationLedger>,
}

impl DirectAccountLedgerV3 {
    /// Canonical absent account.
    pub const ZERO: Self = Self {
        rent: DirectRentPrincipalV3::ZERO,
        donation: None,
    };

    /// Reconstruct the canonical semantic owner from hostile persisted fields.
    ///
    /// No new payer transfer occurs while decoding, so the exact delta is zero;
    /// the stored lower bound remains entirely neutral donation.
    pub fn restore(
        rent: DirectRentPrincipalV3,
        sink: Identity32V1,
        donation_lamports: u64,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        rent.validate()?;
        let donation = DonationLedger::admit_prefunded(
            LivenessId::from_bytes(rent.payer.0),
            LivenessId::from_bytes(sink.0),
            donation_lamports,
            0,
            donation_lamports,
        )?;
        Ok(Self {
            rent,
            donation: Some(donation),
        })
    }

    fn validate(self, sink: Identity32V1) -> Result<(), DirectLifecycleErrorV3> {
        self.rent.validate()?;
        let donation = self.donation.ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        if sink.is_zero() || self.rent.payer == sink || donation.neutral_sink().bytes() != sink.0 {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    /// Donation amount last authenticated by the canonical liveness kernel.
    pub fn donation_lamports(self) -> Result<u64, DirectLifecycleErrorV3> {
        self.donation
            .map(DonationLedger::donation_lamports)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)
    }

    /// Observe one surviving account without crediting donations to principal.
    ///
    /// `protected_lamports` is an independently owned live compartment, such
    /// as the remaining WorkBudget reward balance. The prior donation lower
    /// bound is monotone and an observed shortfall refuses.
    pub fn observe(
        self,
        actual_balance: u64,
        protected_lamports: u64,
        sink: Identity32V1,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        self.validate(sink)?;
        let accounted = self
            .rent
            .lamports
            .checked_add(protected_lamports)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let donation = self
            .donation
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?
            .observe(actual_balance, accounted)?;
        Ok(Self {
            rent: self.rent,
            donation: Some(donation),
        })
    }
}

/// One exact create/allocate/assign request over a predictable PDA target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCreationFundingV3 {
    /// Exact payer principal deposited without crediting the prior balance.
    pub rent: DirectRentPrincipalV3,
    /// Target balance before the exact payer deposit.
    pub balance_before: u64,
    /// Target balance after the exact payer deposit.
    pub balance_after: u64,
}

impl DirectCreationFundingV3 {
    /// Canonical absent creation.
    pub const ZERO: Self = Self {
        rent: DirectRentPrincipalV3::ZERO,
        balance_before: 0,
        balance_after: 0,
    };

    fn account(
        self,
        sink: Identity32V1,
        extra_accounted_lamports: u64,
    ) -> Result<DirectAccountLedgerV3, DirectLifecycleErrorV3> {
        self.rent.validate()?;
        let exact_deposit = self
            .rent
            .lamports
            .checked_add(extra_accounted_lamports)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let donation = DonationLedger::admit_prefunded(
            LivenessId::from_bytes(self.rent.payer.0),
            LivenessId::from_bytes(sink.0),
            self.balance_before,
            exact_deposit,
            self.balance_after,
        )?;
        Ok(DirectAccountLedgerV3 {
            rent: self.rent,
            donation: Some(donation),
        })
    }
}

/// One Candidate created only from the complete direct verifier and exact grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateLeaseV3 {
    /// Byte-exact V2 Candidate body; V3 adds rent fields around this body.
    pub candidate: DirectCandidateV2,
    /// Canonical grid tick of the selected outcome price.
    pub tick: u8,
    /// Staged status stored in the V3 Candidate status byte.
    pub stage: DirectCandidateStageV3,
    /// Exact rent principal plus canonical monotone donation ownership.
    pub account: DirectAccountLedgerV3,
}

/// Source facts independently authenticated from the frozen page and Epoch.
///
/// Candidate-controlled coordinates are intentionally excluded except for the
/// proposed prices, submission slot, and PDA bump.  Every economic coordinate
/// below is re-derived from the frozen authority before a persisted Candidate
/// can be admitted or reverified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectVerificationFactsV3 {
    buy_limit: u64,
    sell_limit: u64,
    quantity: u64,
    buy_index: u8,
    sell_index: u8,
    outcome: u8,
    submission_opens_slot: u64,
    submission_closes_slot: u64,
}

impl DirectCandidateLeaseV3 {
    /// Canonical inactive slot.
    pub const ZERO: Self = Self {
        candidate: DirectCandidateV2 {
            candidate_id: Identity32V1::ZERO,
            epoch_id: Identity32V1::ZERO,
            market_id: Identity32V1::ZERO,
            order_set_id: Identity32V1::ZERO,
            policy_id: Identity32V1::ZERO,
            relation_domain_digest: Identity32V1::ZERO,
            relation_candidate_digest: Identity32V1::ZERO,
            prices: [0; MAX_OUTCOMES],
            fills: [0; 2],
            weighted_direct_volume: 0,
            limit_surplus_price_units: 0,
            submitted_slot: 0,
            quantity: 0,
            buy_index: 0,
            sell_index: 0,
            outcome: 0,
            distinct_owners: 0,
            order_len: 0,
            outcome_count: 0,
            status: 0,
            stored_bump: 0,
            flags: 0,
            reserved: [0; 12],
        },
        tick: 0,
        stage: DirectCandidateStageV3::Verified,
        account: DirectAccountLedgerV3::ZERO,
    };

    /// Stage an admission lease only after byte-exact relation reexecution.
    ///
    /// This pure constructor proves the proposed create delta and DonationLedger;
    /// it does not debit the payer or create an account. `admit` decides whether
    /// the staged create executes. A `RejectedNonCompetitive` result executes
    /// neither the create nor the payer deposit.
    pub fn issue(
        domain: &ValidatedDirectDomainV1,
        grid: &DirectGridV3,
        candidate: DirectCandidateV2,
        input: DirectTwoOrderInputV1,
        creation: DirectCreationFundingV3,
        neutral_sink: Identity32V1,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        candidate.validate()?;
        domain.reverify_decoded_candidate(&candidate, input)?;
        if grid.price_scale
            != candidate.prices[0]
                .checked_add(candidate.prices[1])
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        grid.tick_of(candidate.prices[0])?;
        grid.tick_of(candidate.prices[1])?;
        let tick = grid.tick_of(candidate.prices[usize::from(candidate.outcome)])?;
        let account = creation.account(neutral_sink, 0)?;
        Ok(Self {
            candidate,
            tick,
            stage: DirectCandidateStageV3::Verified,
            account,
        })
    }

    fn validate_body(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.candidate.validate()?;
        if self.candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED
            || self.tick >= MAX_DIRECT_TICKS_V3
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }

    fn entry(&self) -> DirectCandidateEntryV1 {
        self.candidate.entry()
    }

    fn score(&self) -> FullScoreV1 {
        self.candidate.score()
    }
}

/// Immutable identities every staged transaction must authenticate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleAuthorityV3 {
    /// Frozen compile-time verifier release identifier from BatchPolicy.
    ///
    /// This is not an onchain code hash or deployment identity.
    pub verifier_release_id: Identity32V1,
    /// Epoch-bound DirectBatchPolicy V3 artifact identity.
    ///
    /// Commits the canonical 64-byte direct policy bytes and
    /// `verifier_release_id` under the exact Epoch context; disjoint from the
    /// legacy BatchPolicy digest carried by `policy_id`. Every state validate
    /// recomputes it via [`direct_policy_v3_digest`], so the release
    /// identifier is anchored to a content-addressed artifact rather than
    /// standing as free input.
    pub direct_policy_v3_id: Identity32V1,
    /// Realm-authenticated destination for every unsolicited lamport.
    pub neutral_lamport_sink: Identity32V1,
}

impl DirectLifecycleAuthorityV3 {
    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.verifier_release_id.is_zero()
            || self.direct_policy_v3_id.is_zero()
            || self.neutral_lamport_sink.is_zero()
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }

    fn validate_for_epoch(self, epoch_id: Identity32V1) -> Result<(), DirectLifecycleErrorV3> {
        self.validate()?;
        if self.direct_policy_v3_id != direct_policy_v3_digest(epoch_id, self.verifier_release_id)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }
}

/// Epoch-bound DirectBatchPolicy V3 artifact identity over the canonical
/// direct policy bytes and one compile-time verifier release identifier.
///
/// Byte-identical to the layout crate's `DirectBatchPolicyV3::digest_for_epoch`
/// preimage: domain, 32-byte Epoch identity, 64 canonical policy bytes, then
/// the 32-byte release identifier. A cross-crate test in `clutch-solana-layout`
/// asserts the equality.
pub fn direct_policy_v3_digest(
    epoch_id: Identity32V1,
    verifier_release_id: Identity32V1,
) -> Result<Identity32V1, DirectLifecycleErrorV3> {
    if epoch_id.is_zero() || verifier_release_id.is_zero() {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    let policy_bytes = canonical_batch_policy_bytes(&DIRECT_POLICY_V1)
        .map_err(|_| DirectLifecycleErrorV3::MismatchedBinding)?;
    let mut hasher = Sha256::new();
    hasher.update(DIRECT_BATCH_POLICY_V3_DOMAIN);
    hasher.update(epoch_id.0);
    hasher.update(policy_bytes);
    hasher.update(verifier_release_id.0);
    Ok(Identity32V1(hasher.finalize().into()))
}

/// Slot and verifier release identifier supplied to a semantics-bearing transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTransitionContextV3 {
    /// Authenticated Clock slot.
    pub now: u64,
    /// Exact compile-time verifier release identifier checked by this binary.
    pub verifier_release_id: Identity32V1,
}

/// WorkBudget account funding, kept separate from spendable rewards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectWorkBudgetFundingV3 {
    /// Reward sponsor receiving unused spendable rewards.
    pub reward_sponsor: Identity32V1,
    /// Exact create transition over the target's prior donation balance.
    pub creation: DirectCreationFundingV3,
    /// Exact reward-only lamports deposited by the sponsor.
    pub reward_lamports: u64,
}

/// Complete authenticated pre-freeze placement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationPlacementV3 {
    /// Exact page record at the next canonical prefix rank.
    pub order: DirectPrefreezeOrderV3,
    /// Authenticated live Position prestate.
    pub position: DirectPositionV3,
    /// Signed fee headroom included in a buy reservation.
    pub max_fee_atoms: u64,
    /// Reservation PDA bump.
    pub stored_bump: u8,
    /// Exact Reservation V2 account funding.
    pub creation: DirectCreationFundingV3,
}

/// Funding for receipt and pot creation at final selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionFundingV3 {
    /// Exact receipt creation funding.
    pub receipt: DirectCreationFundingV3,
    /// Exact pot creation funding.
    pub pot: DirectCreationFundingV3,
}

/// Live balances observed for accounts a transition closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectObservedBalancesV3 {
    /// Candidate balances in retained slot order.
    pub candidates: [u64; MAX_DIRECT_CANDIDATES],
    /// Window live balance.
    pub window: u64,
    /// Receipt live balance.
    pub receipt: u64,
    /// Pot live balance.
    pub pot: u64,
    /// WorkBudget balance before this transition's keeper reward.
    pub work_budget: u64,
    /// Exact two Reservation live balances.
    pub reservations: [u64; 2],
}

/// Exact immutable facts shared by a V4 pre-freeze page and Reservation V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationDomainV3 {
    /// Market identity.
    pub market_id: Identity32V1,
    /// Epoch identity.
    pub epoch_id: Identity32V1,
    /// Full frozen book identity.
    pub book_id: Identity32V1,
    /// Frozen price-grid identity.
    pub price_grid_id: Identity32V1,
    /// Immutable settlement terms.
    pub terms_id: Identity32V1,
    /// Legacy full-width BatchPolicy digest bound into the relation domain and
    /// every candidate/reservation body. The epoch-bound DirectBatchPolicy V3
    /// artifact identity is the disjoint
    /// [`DirectLifecycleAuthorityV3::direct_policy_v3_id`].
    pub policy_id: Identity32V1,
    /// Epoch index used for exact order-expiry admission.
    pub epoch_index: u64,
    /// Frozen complete-set scale.
    pub price_scale: u64,
    /// Active outcome prefix.
    pub outcome_count: u8,
    /// Frozen largest-remainder permutation seed.
    pub remainder_seed: u64,
}

impl DirectReservationDomainV3 {
    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.market_id.is_zero()
            || self.epoch_id.is_zero()
            || self.book_id.is_zero()
            || self.price_grid_id.is_zero()
            || self.terms_id.is_zero()
            || self.policy_id.is_zero()
            || self.price_scale == 0
            || self.outcome_count != 2
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }

    fn full_relation_domain(
        self,
        order_set_id: Identity32V1,
    ) -> Result<FullRelationDomainV1, DirectLifecycleErrorV3> {
        self.validate()?;
        let domain = FullRelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: self.market_id,
            book_id: self.book_id,
            epoch_id: self.epoch_id,
            policy_id: self.policy_id,
            order_set_id,
            epoch_index: self.epoch_index,
            outcome_count: self.outcome_count,
            owner_count: 2,
            price_scale: self.price_scale,
            remainder_seed: self.remainder_seed,
            policy: DIRECT_POLICY_V1,
        };
        if self.policy_id
            != batch_policy_digest(&DIRECT_POLICY_V1)
                .map_err(|_| DirectLifecycleErrorV3::MismatchedBinding)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        domain
            .validate()
            .map_err(|_| DirectLifecycleErrorV3::MismatchedBinding)?;
        Ok(domain)
    }
}

/// Exact direct single-order facts authenticated from page-zero prefix bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeOrderV3 {
    /// Position owner.
    pub owner: Identity32V1,
    /// Canonical positional order identity.
    pub order_id: Identity32V1,
    /// Egg outcome.
    pub outcome: u8,
    /// Zero buy, one sell.
    pub side: u8,
    /// Exact quantity.
    pub quantity: u64,
    /// Limit on the frozen scale.
    pub limit: u64,
    /// Minimum fill quantity.
    pub minimum_fill: u64,
    /// Bit zero is all-or-none; all other bits are reserved.
    pub flags: u8,
    /// Replay generation.
    pub generation: u64,
    /// Last admissible Epoch index, inclusive.
    pub expiry_epoch: u64,
}

impl DirectPrefreezeOrderV3 {
    /// Canonical empty order-page suffix.
    pub const ZERO: Self = Self {
        owner: Identity32V1::ZERO,
        order_id: Identity32V1::ZERO,
        outcome: 0,
        side: 0,
        quantity: 0,
        limit: 0,
        minimum_fill: 0,
        flags: 0,
        generation: 0,
        expiry_epoch: 0,
    };

    fn validate(
        self,
        expected_rank: u64,
        domain: DirectReservationDomainV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if self.owner.is_zero()
            || self.order_id != canonical_direct_order_id(expected_rank)?
            || self.outcome >= domain.outcome_count
            || self.side > 1
            || self.quantity == 0
            || self.limit > domain.price_scale
            || self.minimum_fill > self.quantity
            || self.flags & !1 != 0
            || (self.flags & 1 != 0 && self.minimum_fill != self.quantity)
            || self.expiry_epoch < domain.epoch_index
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }
}

/// Exact bounded page-zero projection used by direct V3.
///
/// This stores all sixteen slot kinds and bodies even though the profile
/// admits only two single-Egg records. Consequently a portfolio, tombstone,
/// dirty suffix, wrong positional id, or third record is represented and
/// refused rather than erased by an adapter projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezePageV3 {
    /// Exact page header identities.
    pub market_id: Identity32V1,
    /// Exact parent Epoch.
    pub epoch_id: Identity32V1,
    /// Zero while open; canonical one-page set fold after Freeze.
    pub order_set_id: Identity32V1,
    /// Canonical digest over header position/counts and all slot bytes.
    pub page_digest: Identity32V1,
    /// Exact first and last positional identities.
    pub first_order_id: Identity32V1,
    /// Exact last positional identity.
    pub last_order_id: Identity32V1,
    /// Page-zero predecessor, always zero.
    pub prev_page_last_order_id: Identity32V1,
    /// Exactly page zero.
    pub page_index: u16,
    /// Exactly one page in this bounded profile.
    pub page_count: u16,
    /// Zero while open; exact order count after Freeze.
    pub set_order_count: u16,
    /// Dense populated prefix count.
    pub order_count: u8,
    /// Direct V3 admits no retirement slots.
    pub tombstone_count: u8,
    /// Zero open, one frozen.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Exact live slot kinds for all sixteen physical slots.
    pub slot_kinds: [u8; DIRECT_PAGE_SLOT_COUNT_V3],
    /// Exact single-order bodies; empty suffix bodies are all zero.
    pub orders: [DirectPrefreezeOrderV3; DIRECT_PAGE_SLOT_COUNT_V3],
}

impl DirectPrefreezePageV3 {
    fn empty(domain: DirectReservationDomainV3, stored_bump: u8) -> Self {
        let mut page = Self {
            market_id: domain.market_id,
            epoch_id: domain.epoch_id,
            order_set_id: Identity32V1::ZERO,
            page_digest: Identity32V1::ZERO,
            first_order_id: Identity32V1::ZERO,
            last_order_id: Identity32V1::ZERO,
            prev_page_last_order_id: Identity32V1::ZERO,
            page_index: 0,
            page_count: 1,
            set_order_count: 0,
            order_count: 0,
            tombstone_count: 0,
            frozen: 0,
            stored_bump,
            slot_kinds: [0; DIRECT_PAGE_SLOT_COUNT_V3],
            orders: [DirectPrefreezeOrderV3::ZERO; DIRECT_PAGE_SLOT_COUNT_V3],
        };
        page.page_digest = page.recomputed_page_digest();
        page
    }

    fn validate(
        &self,
        domain: DirectReservationDomainV3,
        expected_count: u8,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if self.market_id != domain.market_id
            || self.epoch_id != domain.epoch_id
            || self.page_index != 0
            || self.page_count != 1
            || self.order_count != expected_count
            || self.order_count > 2
            || self.tombstone_count != 0
            || self.frozen > 1
            || !self.prev_page_last_order_id.is_zero()
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < DIRECT_PAGE_SLOT_COUNT_V3 {
            if index < usize::from(self.order_count) {
                if self.slot_kinds[index] != 1 {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                self.orders[index].validate(index as u64 + 1, domain)?;
            } else if self.slot_kinds[index] != 0
                || self.orders[index] != DirectPrefreezeOrderV3::ZERO
            {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        let expected_first = if self.order_count == 0 {
            Identity32V1::ZERO
        } else {
            canonical_direct_order_id(1)?
        };
        let expected_last = if self.order_count == 0 {
            Identity32V1::ZERO
        } else {
            canonical_direct_order_id(u64::from(self.order_count))?
        };
        if self.first_order_id != expected_first
            || self.last_order_id != expected_last
            || self.page_digest != self.recomputed_page_digest()
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        if self.frozen == 0 {
            if !self.order_set_id.is_zero() || self.set_order_count != 0 {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
        } else {
            if self.order_count != 2
                || self.set_order_count != 2
                || self.order_set_id != self.recomputed_order_set_id()
            {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
        }
        Ok(())
    }

    fn append(
        mut self,
        order: DirectPrefreezeOrderV3,
        domain: DirectReservationDomainV3,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        self.validate(domain, self.order_count)?;
        if self.frozen != 0 || self.order_count >= 2 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        let index = usize::from(self.order_count);
        order.validate(index as u64 + 1, domain)?;
        self.slot_kinds[index] = 1;
        self.orders[index] = order;
        self.order_count = self
            .order_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        self.first_order_id = canonical_direct_order_id(1)?;
        self.last_order_id = canonical_direct_order_id(u64::from(self.order_count))?;
        self.page_digest = self.recomputed_page_digest();
        self.validate(domain, self.order_count)?;
        Ok(self)
    }

    fn seal(mut self, domain: DirectReservationDomainV3) -> Result<Self, DirectLifecycleErrorV3> {
        self.validate(domain, 2)?;
        if self.frozen != 0 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.frozen = 1;
        self.set_order_count = 2;
        self.order_set_id = self.recomputed_order_set_id();
        self.validate(domain, 2)?;
        Ok(self)
    }

    fn recomputed_page_digest(&self) -> Identity32V1 {
        let mut hasher = Sha256::new();
        hasher.update(ORDER_PAGE_DOMAIN_V3);
        hasher.update(self.market_id.0);
        hasher.update(self.epoch_id.0);
        hasher.update(self.page_index.to_le_bytes());
        hasher.update([self.order_count, self.tombstone_count]);
        let mut index = 0usize;
        while index < DIRECT_PAGE_SLOT_COUNT_V3 {
            hash_direct_slot(&mut hasher, self.slot_kinds[index], self.orders[index]);
            index += 1;
        }
        Identity32V1(hasher.finalize().into())
    }

    fn recomputed_order_set_id(&self) -> Identity32V1 {
        let mut hasher = Sha256::new();
        hasher.update(ORDER_SET_DOMAIN_V1);
        hasher.update(self.market_id.0);
        hasher.update(self.epoch_id.0);
        hasher.update(self.page_count.to_le_bytes());
        hasher.update(self.set_order_count.to_le_bytes());
        hasher.update(self.page_digest.0);
        Identity32V1(hasher.finalize().into())
    }
}

fn hash_direct_slot(hasher: &mut Sha256, kind: u8, order: DirectPrefreezeOrderV3) {
    hasher.update([kind]);
    if kind == 1 {
        hasher.update(order.owner.0);
        hasher.update(order.order_id.0);
        hasher.update([order.outcome, order.side]);
        hasher.update(order.quantity.to_le_bytes());
        hasher.update(order.limit.to_le_bytes());
        hasher.update(order.minimum_fill.to_le_bytes());
        hasher.update([order.flags]);
        hasher.update(order.generation.to_le_bytes());
        hasher.update(order.expiry_epoch.to_le_bytes());
        hasher.update([0; DIRECT_ORDER_SLOT_BYTES_V3 - 1 - DIRECT_ORDER_BODY_BYTES_V3]);
    } else {
        hasher.update([0; DIRECT_ORDER_SLOT_BYTES_V3 - 1]);
    }
}

/// Exact Position projection used by the existing reservation release kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionV3 {
    /// Market identity.
    pub market_id: Identity32V1,
    /// Position owner.
    pub owner: Identity32V1,
    /// Close/reopen generation.
    pub generation: u64,
    /// Fixed internal Egg balances.
    pub internal: [u64; MAX_OUTCOMES],
    /// Total collateral cash.
    pub cash_atoms: u64,
    /// Encumbered cash included in `cash_atoms`.
    pub reserved_cash_atoms: u64,
    /// Persisted PDA bump.
    pub stored_bump: u8,
    /// Zero means open.
    pub close_state: u8,
}

impl DirectPositionV3 {
    /// Canonical inactive padding for a position-set suffix.
    pub const ZERO: Self = Self {
        market_id: Identity32V1::ZERO,
        owner: Identity32V1::ZERO,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: 0,
        close_state: 0,
    };

    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.market_id.is_zero()
            || self.owner.is_zero()
            || self.close_state > 2
            || self.reserved_cash_atoms > self.cash_atoms
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Exact Reservation V2 semantic body, excluding only its funding ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationBodyV3 {
    /// Canonical reservation identity.
    pub reservation_id: Identity32V1,
    /// Market identity.
    pub market_id: Identity32V1,
    /// Epoch identity.
    pub epoch_id: Identity32V1,
    /// Position owner.
    pub owner: Identity32V1,
    /// Canonical positional order identity.
    pub order_id: Identity32V1,
    /// Frozen price grid.
    pub price_grid_id: Identity32V1,
    /// Immutable settlement terms.
    pub terms_id: Identity32V1,
    /// Exact admission policy.
    pub policy_id: Identity32V1,
    /// Position generation.
    pub position_generation: u64,
    /// Order generation.
    pub order_generation: u64,
    /// Initial and current reserved cash.
    pub initial_cash_atoms: u64,
    /// Current reserved cash.
    pub remaining_cash_atoms: u64,
    /// Signed fee ceiling.
    pub max_fee_atoms: u64,
    /// Zero until release.
    pub release_generation: u64,
    /// Direct page index; V3 is page zero only.
    pub page_index: u16,
    /// Active outcomes.
    pub outcome_count: u8,
    /// Direct single-Egg order kind; exactly one.
    pub order_kind: u8,
    /// Zero buy, one sell.
    pub side: u8,
    /// Reservation state: zero ACTIVE, one RELEASED.
    pub state: u8,
    /// Persisted PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero.
    pub flags: u8,
    /// Exact initial Egg envelope.
    pub initial_internal: [u64; MAX_OUTCOMES],
    /// Exact remaining Egg envelope.
    pub remaining_internal: [u64; MAX_OUTCOMES],
}

impl DirectReservationBodyV3 {
    /// Canonical absent/closed padding.
    pub const ZERO: Self = Self {
        reservation_id: Identity32V1::ZERO,
        market_id: Identity32V1::ZERO,
        epoch_id: Identity32V1::ZERO,
        owner: Identity32V1::ZERO,
        order_id: Identity32V1::ZERO,
        price_grid_id: Identity32V1::ZERO,
        terms_id: Identity32V1::ZERO,
        policy_id: Identity32V1::ZERO,
        position_generation: 0,
        order_generation: 0,
        initial_cash_atoms: 0,
        remaining_cash_atoms: 0,
        max_fee_atoms: 0,
        release_generation: 0,
        page_index: 0,
        outcome_count: 0,
        order_kind: 0,
        side: 0,
        state: 0,
        stored_bump: 0,
        flags: 0,
        initial_internal: [0; MAX_OUTCOMES],
        remaining_internal: [0; MAX_OUTCOMES],
    };

    fn validate_active(
        self,
        domain: DirectReservationDomainV3,
        order: DirectPrefreezeOrderV3,
        expected_rank: u64,
    ) -> Result<(), DirectLifecycleErrorV3> {
        order.validate(expected_rank, domain)?;
        let (cash, internal) = direct_reservation_envelope(order, domain, self.max_fee_atoms)?;
        if self.reservation_id
            != canonical_direct_reservation_id(
                domain.market_id,
                domain.epoch_id,
                order.owner,
                self.position_generation,
                order.order_id,
            )
            || self.owner != order.owner
            || self.market_id != domain.market_id
            || self.epoch_id != domain.epoch_id
            || self.order_id != order.order_id
            || self.price_grid_id != domain.price_grid_id
            || self.terms_id != domain.terms_id
            || self.policy_id != domain.policy_id
            || self.order_generation != order.generation
            || self.initial_cash_atoms != cash
            || self.remaining_cash_atoms != cash
            || self.release_generation != 0
            || self.page_index != 0
            || self.outcome_count != domain.outcome_count
            || self.order_kind != 1
            || self.side != order.side
            || self.state != 0
            || self.flags != 0
            || self.initial_internal != internal
            || self.remaining_internal != internal
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn released(self) -> Result<Self, DirectLifecycleErrorV3> {
        let release_generation = self
            .order_generation
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let mut post = self;
        post.remaining_cash_atoms = 0;
        post.remaining_internal = [0; MAX_OUTCOMES];
        post.release_generation = release_generation;
        post.state = 1;
        Ok(post)
    }

    fn validate_frozen(
        self,
        domain: DirectReservationDomainV3,
        order: DirectPrefreezeOrderV3,
        expected_rank: u64,
        expected_state: u8,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if self.state != expected_state || !matches!(expected_state, 0 | 2) {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let mut active = self;
        active.state = 0;
        active.validate_active(domain, order, expected_rank)
    }

    fn entitled(self) -> Result<Self, DirectLifecycleErrorV3> {
        if self.state != 0 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        let mut post = self;
        post.state = 2;
        Ok(post)
    }

    fn consumed(self) -> Result<Self, DirectLifecycleErrorV3> {
        if self.state != 2 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        let mut post = self;
        post.remaining_cash_atoms = 0;
        post.remaining_internal = [0; MAX_OUTCOMES];
        post.release_generation = 0;
        post.state = 3;
        Ok(post)
    }
}

/// Program-issued Reservation V2 certificate plus exact funding ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeReservationV3 {
    /// Exact page record.
    pub order: DirectPrefreezeOrderV3,
    /// Exact Reservation semantic bytes.
    pub reservation: DirectReservationBodyV3,
    /// Rent principal and neutral donation ledger.
    pub account: DirectAccountLedgerV3,
}

impl DirectPrefreezeReservationV3 {
    /// Canonical inactive suffix.
    pub const ZERO: Self = Self {
        order: DirectPrefreezeOrderV3 {
            owner: Identity32V1::ZERO,
            order_id: Identity32V1::ZERO,
            outcome: 0,
            side: 0,
            quantity: 0,
            limit: 0,
            minimum_fill: 0,
            flags: 0,
            generation: 0,
            expiry_epoch: 0,
        },
        reservation: DirectReservationBodyV3 {
            reservation_id: Identity32V1::ZERO,
            market_id: Identity32V1::ZERO,
            epoch_id: Identity32V1::ZERO,
            owner: Identity32V1::ZERO,
            order_id: Identity32V1::ZERO,
            price_grid_id: Identity32V1::ZERO,
            terms_id: Identity32V1::ZERO,
            policy_id: Identity32V1::ZERO,
            position_generation: 0,
            order_generation: 0,
            initial_cash_atoms: 0,
            remaining_cash_atoms: 0,
            max_fee_atoms: 0,
            release_generation: 0,
            page_index: 0,
            outcome_count: 0,
            order_kind: 0,
            side: 0,
            state: 0,
            stored_bump: 0,
            flags: 0,
            initial_internal: [0; MAX_OUTCOMES],
            remaining_internal: [0; MAX_OUTCOMES],
        },
        account: DirectAccountLedgerV3::ZERO,
    };
}

/// Exact active Position set supplied to pre-freeze abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionSetV3 {
    /// Distinct Position accounts in first-owner appearance order.
    pub positions: [DirectPositionV3; 2],
    /// Active prefix.
    pub count: u8,
}

impl DirectPositionSetV3 {
    /// No positions for a zero-reservation abort.
    pub const ZERO: Self = Self {
        positions: [DirectPositionV3::ZERO; 2],
        count: 0,
    };

    fn validate(self, market: Identity32V1) -> Result<(), DirectLifecycleErrorV3> {
        if self.count > 2 {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < self.positions.len() {
            if index < usize::from(self.count) {
                self.positions[index].validate()?;
                if self.positions[index].market_id != market
                    || self.positions[index].close_state != 0
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                let mut prior = 0usize;
                while prior < index {
                    if self.positions[prior].owner == self.positions[index].owner {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    prior += 1;
                }
            } else if self.positions[index] != DirectPositionV3::ZERO {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Unfrozen V4 Epoch state before WorkBudget creation.
///
/// The active Reservation prefix is byte-exact and bounded to two. Padding is
/// canonical zero so hostile counts cannot trigger unchecked indexing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeV3 {
    /// Immutable lifecycle deadlines.
    pub schedule: DirectScheduleV3,
    /// Immutable verifier-release and neutral-sink authority.
    pub authority: DirectLifecycleAuthorityV3,
    /// Exact immutable reservation/page domain.
    pub reservation_domain: DirectReservationDomainV3,
    /// Exact self-certifying live PriceGrid projection.
    pub grid: DirectGridV3,
    /// Exact open page-zero account projection, including all suffix slots.
    pub page: DirectPrefreezePageV3,
    /// Active prefix length in `reservations`.
    pub reservation_count: u8,
    /// Program-issued Reservation V2 certificates in order-page order.
    pub reservations: [DirectPrefreezeReservationV3; 2],
}

/// One successful Reservation V2 creation before Freeze.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeReservationPlanV3 {
    /// Exact pre-freeze poststate.
    pub post: DirectPrefreezeV3,
    /// Exact Position poststate after funding the new reservation.
    pub position_post: DirectPositionV3,
    /// No close/reward effect; creation funding is authenticated separately.
    pub effects: DirectLifecycleEffectsV3,
}

/// Durable pre-freeze terminal state after bounded abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeTerminalV3 {
    /// Immutable schedule retained by the Epoch archive.
    pub schedule: DirectScheduleV3,
    /// Immutable authority retained by the Epoch archive.
    pub authority: DirectLifecycleAuthorityV3,
    /// Immutable Reservation domain retained by the Epoch archive.
    pub reservation_domain: DirectReservationDomainV3,
    /// Exact durable terminal receipt.
    pub terminal_receipt: DirectTerminalReceiptV3,
}

/// One successful pre-freeze abort/release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrefreezeAbortPlanV3 {
    /// Durable terminal poststate.
    pub post: DirectPrefreezeTerminalV3,
    /// Exact zero-, one-, or two-Reservation close/refund effects.
    pub effects: DirectLifecycleEffectsV3,
}

impl DirectPrefreezeTerminalV3 {
    /// Validate the durable no-Freeze receipt retained by Epoch V4.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.schedule.validate()?;
        self.reservation_domain.validate()?;
        self.authority
            .validate_for_epoch(self.reservation_domain.epoch_id)?;
        let receipt = self.terminal_receipt;
        if receipt.reason != DirectTerminalReasonV3::PrefreezeAbort
            || receipt.terminal_reservation_count > 2
            || (receipt.terminal_reservation_count == 0
                && (!receipt.candidate_id.is_zero()
                    || !receipt.relation_candidate_digest.is_zero()))
            || (receipt.terminal_reservation_count == 1
                && (receipt.candidate_id.is_zero() || !receipt.relation_candidate_digest.is_zero()))
            || (receipt.terminal_reservation_count == 2
                && (receipt.candidate_id.is_zero()
                    || receipt.relation_candidate_digest.is_zero()
                    || receipt.candidate_id == receipt.relation_candidate_digest))
            || receipt.selected_slot != 0
            || receipt.outcome != 0
            || receipt.quantity != 0
            || receipt.price != 0
            || receipt.consideration_price_units != 0
            || receipt.terminal_slot < self.schedule.submission_opens_slot
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }
}

impl DirectObservedBalancesV3 {
    /// Canonical no-close observation.
    pub const ZERO: Self = Self {
        candidates: [0; MAX_DIRECT_CANDIDATES],
        window: 0,
        receipt: 0,
        pot: 0,
        work_budget: 0,
        reservations: [0; 2],
    };
}

/// Closed transient account kind, retained for conservation diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClosedAuthorityV3 {
    /// Candidate at retained index.
    Candidate(u8),
    /// Candidate Window.
    Window,
    /// Settlement receipt.
    Receipt,
    /// Settlement pot.
    Pot,
    /// Reward WorkBudget account.
    WorkBudget,
    /// Reservation at book index.
    Reservation(u8),
}

/// Exact split of one closed account balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseLegV3 {
    /// Closed authority kind.
    pub authority: DirectClosedAuthorityV3,
    /// Original payer.
    pub payer: Identity32V1,
    /// Exact payer-funded principal returned to `payer`.
    pub payer_principal_lamports: u64,
    /// Entire observed balance after any same-transition reward debit.
    pub observed_live_lamports: u64,
    /// Previously authenticated monotone donation lower bound.
    pub prior_donation_lamports: u64,
    /// Excess routed to the immutable neutral sink.
    pub neutral_surplus_lamports: u64,
}

impl DirectCloseLegV3 {
    /// Canonical inactive padding.
    pub const ZERO: Self = Self {
        authority: DirectClosedAuthorityV3::Window,
        payer: Identity32V1::ZERO,
        payer_principal_lamports: 0,
        observed_live_lamports: 0,
        prior_donation_lamports: 0,
        neutral_surplus_lamports: 0,
    };
}

/// Side effects authorized by one modeled transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleEffectsV3 {
    /// Exact closed-account splits; maximum terminal shape is seven accounts.
    pub close_legs: [DirectCloseLegV3; 7],
    /// Active prefix length of `close_legs`.
    pub close_count: u8,
    /// Keeper reward debited only from spendable WorkBudget rewards.
    pub keeper_reward: u64,
    /// Unused spendable rewards returned to their authenticated sponsor.
    pub work_budget_refund: u64,
    /// WorkBudget reward sponsor.
    pub work_budget_refund_recipient: Identity32V1,
    /// Immutable sink for prefunds and later close-time surplus.
    pub neutral_lamport_sink: Identity32V1,
    /// Exact two-reservation state transition.
    pub reservation_transition: DirectReservationTransitionV3,
    /// Active prefix affected by `reservation_transition`.
    pub reservation_transition_count: u8,
    /// Exact Position before/after legs for reservation release.
    pub position_releases: [DirectPositionReleaseV3; 2],
    /// Active prefix in `position_releases`.
    pub position_release_count: u8,
    /// Exact Position before/after legs for successful settlement.
    pub position_settlements: [DirectPositionSettlementV3; 2],
    /// Active prefix in `position_settlements`.
    pub position_settlement_count: u8,
    /// Exact collateral atoms transferred buyer to seller.
    pub settlement_consideration_atoms: u64,
    /// Exact buy Reservation envelope removed from reserved cash.
    pub buyer_reserved_release_atoms: u64,
    /// Exact RELEASED or CONSUMED Reservation poststates before account close.
    pub terminal_reservations: [DirectReservationBodyV3; 2],
}

/// One exact aggregate Position mutation caused by releasing one or two
/// reservations owned by the same Position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionReleaseV3 {
    /// Authenticated prestate.
    pub before: DirectPositionV3,
    /// Exact cash/Egg restoration poststate.
    pub after: DirectPositionV3,
}

impl DirectPositionReleaseV3 {
    /// Canonical inactive suffix.
    pub const ZERO: Self = Self {
        before: DirectPositionV3::ZERO,
        after: DirectPositionV3::ZERO,
    };
}

/// One exact Position mutation caused by the selected full-fill settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionSettlementV3 {
    /// Authenticated prestate.
    pub before: DirectPositionV3,
    /// Exact cash/Egg transfer poststate.
    pub after: DirectPositionV3,
}

impl DirectPositionSettlementV3 {
    /// Canonical inactive suffix.
    pub const ZERO: Self = Self {
        before: DirectPositionV3::ZERO,
        after: DirectPositionV3::ZERO,
    };
}

/// Exact buyer/seller Position pair for settlement, independent of frozen
/// page slot order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSettlementPositionsV3 {
    /// Buyer Position prestate.
    pub buyer: DirectPositionV3,
    /// Seller Position prestate.
    pub seller: DirectPositionV3,
}

impl DirectLifecycleEffectsV3 {
    /// No external effect.
    pub const NONE: Self = Self {
        close_legs: [DirectCloseLegV3::ZERO; 7],
        close_count: 0,
        keeper_reward: 0,
        work_budget_refund: 0,
        work_budget_refund_recipient: Identity32V1::ZERO,
        neutral_lamport_sink: Identity32V1::ZERO,
        reservation_transition: DirectReservationTransitionV3::None,
        reservation_transition_count: 0,
        position_releases: [DirectPositionReleaseV3::ZERO; 2],
        position_release_count: 0,
        position_settlements: [DirectPositionSettlementV3::ZERO; 2],
        position_settlement_count: 0,
        settlement_consideration_atoms: 0,
        buyer_reserved_release_atoms: 0,
        terminal_reservations: [DirectReservationBodyV3::ZERO; 2],
    };

    /// Sum exact payer principal, excluding rewards and neutral surplus.
    pub fn payer_principal_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].payer_principal_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    /// Sum observed closed balances after any same-transition reward debit.
    pub fn observed_close_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].observed_live_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    /// Sum unsolicited close-time surplus sent to the neutral sink.
    pub fn neutral_surplus_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].neutral_surplus_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    fn push_close(
        &mut self,
        authority: DirectClosedAuthorityV3,
        account: DirectAccountLedgerV3,
        observed_live_lamports: u64,
        protected_lamports: u64,
        sink: Identity32V1,
    ) -> Result<(), DirectLifecycleErrorV3> {
        account.validate(sink)?;
        let protected_total = account
            .rent
            .lamports
            .checked_add(protected_lamports)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let disposition = account
            .donation
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?
            .terminal_split(observed_live_lamports, protected_total)?;
        let neutral_surplus_lamports = disposition.neutral_lamports;
        let index = usize::from(self.close_count);
        if index >= self.close_legs.len()
            || sink.is_zero()
            || (!self.neutral_lamport_sink.is_zero() && self.neutral_lamport_sink != sink)
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        self.neutral_lamport_sink = sink;
        self.close_legs[index] = DirectCloseLegV3 {
            authority,
            payer: account.rent.payer,
            payer_principal_lamports: account.rent.lamports,
            observed_live_lamports,
            prior_donation_lamports: account.donation_lamports()?,
            neutral_surplus_lamports,
        };
        self.close_count = self
            .close_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Compact durable terminal receipt embedded in the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTerminalReceiptV3 {
    /// Why authority terminated.
    pub reason: DirectTerminalReasonV3,
    /// Exact Reservation prefix archived by this terminal transition.
    pub terminal_reservation_count: u8,
    /// Exact Finalize slot, zero when selection never occurred.
    pub selected_slot: u64,
    /// Selected Candidate, or zero before selection.
    pub candidate_id: Identity32V1,
    /// Full relation-candidate digest, or zero before selection.
    pub relation_candidate_digest: Identity32V1,
    /// Settled outcome; zero for lapse.
    pub outcome: u8,
    /// Settled quantity; zero for lapse.
    pub quantity: u64,
    /// Settled price units; zero for lapse.
    pub price: u64,
    /// Exact `quantity * price`; zero for lapse.
    pub consideration_price_units: u128,
    /// Slot of terminal transition.
    pub terminal_slot: u64,
}

impl DirectTerminalReceiptV3 {
    /// Canonical placeholder before terminality; phase owns its interpretation.
    pub const EMPTY: Self = Self {
        reason: DirectTerminalReasonV3::EmptyLapse,
        terminal_reservation_count: 0,
        selected_slot: 0,
        candidate_id: Identity32V1::ZERO,
        relation_candidate_digest: Identity32V1::ZERO,
        outcome: 0,
        quantity: 0,
        price: 0,
        consideration_price_units: 0,
        terminal_slot: 0,
    };
}

/// Complete bounded authority/economic state across Epoch, Window, Candidate,
/// WorkBudget, receipt/pot, and two Reservation accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleV3 {
    /// Current lifecycle phase.
    pub phase: DirectLifecyclePhaseV3,
    /// Immutable deadlines.
    pub schedule: DirectScheduleV3,
    /// Immutable semantics/sink authority.
    pub authority: DirectLifecycleAuthorityV3,
    /// Exact immutable Reservation/page domain retained after Freeze.
    pub reservation_domain: DirectReservationDomainV3,
    /// Exact self-certifying frozen PriceGrid projection.
    pub grid: DirectGridV3,
    /// Exact frozen page-zero projection and order-set commitment.
    pub frozen_page: DirectPrefreezePageV3,
    /// Complete full-width relation domain derived only at Freeze.
    pub relation_domain: FullRelationDomainV1,
    /// Exact Reservation V2 semantic bodies; funding ledgers are below.
    pub reservations: [DirectReservationBodyV3; 2],
    /// Immutable reward schedule.
    pub rewards: DirectKeeperRewardsV3,
    /// WorkBudget reward sponsor.
    pub work_budget_sponsor: Identity32V1,
    /// WorkBudget rent and canonical donation ledger.
    pub work_budget_account: DirectAccountLedgerV3,
    /// Remaining spendable reward balance.
    pub work_budget_balance: u64,
    /// Original spendable reward balance.
    pub work_budget_initial_balance: u64,
    /// Rewards already paid before terminality.
    pub work_rewards_paid: u64,
    /// Exact Reservation V2 rent and donation ledgers.
    pub reservation_accounts: [DirectAccountLedgerV3; 2],
    /// Current paired reservation state.
    pub reservation_state: DirectReservationStateV3,
    /// Window top count. It remains unchanged after selection until Window close.
    pub top_count: u8,
    /// Window top commitments. Losers remain here after their accounts close.
    pub top: [DirectCandidateEntryV1; MAX_DIRECT_CANDIDATES],
    /// Live Candidate facts. Closed losers become canonical zero after Finalize.
    pub retained: [DirectCandidateLeaseV3; MAX_DIRECT_CANDIDATES],
    /// Exact physical Candidate-account presence bits.
    pub live_candidate_mask: u8,
    /// Ticks ever competitively admitted, including displaced entries.
    pub seen_competitive_ticks: u64,
    /// Competitive admissions only; bounded by 64.
    pub competitive_admission_count: u8,
    /// Ordered full-width commitment over competitive admissions.
    pub competitive_admission_transcript: Identity32V1,
    /// Window rent and donation ledger.
    pub window_account: DirectAccountLedgerV3,
    /// Bits whose exact Candidate has been fully re-executed in V3.
    pub verification_mask: u8,
    /// Receipt rent/donation ledger, set once by Finalize.
    pub receipt_account: DirectAccountLedgerV3,
    /// Pot rent/donation ledger, set once by Finalize.
    pub pot_account: DirectAccountLedgerV3,
    /// Exact Finalize slot, zero before selection.
    pub selected_slot: u64,
    /// Durable terminal audit evidence retained in Epoch.
    pub terminal_receipt: DirectTerminalReceiptV3,
}

/// Successful initialization at the same atomic boundary as Freeze.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInitializationPlanV3 {
    /// Frozen-empty poststate with two ACTIVE reservations.
    pub post: DirectLifecycleV3,
    /// WorkBudget target prefund normalization.
    pub effects: DirectLifecycleEffectsV3,
}

/// One accepted or explicitly rejected valid submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAdmissionPlanV3 {
    /// Complete poststate; byte-identical for a noncompetitive attempt.
    pub post: DirectLifecycleV3,
    /// Explicit outcome.
    pub disposition: DirectAdmissionDispositionV3,
    /// Prefund sweep and optional displaced-account close.
    pub effects: DirectLifecycleEffectsV3,
}

/// One successful staged lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTransitionPlanV3 {
    /// Complete poststate.
    pub post: DirectLifecycleV3,
    /// Exact account splits, rewards, prefunds, and reservation transition.
    pub effects: DirectLifecycleEffectsV3,
}

impl DirectPrefreezeV3 {
    /// Create an unfrozen Epoch before any Reservation or WorkBudget exists.
    pub fn initialize(
        schedule: DirectScheduleV3,
        authority: DirectLifecycleAuthorityV3,
        reservation_domain: DirectReservationDomainV3,
        grid: DirectGridV3,
        page_bump: u8,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        let state = Self {
            schedule,
            authority,
            reservation_domain,
            grid,
            page: DirectPrefreezePageV3::empty(reservation_domain, page_bump),
            reservation_count: 0,
            reservations: [DirectPrefreezeReservationV3::ZERO; 2],
        };
        state.validate()?;
        Ok(state)
    }

    /// Validate hostile persisted pre-freeze fields before indexing the prefix.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.schedule.validate()?;
        self.reservation_domain.validate()?;
        self.authority
            .validate_for_epoch(self.reservation_domain.epoch_id)?;
        self.grid.validate()?;
        if self.grid.grid_id != self.reservation_domain.price_grid_id
            || self.grid.price_scale != self.reservation_domain.price_scale
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        self.page
            .validate(self.reservation_domain, self.reservation_count)?;
        if self.reservation_count > 2 {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < self.reservations.len() {
            if index < usize::from(self.reservation_count) {
                let lease = self.reservations[index];
                lease
                    .account
                    .validate(self.authority.neutral_lamport_sink)?;
                lease.reservation.validate_active(
                    self.reservation_domain,
                    lease.order,
                    index as u64 + 1,
                )?;
                if lease.order != self.page.orders[index] {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            } else if self.reservations[index] != DirectPrefreezeReservationV3::ZERO {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        Ok(())
    }

    /// Append one Reservation V2 before submission opens.
    ///
    /// Existing Reservation accounts are observed first; the new account uses
    /// its own exact create delta. A third Reservation or late placement fails
    /// without changing either value.
    pub fn place_reservation(
        self,
        context: DirectTransitionContextV3,
        placement: DirectReservationPlacementV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectPrefreezeReservationPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if context.now >= self.schedule.submission_opens_slot {
            return Err(DirectLifecycleErrorV3::PrefreezeWindow);
        }
        if self.reservation_count >= 2 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        let mut post = self;
        post.observe_reservations(observed)?;
        let index = usize::from(post.reservation_count);
        placement
            .order
            .validate(index as u64 + 1, post.reservation_domain)?;
        post.grid.tick_of(placement.order.limit)?;
        let (position_post, reservation) = prepare_direct_reservation_placement(
            post.reservation_domain,
            placement.order,
            placement.position,
            placement.max_fee_atoms,
            placement.stored_bump,
        )?;
        post.reservations[index] = DirectPrefreezeReservationV3 {
            order: placement.order,
            reservation,
            account: placement
                .creation
                .account(post.authority.neutral_lamport_sink, 0)?,
        };
        post.reservation_count = post
            .reservation_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        post.page = post.page.append(placement.order, post.reservation_domain)?;
        post.validate()?;
        Ok(DirectPrefreezeReservationPlanV3 {
            post,
            position_post,
            effects: DirectLifecycleEffectsV3::NONE,
        })
    }

    /// Atomically freeze exactly two observed ACTIVE reservations and create a
    /// fully capitalized WorkBudget. No work balance exists before this step.
    pub fn freeze(
        self,
        context: DirectTransitionContextV3,
        rewards: DirectKeeperRewardsV3,
        work_budget: DirectWorkBudgetFundingV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectInitializationPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if context.now >= self.schedule.submission_opens_slot {
            return Err(DirectLifecycleErrorV3::PrefreezeWindow);
        }
        if self.reservation_count != 2 {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        rewards.validate()?;
        let mut observed_prefreeze = self;
        observed_prefreeze.observe_reservations(observed)?;
        let (buy, sell) = match (
            observed_prefreeze.reservations[0].order.side,
            observed_prefreeze.reservations[1].order.side,
        ) {
            (0, 1) => (
                observed_prefreeze.reservations[0].order,
                observed_prefreeze.reservations[1].order,
            ),
            (1, 0) => (
                observed_prefreeze.reservations[1].order,
                observed_prefreeze.reservations[0].order,
            ),
            _ => return Err(DirectLifecycleErrorV3::MismatchedBinding),
        };
        if observed_prefreeze.reservations[0].order.outcome
            != observed_prefreeze.reservations[1].order.outcome
            || observed_prefreeze.reservations[0].order.quantity
                != observed_prefreeze.reservations[1].order.quantity
            || observed_prefreeze.reservations[0].order.owner
                == observed_prefreeze.reservations[1].order.owner
            || buy.limit < sell.limit
            || observed_prefreeze.reservations[0].order.minimum_fill != 0
            || observed_prefreeze.reservations[1].order.minimum_fill != 0
            || observed_prefreeze.reservations[0].order.flags != 0
            || observed_prefreeze.reservations[1].order.flags != 0
            || observed_prefreeze.reservations[0].reservation.max_fee_atoms != 0
            || observed_prefreeze.reservations[1].reservation.max_fee_atoms != 0
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let frozen_page = observed_prefreeze
            .page
            .seal(observed_prefreeze.reservation_domain)?;
        let relation_domain = observed_prefreeze
            .reservation_domain
            .full_relation_domain(frozen_page.order_set_id)?;
        let work_budget_account = work_budget.creation.account(
            observed_prefreeze.authority.neutral_lamport_sink,
            work_budget.reward_lamports,
        )?;
        if work_budget.reward_sponsor.is_zero()
            || work_budget.reward_sponsor != work_budget_account.rent.payer
            || work_budget.reward_lamports < rewards.worst_case()?
        {
            return Err(DirectLifecycleErrorV3::WorkBudgetInsufficient);
        }
        let post = DirectLifecycleV3 {
            phase: DirectLifecyclePhaseV3::FrozenEmpty,
            schedule: observed_prefreeze.schedule,
            authority: observed_prefreeze.authority,
            reservation_domain: observed_prefreeze.reservation_domain,
            grid: observed_prefreeze.grid,
            frozen_page,
            relation_domain,
            reservations: [
                observed_prefreeze.reservations[0].reservation,
                observed_prefreeze.reservations[1].reservation,
            ],
            rewards,
            work_budget_sponsor: work_budget.reward_sponsor,
            work_budget_account,
            work_budget_balance: work_budget.reward_lamports,
            work_budget_initial_balance: work_budget.reward_lamports,
            work_rewards_paid: 0,
            reservation_accounts: [
                observed_prefreeze.reservations[0].account,
                observed_prefreeze.reservations[1].account,
            ],
            reservation_state: DirectReservationStateV3::Active,
            top_count: 0,
            top: [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES],
            retained: [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES],
            live_candidate_mask: 0,
            seen_competitive_ticks: 0,
            competitive_admission_count: 0,
            competitive_admission_transcript: Identity32V1::ZERO,
            window_account: DirectAccountLedgerV3::ZERO,
            verification_mask: 0,
            receipt_account: DirectAccountLedgerV3::ZERO,
            pot_account: DirectAccountLedgerV3::ZERO,
            selected_slot: 0,
            terminal_receipt: DirectTerminalReceiptV3::EMPTY,
        };
        post.validate()?;
        Ok(DirectInitializationPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3::NONE,
        })
    }

    /// Permissionlessly terminate an Epoch that failed to freeze by submission
    /// open, releasing and closing the exact zero-, one-, or two-account prefix.
    pub fn abort(
        self,
        now: u64,
        observed: DirectObservedBalancesV3,
        positions: DirectPositionSetV3,
    ) -> Result<DirectPrefreezeAbortPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        if now < self.schedule.submission_opens_slot {
            return Err(DirectLifecycleErrorV3::PrefreezeWindow);
        }
        validate_prefreeze_observation_shape(observed, self.reservation_count)?;
        let mut effects = DirectLifecycleEffectsV3 {
            reservation_transition: DirectReservationTransitionV3::ActiveToReleased,
            reservation_transition_count: self.reservation_count,
            ..DirectLifecycleEffectsV3::NONE
        };
        release_reservations(
            self.reservation_domain,
            [self.reservations[0].order, self.reservations[1].order],
            [
                self.reservations[0].reservation,
                self.reservations[1].reservation,
            ],
            self.reservation_count,
            positions,
            &mut effects,
        )?;
        let mut index = 0usize;
        while index < usize::from(self.reservation_count) {
            effects.push_close(
                DirectClosedAuthorityV3::Reservation(index as u8),
                self.reservations[index].account,
                observed.reservations[index],
                0,
                self.authority.neutral_lamport_sink,
            )?;
            index += 1;
        }
        let terminal_receipt = DirectTerminalReceiptV3 {
            reason: DirectTerminalReasonV3::PrefreezeAbort,
            terminal_reservation_count: self.reservation_count,
            candidate_id: if self.reservation_count > 0 {
                self.reservations[0].reservation.reservation_id
            } else {
                Identity32V1::ZERO
            },
            relation_candidate_digest: if self.reservation_count > 1 {
                self.reservations[1].reservation.reservation_id
            } else {
                Identity32V1::ZERO
            },
            terminal_slot: now,
            ..DirectTerminalReceiptV3::EMPTY
        };
        let post = DirectPrefreezeTerminalV3 {
            schedule: self.schedule,
            authority: self.authority,
            reservation_domain: self.reservation_domain,
            terminal_receipt,
        };
        post.validate()?;
        Ok(DirectPrefreezeAbortPlanV3 { post, effects })
    }

    fn require_release(
        &self,
        context: DirectTransitionContextV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if context.verifier_release_id != self.authority.verifier_release_id {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn observe_reservations(
        &mut self,
        observed: DirectObservedBalancesV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        validate_prefreeze_observation_shape(observed, self.reservation_count)?;
        let mut index = 0usize;
        while index < usize::from(self.reservation_count) {
            self.reservations[index].account = self.reservations[index].account.observe(
                observed.reservations[index],
                0,
                self.authority.neutral_lamport_sink,
            )?;
            index += 1;
        }
        Ok(())
    }
}

impl DirectLifecycleV3 {
    /// Validate every hostile persisted field before any public transition.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.schedule.validate()?;
        self.authority
            .validate_for_epoch(self.reservation_domain.epoch_id)?;
        self.rewards.validate()?;
        self.validate_frozen_authority()?;
        if self.top_count as usize > MAX_DIRECT_CANDIDATES
            || self.competitive_admission_count > MAX_DIRECT_TICKS_V3
            || self.seen_competitive_ticks.count_ones()
                != u32::from(self.competitive_admission_count)
            || self.live_candidate_mask & !prefix_mask(self.top_count)? != 0
            || self.verification_mask & !prefix_mask(self.top_count)? != 0
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < MAX_DIRECT_CANDIDATES {
            if index < usize::from(self.top_count) {
                let entry = self.top[index];
                if entry.candidate_id.is_zero() || entry.relation_candidate_digest.is_zero() {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                let mut prior = 0usize;
                while prior < index {
                    if self.top[prior].candidate_id == entry.candidate_id
                        || self.top[prior].relation_candidate_digest
                            == entry.relation_candidate_digest
                    {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    prior += 1;
                }
                if self.live_candidate_mask & (1u8 << index) != 0 {
                    let lease = self.retained[index];
                    lease.validate_body()?;
                    lease
                        .account
                        .validate(self.authority.neutral_lamport_sink)?;
                    if entry != lease.entry()
                        || self.seen_competitive_ticks & (1u64 << lease.tick) == 0
                    {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    let mut live_prior = 0usize;
                    while live_prior < index {
                        if self.live_candidate_mask & (1u8 << live_prior) != 0
                            && self.retained[live_prior].tick == lease.tick
                        {
                            return Err(DirectLifecycleErrorV3::MismatchedBinding);
                        }
                        live_prior += 1;
                    }
                    if index > 0
                        && self.live_candidate_mask & (1u8 << (index - 1)) != 0
                        && !self.retained[index - 1]
                            .score()
                            .is_better_than(&lease.score())
                    {
                        return Err(DirectLifecycleErrorV3::NonCanonical);
                    }
                } else if self.phase != DirectLifecyclePhaseV3::Selected
                    || index == 0
                    || self.retained[index] != DirectCandidateLeaseV3::ZERO
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
            } else if self.top[index] != DirectCandidateEntryV1::ZERO
                || self.retained[index] != DirectCandidateLeaseV3::ZERO
            {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        if self.competitive_admission_count == 0 {
            if !self.competitive_admission_transcript.is_zero() {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
        } else if self.competitive_admission_transcript.is_zero() {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        match self.phase {
            DirectLifecyclePhaseV3::FrozenEmpty => {
                if self.top_count != 0
                    || self.live_candidate_mask != 0
                    || self.window_account != DirectAccountLedgerV3::ZERO
                    || self.verification_mask != 0
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::WindowOpen => {
                if self.top_count == 0
                    || self.live_candidate_mask != prefix_mask(self.top_count)?
                    || self.verification_mask != 0
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.require_stages(false)?;
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Verifying => {
                if self.top_count == 0
                    || self.live_candidate_mask != prefix_mask(self.top_count)?
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                let mut candidate_index = 0usize;
                while candidate_index < usize::from(self.top_count) {
                    let bit = 1u8 << candidate_index;
                    let expected = if self.verification_mask & bit == 0 {
                        DirectCandidateStageV3::Verified
                    } else {
                        DirectCandidateStageV3::Reverified
                    };
                    if self.retained[candidate_index].stage != expected {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    candidate_index += 1;
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Selected => {
                if self.top_count == 0
                    || self.live_candidate_mask != 1
                    || self.verification_mask != prefix_mask(self.top_count)?
                    || self.selected_slot < self.schedule.submission_closes_slot
                    || self.selected_slot >= self.schedule.selection_deadline_slot
                    || self.reservation_state != DirectReservationStateV3::Entitled
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.receipt_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.pot_account
                    .validate(self.authority.neutral_lamport_sink)?;
                if self.retained[0].stage != DirectCandidateStageV3::Selected {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                let mut loser = 1usize;
                while loser < usize::from(self.top_count) {
                    if self.retained[loser] != DirectCandidateLeaseV3::ZERO {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    loser += 1;
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Terminal => self.validate_terminal()?,
        }
        Ok(())
    }

    /// Admit one verified Candidate iff it can enter the monotone top three.
    pub fn admit(
        self,
        context: DirectTransitionContextV3,
        candidate: DirectCandidateLeaseV3,
        first_window: DirectCreationFundingV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectAdmissionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if !matches!(
            self.phase,
            DirectLifecyclePhaseV3::FrozenEmpty | DirectLifecyclePhaseV3::WindowOpen
        ) {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        if context.now < self.schedule.submission_opens_slot
            || context.now >= self.schedule.submission_closes_slot
        {
            return Err(DirectLifecycleErrorV3::SubmissionClosed);
        }
        candidate.validate_body()?;
        candidate
            .account
            .validate(self.authority.neutral_lamport_sink)?;
        if candidate.stage != DirectCandidateStageV3::Verified
            || candidate.candidate.submitted_slot != context.now
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        self.bind_candidate(&candidate.candidate)?;
        let relation = ValidatedDirectDomainV1::new(&self.relation_domain)
            .map_err(|_| DirectLifecycleErrorV3::MismatchedBinding)?;
        verify_lease(
            &relation,
            &self.grid,
            candidate,
            self.verification_facts()?,
            self.authority.neutral_lamport_sink,
        )?;
        let first_window_account = if self.phase == DirectLifecyclePhaseV3::FrozenEmpty {
            first_window.account(self.authority.neutral_lamport_sink, 0)?
        } else if first_window != DirectCreationFundingV3::ZERO {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        } else {
            DirectAccountLedgerV3::ZERO
        };
        let bit = 1u64 << candidate.tick;
        if self.seen_competitive_ticks & bit != 0 {
            return Err(DirectLifecycleErrorV3::Replay);
        }
        let count = usize::from(self.top_count);
        let mut effects = DirectLifecycleEffectsV3::NONE;
        if count == MAX_DIRECT_CANDIDATES
            && !candidate
                .score()
                .is_better_than(&self.retained[MAX_DIRECT_CANDIDATES - 1].score())
        {
            self.require_observations_unchanged(observed)?;
            return Ok(DirectAdmissionPlanV3 {
                post: self,
                disposition: DirectAdmissionDispositionV3::RejectedNonCompetitive,
                effects,
            });
        }
        let mut post = self;
        post.observe_live_accounts(observed)?;
        let disposition;
        if post.phase == DirectLifecyclePhaseV3::FrozenEmpty {
            post.phase = DirectLifecyclePhaseV3::WindowOpen;
            post.window_account = first_window_account;
            disposition = DirectAdmissionDispositionV3::First;
        } else if count == MAX_DIRECT_CANDIDATES {
            effects.push_close(
                DirectClosedAuthorityV3::Candidate((MAX_DIRECT_CANDIDATES - 1) as u8),
                post.retained[MAX_DIRECT_CANDIDATES - 1].account,
                observed.candidates[MAX_DIRECT_CANDIDATES - 1],
                0,
                post.authority.neutral_lamport_sink,
            )?;
            post.retained[MAX_DIRECT_CANDIDATES - 1] = candidate;
            post.top[MAX_DIRECT_CANDIDATES - 1] = candidate.entry();
            disposition = DirectAdmissionDispositionV3::Replaced;
        } else {
            post.retained[count] = candidate;
            post.top[count] = candidate.entry();
            post.top_count = post
                .top_count
                .checked_add(1)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            disposition = DirectAdmissionDispositionV3::Retained;
        }
        if disposition == DirectAdmissionDispositionV3::First {
            post.retained[0] = candidate;
            post.top[0] = candidate.entry();
            post.top_count = 1;
        }
        let mut index = usize::from(post.top_count) - 1;
        while index > 0
            && post.retained[index]
                .score()
                .is_better_than(&post.retained[index - 1].score())
        {
            post.retained.swap(index, index - 1);
            post.top.swap(index, index - 1);
            index -= 1;
        }
        post.live_candidate_mask = prefix_mask(post.top_count)?;
        post.seen_competitive_ticks |= bit;
        post.competitive_admission_count = post
            .competitive_admission_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        post.competitive_admission_transcript = next_transcript(
            post.competitive_admission_transcript,
            post.competitive_admission_count,
            &candidate,
        );
        post.validate()?;
        Ok(DirectAdmissionPlanV3 {
            post,
            disposition,
            effects,
        })
    }

    /// Close submissions and begin staged retained-candidate verification.
    pub fn begin_verification(
        self,
        context: DirectTransitionContextV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if self.phase != DirectLifecyclePhaseV3::WindowOpen {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        let mut post = self;
        post.observe_live_accounts(observed)?;
        let reward = post.debit_reward(post.rewards.begin_verification)?;
        post.phase = DirectLifecyclePhaseV3::Verifying;
        post.verification_mask = 0;
        post.validate()?;
        Ok(DirectTransitionPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3 {
                keeper_reward: reward,
                ..DirectLifecycleEffectsV3::NONE
            },
        })
    }

    /// Re-execute one exact retained Candidate under the frozen verifier/grid.
    pub fn verify_candidate(
        self,
        context: DirectTransitionContextV3,
        index: u8,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if self.phase != DirectLifecyclePhaseV3::Verifying {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        if index >= self.top_count {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let bit = 1u8 << index;
        if self.verification_mask & bit != 0 {
            return Err(DirectLifecycleErrorV3::Replay);
        }
        let retained = self.retained[usize::from(index)];
        self.bind_candidate(&retained.candidate)?;
        let domain = ValidatedDirectDomainV1::new(&self.relation_domain)
            .map_err(|_| DirectLifecycleErrorV3::MismatchedBinding)?;
        verify_lease(
            &domain,
            &self.grid,
            retained,
            self.verification_facts()?,
            self.authority.neutral_lamport_sink,
        )?;
        let mut post = self;
        post.observe_live_accounts(observed)?;
        let reward = post.debit_reward(post.rewards.verify_candidate)?;
        post.retained[usize::from(index)].stage = DirectCandidateStageV3::Reverified;
        post.verification_mask |= bit;
        post.validate()?;
        Ok(DirectTransitionPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3 {
                keeper_reward: reward,
                ..DirectLifecycleEffectsV3::NONE
            },
        })
    }

    /// Select top zero only after exact staged reexecution of the full prefix.
    pub fn finalize_selection(
        self,
        context: DirectTransitionContextV3,
        funding: DirectSelectionFundingV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if self.phase != DirectLifecyclePhaseV3::Verifying {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        let required_mask = prefix_mask(self.top_count)?;
        if self.verification_mask != required_mask {
            return Err(DirectLifecycleErrorV3::VerificationIncomplete);
        }
        let receipt_account = funding
            .receipt
            .account(self.authority.neutral_lamport_sink, 0)?;
        let pot_account = funding
            .pot
            .account(self.authority.neutral_lamport_sink, 0)?;
        let mut candidate_index = 0usize;
        while candidate_index < usize::from(self.top_count) {
            if self.retained[candidate_index].stage != DirectCandidateStageV3::Reverified {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            candidate_index += 1;
        }
        let mut post = self;
        post.observe_live_accounts(observed)?;
        let reward = post.debit_reward(post.rewards.finalize_selection)?;
        let mut effects = DirectLifecycleEffectsV3 {
            keeper_reward: reward,
            reservation_transition: DirectReservationTransitionV3::ActiveToEntitled,
            reservation_transition_count: 2,
            ..DirectLifecycleEffectsV3::NONE
        };
        let mut loser = 1usize;
        while loser < usize::from(post.top_count) {
            effects.push_close(
                DirectClosedAuthorityV3::Candidate(loser as u8),
                post.retained[loser].account,
                observed.candidates[loser],
                0,
                post.authority.neutral_lamport_sink,
            )?;
            post.retained[loser] = DirectCandidateLeaseV3::ZERO;
            loser += 1;
        }
        post.retained[0].stage = DirectCandidateStageV3::Selected;
        post.live_candidate_mask = 1;
        post.phase = DirectLifecyclePhaseV3::Selected;
        post.reservation_state = DirectReservationStateV3::Entitled;
        post.reservations[0] = post.reservations[0].entitled()?;
        post.reservations[1] = post.reservations[1].entitled()?;
        post.receipt_account = receipt_account;
        post.pot_account = pot_account;
        post.selected_slot = context.now;
        post.validate()?;
        Ok(DirectTransitionPlanV3 { post, effects })
    }

    /// Complete exact settlement from the selected Candidate facts.
    pub fn settle(
        self,
        context: DirectTransitionContextV3,
        observed: DirectObservedBalancesV3,
        positions: DirectSettlementPositionsV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_release(context)?;
        if self.phase != DirectLifecyclePhaseV3::Selected {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        if context.now < self.selected_slot || context.now >= self.schedule.settlement_deadline_slot
        {
            return Err(DirectLifecycleErrorV3::SettlementClosed);
        }
        let selected = self.retained[0];
        let price = selected.candidate.prices[usize::from(selected.candidate.outcome)];
        let consideration = u128::from(selected.candidate.quantity)
            .checked_mul(u128::from(price))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let receipt = DirectTerminalReceiptV3 {
            reason: DirectTerminalReasonV3::Settled,
            terminal_reservation_count: 2,
            selected_slot: self.selected_slot,
            candidate_id: selected.candidate.candidate_id,
            relation_candidate_digest: selected.candidate.relation_candidate_digest,
            outcome: selected.candidate.outcome,
            quantity: selected.candidate.quantity,
            price,
            consideration_price_units: consideration,
            terminal_slot: context.now,
        };
        self.finish_terminal(
            receipt,
            DirectReservationTransitionV3::EntitledToConsumed,
            self.rewards.settle,
            observed,
            None,
            Some(positions),
        )
    }

    /// Permissionlessly terminate every frozen lifecycle phase after deadline.
    pub fn lapse(
        self,
        now: u64,
        observed: DirectObservedBalancesV3,
        positions: DirectPositionSetV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        let (reason, reservation_transition) = match self.phase {
            DirectLifecyclePhaseV3::FrozenEmpty if now >= self.schedule.selection_deadline_slot => {
                (
                    DirectTerminalReasonV3::EmptyLapse,
                    DirectReservationTransitionV3::ActiveToReleased,
                )
            }
            DirectLifecyclePhaseV3::WindowOpen | DirectLifecyclePhaseV3::Verifying
                if now >= self.schedule.selection_deadline_slot =>
            {
                (
                    DirectTerminalReasonV3::PreSelectionLapse,
                    DirectReservationTransitionV3::ActiveToReleased,
                )
            }
            DirectLifecyclePhaseV3::Selected if now >= self.schedule.settlement_deadline_slot => (
                DirectTerminalReasonV3::PostSelectionLapse,
                DirectReservationTransitionV3::EntitledToReleased,
            ),
            DirectLifecyclePhaseV3::Terminal => return Err(DirectLifecycleErrorV3::Replay),
            _ => return Err(DirectLifecycleErrorV3::SelectionWindow),
        };
        let selected = if self.top_count == 0 {
            DirectCandidateLeaseV3::ZERO
        } else {
            self.retained[0]
        };
        let receipt = DirectTerminalReceiptV3 {
            reason,
            terminal_reservation_count: 2,
            selected_slot: if reason == DirectTerminalReasonV3::PostSelectionLapse {
                self.selected_slot
            } else {
                0
            },
            candidate_id: if reason == DirectTerminalReasonV3::PostSelectionLapse {
                selected.candidate.candidate_id
            } else {
                Identity32V1::ZERO
            },
            relation_candidate_digest: if reason == DirectTerminalReasonV3::PostSelectionLapse {
                selected.candidate.relation_candidate_digest
            } else {
                Identity32V1::ZERO
            },
            outcome: 0,
            quantity: 0,
            price: 0,
            consideration_price_units: 0,
            terminal_slot: now,
        };
        self.finish_terminal(
            receipt,
            reservation_transition,
            self.rewards.lapse,
            observed,
            Some(positions),
            None,
        )
    }

    /// Persist the canonical monotone DonationLedger for every account that is
    /// live at transition entry. Absent-account observations are exact zero.
    fn observe_live_accounts(
        &mut self,
        observed: DirectObservedBalancesV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        let sink = self.authority.neutral_lamport_sink;
        let mut index = 0usize;
        while index < MAX_DIRECT_CANDIDATES {
            if self.live_candidate_mask & (1u8 << index) != 0 {
                self.retained[index].account =
                    self.retained[index]
                        .account
                        .observe(observed.candidates[index], 0, sink)?;
            } else if observed.candidates[index] != 0 {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            index += 1;
        }
        if self.window_account == DirectAccountLedgerV3::ZERO {
            if observed.window != 0 {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
        } else {
            self.window_account = self.window_account.observe(observed.window, 0, sink)?;
        }
        if self.receipt_account == DirectAccountLedgerV3::ZERO {
            if observed.receipt != 0 {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
        } else {
            self.receipt_account = self.receipt_account.observe(observed.receipt, 0, sink)?;
        }
        if self.pot_account == DirectAccountLedgerV3::ZERO {
            if observed.pot != 0 {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
        } else {
            self.pot_account = self.pot_account.observe(observed.pot, 0, sink)?;
        }
        self.work_budget_account = self.work_budget_account.observe(
            observed.work_budget,
            self.work_budget_balance,
            sink,
        )?;
        let mut reservation = 0usize;
        while reservation < self.reservation_accounts.len() {
            self.reservation_accounts[reservation] = self.reservation_accounts[reservation]
                .observe(observed.reservations[reservation], 0, sink)?;
            reservation += 1;
        }
        Ok(())
    }

    fn require_observations_unchanged(
        &self,
        observed: DirectObservedBalancesV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        let mut checked = *self;
        checked.observe_live_accounts(observed)?;
        if checked != *self {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_open_work_budget(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.work_budget_account
            .validate(self.authority.neutral_lamport_sink)?;
        self.reservation_accounts[0].validate(self.authority.neutral_lamport_sink)?;
        self.reservation_accounts[1].validate(self.authority.neutral_lamport_sink)?;
        if self.work_budget_sponsor.is_zero()
            || self.work_budget_initial_balance
                != self
                    .work_budget_balance
                    .checked_add(self.work_rewards_paid)
                    .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_frozen_authority(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.reservation_domain.validate()?;
        self.grid.validate()?;
        self.frozen_page.validate(self.reservation_domain, 2)?;
        if self.grid.grid_id != self.reservation_domain.price_grid_id
            || self.grid.price_scale != self.reservation_domain.price_scale
            || self.frozen_page.frozen != 1
            || self.relation_domain
                != self
                    .reservation_domain
                    .full_relation_domain(self.frozen_page.order_set_id)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        if self.phase == DirectLifecyclePhaseV3::Terminal {
            if self.reservations != [DirectReservationBodyV3::ZERO; 2] {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            return Ok(());
        }
        let expected_state = match self.reservation_state {
            DirectReservationStateV3::Active => 0,
            DirectReservationStateV3::Entitled => 2,
            DirectReservationStateV3::Terminal => {
                return Err(DirectLifecycleErrorV3::MismatchedBinding)
            }
        };
        let mut index = 0usize;
        while index < 2 {
            self.reservations[index].validate_frozen(
                self.reservation_domain,
                self.frozen_page.orders[index],
                index as u64 + 1,
                expected_state,
            )?;
            index += 1;
        }
        Ok(())
    }

    fn direct_orders(
        &self,
    ) -> Result<(DirectPrefreezeOrderV3, DirectPrefreezeOrderV3, u8, u8), DirectLifecycleErrorV3>
    {
        let zero = self.frozen_page.orders[0];
        let one = self.frozen_page.orders[1];
        match (zero.side, one.side) {
            (0, 1) => Ok((zero, one, 0, 1)),
            (1, 0) => Ok((one, zero, 1, 0)),
            _ => Err(DirectLifecycleErrorV3::MismatchedBinding),
        }
    }

    fn bind_candidate(&self, candidate: &DirectCandidateV2) -> Result<(), DirectLifecycleErrorV3> {
        let (buy, sell, buy_index, sell_index) = self.direct_orders()?;
        if candidate.market_id != self.reservation_domain.market_id
            || candidate.epoch_id != self.reservation_domain.epoch_id
            || candidate.policy_id != self.reservation_domain.policy_id
            || candidate.order_set_id != self.frozen_page.order_set_id
            || candidate.outcome != buy.outcome
            || candidate.quantity != buy.quantity
            || candidate.buy_index != buy_index
            || candidate.sell_index != sell_index
            || buy.outcome != sell.outcome
            || buy.quantity != sell.quantity
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn verification_facts(&self) -> Result<DirectVerificationFactsV3, DirectLifecycleErrorV3> {
        let (buy, sell, buy_index, sell_index) = self.direct_orders()?;
        Ok(DirectVerificationFactsV3 {
            buy_limit: buy.limit,
            sell_limit: sell.limit,
            quantity: buy.quantity,
            buy_index,
            sell_index,
            outcome: buy.outcome,
            submission_opens_slot: self.schedule.submission_opens_slot,
            submission_closes_slot: self.schedule.submission_closes_slot,
        })
    }

    fn validate_terminal(&self) -> Result<(), DirectLifecycleErrorV3> {
        if self.top_count != 0
            || self.top != [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES]
            || self.retained != [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES]
            || self.live_candidate_mask != 0
            || self.window_account != DirectAccountLedgerV3::ZERO
            || self.verification_mask != 0
            || self.receipt_account != DirectAccountLedgerV3::ZERO
            || self.pot_account != DirectAccountLedgerV3::ZERO
            || self.work_budget_sponsor != Identity32V1::ZERO
            || self.work_budget_account != DirectAccountLedgerV3::ZERO
            || self.work_budget_balance != 0
            || self.work_budget_initial_balance != 0
            || self.work_rewards_paid != 0
            || self.reservation_accounts != [DirectAccountLedgerV3::ZERO; 2]
            || self.reservation_state != DirectReservationStateV3::Terminal
            || self.selected_slot != 0
            || self.terminal_receipt.terminal_slot == 0
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let receipt = self.terminal_receipt;
        match receipt.reason {
            DirectTerminalReasonV3::PrefreezeAbort => {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            DirectTerminalReasonV3::EmptyLapse | DirectTerminalReasonV3::PreSelectionLapse => {
                if receipt.terminal_reservation_count != 2
                    || receipt.selected_slot != 0
                    || !receipt.candidate_id.is_zero()
                    || !receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome != 0
                    || receipt.quantity != 0
                    || receipt.price != 0
                    || receipt.consideration_price_units != 0
                    || receipt.terminal_slot < self.schedule.selection_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
            DirectTerminalReasonV3::PostSelectionLapse => {
                if receipt.terminal_reservation_count != 2
                    || receipt.selected_slot < self.schedule.submission_closes_slot
                    || receipt.selected_slot >= self.schedule.selection_deadline_slot
                    || receipt.candidate_id.is_zero()
                    || receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome != 0
                    || receipt.quantity != 0
                    || receipt.price != 0
                    || receipt.consideration_price_units != 0
                    || receipt.terminal_slot < self.schedule.settlement_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
            DirectTerminalReasonV3::Settled => {
                let consideration = u128::from(receipt.quantity)
                    .checked_mul(u128::from(receipt.price))
                    .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
                if receipt.terminal_reservation_count != 2
                    || receipt.selected_slot < self.schedule.submission_closes_slot
                    || receipt.selected_slot >= self.schedule.selection_deadline_slot
                    || receipt.candidate_id.is_zero()
                    || receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome >= 2
                    || receipt.quantity == 0
                    || receipt.price == 0
                    || receipt.consideration_price_units != consideration
                    || receipt.terminal_slot < receipt.selected_slot
                    || receipt.terminal_slot >= self.schedule.settlement_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
        }
        Ok(())
    }

    fn require_stages(&self, reverified: bool) -> Result<(), DirectLifecycleErrorV3> {
        let expected = if reverified {
            DirectCandidateStageV3::Reverified
        } else {
            DirectCandidateStageV3::Verified
        };
        let mut index = 0usize;
        while index < usize::from(self.top_count) {
            if self.retained[index].stage != expected {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            index += 1;
        }
        Ok(())
    }

    fn require_release(
        &self,
        context: DirectTransitionContextV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if context.verifier_release_id != self.authority.verifier_release_id {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn require_selection_slot(&self, now: u64) -> Result<(), DirectLifecycleErrorV3> {
        if now < self.schedule.submission_closes_slot
            || now >= self.schedule.selection_deadline_slot
        {
            return Err(DirectLifecycleErrorV3::SelectionWindow);
        }
        Ok(())
    }

    fn debit_reward(&mut self, reward: u64) -> Result<u64, DirectLifecycleErrorV3> {
        self.work_budget_balance = self
            .work_budget_balance
            .checked_sub(reward)
            .ok_or(DirectLifecycleErrorV3::WorkBudgetInsufficient)?;
        self.work_rewards_paid = self
            .work_rewards_paid
            .checked_add(reward)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(reward)
    }

    fn finish_terminal(
        self,
        receipt: DirectTerminalReceiptV3,
        reservation_transition: DirectReservationTransitionV3,
        keeper_reward: u64,
        observed: DirectObservedBalancesV3,
        release_positions: Option<DirectPositionSetV3>,
        settlement_positions: Option<DirectSettlementPositionsV3>,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        let mut post = self;
        post.observe_live_accounts(observed)?;
        let reward = post.debit_reward(keeper_reward)?;
        let mut effects = DirectLifecycleEffectsV3 {
            keeper_reward: reward,
            reservation_transition,
            reservation_transition_count: 2,
            ..DirectLifecycleEffectsV3::NONE
        };
        match reservation_transition {
            DirectReservationTransitionV3::ActiveToReleased
            | DirectReservationTransitionV3::EntitledToReleased => {
                if settlement_positions.is_some() {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                let positions =
                    release_positions.ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
                release_reservations(
                    post.reservation_domain,
                    [post.frozen_page.orders[0], post.frozen_page.orders[1]],
                    post.reservations,
                    2,
                    positions,
                    &mut effects,
                )?;
            }
            DirectReservationTransitionV3::EntitledToConsumed => {
                let positions =
                    settlement_positions.ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
                if release_positions.is_some() {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                if post.reservations[0].state != 2 || post.reservations[1].state != 2 {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                settle_reservations(&post, positions, &mut effects)?;
            }
            DirectReservationTransitionV3::None
            | DirectReservationTransitionV3::ActiveToEntitled => {
                return Err(DirectLifecycleErrorV3::MismatchedBinding)
            }
        }
        let sink = post.authority.neutral_lamport_sink;
        let mut index = 0usize;
        while index < usize::from(post.top_count) {
            if post.live_candidate_mask & (1u8 << index) != 0 {
                effects.push_close(
                    DirectClosedAuthorityV3::Candidate(index as u8),
                    post.retained[index].account,
                    observed.candidates[index],
                    0,
                    sink,
                )?;
            }
            index += 1;
        }
        if post.window_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Window,
                post.window_account,
                observed.window,
                0,
                sink,
            )?;
        }
        if post.receipt_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Receipt,
                post.receipt_account,
                observed.receipt,
                0,
                sink,
            )?;
        }
        if post.pot_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Pot,
                post.pot_account,
                observed.pot,
                0,
                sink,
            )?;
        }
        let work_live_after_reward = observed
            .work_budget
            .checked_sub(reward)
            .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
        effects.push_close(
            DirectClosedAuthorityV3::WorkBudget,
            post.work_budget_account,
            work_live_after_reward,
            post.work_budget_balance,
            sink,
        )?;
        effects.work_budget_refund = post.work_budget_balance;
        effects.work_budget_refund_recipient = post.work_budget_sponsor;
        let mut reservation_index = 0usize;
        while reservation_index < 2 {
            effects.push_close(
                DirectClosedAuthorityV3::Reservation(reservation_index as u8),
                post.reservation_accounts[reservation_index],
                observed.reservations[reservation_index],
                0,
                sink,
            )?;
            reservation_index += 1;
        }
        post.phase = DirectLifecyclePhaseV3::Terminal;
        post.top_count = 0;
        post.top = [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES];
        post.retained = [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES];
        post.live_candidate_mask = 0;
        post.window_account = DirectAccountLedgerV3::ZERO;
        post.verification_mask = 0;
        post.receipt_account = DirectAccountLedgerV3::ZERO;
        post.pot_account = DirectAccountLedgerV3::ZERO;
        post.work_budget_sponsor = Identity32V1::ZERO;
        post.work_budget_account = DirectAccountLedgerV3::ZERO;
        post.work_budget_balance = 0;
        post.work_budget_initial_balance = 0;
        post.work_rewards_paid = 0;
        post.reservation_accounts = [DirectAccountLedgerV3::ZERO; 2];
        post.reservations = [DirectReservationBodyV3::ZERO; 2];
        post.reservation_state = DirectReservationStateV3::Terminal;
        post.selected_slot = 0;
        post.terminal_receipt = receipt;
        post.validate()?;
        Ok(DirectTransitionPlanV3 { post, effects })
    }
}

fn validate_prefreeze_observation_shape(
    observed: DirectObservedBalancesV3,
    reservation_count: u8,
) -> Result<(), DirectLifecycleErrorV3> {
    if reservation_count > 2
        || observed.candidates != [0; MAX_DIRECT_CANDIDATES]
        || observed.window != 0
        || observed.receipt != 0
        || observed.pot != 0
        || observed.work_budget != 0
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let mut index = usize::from(reservation_count);
    while index < observed.reservations.len() {
        if observed.reservations[index] != 0 {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        index += 1;
    }
    Ok(())
}

fn canonical_direct_order_id(rank: u64) -> Result<Identity32V1, DirectLifecycleErrorV3> {
    if rank == 0 || rank > 16 {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&rank.to_be_bytes());
    Ok(Identity32V1(bytes))
}

fn canonical_direct_reservation_id(
    market: Identity32V1,
    epoch: Identity32V1,
    owner: Identity32V1,
    position_generation: u64,
    order_id: Identity32V1,
) -> Identity32V1 {
    let mut hasher = Sha256::new();
    hasher.update(RESERVATION_DOMAIN_V1);
    hasher.update(market.0);
    hasher.update(epoch.0);
    hasher.update(owner.0);
    hasher.update(position_generation.to_le_bytes());
    hasher.update(order_id.0);
    Identity32V1(hasher.finalize().into())
}

fn direct_reservation_envelope(
    order: DirectPrefreezeOrderV3,
    domain: DirectReservationDomainV3,
    max_fee_atoms: u64,
) -> Result<(u64, [u64; MAX_OUTCOMES]), DirectLifecycleErrorV3> {
    order.validate(order_id_rank(order.order_id)?, domain)?;
    let mut internal = [0u64; MAX_OUTCOMES];
    let cash = if order.side == 0 {
        let price_units = u128::from(order.quantity)
            .checked_mul(u128::from(order.limit))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let scale = u128::from(domain.price_scale);
        let atoms = price_units
            .checked_add(scale - 1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
            / scale;
        u64::try_from(atoms)
            .map_err(|_| DirectLifecycleErrorV3::ArithmeticOverflow)?
            .checked_add(max_fee_atoms)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
    } else {
        internal[usize::from(order.outcome)] = order.quantity;
        0
    };
    // A reservation that encumbers nothing is refused at creation: its release
    // is a Position no-op, and the release kernel's unchanged-poststate refusal
    // would otherwise make every abort/lapse path permanently refuse. For a
    // sell the Egg envelope is always the exact quantity, so only a buy with
    // `limit == 0` and `max_fee_atoms == 0` can reach this.
    if cash == 0 && internal == [0u64; MAX_OUTCOMES] {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    Ok((cash, internal))
}

fn order_id_rank(order_id: Identity32V1) -> Result<u64, DirectLifecycleErrorV3> {
    if order_id.0[..24].iter().any(|byte| *byte != 0) {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    let mut rank = [0u8; 8];
    rank.copy_from_slice(&order_id.0[24..]);
    let rank = u64::from_be_bytes(rank);
    if rank == 0 || rank > 16 {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    Ok(rank)
}

fn prepare_direct_reservation_placement(
    domain: DirectReservationDomainV3,
    order: DirectPrefreezeOrderV3,
    position: DirectPositionV3,
    max_fee_atoms: u64,
    stored_bump: u8,
) -> Result<(DirectPositionV3, DirectReservationBodyV3), DirectLifecycleErrorV3> {
    domain.validate()?;
    position.validate()?;
    let rank = order_id_rank(order.order_id)?;
    order.validate(rank, domain)?;
    if position.market_id != domain.market_id
        || position.owner != order.owner
        || position.close_state != 0
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let (cash, internal) = direct_reservation_envelope(order, domain, max_fee_atoms)?;
    let mut post = position;
    let free = post
        .cash_atoms
        .checked_sub(post.reserved_cash_atoms)
        .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
    if cash > free {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    post.reserved_cash_atoms = post
        .reserved_cash_atoms
        .checked_add(cash)
        .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        post.internal[index] = post.internal[index]
            .checked_sub(internal[index])
            .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
        index += 1;
    }
    post.validate()?;
    let reservation = DirectReservationBodyV3 {
        reservation_id: canonical_direct_reservation_id(
            domain.market_id,
            domain.epoch_id,
            order.owner,
            position.generation,
            order.order_id,
        ),
        market_id: domain.market_id,
        epoch_id: domain.epoch_id,
        owner: order.owner,
        order_id: order.order_id,
        price_grid_id: domain.price_grid_id,
        terms_id: domain.terms_id,
        policy_id: domain.policy_id,
        position_generation: position.generation,
        order_generation: order.generation,
        initial_cash_atoms: cash,
        remaining_cash_atoms: cash,
        max_fee_atoms,
        release_generation: 0,
        page_index: 0,
        outcome_count: domain.outcome_count,
        order_kind: 1,
        side: order.side,
        state: 0,
        stored_bump,
        flags: 0,
        initial_internal: internal,
        remaining_internal: internal,
    };
    reservation.validate_active(domain, order, rank)?;
    Ok((post, reservation))
}

fn release_reservations(
    domain: DirectReservationDomainV3,
    orders: [DirectPrefreezeOrderV3; 2],
    reservations: [DirectReservationBodyV3; 2],
    reservation_count: u8,
    positions: DirectPositionSetV3,
    effects: &mut DirectLifecycleEffectsV3,
) -> Result<(), DirectLifecycleErrorV3> {
    positions.validate(domain.market_id)?;
    let mut expected_position_count = 0usize;
    let mut reservation_index = 0usize;
    while reservation_index < usize::from(reservation_count) {
        let owner = orders[reservation_index].owner;
        let mut prior = 0usize;
        let mut seen = false;
        while prior < reservation_index {
            if orders[prior].owner == owner {
                seen = true;
            }
            prior += 1;
        }
        if !seen {
            if expected_position_count >= usize::from(positions.count)
                || positions.positions[expected_position_count].owner != owner
                || positions.positions[expected_position_count].generation
                    != reservations[reservation_index].position_generation
            {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            expected_position_count += 1;
        }
        reservation_index += 1;
    }
    if expected_position_count != usize::from(positions.count) {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let mut working = positions.positions;
    let mut reservation_index = 0usize;
    while reservation_index < usize::from(reservation_count) {
        let reservation = reservations[reservation_index];
        let mut found = None;
        let mut position_index = 0usize;
        while position_index < usize::from(positions.count) {
            let position = working[position_index];
            if position.owner == reservation.owner {
                if position.generation != reservation.position_generation || found.is_some() {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                found = Some(position_index);
            }
            position_index += 1;
        }
        let position_index = found.ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
        working[position_index].reserved_cash_atoms = working[position_index]
            .reserved_cash_atoms
            .checked_sub(reservation.remaining_cash_atoms)
            .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            working[position_index].internal[outcome] = working[position_index].internal[outcome]
                .checked_add(reservation.remaining_internal[outcome])
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            outcome += 1;
        }
        working[position_index].validate()?;
        effects.terminal_reservations[reservation_index] = reservation.released()?;
        reservation_index += 1;
    }
    let mut position_index = 0usize;
    while position_index < usize::from(positions.count) {
        if working[position_index] == positions.positions[position_index] {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        effects.position_releases[position_index] = DirectPositionReleaseV3 {
            before: positions.positions[position_index],
            after: working[position_index],
        };
        position_index += 1;
    }
    effects.position_release_count = positions.count;
    Ok(())
}

/// Apply the exact economic tail of the live same-page full-fill, zero-fee
/// settlement kernel. The two Position inputs are in frozen page order; buy
/// and sell orientation is derived from the page rather than caller supplied.
fn settle_reservations(
    state: &DirectLifecycleV3,
    positions: DirectSettlementPositionsV3,
    effects: &mut DirectLifecycleEffectsV3,
) -> Result<(), DirectLifecycleErrorV3> {
    positions.buyer.validate()?;
    positions.seller.validate()?;
    if state.reservation_state != DirectReservationStateV3::Entitled
        || positions.buyer.market_id != state.reservation_domain.market_id
        || positions.seller.market_id != state.reservation_domain.market_id
        || positions.buyer.owner == positions.seller.owner
        || positions.buyer.close_state != 0
        || positions.seller.close_state != 0
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }

    let (buy, sell, buy_index, sell_index) = state.direct_orders()?;
    let buyer_reservation = state.reservations[usize::from(buy_index)];
    let seller_reservation = state.reservations[usize::from(sell_index)];
    if positions.buyer.owner != buy.owner
        || positions.seller.owner != sell.owner
        || positions.buyer.generation != buyer_reservation.position_generation
        || positions.seller.generation != seller_reservation.position_generation
        || buyer_reservation.owner != buy.owner
        || seller_reservation.owner != sell.owner
        || buyer_reservation.order_id != buy.order_id
        || seller_reservation.order_id != sell.order_id
        || buyer_reservation.state != 2
        || seller_reservation.state != 2
        || buyer_reservation.max_fee_atoms != 0
        || seller_reservation.max_fee_atoms != 0
        || buy.minimum_fill != 0
        || sell.minimum_fill != 0
        || buy.flags != 0
        || sell.flags != 0
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let selected = state.retained[0].candidate;
    state.bind_candidate(&selected)?;
    let price = selected.prices[usize::from(selected.outcome)];
    if selected.quantity == 0
        || selected.quantity != buy.quantity
        || selected.quantity != sell.quantity
        || selected.fills != [selected.quantity; 2]
        || price > buy.limit
        || price < sell.limit
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let price_units = u128::from(selected.quantity)
        .checked_mul(u128::from(price))
        .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
    let scale = u128::from(state.reservation_domain.price_scale);
    if scale == 0 || price_units % scale != 0 {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let consideration_atoms = u64::try_from(price_units / scale)
        .map_err(|_| DirectLifecycleErrorV3::ArithmeticOverflow)?;
    if buyer_reservation.remaining_cash_atoms < consideration_atoms
        || seller_reservation.remaining_internal[usize::from(selected.outcome)] != selected.quantity
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }

    let mut buyer_post = positions.buyer;
    let mut seller_post = positions.seller;
    buyer_post.cash_atoms = buyer_post
        .cash_atoms
        .checked_sub(consideration_atoms)
        .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
    buyer_post.reserved_cash_atoms = buyer_post
        .reserved_cash_atoms
        .checked_sub(buyer_reservation.remaining_cash_atoms)
        .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
    buyer_post.internal[usize::from(selected.outcome)] = buyer_post.internal
        [usize::from(selected.outcome)]
    .checked_add(selected.quantity)
    .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
    seller_post.cash_atoms = seller_post
        .cash_atoms
        .checked_add(consideration_atoms)
        .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;

    let cash_before = positions
        .buyer
        .cash_atoms
        .checked_add(positions.seller.cash_atoms)
        .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
    let cash_after = buyer_post
        .cash_atoms
        .checked_add(seller_post.cash_atoms)
        .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
    if cash_before != cash_after {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    buyer_post.validate()?;
    seller_post.validate()?;
    effects.position_settlements = [
        DirectPositionSettlementV3 {
            before: positions.buyer,
            after: buyer_post,
        },
        DirectPositionSettlementV3 {
            before: positions.seller,
            after: seller_post,
        },
    ];
    let mut index = 0usize;
    while index < 2 {
        effects.terminal_reservations[index] = state.reservations[index].consumed()?;
        index += 1;
    }
    effects.position_settlement_count = 2;
    effects.settlement_consideration_atoms = consideration_atoms;
    effects.buyer_reserved_release_atoms = buyer_reservation.remaining_cash_atoms;
    Ok(())
}

fn prefix_mask(count: u8) -> Result<u8, DirectLifecycleErrorV3> {
    if usize::from(count) > MAX_DIRECT_CANDIDATES {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    if count == 0 {
        Ok(0)
    } else {
        1u8.checked_shl(u32::from(count))
            .and_then(|value| value.checked_sub(1))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)
    }
}

fn verify_lease(
    domain: &ValidatedDirectDomainV1,
    grid: &DirectGridV3,
    lease: DirectCandidateLeaseV3,
    facts: DirectVerificationFactsV3,
    expected_neutral_sink: Identity32V1,
) -> Result<(), DirectLifecycleErrorV3> {
    lease.validate_body()?;
    lease.account.validate(expected_neutral_sink)?;
    let candidate = lease.candidate;
    if candidate.quantity != facts.quantity
        || candidate.buy_index != facts.buy_index
        || candidate.sell_index != facts.sell_index
        || candidate.outcome != facts.outcome
        || candidate.submitted_slot < facts.submission_opens_slot
        || candidate.submitted_slot >= facts.submission_closes_slot
    {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    let input = DirectTwoOrderInputV1 {
        prices: candidate.prices,
        buy_limit: facts.buy_limit,
        sell_limit: facts.sell_limit,
        quantity: facts.quantity,
        submitted_slot: candidate.submitted_slot,
        buy_index: facts.buy_index,
        sell_index: facts.sell_index,
        outcome: facts.outcome,
        stored_bump: candidate.stored_bump,
    };
    if domain.verify(input)? != candidate {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    grid.tick_of(candidate.prices[0])?;
    grid.tick_of(candidate.prices[1])?;
    if grid.tick_of(candidate.prices[usize::from(candidate.outcome)])? != lease.tick {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    Ok(())
}

fn next_transcript(
    previous: Identity32V1,
    count: u8,
    candidate: &DirectCandidateLeaseV3,
) -> Identity32V1 {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_DOMAIN_V3);
    hasher.update(previous.0);
    hasher.update([count]);
    hasher.update([candidate.tick]);
    hasher.update(candidate.candidate.candidate_id.0);
    hasher.update(candidate.candidate.relation_candidate_digest.0);
    Identity32V1(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch_policy_digest;
    use clutch_batch::relation_v1::PRICE_SCALE;

    const BUY_LIMIT: u64 = PRICE_SCALE;
    const SELL_LIMIT: u64 = 0;

    fn id(byte: u8) -> Identity32V1 {
        let mut value = [byte; 32];
        value[31] = byte.wrapping_add(1);
        Identity32V1(value)
    }

    fn schedule() -> DirectScheduleV3 {
        DirectScheduleV3 {
            submission_opens_slot: 10,
            submission_closes_slot: 20,
            selection_deadline_slot: 30,
            settlement_deadline_slot: 40,
        }
    }

    fn rewards() -> DirectKeeperRewardsV3 {
        DirectKeeperRewardsV3 {
            begin_verification: 2,
            verify_candidate: 3,
            finalize_selection: 5,
            settle: 7,
            lapse: 11,
        }
    }

    fn authority() -> DirectLifecycleAuthorityV3 {
        DirectLifecycleAuthorityV3 {
            verifier_release_id: id(80),
            direct_policy_v3_id: direct_policy_v3_digest(id(3), id(80)).unwrap(),
            neutral_lamport_sink: id(81),
        }
    }

    fn reservation_domain_for(grid: DirectGridV3) -> DirectReservationDomainV3 {
        DirectReservationDomainV3 {
            market_id: id(1),
            epoch_id: id(3),
            book_id: id(2),
            price_grid_id: grid.grid_id,
            terms_id: id(6),
            policy_id: batch_policy_digest(&crate::direct_window_v1::DIRECT_POLICY_V1).unwrap(),
            epoch_index: 7,
            price_scale: PRICE_SCALE,
            outcome_count: 2,
            remainder_seed: 9,
        }
    }

    fn reservation_domain() -> DirectReservationDomainV3 {
        reservation_domain_for(grid())
    }

    fn prefreeze_order(index: usize) -> DirectPrefreezeOrderV3 {
        DirectPrefreezeOrderV3 {
            owner: id(70 + index as u8),
            order_id: canonical_direct_order_id(index as u64 + 1).unwrap(),
            outcome: 0,
            side: index as u8,
            quantity: PRICE_SCALE,
            limit: if index == 0 { PRICE_SCALE } else { 0 },
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        }
    }

    fn prefreeze_position(index: usize) -> DirectPositionV3 {
        let mut internal = [0u64; MAX_OUTCOMES];
        if index == 1 {
            internal[0] = PRICE_SCALE;
        }
        DirectPositionV3 {
            market_id: reservation_domain().market_id,
            owner: prefreeze_order(index).owner,
            generation: 1,
            internal,
            cash_atoms: if index == 0 { PRICE_SCALE } else { 0 },
            reserved_cash_atoms: 0,
            stored_bump: 3 + index as u8,
            close_state: 0,
        }
    }

    fn position_set_one(position: DirectPositionV3) -> DirectPositionSetV3 {
        DirectPositionSetV3 {
            positions: [position, DirectPositionV3::ZERO],
            count: 1,
        }
    }

    fn position_set_two(first: DirectPositionV3, second: DirectPositionV3) -> DirectPositionSetV3 {
        DirectPositionSetV3 {
            positions: [first, second],
            count: 2,
        }
    }

    fn funded_position_set() -> DirectPositionSetV3 {
        let (first, _) = prepare_direct_reservation_placement(
            reservation_domain(),
            prefreeze_order(0),
            prefreeze_position(0),
            0,
            5,
        )
        .unwrap();
        let (second, _) = prepare_direct_reservation_placement(
            reservation_domain(),
            prefreeze_order(1),
            prefreeze_position(1),
            0,
            6,
        )
        .unwrap();
        position_set_two(first, second)
    }

    fn settlement_positions() -> DirectSettlementPositionsV3 {
        let funded = funded_position_set();
        DirectSettlementPositionsV3 {
            buyer: funded.positions[0],
            seller: funded.positions[1],
        }
    }

    fn placement(
        index: usize,
        position: DirectPositionV3,
        creation: DirectCreationFundingV3,
    ) -> DirectReservationPlacementV3 {
        DirectReservationPlacementV3 {
            order: prefreeze_order(index),
            position,
            max_fee_atoms: 0,
            stored_bump: 5 + index as u8,
            creation,
        }
    }

    fn context(now: u64) -> DirectTransitionContextV3 {
        DirectTransitionContextV3 {
            now,
            verifier_release_id: authority().verifier_release_id,
        }
    }

    fn rent(payer: u8, lamports: u64) -> DirectRentPrincipalV3 {
        DirectRentPrincipalV3 {
            payer: id(payer),
            lamports,
        }
    }

    fn create(payer: u8, lamports: u64, prefund: u64) -> DirectCreationFundingV3 {
        create_with_extra(payer, lamports, prefund, 0)
    }

    fn create_with_extra(
        payer: u8,
        lamports: u64,
        prefund: u64,
        extra: u64,
    ) -> DirectCreationFundingV3 {
        DirectCreationFundingV3 {
            rent: rent(payer, lamports),
            balance_before: prefund,
            balance_after: prefund + lamports + extra,
        }
    }

    fn work_funding() -> DirectWorkBudgetFundingV3 {
        DirectWorkBudgetFundingV3 {
            reward_sponsor: id(90),
            creation: create_with_extra(90, 500, 1, 100),
            reward_lamports: 100,
        }
    }

    fn frozen_plan_with_grid(
        schedule: DirectScheduleV3,
        frozen_grid: DirectGridV3,
    ) -> DirectInitializationPlanV3 {
        let domain = reservation_domain_for(frozen_grid);
        let empty =
            DirectPrefreezeV3::initialize(schedule, authority(), domain, frozen_grid, 4).unwrap();
        let one_plan = empty
            .place_reservation(
                context(schedule.submission_opens_slot - 2),
                placement(0, prefreeze_position(0), create(92, 600, 2)),
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap();
        let one = one_plan.post;
        let two = one
            .place_reservation(
                context(schedule.submission_opens_slot - 1),
                placement(1, prefreeze_position(1), create(93, 700, 3)),
                prefreeze_observed(&one, 0),
            )
            .unwrap()
            .post;
        two.freeze(
            context(schedule.submission_opens_slot - 1),
            rewards(),
            work_funding(),
            prefreeze_observed(&two, 0),
        )
        .unwrap()
    }

    fn frozen_plan(schedule: DirectScheduleV3) -> DirectInitializationPlanV3 {
        frozen_plan_with_grid(schedule, grid())
    }

    fn empty() -> DirectLifecycleV3 {
        frozen_plan(schedule()).post
    }

    fn prefreeze() -> DirectPrefreezeV3 {
        DirectPrefreezeV3::initialize(schedule(), authority(), reservation_domain(), grid(), 4)
            .unwrap()
    }

    fn prefreeze_observed(state: &DirectPrefreezeV3, later: u64) -> DirectObservedBalancesV3 {
        let mut observed = DirectObservedBalancesV3::ZERO;
        let mut index = 0usize;
        while index < usize::from(state.reservation_count) {
            observed.reservations[index] =
                account_balance(state.reservations[index].account, 0, later);
            index += 1;
        }
        observed
    }

    fn domain() -> ValidatedDirectDomainV1 {
        ValidatedDirectDomainV1::new(&frozen_plan(schedule()).post.relation_domain).unwrap()
    }

    fn verification_facts() -> DirectVerificationFactsV3 {
        frozen_plan(schedule()).post.verification_facts().unwrap()
    }

    fn grid() -> DirectGridV3 {
        let mut ticks = [0u64; MAX_DIRECT_TICKS_V3 as usize];
        let active = [
            0, 1_000, 2_000, 3_000, 4_000, 5_000, 6_000, 7_000, 8_000, 9_000, 10_000,
        ];
        ticks[..active.len()].copy_from_slice(&active);
        let mut grid = DirectGridV3 {
            grid_id: Identity32V1::ZERO,
            realm_id: id(8),
            price_scale: PRICE_SCALE,
            tick_count: active.len() as u8,
            ticks,
            stored_bump: 4,
            flags: 0,
        };
        grid.grid_id = grid.recomputed_grid_id();
        grid
    }

    fn issued(price: u64, slot: u64, payer: u8) -> DirectCandidateLeaseV3 {
        issued_on_grid(&grid(), price, slot, payer)
    }

    fn issued_on_grid(
        grid: &DirectGridV3,
        price: u64,
        slot: u64,
        payer: u8,
    ) -> DirectCandidateLeaseV3 {
        let domain = domain();
        let input = DirectTwoOrderInputV1 {
            prices: {
                let mut prices = [0u64; MAX_OUTCOMES];
                prices[0] = price;
                prices[1] = PRICE_SCALE - price;
                prices
            },
            buy_limit: BUY_LIMIT,
            sell_limit: SELL_LIMIT,
            quantity: PRICE_SCALE,
            submitted_slot: slot,
            buy_index: 0,
            sell_index: 1,
            outcome: 0,
            stored_bump: 7,
        };
        let candidate = domain.verify(input).unwrap();
        DirectCandidateLeaseV3::issue(
            &domain,
            grid,
            candidate,
            input,
            create(payer, 100 + price / 1_000, 1),
            authority().neutral_lamport_sink,
        )
        .unwrap()
    }

    fn admit_three() -> DirectLifecycleV3 {
        let empty = empty();
        let first = empty
            .admit(
                context(10),
                issued(2_000, 10, 11),
                create(21, 200, 2),
                observed(&empty, 0),
            )
            .unwrap();
        let first_observed = observed(&first.post, 0);
        let second = first
            .post
            .admit(
                context(11),
                issued(3_000, 11, 12),
                DirectCreationFundingV3::ZERO,
                first_observed,
            )
            .unwrap();
        let second_observed = observed(&second.post, 0);
        second
            .post
            .admit(
                context(12),
                issued(4_000, 12, 13),
                DirectCreationFundingV3::ZERO,
                second_observed,
            )
            .unwrap()
            .post
    }

    fn verify_all(state: DirectLifecycleV3) -> DirectLifecycleV3 {
        let begun = state
            .begin_verification(context(20), observed(&state, 0))
            .unwrap()
            .post;
        let mut post = begun;
        let count = post.top_count;
        let mut index = 0u8;
        while index < count {
            let balances = observed(&post, 0);
            post = post
                .verify_candidate(context(21 + u64::from(index)), index, balances)
                .unwrap()
                .post;
            index += 1;
        }
        post
    }

    fn observed(state: &DirectLifecycleV3, donation: u64) -> DirectObservedBalancesV3 {
        let mut out = DirectObservedBalancesV3::ZERO;
        let mut index = 0usize;
        while index < usize::from(state.top_count) {
            if state.live_candidate_mask & (1u8 << index) != 0 {
                out.candidates[index] = account_balance(state.retained[index].account, 0, donation);
            }
            index += 1;
        }
        if state.window_account != DirectAccountLedgerV3::ZERO {
            out.window = account_balance(state.window_account, 0, donation);
        }
        if state.receipt_account != DirectAccountLedgerV3::ZERO {
            out.receipt = account_balance(state.receipt_account, 0, donation);
        }
        if state.pot_account != DirectAccountLedgerV3::ZERO {
            out.pot = account_balance(state.pot_account, 0, donation);
        }
        if state.work_budget_account != DirectAccountLedgerV3::ZERO {
            out.work_budget = account_balance(
                state.work_budget_account,
                state.work_budget_balance,
                donation,
            );
        }
        let mut reservation = 0usize;
        while reservation < 2 {
            if state.reservation_accounts[reservation] != DirectAccountLedgerV3::ZERO {
                out.reservations[reservation] =
                    account_balance(state.reservation_accounts[reservation], 0, donation);
            }
            reservation += 1;
        }
        out
    }

    fn account_balance(account: DirectAccountLedgerV3, protected: u64, later: u64) -> u64 {
        account.rent.lamports + account.donation_lamports().unwrap() + protected + later
    }

    fn selected() -> DirectLifecycleV3 {
        let verified = verify_all(admit_three());
        let balances = observed(&verified, 0);
        verified
            .finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 1),
                    pot: create(51, 400, 1),
                },
                balances,
            )
            .unwrap()
            .post
    }

    #[test]
    fn schedule_reward_and_prefund_rules_are_exact() {
        let mut too_short = schedule();
        too_short.selection_deadline_slot = 24;
        assert_eq!(
            too_short.validate(),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut too_long = schedule();
        too_long.settlement_deadline_slot =
            too_long.selection_deadline_slot + MAX_SETTLEMENT_SPAN_V3 + 1;
        assert_eq!(
            too_long.validate(),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut zero = rewards();
        zero.verify_candidate = 0;
        assert_eq!(
            zero.validate(),
            Err(DirectLifecycleErrorV3::WorkBudgetInsufficient)
        );
        let initialized = frozen_plan(schedule());
        assert_eq!(initialized.effects, DirectLifecycleEffectsV3::NONE);
        assert_eq!(
            initialized.post.work_budget_account.donation_lamports(),
            Ok(1)
        );
        assert_eq!(
            initialized.post.reservation_accounts[0].donation_lamports(),
            Ok(2)
        );
    }

    #[test]
    fn prefreeze_abort_releases_exact_zero_one_or_two_reservation_prefix() {
        let empty = prefreeze();
        assert_eq!(
            empty.abort(9, DirectObservedBalancesV3::ZERO, DirectPositionSetV3::ZERO),
            Err(DirectLifecycleErrorV3::PrefreezeWindow)
        );
        let zero = empty
            .abort(
                10,
                DirectObservedBalancesV3::ZERO,
                DirectPositionSetV3::ZERO,
            )
            .unwrap();
        assert_eq!(
            zero.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PrefreezeAbort
        );
        assert_eq!(zero.effects.close_count, 0);
        assert_eq!(zero.effects.reservation_transition_count, 0);
        assert_eq!(zero.post.terminal_receipt.terminal_reservation_count, 0);
        assert_eq!(
            zero.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToReleased
        );

        let one_plan = empty
            .place_reservation(
                context(5),
                placement(0, prefreeze_position(0), create(92, 600, 2)),
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap();
        let one_position = one_plan.position_post;
        let one = one_plan.post;
        let one_abort = one
            .abort(
                10,
                prefreeze_observed(&one, 7),
                position_set_one(one_position),
            )
            .unwrap();
        assert_eq!(one_abort.effects.close_count, 1);
        assert_eq!(one_abort.effects.reservation_transition_count, 1);
        assert_eq!(one_abort.effects.position_release_count, 1);
        assert_eq!(one_abort.effects.position_releases[0].before, one_position);
        assert_eq!(
            one_abort.effects.position_releases[0].after,
            prefreeze_position(0)
        );
        assert_eq!(one_abort.effects.terminal_reservations[0].state, 1);
        assert_eq!(
            one_abort.effects.terminal_reservations[0].release_generation,
            2
        );
        assert_eq!(
            one_abort.post.terminal_receipt.terminal_reservation_count,
            1
        );
        assert_eq!(
            one_abort.post.terminal_receipt.candidate_id,
            one.reservations[0].reservation.reservation_id
        );
        assert_eq!(one_abort.effects.payer_principal_total(), Ok(600));
        assert_eq!(one_abort.effects.neutral_surplus_total(), Ok(9));

        let mut existing = prefreeze_observed(&one, 5);
        let two_plan = one
            .place_reservation(
                context(6),
                placement(1, prefreeze_position(1), create(93, 700, 3)),
                existing,
            )
            .unwrap();
        let two_position = two_plan.position_post;
        let two = two_plan.post;
        assert_eq!(two.reservations[0].account.donation_lamports(), Ok(7));
        let two_abort = two
            .abort(
                10,
                prefreeze_observed(&two, 11),
                position_set_two(one_position, two_position),
            )
            .unwrap();
        assert_eq!(two_abort.effects.close_count, 2);
        assert_eq!(two_abort.effects.reservation_transition_count, 2);
        assert_eq!(two_abort.effects.position_release_count, 2);
        assert_eq!(
            two_abort.effects.position_releases[0].after,
            prefreeze_position(0)
        );
        assert_eq!(
            two_abort.effects.position_releases[1].after,
            prefreeze_position(1)
        );
        assert_eq!(
            two_abort.post.terminal_receipt.terminal_reservation_count,
            2
        );
        assert_eq!(
            two_abort.post.terminal_receipt.relation_candidate_digest,
            two.reservations[1].reservation.reservation_id
        );
        assert_eq!(two_abort.effects.payer_principal_total(), Ok(1_300));
        assert_eq!(two_abort.effects.neutral_surplus_total(), Ok(32));

        existing = prefreeze_observed(&two, 0);
        assert_eq!(
            two.place_reservation(
                context(7),
                placement(1, prefreeze_position(1), create(94, 800, 4)),
                existing,
            ),
            Err(DirectLifecycleErrorV3::WrongPhase)
        );
    }

    #[test]
    fn prefreeze_reservation_substitution_and_release_aliases_refuse_atomically() {
        let empty = prefreeze();
        let first_plan = empty
            .place_reservation(
                context(5),
                placement(0, prefreeze_position(0), create(92, 600, 2)),
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap();
        let first_position = first_plan.position_post;
        let one = first_plan.post;
        let second_plan = one
            .place_reservation(
                context(6),
                placement(1, prefreeze_position(1), create(93, 700, 3)),
                prefreeze_observed(&one, 0),
            )
            .unwrap();
        let second_position = second_plan.position_post;
        let two = second_plan.post;
        let balances = prefreeze_observed(&two, 0);
        let positions = position_set_two(first_position, second_position);
        let snapshot = (two, balances, positions);

        let mut reordered = two;
        reordered.reservations.swap(0, 1);
        assert_eq!(
            reordered.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let mut wrong_policy = two;
        wrong_policy.reservation_domain.policy_id = id(99);
        assert_eq!(
            wrong_policy.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        // Flipping the sell to a buy leaves `limit == 0`, so the recomputed
        // envelope is the zero envelope refused at creation: NonCanonical.
        let mut wrong_side = two;
        wrong_side.reservations[1].order.side = 0;
        assert_eq!(
            wrong_side.validate(),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );

        let mut wrong_reservation = two;
        wrong_reservation.reservations[0]
            .reservation
            .remaining_cash_atoms -= 1;
        assert_eq!(
            wrong_reservation.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let mut missing = positions;
        missing.count = 1;
        missing.positions[1] = DirectPositionV3::ZERO;
        assert_eq!(
            two.abort(10, balances, missing),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut aliased = positions;
        aliased.positions[1].owner = aliased.positions[0].owner;
        assert_eq!(
            two.abort(10, balances, aliased),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut stale = positions;
        stale.positions[0].generation += 1;
        assert_eq!(
            two.abort(10, balances, stale),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut reversed_positions = positions;
        reversed_positions.positions.swap(0, 1);
        assert_eq!(
            two.abort(10, balances, reversed_positions),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        assert_eq!(snapshot, (two, balances, positions));
    }

    #[test]
    fn prefreeze_grid_page_and_direct_profile_are_exact() {
        let empty = prefreeze();
        let snapshot = empty;
        let mut off_grid = placement(0, prefreeze_position(0), create(92, 600, 2));
        off_grid.order.limit = 9_999;
        assert_eq!(
            empty.place_reservation(context(5), off_grid, DirectObservedBalancesV3::ZERO),
            Err(DirectLifecycleErrorV3::RelationRefused)
        );
        assert_eq!(empty, snapshot);

        for hostile in [
            {
                let mut value = empty;
                value.page.market_id = id(99);
                value
            },
            {
                let mut value = empty;
                value.page.epoch_id = id(99);
                value
            },
            {
                let mut value = empty;
                value.page.page_index = 1;
                value
            },
            {
                let mut value = empty;
                value.page.page_count = 2;
                value
            },
            {
                let mut value = empty;
                value.page.tombstone_count = 1;
                value
            },
            {
                let mut value = empty;
                value.page.frozen = 1;
                value
            },
            {
                let mut value = empty;
                value.page.slot_kinds[2] = 1;
                value.page.orders[2] = prefreeze_order(0);
                value
            },
            {
                let mut value = empty;
                value.grid.grid_id = id(99);
                value
            },
        ] {
            assert!(hostile.validate().is_err());
        }

        let one = empty
            .place_reservation(
                context(5),
                placement(0, prefreeze_position(0), create(92, 600, 2)),
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap()
            .post;
        let mut flagged = placement(1, prefreeze_position(1), create(93, 700, 3));
        flagged.order.minimum_fill = flagged.order.quantity;
        flagged.order.flags = 1;
        let flagged = one
            .place_reservation(context(6), flagged, prefreeze_observed(&one, 0))
            .unwrap()
            .post;
        assert_eq!(
            flagged.freeze(
                context(9),
                rewards(),
                work_funding(),
                prefreeze_observed(&flagged, 0),
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let reversed = prefreeze();
        let mut sell_first = placement(0, prefreeze_position(1), create(92, 600, 2));
        sell_first.order = prefreeze_order(0);
        sell_first.order.side = 1;
        sell_first.order.limit = 0;
        sell_first.position.owner = sell_first.order.owner;
        let reversed_one = reversed
            .place_reservation(context(5), sell_first, DirectObservedBalancesV3::ZERO)
            .unwrap()
            .post;
        let mut buy_second = placement(1, prefreeze_position(0), create(93, 700, 3));
        buy_second.order = prefreeze_order(1);
        buy_second.order.side = 0;
        buy_second.order.limit = PRICE_SCALE;
        buy_second.position.owner = buy_second.order.owner;
        let reversed_two = reversed_one
            .place_reservation(context(6), buy_second, prefreeze_observed(&reversed_one, 0))
            .unwrap()
            .post;
        assert!(reversed_two
            .freeze(
                context(9),
                rewards(),
                work_funding(),
                prefreeze_observed(&reversed_two, 0),
            )
            .is_ok());

        let frozen = frozen_plan(schedule()).post;
        assert_eq!(frozen.frozen_page.frozen, 1);
        assert_eq!(frozen.frozen_page.set_order_count, 2);
        assert_eq!(
            frozen.frozen_page.order_set_id,
            frozen.frozen_page.recomputed_order_set_id()
        );
        let mut hostile_reservation = frozen;
        hostile_reservation.reservations[0].remaining_cash_atoms -= 1;
        assert_eq!(
            hostile_reservation.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
    }

    #[test]
    fn prefreeze_abort_aggregates_two_reservations_into_one_position_once() {
        let empty = prefreeze();
        let buy = prefreeze_order(0);
        let mut shared_before = prefreeze_position(0);
        shared_before.internal[0] = PRICE_SCALE;
        let first = empty
            .place_reservation(
                context(5),
                DirectReservationPlacementV3 {
                    order: buy,
                    position: shared_before,
                    max_fee_atoms: 0,
                    stored_bump: 5,
                    creation: create(92, 600, 2),
                },
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap();
        let mut sell = prefreeze_order(1);
        sell.owner = buy.owner;
        let second = first
            .post
            .place_reservation(
                context(6),
                DirectReservationPlacementV3 {
                    order: sell,
                    position: first.position_post,
                    max_fee_atoms: 0,
                    stored_bump: 6,
                    creation: create(93, 700, 3),
                },
                prefreeze_observed(&first.post, 0),
            )
            .unwrap();
        assert_eq!(
            second.post.freeze(
                context(9),
                rewards(),
                work_funding(),
                prefreeze_observed(&second.post, 0),
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let aborted = second
            .post
            .abort(
                10,
                prefreeze_observed(&second.post, 0),
                position_set_one(second.position_post),
            )
            .unwrap();
        assert_eq!(aborted.effects.position_release_count, 1);
        assert_eq!(
            aborted.effects.position_releases[0].before,
            second.position_post
        );
        assert_eq!(aborted.effects.position_releases[0].after, shared_before);
        assert_eq!(aborted.effects.reservation_transition_count, 2);
    }

    #[test]
    fn prefreeze_freeze_is_exact_bounded_and_hostile_state_refuses() {
        let empty = prefreeze();
        assert_eq!(
            empty.freeze(
                context(9),
                rewards(),
                work_funding(),
                DirectObservedBalancesV3::ZERO,
            ),
            Err(DirectLifecycleErrorV3::WrongPhase)
        );
        let one = empty
            .place_reservation(
                context(5),
                placement(0, prefreeze_position(0), create(92, 600, 2)),
                DirectObservedBalancesV3::ZERO,
            )
            .unwrap()
            .post;
        let two = one
            .place_reservation(
                context(6),
                placement(1, prefreeze_position(1), create(93, 700, 3)),
                prefreeze_observed(&one, 0),
            )
            .unwrap()
            .post;
        let frozen = two
            .freeze(
                context(9),
                rewards(),
                work_funding(),
                prefreeze_observed(&two, 4),
            )
            .unwrap()
            .post;
        assert_eq!(frozen.phase, DirectLifecyclePhaseV3::FrozenEmpty);
        assert_eq!(frozen.reservation_accounts[0].donation_lamports(), Ok(6));
        assert_eq!(frozen.reservation_accounts[1].donation_lamports(), Ok(7));
        assert_eq!(frozen.work_budget_account.donation_lamports(), Ok(1));

        let mut late = context(10);
        assert_eq!(
            two.freeze(late, rewards(), work_funding(), prefreeze_observed(&two, 0),),
            Err(DirectLifecycleErrorV3::PrefreezeWindow)
        );
        late.now = 9;
        late.verifier_release_id = id(99);
        assert_eq!(
            two.freeze(late, rewards(), work_funding(), prefreeze_observed(&two, 0),),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let mut hostile_count = two;
        hostile_count.reservation_count = 3;
        assert_eq!(
            hostile_count.abort(
                10,
                DirectObservedBalancesV3::ZERO,
                DirectPositionSetV3::ZERO,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut hostile_padding = empty;
        hostile_padding.reservations[1] = two.reservations[1];
        assert_eq!(
            hostile_padding.abort(
                10,
                DirectObservedBalancesV3::ZERO,
                DirectPositionSetV3::ZERO,
            ),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut hostile_observation = prefreeze_observed(&one, 0);
        hostile_observation.reservations[1] = 1;
        assert_eq!(
            one.abort(10, hostile_observation, DirectPositionSetV3::ZERO),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
    }

    #[test]
    fn every_surviving_account_observes_monotone_donations() {
        let open = admit_three();
        let begun = open
            .begin_verification(context(20), observed(&open, 7))
            .unwrap()
            .post;
        let mut index = 0usize;
        while index < usize::from(begun.top_count) {
            assert_eq!(
                begun.retained[index].account.donation_lamports(),
                Ok(open.retained[index].account.donation_lamports().unwrap() + 7)
            );
            index += 1;
        }
        assert_eq!(
            begun.window_account.donation_lamports(),
            Ok(open.window_account.donation_lamports().unwrap() + 7)
        );
        assert_eq!(
            begun.work_budget_account.donation_lamports(),
            Ok(open.work_budget_account.donation_lamports().unwrap() + 7)
        );
        assert_eq!(
            begun.reservation_accounts[0].donation_lamports(),
            Ok(open.reservation_accounts[0].donation_lamports().unwrap() + 7)
        );
        assert_eq!(
            begun.reservation_accounts[1].donation_lamports(),
            Ok(open.reservation_accounts[1].donation_lamports().unwrap() + 7)
        );

        let mut drained = observed(&begun, 0);
        drained.window -= 1;
        assert_eq!(
            begun.verify_candidate(context(21), 0, drained,),
            Err(DirectLifecycleErrorV3::LivenessRefused)
        );
        assert_eq!(begun.validate(), Ok(()));

        let verified = begun
            .verify_candidate(context(21), 0, observed(&begun, 5))
            .unwrap()
            .post;
        assert_eq!(
            verified.window_account.donation_lamports(),
            Ok(begun.window_account.donation_lamports().unwrap() + 5)
        );
        assert_eq!(
            verified.work_budget_account.donation_lamports(),
            Ok(begun.work_budget_account.donation_lamports().unwrap() + 5)
        );
    }

    #[test]
    fn canonical_donation_ledgers_refuse_shortfall_and_compose_by_account_class() {
        let underfunded = DirectCreationFundingV3 {
            rent: rent(10, 100),
            balance_before: 7,
            balance_after: 106,
        };
        assert_eq!(
            underfunded.account(authority().neutral_lamport_sink, 0),
            Err(DirectLifecycleErrorV3::LivenessRefused)
        );

        let first = create(10, 100, 7)
            .account(authority().neutral_lamport_sink, 0)
            .unwrap();
        assert_eq!(
            DirectAccountLedgerV3::restore(
                first.rent,
                authority().neutral_lamport_sink,
                first.donation_lamports().unwrap(),
            ),
            Ok(first)
        );
        let second = create(11, 200, 9)
            .account(authority().neutral_lamport_sink, 0)
            .unwrap();
        let mut effects = DirectLifecycleEffectsV3::NONE;
        effects
            .push_close(
                DirectClosedAuthorityV3::Candidate(0),
                first,
                112,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        effects
            .push_close(
                DirectClosedAuthorityV3::Window,
                second,
                212,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        assert_eq!(effects.payer_principal_total(), Ok(300));
        assert_eq!(effects.neutral_surplus_total(), Ok(24));
        assert_eq!(effects.neutral_lamport_sink, id(81));
        assert_eq!(effects.close_legs[0].prior_donation_lamports, 7);
        assert_eq!(effects.close_legs[1].prior_donation_lamports, 9);

        let maximum = DirectCreationFundingV3 {
            rent: rent(12, u64::MAX),
            balance_before: 0,
            balance_after: u64::MAX,
        }
        .account(authority().neutral_lamport_sink, 0)
        .unwrap();
        let mut overflow = DirectLifecycleEffectsV3::NONE;
        overflow
            .push_close(
                DirectClosedAuthorityV3::Candidate(0),
                maximum,
                u64::MAX,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        overflow
            .push_close(
                DirectClosedAuthorityV3::Window,
                first,
                107,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        assert_eq!(
            overflow.payer_principal_total(),
            Err(DirectLifecycleErrorV3::ArithmeticOverflow)
        );
    }

    #[test]
    fn verified_input_binds_grid_tick_candidate_id_score_and_digest() {
        let valid = issued(2_000, 10, 11);
        let mut wrong_tick = valid;
        wrong_tick.tick = 1;
        assert_eq!(
            verify_lease(
                &domain(),
                &grid(),
                wrong_tick,
                verification_facts(),
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut wrong_score = valid;
        wrong_score.candidate.weighted_direct_volume += 1;
        assert_eq!(
            verify_lease(
                &domain(),
                &grid(),
                wrong_score,
                verification_facts(),
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let mut wrong_facts = verification_facts();
        wrong_facts.quantity += 1;
        assert_eq!(
            verify_lease(
                &domain(),
                &grid(),
                valid,
                wrong_facts,
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut wrong_facts = verification_facts();
        core::mem::swap(&mut wrong_facts.buy_index, &mut wrong_facts.sell_index);
        assert_eq!(
            verify_lease(
                &domain(),
                &grid(),
                valid,
                wrong_facts,
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        assert_eq!(
            verify_lease(&domain(), &grid(), valid, verification_facts(), id(82),),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        assert_eq!(
            verify_lease(
                &domain(),
                &grid(),
                issued(2_000, schedule().submission_opens_slot - 1, 11),
                verification_facts(),
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut wrong_id = valid.candidate;
        wrong_id.candidate_id = id(44);
        assert_eq!(
            DirectCandidateLeaseV3::issue(
                &domain(),
                &grid(),
                wrong_id,
                DirectTwoOrderInputV1 {
                    prices: valid.candidate.prices,
                    buy_limit: BUY_LIMIT,
                    sell_limit: SELL_LIMIT,
                    quantity: valid.candidate.quantity,
                    submitted_slot: valid.candidate.submitted_slot,
                    buy_index: valid.candidate.buy_index,
                    sell_index: valid.candidate.sell_index,
                    outcome: valid.candidate.outcome,
                    stored_bump: valid.candidate.stored_bump,
                },
                create(11, valid.account.rent.lamports, 1),
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::RelationRefused)
        );
    }

    #[test]
    fn submission_edges_noncompetitive_noop_and_replacement_conserve() {
        let state = empty();
        assert_eq!(
            state.admit(
                context(9),
                issued(2_000, 9, 11),
                create(21, 200, 0),
                observed(&state, 0),
            ),
            Err(DirectLifecycleErrorV3::SubmissionClosed)
        );
        assert!(state
            .admit(
                context(10),
                issued(2_000, 10, 11),
                create(21, 200, 0),
                observed(&state, 0),
            )
            .is_ok());
        assert_eq!(
            state.admit(
                context(20),
                issued(2_000, 20, 11),
                create(21, 200, 0),
                observed(&state, 0),
            ),
            Err(DirectLifecycleErrorV3::SubmissionClosed)
        );
        let full = admit_three();
        let rejected = full
            .admit(
                context(13),
                issued(1_000, 13, 14),
                DirectCreationFundingV3::ZERO,
                observed(&full, 0),
            )
            .unwrap();
        assert_eq!(
            rejected.disposition,
            DirectAdmissionDispositionV3::RejectedNonCompetitive
        );
        assert_eq!(rejected.post, full);
        assert_eq!(rejected.effects, DirectLifecycleEffectsV3::NONE);
        assert_eq!(
            full.admit(
                context(13),
                issued(1_000, 13, 14),
                DirectCreationFundingV3::ZERO,
                observed(&full, 1),
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let replacement_observed = observed(&full, 9);
        let replacement = full
            .admit(
                context(13),
                issued(5_000, 13, 14),
                DirectCreationFundingV3::ZERO,
                replacement_observed,
            )
            .unwrap();
        assert_eq!(
            replacement.disposition,
            DirectAdmissionDispositionV3::Replaced
        );
        assert_eq!(replacement.post.top_count, 3);
        assert_eq!(
            replacement.effects.payer_principal_total().unwrap(),
            full.retained[2].account.rent.lamports
        );
        assert_eq!(replacement.effects.neutral_surplus_total().unwrap(), 10);
        assert_eq!(
            replacement.post.admit(
                context(14),
                issued(2_000, 14, 11),
                DirectCreationFundingV3::ZERO,
                observed(&replacement.post, 0),
            ),
            Err(DirectLifecycleErrorV3::Replay)
        );
    }

    #[test]
    fn all_relation_admissible_ticks_are_bounded_by_one_bitmap() {
        let mut ticks = [0u64; MAX_DIRECT_TICKS_V3 as usize];
        ticks[0] = 0;
        let mut index = 1usize;
        while index < 32 {
            ticks[index] = index as u64;
            index += 1;
        }
        while index < 64 {
            ticks[index] = PRICE_SCALE - (63 - index) as u64;
            index += 1;
        }
        let mut grid = DirectGridV3 {
            grid_id: Identity32V1::ZERO,
            realm_id: id(8),
            price_scale: PRICE_SCALE,
            tick_count: MAX_DIRECT_TICKS_V3,
            ticks,
            stored_bump: 4,
            flags: 0,
        };
        grid.grid_id = grid.recomputed_grid_id();
        grid.validate().unwrap();
        let long_schedule = DirectScheduleV3 {
            submission_opens_slot: 10,
            submission_closes_slot: 100,
            selection_deadline_slot: 105,
            settlement_deadline_slot: 107,
        };
        let mut state = frozen_plan_with_grid(long_schedule, grid).post;
        // The relation correctly refuses the two zero-volume simplex
        // endpoints. Every one of the 62 competitive interior ticks is still
        // represented by the same 64-bit replay authority.
        let mut candidates = [DirectCandidateLeaseV3::ZERO; 62];
        let mut candidate_index = 0usize;
        while candidate_index < candidates.len() {
            candidates[candidate_index] = issued_on_grid(
                &grid,
                ticks[candidate_index + 1],
                10,
                (candidate_index + 1) as u8,
            );
            candidate_index += 1;
        }
        let mut unsorted = 1usize;
        while unsorted < candidates.len() {
            let mut cursor = unsorted;
            while cursor > 0
                && candidates[cursor - 1]
                    .score()
                    .is_better_than(&candidates[cursor].score())
            {
                candidates.swap(cursor - 1, cursor);
                cursor -= 1;
            }
            unsorted += 1;
        }
        let mut arrival = 0usize;
        while arrival < candidates.len() {
            let candidate = candidates[arrival];
            let window = if arrival == 0 {
                create(21, 200, 0)
            } else {
                DirectCreationFundingV3::ZERO
            };
            let balances = observed(&state, 0);
            state = state
                .admit(context(10), candidate, window, balances)
                .unwrap()
                .post;
            arrival += 1;
        }
        assert_eq!(state.competitive_admission_count, 62);
        assert_eq!(state.seen_competitive_ticks, (1u64 << 63) - 2);
        assert_eq!(state.top_count, MAX_DIRECT_CANDIDATES as u8);
        assert_eq!(state.live_candidate_mask, 0b111);
    }

    #[test]
    fn hostile_prestate_refuses_before_shift_or_mask() {
        let mut hostile = admit_three();
        hostile.top_count = 8;
        assert_eq!(
            hostile.begin_verification(context(20), DirectObservedBalancesV3::ZERO),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut hostile = admit_three();
        hostile.verification_mask = 0x80;
        assert_eq!(
            hostile.begin_verification(context(20), DirectObservedBalancesV3::ZERO),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut hostile = admit_three();
        hostile.retained[1].candidate.candidate_id = hostile.retained[0].candidate.candidate_id;
        assert!(hostile
            .begin_verification(context(20), DirectObservedBalancesV3::ZERO)
            .is_err());
        let mut hostile = admit_three();
        hostile.retained[2] = DirectCandidateLeaseV3::ZERO;
        assert!(hostile.validate().is_err());
    }

    #[test]
    fn every_selection_stage_checks_deadline_release_and_status_mask() {
        let open = admit_three();
        assert_eq!(
            open.begin_verification(context(19), observed(&open, 0)),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
        let mut wrong_release = context(20);
        wrong_release.verifier_release_id = id(99);
        assert_eq!(
            open.begin_verification(wrong_release, observed(&open, 0)),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let begun = open
            .begin_verification(context(20), observed(&open, 0))
            .unwrap()
            .post;
        let one = begun
            .verify_candidate(context(21), 0, observed(&begun, 0))
            .unwrap()
            .post;
        assert_eq!(
            one.verify_candidate(context(22), 0, observed(&one, 0),),
            Err(DirectLifecycleErrorV3::Replay)
        );
        assert_eq!(
            one.finalize_selection(
                context(22),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::VerificationIncomplete)
        );
        let mut hostile = verify_all(admit_three());
        hostile.retained[1].stage = DirectCandidateStageV3::Verified;
        assert_eq!(
            hostile.finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        assert_eq!(
            verify_all(admit_three()).finalize_selection(
                context(30),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
    }

    #[test]
    fn finalize_preserves_window_top_and_entitles_reservations() {
        let verified = verify_all(admit_three());
        let original_top = verified.top;
        let original_count = verified.top_count;
        let mut balances = observed(&verified, 0);
        balances.candidates[1] += 1;
        balances.candidates[2] += 2;
        let plan = verified
            .finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 4),
                    pot: create(51, 400, 5),
                },
                balances,
            )
            .unwrap();
        assert_eq!(plan.post.top_count, original_count);
        assert_eq!(plan.post.top, original_top);
        assert_eq!(plan.post.live_candidate_mask, 1);
        assert_eq!(plan.post.retained[1], DirectCandidateLeaseV3::ZERO);
        assert_eq!(plan.post.retained[2], DirectCandidateLeaseV3::ZERO);
        assert_eq!(plan.effects.close_count, 2);
        assert_eq!(plan.effects.neutral_surplus_total().unwrap(), 5);
        assert_eq!(plan.post.receipt_account.donation_lamports(), Ok(4));
        assert_eq!(plan.post.pot_account.donation_lamports(), Ok(5));
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToEntitled
        );
    }

    #[test]
    fn work_budget_equation_holds_for_partial_verification_lapse() {
        let initial = empty().work_budget_initial_balance;
        let open = admit_three();
        let begun = open
            .begin_verification(context(20), observed(&open, 0))
            .unwrap();
        let verified = begun
            .post
            .verify_candidate(context(21), 0, observed(&begun.post, 0))
            .unwrap();
        let before_lapse = verified.post;
        let plan = before_lapse
            .lapse(30, observed(&before_lapse, 0), funded_position_set())
            .unwrap();
        assert_eq!(
            initial,
            begun.effects.keeper_reward
                + verified.effects.keeper_reward
                + plan.effects.keeper_reward
                + plan.effects.work_budget_refund
        );
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToReleased
        );
    }

    #[test]
    fn empty_preselected_and_selected_lapse_are_distinct_and_exact() {
        let empty_state = empty();
        let empty_plan = empty_state
            .lapse(30, observed(&empty_state, 1), funded_position_set())
            .unwrap();
        assert_eq!(
            empty_plan.post.terminal_receipt.reason,
            DirectTerminalReasonV3::EmptyLapse
        );
        assert_eq!(empty_plan.effects.close_count, 3);
        assert_eq!(
            empty_plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToReleased
        );
        let open = admit_three();
        let pre = open
            .lapse(30, observed(&open, 1), funded_position_set())
            .unwrap();
        assert_eq!(
            pre.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PreSelectionLapse
        );
        assert_eq!(pre.effects.close_count, 7);
        let selected = selected();
        assert_eq!(
            selected.lapse(39, observed(&selected, 0), funded_position_set()),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
        let post = selected
            .lapse(40, observed(&selected, 1), funded_position_set())
            .unwrap();
        assert_eq!(
            post.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PostSelectionLapse
        );
        assert_eq!(
            post.post.terminal_receipt.selected_slot,
            selected.selected_slot
        );
        assert_eq!(post.post.terminal_receipt.terminal_reservation_count, 2);
        assert_eq!(
            post.effects.reservation_transition,
            DirectReservationTransitionV3::EntitledToReleased
        );
        assert_eq!(post.effects.close_count, 7);
    }

    #[test]
    fn settlement_derives_exact_economics_and_composite_conservation() {
        let state = selected();
        let initial_budget = state.work_budget_initial_balance;
        assert_eq!(
            state.settle(context(23), observed(&state, 0), settlement_positions()),
            Err(DirectLifecycleErrorV3::SettlementClosed)
        );
        assert_eq!(
            state.settle(context(40), observed(&state, 0), settlement_positions()),
            Err(DirectLifecycleErrorV3::SettlementClosed)
        );
        let balances = observed(&state, 2);
        let plan = state
            .settle(context(39), balances, settlement_positions())
            .unwrap();
        let receipt = plan.post.terminal_receipt;
        assert_eq!(receipt.reason, DirectTerminalReasonV3::Settled);
        assert_eq!(receipt.selected_slot, state.selected_slot);
        assert_eq!(receipt.terminal_reservation_count, 2);
        assert_eq!(receipt.quantity, PRICE_SCALE);
        assert_eq!(
            receipt.consideration_price_units,
            u128::from(receipt.quantity) * u128::from(receipt.price)
        );
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::EntitledToConsumed
        );
        assert_eq!(plan.effects.close_count, 7);
        assert_eq!(plan.effects.position_settlement_count, 2);
        let settlement = settlement_positions();
        let buyer_leg = plan.effects.position_settlements[0];
        let seller_leg = plan.effects.position_settlements[1];
        assert_eq!(buyer_leg.before, settlement.buyer);
        assert_eq!(seller_leg.before, settlement.seller);
        assert_eq!(
            buyer_leg.after.cash_atoms,
            buyer_leg.before.cash_atoms - plan.effects.settlement_consideration_atoms
        );
        assert_eq!(
            seller_leg.after.cash_atoms,
            seller_leg.before.cash_atoms + plan.effects.settlement_consideration_atoms
        );
        assert_eq!(
            buyer_leg.after.reserved_cash_atoms,
            buyer_leg.before.reserved_cash_atoms - plan.effects.buyer_reserved_release_atoms
        );
        assert_eq!(
            buyer_leg.after.internal[usize::from(receipt.outcome)],
            buyer_leg.before.internal[usize::from(receipt.outcome)] + receipt.quantity
        );
        assert_eq!(seller_leg.after.internal, seller_leg.before.internal);
        assert_eq!(plan.effects.terminal_reservations[0].state, 3);
        assert_eq!(plan.effects.terminal_reservations[1].state, 3);
        assert_eq!(
            plan.effects.terminal_reservations[0].remaining_cash_atoms,
            0
        );
        assert_eq!(
            plan.effects.terminal_reservations[1].remaining_internal,
            [0; MAX_OUTCOMES]
        );
        assert_eq!(
            buyer_leg.before.cash_atoms + seller_leg.before.cash_atoms,
            buyer_leg.after.cash_atoms + seller_leg.after.cash_atoms
        );
        assert_eq!(plan.effects.neutral_lamport_sink, id(81));
        assert_eq!(plan.effects.neutral_surplus_total(), Ok(25));
        assert_eq!(
            plan.effects.observed_close_total().unwrap(),
            plan.effects.payer_principal_total().unwrap()
                + plan.effects.work_budget_refund
                + plan.effects.neutral_surplus_total().unwrap()
        );
        assert_eq!(
            initial_budget,
            state.work_rewards_paid + plan.effects.keeper_reward + plan.effects.work_budget_refund
        );
        let work_leg = plan
            .effects
            .close_legs
            .iter()
            .find(|leg| leg.authority == DirectClosedAuthorityV3::WorkBudget)
            .unwrap();
        assert_eq!(work_leg.payer_principal_lamports, 500);
        assert_eq!(
            balances.work_budget,
            work_leg.observed_live_lamports + plan.effects.keeper_reward
        );
    }

    #[test]
    fn settlement_position_substitution_and_arithmetic_refuse_atomically() {
        let state = selected();
        let original = settlement_positions();
        let effects = DirectLifecycleEffectsV3::NONE;

        let mut swapped = DirectSettlementPositionsV3 {
            buyer: original.seller,
            seller: original.buyer,
        };
        assert_eq!(
            settle_reservations(&state, swapped, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        swapped = original;
        swapped.seller = swapped.buyer;
        assert_eq!(
            settle_reservations(&state, swapped, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut wrong = original;
        wrong.buyer.market_id = id(99);
        assert_eq!(
            settle_reservations(&state, wrong, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        wrong = original;
        wrong.buyer.generation += 1;
        assert_eq!(
            settle_reservations(&state, wrong, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        wrong = original;
        wrong.buyer.reserved_cash_atoms = 0;
        assert_eq!(
            settle_reservations(&state, wrong, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        wrong = original;
        wrong.buyer.internal[0] = u64::MAX;
        assert_eq!(
            settle_reservations(&state, wrong, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::ArithmeticOverflow)
        );
        wrong = original;
        wrong.seller.cash_atoms = u64::MAX;
        assert_eq!(
            settle_reservations(&state, wrong, &mut effects.clone()),
            Err(DirectLifecycleErrorV3::ArithmeticOverflow)
        );
        assert_eq!(original, settlement_positions());
        assert_eq!(effects, DirectLifecycleEffectsV3::NONE);
    }

    #[test]
    fn close_balance_shortfall_refuses_without_state() {
        let state = selected();
        let mut balances = observed(&state, 0);
        balances.candidates[0] = state.retained[0].account.rent.lamports - 1;
        assert_eq!(
            state.settle(context(39), balances, settlement_positions()),
            Err(DirectLifecycleErrorV3::LivenessRefused)
        );
        assert_eq!(state.validate(), Ok(()));
    }

    #[test]
    fn zero_envelope_buy_reservation_refuses_at_placement() {
        let empty = prefreeze();
        let mut order = prefreeze_order(0);
        order.limit = 0;
        let placement = DirectReservationPlacementV3 {
            order,
            position: prefreeze_position(0),
            max_fee_atoms: 0,
            stored_bump: 5,
            creation: create(92, 600, 2),
        };
        assert_eq!(
            empty.place_reservation(context(5), placement, DirectObservedBalancesV3::ZERO),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
    }

    #[test]
    fn fee_only_zero_limit_buy_places_and_releases_exactly() {
        let empty = prefreeze();
        let mut order = prefreeze_order(0);
        order.limit = 0;
        let placement = DirectReservationPlacementV3 {
            order,
            position: prefreeze_position(0),
            max_fee_atoms: 25,
            stored_bump: 5,
            creation: create(92, 600, 2),
        };
        let plan = empty
            .place_reservation(context(5), placement, DirectObservedBalancesV3::ZERO)
            .unwrap();
        assert_eq!(plan.post.reservations[0].reservation.initial_cash_atoms, 25);
        assert_eq!(plan.post.reservations[0].reservation.max_fee_atoms, 25);
        assert_eq!(plan.position_post.reserved_cash_atoms, 25);
        let aborted = plan
            .post
            .abort(
                10,
                prefreeze_observed(&plan.post, 0),
                position_set_one(plan.position_post),
            )
            .unwrap();
        assert_eq!(
            aborted.effects.position_releases[0].after,
            prefreeze_position(0)
        );
        assert_eq!(aborted.effects.terminal_reservations[0].state, 1);
        assert_eq!(
            aborted.effects.terminal_reservations[0].release_generation,
            2
        );
        assert_eq!(aborted.effects.payer_principal_total(), Ok(600));
        assert_eq!(
            aborted.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PrefreezeAbort
        );
    }

    #[test]
    fn direct_policy_v3_identity_binds_epoch_and_release() {
        let expected = direct_policy_v3_digest(id(3), id(80)).unwrap();
        assert_ne!(direct_policy_v3_digest(id(4), id(80)).unwrap(), expected);
        assert_ne!(direct_policy_v3_digest(id(3), id(81)).unwrap(), expected);
        assert_eq!(
            direct_policy_v3_digest(Identity32V1::ZERO, id(80)),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );

        let mut pre = prefreeze();
        pre.authority.direct_policy_v3_id = id(99);
        assert_eq!(
            pre.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        let mut wrong_id = empty();
        wrong_id.authority.direct_policy_v3_id = id(99);
        assert_eq!(
            wrong_id.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );

        // A substituted release identifier no longer matches the persisted
        // epoch-bound artifact identity, so the stale digest refuses.
        let mut swapped_release = empty();
        swapped_release.authority.verifier_release_id = id(82);
        assert_eq!(
            swapped_release.validate(),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
    }
}
