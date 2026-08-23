// SPDX-License-Identifier: AGPL-3.0-or-later

//! Individually funded, copy-resistant candidate-admission successor.
//!
//! This module deliberately does not reinterpret any V2 wire byte or allocate
//! a Solana account tag. It is a fixed-memory kernel seam for a future account
//! family. The adapter must authenticate the commitment hash, identities,
//! fresh canonical node account, funding, and every external candidate bundle.

use core::cmp::Ordering;

use crate::{add, live, Error, Id, RankKey, ScorePolicyBindingV1, RANK_KEY_CAPACITY};

/// Hash-domain bytes the adapter must prefix to every V3 commitment preimage.
pub const CANDIDATE_COMMITMENT_DOMAIN_V1: &[u8] = b"dragons-clutch/candidate-commitment/v1";

/// Security facts supplied by the future account adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionAdapterObligationV3 {
    AuthenticateClock,
    AuthenticatePolicyIdentity,
    DeriveFreshCanonicalNode,
    VerifyDomainSeparatedCommitmentOpening,
    DeriveCandidateGeometryAndFunding,
    AuthenticateCandidateVerdict,
    AuthenticateCandidateBundleClosure,
    AuthenticateSelectedSettlementTerminal,
    MoveLamportsAndCloseAtomically,
    CountNodeCreationAndDeletionInEpoch,
}

pub const ADMISSION_ADAPTER_OBLIGATIONS_V3: [AdmissionAdapterObligationV3; 10] = [
    AdmissionAdapterObligationV3::AuthenticateClock,
    AdmissionAdapterObligationV3::AuthenticatePolicyIdentity,
    AdmissionAdapterObligationV3::DeriveFreshCanonicalNode,
    AdmissionAdapterObligationV3::VerifyDomainSeparatedCommitmentOpening,
    AdmissionAdapterObligationV3::DeriveCandidateGeometryAndFunding,
    AdmissionAdapterObligationV3::AuthenticateCandidateVerdict,
    AdmissionAdapterObligationV3::AuthenticateCandidateBundleClosure,
    AdmissionAdapterObligationV3::AuthenticateSelectedSettlementTerminal,
    AdmissionAdapterObligationV3::MoveLamportsAndCloseAtomically,
    AdmissionAdapterObligationV3::CountNodeCreationAndDeletionInEpoch,
];

/// Remaining limits after removing the V2 shared finite admission page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionSuccessorLimitV3 {
    SbfAdapterNotConnected,
    LiveAccountTagsNotAllocated,
    CommitmentHasherOutsideKernel,
    CandidateBundleJoinNotConnected,
    VerificationBandwidthCanStillBeContended,
    ProposerCensorshipAndGeneralMevNotSolved,
}

pub const ADMISSION_SUCCESSOR_LIMITS_V3: [AdmissionSuccessorLimitV3; 6] = [
    AdmissionSuccessorLimitV3::SbfAdapterNotConnected,
    AdmissionSuccessorLimitV3::LiveAccountTagsNotAllocated,
    AdmissionSuccessorLimitV3::CommitmentHasherOutsideKernel,
    AdmissionSuccessorLimitV3::CandidateBundleJoinNotConnected,
    AdmissionSuccessorLimitV3::VerificationBandwidthCanStillBeContended,
    AdmissionSuccessorLimitV3::ProposerCensorshipAndGeneralMevNotSolved,
];

/// The two public windows, with commit/reveal as submission subintervals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdmissionPhaseV3 {
    Commit,
    Reveal,
    Verification,
    Terminal,
}

/// Immutable half-open schedule stamped at freeze.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionScheduleV3 {
    pub frozen_slot: u64,
    pub reveal_opens_slot: u64,
    pub submission_closes_slot: u64,
    pub verification_closes_slot: u64,
}

