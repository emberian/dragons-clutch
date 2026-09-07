//! Which conjunct of the VerifyCandidateRow bank join disagreed.
//!
//! The row verifier already names its own refusals:
//! [`crate::general::runtime_verify::RuntimeVerifyErrorV2`] carries sixteen and
//! reaches a reader through `GeneralHotCandidateErrorV3::Verify`. What did NOT
//! have names is the join AROUND it -- the ten clauses that authenticate the
//! register bank before the verifier runs, the manifest equality the workspace
//! form checks after it, and the six that rejoin the verifier's own summary to
//! the bank before a single byte is written. All seventeen published
//! `InvalidCoordinate`.
//!
//! THAT MATTERS BECAUSE VERIFICATION IS A LOOP. A solver's candidate is
//! verified one row at a time, and every row carries an optimistic page index,
//! row index, and revision that the previous row advanced. When a keeper's
//! cursor drifts by one, the clause that catches it is
//! [`VerifyCandidateClauseV3::ScalarPageIndex`],
//! [`VerifyCandidateClauseV3::ScalarRowIndex`], or
//! [`VerifyCandidateClauseV3::ScalarExpectedRevision`] -- three different
//! restarts -- and all three arrived as the same six words as a substituted
//! payer.
//!
//! The variants are in the order the workspace form evaluates them: the bank
//! authentication, then the manifest the verifier produced, then the summary
//! rejoin the failure-atomic forms share.

/// One named clause of the VerifyCandidateRow bank join.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyCandidateClauseV3 {
    /// `PARENT_REQUEST_DIGEST` is not the candidate the submission names.
    RequestSubject,
    /// `ROOT_EXPECTED_REVISION` is not the optimistic verifier revision.
    ScalarExpectedRevision,
    /// `COMPLETE_SET_MOVE` is not the optimistic page index.
    ScalarPageIndex,
    /// `CLAIMS_AFFINE_ACTIVE` is not the optimistic row index.
    ScalarRowIndex,
    /// `OUTCOME_COUNT` is not the executing width.
    ScalarOutcomeCount,
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `OBSERVED_POSITION_LAMPORTS` is not rent plus both open compartments.
    ScalarObservedLamports,
    /// The Candidate state carries no rent principal.
    ScalarPrincipal,
    /// `PAYER` carries no permissionless caller.
    Payer,
    /// `TRADING_PROGRAM` carries no executing program.
    TradingProgram,
    /// The manifest the verifier produced is not the one the request supplied.
    ///
    /// Settlement evidence is readonly and derived, never a caller-selected
    /// order list; this is the clause that makes it so.
    Manifest,
    /// The row reward is not the submission's own reward rate.
    SummaryReward,
    /// The advanced cursor was written at another width.
    CursorOutcomeCount,
    /// The advanced cursor names another candidate.
    CursorCandidate,
    /// The advanced cursor names another batch.
    CursorBatch,
    /// The advanced cursor's revision is not the summary's.
    CursorRevision,
    /// The advanced cursor's order count is not the summary's.
    CursorOrderCount,
}

impl VerifyCandidateClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so an eighteenth clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RequestSubject => "verify-row: the request names another candidate",
            Self::ScalarExpectedRevision => "verify-row: the observed cursor revision disagrees",
            Self::ScalarPageIndex => "verify-row: the observed page index disagrees",
            Self::ScalarRowIndex => "verify-row: the observed row index disagrees",
            Self::ScalarOutcomeCount => "verify-row: OUTCOME_COUNT is not the width",
            Self::ScalarRootLifecycle => "verify-row: the observed root is not Active",
            Self::ScalarObservedLamports => {
                "verify-row: the observed balance is not rent plus the compartments"
            }
            Self::ScalarPrincipal => "verify-row: the state carries no rent principal",
            Self::Payer => "verify-row: PAYER carries no caller",
            Self::TradingProgram => "verify-row: TRADING_PROGRAM carries no program",
            Self::Manifest => "verify-row: the supplied manifest is not the derived one",
            Self::SummaryReward => "verify-row: the row reward is not the submission rate",
            Self::CursorOutcomeCount => "verify-row: the advanced cursor is another width",
            Self::CursorCandidate => "verify-row: the advanced cursor names another candidate",
            Self::CursorBatch => "verify-row: the advanced cursor names another batch",
            Self::CursorRevision => "verify-row: the advanced cursor revision disagrees",
            Self::CursorOrderCount => "verify-row: the advanced cursor order count disagrees",
        }
    }
}
