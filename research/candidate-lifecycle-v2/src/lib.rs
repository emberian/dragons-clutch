#![no_std]
#![forbid(unsafe_code)]

//! Executable model of the proposed two-window general candidate lifecycle.
//!
//! This crate deliberately models only state-machine facts. Candidate relation
//! verification and score computation are inputs from separately versioned
//! components; this model accepts only a policy-bound canonical rank key.

/// Model capacity. The protocol ADR permits versioned/paged deployment
/// capacities; this small fixed width keeps the executable transition model
/// allocation-free.
pub const MAX_MODEL_CANDIDATES: usize = 8;
/// The bounded verified acceleration registry.
pub const TOP_CAPACITY: usize = 3;
/// Fixed-capacity canonical score ordering output.
pub const RANK_KEY_BYTES: usize = 16;

/// Stable identity of one independently versioned score policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScorePolicyId(pub [u8; 32]);

impl ScorePolicyId {
    /// Canonical absent identity.
    pub const ZERO: Self = Self([0; 32]);
}

/// Score-policy output. Greater lexicographic bytes rank first.
///
/// A real score policy must include its candidate-identity tie component in
/// these bytes, making equal keys for distinct candidates a refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RankKey(pub [u8; RANK_KEY_BYTES]);

impl RankKey {
    /// Canonical absent key.
    pub const ZERO: Self = Self([0; RANK_KEY_BYTES]);
}

/// Half-open lifecycle interval at one slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interval {
    /// Candidate bytes may be staged and sealed.
    Submission,
    /// The sealed set is immutable and verification may progress.
    Verification,
    /// Verification is closed; deadline finalization and expiry are open.
    Terminal,
}

/// Immutable schedule stamped by the freeze transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Schedule {
    /// Slot observed by the successful freeze.
    pub frozen_slot: u64,
    /// Exclusive candidate seal boundary.
    pub submission_closes_slot: u64,
    /// Exclusive verification boundary and ordinary finalization opening.
    pub verification_closes_slot: u64,
}

impl Schedule {
    /// Construct the schedule using checked additions and nonzero spans.
    pub fn new(
        frozen_slot: u64,
        submission_span: u64,
        verification_span: u64,
    ) -> Result<Self, Error> {
        if submission_span == 0 || verification_span == 0 {
            return Err(Error::InvalidSchedule);
        }
        let submission_closes_slot = frozen_slot
            .checked_add(submission_span)
            .ok_or(Error::Arithmetic)?;
        let verification_closes_slot = submission_closes_slot
            .checked_add(verification_span)
            .ok_or(Error::Arithmetic)?;
        Ok(Self {
            frozen_slot,
            submission_closes_slot,
            verification_closes_slot,
        })
    }

    /// Classify one authoritative Clock slot.
    pub const fn interval(self, slot: u64) -> Result<Interval, Error> {
        if slot < self.frozen_slot {
            return Err(Error::NotActive);
        }
        if slot < self.submission_closes_slot {
            Ok(Interval::Submission)
        } else if slot < self.verification_closes_slot {
            Ok(Interval::Verification)
        } else {
            Ok(Interval::Terminal)
        }
    }
}

/// Frozen liveness economics. Every amount exists before its obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingPolicy {
    /// Reward per newly consumed verification unit.
    pub progress_reward: u64,
    /// Reward for the one expiry cleanup.
    pub cleanup_reward: u64,
    /// Bond forfeited only by a checked invalid verdict.
    pub invalidity_bond: u64,
    /// Fixed penalty for failing to seal an opened stage.
    pub abandonment_penalty: u64,
    /// Winner prize funded by the epoch sponsor.
    pub solver_prize: u64,
    /// Reward for the permissionless finalization transition.
    pub finalizer_reward: u64,
}

/// Candidate-local prepaid balances. Rent remains a distinct principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Escrow {
    /// Exact account rent principal, unavailable for rewards or penalties.
    pub rent_principal: u64,
    /// Remaining verification reward reserve.
    pub work_reward_balance: u64,
    /// Remaining invalidity/abandonment bond.
    pub bond_balance: u64,
    /// Remaining expiry cleanup reserve.
    pub cleanup_balance: u64,
    /// Rewards already paid for monotone progress and cleanup.
    pub keeper_paid: u64,
    /// Amount sent to the policy's non-Hoard penalty sink.
    pub slashed: u64,
    /// Winner prize credited by finalization.
    pub solver_credit: u64,
    /// Whether the refundable work/bond/cleanup balances were claimed.
    pub balances_refund_claimed: bool,
    /// Whether a later winner credit was claimed.
    pub solver_credit_claimed: bool,
}

