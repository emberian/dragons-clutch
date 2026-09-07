//! Which conjunct of the CloseBatch projection disagreed.
//!
//! `project_general_close_batch_candidate_in_place_v3` joined THIRTY-ONE
//! accusations with `||` and published one word for all of them:
//! `InvalidCoordinate`. Among those thirty-one is the one refusal a
//! permissionless cranker sees constantly and honestly -- the collection window
//! has not elapsed and the batch is not full, so this close is early -- and it
//! was indistinguishable from a substituted Product or a root belonging to
//! another Market. A cranker that cannot tell "come back in forty slots" from
//! "your inputs are wrong" retries the wrong thing forever.
//!
//! [`CloseBatchClauseV3::CloseWindow`] is that clause, and it is LAST because
//! the projection reaches it last: everything a close needs to be about must
//! agree before the window question is even asked.
//!
//! THE FIRST CLAUSE IS THE REVISION. CloseBatch compared `expected_revision`
//! against both the live root and the bank's `ROOT_EXPECTED_REVISION` register,
//! from the middle of the chain. The live root's is the one a racing cranker
//! loses on, so it is asked first, by name, and the register comparison keeps
//! its own name behind it.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: one enum, in
//! evaluation order, with the sentence a reader sees beside the variant.

/// One named clause of the CloseBatch conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseBatchClauseV3 {
    /// The request's expected revision is not the live root's own.
    RootRevision,
    /// The request names a batch this record is not.
    RequestSubject,
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
    /// `BATCH_STATUS_OBSERVATION` is not the batch's own status.
    ScalarBatchStatus,
    /// `BATCH_ORDER_COUNT_OBSERVATION` is not the batch's own order count.
    ScalarBatchOrderCount,
    /// `BATCH_COLLECTION_CLOSE_SLOT` is not the batch's own.
    ScalarBatchCollectionClose,
    /// `CONFIG_MAX_ORDERS` is not the batch's own immutable order bound.
    ScalarConfigMaxOrders,
    /// The batch was opened at another width.
    BatchOutcomeCount,
    /// The batch belongs to another Market.
    BatchMarket,
    /// The batch names another Product.
    BatchProduct,
    /// The batch belongs to another config.
    BatchConfigId,
    /// The batch belongs to another generation.
    BatchGeneration,
    /// The batch was opened at another price scale.
    BatchPriceScale,
    /// The batch was opened under another per-candidate order bound.
    BatchMaxOrders,
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
    /// The batch is neither full nor past its collection window.
    ///
    /// The one clause a permissionless cranker is supposed to see, and the one
    /// whose remedy is to wait rather than to fix an input.
    CloseWindow,
}

impl CloseBatchClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a thirty-second clause does not compile until
    /// its author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RootRevision => "close-batch: the request names another root revision",
            Self::RequestSubject => "close-batch: the request names another batch",
            Self::ProductIdentity => "close-batch: SELECTION_PRODUCT carries no Product",
            Self::EnvironmentGeneralConfigId => {
                "close-batch: the environment has no General config"
            }
            Self::RootLifecycle => "close-batch: the capability root is not Active",
            Self::RootMarket => "close-batch: the root names another Market",
            Self::RootConfigId => "close-batch: the root names another config",
            Self::RootGeneration => "close-batch: the root names another generation",
            Self::ScalarRootRevisionObservation => {
                "close-batch: the observed root revision disagrees"
            }
            Self::ScalarRootOpenBatches => "close-batch: the observed open-batch count disagrees",
            Self::ScalarRootExpectedRevision => {
                "close-batch: the observed expected revision disagrees"
            }
            Self::ScalarOutcomeCount => "close-batch: OUTCOME_COUNT is not the width",
            Self::ScalarZeroOutcomeCount => "close-batch: ZERO is not the width",
            Self::ScalarRootLifecycle => "close-batch: the observed root is not Active",
            Self::ScalarBatchStatus => "close-batch: the observed batch status disagrees",
            Self::ScalarBatchOrderCount => "close-batch: the observed order count disagrees",
            Self::ScalarBatchCollectionClose => {
                "close-batch: the observed collection close disagrees"
            }
            Self::ScalarConfigMaxOrders => "close-batch: the observed order bound disagrees",
            Self::BatchOutcomeCount => "close-batch: the batch is another width",
            Self::BatchMarket => "close-batch: the batch belongs to another Market",
            Self::BatchProduct => "close-batch: the batch names another Product",
            Self::BatchConfigId => "close-batch: the batch belongs to another config",
            Self::BatchGeneration => "close-batch: the batch belongs to another generation",
            Self::BatchPriceScale => "close-batch: the batch is another price scale",
            Self::BatchMaxOrders => "close-batch: the batch is another order bound",
            Self::IdentityMarket => "close-batch: MARKET is not the root Market",
            Self::IdentityGeneralConfigId => {
                "close-batch: GENERAL_CONFIG_ID is not the root config"
            }
            Self::ScalarStateBump => "close-batch: the witnessed bump is not canonical",
            Self::IdentityPrimaryOwner => "close-batch: the batch state owner is not Trading",
            Self::ScalarPrimaryRentPrincipal => {
                "close-batch: the batch state carries no rent principal"
            }
            Self::CloseWindow => "close-batch: the batch is neither full nor past its window",
        }
    }
}
