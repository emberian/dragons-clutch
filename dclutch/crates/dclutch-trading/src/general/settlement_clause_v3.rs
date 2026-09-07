//! Which conjunct of General's settlement-side projections disagreed.
//!
//! Six functions share this enum because they share a caller and a reader: the
//! bank environment reader every action runs first, InitializeSettlement, the
//! Consider/Freeze selection applier, the settlement environment validator, its
//! custody shape check, and the position geometry. Between them they joined
//! about seventy-seven accusations and published `InvalidCoordinate` for all of
//! them -- plus two that published `InvalidPlan`, "a record did not decode",
//! for conditions where every record decoded perfectly and the ACTION was
//! simply not one this geometry serves.
//!
//! WHY ONE ENUM AND NOT SIX. These are not six independent conjuncts; they are
//! one pipeline. `general_hot_environment_from_bank_v3` builds the environment,
//! `validate_environment` decides whether it is coherent, `custody`/`position`
//! decide whether the movement it describes is representable, and Initialize is
//! the same environment read one lifecycle step earlier. A refusal from any of
//! them is a settlement refusal, and a reader bisecting one wants the whole
//! sequence in one order. The GROUP PREFIX on each variant says which stage
//! spoke: `Bank`, `Initialize`, `Selection`, `Environment`, `Custody`,
//! `Position`.
//!
//! THE TWO NONZERO LOOPS ARE NOT ONE CLAUSE EACH. `validate_environment`
//! walked twelve identities and Initialize walked eleven, refusing with one
//! code for whichever was zero -- so "a required identity is missing" named
//! twenty-three different missing accounts. Each identity carries its own
//! variant now, because "the realm is absent" and "the mint is absent" are
//! answered by different people.
//!
//! Same shape as [`crate::general::submit_candidate_clause_v3`]: variants in
//! evaluation order within each group, with the sentence a reader sees beside
//! the variant.