impl AdmissionScheduleV3 {
    pub fn stamp(
        frozen_slot: u64,
        commit_span_slots: u64,
        reveal_span_slots: u64,
        verification_span_slots: u64,
    ) -> Result<Self, Error> {
        if frozen_slot == 0
            || commit_span_slots == 0
            || reveal_span_slots == 0
            || verification_span_slots == 0
        {
            return Err(Error::InvalidSchedule);
        }
        let reveal_opens_slot = add(frozen_slot, commit_span_slots)?;
        let submission_closes_slot = add(reveal_opens_slot, reveal_span_slots)?;
        let verification_closes_slot = add(submission_closes_slot, verification_span_slots)?;
        Ok(Self {
            frozen_slot,
            reveal_opens_slot,
            submission_closes_slot,
            verification_closes_slot,
        })
    }

    pub const fn phase(self, slot: u64) -> Result<AdmissionPhaseV3, Error> {
        if slot < self.frozen_slot {
            Err(Error::NotActive)
        } else if slot < self.reveal_opens_slot {
            Ok(AdmissionPhaseV3::Commit)
        } else if slot < self.submission_closes_slot {
            Ok(AdmissionPhaseV3::Reveal)
        } else if slot < self.verification_closes_slot {
            Ok(AdmissionPhaseV3::Verification)
        } else {
            Ok(AdmissionPhaseV3::Terminal)
        }
    }
}

/// Immutable policy for the successor admission ledger.
///
/// There is intentionally no candidate-count cap and no sponsor-funded index
/// geometry. The finite submission interval and chain throughput bound the
/// number of successful admissions, while each admission capitalizes its own
/// node and eventual cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateAdmissionPolicyV3 {
    pub policy_id: Id,
    pub neutral_sink: Id,
    pub commit_span_slots: u64,
    pub reveal_span_slots: u64,
    pub verification_span_slots: u64,
    pub bond_lamports: u64,
    pub abandonment_penalty: u64,
    pub node_cleanup_reward: u64,
    pub flags: u8,
}

impl CandidateAdmissionPolicyV3 {
    pub fn validate(self) -> Result<(), Error> {
        live(self.policy_id)?;
        live(self.neutral_sink)?;
        if self.commit_span_slots == 0
            || self.reveal_span_slots == 0
            || self.verification_span_slots == 0
            || self.bond_lamports == 0
            || self.node_cleanup_reward == 0
            || self.abandonment_penalty > self.bond_lamports
            || self.flags != 0
        {
            return Err(Error::InvalidPolicy);
        }
        AdmissionScheduleV3::stamp(
            1,
            self.commit_span_slots,
            self.reveal_span_slots,
            self.verification_span_slots,
        )?;
        Ok(())
    }
}

