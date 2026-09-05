use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, OpenOptions},
    io::Write as _,
    path::Path,
};

use dclutch_trading::successor::DIRECT_FEE_DENOMINATOR_V1;
use sha2::{Digest as _, Sha256};

use crate::{
    model::{
        AccountKindV1, AccountPresenceV1, AccountSpecV1, AccountStateChangeV1, AccountStateV1,
        ActivityLimitsV1, BODY_DIGEST_SCOPE_V1, CallerAvailabilityV1, ClusterTargetV1,
        DEVNET_FEE_BASIS_POINTS_V1, DEVNET_GENESIS_HASH_V1, EvidenceLevelV1, FillCompletionV1,
        LedgerDeltaV1, LedgerSnapshotV1, MANIFEST_SCHEMA_V1, MANIFEST_VERSION_V1,
        ManifestEnvelopeV1, MarketProfileV1, MarketSpecV1, OperationCaptureV1, OperationInputV1,
        OperationKindV1, OperationV1, PositionChangeV1, PositionRevisionV1, ResolutionSpecV1,
        ScenarioBodyV1, TokenBalanceV1, TokenDeltaV1, WalletSpecV1,
    },
    scenarios::{
        DEFINITIONS, OUTCOME_COUNT, PRICE_SCALE, ResolutionDefinition, ScenarioDefinition,
        TradeDefinition,
    },
};

/// Stable scenario-generation or validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::new(format!("I/O: {value}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::new(format!("JSON: {value}"))
    }
}

/// Result alias for scenario generation and validation.
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone)]
struct TokenState {
    mint_ref: String,
    atoms: u64,
}

struct Ledger {
    presence: BTreeMap<String, AccountPresenceV1>,
    tokens: BTreeMap<String, TokenState>,
    revisions: BTreeMap<String, u64>,
}

impl Ledger {
    fn snapshot(&self) -> LedgerSnapshotV1 {
        LedgerSnapshotV1 {
            account_states: self
                .presence
                .iter()
                .map(|(account_ref, state)| AccountStateV1 {
                    account_ref: account_ref.clone(),
                    state: *state,
                })
                .collect(),
            token_balances: self
                .tokens
                .iter()
                .map(|(account_ref, state)| TokenBalanceV1 {
                    account_ref: account_ref.clone(),
                    mint_ref: state.mint_ref.clone(),
                    atoms: state.atoms.to_string(),
                })
                .collect(),
            position_revisions: self
                .revisions
                .iter()
                .map(|(position_account_ref, revision)| PositionRevisionV1 {
                    position_account_ref: position_account_ref.clone(),
                    revision: revision.to_string(),
                })
                .collect(),
        }
    }

    fn state_change(
        &mut self,
        account_ref: &str,
        before: AccountPresenceV1,
        after: AccountPresenceV1,
    ) -> Result<AccountStateChangeV1> {
        let observed = self
            .presence
            .get(account_ref)
            .copied()
            .ok_or_else(|| Error::new(format!("unknown account {account_ref}")))?;
        if observed != before {
            return Err(Error::new(format!(
                "account {account_ref} state is {observed:?}, expected {before:?}"
            )));
        }
        self.presence.insert(account_ref.to_owned(), after);
        Ok(AccountStateChangeV1 {
            account_ref: account_ref.to_owned(),
            before,
            after,
        })
    }

    fn token_delta(
        &mut self,
        wallet_ref: Option<&str>,
        account_ref: &str,
        delta: i128,
    ) -> Result<TokenDeltaV1> {
        let presence = self
            .presence
            .get(account_ref)
            .copied()
            .ok_or_else(|| Error::new(format!("unknown token account {account_ref}")))?;
        if presence != AccountPresenceV1::Present {
            return Err(Error::new(format!(
                "token account {account_ref} is not present"
            )));
        }
        let token = self
            .tokens
            .get_mut(account_ref)
            .ok_or_else(|| Error::new(format!("missing token balance {account_ref}")))?;
        let before = i128::from(token.atoms);
        let after = before
            .checked_add(delta)
            .ok_or_else(|| Error::new("signed token arithmetic overflowed"))?;
        token.atoms = u64::try_from(after)
            .map_err(|_| Error::new(format!("token account {account_ref} underflowed u64")))?;
        Ok(TokenDeltaV1 {
            wallet_ref: wallet_ref.map(str::to_owned),
            account_ref: account_ref.to_owned(),
            mint_ref: token.mint_ref.clone(),
            before_state: presence,
            after_state: presence,
            delta_atoms: signed_decimal(delta),
        })
    }

    fn position_change(&mut self, position_account_ref: &str) -> Result<PositionChangeV1> {
        let before = self
            .revisions
            .get(position_account_ref)
            .copied()
            .ok_or_else(|| Error::new(format!("unknown Position {position_account_ref}")))?;
        let after = before
            .checked_add(1)
            .ok_or_else(|| Error::new("Position revision overflowed"))?;
        self.revisions
            .insert(position_account_ref.to_owned(), after);
        Ok(PositionChangeV1 {
            position_account_ref: position_account_ref.to_owned(),
            before_revision: before.to_string(),
            after_revision: after.to_string(),
            before_data_sha256: None,
            after_data_sha256: None,
        })
    }

    fn token_atoms(&self, account_ref: &str) -> Result<u64> {
        self.tokens
            .get(account_ref)
            .map(|value| value.atoms)
            .ok_or_else(|| Error::new(format!("unknown token account {account_ref}")))
    }
}

struct OperationBuilder {
    scenario_id: String,
    fee_payer_wallet_ref: String,
    operations: Vec<OperationV1>,
}

struct OperationDraft<'a> {
    id: String,
    kind: OperationKindV1,
    caller_target: &'a str,
    caller_availability: CallerAvailabilityV1,
    caller_schema: Option<&'a str>,
    mutation_expected: bool,
    input: OperationInputV1,
    projected: LedgerDeltaV1,
}

impl OperationBuilder {
    fn new(scenario_id: &str) -> Self {
        Self {
            scenario_id: scenario_id.to_owned(),
            fee_payer_wallet_ref: "deployer".to_owned(),
            operations: Vec::new(),
        }
    }

    fn push(&mut self, draft: OperationDraft<'_>) -> Result<()> {
        if self
            .operations
            .iter()
            .any(|operation| operation.id == draft.id)
        {
            return Err(Error::new(format!("duplicate operation {}", draft.id)));
        }
        let order = u32::try_from(self.operations.len())
            .map_err(|_| Error::new("operation count exceeded u32"))?;
        let predecessor_id = self.operations.last().map(|operation| operation.id.clone());
        let dependency_ids = predecessor_id.iter().cloned().collect();
        let expected_observed_delta = if draft.mutation_expected {
            draft.projected.clone()
        } else {
            LedgerDeltaV1::empty()
        };
        self.operations.push(OperationV1 {
            evidence_output_ref: format!("evidence.{}.{}", self.scenario_id, draft.id),
            id: draft.id,
            order,
            kind: draft.kind,
            predecessor_id,
            dependency_ids,
            fee_payer_wallet_ref: self.fee_payer_wallet_ref.clone(),
            caller_target: draft.caller_target.to_owned(),
            caller_availability: draft.caller_availability,
            caller_schema: draft.caller_schema.map(str::to_owned),
            mutation_expected: draft.mutation_expected,
            capture: OperationCaptureV1 {
                signature: None,
                finalized_slot: None,
                transaction_fee_lamports: None,
            },
            input: draft.input,
            expected_observed_delta,
            projected_accepted_delta: draft.projected,
        });
        Ok(())
    }
}

