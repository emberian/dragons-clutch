//! Which conjunct of the SubmitCandidate projection disagreed.
//!
//! `project_general_submit_candidate_in_place_v3` used to publish exactly one
//! code for FIFTY-EIGHT accusations: fifty-seven `||`-joined clauses plus the
//! per-item outcome column in front of them. Every one of them reached a reader
//! as `InvalidCoordinate`, "an authenticated coordinate disagrees", which is
//! true of a submission body one byte off canonical, of a batch closed under
//! another market's config, of a lifecycle beneficiary naming the market's
//! sponsor instead of the solver, and of a register nothing projects at all.
//!
//! WHAT THAT COST, TWICE, MEASURED. `44c0ccf19` spent an instrumented replay
//! printing all thirty-nine coordinates the conjunct reads to learn that two of
//! them disagreed, and its first inference from the same code -- the item
//! outcome column, because that clause is FIRST -- was wrong. `160ebdfbb` then
//! narrowed the ten arms of the outer enum and recorded that "splitting that
//! discriminant is the next thing this route owes". This is that split, and the
//! campaign paid for it a third time on 2026-09-04: a bundle that had just
//! gained both missing producers still refused `InvalidCoordinate`, with no way
//! to say which of fifty-eight.
//!
//! ONE ENUM PER CONJUNCT, IN ITS OWN MODULE, exactly as
//! [`crate::runtime_verify::RuntimeVerifyErrorV2`] carries the row verifier's
//! sixteen. The sentence a reader sees lives beside the variant, so a clause
//! later split cannot leave a stale sentence behind in the accelerator; the
//! consumer logs [`SubmitCandidateClauseV3::log_line`] beneath the arm's own
//! line and publishes the same canonical refusal it always did. NO WIRE CODE
//! MOVES: `GeneralHotCandidateErrorV3` carries no `#[repr]` and never reaches
//! the chain as a discriminant -- the accelerator's acknowledgement is one
//! canonical shape and the log is its only reader (decision 0007 governs the
//! codes that DO reach the wire, and none of them is here).
//!
//! The variants are in the order the projection evaluates them, which is the
//! order a reader bisecting by hand would have had to rediscover.