impl Escrow {
    const ZERO: Self = Self {
        rent_principal: 0,
        work_reward_balance: 0,
        bond_balance: 0,
        cleanup_balance: 0,
        keeper_paid: 0,
        slashed: 0,
        solver_credit: 0,
        balances_refund_claimed: false,
        solver_credit_claimed: false,
    };
}

/// Candidate lifecycle state. Top-registry membership is deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateState {
    Empty,
    Staging,
    Sealed,
    VerifiedValid,
    VerifiedRefused,
    Selected,
    ExpiredStaging,
    ExpiredUnverified,
}

/// One model candidate record plus its independent escrow projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub id: u64,
    pub state: CandidateState,
    pub submitted_slot: u64,
    pub sealed_slot: u64,
    pub verdict_slot: u64,
    pub upload_cursor: u16,
    pub verification_progress: u16,
    pub verification_units: u16,
    pub rank_key: RankKey,
    pub escrow: Escrow,
}

impl Candidate {
    const EMPTY: Self = Self {
        id: 0,
        state: CandidateState::Empty,
        submitted_slot: 0,
        sealed_slot: 0,
        verdict_slot: 0,
        upload_cursor: 0,
        verification_progress: 0,
        verification_units: 0,
        rank_key: RankKey::ZERO,
        escrow: Escrow::ZERO,
    };

    const fn is_valid(self) -> bool {
        matches!(
            self.state,
            CandidateState::VerifiedValid | CandidateState::Selected
        )
    }
}

/// Coarse Epoch terminal state owned outside the Window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochState {
    Frozen,
    Cleared,
    Lapsed,
}

/// Valid or refused deterministic relation verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationVerdict {
    Valid {
        score_policy_id: ScorePolicyId,
        rank_key: RankKey,
    },
    Refused,
}

/// Observable transfer effects returned by state transitions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Payout {
    pub keeper_reward: u64,
    pub solver_credit: u64,
    pub refundable: u64,
    pub slashed: u64,
}

/// Explicit model refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Arithmetic,
    InvalidSchedule,
    InvalidPolicy,
    NotActive,
    Capacity,
    Duplicate,
    NotFound,
    Replay,
    MismatchedState,
    Underfunded,
    ScorePolicyMismatch,
    RankCollision,
    UnresolvedCandidates,
}

/// Fixed-capacity model of Window, candidate records, and prepaid epoch work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    pub schedule: Schedule,
    pub score_policy_id: ScorePolicyId,
    pub funding_policy: FundingPolicy,
    pub capacity: u8,
    pub sealed_candidate_count: u8,
    pub verdict_count: u8,
    pub top: [u64; TOP_CAPACITY],
    pub top_count: u8,
    pub epoch_state: EpochState,
    pub selected_candidate: u64,
    pub finalized_slot: u64,
    pub epoch_reward_balance: u64,
    pub finalizer_paid: u64,
    pub candidates: [Candidate; MAX_MODEL_CANDIDATES],
}

impl Window {
    /// Construct a fully prepaid frozen Window.
    pub fn new(
        schedule: Schedule,
        score_policy_id: ScorePolicyId,
        capacity: u8,
        funding: FundingPolicy,
        epoch_reward_balance: u64,
    ) -> Result<Self, Error> {
        if score_policy_id == ScorePolicyId::ZERO
            || capacity == 0
            || usize::from(capacity) > MAX_MODEL_CANDIDATES
            || funding.abandonment_penalty > funding.invalidity_bond
        {
            return Err(Error::InvalidPolicy);
        }
        let required = funding
            .solver_prize
            .checked_add(funding.finalizer_reward)
            .ok_or(Error::Arithmetic)?;
        if epoch_reward_balance < required {
            return Err(Error::Underfunded);
        }
        Ok(Self {
            schedule,
            score_policy_id,
            funding_policy: funding,
            capacity,
            sealed_candidate_count: 0,
            verdict_count: 0,
            top: [0; TOP_CAPACITY],
            top_count: 0,
            epoch_state: EpochState::Frozen,
            selected_candidate: 0,
            finalized_slot: 0,
            epoch_reward_balance,
            finalizer_paid: 0,
            candidates: [Candidate::EMPTY; MAX_MODEL_CANDIDATES],
        })
    }