/// Return every canonical manifest file as exact bytes, excluding `SHA256SUMS`.
pub fn canonical_manifest_set() -> Result<BTreeMap<String, Vec<u8>>> {
    let mut output = BTreeMap::new();
    for definition in DEFINITIONS {
        let bytes = canonical_manifest_bytes(definition.id)?;
        if output
            .insert(definition.filename.to_owned(), bytes)
            .is_some()
        {
            return Err(Error::new("duplicate canonical fixture filename"));
        }
    }
    Ok(output)
}

/// Generate one canonical manifest by stable scenario identity.
pub fn canonical_manifest_bytes(scenario_id: &str) -> Result<Vec<u8>> {
    let definition = definition(scenario_id)?;
    let envelope = build_envelope(definition)?;
    validate_manifest(&envelope)?;
    let mut bytes = serde_json::to_vec_pretty(&envelope)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Validate one parsed canonical scenario manifest.
pub fn validate_manifest(envelope: &ManifestEnvelopeV1) -> Result<()> {
    if envelope.schema != MANIFEST_SCHEMA_V1
        || envelope.version != MANIFEST_VERSION_V1
        || envelope.body_digest_scope != BODY_DIGEST_SCOPE_V1
        || envelope.scenario_id != envelope.body.scenario_id
    {
        return Err(Error::new("manifest envelope schema or identity changed"));
    }
    let body_bytes = serde_json::to_vec(&envelope.body)?;
    if envelope.body_sha256 != hex_sha256(&body_bytes) {
        return Err(Error::new("manifest body digest changed"));
    }
    validate_body_structure(&envelope.body)?;
    let canonical = build_body(definition(&envelope.scenario_id)?)?;
    if envelope.body != canonical {
        return Err(Error::new(
            "manifest differs from its deterministic canonical scenario",
        ));
    }
    Ok(())
}

/// Atomically create a fresh fixture directory containing all manifests and a
/// full-file SHA-256 sidecar. Existing output is refused rather than replaced.
pub fn write_fixture_directory(output: &Path) -> Result<()> {
    if !output.is_absolute() {
        return Err(Error::new("fixture output directory must be absolute"));
    }
    if output.exists() {
        return Err(Error::new("fixture output directory already exists"));
    }
    let parent = output
        .parent()
        .ok_or_else(|| Error::new("fixture output has no parent"))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("fixture output name is not UTF-8"))?;
    let temporary = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    fs::create_dir(&temporary)?;
    let manifests = canonical_manifest_set()?;
    let mut sums = String::new();
    for (filename, bytes) in &manifests {
        let path = temporary.join(filename);
        write_new_synced(&path, bytes)?;
        sums.push_str(&format!("{}  {filename}\n", hex_sha256(bytes)));
    }
    write_new_synced(&temporary.join("SHA256SUMS"), sums.as_bytes())?;
    fs::File::open(&temporary)?.sync_all()?;
    fs::rename(&temporary, output)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

/// Check one tracked fixture directory byte-for-byte without changing it.
pub fn check_fixture_directory(directory: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(directory)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Error::new("fixture path is not a regular directory"));
    }
    let manifests = canonical_manifest_set()?;
    let mut expected_names = manifests.keys().cloned().collect::<BTreeSet<_>>();
    expected_names.insert("SHA256SUMS".to_owned());
    let mut observed_names = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::new("fixture filename is not UTF-8"))?;
        let entry_metadata = fs::symlink_metadata(entry.path())?;
        if entry_metadata.file_type().is_symlink() || !entry_metadata.file_type().is_file() {
            return Err(Error::new("fixture directory contains a non-regular file"));
        }
        if !observed_names.insert(name) {
            return Err(Error::new("fixture directory repeats a filename"));
        }
    }
    if observed_names != expected_names {
        return Err(Error::new("fixture directory has missing or extra files"));
    }
    let mut sums = String::new();
    for (filename, expected) in manifests {
        let path = directory.join(&filename);
        let observed = fs::read(&path)?;
        if observed != expected {
            return Err(Error::new(format!("fixture {filename} changed")));
        }
        let parsed: ManifestEnvelopeV1 = serde_json::from_slice(&observed)?;
        validate_manifest(&parsed)?;
        sums.push_str(&format!("{}  {filename}\n", hex_sha256(&observed)));
    }
    if fs::read(directory.join("SHA256SUMS"))? != sums.as_bytes() {
        return Err(Error::new("fixture SHA256SUMS changed"));
    }
    Ok(())
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn definition(scenario_id: &str) -> Result<&'static ScenarioDefinition> {
    DEFINITIONS
        .iter()
        .find(|definition| definition.id == scenario_id)
        .ok_or_else(|| Error::new(format!("unknown scenario {scenario_id}")))
}

fn build_envelope(definition: &ScenarioDefinition) -> Result<ManifestEnvelopeV1> {
    let body = build_body(definition)?;
    let body_sha256 = hex_sha256(&serde_json::to_vec(&body)?);
    Ok(ManifestEnvelopeV1 {
        schema: MANIFEST_SCHEMA_V1.to_owned(),
        version: MANIFEST_VERSION_V1,
        scenario_id: definition.id.to_owned(),
        body_digest_scope: BODY_DIGEST_SCOPE_V1.to_owned(),
        body_sha256,
        body,
    })
}

