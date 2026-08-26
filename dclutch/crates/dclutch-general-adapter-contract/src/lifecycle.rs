//! Exact funded lifecycle surrounding streamed General clearing.
//!
//! Candidate verification and settlement facts remain owned by the canonical
//! controller cursors in the parent module. This module owns only child
//! existence, terminal absence, and the native lamports that make every
//! permissionless continuation and cleanup independently payable.

/// Persistent batch lifecycle surrounding one selection cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BatchPhaseV2 {
    /// Candidate accounts may be admitted.
    Open = 1,
    /// Admission is closed and the best valid submitted candidate is selected.
    Selecting = 2,
    /// The selected candidate may be physically settled.
    Settling = 3,
    /// Settlement finished and only child cleanup remains.
    Terminal = 4,
    /// No candidate was selected before the immutable deadline.
    Aborted = 5,
}

/// Persistent candidate lifecycle surrounding verification and settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidatePhaseV2 {
    /// Immutable pages are still being published.
    Publishing = 1,
    /// Every declared page exists and verification may advance.
    Verifying = 2,
    /// The candidate is valid and may participate in selection.
    Verified = 3,
    /// This candidate is the frozen best valid submitted candidate.
    Selected = 4,
    /// The two-pass physical settlement is in progress.
    Applying = 5,
    /// Settlement has reached its zero-inventory terminal state.
    Terminal = 6,
    /// The candidate lost, expired, or was refused and may only be cleaned up.
    Aborted = 7,
}

/// Stable refusal from funded General child lifecycle transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    /// An immutable identity, count, rent value, or work reward was zero.
    InvalidInput,
    /// A caller supplied a stale optimistic revision or page coordinate.
    CoordinateMismatch,
    /// The requested transition was not admitted by the current phase.
    InvalidPhase,
    /// A close was attempted while a persisted child still exists.
    LiveChildren,
    /// Observed lamports did not exactly equal Rent plus segregated reserves.
    PhysicalBalance,
    /// The selected compartment could not fund its one exact operation.
    InsufficientCompartment,
    /// Checked integer arithmetic overflowed or underflowed.
    Arithmetic,
}

/// Result alias for General lifecycle operations.
pub type LifecycleResult<T> = core::result::Result<T, LifecycleError>;

/// Exact native-lamport compartments owned by one candidate account.
///
/// These balances are deliberately not interchangeable. In particular,
/// unused settlement capital cannot make an underfunded cleanup admissible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFundingV2 {
    state_rent: u64,
    page_rent: u64,
    verification_work: u64,
    settlement_work: u64,
    cleanup_work: u64,
}

impl CandidateFundingV2 {
    /// Construct exact segregated candidate funding.
    pub fn new(
        state_rent: u64,
        page_rent: u64,
        verification_work: u64,
        settlement_work: u64,
        cleanup_work: u64,
    ) -> LifecycleResult<Self> {
        if state_rent == 0 || cleanup_work == 0 {
            return Err(LifecycleError::InvalidInput);
        }
        let value = Self {
            state_rent,
            page_rent,
            verification_work,
            settlement_work,
            cleanup_work,
        };
        value.exact_lamports()?;
        Ok(value)
    }

    /// Exact physical lamports which must be present on the candidate account.
    pub fn exact_lamports(self) -> LifecycleResult<u64> {
        self.state_rent
            .checked_add(self.page_rent)
            .and_then(|value| value.checked_add(self.verification_work))
            .and_then(|value| value.checked_add(self.settlement_work))
            .and_then(|value| value.checked_add(self.cleanup_work))
            .ok_or(LifecycleError::Arithmetic)
    }

    /// Require byte-state compartments to match the physical native balance.
    pub fn require_physical(self, observed_lamports: u64) -> LifecycleResult<()> {
        if self.exact_lamports()? == observed_lamports {
            Ok(())
        } else {
            Err(LifecycleError::PhysicalBalance)
        }
    }