/// One named clause of the SubmitCandidate coordinate conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitCandidateClauseV3 {
    /// A per-outcome `OUTCOME` column did not carry its own index.
    ItemOutcomeColumn,
    /// The supplied body is not byte-equal to `GeneralCandidateV1::submit`.
    SubmissionNotCanonical,
    /// The request names a candidate the image does not.
    RequestSubject,
    /// The image's outcome count is not the executing width.
    ImageOutcomeCount,
    /// The environment carries no General root.
    EnvironmentGeneralRoot,
    /// The environment carries no Trading program.
    EnvironmentTradingProgram,
    /// The capability root is not `Active`.
    RootLifecycle,
    /// The root names another Market.
    RootMarket,
    /// The root names another General config.
    RootConfigId,
    /// The root names another generation.
    RootGeneration,
    /// The config names another generation.
    ConfigGeneration,
    /// The batch is not `Closed`.
    BatchStatus,
    /// The batch belongs to another Market.
    BatchMarket,
    /// The batch belongs to another config.
    BatchConfigId,
    /// The batch belongs to another generation.
    BatchGeneration,
    /// The batch was opened at another width.
    BatchOutcomeCount,
    /// The batch was opened at another price scale.
    BatchPriceScale,
    /// The batch was opened under another per-candidate order bound.
    BatchMaxOrders,
    /// The batch's settlement close is not collection close plus the windows.
    BatchSettlementClose,
    /// `OUTCOME_COUNT` is not the executing width.
    ScalarOutcomeCount,
    /// `ZERO` is not the image's own outcome count.
    ScalarImageOutcomeCount,
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `BATCH_STATUS_OBSERVATION` is not the batch's own status.
    ScalarBatchStatus,
    /// `BATCH_POST_ORDER_COUNT` is not the batch's own width.
    ScalarBatchOrderCount,
    /// `BATCH_COLLECTION_CLOSE_SLOT` is not the batch's own.
    ScalarBatchCollectionClose,
    /// `BATCH_SETTLEMENT_CLOSE_SLOT` is not the batch's own.
    ScalarBatchSettlementClose,
    /// `ORDER_MAX_LOTS` is not the batch's own price scale.
    ScalarBatchPriceScale,
    /// `CANDIDATE_PAGE_COUNT` is not the image's own page count.
    ScalarImagePageCount,
    /// `SELECTION_BEST_CANDIDATE_COORDINATE` is not the image's own ordinal.
    ScalarImageCoordinate,
    /// `SELECTION_PRICE_SCALE` is not the image's own price scale.
    ScalarImagePriceScale,
    /// `VERIFY_POST_ORDER_COUNT` is not the submission's own width.
    ScalarSubmissionOutcomeCount,
    /// `VERIFY_POST_PAGE` is not the submission's own page count.
    ScalarSubmissionPageCount,
    /// `CANDIDATE_STATUS_OBSERVATION` is not the submission's own status.
    ScalarSubmissionStatus,
    /// `CANDIDATE_PAGE_REVISION` is not the submission's own revision.
    ScalarSubmissionPageRevision,
    /// `CANDIDATE_SUBMITTED_SLOT` is not the submission's own slot.
    ScalarSubmissionSlot,
    /// `CANDIDATE_ROW_COUNT` is not the submission's own row count.
    ScalarSubmissionRowCount,
    /// `CANDIDATE_REWARD_RATE` is not the submission's own reward rate.
    ScalarSubmissionRewardRate,
    /// `CANDIDATE_VERIFICATION_REMAINING_OBSERVATION` is not the submission's.
    ScalarVerificationRemaining,
    /// `CANDIDATE_CLEANUP_REMAINING_OBSERVATION` is not the submission's.
    ScalarCleanupRemaining,
    /// A create observed a nonzero prior lifecycle bump.
    LifecycleBumpObservation,
    /// A create observed a nonzero prior rent principal.
    LifecyclePrincipalObservation,
    /// The lifecycle did not report the state as created.
    LifecycleCreated,
    /// The witnessed state bump is not the canonical one.
    LifecycleBump,
    /// The lifecycle recorded no rent principal.
    LifecycleRentPrincipal,
    /// A create observed a prior lifecycle beneficiary.
    LifecycleBeneficiaryObservation,
    /// The lifecycle beneficiary is not the solver who funded the candidate.
    ///
    /// The one clause decision 0021's byte five decides. Under a `Credit` plan
    /// the preplan writes the market's RentCredit wallet here and this clause
    /// refuses every permissionless solver; the Candidate recipe declares
    /// `Payer`, so it reads the paying account and this is the join that proves
    /// that account is the solver the submission names.
    LifecycleBeneficiary,
    /// The lifecycle-owned state owner is not the Trading program.
    LifecycleOwner,
    /// `TRADING_PROGRAM` is not the executing program.
    IdentityTradingProgram,
    /// `GENERAL_ROOT` is not the environment's root.
    IdentityGeneralRoot,
    /// `CANDIDATE` is not the image's own identity.
    ///
    /// The register `GENERAL_CANDIDATE_STATE_RECIPE_V3` seeds the state address
    /// on. Until SubmitCandidate's AccountProfile gained operation 33 nothing
    /// wrote it and this clause refused every submission.
    IdentityCandidate,
    /// `BEST_VERIFIED_DIGEST` is not the image's own identity.
    IdentityBestVerifiedDigest,
    /// `ORDER` is not the image's Product.
    IdentityImageProduct,
    /// `SELECTION_POLICY` is not the image's batch.
    IdentityImageBatch,
    /// `SELECTION_PRODUCT` is not the batch's Product.
    IdentityBatchProduct,
    /// `RESULT_BENEFICIARY_OBSERVATION` is not the submission's candidate.
    IdentitySubmissionCandidate,
    /// `BENEFICIARY` is not the submission's batch.
    IdentitySubmissionBatch,
    /// `OWNER` is not the submission's solver.
    IdentitySubmissionSolver,
}