fn build_body(definition: &ScenarioDefinition) -> Result<ScenarioBodyV1> {
    validate_definition(definition)?;
    let market_ref = format!("market.{}", definition.id);
    let collateral_mint_ref = format!("mint.{}.collateral", definition.id);
    let claim_mint_refs = (0..OUTCOME_COUNT)
        .map(|outcome| format!("mint.{}.claim.{outcome}", definition.id))
        .collect::<Vec<_>>();
    let fee_recipient_account_ref = collateral_ref(definition.id, "deployer");
    let hoard_principal_account_ref = format!("token.{}.hoard-principal", definition.id);
    let certificate_account_ref = format!("certificate.{}", definition.id);
    let resolution_payouts = definition.resolution.payouts();

    let wallets = build_wallets(definition);
    let accounts = build_accounts(
        definition,
        &market_ref,
        &certificate_account_ref,
        &collateral_mint_ref,
        &claim_mint_refs,
        &hoard_principal_account_ref,
    );
    let mut ledger = initial_ledger(
        definition,
        &accounts,
        &collateral_mint_ref,
        &claim_mint_refs,
        &hoard_principal_account_ref,
    )?;
    let initial_snapshot = ledger.snapshot();
    let mut operations = OperationBuilder::new(definition.id);

    let found_delta = LedgerDeltaV1 {
        lamport_deltas: Vec::new(),
        token_deltas: Vec::new(),
        account_state_changes: vec![
            ledger.state_change(
                &market_ref,
                AccountPresenceV1::Absent,
                AccountPresenceV1::Present,
            )?,
            ledger.state_change(
                &hoard_principal_account_ref,
                AccountPresenceV1::Absent,
                AccountPresenceV1::Present,
            )?,
        ],
        position_changes: Vec::new(),
    };
    let (found_target, found_availability, found_schema) = match definition.profile {
        MarketProfileV1::Flagship => (
            "dclutch-local-successor-bootstrap/devnet-market + campaign/found",
            CallerAvailabilityV1::PublicExecutable,
            Some("MarketRunInput (bare deny-unknown-fields projection)"),
        ),
        MarketProfileV1::Graduation => (
            "dclutch-local-successor-bootstrap/graduation-market + campaign/found",
            CallerAvailabilityV1::PublicExecutable,
            Some("dclutch-graduation-market-input-v1"),
        ),
        MarketProfileV1::Abandoned => (
            "graded-failure-market-founding",
            CallerAvailabilityV1::AdapterRequired,
            None,
        ),
    };
    operations.push(OperationDraft {
        id: format!("{}-found", definition.id),
        kind: OperationKindV1::Found,
        caller_target: found_target,
        caller_availability: found_availability,
        caller_schema: found_schema,
        mutation_expected: found_availability == CallerAvailabilityV1::PublicExecutable,
        input: OperationInputV1::Found {
            market_ref: market_ref.clone(),
            market_input_artifact: None,
        },
        projected: found_delta,
    })?;

    for wallet in definition.wallets {
        let principal = wallet
            .complete_sets
            .checked_mul(definition.resolution.payout_scale())
            .ok_or_else(|| Error::new("complete-set principal overflowed"))?;
        let position = position_ref(definition.id, wallet.id);
        let mut token_deltas = vec![
            ledger.token_delta(
                Some(wallet.id),
                &collateral_ref(definition.id, wallet.id),
                -i128::from(principal),
            )?,
            ledger.token_delta(None, &hoard_principal_account_ref, i128::from(principal))?,
        ];
        for outcome in 0..OUTCOME_COUNT {
            token_deltas.push(ledger.token_delta(
                Some(wallet.id),
                &claim_ref(definition.id, wallet.id, outcome),
                i128::from(wallet.complete_sets),
            )?);
        }
        let state_change = ledger.state_change(
            &position,
            AccountPresenceV1::Absent,
            AccountPresenceV1::Present,
        )?;
        let position_change = ledger.position_change(&position)?;
        operations.push(OperationDraft {
            id: format!("{}-participant-{}", definition.id, wallet.id),
            kind: OperationKindV1::Participant,
            caller_target: "dclutch-local-successor-bootstrap/devnet-market-participant-v1",
            caller_availability: CallerAvailabilityV1::PublicExecutable,
            caller_schema: Some("dclutch-devnet-market-participant-operation-v1"),
            mutation_expected: true,
            input: OperationInputV1::Participant {
                wallet_ref: wallet.id.to_owned(),
                complete_set_atoms: wallet.complete_sets.to_string(),
                collateral_principal_atoms: principal.to_string(),
            },
            projected: LedgerDeltaV1 {
                lamport_deltas: Vec::new(),
                token_deltas,
                account_state_changes: vec![state_change],
                position_changes: vec![position_change],
            },
        })?;
    }

    for trade in definition.trades {
        let (input, delta) = apply_trade(
            definition,
            trade,
            &claim_mint_refs,
            &fee_recipient_account_ref,
            &mut ledger,
        )?;
        // The present public Direct command is preflight-only. Keeping this
        // false prevents a scenario projection from being counted as a chain
        // mutation. A runtime adapter must promote and bind it explicitly.
        operations.push(OperationDraft {
            id: trade.id.to_owned(),
            kind: OperationKindV1::Direct,
            caller_target: "direct-accepted-transition",
            caller_availability: CallerAvailabilityV1::AdapterRequired,
            caller_schema: None,
            mutation_expected: false,
            input,
            projected: delta,
        })?;
    }

    let resolve_delta = LedgerDeltaV1 {
        lamport_deltas: Vec::new(),
        token_deltas: Vec::new(),
        account_state_changes: vec![ledger.state_change(
            &certificate_account_ref,
            AccountPresenceV1::Absent,
            AccountPresenceV1::Present,
        )?],
        position_changes: Vec::new(),
    };
    let (resolution_target, resolution_availability, resolution_schema) = match definition.profile {
        MarketProfileV1::Flagship => (
            "dclutch-local-successor-bootstrap/flagship-resolution-v1",
            CallerAvailabilityV1::PublicExecutable,
            Some("dclutch-flagship-resolution-input-v1"),
        ),
        MarketProfileV1::Graduation => (
            "relayed-graduation-resolution",
            CallerAvailabilityV1::AdapterRequired,
            None,
        ),
        MarketProfileV1::Abandoned => (
            "funded-failure-resolution",
            CallerAvailabilityV1::AdapterRequired,
            None,
        ),
    };
    operations.push(OperationDraft {
        id: format!("{}-resolve", definition.id),
        kind: OperationKindV1::Resolve,
        caller_target: resolution_target,
        caller_availability: resolution_availability,
        caller_schema: resolution_schema,
        mutation_expected: resolution_availability == CallerAvailabilityV1::PublicExecutable,
        input: OperationInputV1::Resolve {
            certificate_account_ref: certificate_account_ref.clone(),
            selector: u32::try_from(definition.resolution.selector())
                .map_err(|_| Error::new("resolution selector exceeded u32"))?,
            payout_atoms_per_claim: resolution_payouts.iter().map(u64::to_string).collect(),
            expected_owner_ref: "program.resolution".to_owned(),
            data_sha256: None,
        },
        projected: resolve_delta,
    })?;

    for wallet in definition.wallets {
        for (outcome, payout_per_claim) in resolution_payouts.iter().copied().enumerate() {
            let claim_account = claim_ref(definition.id, wallet.id, outcome);
            let quantity = ledger.token_atoms(&claim_account)?;
            if quantity == 0 {
                continue;
            }
            let payout = quantity
                .checked_mul(payout_per_claim)
                .ok_or_else(|| Error::new("terminal payout overflowed"))?;
            let delta = LedgerDeltaV1 {
                lamport_deltas: Vec::new(),
                token_deltas: vec![
                    ledger.token_delta(Some(wallet.id), &claim_account, -i128::from(quantity))?,
                    ledger.token_delta(
                        Some(wallet.id),
                        &collateral_ref(definition.id, wallet.id),
                        i128::from(payout),
                    )?,
                    ledger.token_delta(None, &hoard_principal_account_ref, -i128::from(payout))?,
                ],
                account_state_changes: Vec::new(),
                position_changes: vec![
                    ledger.position_change(&position_ref(definition.id, wallet.id))?,
                ],
            };
            operations.push(OperationDraft {
                id: format!("{}-redeem-{}-o{}", definition.id, wallet.id, outcome),
                kind: OperationKindV1::Redeem,
                caller_target: "dclutch-local-successor-bootstrap/wallet-terminal-payout-input",
                caller_availability: CallerAvailabilityV1::PreflightOnly,
                caller_schema: Some("dclutch-wallet-terminal-payout-plan-input-v1"),
                mutation_expected: false,
                input: OperationInputV1::Redeem {
                    wallet_ref: wallet.id.to_owned(),
                    outcome: u32::try_from(outcome)
                        .map_err(|_| Error::new("redemption outcome exceeded u32"))?,
                    claim_atoms_burned: quantity.to_string(),
                    payout_atoms_per_claim: payout_per_claim.to_string(),
                    collateral_atoms_credited: payout.to_string(),
                    hoard_principal_atoms_debited: payout.to_string(),
                },
                projected: delta,
            })?;
        }
    }

    let retire_eligible = retirement_eligible(definition, &ledger, &hoard_principal_account_ref)?;
    if !retire_eligible {
        return Err(Error::new(
            "canonical scenario did not discharge every liability",
        ));
    }
    let mut closure_account_refs = vec![market_ref.clone(), certificate_account_ref.clone()];
    closure_account_refs.extend(
        definition
            .wallets
            .iter()
            .map(|wallet| position_ref(definition.id, wallet.id)),
    );
    closure_account_refs.push(hoard_principal_account_ref.clone());
    let mut state_changes = Vec::new();
    for account_ref in &closure_account_refs {
        state_changes.push(ledger.state_change(
            account_ref,
            AccountPresenceV1::Present,
            AccountPresenceV1::Closed,
        )?);
    }
    operations.push(OperationDraft {
        id: format!("{}-retire", definition.id),
        kind: OperationKindV1::Retire,
        caller_target: "terminal-retirement",
        caller_availability: CallerAvailabilityV1::AdapterRequired,
        caller_schema: None,
        mutation_expected: false,
        input: OperationInputV1::Retire {
            eligible: true,
            closure_account_refs,
            rent_refund_wallet_ref: "deployer".to_owned(),
            rent_refund_lamports: None,
        },
        projected: LedgerDeltaV1 {
            lamport_deltas: Vec::new(),
            token_deltas: Vec::new(),
            account_state_changes: state_changes,
            position_changes: Vec::new(),
        },
    })?;

    let final_snapshot = ledger.snapshot();
    validate_conservation(
        definition,
        &initial_snapshot,
        &final_snapshot,
        &fee_recipient_account_ref,
        &hoard_principal_account_ref,
    )?;

    Ok(ScenarioBodyV1 {
        scenario_id: definition.id.to_owned(),
        title: definition.title.to_owned(),
        description: definition.description.to_owned(),
        cluster_target: ClusterTargetV1::Devnet,
        genesis_hash: DEVNET_GENESIS_HASH_V1.to_owned(),
        evidence_level: EvidenceLevelV1::ScenarioOnly,
        market: MarketSpecV1 {
            profile: definition.profile,
            market_ref,
            input_artifact: None,
            outcome_count: u32::try_from(OUTCOME_COUNT)
                .map_err(|_| Error::new("outcome count exceeded u32"))?,
            collateral_mint_ref,
            claim_mint_refs,
            resolution: resolution_spec(definition.resolution),
            price_scale_atoms: PRICE_SCALE.to_string(),
            fee_denominator: u64::from(DIRECT_FEE_DENOMINATOR_V1).to_string(),
            fee_basis_points_per_side: DEVNET_FEE_BASIS_POINTS_V1,
            fee_recipient_account_ref,
            hoard_principal_account_ref,
        },
        limits: ActivityLimitsV1 {
            max_concurrency: 1,
            min_dispatch_interval_ms: 1_500,
            max_transactions: 96,
            poll_interval_ms: 1_000,
            max_polls: 180,
        },
        wallets,
        accounts,
        initial_snapshot,
        operations: operations.operations,
        final_snapshot,
        retire_eligible,
    })
}

