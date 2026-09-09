//! Which conjunct of the PlaceOrder projection disagreed.
//!
//! `project_general_place_order_candidate_in_place_v3` is the widest conjunct
//! in the family: fifty-three `||`-joined clauses, then a per-outcome row join
//! over the signed terms, then five postconditions on the escrow the batch
//! produced. Every one of them published `InvalidCoordinate`.
//!
//! THAT ONE WORD SPANS THREE DIFFERENT AUTHORS. Some of these clauses accuse
//! the MAKER's signed terms (a nonce, a cap, a validity horizon that does not
//! reach the settlement close); some accuse the PROJECTION the AccountProfile
//! wrote (an observation register that does not carry what the record it
//! observed says); and some accuse the ESCROW GEOMETRY the accelerator will
//! move atoms through (a vault context naming the wrong order, a position row
//! owned by the wrong party). A maker debugging a rejected order, a profile
//! author debugging a register, and a custody reviewer debugging an escrow all
//! read the same six words today. The prefix on every line below says which of
//! the three is being accused, and the variant says which coordinate.
//!
//! THE ROW JOIN IS TWO CLAUSES, NOT ONE. The per-outcome loop compares both
//! halves of a lot -- what the maker receives and what they deliver -- and
//! refused with one code for either. Those are opposite sides of the trade and
//! opposite mistakes; they get separate names.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: one enum, in
//! evaluation order, with the sentence a reader sees beside the variant.

/// One named clause of the PlaceOrder conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceOrderClauseV3 {
    /// The request names an order the signed terms do not.
    RequestSubject,
    /// `ORDER` is not the order the signed terms name.
    IdentityOrder,
    /// The signed terms were authored at another width.
    TermsOutcomeCount,
    /// `ORDER_NONCE` is not the signed nonce.
    TermsNonce,
    /// The signed terms name another maker than `OWNER`.
    TermsOwner,
    /// The signed terms name another Market than the root.
    TermsMarket,
    /// The signed terms name another batch than `SELECTION_BATCH`.
    TermsBatch,
    /// The signed terms name another generation than `GENERATION`.
    TermsGeneration,
    /// `ORDER_MAX_LOTS` is not the signed lot bound.
    TermsMaxLots,
    /// `ORDER_MAX_QUOTE_DEBIT_PER_LOT` is not the signed price cap.
    TermsMaxQuoteDebit,
    /// `ORDER_MIN_QUOTE_CREDIT_PER_LOT` differs from the authenticated signed terms.
    TermsMinQuoteCredit,
    /// `ORDER_SIDE` differs from the authenticated signed terms.
    TermsSide,
    /// `ORDER_OUTCOME_LO` differs from the authenticated signed terms.
    TermsOutcomeLo,
    /// `ORDER_OUTCOME_HI` differs from the authenticated signed terms.
    TermsOutcomeHi,
    /// `ORDER_CLAIMS_PER_LOT` differs from the authenticated signed terms.
    TermsClaimsPerLot,
    /// `ORDER_VALID_UNTIL_SLOT` is not the signed validity horizon.
    TermsValidUntil,
    /// `OWNER` carries no maker.
    OwnerIdentity,
    /// `SELECTION_PRODUCT` carries no Product.
    ProductIdentity,
    /// `SELECTION_BATCH` is not the batch this record is.
    BatchSubject,
    /// `CANDIDATE` is not the batch the order joins.
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
    /// `BATCH_QUOTE_RESERVE_OBSERVATION` is not the batch's own committed quote.
    ScalarBatchQuoteReserve,
    /// `BATCH_COLLECTION_CLOSE_SLOT` is not the batch's own.
    ScalarBatchCollectionClose,
    /// `BATCH_SETTLEMENT_CLOSE_SLOT` is not the batch's own.
    ScalarBatchSettlementClose,
    /// `CONFIG_MAX_ORDERS` is not the batch's own immutable order bound.
    ScalarConfigMaxOrders,
    /// The signed validity horizon is not the batch's settlement close.
    ///
    /// The order must outlive exactly the window it may be settled in: shorter
    /// and its escrow strands, longer and it survives the batch that bounded it.
    TermsSettlementHorizon,
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
    /// The lifecycle payer is not the maker authenticated by the signed terms.
    IdentityPayer,
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
    /// The custody destination vault is not keyed on this order.
    EnvironmentDestinationVault,
    /// The custody source is not owned by the maker.
    EnvironmentCustodySourceOwner,
    /// `POSITION_ZERO_OWNER` is not the maker.
    IdentityPositionZeroOwner,
    /// `POSITION_ONE_OWNER` is not the order.
    IdentityPositionOneOwner,
    /// The settlement position is not keyed on this order.
    EnvironmentSettlementPositionOwner,
    /// The authenticated rent-refund wallet does not name the maker.
    EnvironmentRentRefund,
    /// A per-outcome `CURSOR_INVENTORY` is not the signed receive-per-lot.
    ItemReceivePerLot,
    /// A per-outcome `QUANTITY` is not the signed deliver-per-lot.
    ItemDeliverPerLot,
    /// The admitted escrow names another order.
    EscrowOrder,
    /// The admitted escrow names another maker.
    EscrowOwner,
    /// The admitted escrow was computed at another width.
    EscrowOutcomeCount,
    /// The admitted escrow's quote atoms are not the signed reserve.
    EscrowQuoteAtoms,
    /// The admitted escrow is not a deposit.
    EscrowDirection,
}