    /// Reserve exact Rent for one immutable page creation.
    pub fn create_page(
        &mut self,
        observed_lamports: u64,
        exact_page_rent: u64,
    ) -> LifecycleResult<FundingMoveV2> {
        self.require_physical(observed_lamports)?;
        if exact_page_rent == 0 || self.page_rent < exact_page_rent {
            return Err(LifecycleError::InsufficientCompartment);
        }
        self.page_rent = self
            .page_rent
            .checked_sub(exact_page_rent)
            .ok_or(LifecycleError::Arithmetic)?;
        Ok(FundingMoveV2 {
            account_top_up: exact_page_rent,
            actor_reward: 0,
            rent_credit: 0,
            candidate_lamports_after: self.exact_lamports()?,
        })
    }

    /// Pay one permissionless verification continuation.
    pub fn pay_verification(
        &mut self,
        observed_lamports: u64,
        reward: u64,
    ) -> LifecycleResult<FundingMoveV2> {
        self.pay_work(observed_lamports, reward, FundingKind::Verification)
    }

    /// Pay one permissionless selected-settlement continuation.
    pub fn pay_settlement(
        &mut self,
        observed_lamports: u64,
        reward: u64,
    ) -> LifecycleResult<FundingMoveV2> {
        self.pay_work(observed_lamports, reward, FundingKind::Settlement)
    }

    /// Pay one permissionless page or candidate cleanup continuation.
    pub fn pay_cleanup(
        &mut self,
        observed_lamports: u64,
        reward: u64,
    ) -> LifecycleResult<FundingMoveV2> {
        self.pay_work(observed_lamports, reward, FundingKind::Cleanup)
    }

    /// Close the candidate after its final cleanup reward has been consumed.
    ///
    /// Every unused segregated reserve and the account's own Rent go only to
    /// the immutable RentCredit. Nothing is reclassified as a fee or bounty.
    pub fn close(
        &mut self,
        observed_lamports: u64,
        final_cleanup_reward: u64,
    ) -> LifecycleResult<FundingMoveV2> {
        let reward = self.pay_cleanup(observed_lamports, final_cleanup_reward)?;
        let rent_credit = reward.candidate_lamports_after;
        *self = Self {
            state_rent: 0,
            page_rent: 0,
            verification_work: 0,
            settlement_work: 0,
            cleanup_work: 0,
        };
        Ok(FundingMoveV2 {
            account_top_up: 0,
            actor_reward: final_cleanup_reward,
            rent_credit,
            candidate_lamports_after: 0,
        })
    }

    fn pay_work(
        &mut self,
        observed_lamports: u64,
        reward: u64,
        kind: FundingKind,
    ) -> LifecycleResult<FundingMoveV2> {
        self.require_physical(observed_lamports)?;
        if reward == 0 {
            return Err(LifecycleError::InvalidInput);
        }
        let compartment = match kind {
            FundingKind::Verification => &mut self.verification_work,
            FundingKind::Settlement => &mut self.settlement_work,
            FundingKind::Cleanup => &mut self.cleanup_work,
        };
        *compartment = compartment
            .checked_sub(reward)
            .ok_or(LifecycleError::InsufficientCompartment)?;
        Ok(FundingMoveV2 {
            account_top_up: 0,
            actor_reward: reward,
            rent_credit: 0,
            candidate_lamports_after: self.exact_lamports()?,
        })
    }

