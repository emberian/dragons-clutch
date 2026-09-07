//! Which conjunct of the CancelOrder projection disagreed.
//!
//! `project_general_cancel_order_candidate_in_place_v3` joined fifty
//! accusations with `||`, then four more on the refund escrow the batch
//! produced, and published `InvalidCoordinate` for all fifty-four.
//!
//! CANCEL IS THE ROUTE A MAKER REACHES FOR WHEN SOMETHING IS ALREADY WRONG,
//! which makes an undiagnosable refusal here especially expensive: the maker's
//! atoms are in escrow, the remedy differs completely between "you are not this
//! order's maker" and "this batch has stopped collecting", and both arrived as
//! six identical words. [`CancelOrderClauseV3::OwnerIsMaker`] and
//! [`CancelOrderClauseV3::ScalarBatchStatus`] are now separate sentences.
//!
//! The order's own header is a separate author from the batch's opening and
//! from the observation registers, and the log line's wording says which of the
//! three an accusation is against.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: one enum, in
//! evaluation order, with the sentence a reader sees beside the variant.

/// One named clause of the CancelOrder conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOrderClauseV3 {
    /// The request names an order this record is not.
    RequestSubject,
    /// `ORDER` is not the order this record is.
    IdentityOrder,
    /// `OWNER` is not the maker who signed the order.
    OwnerIsMaker,
    /// `OWNER` carries no maker.
    OwnerIdentity,
    /// `SELECTION_BATCH` is not the batch the order joined.
    IdentitySelectionBatch,
    /// `CANDIDATE` is not the batch the order joined.
    IdentityCandidate,
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
    /// `OUTCOME_COUNT` is not the executing width.
    ScalarOutcomeCount,
    /// `ZERO` is not the executing width.
    ScalarZeroOutcomeCount,
    /// `SCRATCH_A` is not the executing width.
    ScalarScratchOutcomeCount,
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `BATCH_STATUS_OBSERVATION` is not the batch's own status.
    ScalarBatchStatus,
    /// `BATCH_ORDER_COUNT_OBSERVATION` is not the batch's own order count.
    ScalarBatchOrderCount,
    /// `BATCH_CANCELLED_COUNT_OBSERVATION` is not the batch's own cancelled count.
    ScalarBatchCancelledCount,
    /// `BATCH_QUOTE_RESERVE_OBSERVATION` is not the batch's own committed quote.
    ScalarBatchQuoteReserve,
    /// `BATCH_COLLECTION_CLOSE_SLOT` is not the batch's own.
    ScalarBatchCollectionClose,
    /// `ORDER_PHASE_OBSERVATION` is not the order's own phase.
    ScalarOrderPhase,
    /// `ORDER_ADMITTED_SLOT_OBSERVATION` is not the order's own admitted slot.
    ScalarOrderAdmittedSlot,
    /// `ORDER_MAX_LOTS` is not the order's own lot bound.
    ScalarOrderMaxLots,
    /// `ORDER_MAX_QUOTE_DEBIT_PER_LOT` is not the order's own price cap.
    ScalarOrderMaxQuoteDebit,
    /// `ORDER_NONCE` is not the order's own nonce.
    ScalarOrderNonce,
    /// The batch was opened at another width.
    BatchOutcomeCount,
    /// The batch belongs to another Market.
    BatchMarket,
    /// The batch names another Product than `SELECTION_PRODUCT`.
    BatchProduct,
    /// The batch belongs to another config.
    BatchConfigId,
    /// The batch belongs to another generation.
    BatchGeneration,
    /// The batch was opened at another price scale.
    BatchPriceScale,
    /// The batch was opened under another per-candidate order bound.
    BatchMaxOrders,
    /// The order was signed at another width.
    OrderOutcomeCount,
    /// The order names another Market than the root.
    OrderMarket,
    /// The order names another generation than the root.
    OrderGeneration,
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
    /// `TERMINAL_RECORD_BUMP` is not the canonical bump for the order address.
    ScalarTerminalBump,
    /// `TERMINAL_OWNER` is not the Trading program.
    IdentityTerminalOwner,
    /// The order state carries no rent principal.
    ScalarTerminalRentPrincipal,
    /// The custody source vault is not keyed on this order.
    EnvironmentSourceVault,
    /// The custody destination is not owned by the maker.
    EnvironmentCustodyDestinationOwner,
    /// `POSITION_ZERO_OWNER` is not the order.
    IdentityPositionZeroOwner,
    /// `POSITION_ONE_OWNER` is not the maker.
    IdentityPositionOneOwner,
    /// The settlement position is not keyed on this order.
    EnvironmentSettlementPositionOwner,
    /// The rent credit does not name the maker.
    EnvironmentRentCredit,
    /// The rent refund does not name the maker.
    EnvironmentRentRefund,
    /// The refunded escrow names another order.
    EscrowOrder,
    /// The refunded escrow names another maker.
    EscrowOwner,
    /// The refunded escrow was computed at another width.
    EscrowOutcomeCount,
    /// The refunded escrow is not a refund.
    EscrowDirection,
}