/// Successor Window state. This is not a codec or a live account allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateWindowV4 {
    pub epoch: Id,
    pub market: Id,
    pub relation_policy_id: Id,
    pub admission_policy_id: Id,
    pub score_policy_id: Id,
    pub freeze_deadline_slot: u64,
    pub frozen_slot: u64,
    pub reveal_opens_slot: u64,
    pub submission_closes_slot: u64,
    pub verification_closes_slot: u64,
    pub finalized_slot: u64,
    /// Newest still-live node. Every node authenticates its predecessor.
    pub admission_head: Id,
    pub best_candidate_node: Id,
    pub selected_candidate_node: Id,
    pub best_rank_key: RankKey,
    pub admitted_count: u64,
    pub revealed_count: u64,
    pub verdict_count: u64,
    pub valid_verdict_count: u64,
    pub expired_commitment_count: u64,
    pub expired_unverified_count: u64,
    pub live_node_count: u64,
    pub closed_node_count: u64,
    pub rank_key_len: u8,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateWindowV4 {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        epoch: Id,
        market: Id,
        relation_policy_id: Id,
        admission: CandidateAdmissionPolicyV3,
        score: ScorePolicyBindingV1,
        freeze_deadline_slot: u64,
        stored_bump: u8,
    ) -> Result<Self, Error> {
        for identity in [epoch, market, relation_policy_id] {
            live(identity)?;
        }
        admission.validate()?;
        score.validate()?;
        if freeze_deadline_slot == 0 {
            return Err(Error::InvalidSchedule);
        }
        let value = Self {
            epoch,
            market,
            relation_policy_id,
            admission_policy_id: admission.policy_id,
            score_policy_id: score.policy_id,
            freeze_deadline_slot,
            frozen_slot: 0,
            reveal_opens_slot: 0,
            submission_closes_slot: 0,
            verification_closes_slot: 0,
            finalized_slot: 0,
            admission_head: Id::ZERO,
            best_candidate_node: Id::ZERO,
            selected_candidate_node: Id::ZERO,
            best_rank_key: RankKey::EMPTY,
            admitted_count: 0,
            revealed_count: 0,
            verdict_count: 0,
            valid_verdict_count: 0,
            expired_commitment_count: 0,
            expired_unverified_count: 0,
            live_node_count: 0,
            closed_node_count: 0,
            rank_key_len: score.rank_key_len,
            stored_bump,
            flags: 0,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), Error> {
        for identity in [
            self.epoch,
            self.market,
            self.relation_policy_id,
            self.admission_policy_id,
            self.score_policy_id,
        ] {
            live(identity)?;
        }
        if self.freeze_deadline_slot == 0
            || self.rank_key_len < 32
            || usize::from(self.rank_key_len) > RANK_KEY_CAPACITY
            || self.flags != 0
            || self.revealed_count > self.admitted_count
            || self.verdict_count > self.revealed_count
            || self.valid_verdict_count > self.verdict_count
            || self.expired_commitment_count > self.admitted_count
            || self.expired_unverified_count > self.revealed_count
            || add(self.revealed_count, self.expired_commitment_count)? > self.admitted_count
            || add(self.verdict_count, self.expired_unverified_count)? > self.revealed_count
            || add(self.live_node_count, self.closed_node_count)? != self.admitted_count
        {
            return Err(Error::InvalidCount);
        }
        if self.live_node_count == 0 {
            if !self.admission_head.is_zero() {
                return Err(Error::MismatchedBinding);
            }
        } else {
            live(self.admission_head)?;
        }
        if self.valid_verdict_count == 0 {
            if !self.best_candidate_node.is_zero() || !self.best_rank_key.is_empty() {
                return Err(Error::MismatchedBinding);
            }
        } else {
            live(self.best_candidate_node)?;
            self.best_rank_key
                .validate_for_candidate(self.best_candidate_node)?;
            if self.best_rank_key.len() != self.rank_key_len {
                return Err(Error::MismatchedBinding);
            }
        }
        if self.frozen_slot == 0 {
            if self.reveal_opens_slot != 0
                || self.submission_closes_slot != 0
                || self.verification_closes_slot != 0
                || self.finalized_slot != 0
                || self.admitted_count != 0
                || !self.selected_candidate_node.is_zero()
            {
                return Err(Error::InvalidState);
            }
        } else if self.frozen_slot < self.freeze_deadline_slot
            || self.frozen_slot >= self.reveal_opens_slot
            || self.reveal_opens_slot >= self.submission_closes_slot
            || self.submission_closes_slot >= self.verification_closes_slot
        {
            return Err(Error::InvalidSchedule);
        }
        if self.finalized_slot == 0 {
            if !self.selected_candidate_node.is_zero() || self.closed_node_count != 0 {
                return Err(Error::InvalidState);
            }
        } else {
            if self.finalized_slot < self.submission_closes_slot
                || self.selected_candidate_node != self.best_candidate_node
            {
                return Err(Error::InvalidSchedule);
            }
            if self.finalized_slot < self.verification_closes_slot
                && self.terminal_candidate_count()? != self.admitted_count
            {
                return Err(Error::UnresolvedCandidates);
            }
        }
        Ok(())
    }

    pub fn bind_policies(
        self,
        admission: CandidateAdmissionPolicyV3,
        score: ScorePolicyBindingV1,
    ) -> Result<(), Error> {
        self.validate()?;
        admission.validate()?;
        score.validate()?;
        if self.admission_policy_id != admission.policy_id
            || self.score_policy_id != score.policy_id
            || self.rank_key_len != score.rank_key_len
        {
            return Err(Error::MismatchedBinding);
        }
        if self.frozen_slot != 0 {
            let schedule = AdmissionScheduleV3::stamp(
                self.frozen_slot,
                admission.commit_span_slots,
                admission.reveal_span_slots,
                admission.verification_span_slots,
            )?;
            if self.reveal_opens_slot != schedule.reveal_opens_slot
                || self.submission_closes_slot != schedule.submission_closes_slot
                || self.verification_closes_slot != schedule.verification_closes_slot
            {
                return Err(Error::InvalidSchedule);
            }
        }
        Ok(())
    }

    pub fn schedule(self) -> Result<AdmissionScheduleV3, Error> {
        self.validate()?;
        if self.frozen_slot == 0 {
            return Err(Error::NotActive);
        }
        Ok(AdmissionScheduleV3 {
            frozen_slot: self.frozen_slot,
            reveal_opens_slot: self.reveal_opens_slot,
            submission_closes_slot: self.submission_closes_slot,
            verification_closes_slot: self.verification_closes_slot,
        })
    }

    pub fn terminal_candidate_count(self) -> Result<u64, Error> {
        add(
            add(self.verdict_count, self.expired_commitment_count)?,
            self.expired_unverified_count,
        )
    }

    pub fn admission_ledger_retired(self) -> Result<bool, Error> {
        self.validate()?;
        Ok(self.finalized_slot != 0
            && self.live_node_count == 0
            && self.closed_node_count == self.admitted_count)
    }
}