    /// Create an inert staging record and fully prepay its bounded obligations.
    pub fn begin_candidate(
        &mut self,
        slot: u64,
        id: u64,
        verification_units: u16,
        rent_principal: u64,
    ) -> Result<(), Error> {
        self.require_epoch_frozen()?;
        if self.schedule.interval(slot)? != Interval::Submission {
            return Err(Error::NotActive);
        }
        if id == 0 || verification_units == 0 {
            return Err(Error::MismatchedState);
        }
        if self.find(id).is_ok() {
            return Err(Error::Duplicate);
        }
        let index = self
            .candidates
            .iter()
            .position(|candidate| candidate.state == CandidateState::Empty)
            .ok_or(Error::Capacity)?;
        let work_reward_balance = self
            .funding_policy
            .progress_reward
            .checked_mul(u64::from(verification_units))
            .ok_or(Error::Arithmetic)?;
        self.candidates[index] = Candidate {
            id,
            state: CandidateState::Staging,
            submitted_slot: slot,
            sealed_slot: 0,
            verdict_slot: 0,
            upload_cursor: 0,
            verification_progress: 0,
            verification_units,
            rank_key: RankKey::ZERO,
            escrow: Escrow {
                rent_principal,
                work_reward_balance,
                bond_balance: self.funding_policy.invalidity_bond,
                cleanup_balance: self.funding_policy.cleanup_reward,
                ..Escrow::ZERO
            },
        };
        Ok(())
    }

    /// Append exactly the next stage chunk. The inert stage may finish bytes
    /// after submission closes, but can never be sealed then.
    pub fn write_stage(&mut self, id: u64, cursor: u16) -> Result<(), Error> {
        let candidate = self.candidate_mut(id)?;
        if candidate.state != CandidateState::Staging {
            return Err(Error::MismatchedState);
        }
        if candidate.upload_cursor != cursor {
            return Err(Error::Replay);
        }
        candidate.upload_cursor = candidate
            .upload_cursor
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        Ok(())
    }

    /// Atomically make a complete stage part of the immutable submitted set.
    pub fn seal_candidate(&mut self, slot: u64, id: u64) -> Result<(), Error> {
        self.require_epoch_frozen()?;
        if self.schedule.interval(slot)? != Interval::Submission {
            return Err(Error::NotActive);
        }
        if self.sealed_candidate_count >= self.capacity {
            return Err(Error::Capacity);
        }
        let candidate = self.candidate_mut(id)?;
        if candidate.state != CandidateState::Staging || candidate.upload_cursor == 0 {
            return Err(Error::MismatchedState);
        }
        candidate.state = CandidateState::Sealed;
        candidate.sealed_slot = slot;
        self.sealed_candidate_count = self
            .sealed_candidate_count
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        Ok(())
    }

    /// Advance exactly one new verification unit and pay exactly one reward.
    pub fn advance_verification(&mut self, slot: u64, id: u64) -> Result<Payout, Error> {
        self.require_epoch_frozen()?;
        if self.schedule.interval(slot)? != Interval::Verification {
            return Err(Error::NotActive);
        }
        let progress_reward = self.funding_policy.progress_reward;
        let candidate = self.candidate_mut(id)?;
        if candidate.state != CandidateState::Sealed
            || candidate.verification_progress >= candidate.verification_units
        {
            return Err(Error::Replay);
        }
        if candidate.escrow.work_reward_balance < progress_reward {
            return Err(Error::Underfunded);
        }
        candidate.verification_progress += 1;
        candidate.escrow.work_reward_balance -= progress_reward;
        candidate.escrow.keeper_paid = candidate
            .escrow
            .keeper_paid
            .checked_add(progress_reward)
            .ok_or(Error::Arithmetic)?;
        Ok(Payout {
            keeper_reward: progress_reward,
            ..Payout::default()
        })
    }