/// One named clause of General's settlement-side conjuncts.
///
/// Within each group the order is the evaluation order. A refusal names the
/// FIRST clause that disagreed, so an environment failing several reports the
/// earliest -- the same short-circuit the `||` chains had, with a word for
/// where they stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementClauseV3 {
    /// `SETTLEMENT_POSITION_PRESENT` is neither zero nor one.
    BankPositionPresent,
    /// `PAGE_INDEX` does not fit the page ordinal it feeds.
    BankPageIndex,
    /// `EXECUTION_INDEX` does not fit the row ordinal it feeds.
    BankExecutionIndex,
    /// `TRANSFER_INDEX` does not fit the Custody transfer ordinal it feeds.
    BankTransferIndex,
    /// The settlement cursor was written at another width.
    InitializeCursorOutcomeCount,
    /// The settlement cursor is not `Collecting`.
    InitializeCursorPhase,
    /// The settlement cursor is not at its first revision.
    InitializeCursorRevision,
    /// The environment carries no generation.
    InitializeGeneration,
    /// Initialization was offered at a nonzero page ordinal.
    InitializePageIndex,
    /// Initialization was offered at a nonzero row ordinal.
    InitializeExecutionIndex,
    /// The Custody replay has already advanced.
    InitializeCustodyRevision,
    /// The Custody replay account carries no rent principal.
    InitializeCustodyReplayRent,
    /// The Custody vault carries no rent principal.
    InitializeCustodyVaultRent,
    /// The Claims market revision cannot advance.
    InitializeClaimsMarketRevision,
    /// A settlement position already exists.
    InitializeSettlementPositionPresent,
    /// The settlement position revision is not zero.
    InitializeSettlementPositionRevision,
    /// The settlement position names no owner.
    InitializeSettlementPositionOwner,
    /// The environment carries no rent-credit wallet.
    InitializeRentCredit,
    /// The environment carries no Rent program.
    InitializeRentProgram,
    /// The settlement position carries no rent principal.
    InitializePositionRentPrincipal,
    /// The admission account carries no rent principal.
    InitializeAdmissionRentPrincipal,
    /// The observed position balance is under its own rent principal.
    InitializePositionLamports,
    /// The observed admission balance is under its own rent principal.
    InitializeAdmissionLamports,
    /// `OUTCOME_COUNT` is not the executing width.
    InitializeScalarOutcomeCount,
    /// `PARENT_REQUEST_DIGEST` is not the environment's own request digest.
    InitializeRequestDigest,
    /// The environment carries no General root.
    InitializeGeneralRoot,
    /// The environment carries no parent request digest.
    InitializeParentRequestDigest,
    /// The environment carries no release set.
    InitializeReleaseSet,
    /// The environment carries no Market.
    InitializeMarket,
    /// The environment carries no realm.
    InitializeRealm,
    /// The environment carries no Trading program.
    InitializeTradingProgram,
    /// The environment carries no Custody destination.
    InitializeCustodyDestination,
    /// The environment carries no mint.
    InitializeMint,
    /// The environment carries no Token program.
    InitializeTokenProgram,
    /// The environment carries no payer.
    InitializePayer,
    /// The environment carries no rent refund.
    InitializeRentRefund,
    /// The selection projection was handed an action that is not Consider or Freeze.
    ///
    /// Nothing failed to decode here: the caller dispatched a selection
    /// projection for an action that has no selection successor at all.
    SelectionAction,
    /// `OUTCOME_COUNT` is not the executing width.
    EnvironmentScalarOutcomeCount,
    /// `PARENT_REQUEST_DIGEST` is not the environment's own request digest.
    EnvironmentRequestDigest,
    /// The environment carries no General root.
    EnvironmentGeneralRoot,
    /// The environment carries no parent request digest.
    EnvironmentParentRequestDigest,
    /// The environment carries no release set.
    EnvironmentReleaseSet,
    /// The environment carries no Market.
    EnvironmentMarket,
    /// The environment carries no Product record digest.
    EnvironmentProductRecordDigest,
    /// The environment carries no semantic basis.
    EnvironmentSemanticBasisId,
    /// The environment carries no linked basis record digest.
    EnvironmentLinkedBasisRecordDigest,
    /// The environment carries no realm.
    EnvironmentRealm,
    /// The environment carries no Trading program.
    EnvironmentTradingProgram,
    /// The environment carries no settlement position owner.
    EnvironmentSettlementPositionOwner,
    /// The environment carries no rent-credit wallet.
    EnvironmentRentCredit,
    /// The environment carries no Rent program.
    EnvironmentRentProgram,
    /// The Claims market revision cannot advance.
    EnvironmentClaimsMarketRevision,
    /// The owner position revision cannot advance.
    EnvironmentOwnerPositionRevision,
    /// The settlement position revision cannot advance.
    EnvironmentSettlementPositionRevision,
    /// The settlement position carries no rent principal.
    EnvironmentPositionRentPrincipal,
    /// The admission account carries no rent principal.
    EnvironmentAdmissionRentPrincipal,
    /// The Custody replay account carries no rent principal.
    EnvironmentCustodyReplayRent,
    /// The Custody vault carries no rent principal.
    EnvironmentCustodyVaultRent,
    /// The observed position balance is under its own rent principal.
    EnvironmentPositionLamports,
    /// The observed admission balance is under its own rent principal.
    EnvironmentAdmissionLamports,
    /// A settlement step named a payer.
    ///
    /// Settlement creates no account, so a nonzero payer register means the
    /// request was assembled for a different family of action.
    EnvironmentPayer,
    /// A settlement close named no rent refund.
    EnvironmentCloseRentRefund,
    /// A settlement close does not both hold and vacate the settlement position.
    EnvironmentClosePosition,
    /// A settlement step other than close found no settlement position.
    EnvironmentPositionAbsent,
    /// The Custody source is absent.
    CustodySource,
    /// The Custody destination is absent.
    CustodyDestination,
    /// The Custody mint is absent.
    CustodyMint,
    /// The Custody Token program is absent.
    CustodyTokenProgram,
    /// The Custody source and destination are the same account.
    CustodyAlias,
    /// The Custody replay revision cannot advance.
    CustodyRevision,
    /// The Custody source is not the internal-or-external shape this action moves from.
    CustodySourceShape,
    /// The Custody destination is not the internal-or-external shape this action moves to.
    CustodyDestinationShape,
    /// A Collect or Distribute found no settlement position.
    PositionSettlementAbsent,
    /// A Collect or Distribute names the settlement position as its counterparty.
    PositionOwnerAliasesSettlement,
    /// A Materialize found no settlement position.
    PositionMaterializeSettlementAbsent,
    /// A Materialize moves no complete set.
    ///
    /// Nothing failed to decode: a Materialize with no mint and no merge has no
    /// position geometry to project at all.
    PositionMaterializeMoveNone,
    /// A Close asked for position geometry it does not have.
    PositionCloseGeometry,
}