fn validate_definition(definition: &ScenarioDefinition) -> Result<()> {
    if definition.wallets.len() < 4 || definition.trades.is_empty() {
        return Err(Error::new("scenario lacks multi-wallet trade coverage"));
    }
    let mut wallet_ids = BTreeSet::new();
    for wallet in definition.wallets {
        if wallet.id.is_empty()
            || !wallet_ids.insert(wallet.id)
            || wallet.complete_sets == 0
            || wallet.initial_collateral == 0
        {
            return Err(Error::new("invalid or duplicate scenario wallet"));
        }
        let principal = wallet
            .complete_sets
            .checked_mul(definition.resolution.payout_scale())
            .ok_or_else(|| Error::new("participant principal overflowed"))?;
        if principal > wallet.initial_collateral {
            return Err(Error::new("participant collateral is insufficient"));
        }
    }
    let mut trade_ids = BTreeSet::new();
    let mut partial = false;
    let mut full = false;
    for trade in definition.trades {
        if !trade_ids.insert(trade.id)
            || !wallet_ids.contains(trade.seller)
            || !wallet_ids.contains(trade.buyer)
            || trade.seller == trade.buyer
            || trade.outcome >= OUTCOME_COUNT
            || trade.fill == 0
            || trade.fill > trade.remaining_before
            || trade.execution_price > PRICE_SCALE
        {
            return Err(Error::new("invalid or duplicate Direct scenario row"));
        }
        partial |= trade.fill < trade.remaining_before;
        full |= trade.fill == trade.remaining_before;
        let _ = exact_quote(trade.fill, trade.execution_price, PRICE_SCALE)?;
    }
    if !partial || !full {
        return Err(Error::new(
            "scenario omitted partial or full Direct coverage",
        ));
    }
    let total = definition
        .resolution
        .payouts()
        .iter()
        .try_fold(0_u64, |sum, payout| sum.checked_add(*payout))
        .ok_or_else(|| Error::new("resolution partition overflowed"))?;
    if total != definition.resolution.payout_scale() {
        return Err(Error::new(
            "resolution payouts do not partition the exact scale",
        ));
    }
    Ok(())
}

fn build_wallets(definition: &ScenarioDefinition) -> Vec<WalletSpecV1> {
    let mut wallets = definition
        .wallets
        .iter()
        .map(|wallet| WalletSpecV1 {
            id: wallet.id.to_owned(),
            roles: vec![
                "participant".to_owned(),
                "seller".to_owned(),
                "buyer".to_owned(),
                "redeemer".to_owned(),
            ],
            funding_lamports: "50000000".to_owned(),
            collateral_account_ref: collateral_ref(definition.id, wallet.id),
            claim_account_refs: (0..OUTCOME_COUNT)
                .map(|outcome| claim_ref(definition.id, wallet.id, outcome))
                .collect(),
            position_account_ref: Some(position_ref(definition.id, wallet.id)),
        })
        .collect::<Vec<_>>();
    wallets.push(WalletSpecV1 {
        id: "deployer".to_owned(),
        roles: vec![
            "fee-payer".to_owned(),
            "fee-recipient".to_owned(),
            "retirement-beneficiary".to_owned(),
        ],
        funding_lamports: "150000000".to_owned(),
        collateral_account_ref: collateral_ref(definition.id, "deployer"),
        claim_account_refs: Vec::new(),
        position_account_ref: None,
    });
    wallets
}

fn build_accounts(
    definition: &ScenarioDefinition,
    market_ref: &str,
    certificate_ref: &str,
    collateral_mint_ref: &str,
    claim_mint_refs: &[String],
    hoard_ref: &str,
) -> Vec<AccountSpecV1> {
    let mut accounts = vec![
        AccountSpecV1 {
            id: market_ref.to_owned(),
            kind: AccountKindV1::Market,
            address: None,
            expected_owner_ref: "program.core".to_owned(),
            mint_ref: None,
            token_authority_wallet_ref: None,
        },
        AccountSpecV1 {
            id: certificate_ref.to_owned(),
            kind: AccountKindV1::Certificate,
            address: None,
            expected_owner_ref: "program.resolution".to_owned(),
            mint_ref: None,
            token_authority_wallet_ref: None,
        },
        AccountSpecV1 {
            id: hoard_ref.to_owned(),
            kind: AccountKindV1::HoardPrincipal,
            address: None,
            expected_owner_ref: "realm.token-program".to_owned(),
            mint_ref: Some(collateral_mint_ref.to_owned()),
            token_authority_wallet_ref: None,
        },
    ];
    for wallet in definition.wallets {
        accounts.push(wallet_account(wallet.id));
        accounts.push(token_account(
            collateral_ref(definition.id, wallet.id),
            collateral_mint_ref,
            wallet.id,
        ));
        for (outcome, mint_ref) in claim_mint_refs.iter().enumerate() {
            accounts.push(token_account(
                claim_ref(definition.id, wallet.id, outcome),
                mint_ref,
                wallet.id,
            ));
        }
        accounts.push(AccountSpecV1 {
            id: position_ref(definition.id, wallet.id),
            kind: AccountKindV1::Position,
            address: None,
            expected_owner_ref: "program.claims".to_owned(),
            mint_ref: None,
            token_authority_wallet_ref: Some(wallet.id.to_owned()),
        });
    }
    accounts.push(wallet_account("deployer"));
    accounts.push(token_account(
        collateral_ref(definition.id, "deployer"),
        collateral_mint_ref,
        "deployer",
    ));
    accounts
}