    /// Persist one checked terminal relation verdict and update the generic
    /// verified-only top registry.
    pub fn complete_verification(
        &mut self,
        slot: u64,
        id: u64,
        verdict: RelationVerdict,
    ) -> Result<Payout, Error> {
        self.require_epoch_frozen()?;
        if self.schedule.interval(slot)? != Interval::Verification {
            return Err(Error::NotActive);
        }
        let index = self.find(id)?;
        let candidate = self.candidates[index];
        if candidate.state != CandidateState::Sealed
            || candidate.verification_progress != candidate.verification_units
        {
            return Err(Error::MismatchedState);
        }

        let mut payout = Payout::default();
        match verdict {
            RelationVerdict::Valid {
                score_policy_id,
                rank_key,
            } => {
                if score_policy_id != self.score_policy_id {
                    return Err(Error::ScorePolicyMismatch);
                }
                if rank_key == RankKey::ZERO {
                    return Err(Error::MismatchedState);
                }
                self.ensure_rank_unique(rank_key)?;
                self.candidates[index].state = CandidateState::VerifiedValid;
                self.candidates[index].rank_key = rank_key;
                self.insert_top(id)?;
            }
            RelationVerdict::Refused => {
                self.candidates[index].state = CandidateState::VerifiedRefused;
                let slash = self.candidates[index].escrow.bond_balance;
                self.candidates[index].escrow.bond_balance = 0;
                self.candidates[index].escrow.slashed = slash;
                payout.slashed = slash;
            }
        }
        self.candidates[index].verdict_slot = slot;
        self.verdict_count = self.verdict_count.checked_add(1).ok_or(Error::Arithmetic)?;
        Ok(payout)
    }

    /// Expire an unsealed stage at `S`, or an unresolved sealed candidate at
    /// `V`. Expiry never claims that a sealed candidate was invalid.
    pub fn expire_candidate(&mut self, slot: u64, id: u64) -> Result<Payout, Error> {
        let schedule = self.schedule;
        let funding = self.funding_policy;
        let candidate = self.candidate_mut(id)?;
        let slash = match candidate.state {
            CandidateState::Staging if slot >= schedule.submission_closes_slot => {
                let penalty =
                    core::cmp::min(candidate.escrow.bond_balance, funding.abandonment_penalty);
                candidate.escrow.bond_balance -= penalty;
                candidate.escrow.slashed = candidate
                    .escrow
                    .slashed
                    .checked_add(penalty)
                    .ok_or(Error::Arithmetic)?;
                candidate.state = CandidateState::ExpiredStaging;
                penalty
            }
            CandidateState::Sealed if slot >= schedule.verification_closes_slot => {
                candidate.state = CandidateState::ExpiredUnverified;
                0
            }
            _ => return Err(Error::NotActive),
        };
        let reward = core::cmp::min(candidate.escrow.cleanup_balance, funding.cleanup_reward);
        candidate.escrow.cleanup_balance -= reward;
        candidate.escrow.keeper_paid = candidate
            .escrow
            .keeper_paid
            .checked_add(reward)
            .ok_or(Error::Arithmetic)?;
        Ok(Payout {
            keeper_reward: reward,
            slashed: slash,
            ..Payout::default()
        })
    }

    /// Finalize after every sealed candidate has a verdict, or at the hard
    /// verification boundary. Selection uses only the verified top registry.
    pub fn finalize(&mut self, slot: u64) -> Result<Payout, Error> {
        self.require_epoch_frozen()?;
        let funding = self.funding_policy;
        let interval = self.schedule.interval(slot)?;
        if interval == Interval::Submission {
            return Err(Error::NotActive);
        }
        if interval == Interval::Verification && self.verdict_count != self.sealed_candidate_count {
            return Err(Error::UnresolvedCandidates);
        }
        let required = funding
            .finalizer_reward
            .checked_add(if self.top_count == 0 {
                0
            } else {
                funding.solver_prize
            })
            .ok_or(Error::Arithmetic)?;
        if self.epoch_reward_balance < required {
            return Err(Error::Underfunded);
        }
        self.epoch_reward_balance -= funding.finalizer_reward;
        self.finalizer_paid = self
            .finalizer_paid
            .checked_add(funding.finalizer_reward)
            .ok_or(Error::Arithmetic)?;
        let mut payout = Payout {
            keeper_reward: funding.finalizer_reward,
            ..Payout::default()
        };
        if self.top_count == 0 {
            self.epoch_state = EpochState::Lapsed;
        } else {
            let winner = self.top[0];
            let index = self.find(winner)?;
            if !self.candidates[index].is_valid() {
                return Err(Error::MismatchedState);
            }
            self.epoch_reward_balance -= funding.solver_prize;
            self.candidates[index].escrow.solver_credit = self.candidates[index]
                .escrow
                .solver_credit
                .checked_add(funding.solver_prize)
                .ok_or(Error::Arithmetic)?;
            self.candidates[index].state = CandidateState::Selected;
            self.selected_candidate = winner;
            self.epoch_state = EpochState::Cleared;
            payout.solver_credit = funding.solver_prize;
        }
        self.finalized_slot = slot;
        Ok(payout)
    }

