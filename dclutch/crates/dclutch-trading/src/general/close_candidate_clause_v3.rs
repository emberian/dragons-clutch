//! Which conjunct of the CloseCandidate authentication disagreed.
//!
//! `authenticate_general_close_candidate_v3` joined twenty-nine accusations
//! with `||`, ran the work-escrow plan, then rejoined five of the plan's own
//! numbers -- and published `InvalidCoordinate` for all thirty-four.
//!
//! THE FIVE POSTCONDITION CLAUSES ARE NOT COORDINATE TYPOS. They are the
//! rejoin of [`crate::general::escrow_v1::WorkEscrowClosePlanV1`] to the three
//! observed balances the request came in with, which is the last place an
//! overcapitalized or undercapitalized close can be caught before Trading moves
//! lamports. Reading "an authenticated coordinate disagrees" for a conservation
//! failure is the same word for two different kinds of accusation, and it hid
//! the one that matters.
//!
//! ONE CONJUNCT WAS DELETED RATHER THAN NAMED. The chain also compared
//! `PRIMARY_BENEFICIARY` -- the lifecycle's WRITE register -- against the
//! solver. A Close plan emits no lifecycle protected output, so nothing ever
//! writes that register on this route and the conjunct could only ever hold by
//! accident of a zeroed bank. Its honest half survives as
//! [`CloseCandidateClauseV3::RecordedBeneficiary`], which reads the OBSERVATION
//! register the profile projects from the live state account. A clause nothing
//! can satisfy is not a check; naming it would have made a permanent
//! false-positive legible instead of removing it.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: one enum, in
//! evaluation order, with the sentence a reader sees beside the variant.

/// One named clause of the CloseCandidate conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseCandidateClauseV3 {
    /// The executing Product width is zero.
    OutcomeCountZero,
    /// The submission was made at another width.
    SubmissionOutcomeCount,
    /// The batch was opened at another width.
    BatchOutcomeCount,
    /// The submission names another batch than the one supplied.
    SubmissionBatch,
    /// The environment carries no General root.
    EnvironmentGeneralRoot,
    /// The environment carries no Trading program.
    EnvironmentTradingProgram,
    /// The batch belongs to another Market.
    BatchMarket,
    /// The batch names another Product than the environment's record digest.
    BatchProduct,
    /// The batch belongs to another config.
    BatchConfigId,
    /// The batch belongs to another generation.
    BatchGeneration,
    /// The batch is not `Closed`.
    BatchStatus,
    /// The CloseCandidate request names another candidate.
    RequestSubject,
    /// `OUTCOME_COUNT` is not the executing width.
    ScalarOutcomeCount,
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `CANDIDATE_STATUS_OBSERVATION` is not the submission's own status.
    ScalarCandidateStatus,
    /// `CANDIDATE_VERIFICATION_REMAINING_OBSERVATION` is not the submission's.
    ScalarVerificationRemaining,
    /// `CANDIDATE_CLEANUP_REMAINING_OBSERVATION` is not the submission's.
    ScalarCleanupRemaining,
    /// `CANDIDATE_REWARD_RATE` is not the submission's own reward rate.
    ScalarRewardRate,
    /// `BATCH_SETTLEMENT_CLOSE_SLOT` is not the batch's own.
    ScalarBatchSettlementClose,
    /// `BATCH_STATUS_OBSERVATION` is not the batch's own status.
    ScalarBatchStatus,
    /// `PRIMARY_PRINCIPAL_OBSERVATION` is not the rent principal being closed.
    ScalarPrincipalObservation,
    /// The Candidate state carries no rent principal.
    ScalarRentPrincipal,
    /// `PARENT_REQUEST_DIGEST` is not the candidate being closed.
    RequestDigest,
    /// `CANDIDATE` is not the candidate being closed.
    IdentityCandidate,
    /// `SELECTION_BATCH` is not the batch the submission named.
    RecordedBatch,
    /// `OWNER` is not the solver who submitted the candidate.
    IdentityOwner,
    /// The rent-credit wallet is not the solver's.
    ///
    /// At CloseCandidate the credit coordinate IS the solver's wallet: the
    /// generic Effect pays the verification remainder there and Lifecycle V5
    /// returns the historical rent principal to the same account.
    SolverWallet,
    /// `PRIMARY_BENEFICIARY_OBSERVATION` is not the solver who funded the state.
    ///
    /// The observation of what the live Candidate account records, not the
    /// register a lifecycle write would fill -- this route performs no such
    /// write.
    RecordedBeneficiary,
    /// `PAYER` carries no permissionless caller.
    IdentityPayer,
    /// The plan's escrow prestate is not the observed position balance.
    PlanEscrowBefore,
    /// The plan's cranker prestate is not the observed admission balance.
    PlanCrankerBefore,
    /// The plan's solver prestate is not the observed escrow balance.
    PlanSolverBefore,
    /// The plan's cleanup reward is not the submission's cleanup compartment.
    PlanCleanupReward,
    /// The plan's solver credit is not the verification remainder plus rent.
    PlanSolverCredit,
}

