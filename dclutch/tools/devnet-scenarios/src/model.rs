use serde::{Deserialize, Serialize};

/// Stable manifest envelope schema.
pub const MANIFEST_SCHEMA_V1: &str = "dclutch-devnet-economic-scenario-v1";
/// Implemented manifest envelope version.
pub const MANIFEST_VERSION_V1: u32 = 1;
/// Stable digest scope for [`ManifestEnvelopeV1::body_sha256`].
pub const BODY_DIGEST_SCOPE_V1: &str = "canonical-compact-scenario-body-json-v1";
/// Exact public-devnet identity which the runtime must authenticate again.
pub const DEVNET_GENESIS_HASH_V1: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
/// Exact development fee rate applied independently to each Direct side.
pub const DEVNET_FEE_BASIS_POINTS_V1: u16 = 50;

/// Digest-bearing canonical scenario envelope.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestEnvelopeV1 {
    /// Stable schema name.
    pub schema: String,
    /// Implemented schema version.
    pub version: u32,
    /// Stable scenario identity repeated outside the digested body.
    pub scenario_id: String,
    /// Digest scope used by `bodySha256`.
    pub body_digest_scope: String,
    /// SHA-256 of the compact canonical JSON serialization of `body`.
    pub body_sha256: String,
    /// Complete scenario body.
    pub body: ScenarioBodyV1,
}

/// One deterministic economic journey.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioBodyV1 {
    /// Stable scenario identity.
    pub scenario_id: String,
    /// Reader-facing title.
    pub title: String,
    /// Reader-facing purpose and limits.
    pub description: String,
    /// Cluster class the runtime binder must select.
    pub cluster_target: ClusterTargetV1,
    /// Exact public-devnet genesis identity the runtime must authenticate.
    pub genesis_hash: String,
    /// Honest evidence level of an unbound deterministic fixture.
    pub evidence_level: EvidenceLevelV1,
    /// Market and exact integer-unit policy.
    pub market: MarketSpecV1,
    /// Bounded activity defaults.
    pub limits: ActivityLimitsV1,
    /// Stable logical wallets; no private material or key paths.
    pub wallets: Vec<WalletSpecV1>,
    /// Complete logical account inventory.
    pub accounts: Vec<AccountSpecV1>,
    /// Exact state before the first operation.
    pub initial_snapshot: LedgerSnapshotV1,
    /// Canonically ordered, dependency-linked operations.
    pub operations: Vec<OperationV1>,
    /// Exact state after the final projected operation.
    pub final_snapshot: LedgerSnapshotV1,
    /// Whether the projected terminal state admits the retirement operation.
    pub retire_eligible: bool,
}

/// Runtime cluster class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClusterTargetV1 {
    /// A disposable validator owned by the operator.
    OwnedLoopback,
    /// Solana public devnet.
    Devnet,
}

/// Evidence level of a scenario document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceLevelV1 {
    /// Deterministic projection with no claim that a transaction executed.
    ScenarioOnly,
    /// Runtime-bound captured evidence. Canonical fixtures never use this.
    CapturedExecution,
}

/// Logical market facts needed by both activity and reconciliation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarketSpecV1 {
    /// Stable profile name.
    pub profile: MarketProfileV1,
    /// Logical Market account reference.
    pub market_ref: String,
    /// Runtime-supplied market-input artifact path, absent in scenario fixtures.
    pub input_artifact: Option<String>,
    /// Exact four-outcome width.
    pub outcome_count: u32,
    /// Logical Realm-selected collateral Mint reference.
    pub collateral_mint_ref: String,
    /// Ordered claim Mint references.
    pub claim_mint_refs: Vec<String>,
    /// Exact Product payout partition.
    pub resolution: ResolutionSpecV1,
    /// Direct price scale in collateral atoms.
    pub price_scale_atoms: String,
    /// Canonical Direct basis-point denominator.
    pub fee_denominator: String,
    /// Development policy rate applied independently to buyer and seller.
    pub fee_basis_points_per_side: u16,
    /// Logical fee recipient collateral-token account.
    pub fee_recipient_account_ref: String,
    /// Logical Hoard-principal collateral-token account.
    pub hoard_principal_account_ref: String,
}