impl PlaceOrderClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a sixty-first clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RequestSubject => "place-order: the request names another order",
            Self::IdentityOrder => "place-order: ORDER is not the signed order",
            Self::TermsOutcomeCount => "place-order: the signed terms are another width",
            Self::TermsNonce => "place-order: the observed nonce is not the signed one",
            Self::TermsOwner => "place-order: the signed terms name another maker",
            Self::TermsMarket => "place-order: the signed terms name another Market",
            Self::TermsBatch => "place-order: the signed terms name another batch",
            Self::TermsGeneration => "place-order: the signed terms name another generation",
            Self::TermsMaxLots => "place-order: the observed lot bound is not the signed one",
            Self::TermsMaxQuoteDebit => "place-order: the observed price cap is not the signed one",
            Self::TermsMinQuoteCredit => {
                "place-order: ORDER_MIN_QUOTE_CREDIT_PER_LOT is not the signed value"
            }
            Self::TermsSide => "place-order: ORDER_SIDE is not the signed value",
            Self::TermsOutcomeLo => "place-order: ORDER_OUTCOME_LO is not the signed value",
            Self::TermsOutcomeHi => "place-order: ORDER_OUTCOME_HI is not the signed value",
            Self::TermsClaimsPerLot => "place-order: ORDER_CLAIMS_PER_LOT is not the signed value",
            Self::TermsValidUntil => "place-order: the observed horizon is not the signed one",
            Self::OwnerIdentity => "place-order: OWNER carries no maker",
            Self::ProductIdentity => "place-order: SELECTION_PRODUCT carries no Product",
            Self::BatchSubject => "place-order: SELECTION_BATCH is not this batch",
            Self::IdentityCandidate => "place-order: CANDIDATE is not the order batch",
            Self::EnvironmentGeneralConfigId => {
                "place-order: the environment has no General config"
            }
            Self::RootLifecycle => "place-order: the capability root is not Active",
            Self::RootMarket => "place-order: the root names another Market",
            Self::RootConfigId => "place-order: the root names another config",
            Self::RootGeneration => "place-order: the root names another generation",
            Self::ScalarOutcomeCount => "place-order: OUTCOME_COUNT is not the width",
            Self::ScalarZeroOutcomeCount => "place-order: ZERO is not the width",
            Self::ScalarScratchOutcomeCount => "place-order: SCRATCH_A is not the width",
            Self::ScalarRootLifecycle => "place-order: the observed root is not Active",
            Self::ScalarBatchStatus => "place-order: the observed batch status disagrees",
            Self::ScalarBatchOrderCount => "place-order: the observed order count disagrees",
            Self::ScalarBatchQuoteReserve => "place-order: the observed quote reserve disagrees",
            Self::ScalarBatchCollectionClose => {
                "place-order: the observed collection close disagrees"
            }
            Self::ScalarBatchSettlementClose => {
                "place-order: the observed settlement close disagrees"
            }
            Self::ScalarConfigMaxOrders => "place-order: the observed order bound disagrees",
            Self::TermsSettlementHorizon => {
                "place-order: the order does not expire at the settlement close"
            }
            Self::BatchOutcomeCount => "place-order: the batch is another width",
            Self::BatchMarket => "place-order: the batch belongs to another Market",
            Self::BatchProduct => "place-order: the batch names another Product",
            Self::BatchConfigId => "place-order: the batch belongs to another config",
            Self::BatchGeneration => "place-order: the batch belongs to another generation",
            Self::BatchPriceScale => "place-order: the batch is another price scale",
            Self::BatchMaxOrders => "place-order: the batch is another order bound",
            Self::IdentityMarket => "place-order: MARKET is not the root Market",
            Self::IdentityGeneralConfigId => {
                "place-order: GENERAL_CONFIG_ID is not the root config"
            }
            Self::IdentityPayer => "place-order: PAYER is not the signed maker",
            Self::ScalarStateBump => "place-order: the witnessed batch bump is not canonical",
            Self::IdentityPrimaryOwner => "place-order: the batch state owner is not Trading",
            Self::ScalarPrimaryRentPrincipal => {
                "place-order: the batch state carries no rent principal"
            }
            Self::ScalarTerminalBump => "place-order: the witnessed order bump is not canonical",
            Self::IdentityTerminalOwner => "place-order: the order state owner is not Trading",
            Self::ScalarTerminalRentPrincipal => {
                "place-order: the order state carries no rent principal"
            }
            Self::EnvironmentDestinationVault => {
                "place-order: the escrow vault is not keyed on this order"
            }
            Self::EnvironmentCustodySourceOwner => {
                "place-order: the custody source is not the maker"
            }
            Self::IdentityPositionZeroOwner => "place-order: position zero is not the maker",
            Self::IdentityPositionOneOwner => "place-order: position one is not the order",
            Self::EnvironmentSettlementPositionOwner => {
                "place-order: the settlement position is not keyed on this order"
            }
            Self::EnvironmentRentRefund => "place-order: the rent refund is not the maker",
            Self::ItemReceivePerLot => "place-order: an observed receive row is not the signed one",
            Self::ItemDeliverPerLot => "place-order: an observed deliver row is not the signed one",
            Self::EscrowOrder => "place-order: the escrow names another order",
            Self::EscrowOwner => "place-order: the escrow names another maker",
            Self::EscrowOutcomeCount => "place-order: the escrow is another width",
            Self::EscrowQuoteAtoms => "place-order: the escrow is not the signed quote reserve",
            Self::EscrowDirection => "place-order: the escrow is not a deposit",
        }
    }
}