impl SettlementClauseV3 {
    /// The exact line a program writes to the validator log for this clause.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the reader is a
    /// `no_std` program and `sol_log` takes a `&str` with no allocation. The
    /// match is exhaustive, so a seventy-eighth clause does not compile until
    /// its author says what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::BankPositionPresent => {
                "settlement-bank: the position-present column is not a flag"
            }
            Self::BankPageIndex => "settlement-bank: PAGE_INDEX does not fit a page ordinal",
            Self::BankExecutionIndex => {
                "settlement-bank: EXECUTION_INDEX does not fit a row ordinal"
            }
            Self::BankTransferIndex => {
                "settlement-bank: TRANSFER_INDEX does not fit a transfer ordinal"
            }
            Self::InitializeCursorOutcomeCount => {
                "initialize-settlement: the cursor is another width"
            }
            Self::InitializeCursorPhase => "initialize-settlement: the cursor is not collecting",
            Self::InitializeCursorRevision => {
                "initialize-settlement: the cursor is not at its first revision"
            }
            Self::InitializeGeneration => {
                "initialize-settlement: the environment has no generation"
            }
            Self::InitializePageIndex => "initialize-settlement: the page ordinal is not zero",
            Self::InitializeExecutionIndex => "initialize-settlement: the row ordinal is not zero",
            Self::InitializeCustodyRevision => {
                "initialize-settlement: the Custody replay has already advanced"
            }
            Self::InitializeCustodyReplayRent => {
                "initialize-settlement: the Custody replay carries no rent principal"
            }
            Self::InitializeCustodyVaultRent => {
                "initialize-settlement: the Custody vault carries no rent principal"
            }
            Self::InitializeClaimsMarketRevision => {
                "initialize-settlement: the Claims revision cannot advance"
            }
            Self::InitializeSettlementPositionPresent => {
                "initialize-settlement: a settlement position already exists"
            }
            Self::InitializeSettlementPositionRevision => {
                "initialize-settlement: the position revision is not zero"
            }
            Self::InitializeSettlementPositionOwner => {
                "initialize-settlement: the settlement position names no owner"
            }
            Self::InitializeRentCredit => "initialize-settlement: there is no rent-credit wallet",
            Self::InitializeRentProgram => "initialize-settlement: there is no Rent program",
            Self::InitializePositionRentPrincipal => {
                "initialize-settlement: the position carries no rent principal"
            }
            Self::InitializeAdmissionRentPrincipal => {
                "initialize-settlement: the admission carries no rent principal"
            }
            Self::InitializePositionLamports => {
                "initialize-settlement: the position balance is under its rent"
            }
            Self::InitializeAdmissionLamports => {
                "initialize-settlement: the admission balance is under its rent"
            }
            Self::InitializeScalarOutcomeCount => {
                "initialize-settlement: OUTCOME_COUNT is not the width"
            }
            Self::InitializeRequestDigest => {
                "initialize-settlement: PARENT_REQUEST_DIGEST is not the request"
            }
            Self::InitializeGeneralRoot => "initialize-settlement: there is no General root",
            Self::InitializeParentRequestDigest => {
                "initialize-settlement: there is no parent request digest"
            }
            Self::InitializeReleaseSet => "initialize-settlement: there is no release set",
            Self::InitializeMarket => "initialize-settlement: there is no Market",
            Self::InitializeRealm => "initialize-settlement: there is no realm",
            Self::InitializeTradingProgram => "initialize-settlement: there is no Trading program",
            Self::InitializeCustodyDestination => {
                "initialize-settlement: there is no Custody destination"
            }
            Self::InitializeMint => "initialize-settlement: there is no mint",
            Self::InitializeTokenProgram => "initialize-settlement: there is no Token program",
            Self::InitializePayer => "initialize-settlement: there is no payer",
            Self::InitializeRentRefund => "initialize-settlement: there is no rent refund",
            Self::SelectionAction => "selection: this action has no selection successor",
            Self::EnvironmentScalarOutcomeCount => "settlement-env: OUTCOME_COUNT is not the width",
            Self::EnvironmentRequestDigest => {
                "settlement-env: PARENT_REQUEST_DIGEST is not the request"
            }
            Self::EnvironmentGeneralRoot => "settlement-env: there is no General root",
            Self::EnvironmentParentRequestDigest => {
                "settlement-env: there is no parent request digest"
            }
            Self::EnvironmentReleaseSet => "settlement-env: there is no release set",
            Self::EnvironmentMarket => "settlement-env: there is no Market",
            Self::EnvironmentProductRecordDigest => {
                "settlement-env: there is no Product record digest"
            }
            Self::EnvironmentSemanticBasisId => "settlement-env: there is no semantic basis",
            Self::EnvironmentLinkedBasisRecordDigest => {
                "settlement-env: there is no linked basis digest"
            }
            Self::EnvironmentRealm => "settlement-env: there is no realm",
            Self::EnvironmentTradingProgram => "settlement-env: there is no Trading program",
            Self::EnvironmentSettlementPositionOwner => {
                "settlement-env: the settlement position names no owner"
            }
            Self::EnvironmentRentCredit => "settlement-env: there is no rent-credit wallet",
            Self::EnvironmentRentProgram => "settlement-env: there is no Rent program",
            Self::EnvironmentClaimsMarketRevision => {
                "settlement-env: the Claims revision cannot advance"
            }
            Self::EnvironmentOwnerPositionRevision => {
                "settlement-env: the owner position revision cannot advance"
            }
            Self::EnvironmentSettlementPositionRevision => {
                "settlement-env: the settlement position revision cannot advance"
            }
            Self::EnvironmentPositionRentPrincipal => {
                "settlement-env: the position carries no rent principal"
            }
            Self::EnvironmentAdmissionRentPrincipal => {
                "settlement-env: the admission carries no rent principal"
            }
            Self::EnvironmentCustodyReplayRent => {
                "settlement-env: the Custody replay carries no rent principal"
            }
            Self::EnvironmentCustodyVaultRent => {
                "settlement-env: the Custody vault carries no rent principal"
            }
            Self::EnvironmentPositionLamports => {
                "settlement-env: the position balance is under its rent"
            }
            Self::EnvironmentAdmissionLamports => {
                "settlement-env: the admission balance is under its rent"
            }
            Self::EnvironmentPayer => "settlement-env: a settlement step named a payer",
            Self::EnvironmentCloseRentRefund => "settlement-env: the close names no rent refund",
            Self::EnvironmentClosePosition => {
                "settlement-env: the close does not vacate a held position"
            }
            Self::EnvironmentPositionAbsent => "settlement-env: there is no settlement position",
            Self::CustodySource => "settlement-custody: there is no source",
            Self::CustodyDestination => "settlement-custody: there is no destination",
            Self::CustodyMint => "settlement-custody: there is no mint",
            Self::CustodyTokenProgram => "settlement-custody: there is no Token program",
            Self::CustodyAlias => "settlement-custody: the source is the destination",
            Self::CustodyRevision => "settlement-custody: the replay revision cannot advance",
            Self::CustodySourceShape => "settlement-custody: the source is the wrong shape",
            Self::CustodyDestinationShape => {
                "settlement-custody: the destination is the wrong shape"
            }
            Self::PositionSettlementAbsent => {
                "settlement-position: there is no settlement position"
            }
            Self::PositionOwnerAliasesSettlement => {
                "settlement-position: the counterparty is the settlement position"
            }
            Self::PositionMaterializeSettlementAbsent => {
                "settlement-position: a materialize found no settlement position"
            }
            Self::PositionMaterializeMoveNone => {
                "settlement-position: a materialize moves no complete set"
            }
            Self::PositionCloseGeometry => "settlement-position: a close has no position geometry",
        }
    }
}