impl SubmitCandidateClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a fifty-ninth clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::ItemOutcomeColumn => "submit-candidate: an outcome column is not its own index",
            Self::SubmissionNotCanonical => {
                "submit-candidate: the body is not the canonical submission record"
            }
            Self::RequestSubject => "submit-candidate: the request names another candidate",
            Self::ImageOutcomeCount => "submit-candidate: the image is another width",
            Self::EnvironmentGeneralRoot => "submit-candidate: the environment has no General root",
            Self::EnvironmentTradingProgram => {
                "submit-candidate: the environment has no Trading program"
            }
            Self::RootLifecycle => "submit-candidate: the capability root is not Active",
            Self::RootMarket => "submit-candidate: the root names another Market",
            Self::RootConfigId => "submit-candidate: the root names another config",
            Self::RootGeneration => "submit-candidate: the root names another generation",
            Self::ConfigGeneration => "submit-candidate: the config names another generation",
            Self::BatchStatus => "submit-candidate: the batch is not closed",
            Self::BatchMarket => "submit-candidate: the batch belongs to another Market",
            Self::BatchConfigId => "submit-candidate: the batch belongs to another config",
            Self::BatchGeneration => "submit-candidate: the batch belongs to another generation",
            Self::BatchOutcomeCount => "submit-candidate: the batch is another width",
            Self::BatchPriceScale => "submit-candidate: the batch is another price scale",
            Self::BatchMaxOrders => "submit-candidate: the batch is another order bound",
            Self::BatchSettlementClose => "submit-candidate: the batch settlement window is wrong",
            Self::ScalarOutcomeCount => "submit-candidate: OUTCOME_COUNT is not the width",
            Self::ScalarImageOutcomeCount => "submit-candidate: ZERO is not the image width",
            Self::ScalarRootLifecycle => "submit-candidate: the observed root is not Active",
            Self::ScalarBatchStatus => "submit-candidate: the observed batch status disagrees",
            Self::ScalarBatchOrderCount => "submit-candidate: the observed batch width disagrees",
            Self::ScalarBatchCollectionClose => {
                "submit-candidate: the observed collection close disagrees"
            }
            Self::ScalarBatchSettlementClose => {
                "submit-candidate: the observed settlement close disagrees"
            }
            Self::ScalarBatchPriceScale => {
                "submit-candidate: the observed batch price scale disagrees"
            }
            Self::ScalarImagePageCount => "submit-candidate: the observed page count disagrees",
            Self::ScalarImageCoordinate => {
                "submit-candidate: the observed candidate ordinal disagrees"
            }
            Self::ScalarImagePriceScale => {
                "submit-candidate: the observed image price scale disagrees"
            }
            Self::ScalarSubmissionOutcomeCount => {
                "submit-candidate: the observed submission width disagrees"
            }
            Self::ScalarSubmissionPageCount => {
                "submit-candidate: the observed submission pages disagree"
            }
            Self::ScalarSubmissionStatus => {
                "submit-candidate: the observed submission status disagrees"
            }
            Self::ScalarSubmissionPageRevision => {
                "submit-candidate: the observed page revision disagrees"
            }
            Self::ScalarSubmissionSlot => "submit-candidate: the observed submitted slot disagrees",
            Self::ScalarSubmissionRowCount => "submit-candidate: the observed row count disagrees",
            Self::ScalarSubmissionRewardRate => {
                "submit-candidate: the observed reward rate disagrees"
            }
            Self::ScalarVerificationRemaining => {
                "submit-candidate: the observed verification compartment disagrees"
            }
            Self::ScalarCleanupRemaining => {
                "submit-candidate: the observed cleanup compartment disagrees"
            }
            Self::LifecycleBumpObservation => {
                "submit-candidate: a create observed a prior lifecycle bump"
            }
            Self::LifecyclePrincipalObservation => {
                "submit-candidate: a create observed a prior rent principal"
            }
            Self::LifecycleCreated => "submit-candidate: the lifecycle did not create the state",
            Self::LifecycleBump => "submit-candidate: the witnessed bump is not canonical",
            Self::LifecycleRentPrincipal => "submit-candidate: the state carries no rent principal",
            Self::LifecycleBeneficiaryObservation => {
                "submit-candidate: a create observed a prior beneficiary"
            }
            Self::LifecycleBeneficiary => {
                "submit-candidate: the rent beneficiary is not the solver who paid"
            }
            Self::LifecycleOwner => "submit-candidate: the state owner is not Trading",
            Self::IdentityTradingProgram => {
                "submit-candidate: TRADING_PROGRAM is not the executing program"
            }
            Self::IdentityGeneralRoot => "submit-candidate: GENERAL_ROOT is not the root",
            Self::IdentityCandidate => {
                "submit-candidate: CANDIDATE is not the image the address is seeded on"
            }
            Self::IdentityBestVerifiedDigest => {
                "submit-candidate: BEST_VERIFIED_DIGEST is not the image"
            }
            Self::IdentityImageProduct => "submit-candidate: ORDER is not the image Product",
            Self::IdentityImageBatch => "submit-candidate: SELECTION_POLICY is not the image batch",
            Self::IdentityBatchProduct => {
                "submit-candidate: SELECTION_PRODUCT is not the batch Product"
            }
            Self::IdentitySubmissionCandidate => {
                "submit-candidate: the result beneficiary is not the submission candidate"
            }
            Self::IdentitySubmissionBatch => {
                "submit-candidate: BENEFICIARY is not the submission batch"
            }
            Self::IdentitySubmissionSolver => {
                "submit-candidate: OWNER is not the submission solver"
            }
        }
    }
}
