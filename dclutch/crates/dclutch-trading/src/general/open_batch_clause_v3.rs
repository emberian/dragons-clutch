//! Which conjunct of the OpenBatch projection disagreed.
//!
//! `project_general_open_batch_candidate_in_place_v3` joined twenty-five
//! accusations with `||` and published one word for all of them:
//! `InvalidCoordinate`, "an authenticated coordinate disagrees". That sentence
//! is equally true of a root belonging to another Market, of a config whose
//! collection window is one slot off the register that observed it, and of a
//! state account whose rent principal nobody funded. A reader who had only the
//! word had to bisect the conjunct by hand, which is the cost
//! [`crate::general::submit_candidate_clause_v3`] was written to stop paying
//! for SubmitCandidate. This is the same split for the action that creates the
//! batch every later action names.
//!
//! THE FIRST CLAUSE IS NEW. OpenBatch used to compare the caller's
//! `expected_revision` only against the bank's `ROOT_EXPECTED_REVISION`
//! register and let [`crate::general::collection_v1::GeneralBatchV1::open`]
//! refuse a stale revision from inside the collection contract -- so the most
//! common honest failure on a busy market, two openers racing one root, reached
//! a reader as the collection contract's coarse refusal rather than as a named
//! revision disagreement. [`OpenBatchClauseV3::RootRevision`] asks the live
//! root first, by name.
//!
//! The two postconditions after the transition are clauses too: `open` may
//! succeed and still produce a batch the request did not ask for, and that is a
//! different accusation from a coordinate that disagreed before it ran.
//!
//! The variants are in the order the projection evaluates them.

/// One named clause of the OpenBatch conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenBatchClauseV3 {
    /// The request's expected revision is not the live root's own.
    RootRevision,
    /// `SELECTION_PRODUCT` carries no Product.
    ProductIdentity,
    /// The environment carries no General config.
    EnvironmentGeneralConfigId,
    /// The capability root is not `Active`.
    RootLifecycle,
    /// The root names another Market.
    RootMarket,
    /// The root names another General config.
    RootConfigId,
    /// The root names another generation.
    RootGeneration,
    /// `ROOT_REVISION_OBSERVATION` is not the root's own revision.
    ScalarRootRevisionObservation,
    /// `ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION` is not the root's own sequence.
    ScalarRootNextBatchSequence,
    /// `ROOT_OPEN_BATCHES_OBSERVATION` is not the root's own open count.
    ScalarRootOpenBatches,
    /// `ROOT_EXPECTED_REVISION` is not the revision the request declared.
    ScalarRootExpectedRevision,
    /// `OUTCOME_COUNT` is not the executing width.
    ScalarOutcomeCount,
    /// `ZERO` is not the executing width.
    ScalarZeroOutcomeCount,
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `CONFIG_COLLECTION_SLOTS` is not the config's own collection window.
    ScalarConfigCollectionSlots,
    /// `CONFIG_SELECTION_SLOTS` is not the config's own selection window.
    ScalarConfigSelectionSlots,
    /// `CONFIG_SETTLEMENT_SLOTS` is not the config's own settlement window.
    ScalarConfigSettlementSlots,
    /// `CONFIG_MAX_ORDERS` is not the config's own per-candidate order bound.
    ScalarConfigMaxOrders,
    /// `SELECTION_PRICE_SCALE` is not the config's own price scale.
    ScalarConfigPriceScale,
    /// `GENERATION` is not the config's own generation.
    ScalarConfigGeneration,
    /// `MARKET` is not the root's own Market.
    IdentityMarket,
    /// `GENERAL_CONFIG_ID` is not the root's own config.
    IdentityGeneralConfigId,
    /// `STATE_BUMP` is not the canonical bump for the batch address.
    ScalarStateBump,
    /// `PRIMARY_OWNER` is not the Trading program.
    IdentityPrimaryOwner,
    /// The batch state carries no rent principal.
    ScalarPrimaryRentPrincipal,
    /// The opened batch is not the batch the request named.
    RequestSubject,
    /// The opened batch is not `Collecting`.
    BatchStatus,
}

impl OpenBatchClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a twenty-eighth clause does not compile until
    /// its author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RootRevision => "open-batch: the request names another root revision",
            Self::ProductIdentity => "open-batch: SELECTION_PRODUCT carries no Product",
            Self::EnvironmentGeneralConfigId => "open-batch: the environment has no General config",
            Self::RootLifecycle => "open-batch: the capability root is not Active",
            Self::RootMarket => "open-batch: the root names another Market",
            Self::RootConfigId => "open-batch: the root names another config",
            Self::RootGeneration => "open-batch: the root names another generation",
            Self::ScalarRootRevisionObservation => {
                "open-batch: the observed root revision disagrees"
            }
            Self::ScalarRootNextBatchSequence => {
                "open-batch: the observed batch sequence disagrees"
            }
            Self::ScalarRootOpenBatches => "open-batch: the observed open-batch count disagrees",
            Self::ScalarRootExpectedRevision => {
                "open-batch: the observed expected revision disagrees"
            }
            Self::ScalarOutcomeCount => "open-batch: OUTCOME_COUNT is not the width",
            Self::ScalarZeroOutcomeCount => "open-batch: ZERO is not the width",
            Self::ScalarRootLifecycle => "open-batch: the observed root is not Active",
            Self::ScalarConfigCollectionSlots => {
                "open-batch: the observed collection window disagrees"
            }
            Self::ScalarConfigSelectionSlots => {
                "open-batch: the observed selection window disagrees"
            }
            Self::ScalarConfigSettlementSlots => {
                "open-batch: the observed settlement window disagrees"
            }
            Self::ScalarConfigMaxOrders => "open-batch: the observed order bound disagrees",
            Self::ScalarConfigPriceScale => "open-batch: the observed price scale disagrees",
            Self::ScalarConfigGeneration => "open-batch: the observed generation disagrees",
            Self::IdentityMarket => "open-batch: MARKET is not the root Market",
            Self::IdentityGeneralConfigId => "open-batch: GENERAL_CONFIG_ID is not the root config",
            Self::ScalarStateBump => "open-batch: the witnessed bump is not canonical",
            Self::IdentityPrimaryOwner => "open-batch: the batch state owner is not Trading",
            Self::ScalarPrimaryRentPrincipal => {
                "open-batch: the batch state carries no rent principal"
            }
            Self::RequestSubject => "open-batch: the opened batch is not the requested one",
            Self::BatchStatus => "open-batch: the opened batch is not collecting",
        }
    }
}