    /// Remaining page-Rent reserve.
    #[must_use]
    pub const fn page_rent(self) -> u64 {
        self.page_rent
    }
    /// Remaining verification-work reserve.
    #[must_use]
    pub const fn verification_work(self) -> u64 {
        self.verification_work
    }
    /// Remaining selected-settlement-work reserve.
    #[must_use]
    pub const fn settlement_work(self) -> u64 {
        self.settlement_work
    }
    /// Remaining cleanup-work reserve.
    #[must_use]
    pub const fn cleanup_work(self) -> u64 {
        self.cleanup_work
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FundingKind {
    Verification,
    Settlement,
    Cleanup,
}

/// Exact lamport effects returned by one funded transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingMoveV2 {
    /// Rent transferred into a newly created child account.
    pub account_top_up: u64,
    /// Permissionless continuation reward paid to the current actor.
    pub actor_reward: u64,
    /// Rent or unused reserves routed to immutable RentCredit.
    pub rent_credit: u64,
    /// Exact post-transition lamports retained by the candidate account.
    pub candidate_lamports_after: u64,
}

/// Exact child-count and phase owner for one candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateLifecycleV2 {
    phase: CandidatePhaseV2,
    candidate_id: [u8; 32],
    page_count: u32,
    created_pages: u32,
    live_pages: u32,
    revision: u64,
}

impl CandidateLifecycleV2 {
    /// Begin publication for a runtime page count.
    pub fn publishing(candidate_id: [u8; 32], page_count: u32) -> LifecycleResult<Self> {
        if is_zero(&candidate_id) || page_count == 0 {
            return Err(LifecycleError::InvalidInput);
        }
        Ok(Self {
            phase: CandidatePhaseV2::Publishing,
            candidate_id,
            page_count,
            created_pages: 0,
            live_pages: 0,
            revision: 0,
        })
    }