impl CancelOrderClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a fifty-fifth clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RequestSubject => "cancel-order: the request names another order",
            Self::IdentityOrder => "cancel-order: ORDER is not this order",
            Self::OwnerIsMaker => "cancel-order: OWNER is not the order maker",
            Self::OwnerIdentity => "cancel-order: OWNER carries no maker",
            Self::IdentitySelectionBatch => "cancel-order: SELECTION_BATCH is not the order batch",
            Self::IdentityCandidate => "cancel-order: CANDIDATE is not the order batch",
            Self::EnvironmentGeneralConfigId => {
                "cancel-order: the environment has no General config"
            }
            Self::RootLifecycle => "cancel-order: the capability root is not Active",
            Self::RootMarket => "cancel-order: the root names another Market",
            Self::RootConfigId => "cancel-order: the root names another config",
            Self::RootGeneration => "cancel-order: the root names another generation",
            Self::ScalarOutcomeCount => "cancel-order: OUTCOME_COUNT is not the width",
            Self::ScalarZeroOutcomeCount => "cancel-order: ZERO is not the width",
            Self::ScalarScratchOutcomeCount => "cancel-order: SCRATCH_A is not the width",
            Self::ScalarRootLifecycle => "cancel-order: the observed root is not Active",
            Self::ScalarBatchStatus => "cancel-order: the observed batch status disagrees",
            Self::ScalarBatchOrderCount => "cancel-order: the observed order count disagrees",
            Self::ScalarBatchCancelledCount => {
                "cancel-order: the observed cancelled count disagrees"
            }
            Self::ScalarBatchQuoteReserve => "cancel-order: the observed quote reserve disagrees",
            Self::ScalarBatchCollectionClose => {
                "cancel-order: the observed collection close disagrees"
            }
            Self::ScalarOrderPhase => "cancel-order: the observed order phase disagrees",
            Self::ScalarOrderAdmittedSlot => "cancel-order: the observed admitted slot disagrees",
            Self::ScalarOrderMaxLots => "cancel-order: the observed lot bound disagrees",
            Self::ScalarOrderMaxQuoteDebit => "cancel-order: the observed price cap disagrees",
            Self::ScalarOrderNonce => "cancel-order: the observed nonce disagrees",
            Self::BatchOutcomeCount => "cancel-order: the batch is another width",
            Self::BatchMarket => "cancel-order: the batch belongs to another Market",
            Self::BatchProduct => "cancel-order: the batch names another Product",
            Self::BatchConfigId => "cancel-order: the batch belongs to another config",
            Self::BatchGeneration => "cancel-order: the batch belongs to another generation",
            Self::BatchPriceScale => "cancel-order: the batch is another price scale",
            Self::BatchMaxOrders => "cancel-order: the batch is another order bound",
            Self::OrderOutcomeCount => "cancel-order: the order is another width",
            Self::OrderMarket => "cancel-order: the order names another Market",
            Self::OrderGeneration => "cancel-order: the order names another generation",
            Self::IdentityMarket => "cancel-order: MARKET is not the root Market",
            Self::IdentityGeneralConfigId => {
                "cancel-order: GENERAL_CONFIG_ID is not the root config"
            }
            Self::ScalarStateBump => "cancel-order: the witnessed batch bump is not canonical",
            Self::IdentityPrimaryOwner => "cancel-order: the batch state owner is not Trading",
            Self::ScalarPrimaryRentPrincipal => {
                "cancel-order: the batch state carries no rent principal"
            }
            Self::ScalarTerminalBump => "cancel-order: the witnessed order bump is not canonical",
            Self::IdentityTerminalOwner => "cancel-order: the order state owner is not Trading",
            Self::ScalarTerminalRentPrincipal => {
                "cancel-order: the order state carries no rent principal"
            }
            Self::EnvironmentSourceVault => {
                "cancel-order: the escrow vault is not keyed on this order"
            }
            Self::EnvironmentCustodyDestinationOwner => {
                "cancel-order: the refund destination is not the maker"
            }
            Self::IdentityPositionZeroOwner => "cancel-order: position zero is not the order",
            Self::IdentityPositionOneOwner => "cancel-order: position one is not the maker",
            Self::EnvironmentSettlementPositionOwner => {
                "cancel-order: the settlement position is not keyed on this order"
            }
            Self::EnvironmentRentCredit => "cancel-order: the rent credit is not the maker",
            Self::EnvironmentRentRefund => "cancel-order: the rent refund is not the maker",
            Self::EscrowOrder => "cancel-order: the refund names another order",
            Self::EscrowOwner => "cancel-order: the refund names another maker",
            Self::EscrowOutcomeCount => "cancel-order: the refund is another width",
            Self::EscrowDirection => "cancel-order: the escrow movement is not a refund",
        }
    }
}
