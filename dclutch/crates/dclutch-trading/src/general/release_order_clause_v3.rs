//! Which conjunct of the ReleaseOrder projection disagreed.
//!
//! `project_general_release_order_candidate_in_place_v3` joined forty-one
//! accusations with `||` and published `InvalidCoordinate` for every one.
//!
//! RELEASE IS PERMISSIONLESS, which is what makes the coarse code worst here.
//! Anybody may crank a residual release after an order's validity horizon, so
//! the caller who reads the log is usually NOT the party who built the inputs
//! -- they are a keeper holding somebody else's order and a refusal with no
//! subject. [`ReleaseOrderClauseV3::ObservedQuoteResidual`] in particular says
//! the physical escrow holds less than the order reserved, which is a fact
//! about the chain rather than about the request, and a keeper that cannot
//! distinguish it from a substituted register retries a transaction that can
//! never succeed.
//!
//! The residual escrow itself contributes five clauses: this route asserts the
//! movement is a `Residual` carrying zero quote atoms, and a violation of that
//! is a conservation accusation, not a coordinate typo.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: one enum, in
//! evaluation order, with the sentence a reader sees beside the variant.

/// One named clause of the ReleaseOrder conjunct.
///
/// The order is the evaluation order. A refusal names the FIRST clause that
/// disagreed, so a bank failing several reports the earliest -- the same
/// short-circuit the `||` chain had, with a word for where it stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseOrderClauseV3 {
    /// The request names an order this record is not.
    RequestSubject,
    /// `ORDER` is not the order this record is.
    IdentityOrder,
    /// `OWNER` is not the maker who signed the order.
    OwnerIsMaker,
    /// `OWNER` carries no maker.
    OwnerIdentity,
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
    /// `ROOT_LIFECYCLE_OBSERVATION` is not `Active`.
    ScalarRootLifecycle,
    /// `ORDER_PHASE_OBSERVATION` is not the order's own phase.
    ScalarOrderPhase,
    /// `ORDER_ADMITTED_SLOT_OBSERVATION` is not the order's own admitted slot.
    ScalarOrderAdmittedSlot,
    /// `ORDER_VALID_UNTIL_SLOT` is not the order's own validity horizon.
    ScalarOrderValidUntil,
    /// `ORDER_MAX_LOTS` is not the order's own lot bound.
    ScalarOrderMaxLots,
    /// `ORDER_MAX_QUOTE_DEBIT_PER_LOT` is not the order's own price cap.
    ScalarOrderMaxQuoteDebit,
    /// `ORDER_NONCE` is not the order's own nonce.
    ScalarOrderNonce,
    /// The observed escrow balance exceeds the order's own quote reserve.
    ///
    /// A residual is an OBSERVATION of what physical escrow still holds, never
    /// a promise recomputed here. More than the reserve means the account being
    /// released is not the escrow this order funded.
    ObservedQuoteResidual,
    /// The order was signed at another width.
    OrderOutcomeCount,
    /// The order names another Market than the root.
    OrderMarket,
    /// The order names another generation than the root.
    OrderGeneration,
    /// The config names another generation than the root.
    ConfigGeneration,
    /// `MARKET` is not the root's own Market.
    IdentityMarket,
    /// `GENERATION` is not the root's own generation.
    ScalarGeneration,
    /// `STATE_BUMP` is not the canonical bump for the order address.
    ScalarStateBump,
    /// `PRIMARY_OWNER` is not the Trading program.
    IdentityPrimaryOwner,
    /// The order state carries no rent principal.
    ScalarPrimaryRentPrincipal,
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
    /// The released escrow names another order.
    EscrowOrder,
    /// The released escrow names another maker.
    EscrowOwner,
    /// The released escrow was computed at another width.
    EscrowOutcomeCount,
    /// The released escrow states a nonzero promised quote movement.
    EscrowQuoteAtoms,
    /// The released escrow is not a residual.
    EscrowDirection,
}

impl ReleaseOrderClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a forty-second clause does not compile until its
    /// author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::RequestSubject => "release-order: the request names another order",
            Self::IdentityOrder => "release-order: ORDER is not this order",
            Self::OwnerIsMaker => "release-order: OWNER is not the order maker",
            Self::OwnerIdentity => "release-order: OWNER carries no maker",
            Self::IdentityCandidate => "release-order: CANDIDATE is not the order batch",
            Self::EnvironmentGeneralConfigId => {
                "release-order: the environment has no General config"
            }
            Self::RootLifecycle => "release-order: the capability root is not Active",
            Self::RootMarket => "release-order: the root names another Market",
            Self::RootConfigId => "release-order: the root names another config",
            Self::RootGeneration => "release-order: the root names another generation",
            Self::ScalarOutcomeCount => "release-order: OUTCOME_COUNT is not the width",
            Self::ScalarZeroOutcomeCount => "release-order: ZERO is not the width",
            Self::ScalarRootLifecycle => "release-order: the observed root is not Active",
            Self::ScalarOrderPhase => "release-order: the observed order phase disagrees",
            Self::ScalarOrderAdmittedSlot => "release-order: the observed admitted slot disagrees",
            Self::ScalarOrderValidUntil => "release-order: the observed horizon disagrees",
            Self::ScalarOrderMaxLots => "release-order: the observed lot bound disagrees",
            Self::ScalarOrderMaxQuoteDebit => "release-order: the observed price cap disagrees",
            Self::ScalarOrderNonce => "release-order: the observed nonce disagrees",
            Self::ObservedQuoteResidual => {
                "release-order: the observed escrow exceeds the order reserve"
            }
            Self::OrderOutcomeCount => "release-order: the order is another width",
            Self::OrderMarket => "release-order: the order names another Market",
            Self::OrderGeneration => "release-order: the order names another generation",
            Self::ConfigGeneration => "release-order: the config names another generation",
            Self::IdentityMarket => "release-order: MARKET is not the root Market",
            Self::ScalarGeneration => "release-order: GENERATION is not the root generation",
            Self::ScalarStateBump => "release-order: the witnessed bump is not canonical",
            Self::IdentityPrimaryOwner => "release-order: the order state owner is not Trading",
            Self::ScalarPrimaryRentPrincipal => {
                "release-order: the order state carries no rent principal"
            }
            Self::EnvironmentSourceVault => {
                "release-order: the escrow vault is not keyed on this order"
            }
            Self::EnvironmentCustodyDestinationOwner => {
                "release-order: the refund destination is not the maker"
            }
            Self::IdentityPositionZeroOwner => "release-order: position zero is not the order",
            Self::IdentityPositionOneOwner => "release-order: position one is not the maker",
            Self::EnvironmentSettlementPositionOwner => {
                "release-order: the settlement position is not keyed on this order"
            }
            Self::EnvironmentRentCredit => "release-order: the rent credit is not the maker",
            Self::EnvironmentRentRefund => "release-order: the rent refund is not the maker",
            Self::EscrowOrder => "release-order: the residual names another order",
            Self::EscrowOwner => "release-order: the residual names another maker",
            Self::EscrowOutcomeCount => "release-order: the residual is another width",
            Self::EscrowQuoteAtoms => "release-order: the residual promises a quote movement",
            Self::EscrowDirection => "release-order: the escrow movement is not a residual",
        }
    }
}