    /// Claim refundable non-rent balances once. Rent becomes refundable only
    /// through the account-family close transition and is not modeled here.
    pub fn claim_refund(&mut self, id: u64) -> Result<Payout, Error> {
        let candidate = self.candidate_mut(id)?;
        if candidate.escrow.balances_refund_claimed
            || !matches!(
                candidate.state,
                CandidateState::VerifiedValid
                    | CandidateState::VerifiedRefused
                    | CandidateState::Selected
                    | CandidateState::ExpiredStaging
                    | CandidateState::ExpiredUnverified
            )
        {
            return Err(Error::NotActive);
        }
        let refundable = candidate
            .escrow
            .work_reward_balance
            .checked_add(candidate.escrow.bond_balance)
            .and_then(|value| value.checked_add(candidate.escrow.cleanup_balance))
            .ok_or(Error::Arithmetic)?;
        candidate.escrow.work_reward_balance = 0;
        candidate.escrow.bond_balance = 0;
        candidate.escrow.cleanup_balance = 0;
        candidate.escrow.balances_refund_claimed = true;
        Ok(Payout {
            refundable,
            ..Payout::default()
        })
    }

    /// Claim a separately credited winner prize once. A solver may therefore
    /// recover its validity bond before selection without losing a later
    /// epoch-funded prize.
    pub fn claim_solver_credit(&mut self, id: u64) -> Result<Payout, Error> {
        let candidate = self.candidate_mut(id)?;
        if candidate.state != CandidateState::Selected
            || candidate.escrow.solver_credit_claimed
            || candidate.escrow.solver_credit == 0
        {
            return Err(Error::NotActive);
        }
        let refundable = candidate.escrow.solver_credit;
        candidate.escrow.solver_credit = 0;
        candidate.escrow.solver_credit_claimed = true;
        Ok(Payout {
            refundable,
            ..Payout::default()
        })
    }

    fn require_epoch_frozen(&self) -> Result<(), Error> {
        if self.epoch_state != EpochState::Frozen {
            return Err(Error::Replay);
        }
        Ok(())
    }

    fn find(&self, id: u64) -> Result<usize, Error> {
        self.candidates
            .iter()
            .position(|candidate| candidate.id == id && candidate.state != CandidateState::Empty)
            .ok_or(Error::NotFound)
    }

    fn candidate_mut(&mut self, id: u64) -> Result<&mut Candidate, Error> {
        let index = self.find(id)?;
        Ok(&mut self.candidates[index])
    }

    fn ensure_rank_unique(&self, rank_key: RankKey) -> Result<(), Error> {
        if self
            .candidates
            .iter()
            .any(|candidate| candidate.is_valid() && candidate.rank_key == rank_key)
        {
            return Err(Error::RankCollision);
        }
        Ok(())
    }