pub fn freeze_candidate_window_v4(
    window: CandidateWindowV4,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<CandidateWindowV4, Error> {
    window.bind_policies(admission, score)?;
    if window.frozen_slot != 0 {
        return Err(Error::Replay);
    }
    if now_slot < window.freeze_deadline_slot {
        return Err(Error::NotActive);
    }
    let schedule = AdmissionScheduleV3::stamp(
        now_slot,
        admission.commit_span_slots,
        admission.reveal_span_slots,
        admission.verification_span_slots,
    )?;
    let mut next = window;
    next.frozen_slot = schedule.frozen_slot;
    next.reveal_opens_slot = schedule.reveal_opens_slot;
    next.submission_closes_slot = schedule.submission_closes_slot;
    next.verification_closes_slot = schedule.verification_closes_slot;
    next.validate()?;
    Ok(next)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdmissionNodeStatusV3 {
    Committed,
    Revealed,
    VerifiedValid,
    VerifiedRefused,
    ExpiredCommitment,
    ExpiredUnverified,
}

/// One individually funded reverse-linked admission node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateAdmissionNodeV3 {
    pub epoch: Id,
    pub node: Id,
    pub previous_node: Id,
    pub admission_policy_id: Id,
    pub commitment: Id,
    pub submitter_authority: Id,
    pub solver_reward_destination: Id,
    pub payer: Id,
    pub refund_destination: Id,
    pub candidate_digest: Id,
    pub rank_key: RankKey,
    pub ordinal: u64,
    pub committed_slot: u64,
    pub revealed_slot: u64,
    pub terminal_slot: u64,
    pub node_rent_principal: u64,
    pub bond_lamports: u64,
    pub cleanup_reward: u64,
    pub status: AdmissionNodeStatusV3,
    pub stored_bump: u8,
    pub flags: u8,
}

impl CandidateAdmissionNodeV3 {
    pub fn validate(self) -> Result<(), Error> {
        for identity in [
            self.epoch,
            self.node,
            self.admission_policy_id,
            self.commitment,
            self.submitter_authority,
            self.solver_reward_destination,
            self.payer,
            self.refund_destination,
        ] {
            live(identity)?;
        }
        if self.ordinal == 0
            || self.committed_slot == 0
            || self.node_rent_principal == 0
            || self.bond_lamports == 0
            || self.cleanup_reward == 0
            || self.flags != 0
            || self.node == self.previous_node
        {
            return Err(Error::InvalidState);
        }
        match self.status {
            AdmissionNodeStatusV3::Committed => {
                if !self.candidate_digest.is_zero()
                    || !self.rank_key.is_empty()
                    || self.revealed_slot != 0
                    || self.terminal_slot != 0
                {
                    return Err(Error::InvalidState);
                }
            }
            AdmissionNodeStatusV3::Revealed => {
                live(self.candidate_digest)?;
                if !self.rank_key.is_empty()
                    || self.revealed_slot < self.committed_slot
                    || self.terminal_slot != 0
                {
                    return Err(Error::InvalidState);
                }
            }
            AdmissionNodeStatusV3::VerifiedValid => {
                live(self.candidate_digest)?;
                self.rank_key.validate_for_candidate(self.node)?;
                if self.revealed_slot < self.committed_slot
                    || self.terminal_slot < self.revealed_slot
                {
                    return Err(Error::InvalidState);
                }
            }
            AdmissionNodeStatusV3::VerifiedRefused | AdmissionNodeStatusV3::ExpiredUnverified => {
                live(self.candidate_digest)?;
                if !self.rank_key.is_empty()
                    || self.revealed_slot < self.committed_slot
                    || self.terminal_slot < self.revealed_slot
                {
                    return Err(Error::InvalidState);
                }
            }
            AdmissionNodeStatusV3::ExpiredCommitment => {
                if !self.candidate_digest.is_zero()
                    || !self.rank_key.is_empty()
                    || self.revealed_slot != 0
                    || self.terminal_slot < self.committed_slot
                {
                    return Err(Error::InvalidState);
                }
            }
        }
        Ok(())
    }

    pub fn bind_window(self, window: CandidateWindowV4) -> Result<(), Error> {
        self.validate()?;
        window.validate()?;
        if self.epoch != window.epoch
            || self.admission_policy_id != window.admission_policy_id
            || self.ordinal > window.admitted_count
            || self.committed_slot < window.frozen_slot
            || self.committed_slot >= window.reveal_opens_slot
        {
            return Err(Error::MismatchedBinding);
        }
        match self.status {
            AdmissionNodeStatusV3::Committed => {}
            AdmissionNodeStatusV3::Revealed => {
                if self.revealed_slot < window.reveal_opens_slot
                    || self.revealed_slot >= window.submission_closes_slot
                {
                    return Err(Error::InvalidSchedule);
                }
            }
            AdmissionNodeStatusV3::VerifiedValid | AdmissionNodeStatusV3::VerifiedRefused => {
                if self.revealed_slot < window.reveal_opens_slot
                    || self.revealed_slot >= window.submission_closes_slot
                    || self.terminal_slot < window.submission_closes_slot
                    || self.terminal_slot >= window.verification_closes_slot
                {
                    return Err(Error::InvalidSchedule);
                }
            }
            AdmissionNodeStatusV3::ExpiredCommitment => {
                if self.terminal_slot < window.submission_closes_slot {
                    return Err(Error::InvalidSchedule);
                }
            }
            AdmissionNodeStatusV3::ExpiredUnverified => {
                if self.revealed_slot < window.reveal_opens_slot
                    || self.revealed_slot >= window.submission_closes_slot
                    || self.terminal_slot < window.verification_closes_slot
                {
                    return Err(Error::InvalidSchedule);
                }
            }
        }
        if self.status == AdmissionNodeStatusV3::VerifiedValid
            && self.rank_key.len() != window.rank_key_len
        {
            return Err(Error::MismatchedBinding);
        }
        if self.node == window.best_candidate_node
            && (self.status != AdmissionNodeStatusV3::VerifiedValid
                || self.rank_key != window.best_rank_key)
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    pub fn bind_policy(self, admission: CandidateAdmissionPolicyV3) -> Result<(), Error> {
        self.validate()?;
        admission.validate()?;
        if self.admission_policy_id != admission.policy_id
            || self.bond_lamports != admission.bond_lamports
            || self.cleanup_reward != admission.node_cleanup_reward
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self.status,
            AdmissionNodeStatusV3::VerifiedValid
                | AdmissionNodeStatusV3::VerifiedRefused
                | AdmissionNodeStatusV3::ExpiredCommitment
                | AdmissionNodeStatusV3::ExpiredUnverified
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitCandidateInputV3 {
    pub node: Id,
    pub commitment: Id,
    pub submitter_authority: Id,
    pub solver_reward_destination: Id,
    pub payer: Id,
    pub refund_destination: Id,
    pub node_rent_principal: u64,
    pub stored_bump: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitCandidateTransitionV3 {
    pub window: CandidateWindowV4,
    pub node: CandidateAdmissionNodeV3,
    /// Exact caller capitalization. No epoch sponsor page is debited.
    pub required_lamports: u64,
}

pub fn commit_candidate_v3(
    window: CandidateWindowV4,
    input: CommitCandidateInputV3,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<CommitCandidateTransitionV3, Error> {
    window.bind_policies(admission, score)?;
    if window.finalized_slot != 0 || window.schedule()?.phase(now_slot)? != AdmissionPhaseV3::Commit
    {
        return Err(Error::NotActive);
    }
    for identity in [
        input.node,
        input.commitment,
        input.submitter_authority,
        input.solver_reward_destination,
        input.payer,
        input.refund_destination,
    ] {
        live(identity)?;
    }
    if input.node == window.admission_head || input.node_rent_principal == 0 {
        return Err(Error::DuplicateIdentity);
    }
    let ordinal = add(window.admitted_count, 1)?;
    let required_lamports = add(
        add(input.node_rent_principal, admission.bond_lamports)?,
        admission.node_cleanup_reward,
    )?;
    let node = CandidateAdmissionNodeV3 {
        epoch: window.epoch,
        node: input.node,
        previous_node: window.admission_head,
        admission_policy_id: admission.policy_id,
        commitment: input.commitment,
        submitter_authority: input.submitter_authority,
        solver_reward_destination: input.solver_reward_destination,
        payer: input.payer,
        refund_destination: input.refund_destination,
        candidate_digest: Id::ZERO,
        rank_key: RankKey::EMPTY,
        ordinal,
        committed_slot: now_slot,
        revealed_slot: 0,
        terminal_slot: 0,
        node_rent_principal: input.node_rent_principal,
        bond_lamports: admission.bond_lamports,
        cleanup_reward: admission.node_cleanup_reward,
        status: AdmissionNodeStatusV3::Committed,
        stored_bump: input.stored_bump,
        flags: 0,
    };
    node.validate()?;
    let mut next_window = window;
    next_window.admission_head = node.node;
    next_window.admitted_count = ordinal;
    next_window.live_node_count = add(next_window.live_node_count, 1)?;
    next_window.validate()?;
    Ok(CommitCandidateTransitionV3 {
        window: next_window,
        node,
        required_lamports,
    })
}

/// Adapter-attested opening of the domain-separated commitment.
///
/// The adapter verifies `H(domain, epoch, policy, submitter, reward_destination,
/// candidate_digest, secret)` against `commitment`. The secret is supplied to
/// the instruction but is never persisted by this kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterVerifiedCommitmentOpeningV1 {
    pub epoch: Id,
    pub node: Id,
    pub admission_policy_id: Id,
    pub commitment: Id,
    pub submitter_authority: Id,
    pub solver_reward_destination: Id,
    pub candidate_digest: Id,
}

pub fn reveal_candidate_v3(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    opening: AdapterVerifiedCommitmentOpeningV1,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<(CandidateWindowV4, CandidateAdmissionNodeV3), Error> {
    window.bind_policies(admission, score)?;
    node.bind_window(window)?;
    node.bind_policy(admission)?;
    live(opening.candidate_digest)?;
    if window.finalized_slot != 0 || window.schedule()?.phase(now_slot)? != AdmissionPhaseV3::Reveal
    {
        return Err(Error::NotActive);
    }
    if node.status != AdmissionNodeStatusV3::Committed {
        return Err(Error::Replay);
    }
    if opening.epoch != node.epoch
        || opening.node != node.node
        || opening.admission_policy_id != node.admission_policy_id
        || opening.commitment != node.commitment
        || opening.submitter_authority != node.submitter_authority
        || opening.solver_reward_destination != node.solver_reward_destination
    {
        return Err(Error::MismatchedBinding);
    }
    let mut next_node = node;
    next_node.candidate_digest = opening.candidate_digest;
    next_node.revealed_slot = now_slot;
    next_node.status = AdmissionNodeStatusV3::Revealed;
    next_node.validate()?;
    let mut next_window = window;
    next_window.revealed_count = add(next_window.revealed_count, 1)?;
    next_window.validate()?;
    Ok((next_window, next_node))
}

pub fn expire_commitment_v3(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<(CandidateWindowV4, CandidateAdmissionNodeV3), Error> {
    window.bind_policies(admission, score)?;
    node.bind_window(window)?;
    node.bind_policy(admission)?;
    if now_slot < window.submission_closes_slot {
        return Err(Error::NotActive);
    }
    if node.status != AdmissionNodeStatusV3::Committed {
        return Err(Error::Replay);
    }
    let mut next_node = node;
    next_node.status = AdmissionNodeStatusV3::ExpiredCommitment;
    next_node.terminal_slot = now_slot;
    next_node.validate()?;
    let mut next_window = window;
    next_window.expired_commitment_count = add(next_window.expired_commitment_count, 1)?;
    next_window.validate()?;
    Ok((next_window, next_node))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterVerifiedVerdictKindV3 {
    Valid { rank_key: RankKey },
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterVerifiedVerdictV3 {
    pub epoch: Id,
    pub node: Id,
    pub candidate_digest: Id,
    pub relation_policy_id: Id,
    pub score_policy_id: Id,
    pub kind: AdapterVerifiedVerdictKindV3,
}

pub fn record_verdict_v3(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    verdict: AdapterVerifiedVerdictV3,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<(CandidateWindowV4, CandidateAdmissionNodeV3), Error> {
    window.bind_policies(admission, score)?;
    node.bind_window(window)?;
    node.bind_policy(admission)?;
    if window.finalized_slot != 0
        || window.schedule()?.phase(now_slot)? != AdmissionPhaseV3::Verification
    {
        return Err(Error::NotActive);
    }
    if node.status != AdmissionNodeStatusV3::Revealed {
        return Err(Error::Replay);
    }
    if verdict.epoch != window.epoch
        || verdict.node != node.node
        || verdict.candidate_digest != node.candidate_digest
        || verdict.relation_policy_id != window.relation_policy_id
        || verdict.score_policy_id != window.score_policy_id
    {
        return Err(Error::MismatchedBinding);
    }
    let mut next_window = window;
    let mut next_node = node;
    next_window.verdict_count = add(next_window.verdict_count, 1)?;
    next_node.terminal_slot = now_slot;
    match verdict.kind {
        AdapterVerifiedVerdictKindV3::Valid { rank_key } => {
            rank_key.validate_for_candidate(node.node)?;
            if rank_key.len() != score.rank_key_len {
                return Err(Error::MismatchedBinding);
            }
            next_node.rank_key = rank_key;
            next_node.status = AdmissionNodeStatusV3::VerifiedValid;
            next_window.valid_verdict_count = add(next_window.valid_verdict_count, 1)?;
            let replace = if next_window.best_candidate_node.is_zero() {
                true
            } else {
                rank_key.compare(next_window.best_rank_key)? == Ordering::Greater
            };
            if replace {
                next_window.best_candidate_node = node.node;
                next_window.best_rank_key = rank_key;
            }
        }
        AdapterVerifiedVerdictKindV3::Refused => {
            next_node.status = AdmissionNodeStatusV3::VerifiedRefused;
        }
    }
    next_node.validate()?;
    next_window.validate()?;
    Ok((next_window, next_node))
}

pub fn expire_unverified_v3(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<(CandidateWindowV4, CandidateAdmissionNodeV3), Error> {
    window.bind_policies(admission, score)?;
    node.bind_window(window)?;
    node.bind_policy(admission)?;
    if now_slot < window.verification_closes_slot {
        return Err(Error::NotActive);
    }
    if node.status != AdmissionNodeStatusV3::Revealed {
        return Err(Error::Replay);
    }
    let mut next_node = node;
    next_node.status = AdmissionNodeStatusV3::ExpiredUnverified;
    next_node.terminal_slot = now_slot;
    next_node.validate()?;
    let mut next_window = window;
    next_window.expired_unverified_count = add(next_window.expired_unverified_count, 1)?;
    next_window.validate()?;
    Ok((next_window, next_node))
}

pub fn finalize_selection_v3(
    window: CandidateWindowV4,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    now_slot: u64,
) -> Result<CandidateWindowV4, Error> {
    window.bind_policies(admission, score)?;
    if window.finalized_slot != 0 {
        return Err(Error::Replay);
    }
    if now_slot < window.submission_closes_slot {
        return Err(Error::NotActive);
    }
    if now_slot < window.verification_closes_slot
        && window.terminal_candidate_count()? != window.admitted_count
    {
        return Err(Error::UnresolvedCandidates);
    }
    let mut next = window;
    next.finalized_slot = now_slot;
    next.selected_candidate_node = next.best_candidate_node;
    next.validate()?;
    Ok(next)
}

/// Adapter-authenticated evidence required before deleting a node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterVerifiedAdmissionCleanupV3 {
    pub epoch: Id,
    pub node: Id,
    pub candidate_digest: Id,
    pub candidate_bundle_closed: u8,
    pub selected_settlement_terminal_slot: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionNodeCloseDispositionV3 {
    pub keeper_reward: u64,
    pub bond_refund: u64,
    pub neutral_sink_credit: u64,
    pub rent_principal_refund: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseAdmissionHeadTransitionV3 {
    pub window: CandidateWindowV4,
    pub disposition: AdmissionNodeCloseDispositionV3,
}

pub fn close_admission_head_v3(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    evidence: AdapterVerifiedAdmissionCleanupV3,
    admission: CandidateAdmissionPolicyV3,
    score: ScorePolicyBindingV1,
    observed_lamports: u64,
) -> Result<CloseAdmissionHeadTransitionV3, Error> {
    window.bind_policies(admission, score)?;
    node.bind_window(window)?;
    node.bind_policy(admission)?;
    if window.finalized_slot == 0 || !node.is_terminal() {
        return Err(Error::NotActive);
    }
    if window.admission_head != node.node || window.live_node_count != node.ordinal {
        return Err(Error::MismatchedBinding);
    }
    if evidence.epoch != node.epoch
        || evidence.node != node.node
        || evidence.candidate_digest != node.candidate_digest
        || evidence.candidate_bundle_closed != 1
        || (node.node == window.selected_candidate_node
            && evidence.selected_settlement_terminal_slot < window.finalized_slot)
        || (node.node != window.selected_candidate_node
            && evidence.selected_settlement_terminal_slot != 0)
    {
        return Err(Error::MismatchedBinding);
    }
    let expected = add(
        add(node.node_rent_principal, node.bond_lamports)?,
        node.cleanup_reward,
    )?;
    let surplus = observed_lamports
        .checked_sub(expected)
        .ok_or(Error::Underfunded)?;
    let abandonment_penalty = if node.status == AdmissionNodeStatusV3::ExpiredCommitment {
        admission.abandonment_penalty
    } else {
        0
    };
    let bond_refund = node
        .bond_lamports
        .checked_sub(abandonment_penalty)
        .ok_or(Error::ArithmeticOverflow)?;
    let neutral_sink_credit = add(surplus, abandonment_penalty)?;
    let mut next_window = window;
    next_window.admission_head = node.previous_node;
    next_window.live_node_count = next_window
        .live_node_count
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next_window.closed_node_count = add(next_window.closed_node_count, 1)?;
    next_window.validate()?;
    Ok(CloseAdmissionHeadTransitionV3 {
        window: next_window,
        disposition: AdmissionNodeCloseDispositionV3 {
            keeper_reward: node.cleanup_reward,
            bond_refund,
            neutral_sink_credit,
            rent_principal_refund: node.node_rent_principal,
        },
    })
}