impl CloseCandidateClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a thirty-fifth clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::OutcomeCountZero => "close-candidate: the executing width is zero",
            Self::SubmissionOutcomeCount => "close-candidate: the submission is another width",
            Self::BatchOutcomeCount => "close-candidate: the batch is another width",
            Self::SubmissionBatch => "close-candidate: the submission names another batch",
            Self::EnvironmentGeneralRoot => "close-candidate: the environment has no General root",
            Self::EnvironmentTradingProgram => {
                "close-candidate: the environment has no Trading program"
            }
            Self::BatchMarket => "close-candidate: the batch belongs to another Market",
            Self::BatchProduct => "close-candidate: the batch names another Product",
            Self::BatchConfigId => "close-candidate: the batch belongs to another config",
            Self::BatchGeneration => "close-candidate: the batch belongs to another generation",
            Self::BatchStatus => "close-candidate: the batch is not closed",
            Self::RequestSubject => "close-candidate: the request names another candidate",
            Self::ScalarOutcomeCount => "close-candidate: OUTCOME_COUNT is not the width",
            Self::ScalarRootLifecycle => "close-candidate: the observed root is not Active",
            Self::ScalarCandidateStatus => {
                "close-candidate: the observed candidate status disagrees"
            }
            Self::ScalarVerificationRemaining => {
                "close-candidate: the observed verification compartment disagrees"
            }
            Self::ScalarCleanupRemaining => {
                "close-candidate: the observed cleanup compartment disagrees"
            }
            Self::ScalarRewardRate => "close-candidate: the observed reward rate disagrees",
            Self::ScalarBatchSettlementClose => {
                "close-candidate: the observed settlement close disagrees"
            }
            Self::ScalarBatchStatus => "close-candidate: the observed batch status disagrees",
            Self::ScalarPrincipalObservation => {
                "close-candidate: the observed rent principal disagrees"
            }
            Self::ScalarRentPrincipal => "close-candidate: the state carries no rent principal",
            Self::RequestDigest => "close-candidate: the request digest is not the candidate",
            Self::IdentityCandidate => "close-candidate: CANDIDATE is not the candidate",
            Self::RecordedBatch => "close-candidate: SELECTION_BATCH is not the submission batch",
            Self::IdentityOwner => "close-candidate: OWNER is not the solver",
            Self::SolverWallet => "close-candidate: the rent credit is not the solver wallet",
            Self::RecordedBeneficiary => {
                "close-candidate: the recorded beneficiary is not the solver"
            }
            Self::IdentityPayer => "close-candidate: PAYER carries no caller",
            Self::PlanEscrowBefore => "close-candidate: the plan escrow prestate disagrees",
            Self::PlanCrankerBefore => "close-candidate: the plan cranker prestate disagrees",
            Self::PlanSolverBefore => "close-candidate: the plan solver prestate disagrees",
            Self::PlanCleanupReward => "close-candidate: the plan cleanup reward disagrees",
            Self::PlanSolverCredit => "close-candidate: the plan solver credit disagrees",
        }
    }
}