fn wallet_account(wallet: &str) -> AccountSpecV1 {
    AccountSpecV1 {
        id: wallet_ref(wallet),
        kind: AccountKindV1::Wallet,
        address: None,
        expected_owner_ref: "solana-system-program".to_owned(),
        mint_ref: None,
        token_authority_wallet_ref: None,
    }
}

fn token_account(id: String, mint_ref: &str, wallet: &str) -> AccountSpecV1 {
    AccountSpecV1 {
        id,
        kind: AccountKindV1::Token,
        address: None,
        expected_owner_ref: "realm.token-program".to_owned(),
        mint_ref: Some(mint_ref.to_owned()),
        token_authority_wallet_ref: Some(wallet.to_owned()),
    }
}

fn initial_ledger(
    definition: &ScenarioDefinition,
    accounts: &[AccountSpecV1],
    collateral_mint_ref: &str,
    claim_mint_refs: &[String],
    hoard_ref: &str,
) -> Result<Ledger> {
    let mut presence = BTreeMap::new();
    for account in accounts {
        let state = match account.kind {
            AccountKindV1::Market
            | AccountKindV1::Certificate
            | AccountKindV1::HoardPrincipal
            | AccountKindV1::Position => AccountPresenceV1::Absent,
            AccountKindV1::Wallet | AccountKindV1::Token => AccountPresenceV1::Present,
        };
        if presence.insert(account.id.clone(), state).is_some() {
            return Err(Error::new("duplicate logical account"));
        }
    }
    let mut tokens = BTreeMap::new();
    for wallet in definition.wallets {
        insert_token(
            &mut tokens,
            collateral_ref(definition.id, wallet.id),
            collateral_mint_ref,
            wallet.initial_collateral,
        )?;
        for (outcome, mint_ref) in claim_mint_refs.iter().enumerate() {
            insert_token(
                &mut tokens,
                claim_ref(definition.id, wallet.id, outcome),
                mint_ref,
                0,
            )?;
        }
    }
    insert_token(
        &mut tokens,
        collateral_ref(definition.id, "deployer"),
        collateral_mint_ref,
        0,
    )?;
    insert_token(&mut tokens, hoard_ref.to_owned(), collateral_mint_ref, 0)?;
    let revisions = definition
        .wallets
        .iter()
        .map(|wallet| (position_ref(definition.id, wallet.id), 0_u64))
        .collect();
    Ok(Ledger {
        presence,
        tokens,
        revisions,
    })
}

fn insert_token(
    tokens: &mut BTreeMap<String, TokenState>,
    account_ref: String,
    mint_ref: &str,
    atoms: u64,
) -> Result<()> {
    if tokens
        .insert(
            account_ref,
            TokenState {
                mint_ref: mint_ref.to_owned(),
                atoms,
            },
        )
        .is_some()
    {
        return Err(Error::new("duplicate token account"));
    }
    Ok(())
}

fn apply_trade(
    definition: &ScenarioDefinition,
    trade: &TradeDefinition,
    claim_mint_refs: &[String],
    fee_recipient_account_ref: &str,
    ledger: &mut Ledger,
) -> Result<(OperationInputV1, LedgerDeltaV1)> {
    let gross = exact_quote(trade.fill, trade.execution_price, PRICE_SCALE)?;
    let seller_fee = fee_floor(gross, DEVNET_FEE_BASIS_POINTS_V1)?;
    let buyer_fee = fee_floor(gross, DEVNET_FEE_BASIS_POINTS_V1)?;
    let seller_net = gross
        .checked_sub(seller_fee)
        .ok_or_else(|| Error::new("seller fee exceeded gross"))?;
    let buyer_debit = gross
        .checked_add(buyer_fee)
        .ok_or_else(|| Error::new("buyer debit overflowed"))?;
    let fee_credit = seller_fee
        .checked_add(buyer_fee)
        .ok_or_else(|| Error::new("fee credit overflowed"))?;
    if seller_net.checked_add(fee_credit) != Some(buyer_debit) {
        return Err(Error::new("Direct collateral conservation failed"));
    }
    let remaining_after = trade
        .remaining_before
        .checked_sub(trade.fill)
        .ok_or_else(|| Error::new("Direct fill exceeded remaining intent"))?;
    let completion = if remaining_after == 0 {
        FillCompletionV1::Full
    } else {
        FillCompletionV1::Partial
    };
    let claim_mint_ref = claim_mint_refs
        .get(trade.outcome)
        .ok_or_else(|| Error::new("Direct outcome has no claim Mint"))?
        .clone();
    let seller_claim = claim_ref(definition.id, trade.seller, trade.outcome);
    let buyer_claim = claim_ref(definition.id, trade.buyer, trade.outcome);
    let delta = LedgerDeltaV1 {
        lamport_deltas: Vec::new(),
        token_deltas: vec![
            ledger.token_delta(Some(trade.seller), &seller_claim, -i128::from(trade.fill))?,
            ledger.token_delta(Some(trade.buyer), &buyer_claim, i128::from(trade.fill))?,
            ledger.token_delta(
                Some(trade.seller),
                &collateral_ref(definition.id, trade.seller),
                i128::from(seller_net),
            )?,
            ledger.token_delta(
                Some(trade.buyer),
                &collateral_ref(definition.id, trade.buyer),
                -i128::from(buyer_debit),
            )?,
            ledger.token_delta(None, fee_recipient_account_ref, i128::from(fee_credit))?,
        ],
        account_state_changes: Vec::new(),
        position_changes: vec![
            ledger.position_change(&position_ref(definition.id, trade.seller))?,
            ledger.position_change(&position_ref(definition.id, trade.buyer))?,
        ],
    };
    Ok((
        OperationInputV1::Direct {
            seller_wallet_ref: trade.seller.to_owned(),
            buyer_wallet_ref: trade.buyer.to_owned(),
            outcome: u32::try_from(trade.outcome)
                .map_err(|_| Error::new("Direct outcome exceeded u32"))?,
            claim_mint_ref,
            fill_atoms: trade.fill.to_string(),
            remaining_before_atoms: trade.remaining_before.to_string(),
            remaining_after_atoms: remaining_after.to_string(),
            completion,
            price_scale_atoms: PRICE_SCALE.to_string(),
            execution_price_atoms: trade.execution_price.to_string(),
            gross_collateral_atoms: gross.to_string(),
            fee_basis_points: DEVNET_FEE_BASIS_POINTS_V1,
            seller_fee_atoms: seller_fee.to_string(),
            buyer_fee_atoms: buyer_fee.to_string(),
            seller_net_atoms: seller_net.to_string(),
            buyer_debit_atoms: buyer_debit.to_string(),
            fee_recipient_credit_atoms: fee_credit.to_string(),
        },
        delta,
    ))
}