    /// Record exact in-order immutable page creation.
    pub fn create_page(&mut self, expected_revision: u64, page_index: u32) -> LifecycleResult<()> {
        if self.phase != CandidatePhaseV2::Publishing {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        if page_index != self.created_pages || page_index >= self.page_count {
            return Err(LifecycleError::CoordinateMismatch);
        }
        self.created_pages = self
            .created_pages
            .checked_add(1)
            .ok_or(LifecycleError::Arithmetic)?;
        self.live_pages = self
            .live_pages
            .checked_add(1)
            .ok_or(LifecycleError::Arithmetic)?;
        self.advance_revision()?;
        if self.created_pages == self.page_count {
            self.phase = CandidatePhaseV2::Verifying;
        }
        Ok(())
    }

    /// Mark completion of the canonical verification cursor.
    pub fn verified(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if self.phase != CandidatePhaseV2::Verifying || self.created_pages != self.page_count {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.phase = CandidatePhaseV2::Verified;
        self.advance_revision()
    }

    /// Mark this candidate as the frozen best valid submitted candidate.
    pub fn select(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if self.phase != CandidatePhaseV2::Verified {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.phase = CandidatePhaseV2::Selected;
        self.advance_revision()
    }

    /// Enter two-pass physical settlement.
    pub fn begin_apply(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if self.phase != CandidatePhaseV2::Selected || self.live_pages != self.page_count {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.phase = CandidatePhaseV2::Applying;
        self.advance_revision()
    }

    /// Record the controller cursor's zero-inventory terminal state.
    pub fn finish_apply(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if self.phase != CandidatePhaseV2::Applying {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.phase = CandidatePhaseV2::Terminal;
        self.advance_revision()
    }

    /// Abort any nonapplying candidate after external deadline authentication.
    pub fn abort(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if matches!(
            self.phase,
            CandidatePhaseV2::Applying | CandidatePhaseV2::Terminal | CandidatePhaseV2::Aborted
        ) {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.phase = CandidatePhaseV2::Aborted;
        self.advance_revision()
    }

    /// Close one immutable page after it is no longer needed.
    pub fn close_page(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if !matches!(
            self.phase,
            CandidatePhaseV2::Terminal | CandidatePhaseV2::Aborted
        ) {
            return Err(LifecycleError::InvalidPhase);
        }
        self.require_revision(expected_revision)?;
        self.live_pages = self
            .live_pages
            .checked_sub(1)
            .ok_or(LifecycleError::LiveChildren)?;
        self.advance_revision()
    }

    /// Prove terminal absence before closing the candidate account.
    pub fn close(self, expected_revision: u64) -> LifecycleResult<()> {
        self.require_revision(expected_revision)?;
        if !matches!(
            self.phase,
            CandidatePhaseV2::Terminal | CandidatePhaseV2::Aborted
        ) {
            return Err(LifecycleError::InvalidPhase);
        }
        if self.live_pages != 0 {
            return Err(LifecycleError::LiveChildren);
        }
        Ok(())
    }

    fn require_revision(self, expected: u64) -> LifecycleResult<()> {
        if self.revision == expected {
            Ok(())
        } else {
            Err(LifecycleError::CoordinateMismatch)
        }
    }

    fn advance_revision(&mut self) -> LifecycleResult<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(LifecycleError::Arithmetic)?;
        Ok(())
    }

    /// Current candidate phase.
    #[must_use]
    pub const fn phase(self) -> CandidatePhaseV2 {
        self.phase
    }
    /// Runtime number of immutable candidate pages.
    #[must_use]
    pub const fn page_count(self) -> u32 {
        self.page_count
    }
    /// Exact number of still-live page children.
    #[must_use]
    pub const fn live_pages(self) -> u32 {
        self.live_pages
    }
    /// Exact optimistic revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Immutable candidate identity.
    #[must_use]
    pub const fn candidate_id(self) -> [u8; 32] {
        self.candidate_id
    }
}

/// Exact child-count and phase owner for one General batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchLifecycleV2 {
    phase: BatchPhaseV2,
    batch_id: [u8; 32],
    open_candidates: u64,
    revision: u64,
}

impl BatchLifecycleV2 {
    /// Construct one open batch.
    pub fn open(batch_id: [u8; 32]) -> LifecycleResult<Self> {
        if is_zero(&batch_id) {
            return Err(LifecycleError::InvalidInput);
        }
        Ok(Self {
            phase: BatchPhaseV2::Open,
            batch_id,
            open_candidates: 0,
            revision: 0,
        })
    }

    /// Record one newly admitted candidate child.
    pub fn admit_candidate(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        self.require(BatchPhaseV2::Open, expected_revision)?;
        self.open_candidates = self
            .open_candidates
            .checked_add(1)
            .ok_or(LifecycleError::Arithmetic)?;
        self.advance_revision()
    }

    /// Close candidate admission.
    pub fn lock(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        self.require(BatchPhaseV2::Open, expected_revision)?;
        self.phase = BatchPhaseV2::Selecting;
        self.advance_revision()
    }

    /// Begin settlement after the external selection cursor is frozen nonempty.
    pub fn begin_settlement(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        self.require(BatchPhaseV2::Selecting, expected_revision)?;
        self.phase = BatchPhaseV2::Settling;
        self.advance_revision()
    }

    /// Finish the selected candidate's physical settlement.
    pub fn finish_settlement(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        self.require(BatchPhaseV2::Settling, expected_revision)?;
        self.phase = BatchPhaseV2::Terminal;
        self.advance_revision()
    }

    /// Abort an empty selection after external deadline authentication.
    pub fn abort_empty(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        self.require(BatchPhaseV2::Selecting, expected_revision)?;
        self.phase = BatchPhaseV2::Aborted;
        self.advance_revision()
    }

    /// Record exact closure of one candidate child.
    pub fn close_candidate(&mut self, expected_revision: u64) -> LifecycleResult<()> {
        if !matches!(self.phase, BatchPhaseV2::Terminal | BatchPhaseV2::Aborted) {
            return Err(LifecycleError::InvalidPhase);
        }
        if self.revision != expected_revision {
            return Err(LifecycleError::CoordinateMismatch);
        }
        self.open_candidates = self
            .open_candidates
            .checked_sub(1)
            .ok_or(LifecycleError::LiveChildren)?;
        self.advance_revision()
    }

    /// Prove terminal absence before closing the batch account.
    pub fn close(self, expected_revision: u64) -> LifecycleResult<()> {
        if self.revision != expected_revision {
            return Err(LifecycleError::CoordinateMismatch);
        }
        if !matches!(self.phase, BatchPhaseV2::Terminal | BatchPhaseV2::Aborted) {
            return Err(LifecycleError::InvalidPhase);
        }
        if self.open_candidates != 0 {
            return Err(LifecycleError::LiveChildren);
        }
        Ok(())
    }

    fn require(&self, phase: BatchPhaseV2, expected_revision: u64) -> LifecycleResult<()> {
        if self.phase != phase {
            return Err(LifecycleError::InvalidPhase);
        }
        if self.revision != expected_revision {
            return Err(LifecycleError::CoordinateMismatch);
        }
        Ok(())
    }

    fn advance_revision(&mut self) -> LifecycleResult<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(LifecycleError::Arithmetic)?;
        Ok(())
    }

    /// Current batch phase.
    #[must_use]
    pub const fn phase(self) -> BatchPhaseV2 {
        self.phase
    }
    /// Exact number of still-live candidate children.
    #[must_use]
    pub const fn open_candidates(self) -> u64 {
        self.open_candidates
    }
    /// Exact optimistic revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Immutable batch identity.
    #[must_use]
    pub const fn batch_id(self) -> [u8; 32] {
        self.batch_id
    }
}

const fn is_zero(value: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        let mut value = [byte; 32];
        value[31] = byte.wrapping_add(1);
        value
    }

    #[test]
    fn runtime_page_lifecycle_has_no_page_balance_restriction() {
        let mut candidate = CandidateLifecycleV2::publishing(id(7), 300).expect("candidate");
        for page in 0..300 {
            let revision = candidate.revision();
            candidate.create_page(revision, page).expect("page");
        }
        assert_eq!(candidate.phase(), CandidatePhaseV2::Verifying);
        assert_eq!(candidate.live_pages(), 300);
        candidate.verified(candidate.revision()).expect("verified");
        candidate.select(candidate.revision()).expect("selected");
        candidate
            .begin_apply(candidate.revision())
            .expect("applying");
        candidate
            .finish_apply(candidate.revision())
            .expect("terminal");
        for _ in 0..300 {
            candidate
                .close_page(candidate.revision())
                .expect("close page");
        }
        candidate.close(candidate.revision()).expect("close");
    }

    #[test]
    fn compartments_never_borrow_and_close_refunds_only_rent_credit() {
        let mut funding = CandidateFundingV2::new(10, 20, 30, 40, 50).expect("funding");
        let before = funding.exact_lamports().expect("total");
        assert_eq!(before, 150);
        let page = funding.create_page(before, 20).expect("page");
        assert_eq!(page.account_top_up, 20);
        assert_eq!(page.candidate_lamports_after, 130);
        assert_eq!(
            funding.pay_verification(130, 31),
            Err(LifecycleError::InsufficientCompartment)
        );
        let verify = funding.pay_verification(130, 30).expect("verify");
        assert_eq!(verify.actor_reward, 30);
        let settle = funding.pay_settlement(100, 40).expect("settle");
        assert_eq!(settle.actor_reward, 40);
        let closed = funding.close(60, 10).expect("close");
        assert_eq!(closed.actor_reward, 10);
        assert_eq!(closed.rent_credit, 50);
        assert_eq!(closed.candidate_lamports_after, 0);
    }

    #[test]
    fn stale_and_live_child_closes_refuse_without_mutation() {
        let mut batch = BatchLifecycleV2::open(id(9)).expect("batch");
        batch.admit_candidate(0).expect("admit");
        assert_eq!(batch.lock(0), Err(LifecycleError::CoordinateMismatch));
        batch.lock(1).expect("lock");
        batch.abort_empty(2).expect("abort");
        assert_eq!(batch.close(3), Err(LifecycleError::LiveChildren));
        batch.close_candidate(3).expect("child close");
        batch.close(4).expect("batch close");
    }
}