    fn insert_top(&mut self, id: u64) -> Result<(), Error> {
        let rank = self.candidates[self.find(id)?].rank_key;
        let count = usize::from(self.top_count);
        let mut insertion = count;
        let mut index = 0usize;
        while index < count {
            let current = self.candidates[self.find(self.top[index])?].rank_key;
            if rank > current {
                insertion = index;
                break;
            }
            index += 1;
        }
        if insertion >= TOP_CAPACITY {
            return Ok(());
        }
        let new_count = core::cmp::min(count + 1, TOP_CAPACITY);
        let mut cursor = new_count;
        while cursor > insertion + 1 {
            self.top[cursor - 1] = self.top[cursor - 2];
            cursor -= 1;
        }
        self.top[insertion] = id;
        self.top_count = new_count as u8;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCORE: ScorePolicyId = ScorePolicyId([7; 32]);
    const OTHER_SCORE: ScorePolicyId = ScorePolicyId([8; 32]);
    const FUNDING: FundingPolicy = FundingPolicy {
        progress_reward: 3,
        cleanup_reward: 5,
        invalidity_bond: 17,
        abandonment_penalty: 7,
        solver_prize: 19,
        finalizer_reward: 11,
    };

    fn rank(value: u8, tie: u64) -> RankKey {
        let mut bytes = [0; RANK_KEY_BYTES];
        bytes[0] = value;
        bytes[8..].copy_from_slice(&tie.to_be_bytes());
        RankKey(bytes)
    }

    fn window() -> Window {
        Window::new(
            Schedule::new(100, 10, 20).unwrap(),
            SCORE,
            8,
            FUNDING,
            FUNDING.solver_prize + FUNDING.finalizer_reward,
        )
        .unwrap()
    }

    fn begin_and_seal(window: &mut Window, id: u64, units: u16) {
        window.begin_candidate(101, id, units, 1_000).unwrap();
        window.write_stage(id, 0).unwrap();
        window.seal_candidate(109, id).unwrap();
    }

    fn verify_valid(window: &mut Window, id: u64, key: RankKey) {
        let units = window.candidates[window.find(id).unwrap()].verification_units;
        let mut unit = 0;
        while unit < units {
            window.advance_verification(110, id).unwrap();
            unit += 1;
        }
        window
            .complete_verification(
                111,
                id,
                RelationVerdict::Valid {
                    score_policy_id: SCORE,
                    rank_key: key,
                },
            )
            .unwrap();
    }

    #[test]
    fn interval_boundaries_are_half_open_and_checked() {
        let schedule = Schedule::new(100, 10, 20).unwrap();
        assert_eq!(schedule.interval(99), Err(Error::NotActive));
        assert_eq!(schedule.interval(100), Ok(Interval::Submission));
        assert_eq!(schedule.interval(109), Ok(Interval::Submission));
        assert_eq!(schedule.interval(110), Ok(Interval::Verification));
        assert_eq!(schedule.interval(129), Ok(Interval::Verification));
        assert_eq!(schedule.interval(130), Ok(Interval::Terminal));
        assert_eq!(Schedule::new(100, 0, 1), Err(Error::InvalidSchedule));
        assert_eq!(Schedule::new(u64::MAX, 1, 1), Err(Error::Arithmetic));
    }

    #[test]
    fn seal_at_submission_boundary_refuses_but_late_stage_write_is_inert() {
        let mut window = window();
        window.begin_candidate(109, 1, 1, 1_000).unwrap();
        window.write_stage(1, 0).unwrap();
        assert_eq!(window.seal_candidate(110, 1), Err(Error::NotActive));
        assert_eq!(window.write_stage(1, 1), Ok(()));
        assert_eq!(window.sealed_candidate_count, 0);
        let payout = window.expire_candidate(110, 1).unwrap();
        assert_eq!(payout.keeper_reward, FUNDING.cleanup_reward);
        assert_eq!(payout.slashed, FUNDING.abandonment_penalty);
        assert_eq!(window.candidates[0].state, CandidateState::ExpiredStaging);
    }

    #[test]
    fn verification_boundary_is_exclusive_and_unverified_expiry_does_not_slash() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 2);
        assert_eq!(window.advance_verification(109, 1), Err(Error::NotActive));
        assert_eq!(
            window.advance_verification(110, 1).unwrap().keeper_reward,
            FUNDING.progress_reward
        );
        assert_eq!(window.advance_verification(130, 1), Err(Error::NotActive));
        let payout = window.expire_candidate(130, 1).unwrap();
        assert_eq!(payout.slashed, 0);
        assert_eq!(window.candidates[0].escrow.bond_balance, 17);
        assert_eq!(
            window.candidates[0].state,
            CandidateState::ExpiredUnverified
        );
    }

    #[test]
    fn progress_rewards_are_prepaid_monotone_and_replay_safe() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        let before = window.candidates[0].escrow.work_reward_balance;
        let payout = window.advance_verification(110, 1).unwrap();
        assert_eq!(payout.keeper_reward, 3);
        assert_eq!(window.candidates[0].escrow.work_reward_balance, before - 3);
        assert_eq!(window.advance_verification(111, 1), Err(Error::Replay));
        assert_eq!(window.candidates[0].escrow.keeper_paid, 3);
    }

    #[test]
    fn score_policy_is_independent_but_cannot_mix_inside_an_epoch() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        window.advance_verification(110, 1).unwrap();
        assert_eq!(
            window.complete_verification(
                111,
                1,
                RelationVerdict::Valid {
                    score_policy_id: OTHER_SCORE,
                    rank_key: rank(9, 1),
                }
            ),
            Err(Error::ScorePolicyMismatch)
        );
        assert_eq!(window.candidates[0].state, CandidateState::Sealed);
        window
            .complete_verification(
                111,
                1,
                RelationVerdict::Valid {
                    score_policy_id: SCORE,
                    rank_key: rank(9, 1),
                },
            )
            .unwrap();
        assert_eq!(window.candidates[0].state, CandidateState::VerifiedValid);
    }

    #[test]
    fn top_registry_is_bounded_without_erasing_valid_losers() {
        let mut window = window();
        for id in 1..=5 {
            begin_and_seal(&mut window, id, 1);
        }
        for id in 1..=5 {
            verify_valid(&mut window, id, rank(id as u8, id));
        }
        assert_eq!(window.top, [5, 4, 3]);
        assert_eq!(window.top_count, 3);
        assert_eq!(window.verdict_count, 5);
        assert_eq!(window.candidates[0].state, CandidateState::VerifiedValid);
        assert_eq!(window.candidates[1].state, CandidateState::VerifiedValid);
    }

    #[test]
    fn early_finalize_requires_every_sealed_verdict_and_is_one_shot() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        begin_and_seal(&mut window, 2, 1);
        verify_valid(&mut window, 1, rank(2, 1));
        assert_eq!(window.finalize(112), Err(Error::UnresolvedCandidates));
        verify_valid(&mut window, 2, rank(3, 2));
        let payout = window.finalize(112).unwrap();
        assert_eq!(window.selected_candidate, 2);
        assert_eq!(window.epoch_state, EpochState::Cleared);
        assert_eq!(payout.keeper_reward, FUNDING.finalizer_reward);
        assert_eq!(payout.solver_credit, FUNDING.solver_prize);
        assert_eq!(window.finalize(113), Err(Error::Replay));
    }

    #[test]
    fn hard_deadline_finalizes_without_withheld_unverified_candidate() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        begin_and_seal(&mut window, 2, 2);
        verify_valid(&mut window, 1, rank(2, 1));
        window.advance_verification(129, 2).unwrap();
        window.finalize(130).unwrap();
        assert_eq!(window.selected_candidate, 1);
        assert_eq!(window.candidates[1].state, CandidateState::Sealed);
        assert_eq!(window.advance_verification(130, 2), Err(Error::Replay));
        let expiry = window.expire_candidate(130, 2).unwrap();
        assert_eq!(expiry.slashed, 0);
    }

    #[test]
    fn checked_refusal_slashes_bond_but_not_rent_or_unearned_reserve() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        window.advance_verification(110, 1).unwrap();
        let payout = window
            .complete_verification(111, 1, RelationVerdict::Refused)
            .unwrap();
        assert_eq!(payout.slashed, FUNDING.invalidity_bond);
        assert_eq!(window.candidates[0].escrow.rent_principal, 1_000);
        let refund = window.claim_refund(1).unwrap();
        assert_eq!(refund.refundable, FUNDING.cleanup_reward);
        assert_eq!(window.claim_refund(1), Err(Error::NotActive));
    }

    #[test]
    fn valid_bond_refund_before_selection_does_not_consume_later_solver_prize() {
        let mut window = window();
        begin_and_seal(&mut window, 1, 1);
        verify_valid(&mut window, 1, rank(2, 1));
        let bond_refund = window.claim_refund(1).unwrap();
        assert_eq!(bond_refund.refundable, FUNDING.invalidity_bond + 5);
        window.finalize(112).unwrap();
        let prize = window.claim_solver_credit(1).unwrap();
        assert_eq!(prize.refundable, FUNDING.solver_prize);
        assert_eq!(window.claim_solver_credit(1), Err(Error::NotActive));
    }

    #[test]
    fn empty_window_lapses_at_submission_close_without_waiting_for_v() {
        let mut window = window();
        let payout = window.finalize(110).unwrap();
        assert_eq!(window.epoch_state, EpochState::Lapsed);
        assert_eq!(payout.keeper_reward, FUNDING.finalizer_reward);
        assert_eq!(payout.solver_credit, 0);
    }
}