/// Stable market profiles in the public demo charter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarketProfileV1 {
    /// SOL/USD four-outcome flagship.
    Flagship,
    /// Mainnet graduation observation relayed to devnet.
    Graduation,
    /// Deliberately silent relayer and funded failure walk.
    Abandoned,
}

/// Product terminal partition selected by the scenario.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResolutionSpecV1 {
    /// Unit-scale categorical selector.
    Categorical {
        /// Winning outcome coordinate.
        selector: u32,
        /// Ordered payout per claim atom; exactly one entry is one.
        payout_atoms_per_claim: Vec<String>,
    },
    /// Graded exact-complement success at one rational coordinate.
    GradedSuccess {
        /// Signed exact result numerator.
        result_numerator: String,
        /// Positive exact result denominator.
        result_denominator: String,
        /// Positive Product payout scale.
        payout_scale: String,
        /// Ordered exact payout partition.
        payout_atoms_per_claim: Vec<String>,
    },
    /// Graded explicit resolution-failure partition.
    GradedFailure {
        /// Positive Product payout scale.
        payout_scale: String,
        /// Ordered exact failure payout partition.
        payout_atoms_per_claim: Vec<String>,
    },
}

/// Bounded activity defaults, all provisional operator limits.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityLimitsV1 {
    /// Maximum simultaneously dispatched operations.
    pub max_concurrency: u32,
    /// Minimum wall-clock spacing between mutation attempts.
    pub min_dispatch_interval_ms: u64,
    /// Hard transaction-attempt ceiling.
    pub max_transactions: u32,
    /// Poll interval for durable resume.
    pub poll_interval_ms: u64,
    /// Hard poll ceiling.
    pub max_polls: u32,
}

/// Secret-free logical wallet declaration.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WalletSpecV1 {
    /// Stable wallet reference.
    pub id: String,
    /// Scenario roles such as seller, buyer, redeemer, or fee-payer.
    pub roles: Vec<String>,
    /// Exact requested runtime lamport funding, not collateral.
    pub funding_lamports: String,
    /// Logical collateral-token account controlled by this wallet.
    pub collateral_account_ref: String,
    /// Ordered logical claim-token accounts controlled by this wallet.
    pub claim_account_refs: Vec<String>,
    /// Logical Position account for this wallet and Market.
    pub position_account_ref: Option<String>,
}

/// Physical account kind expected after runtime binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountKindV1 {
    /// Native signer/funding account.
    Wallet,
    /// Token account.
    Token,
    /// Hoard-principal token account; never a fee account.
    HoardPrincipal,
    /// Per-wallet protocol Position.
    Position,
    /// Resolution certificate.
    Certificate,
    /// Core Market account.
    Market,
}

/// Logical account declaration awaiting runtime address binding.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountSpecV1 {
    /// Stable logical account reference.
    pub id: String,
    /// Physical account kind.
    pub kind: AccountKindV1,
    /// Exact address after capture; absent in a scenario-only fixture.
    pub address: Option<String>,
    /// Logical or literal expected owner/program reference.
    pub expected_owner_ref: String,
    /// Mint reference for Token and Hoard accounts.
    pub mint_ref: Option<String>,
    /// Token authority wallet reference for ordinary token accounts.
    pub token_authority_wallet_ref: Option<String>,
}

/// Presence of one logical account at a snapshot or transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountPresenceV1 {
    /// Account has not been created.
    Absent,
    /// Account exists.
    Present,
    /// Account was explicitly closed.
    Closed,
}

/// Exact logical ledger state.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerSnapshotV1 {
    /// Logical account presence rows.
    pub account_states: Vec<AccountStateV1>,
    /// Exact token balances for every Token/Hoard account.
    pub token_balances: Vec<TokenBalanceV1>,
    /// Scenario-local Position revisions.
    pub position_revisions: Vec<PositionRevisionV1>,
}

/// One logical account presence row.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountStateV1 {
    /// Logical account reference.
    pub account_ref: String,
    /// Presence state.
    pub state: AccountPresenceV1,
}

/// Exact unsigned token balance.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenBalanceV1 {
    /// Logical token account reference.
    pub account_ref: String,
    /// Exact Mint reference.
    pub mint_ref: String,
    /// Unsigned decimal atom balance.
    pub atoms: String,
}