fn retirement_eligible(
    definition: &ScenarioDefinition,
    ledger: &Ledger,
    hoard_ref: &str,
) -> Result<bool> {
    if ledger.token_atoms(hoard_ref)? != 0 {
        return Ok(false);
    }
    for wallet in definition.wallets {
        for outcome in 0..OUTCOME_COUNT {
            if ledger.token_atoms(&claim_ref(definition.id, wallet.id, outcome))? != 0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn validate_conservation(
    definition: &ScenarioDefinition,
    initial: &LedgerSnapshotV1,
    final_state: &LedgerSnapshotV1,
    fee_ref: &str,
    hoard_ref: &str,
) -> Result<()> {
    let initial_total = initial
        .token_balances
        .iter()
        .filter(|row| row.mint_ref.ends_with(".collateral"))
        .try_fold(0_u128, |sum, row| {
            let value = decimal_u64(&row.atoms, "initial collateral")?;
            sum.checked_add(u128::from(value))
                .ok_or_else(|| Error::new("initial collateral total overflowed"))
        })?;
    let final_total = final_state
        .token_balances
        .iter()
        .filter(|row| row.mint_ref.ends_with(".collateral"))
        .try_fold(0_u128, |sum, row| {
            let value = decimal_u64(&row.atoms, "final collateral")?;
            sum.checked_add(u128::from(value))
                .ok_or_else(|| Error::new("final collateral total overflowed"))
        })?;
    if initial_total != final_total
        || snapshot_token(final_state, hoard_ref)? != 0
        || snapshot_token(final_state, fee_ref)? == 0
    {
        return Err(Error::new("scenario collateral conservation failed"));
    }
    for outcome in 0..OUTCOME_COUNT {
        let mint_ref = format!("mint.{}.claim.{outcome}", definition.id);
        let final_supply = final_state
            .token_balances
            .iter()
            .filter(|row| row.mint_ref == mint_ref)
            .try_fold(0_u128, |sum, row| {
                sum.checked_add(u128::from(decimal_u64(&row.atoms, "claim balance")?))
                    .ok_or_else(|| Error::new("claim total overflowed"))
            })?;
        if final_supply != 0 {
            return Err(Error::new("scenario left outstanding claim supply"));
        }
    }
    Ok(())
}

fn snapshot_token(snapshot: &LedgerSnapshotV1, account_ref: &str) -> Result<u64> {
    let row = snapshot
        .token_balances
        .iter()
        .find(|row| row.account_ref == account_ref)
        .ok_or_else(|| Error::new(format!("snapshot omitted {account_ref}")))?;
    decimal_u64(&row.atoms, account_ref)
}

fn validate_body_structure(body: &ScenarioBodyV1) -> Result<()> {
    if body.evidence_level != EvidenceLevelV1::ScenarioOnly
        || body.cluster_target != ClusterTargetV1::Devnet
        || body.genesis_hash != DEVNET_GENESIS_HASH_V1
        || body.market.outcome_count
            != u32::try_from(OUTCOME_COUNT).map_err(|_| Error::new("outcome width"))?
        || body.market.fee_basis_points_per_side != DEVNET_FEE_BASIS_POINTS_V1
        || decimal_u64(&body.market.fee_denominator, "fee denominator")?
            != u64::from(DIRECT_FEE_DENOMINATOR_V1)
        || decimal_u64(&body.market.price_scale_atoms, "price scale")? != PRICE_SCALE
    {
        return Err(Error::new("scenario policy or evidence level changed"));
    }
    let mut wallet_ids = BTreeSet::new();
    for wallet in &body.wallets {
        if wallet.id.is_empty()
            || wallet.roles.is_empty()
            || !wallet_ids.insert(wallet.id.clone())
            || decimal_u64(&wallet.funding_lamports, "wallet funding")? == 0
        {
            return Err(Error::new("duplicate or invalid wallet"));
        }
    }
    let mut account_ids = BTreeSet::new();
    let mut bound_addresses = BTreeSet::new();
    let mut carried_live_address = false;
    for account in &body.accounts {
        if account.id.is_empty() || !account_ids.insert(account.id.clone()) {
            return Err(Error::new("duplicate logical account"));
        }
        if let Some(address) = &account.address {
            if !bound_addresses.insert(address.clone()) {
                return Err(Error::new("cross-kind or repeated account address alias"));
            }
            carried_live_address = true;
        }
        match account.kind {
            AccountKindV1::Token | AccountKindV1::HoardPrincipal if account.mint_ref.is_none() => {
                return Err(Error::new("token account omitted its Mint"));
            }
            AccountKindV1::Wallet
            | AccountKindV1::Position
            | AccountKindV1::Certificate
            | AccountKindV1::Market
                if account.mint_ref.is_some() =>
            {
                return Err(Error::new("non-token account carried a Mint"));
            }
            _ => {}
        }
    }
    if carried_live_address {
        return Err(Error::new(
            "scenario-only fixture carried a live account address",
        ));
    }
    let claim_mints = body
        .market
        .claim_mint_refs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if claim_mints.len() != OUTCOME_COUNT || claim_mints.contains(&body.market.collateral_mint_ref)
    {
        return Err(Error::new("duplicate or mixed claim/collateral Mint"));
    }
    validate_resolution(&body.market.resolution)?;
    validate_operations(body, &wallet_ids, &account_ids, &claim_mints)?;
    validate_snapshot(&body.initial_snapshot, &account_ids)?;
    validate_snapshot(&body.final_snapshot, &account_ids)?;
    Ok(())
}

fn validate_resolution(resolution: &ResolutionSpecV1) -> Result<()> {
    let (scale, payouts) = match resolution {
        ResolutionSpecV1::Categorical {
            selector,
            payout_atoms_per_claim,
        } => {
            if usize::try_from(*selector)
                .ok()
                .filter(|value| *value < OUTCOME_COUNT)
                .is_none()
            {
                return Err(Error::new("categorical selector is out of range"));
            }
            (1, payout_atoms_per_claim)
        }
        ResolutionSpecV1::GradedSuccess {
            result_numerator,
            result_denominator,
            payout_scale,
            payout_atoms_per_claim,
        } => {
            let _ = decimal_i128(result_numerator, "graded result numerator")?;
            if decimal_u64(result_denominator, "graded result denominator")? == 0 {
                return Err(Error::new("graded result denominator is zero"));
            }
            (
                decimal_u64(payout_scale, "graded payout scale")?,
                payout_atoms_per_claim,
            )
        }
        ResolutionSpecV1::GradedFailure {
            payout_scale,
            payout_atoms_per_claim,
        } => (
            decimal_u64(payout_scale, "failure payout scale")?,
            payout_atoms_per_claim,
        ),
    };
    if payouts.len() != OUTCOME_COUNT || scale == 0 {
        return Err(Error::new("resolution payout width or scale is invalid"));
    }
    let total = payouts.iter().try_fold(0_u64, |sum, payout| {
        sum.checked_add(decimal_u64(payout, "resolution payout")?)
            .ok_or_else(|| Error::new("resolution payout partition overflowed"))
    })?;
    if total != scale {
        return Err(Error::new(
            "resolution payouts do not partition their scale",
        ));
    }
    Ok(())
}

fn validate_operations(
    body: &ScenarioBodyV1,
    wallet_ids: &BTreeSet<String>,
    account_ids: &BTreeSet<String>,
    claim_mints: &BTreeSet<String>,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut prior: Option<&str> = None;
    for (index, operation) in body.operations.iter().enumerate() {
        let expected_order =
            u32::try_from(index).map_err(|_| Error::new("operation index exceeded u32"))?;
        let expected_dependencies = prior.into_iter().map(str::to_owned).collect::<Vec<_>>();
        if operation.order != expected_order
            || operation.predecessor_id.as_deref() != prior
            || operation.dependency_ids != expected_dependencies
            || !ids.insert(operation.id.clone())
            || !evidence.insert(operation.evidence_output_ref.clone())
            || !wallet_ids.contains(&operation.fee_payer_wallet_ref)
            || operation.caller_target.is_empty()
        {
            return Err(Error::new(
                "operation order, dependency, or identity is invalid",
            ));
        }
        match operation.caller_availability {
            CallerAvailabilityV1::PublicExecutable
                if operation
                    .caller_schema
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                    && operation.mutation_expected => {}
            CallerAvailabilityV1::PreflightOnly
                if operation
                    .caller_schema
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                    && !operation.mutation_expected => {}
            CallerAvailabilityV1::AdapterRequired
                if operation.caller_schema.is_none() && !operation.mutation_expected => {}
            _ => {
                return Err(Error::new(
                    "caller availability, schema, and mutation truth disagree",
                ));
            }
        }
        if operation.capture.signature.is_some()
            || operation.capture.finalized_slot.is_some()
            || operation.capture.transaction_fee_lamports.is_some()
        {
            return Err(Error::new(
                "scenario-only operation carried live capture evidence",
            ));
        }
        if operation.mutation_expected {
            if operation.expected_observed_delta != operation.projected_accepted_delta {
                return Err(Error::new("mutating operation hid its projected delta"));
            }
        } else if operation.expected_observed_delta != LedgerDeltaV1::empty() {
            return Err(Error::new(
                "non-mutating operation carried an observed ledger delta",
            ));
        }
        validate_delta(&operation.projected_accepted_delta, wallet_ids, account_ids)?;
        if let OperationInputV1::Direct {
            outcome,
            claim_mint_ref,
            fill_atoms,
            remaining_before_atoms,
            remaining_after_atoms,
            completion,
            price_scale_atoms,
            execution_price_atoms,
            gross_collateral_atoms,
            fee_basis_points,
            seller_fee_atoms,
            buyer_fee_atoms,
            seller_net_atoms,
            buyer_debit_atoms,
            fee_recipient_credit_atoms,
            seller_wallet_ref,
            buyer_wallet_ref,
        } = &operation.input
        {
            if !wallet_ids.contains(seller_wallet_ref)
                || !wallet_ids.contains(buyer_wallet_ref)
                || seller_wallet_ref == buyer_wallet_ref
                || usize::try_from(*outcome)
                    .ok()
                    .filter(|value| *value < OUTCOME_COUNT)
                    .is_none()
                || !claim_mints.contains(claim_mint_ref)
                || *fee_basis_points != DEVNET_FEE_BASIS_POINTS_V1
            {
                return Err(Error::new("Direct role, outcome, fee, or Mint changed"));
            }
            let fill = decimal_u64(fill_atoms, "Direct fill")?;
            let before = decimal_u64(remaining_before_atoms, "Direct remaining before")?;
            let after = decimal_u64(remaining_after_atoms, "Direct remaining after")?;
            if before.checked_sub(fill) != Some(after)
                || (*completion == FillCompletionV1::Full) != (after == 0)
            {
                return Err(Error::new("Direct fill completion arithmetic changed"));
            }
            let scale = decimal_u64(price_scale_atoms, "Direct price scale")?;
            let price = decimal_u64(execution_price_atoms, "Direct execution price")?;
            let gross = exact_quote(fill, price, scale)?;
            let seller_fee = fee_floor(gross, *fee_basis_points)?;
            let buyer_fee = fee_floor(gross, *fee_basis_points)?;
            let seller_net = gross
                .checked_sub(seller_fee)
                .ok_or_else(|| Error::new("Direct seller net underflowed"))?;
            let buyer_debit = gross
                .checked_add(buyer_fee)
                .ok_or_else(|| Error::new("Direct buyer debit overflowed"))?;
            let fee_credit = seller_fee
                .checked_add(buyer_fee)
                .ok_or_else(|| Error::new("Direct fee credit overflowed"))?;
            if decimal_u64(gross_collateral_atoms, "Direct gross")? != gross
                || decimal_u64(seller_fee_atoms, "Direct seller fee")? != seller_fee
                || decimal_u64(buyer_fee_atoms, "Direct buyer fee")? != buyer_fee
                || decimal_u64(seller_net_atoms, "Direct seller net")? != seller_net
                || decimal_u64(buyer_debit_atoms, "Direct buyer debit")? != buyer_debit
                || decimal_u64(fee_recipient_credit_atoms, "Direct fee credit")? != fee_credit
            {
                return Err(Error::new(
                    "Direct exact quote or side-floor arithmetic changed",
                ));
            }
        }
        prior = Some(operation.id.as_str());
    }
    Ok(())
}

fn validate_delta(
    delta: &LedgerDeltaV1,
    wallet_ids: &BTreeSet<String>,
    account_ids: &BTreeSet<String>,
) -> Result<()> {
    let mut tokens = BTreeSet::new();
    for row in &delta.token_deltas {
        if !account_ids.contains(&row.account_ref)
            || !tokens.insert(row.account_ref.clone())
            || row.before_state != AccountPresenceV1::Present
            || row.after_state != AccountPresenceV1::Present
        {
            return Err(Error::new(
                "token delta aliases or names an invalid account",
            ));
        }
        if let Some(wallet) = &row.wallet_ref
            && !wallet_ids.contains(wallet)
        {
            return Err(Error::new("token delta names an unknown wallet"));
        }
        let _ = decimal_i128(&row.delta_atoms, "token delta")?;
    }
    let mut states = BTreeSet::new();
    for row in &delta.account_state_changes {
        if !account_ids.contains(&row.account_ref)
            || !states.insert(row.account_ref.clone())
            || row.before == row.after
        {
            return Err(Error::new("account state delta aliases or is inert"));
        }
    }
    let mut positions = BTreeSet::new();
    for row in &delta.position_changes {
        let before = decimal_u64(&row.before_revision, "Position pre-revision")?;
        let after = decimal_u64(&row.after_revision, "Position post-revision")?;
        if !account_ids.contains(&row.position_account_ref)
            || !positions.insert(row.position_account_ref.clone())
            || before.checked_add(1) != Some(after)
            || row.before_data_sha256.is_some()
            || row.after_data_sha256.is_some()
        {
            return Err(Error::new("Position revision delta is invalid"));
        }
    }
    if !delta.lamport_deltas.is_empty() {
        return Err(Error::new(
            "scenario-only fixture cannot project runtime lamport/rent deltas",
        ));
    }
    Ok(())
}

fn validate_snapshot(snapshot: &LedgerSnapshotV1, account_ids: &BTreeSet<String>) -> Result<()> {
    let mut states = BTreeSet::new();
    for row in &snapshot.account_states {
        if !account_ids.contains(&row.account_ref) || !states.insert(row.account_ref.clone()) {
            return Err(Error::new(
                "snapshot account state is missing or duplicated",
            ));
        }
    }
    if states != *account_ids {
        return Err(Error::new(
            "snapshot does not close over every logical account",
        ));
    }
    let mut tokens = BTreeSet::new();
    for row in &snapshot.token_balances {
        if !account_ids.contains(&row.account_ref)
            || !tokens.insert(row.account_ref.clone())
            || row.mint_ref.is_empty()
        {
            return Err(Error::new("snapshot token row is invalid or duplicated"));
        }
        let _ = decimal_u64(&row.atoms, "snapshot token balance")?;
    }
    let mut positions = BTreeSet::new();
    for row in &snapshot.position_revisions {
        if !account_ids.contains(&row.position_account_ref)
            || !positions.insert(row.position_account_ref.clone())
        {
            return Err(Error::new("snapshot Position row is invalid or duplicated"));
        }
        let _ = decimal_u64(&row.revision, "snapshot Position revision")?;
    }
    Ok(())
}

fn resolution_spec(resolution: ResolutionDefinition) -> ResolutionSpecV1 {
    let payouts = resolution.payouts().iter().map(u64::to_string).collect();
    match resolution {
        ResolutionDefinition::Categorical { selector } => ResolutionSpecV1::Categorical {
            selector: u32::try_from(selector).unwrap_or_default(),
            payout_atoms_per_claim: payouts,
        },
        ResolutionDefinition::GradedFailure { payout_scale, .. } => {
            ResolutionSpecV1::GradedFailure {
                payout_scale: payout_scale.to_string(),
                payout_atoms_per_claim: payouts,
            }
        }
    }
}

fn exact_quote(fill: u64, price: u64, scale: u64) -> Result<u64> {
    if scale == 0 {
        return Err(Error::new("Direct price scale is zero"));
    }
    let product = u128::from(fill)
        .checked_mul(u128::from(price))
        .ok_or_else(|| Error::new("Direct quote multiplication overflowed"))?;
    let denominator = u128::from(scale);
    if product % denominator != 0 {
        return Err(Error::new(
            "Direct quote is not integral at the named boundary",
        ));
    }
    u64::try_from(product / denominator).map_err(|_| Error::new("Direct quote exceeded u64"))
}

fn fee_floor(gross: u64, basis_points: u16) -> Result<u64> {
    if basis_points > DIRECT_FEE_DENOMINATOR_V1 {
        return Err(Error::new("Direct fee exceeds its denominator"));
    }
    let product = u128::from(gross)
        .checked_mul(u128::from(basis_points))
        .ok_or_else(|| Error::new("Direct fee multiplication overflowed"))?;
    u64::try_from(product / u128::from(DIRECT_FEE_DENOMINATOR_V1))
        .map_err(|_| Error::new("Direct fee exceeded u64"))
}

fn decimal_u64(value: &str, label: &str) -> Result<u64> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(Error::new(format!("{label} is not canonical decimal u64")));
    }
    value
        .parse::<u64>()
        .map_err(|_| Error::new(format!("{label} is not decimal u64")))
}

fn decimal_i128(value: &str, label: &str) -> Result<i128> {
    if value.is_empty()
        || value == "-0"
        || value.starts_with('+')
        || (value.len() > 1 && value.starts_with('0'))
        || (value.len() > 2 && value.starts_with("-0"))
    {
        return Err(Error::new(format!("{label} is not canonical decimal i128")));
    }
    value
        .parse::<i128>()
        .map_err(|_| Error::new(format!("{label} is not decimal i128")))
}

fn signed_decimal(value: i128) -> String {
    value.to_string()
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(nibble: u8) -> char {
    char::from(if nibble < 10 {
        b'0' + nibble
    } else {
        b'a' + (nibble - 10)
    })
}

fn wallet_ref(wallet: &str) -> String {
    format!("wallet.{wallet}")
}

fn collateral_ref(scenario: &str, wallet: &str) -> String {
    format!("token.{scenario}.{wallet}.collateral")
}

fn claim_ref(scenario: &str, wallet: &str, outcome: usize) -> String {
    format!("token.{scenario}.{wallet}.claim.{outcome}")
}

fn position_ref(scenario: &str, wallet: &str) -> String {
    format!("position.{scenario}.{wallet}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OperationInputV1;

    fn manifest(id: &str) -> ManifestEnvelopeV1 {
        let bytes = canonical_manifest_bytes(id).expect("canonical manifest");
        serde_json::from_slice(&bytes).expect("parse canonical manifest")
    }

    #[test]
    fn all_three_scenarios_are_canonical_and_cover_required_economics() {
        let set = canonical_manifest_set().expect("canonical set");
        assert_eq!(set.len(), 3);
        for bytes in set.values() {
            let parsed: ManifestEnvelopeV1 = serde_json::from_slice(bytes).expect("manifest");
            validate_manifest(&parsed).expect("canonical validation");
            assert!(parsed.body.retire_eligible);
            assert_eq!(parsed.body.market.outcome_count, 4);
            assert_eq!(parsed.body.market.fee_basis_points_per_side, 50);
            assert!(parsed.body.wallets.len() >= 5);
            assert!(parsed.body.operations.iter().any(|operation| {
                matches!(
                    operation.input,
                    OperationInputV1::Direct {
                        completion: FillCompletionV1::Partial,
                        ..
                    }
                )
            }));
            assert!(parsed.body.operations.iter().any(|operation| {
                matches!(
                    operation.input,
                    OperationInputV1::Direct {
                        completion: FillCompletionV1::Full,
                        ..
                    }
                )
            }));
            assert!(parsed.body.operations.iter().all(|operation| {
                operation.kind != OperationKindV1::Direct || !operation.mutation_expected
            }));
        }
        let abandoned = manifest("abandoned-graded-failure");
        assert!(matches!(
            abandoned.body.market.resolution,
            ResolutionSpecV1::GradedFailure { .. }
        ));
    }

    #[test]
    fn overflow_and_nonintegral_rounding_refuse() {
        let overflow = exact_quote(u64::MAX, u64::MAX, 1);
        assert!(overflow.is_err());
        let rounding = exact_quote(3, 1, 2);
        assert!(rounding.is_err());

        let mut hostile = manifest("flagship-four-outcome");
        let direct = hostile
            .body
            .operations
            .iter_mut()
            .find(|operation| operation.kind == OperationKindV1::Direct)
            .expect("Direct operation");
        if let OperationInputV1::Direct {
            execution_price_atoms,
            ..
        } = &mut direct.input
        {
            *execution_price_atoms = "251".to_owned();
        }
        hostile.body_sha256 = hex_sha256(&serde_json::to_vec(&hostile.body).expect("body"));
        let error = validate_manifest(&hostile).expect_err("nonintegral quote must refuse");
        assert!(error.to_string().contains("not integral"));
    }

    #[test]
    fn mixed_mint_and_duplicate_rows_refuse() {
        let mut mixed = manifest("flagship-four-outcome");
        let direct = mixed
            .body
            .operations
            .iter_mut()
            .find(|operation| operation.kind == OperationKindV1::Direct)
            .expect("Direct operation");
        if let OperationInputV1::Direct { claim_mint_ref, .. } = &mut direct.input {
            *claim_mint_ref = mixed.body.market.collateral_mint_ref.clone();
        }
        mixed.body_sha256 = hex_sha256(&serde_json::to_vec(&mixed.body).expect("body"));
        let error = validate_manifest(&mixed).expect_err("mixed Mint must refuse");
        assert!(error.to_string().contains("Mint"));

        let mut duplicate = manifest("graduation-four-outcome");
        let first = duplicate
            .body
            .accounts
            .first()
            .expect("first account")
            .id
            .clone();
        duplicate
            .body
            .accounts
            .get_mut(1)
            .expect("second account")
            .id = first;
        duplicate.body_sha256 = hex_sha256(&serde_json::to_vec(&duplicate.body).expect("body"));
        let error = validate_manifest(&duplicate).expect_err("duplicate account must refuse");
        assert!(error.to_string().contains("duplicate logical account"));
    }

    #[test]
    fn cross_kind_bound_address_alias_refuses() {
        let mut hostile = manifest("abandoned-graded-failure");
        hostile
            .body
            .accounts
            .first_mut()
            .expect("first account")
            .address = Some("same-address".to_owned());
        hostile
            .body
            .accounts
            .get_mut(1)
            .expect("second account")
            .address = Some("same-address".to_owned());
        hostile.body_sha256 = hex_sha256(&serde_json::to_vec(&hostile.body).expect("body"));
        let error = validate_manifest(&hostile).expect_err("address alias must refuse");
        assert!(error.to_string().contains("alias"));
    }

    #[test]
    fn fresh_directory_write_and_exact_check_are_reproducible() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let output = temporary.path().join("fixtures");
        write_fixture_directory(&output).expect("atomic fixture directory");
        check_fixture_directory(&output).expect("exact fixture check");
        let error = write_fixture_directory(&output).expect_err("existing output must refuse");
        assert!(error.to_string().contains("already exists"));
    }
}