/// Scenario-local Position revision.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PositionRevisionV1 {
    /// Logical Position account.
    pub position_account_ref: String,
    /// Unsigned decimal revision.
    pub revision: String,
}

/// Canonical activity operation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationV1 {
    /// Stable operation identity.
    pub id: String,
    /// Zero-based canonical order.
    pub order: u32,
    /// Operation family.
    pub kind: OperationKindV1,
    /// Immediate predecessor, absent only for order zero.
    pub predecessor_id: Option<String>,
    /// Ordered dependency identities; currently the immediate predecessor.
    pub dependency_ids: Vec<String>,
    /// Logical fee payer wallet.
    pub fee_payer_wallet_ref: String,
    /// Public caller target the runtime adapter must use.
    pub caller_target: String,
    /// Current availability of that target on committed HEAD.
    pub caller_availability: CallerAvailabilityV1,
    /// Exact committed public input schema, absent when an adapter is required.
    pub caller_schema: Option<String>,
    /// Whether the present caller is expected to mutate external state.
    pub mutation_expected: bool,
    /// Stable receipt/evidence output reference.
    pub evidence_output_ref: String,
    /// Runtime capture; all fields absent in canonical scenario fixtures.
    pub capture: OperationCaptureV1,
    /// Exact action-specific inputs and derived arithmetic.
    pub input: OperationInputV1,
    /// Deltas an execution may claim now. Empty for preflight-only Direct.
    pub expected_observed_delta: LedgerDeltaV1,
    /// Exact accepted-transition economics, including preflight-only Direct.
    pub projected_accepted_delta: LedgerDeltaV1,
}

/// Availability of an operation's committed public caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallerAvailabilityV1 {
    /// A committed public caller can submit the mutation.
    PublicExecutable,
    /// A committed public caller only plans or preflights the operation.
    PreflightOnly,
    /// No committed public caller schema exists; runtime integration must stop.
    AdapterRequired,
}

/// Stable activity operation family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKindV1 {
    /// Create/found one Market.
    Found,
    /// Admit one participant and split complete sets.
    Participant,
    /// Execute or preflight one Direct fill.
    Direct,
    /// Persist one terminal Resolution certificate.
    Resolve,
    /// Burn one wallet claim balance and receive its exact payout.
    Redeem,
    /// Close the fully discharged Market lifecycle.
    Retire,
}

/// Runtime transaction evidence absent from scenario-only fixtures.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationCaptureV1 {
    /// Transaction signature.
    pub signature: Option<String>,
    /// Finalized observation slot.
    pub finalized_slot: Option<String>,
    /// Exact transaction fee.
    pub transaction_fee_lamports: Option<String>,
}

/// Action-specific exact inputs.
// Keeping the Direct fields flat is part of the consumer JSON contract. A
// boxed nested payload would reduce this Rust enum but introduce a second
// object layer that the activity adapter would otherwise have to unwrap.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OperationInputV1 {
    /// Market founding.
    Found {
        /// Logical Market account.
        market_ref: String,
        /// Runtime-supplied market input artifact.
        market_input_artifact: Option<String>,
    },
    /// Participant complete-set split.
    Participant {
        /// Logical participant wallet.
        wallet_ref: String,
        /// Exact complete-set quantity.
        complete_set_atoms: String,
        /// Exact collateral principal moved into Hoard.
        collateral_principal_atoms: String,
    },
    /// Direct fill with independently floored side fees.
    Direct {
        /// Logical seller wallet.
        seller_wallet_ref: String,
        /// Logical buyer wallet.
        buyer_wallet_ref: String,
        /// Traded claim coordinate.
        outcome: u32,
        /// Exact traded claim Mint.
        claim_mint_ref: String,
        /// Exact fill quantity.
        fill_atoms: String,
        /// Exact intent capacity before this fill.
        remaining_before_atoms: String,
        /// Exact intent capacity after this fill.
        remaining_after_atoms: String,
        /// Whether this fill exhausts the selected intent.
        completion: FillCompletionV1,
        /// Exact price scale.
        price_scale_atoms: String,
        /// Exact execution price.
        execution_price_atoms: String,
        /// Exact integral gross quote.
        gross_collateral_atoms: String,
        /// Exact fee policy basis points.
        fee_basis_points: u16,
        /// Seller-side independent floor.
        seller_fee_atoms: String,
        /// Buyer-side independent floor.
        buyer_fee_atoms: String,
        /// Exact seller collateral credit after fee.
        seller_net_atoms: String,
        /// Exact buyer collateral debit including fee.
        buyer_debit_atoms: String,
        /// Exact fee-recipient credit.
        fee_recipient_credit_atoms: String,
    },
    /// Terminal resolution certificate projection.
    Resolve {
        /// Logical certificate account.
        certificate_account_ref: String,
        /// Categorical selector persisted by Core.
        selector: u32,
        /// Exact ordered payout partition.
        payout_atoms_per_claim: Vec<String>,
        /// Certificate owner/program reference.
        expected_owner_ref: String,
        /// Runtime-bound exact data digest.
        data_sha256: Option<String>,
    },
    /// One wallet/outcome terminal redemption.
    Redeem {
        /// Logical redeemer wallet.
        wallet_ref: String,
        /// Redeemed outcome coordinate.
        outcome: u32,
        /// Exact claim atoms burned.
        claim_atoms_burned: String,
        /// Exact payout per burned claim atom.
        payout_atoms_per_claim: String,
        /// Exact collateral credited, possibly zero.
        collateral_atoms_credited: String,
        /// Exact Hoard principal debit, equal to the payout.
        hoard_principal_atoms_debited: String,
    },
    /// Terminal retirement projection.
    Retire {
        /// Whether the projected prestate is eligible.
        eligible: bool,
        /// Exact logical closure set expected from the high-level journey.
        closure_account_refs: Vec<String>,
        /// Logical rent-refund wallet.
        rent_refund_wallet_ref: String,
        /// Runtime-observed exact refund, absent in scenario fixtures.
        rent_refund_lamports: Option<String>,
    },
}

/// Direct fill completion class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillCompletionV1 {
    /// Some selected intent capacity remains.
    Partial,
    /// The selected intent is exactly exhausted.
    Full,
}

/// Complete logical ledger delta for one operation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerDeltaV1 {
    /// Signed native-lamport changes; empty before runtime binding.
    pub lamport_deltas: Vec<LamportDeltaV1>,
    /// Signed exact token changes.
    pub token_deltas: Vec<TokenDeltaV1>,
    /// Account creation/closure transitions.
    pub account_state_changes: Vec<AccountStateChangeV1>,
    /// Position revision transitions.
    pub position_changes: Vec<PositionChangeV1>,
}

impl LedgerDeltaV1 {
    /// Construct an empty delta.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            lamport_deltas: Vec::new(),
            token_deltas: Vec::new(),
            account_state_changes: Vec::new(),
            position_changes: Vec::new(),
        }
    }
}

/// Signed native-lamport delta.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LamportDeltaV1 {
    /// Logical wallet or account.
    pub account_ref: String,
    /// Signed decimal lamports.
    pub delta_lamports: String,
}

/// Signed exact token delta.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenDeltaV1 {
    /// Logical controlling wallet, absent for Hoard and fee collection.
    pub wallet_ref: Option<String>,
    /// Logical token account.
    pub account_ref: String,
    /// Exact Mint reference.
    pub mint_ref: String,
    /// Account presence before the operation.
    pub before_state: AccountPresenceV1,
    /// Account presence after the operation.
    pub after_state: AccountPresenceV1,
    /// Signed decimal atom delta.
    pub delta_atoms: String,
}

/// Account presence transition.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountStateChangeV1 {
    /// Logical account.
    pub account_ref: String,
    /// Presence before the operation.
    pub before: AccountPresenceV1,
    /// Presence after the operation.
    pub after: AccountPresenceV1,
}

/// Scenario-local Position revision transition.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PositionChangeV1 {
    /// Logical Position account.
    pub position_account_ref: String,
    /// Unsigned decimal revision before.
    pub before_revision: String,
    /// Unsigned decimal revision after.
    pub after_revision: String,
    /// Runtime-bound exact prestate digest.
    pub before_data_sha256: Option<String>,
    /// Runtime-bound exact poststate digest.
    pub after_data_sha256: Option<String>,
}
