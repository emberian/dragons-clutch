//! The external-cluster campaign driver.
//!
//! `runtime.rs` is the *supervisor*: it starts a validator, owns an ephemeral
//! authority that dies with the process, and drives a chain it created. That
//! shape is loopback-only for good structural reasons and stays that way.
//!
//! This is the other shape, the one `docs/evidence/DEVNET_SMOKE_0.md` W3 records
//! as absent: a driver that launches nothing, signs with keys an operator holds
//! on disk, and reaches a cluster it did not create. What it inherits from the
//! supervisor is everything that matters — the same plan producer, the same
//! instruction builders, the same poststate verifiers, the same founding ladder.
//! It is a different *entry*, not a second implementation. (`market.rs`'s
//! `execute_found_market` already takes only `&mut Rpc`, a plan, an authority
//! and a forge; it never asked where the chain came from.)
//!
//! # The four rails, and what each one is for
//!
//! 1. **Origin.** [`crate::cluster`] admits loopback with no ceremony and a
//!    non-loopback origin only against a typed acknowledgment naming devnet's
//!    genesis hash, then re-checks the chain's own answer at connect. Mainnet is
//!    refused unconditionally at three independent points. The supervisor's
//!    `127.0.0.1` rail is preserved *as a rail* while ceasing to be the only
//!    way to state it.
//! 2. **Reads before writes.** `--execute` is opt-in. Without it the connection
//!    is [`crate::rpc::WritePolicyV1::ReadsOnly`], which is enforced by a method
//!    allowlist at the single call site every request passes through — so a
//!    preflight *cannot* write, rather than intending not to.
//! 3. **Pacing.** SMOKE-0 friction 1 measured one busy writer starving every
//!    other request from the same IP, a 1-per-20-second poll included. Every
//!    call on a devnet connection waits its turn, and this driver is a single
//!    sequential writer by construction: it never holds two write buffers open
//!    and never fans out.
//! 4. **Resumability.** Devnet dies mid-ladder — SMOKE-0 measured exactly that
//!    and resumed into the same buffer. So every stage here detects its own
//!    completion *by reading the chain*, never from a local state file. A state
//!    file can disagree with the chain; the chain cannot disagree with itself.
//!    Re-running the driver after any failure is always safe and always the
//!    right move.
//!
//! # What this driver does NOT do, deliberately
//!
//! It does not deploy programs, and it has no code path that could. Deployment
//! is `solana program deploy`'s job, it is the act that parks ~31.7 SOL of rent,
//! and under `docs/decisions/0012-devnet-iteration-substrate.md` it is a
//! *mutable* deploy that is then iterated by `Upgrade`. What the driver owes
//! that decision is the other half: [`substrate_state`] reads each role's
//! observed deployment slot and upgrade authority and compares them to what the
//! plan pinned, and requires Loader ownership and non-executable ProgramData
//! shape. Under 0012 a moved slot is not a deploy error; any mismatch is the
//! fail-closed condition every open market is already in.
//!
//! # Transport, and where SMOKE-0's 100× actually applies
//!
//! SMOKE-0 §3.1 measured TPU submission at ~100× `--use-rpc` for **buffer
//! writes**, and §6.4 says the rest in its own words: "the founding ladder +
//! life are RPC-shaped end to end." The 100× belongs to the ~1,310-write buffer
//! ladder, which is the CLI's, not this driver's. Re-implementing a QUIC TPU
//! client here to submit the founding's ~116 sequential transactions — each of
//! which must be confirmed before the next is built — would buy nothing the
//! measurement supports and would put a second transaction transport in a tool
//! that has one. So: [`deploy_ladder`] emits the exact `solana program` command
//! ladder with TPU as the default and `--use-rpc` as the named fallback, and the
//! driver's own traffic is paced RPC. The transport policy is stated, tested,
//! and printed; it is not silently assumed.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{ErrorKind, Write as _},
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use dclutch_pyth_svm::devnet_release_v1;
use dclutch_registry_contract::{
    ACTIVATION_CACHE_BUMP_OFFSET_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1,
    ActivatedExecutionReleaseSetV1, ActivatedExecutionReleaseSetViewV1, ActivationCacheProgressV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, activation_cache_progress_v1,
};
use dclutch_registry_svm::{
    LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataMetadataV3View, ProgramDataV3View,
    ProgramV3View,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_ORDER_V1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
};
use serde::{
    Deserialize,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use solana_sdk::{
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, MAINNET_BETA_GENESIS_HASH,
    },
    model::{
        CheckedDeploymentDispositionV1, CheckedLocalMutableRolePinV1, CheckedUpgradeRolePinV1,
        ProgramPin, SuccessorPlan,
    },
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, WritePolicyV1, account_evidence, parse_json_without_duplicate_keys_v1},
    runtime,
    seed::{KeyForge, role},
};

/// The acknowledgment flag's literal spelling, for the argument table.
pub(crate) const DEVNET_ACKNOWLEDGMENT_FLAG_NAME: &str = DEVNET_ACKNOWLEDGMENT_FLAG;

const CAMPAIGN_REPORT_SCHEMA_V1: &str = "dclutch-successor-campaign-report-v1";
const MAX_CAMPAIGN_REPORT_BYTES_V1: usize = 16 * 1024 * 1024;

/// The only terminal-consumable projection of this module's campaign report.
///
/// The campaign emitter and this parser deliberately live together. Other
/// clients may reject an unrelated envelope cheaply, but they must not grow a
/// second list of the exact execution, transaction, Market, or account fields.
#[derive(Clone, Debug)]
pub(crate) struct CampaignTerminalEvidenceV1 {
    pub(crate) plan_sha256: String,
    pub(crate) market_sha256: String,
    pub(crate) founding_custody_context: String,
    pub(crate) direct_selected_manifest_entry_index: u16,
    pub(crate) accounts: BTreeMap<String, CampaignAccountEvidenceV1>,
    /// The Direct capability root coordinate, when the sealed report carries
    /// it only as a founding-checkpoint scalar rather than an accounts row.
    /// Founding-only devnet campaigns seal exactly that shape. This is a
    /// routing coordinate, never account authority: every consumer re-reads
    /// and re-authenticates the root account from finalized chain state.
    pub(crate) checkpoint_direct_capability_root: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignAccountEvidenceV1 {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalExecutionV1 {
    completed: bool,
    recovered_finalized_founding: bool,
    transactions: Vec<TerminalTransactionEvidenceV1>,
    market: Option<TerminalMarketEvidenceV1>,
    local_participant_fixture_liquidity:
        Option<crate::market::LocalParticipantFixtureLiquidityEvidenceV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalTransactionEvidenceV1 {
    label: String,
    signature: String,
    slot: u64,
    transaction_metadata_available: bool,
    fee_lamports: NullableV1<u64>,
    fee_only_balance_change: NullableV1<bool>,
    compute_units_consumed: NullableV1<u64>,
    error: Value,
    logs: Vec<String>,
}

/// Required JSON field whose value may itself be null.
struct NullableV1<T>(Option<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NullableV1<T> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalMarketEvidenceV1 {
    completed: Vec<String>,
    accounts: BTreeMap<String, CampaignAccountEvidenceV1>,
    founding_custody_context: String,
    direct_selected_manifest_entry_index: u16,
}

fn authenticate_local_participant_fixture_evidence_v1(
    expected_cluster: crate::cluster::ExpectedClusterV1,
    receipt: Option<&crate::market::LocalParticipantFixtureLiquidityEvidenceV1>,
    transactions: &[TerminalTransactionEvidenceV1],
    market: &TerminalMarketEvidenceV1,
) -> Result<()> {
    match (expected_cluster, receipt) {
        (crate::cluster::ExpectedClusterV1::Devnet, None) => return Ok(()),
        (crate::cluster::ExpectedClusterV1::Devnet, Some(_)) => {
            return Err(Error::new(
                "public devnet campaign carried forbidden local participant fixture liquidity",
            ));
        }
        (crate::cluster::ExpectedClusterV1::OwnedLoopback, None) => {
            return Err(Error::new(
                "owned-loopback founding omitted local participant fixture liquidity",
            ));
        }
        (crate::cluster::ExpectedClusterV1::OwnedLoopback, Some(_)) => {}
    }
    let receipt = receipt.ok_or_else(|| Error::new("fixture receipt disappeared"))?;
    let expected_total = receipt
        .founding_collateral_atoms
        .checked_add(receipt.quantity_atoms)
        .ok_or_else(|| Error::new("fixture receipt supply overflow"))?;
    // THE FIXTURE LIQUIDITY IS PINNED; THE MARKET'S OWN COLLATERAL IS NOT.
    //
    // `quantity_atoms` is the lab fixture's, and one exact amount is the whole
    // point of it: a caller-chosen fixture supply would be a hidden multiplier,
    // and `validate_market_input` refuses one for the same reason.
    //
    // `founding_collateral_atoms` was pinned to the same literal
    // `demo_market_input_base` used to hard-code, and that made the collateral
    // knob FOUNDABLE BUT NOT CONSUMABLE: a market opened at any other stake
    // stood on the chain perfectly well and then every driver that
    // authenticates its campaign report -- the Direct trade producer, the
    // wallet terminal payout -- refused it here, at a constant, with a sentence
    // about a "changed supply" that was really about a changed market. Measured
    // by founding four markets at four stakes and being refused by all of them.
    //
    // What is left is the arithmetic that actually binds: the total supply is
    // the founding collateral plus the fixture liquidity and nothing else, so a
    // receipt cannot claim a stake its own mint does not carry. A zero stake is
    // still refused, exactly as `validate_market_input` refuses one.
    if receipt.quantity_atoms != crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
        || receipt.founding_collateral_atoms == 0
        || receipt.total_supply_atoms != expected_total
        || !receipt.mint_authority_removed
        || receipt.finalized_slot == 0
        || receipt.compute_units_consumed == 0
        || receipt.source_token_account.parse::<Pubkey>().is_err()
        || receipt.source_owner.parse::<Pubkey>().is_err()
        || receipt.mint.parse::<Pubkey>().is_err()
        || receipt
            .transaction_signature
            .parse::<solana_sdk::signature::Signature>()
            .is_err()
    {
        return Err(Error::new(
            "local participant fixture receipt changed its exact supply or finalized identity",
        ));
    }
    let source = market
        .accounts
        .get("local_participant_fixture_source")
        .ok_or_else(|| Error::new("fixture receipt omitted its source account evidence"))?;
    let mint = market
        .accounts
        .get("collateral_mint")
        .ok_or_else(|| Error::new("fixture receipt omitted its mint account evidence"))?;
    if source.address != receipt.source_token_account || mint.address != receipt.mint {
        return Err(Error::new(
            "fixture receipt address differs from the founding account projection",
        ));
    }
    let transaction = transactions
        .iter()
        .find(|transaction| transaction.signature == receipt.transaction_signature)
        .ok_or_else(|| Error::new("fixture receipt signature is absent from campaign history"))?;
    if transaction.slot != receipt.finalized_slot
        || transaction.compute_units_consumed.0 != Some(receipt.compute_units_consumed)
        || transaction.error != Value::Null
    {
        return Err(Error::new(
            "fixture receipt signature does not name its exact finalized transaction",
        ));
    }
    Ok(())
}

pub(crate) fn parse_campaign_terminal_evidence_v1(
    source: &[u8],
) -> Result<CampaignTerminalEvidenceV1> {
    parse_campaign_terminal_evidence_with_expected_cluster_v1(
        source,
        crate::cluster::ExpectedClusterV1::Devnet,
    )
}

pub(crate) fn parse_campaign_terminal_evidence_with_expected_cluster_v1(
    source: &[u8],
    expected_cluster: crate::cluster::ExpectedClusterV1,
) -> Result<CampaignTerminalEvidenceV1> {
    if source.is_empty() || source.len() > MAX_CAMPAIGN_REPORT_BYTES_V1 {
        return Err(Error::new(
            "campaign report is outside the 1..16777216 byte bound",
        ));
    }
    let report: Value = parse_json_without_duplicate_keys_v1(source)?;
    if report.get("schema").and_then(Value::as_str) != Some(CAMPAIGN_REPORT_SCHEMA_V1) {
        return Err(Error::new(
            "terminal evidence is not dclutch-successor-campaign-report-v1",
        ));
    }
    let expected_label = match expected_cluster {
        crate::cluster::ExpectedClusterV1::Devnet => "devnet",
        crate::cluster::ExpectedClusterV1::OwnedLoopback => "loopback",
    };
    if report.get("cluster").and_then(Value::as_str) != Some(expected_label)
        || report.get("mode").and_then(Value::as_str) != Some("execute")
    {
        return Err(Error::new(match expected_cluster {
            crate::cluster::ExpectedClusterV1::Devnet => {
                "terminal evidence requires an executed external devnet campaign; loopback and preflight reports are non-consumable"
            }
            crate::cluster::ExpectedClusterV1::OwnedLoopback => {
                "terminal evidence requires an executed owned loopback campaign; external and preflight reports are non-consumable"
            }
        }));
    }
    let genesis_hash = report
        .get("genesis_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("campaign report omitted genesis_hash"))?;
    let parsed_genesis = genesis_hash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("campaign genesis_hash: {error}")))?;
    if parsed_genesis == Hash::default()
        || genesis_hash == MAINNET_BETA_GENESIS_HASH
        || match expected_cluster {
            crate::cluster::ExpectedClusterV1::Devnet => genesis_hash != DEVNET_GENESIS_HASH,
            crate::cluster::ExpectedClusterV1::OwnedLoopback => false,
        }
    {
        return Err(Error::new(
            "campaign report genesis_hash does not match its admitted cluster identity",
        ));
    }
    let plan_sha256 = report
        .get("plan_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("campaign report omitted plan_sha256"))?
        .to_owned();
    hex32(&plan_sha256)?;
    let market_sha256 = report
        .get("market_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("campaign report omitted market_sha256"))?
        .to_owned();
    hex32(&market_sha256)?;
    let execution: TerminalExecutionV1 = serde_json::from_value(
        report
            .get("execution")
            .cloned()
            .ok_or_else(|| Error::new("campaign report omitted execution"))?,
    )
    .map_err(|error| Error::new(format!("campaign execution: {error}")))?;
    if !execution.completed {
        return Err(Error::new(
            "campaign report does not carry completed execution",
        ));
    }
    if execution.recovered_finalized_founding {
        return Err(Error::new(
            "crash-recovered founding evidence is non-consumable; a separate recovery-to-complete step must reconstruct and authenticate execution.market before terminal use",
        ));
    }
    if execution.transactions.len() > 4_096 {
        return Err(Error::new(
            "campaign report transaction projection exceeds 4096 rows",
        ));
    }
    // A transaction row's identity is its signature. Labels are display
    // routing only and legitimately repeat: the record-publication writer
    // emits one Begin/Append/Finalize row per published record, and every
    // label consumer matches prefixes rather than keying rows by label.
    let mut transaction_signatures = BTreeSet::new();
    for transaction in &execution.transactions {
        if transaction.label.is_empty()
            || transaction.label.len() > 512
            || !transaction_signatures.insert(transaction.signature.as_str())
            || solana_sdk::signature::Signature::from_str(&transaction.signature).is_err()
            || transaction.logs.len() > 512
            || transaction
                .logs
                .iter()
                .any(|line| line.as_bytes().len() > 4_096)
        {
            return Err(Error::new(
                "campaign report transaction projection is noncanonical",
            ));
        }
        // Reading every field here makes the required-field contract explicit;
        // none of these physical observations is treated as semantic authority.
        let _routing_only_physical_facts = (
            transaction.slot,
            transaction.transaction_metadata_available,
            &transaction.fee_lamports.0,
            &transaction.fee_only_balance_change.0,
            &transaction.compute_units_consumed.0,
            &transaction.error,
        );
    }
    let market = execution
        .market
        .ok_or_else(|| Error::new("campaign report omitted execution.market"))?;
    authenticate_local_participant_fixture_evidence_v1(
        expected_cluster,
        execution.local_participant_fixture_liquidity.as_ref(),
        &execution.transactions,
        &market,
    )?;
    let mut completed_names = BTreeSet::new();
    if market.completed.is_empty()
        || market.completed.len() > 512
        || market.completed.iter().any(|stage| {
            stage.is_empty() || stage.len() > 512 || !completed_names.insert(stage.as_str())
        })
    {
        return Err(Error::new(
            "campaign report market completion list is noncanonical",
        ));
    }
    hex32(&market.founding_custody_context)?;
    if market.accounts.is_empty() || market.accounts.len() > 4_096 {
        return Err(Error::new(
            "campaign report account projection is outside the 1..4096 row bound",
        ));
    }
    if market.accounts.contains_key("terminal_record") {
        return Err(Error::new(
            "campaign report carries a stale terminal_record row; live Core terminal_receipt is the sole terminal certificate identity",
        ));
    }
    for (label, row) in &market.accounts {
        if label.is_empty() || label.len() > 128 {
            return Err(Error::new("campaign report account label is not bounded"));
        }
        pubkey(&row.address)?;
        pubkey(&row.owner)?;
        hex32(&row.data_sha256)?;
        hex32(&row.account_sha256)?;
        let _routing_only_physical_facts = (row.lamports, row.executable);
        if row.data_len > MAX_CAMPAIGN_REPORT_BYTES_V1 {
            return Err(Error::new(format!(
                "campaign account {label} data length is outside the bound"
            )));
        }
    }
    let checkpoint_direct_capability_root = report
        .get("foundingCheckpoint")
        .and_then(|checkpoint| checkpoint.get("direct_capability_root"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(root) = checkpoint_direct_capability_root.as_deref() {
        crate::plan::pubkey(root)?;
    }
    Ok(CampaignTerminalEvidenceV1 {
        plan_sha256,
        market_sha256,
        founding_custody_context: market.founding_custody_context,
        direct_selected_manifest_entry_index: market.direct_selected_manifest_entry_index,
        accounts: market.accounts,
        checkpoint_direct_capability_root,
    })
}

/// The roles a driver run must be handed a keypair file for.
///
/// Exactly the signers the stages below reach. `hostile-authority` is not here:
/// its only job is to prove a refusal, the proof costs a funded wallet and two
/// transaction fees, and a driver that silently demanded a second funded key to
/// run at all would be trading the operator's lamports for evidence they did not
/// ask for. It is opt-in through `--keypair-hostile-authority`.
pub(crate) const ADMIN_REQUIRED_ROLES: &[&str] = &[role::CORE_UPGRADE_AUTHORITY];
pub(crate) const ADMIN_ALLOWED_ROLES: &[&str] =
    &[role::CORE_UPGRADE_AUTHORITY, role::CAMPAIGN_PAYER];

pub(crate) const FOUNDING_REQUIRED_ROLES: &[&str] = &[
    role::CAMPAIGN_PAYER,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
];

/// Every role a `--keypair-<role>` flag may name.
pub(crate) const KEYPAIR_ROLES: &[&str] = &[
    role::CORE_UPGRADE_AUTHORITY,
    role::CAMPAIGN_PAYER,
    role::HOSTILE_AUTHORITY,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
    crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
    crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CampaignModeV1 {
    Administration,
    FoundingOnly,
}

impl CampaignModeV1 {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Administration => "administration",
            Self::FoundingOnly => "founding-only",
        }
    }
}

/// The stages a campaign passes through, in the only order a chain accepts.
///
/// Each one owns two things: a **detector** that reads the chain and says
/// whether it is already done, and (for the stages that write) an executor. The
/// detector is what makes the driver resumable, and it is deliberately the
/// *same* poststate check the supervisor runs after executing the stage — a
/// detector that agreed with a weaker condition than the verifier would let a
/// resumed run skip work that never completed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StageV1 {
    /// The seven roles are deployed and their observed ProgramData matches what
    /// the plan pinned. Never writes: deployment is not this tool's act.
    Substrate,
    /// The nine infrastructure record bodies are finalized at their derived
    /// coordinates.
    Publication,
    /// Core's infrastructure profile exists and verifies.
    Initialize,
    /// On the checked-local rehearsal only: consume the genesis-planted
    /// Registry Buffer in one real Loader upgrade, publish the chain-derived
    /// successor record, and create the V2 profile.  These three writes are
    /// one resumable stage so Activation can never observe a half-flipped
    /// infrastructure generation as admissible.
    Succession,
    /// The release activation cache exists and verifies.
    Activation,
    /// A Market exists, founded and Open.
    Founding,
}

impl StageV1 {
    /// The canonical order. A campaign runs a prefix of this and stops.
    pub(crate) const ORDER: [Self; 6] = [
        Self::Substrate,
        Self::Publication,
        Self::Initialize,
        Self::Succession,
        Self::Activation,
        Self::Founding,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Substrate => "substrate",
            Self::Publication => "publication",
            Self::Initialize => "initialize",
            Self::Succession => "succession",
            Self::Activation => "activation",
            Self::Founding => "founding",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        Self::ORDER
            .into_iter()
            .find(|stage| stage.name() == value)
            .ok_or_else(|| {
                Error::new(format!(
                    "unknown stage {value:?}; the stages are {}",
                    Self::ORDER
                        .iter()
                        .map(|stage| stage.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }
}

/// What one stage's detector found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StageStateV1 {
    /// Nothing of this stage exists on the chain yet.
    Absent,
    /// Some of it exists. Named because a partially published record set is
    /// exactly the shape a devnet outage leaves behind, and it must not read as
    /// either "done" or "untouched".
    Partial(String),
    /// The stage's own poststate verifier passes.
    Complete,
    /// It exists and is WRONG — a different chain, a different plan, or drift.
    /// Never something a resumed run may write over.
    Conflict(String),
}

impl StageStateV1 {
    fn label(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Partial(_) => "partial",
            Self::Complete => "complete",
            Self::Conflict(_) => "conflict",
        }
    }

    fn detail(&self) -> Option<&str> {
        match self {
            Self::Absent | Self::Complete => None,
            Self::Partial(detail) | Self::Conflict(detail) => Some(detail),
        }
    }
}

/// One role's observed deployment, read off the cluster.
#[derive(Clone, Debug)]
pub(crate) struct ObservedRoleV1 {
    pub(crate) role: String,
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    /// `None` when the ProgramData account does not exist at all.
    pub(crate) observed_slot: Option<u64>,
    pub(crate) pinned_slot: u64,
    /// `None` for a revoked (immutable) deployment, which is what the pre-0012
    /// ceremony produces.
    pub(crate) observed_authority: Option<String>,
    pub(crate) pinned_authority: Option<String>,
    /// Account owner observed at the ProgramData coordinate. An existing
    /// ProgramData image is authoritative only under Loader V3 ownership.
    pub(crate) observed_owner: Option<String>,
    /// ProgramData must remain non-executable; the linked Program account is
    /// the executable half of the Loader V3 pair.
    pub(crate) observed_executable: Option<bool>,
    pub(crate) observed_live_elf_sha256: Option<String>,
    pub(crate) pinned_live_elf_sha256: String,
    pub(crate) checked_candidate_elf_sha256: String,
    pub(crate) live_elf_padding_bytes: usize,
    pub(crate) observed_data_len: Option<usize>,
}

impl ObservedRoleV1 {
    /// Whether the observed deployment slot is still the release's slot pin.
    pub(crate) fn slot_pin_holds(&self) -> bool {
        self.observed_slot == Some(self.pinned_slot)
    }

    fn authority_pin_holds(&self) -> bool {
        self.observed_authority == self.pinned_authority
    }

    fn loader_owner_holds(&self) -> bool {
        self.observed_owner.as_deref() == Some(bpf_loader_upgradeable::ID.to_string().as_str())
    }

    /// Exact 0012 substrate pins that an existing ProgramData account must
    /// retain before the driver may write any release-generation state.
    fn pin_conflicts(&self) -> Vec<String> {
        self.pin_conflicts_allowing_forward_slot(false)
    }

    fn pin_conflicts_allowing_forward_slot(&self, allow_forward_slot: bool) -> Vec<String> {
        let mut conflicts = Vec::new();
        let admitted_forward_slot = allow_forward_slot
            && self
                .observed_slot
                .is_some_and(|slot| slot > self.pinned_slot);
        if !self.slot_pin_holds() && !admitted_forward_slot {
            conflicts.push(format!(
                "{} observed slot {} but the release binds {}",
                self.role,
                self.observed_slot
                    .map(|slot| slot.to_string())
                    .unwrap_or_else(|| "none".into()),
                self.pinned_slot
            ));
        }
        let loader = bpf_loader_upgradeable::ID.to_string();
        if !self.loader_owner_holds() {
            conflicts.push(format!(
                "{} ProgramData owner is {} but Loader V3 is {}",
                self.role,
                self.observed_owner.as_deref().unwrap_or("none"),
                loader
            ));
        }
        if self.observed_executable != Some(false) {
            conflicts.push(format!(
                "{} ProgramData executable flag is {:?}, expected false",
                self.role, self.observed_executable
            ));
        }
        if !self.authority_pin_holds() {
            conflicts.push(format!(
                "{} observed upgrade authority {} but the release binds {}",
                self.role,
                self.observed_authority.as_deref().unwrap_or("none"),
                self.pinned_authority.as_deref().unwrap_or("none")
            ));
        }
        if self.observed_live_elf_sha256.as_deref() != Some(self.pinned_live_elf_sha256.as_str()) {
            conflicts.push(format!(
                "{} observed complete live ELF SHA-256 {} but the release binds {}",
                self.role,
                self.observed_live_elf_sha256.as_deref().unwrap_or("none"),
                self.pinned_live_elf_sha256
            ));
        }
        conflicts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActivationComputeProjectionV1 {
    role: &'static str,
    pending: bool,
    live_elf_bytes: u64,
    conservative_compute_units: u64,
    headroom_compute_units: u64,
}

/// Refuse a pending first activation whose authenticated live ELF cannot fit
/// beneath the pinned runtime's transaction ceiling even under a conservative
/// size-only projection. This is key-free and runs before any campaign signer
/// file is opened. It complements, but never replaces, the measured CU gate.
fn activation_compute_preflight_v1(
    observed: &[ObservedRoleV1],
    activated_prefix: usize,
) -> Result<Vec<ActivationComputeProjectionV1>> {
    const ROLES: [&str; 5] = ["core", "claims", "trading", "resolution", "custody"];
    if activated_prefix > ROLES.len() {
        return Err(Error::new(
            "activation compute preflight received an impossible written-role count",
        ));
    }
    let mut projection = Vec::with_capacity(ROLES.len());
    for (ordinal, role) in ROLES.into_iter().enumerate() {
        let row = observed
            .iter()
            .find(|row| row.role == role)
            .ok_or_else(|| Error::new(format!("activation compute preflight omitted {role}")))?;
        let Some(programdata_bytes) = row.observed_data_len else {
            // An absent deployment is already a substrate-stage refusal. It
            // carries no authenticated width to project and cannot reach an
            // activation send.
            continue;
        };
        let live_elf_bytes = programdata_bytes
            .checked_sub(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
            .ok_or_else(|| Error::new(format!("{role} ProgramData is shorter than Loader V3")))?;
        let live_elf_bytes = u64::try_from(live_elf_bytes)
            .map_err(|_| Error::new(format!("{role} live ELF width does not fit u64")))?;
        let conservative_compute_units =
            runtime::activation_compute_upper_bound_v1(live_elf_bytes)?;
        let pending = ordinal >= activated_prefix;
        if pending && conservative_compute_units > runtime::ACTIVATION_TRANSACTION_CU_LIMIT_V1 {
            return Err(Error::new(format!(
                "pending {role} activation is unreachable: authenticated live ELF has \
                 {live_elf_bytes} bytes, above the {}-byte size-only ceiling, projecting \
                 {conservative_compute_units} CU against the {}-CU transaction maximum; rebuild \
                 the exact checked role and rerun the measured CU gate before publishing",
                runtime::MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1,
                runtime::ACTIVATION_TRANSACTION_CU_LIMIT_V1,
            )));
        }
        projection.push(ActivationComputeProjectionV1 {
            role,
            pending,
            live_elf_bytes,
            conservative_compute_units,
            headroom_compute_units: runtime::ACTIVATION_TRANSACTION_CU_LIMIT_V1
                .saturating_sub(conservative_compute_units),
        });
    }
    Ok(projection)
}

/// The command surface, already parsed and validated.
#[derive(Debug)]
pub(crate) struct CampaignArgsV1 {
    pub(crate) origin: ClusterOriginV1,
    pub(crate) mode: CampaignModeV1,
    pub(crate) plan_path: PathBuf,
    /// The market input the founding stage founds — the run spec's `market`
    /// block as its own JSON document. Optional because every earlier stage
    /// runs without one; the founding stage refuses by name when it is absent.
    pub(crate) market_path: Option<PathBuf>,
    pub(crate) evidence_path: Option<PathBuf>,
    /// Optional standalone, chain-rederived infrastructure lineage artifact.
    /// Administration execution writes it only after the requested prefix is
    /// complete. Founding refuses the flag rather than growing a second owner
    /// for the infrastructure facts already fixed by administration.
    pub(crate) infrastructure_lineage_path: Option<PathBuf>,
    /// Public identities only. Neither party signs a founding transaction, so
    /// requiring secret-bearing files for them would manufacture authority the
    /// protocol does not use.
    pub(crate) founding_founder: Option<Pubkey>,
    pub(crate) substituted_founder: Option<Pubkey>,
    /// Paths only. Their contents are first read after the durable key-free
    /// plan and live-substrate preflight has been fsynced.
    pub(crate) keypairs: BTreeMap<String, PathBuf>,
    pub(crate) execute: bool,
    pub(crate) through: StageV1,
}

const GRADUATION_MARKET_INPUT_SCHEMA_V1: &str = "dclutch-graduation-market-input-v1";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GraduationMarketWindowV1 {
    start_unix_seconds: i64,
    end_unix_seconds: i64,
    max_age_seconds: u32,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GraduationMarketInputV1 {
    schema: String,
    market: crate::model::MarketRunInput,
    account_set_id: String,
    relayer_attestation: String,
    relayer_key_set_hex: String,
    relayer_key_set_digest: String,
    venue_release_digest: String,
    relayed_adapter_config_digest: String,
    source_spec_digest: String,
    window: GraduationMarketWindowV1,
    walk_bounty_lamports: u64,
    admitted_principal_atoms: String,
    admitted_principal_cap_atoms: String,
    disclosed_failure_conflation: String,
}

#[derive(Clone, Copy)]
struct ExactMarketJsonValueSeedV1;

impl<'de> DeserializeSeed<'de> for ExactMarketJsonValueSeedV1 {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> core::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ExactMarketJsonValueVisitorV1)
    }
}

struct ExactMarketJsonValueVisitorV1;

impl<'de> Visitor<'de> for ExactMarketJsonValueVisitorV1 {
    type Value = Value;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("one market JSON value with no duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> core::result::Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> core::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> core::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> core::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("market JSON number was not finite"))
    }

    fn visit_str<E>(self, value: &str) -> core::result::Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> core::result::Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> core::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExactMarketJsonValueSeedV1.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(ExactMarketJsonValueSeedV1)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::with_capacity(map.size_hint().unwrap_or(0));
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            let value = map.next_value_seed(ExactMarketJsonValueSeedV1)?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn parse_exact_market_json_v1(bytes: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactMarketJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| Error::new(format!("market input JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| Error::new(format!("market input JSON trailing bytes: {error}")))?;
    Ok(value)
}

/// Decode the campaign's existing bare `MarketRunInput` or the exact envelope
/// emitted by the already-shipped `graduation-market` command.
///
/// Presence of `schema` selects the envelope parser. There is deliberately no
/// untagged/try-one-then-the-other fallback: an envelope with a damaged schema
/// must not be reinterpreted as a different input family. Both structs deny
/// unknown fields, and the graduation envelope is authenticated all the way
/// back into the inner source graph before its market is returned.
pub(crate) fn load_market_input(bytes: &[u8]) -> Result<crate::model::MarketRunInput> {
    // Parse the original bytes with a recursive visitor before any ordinary
    // `Value` normalization can collapse an earlier object member. This is the
    // same refusal boundary as the RPC parser but stays local to the campaign
    // input caller: neither parser makes the other's transport authoritative.
    let value = parse_exact_market_json_v1(bytes)?;
    let input = if value.get("schema").is_some() {
        let wrapped: GraduationMarketInputV1 = serde_json::from_value(value)?;
        authenticate_graduation_market_input_v1(&wrapped)?;
        wrapped.market
    } else {
        serde_json::from_value(value)?
    };
    crate::market::validate_market_input(&input)?;
    Ok(input)
}

fn canonical_hex_32(value: &str, label: &str) -> Result<[u8; 32]> {
    let decoded = runtime::decode_hex(value)?;
    let output: [u8; 32] = decoded.try_into().map_err(|bytes: Vec<u8>| {
        Error::new(format!(
            "graduation {label} must be exactly 32 bytes, not {}",
            bytes.len()
        ))
    })?;
    if hex(&output) != value {
        return Err(Error::new(format!(
            "graduation {label} must use canonical lowercase hex"
        )));
    }
    Ok(output)
}

fn digest_hex(bytes: &[u8]) -> String {
    hex(&<sha2::Sha256 as sha2::Digest>::digest(bytes))
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    <sha2::Sha256 as sha2::Digest>::digest(bytes).into()
}

fn canonical_u128(value: &str, label: &str) -> Result<u128> {
    let parsed = value
        .parse::<u128>()
        .map_err(|_| Error::new(format!("graduation {label} must be an unsigned decimal")))?;
    if parsed.to_string() != value {
        return Err(Error::new(format!(
            "graduation {label} must use canonical decimal spelling"
        )));
    }
    Ok(parsed)
}

fn authenticate_graduation_market_input_v1(input: &GraduationMarketInputV1) -> Result<()> {
    if input.schema != GRADUATION_MARKET_INPUT_SCHEMA_V1 {
        return Err(Error::new(format!(
            "unsupported graduation market input schema {:?}",
            input.schema
        )));
    }
    crate::market::validate_market_input(&input.market)?;

    // Profile-v1 is not an arbitrary market hidden under a trusted-looking
    // wrapper. These are the fixed graduation Product coordinates compiled by
    // `relayed_market_input`; venue, relayer, window and Direct closure remain
    // explicit inputs and are joined below.
    let account_set_id = canonical_hex_32(&input.account_set_id, "account_set_id")?;
    let coordinate_domain =
        crate::market::demo_id("relayed/coordinate-domain/dbc-migration-progress", &[]);
    let result_unit =
        crate::market::demo_id("relayed/result-unit/migration-progress-discriminant", &[]);
    let expected_product =
        crate::market::demo_id("relayed/product/dbc-graduation", &[&account_set_id]);
    let expected_claim_basis = crate::market::demo_id("claim-basis/unit-complete-set", &[]);
    let expected_representation =
        crate::market::demo_id("representation/categorical-fixed-width", &[]);
    let expected_mapping =
        crate::market::demo_id("mapping/scaled-integer-cut", &[&coordinate_domain]);
    if input.market.generation != 1
        || input.market.collateral_display_decimals != 6
        || input.market.local_participant_fixture_liquidity_atoms != 0
        || input.market.initial_collateral_atoms != 1_000_000_000
        || input.market.product_id != hex(&expected_product)
        || input.market.coordinate_domain_id != hex(&coordinate_domain)
        || input.market.result_unit_id != hex(&result_unit)
        || input.market.claim_basis_id != hex(&expected_claim_basis)
        || input.market.representation_release_id != hex(&expected_representation)
        || input.market.mapping_release_id != hex(&expected_mapping)
        || input.market.cut_denominator != 1
        || !input.market.cuts.is_empty()
        || input.market.portfolio_denominator != 1
        || input.market.coefficients != [1, 0]
        || !input.market.recovery_policy_hex.is_empty()
        || input.market.failure_policy_release_id
            != hex(&dclutch_source_contract::SOURCE_FAILURE_POLICY_RELEASE_ID_V2)
    {
        return Err(Error::new(
            "graduation wrapper substituted the fixed profile-v1 market geometry",
        ));
    }

    let source_bytes = runtime::decode_hex(&input.market.source_spec_hex)?;
    if digest_hex(&source_bytes) != input.source_spec_digest
        || input.source_spec_digest != input.market.primary_source_spec_id
    {
        return Err(Error::new(
            "graduation source_spec_digest does not name the inner source body",
        ));
    }
    let source = dclutch_source_contract::SourceSpecV1::decode(&source_bytes)
        .map_err(|error| Error::new(format!("graduation SourceSpecV1: {error:?}")))?;
    if source.domain_id().to_bytes() != coordinate_domain
        || source.unit_id().to_bytes() != result_unit
        || source.access_profile()
            != dclutch_source_contract::SourceAccessProfile::RelayedObservationRecord
    {
        return Err(Error::new(
            "graduation source body is not the relayed profile-v1 Product source",
        ));
    }

    let window_bytes = runtime::decode_hex(&input.market.window_spec_hex)?;
    let window = dclutch_source_contract::WindowSpecV1::decode(&window_bytes)
        .map_err(|error| Error::new(format!("graduation WindowSpecV1: {error:?}")))?;
    if digest_hex(&window_bytes) != input.market.window_spec_id
        || window.kind() != dclutch_source_contract::WindowKind::Terminal
        || window.source_spec_id().to_bytes()
            != canonical_hex_32(&input.source_spec_digest, "source_spec_digest")?
        || window.start_unix_seconds() != input.window.start_unix_seconds
        || window.end_unix_seconds() != input.window.end_unix_seconds
        || window.max_age_seconds() != input.window.max_age_seconds
        || window.max_future_skew_seconds() != 1
        || window.cadence_tolerance_seconds() != 0
    {
        return Err(Error::new(
            "graduation wrapper window does not equal its canonical inner terminal window",
        ));
    }

    let relayer: Pubkey = input
        .relayer_attestation
        .parse()
        .map_err(|_| Error::new("graduation relayer_attestation is not a public key"))?;
    if relayer.to_string() != input.relayer_attestation {
        return Err(Error::new(
            "graduation relayer_attestation must use canonical base58",
        ));
    }
    let key_set_bytes = runtime::decode_hex(&input.relayer_key_set_hex)?;
    let key_set = dclutch_relay_contract::release::RelayerKeySetV1::decode(&key_set_bytes)
        .map_err(|error| Error::new(format!("graduation RelayerKeySetV1: {error:?}")))?;
    let canonical_key_set = key_set
        .to_bytes()
        .map_err(|error| Error::new(format!("graduation RelayerKeySetV1 bytes: {error:?}")))?;
    if canonical_key_set.as_slice() != key_set_bytes
        || key_set.key_count() != 1
        || key_set.seal_threshold() != 1
        || key_set.keys() != [relayer.to_bytes()]
        || digest_hex(&key_set_bytes) != input.relayer_key_set_digest
    {
        return Err(Error::new(
            "graduation relayer key set, attestation key, or digest was substituted",
        ));
    }

    // The adapter configuration is not duplicated as a second body in the
    // wrapper. Recompile it from the wrapper's authenticated set and window,
    // then bind its digest through ProviderReleaseV1. This closes the otherwise
    // invisible account-set/config substitution seam.
    let adapter = dclutch_relay_contract::release::RelayedAdapterConfigV1::new(
        account_set_id,
        0,
        0,
        u64::from(input.window.max_age_seconds),
        crate::relayed::MAX_CLUSTER_SKEW_SECONDS,
    )
    .map_err(|error| Error::new(format!("graduation adapter config: {error:?}")))?;
    let adapter_bytes = adapter
        .to_bytes()
        .map_err(|error| Error::new(format!("graduation adapter config bytes: {error:?}")))?;
    if digest_hex(&adapter_bytes) != input.relayed_adapter_config_digest {
        return Err(Error::new(
            "graduation relayed_adapter_config_digest does not match its set and window",
        ));
    }

    let provider_bytes = runtime::decode_hex(&input.market.provider_release_hex)?;
    let provider = dclutch_source_contract::ProviderReleaseV1::decode(&provider_bytes)
        .map_err(|error| Error::new(format!("graduation ProviderReleaseV1: {error:?}")))?;
    if provider.to_bytes().as_slice() != provider_bytes
        || provider.provider_family_id().to_bytes()
            != dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1
        || provider.adapter_release_id().to_bytes()
            != dclutch_source_contract::RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1
        || provider.provider_deployment_release_id().to_bytes()
            != canonical_hex_32(&input.relayer_key_set_digest, "relayer_key_set_digest")?
        || provider.decoding_rules_id().to_bytes()
            != canonical_hex_32(
                &input.relayed_adapter_config_digest,
                "relayed_adapter_config_digest",
            )?
        || provider.transport_profile_id().to_bytes()
            != dclutch_relay_contract::RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1
        || source.provider_release_id().to_bytes() != sha256_bytes(&provider_bytes)
    {
        return Err(Error::new(
            "graduation provider release does not bind the relayer and adapter digests",
        ));
    }

    let venue_bytes = runtime::decode_hex(&input.market.pyth_adapter_config_hex)?;
    let venue = dclutch_registry_contract::ArtifactReleaseV1::decode(&venue_bytes)
        .map_err(|error| Error::new(format!("graduation venue ArtifactReleaseV1: {error:?}")))?;
    if venue.to_bytes().as_slice() != venue_bytes
        || venue.loader_program().to_bytes()
            != dclutch_relay_contract::identity::LOADER_V3_PROGRAM_ID
        || venue.semantic_release_id().to_bytes()
            != crate::market::demo_id("relayed/venue-semantic-release/meteora-dbc", &[])
        || venue.upgrade_policy()
            != dclutch_registry_contract::ArtifactUpgradePolicyV1::ExactAuthority
        || venue.upgrade_authority().is_none()
        || digest_hex(&venue_bytes) != input.venue_release_digest
        || source.adapter_config_id().to_bytes()
            != canonical_hex_32(&input.venue_release_digest, "venue_release_digest")?
    {
        return Err(Error::new(
            "graduation venue release digest or inner source binding was substituted",
        ));
    }

    // The wrapper's disclosed principal cap is not authority for the Source
    // graph. Authenticate the two immutable records that actually make the
    // graph bounded. In particular, an empty floor is otherwise a valid bare
    // Market input and `compile_market_bodies` deliberately interprets it as
    // `SourceMaterialV3::explicitly_unbounded`; accepting that shape here
    // while retaining the wrapper's 1/4-cap fields would make the disclosure
    // and the Market disagree.
    let capacity_bytes = runtime::decode_hex(&input.market.source_capacity_profile_hex)?;
    let capacity = dclutch_source_contract::SourceCapacityProfileV1::decode(&capacity_bytes)
        .map_err(|error| Error::new(format!("graduation SourceCapacityProfileV1: {error:?}")))?;
    let expected_capacity = dclutch_source_contract::SourceCapacityProfileV1::new(
        dclutch_source_contract::CapacityEnvelope::Provisional,
        1,
        0,
        dclutch_source_contract::ContentId::new(crate::market::demo_id(
            "relayed/capacity/terminal-verifier",
            &[],
        ))
        .map_err(|error| Error::new(format!("graduation capacity verifier: {error:?}")))?,
        dclutch_source_contract::ContentId::new(
            dclutch_source_contract::PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1,
        )
        .map_err(|error| Error::new(format!("graduation capacity lifting plan: {error:?}")))?,
        512,
        4,
    )
    .map_err(|error| Error::new(format!("graduation source capacity: {error:?}")))?
    .bounding_principal(
        dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
        dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
    )
    .map_err(|error| Error::new(format!("graduation source kappa: {error:?}")))?;
    if capacity.to_bytes().as_slice() != capacity_bytes
        || capacity != expected_capacity
        || source.capacity_profile_id().to_bytes() != sha256_bytes(&capacity_bytes)
    {
        return Err(Error::new(
            "graduation SourceCapacityProfileV1 is not the exact selected provisional 1/4 kappa record",
        ));
    }

    let floor_bytes = runtime::decode_hex(&input.market.manipulation_floor_hex)?;
    if floor_bytes.is_empty() {
        return Err(Error::new(
            "graduation Market omitted its exact bounded manipulation floor",
        ));
    }
    let floor = dclutch_source_contract::ManipulationFloorV1::decode(&floor_bytes)
        .map_err(|error| Error::new(format!("graduation ManipulationFloorV1: {error:?}")))?;
    let expected_floor = dclutch_source_contract::ManipulationFloorV1::new(
        dclutch_source_contract::ManipulationFloorBasis::CurveDerived,
        dclutch_source_contract::ContentId::new(sha256_bytes(&source_bytes))
            .map_err(|error| Error::new(format!("graduation source identity: {error:?}")))?,
        source.adapter_config_id(),
        dclutch_source_contract::ContentId::new(crate::market::demo_id(
            "relayed/collateral-unit/realm-native-lamports",
            &[],
        ))
        .map_err(|error| Error::new(format!("graduation collateral unit: {error:?}")))?,
        dclutch_source_contract::ContentId::new(
            dclutch_source_contract::BONDING_CURVE_FLOOR_DERIVATION_ID_V1,
        )
        .map_err(|error| Error::new(format!("graduation floor derivation: {error:?}")))?,
        dclutch_source_contract::BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
    );
    if floor.to_bytes().as_slice() != floor_bytes || floor != expected_floor {
        return Err(Error::new(
            "graduation ManipulationFloorV1 changed its source, venue, collateral unit, derivation, basis, or floor",
        ));
    }

    let admitted = canonical_u128(&input.admitted_principal_atoms, "admitted_principal_atoms")?;
    let cap = canonical_u128(
        &input.admitted_principal_cap_atoms,
        "admitted_principal_cap_atoms",
    )?;
    let expected_admitted = u128::from(input.market.initial_collateral_atoms / 2);
    let expected_cap = u128::from(dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1)
        * u128::from(dclutch_source_contract::BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1)
        / u128::from(dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1);
    if input.walk_bounty_lamports != crate::relayed::WALK_BOUNTY_LAMPORTS
        || admitted != expected_admitted
        || cap != expected_cap
        || input.disclosed_failure_conflation != crate::relayed::DISCLOSED_FAILURE_CONFLATION
    {
        return Err(Error::new(
            "graduation wrapper substituted its disclosed bounty, principal, cap, or failure policy",
        ));
    }
    let capacity = capacity
        .principal_capacity()
        .map_err(|error| Error::new(format!("graduation source kappa read-back: {error:?}")))?;
    if capacity.admit(floor.floor_atoms(), admitted).is_err()
        || capacity.admit(floor.floor_atoms(), cap).is_err()
        || capacity
            .admit(
                floor.floor_atoms(),
                cap.checked_add(1)
                    .ok_or_else(|| Error::new("graduation principal cap overflow"))?,
            )
            .is_ok()
    {
        return Err(Error::new(
            "graduation principal disclosure does not match the authenticated kappa boundary",
        ));
    }
    Ok(())
}

struct CampaignEvidenceLeaseV1 {
    path: PathBuf,
    parent: PathBuf,
    file: fs::File,
}

impl CampaignEvidenceLeaseV1 {
    fn acquire(evidence_path: &Path) -> Result<Self> {
        let parent = evidence_path
            .parent()
            .ok_or_else(|| Error::new("evidence output requires a parent directory"))?;
        fs::create_dir_all(parent).map_err(|error| {
            Error::new(format!(
                "create evidence directory {}: {error}",
                parent.display()
            ))
        })?;
        let file_name = evidence_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| Error::new("evidence output requires a UTF-8 file name"))?;
        let path = evidence_path.with_file_name(format!("{file_name}.lock"));
        let created_at_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::new("system clock precedes the Unix epoch"))?
            .as_secs();
        let owner = serde_json::to_vec_pretty(&json!({
            "schema": "dclutch-successor-campaign-evidence-lock-v1",
            "pid": std::process::id(),
            "evidence": evidence_path.display().to_string(),
            "createdAtUnixSeconds": created_at_unix_seconds,
            "stalePolicy": "never-auto-remove; confirm no live owner, then remove manually",
        }))?;
        let mut file = match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(Error::new(format!(
                    "campaign evidence is locked at {}; locks are never removed automatically. Confirm that no process owns it before removing a stale lock manually",
                    path.display()
                )));
            }
            Err(error) => {
                return Err(Error::new(format!(
                    "create campaign evidence lock {}: {error}",
                    path.display()
                )));
            }
        };
        let initialize = file
            .write_all(&owner)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::File::open(parent)?.sync_all());
        if let Err(error) = initialize {
            if Self::owns_link(&file, &path) {
                let _ = fs::remove_file(&path);
            }
            return Err(Error::new(format!(
                "initialize campaign evidence lock {}: {error}",
                path.display()
            )));
        }
        if !Self::owns_link(&file, &path) {
            return Err(Error::new(format!(
                "campaign evidence lock {} changed while it was acquired",
                path.display()
            )));
        }
        Ok(Self {
            path,
            parent: parent.to_path_buf(),
            file,
        })
    }

    fn owns_link(file: &fs::File, path: &Path) -> bool {
        let Ok(held) = file.metadata() else {
            return false;
        };
        let Ok(linked) = fs::symlink_metadata(path) else {
            return false;
        };
        held.dev() == linked.dev() && held.ino() == linked.ino()
    }
}

impl Drop for CampaignEvidenceLeaseV1 {
    fn drop(&mut self) {
        if Self::owns_link(&self.file, &self.path) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::File::open(&self.parent).and_then(|directory| directory.sync_all());
        }
    }
}

fn write_evidence_atomically(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("evidence output requires a parent directory"))?;
    fs::create_dir_all(parent).map_err(|error| {
        Error::new(format!(
            "create evidence directory {}: {error}",
            parent.display()
        ))
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("evidence output requires a UTF-8 file name"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("system clock precedes the Unix epoch"))?
        .as_nanos();
    let temporary = path.with_file_name(format!(
        ".{file_name}.dclutch-{}-{nonce}.tmp",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create {}: {error}", temporary.display())))?;
    file.write_all(&bytes)
        .map_err(|error| Error::new(format!("write {}: {error}", temporary.display())))?;
    file.sync_all()
        .map_err(|error| Error::new(format!("fsync {}: {error}", temporary.display())))?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically replace {} from {}: {error}",
            path.display(),
            temporary.display()
        ))
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| Error::new(format!("fsync evidence directory: {error}")))?;
    Ok(())
}

struct PriorCampaignEvidenceV1 {
    checkpoint: Option<crate::market::MarketExecutionCheckpointV1>,
    founding_submission_journals: BTreeMap<
        crate::market::founding_submission_journal::FoundingSubmissionOperationV1,
        crate::market::founding_submission_journal::FoundingSubmissionJournalV1,
    >,
    terminal_consumable_source: Option<Vec<u8>>,
}

fn load_prior_campaign_evidence(
    path: &Path,
    plan_sha256: &str,
    market_sha256: Option<&str>,
    expected_cluster: &str,
    expected_rpc_url: &str,
) -> Result<PriorCampaignEvidenceV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PriorCampaignEvidenceV1 {
                checkpoint: None,
                founding_submission_journals: BTreeMap::new(),
                terminal_consumable_source: None,
            });
        }
        Err(error) => {
            return Err(Error::new(format!(
                "read prior evidence {}: {error}",
                path.display()
            )));
        }
    };
    if bytes.is_empty() || bytes.len() > MAX_CAMPAIGN_REPORT_BYTES_V1 {
        return Err(Error::new(
            "prior campaign evidence is outside the 1..16777216 byte bound",
        ));
    }
    let prior = parse_json_without_duplicate_keys_v1(&bytes)?;
    if prior.get("schema").and_then(Value::as_str) != Some(CAMPAIGN_REPORT_SCHEMA_V1)
        || prior.get("cluster").and_then(Value::as_str) != Some(expected_cluster)
        || prior.get("rpc_url").and_then(Value::as_str) != Some(expected_rpc_url)
        || prior.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256)
        || prior.get("market_sha256").and_then(Value::as_str) != market_sha256
    {
        return Err(Error::new(
            "existing campaign evidence belongs to another schema, cluster, RPC origin, plan, or Market input; it was not replaced",
        ));
    }
    let checkpoint = prior
        .get("foundingCheckpoint")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| Error::new(format!("prior founding checkpoint: {error}")))?;
    let founding_submission_rows = prior
        .get("foundingSubmissionJournals")
        .cloned()
        .map(
            serde_json::from_value::<
                Vec<crate::market::founding_submission_journal::FoundingSubmissionJournalV1>,
            >,
        )
        .transpose()
        .map_err(|error| Error::new(format!("prior founding submission journals: {error}")))?
        .unwrap_or_default();
    crate::market::founding_submission_journal::authenticate_founding_submission_prefix_v1(
        &founding_submission_rows,
    )?;
    let mut founding_submission_journals = BTreeMap::new();
    for journal in founding_submission_rows {
        let operation = journal.operation;
        if founding_submission_journals
            .insert(operation, journal)
            .is_some()
        {
            return Err(Error::new(format!(
                "prior founding evidence duplicated {} journal owner",
                operation.label()
            )));
        }
    }
    let terminal_consumable_source = match prior.get("execution") {
        None => None,
        Some(execution) => {
            let execution: TerminalExecutionV1 = serde_json::from_value(execution.clone())
                .map_err(|error| Error::new(format!("prior campaign execution: {error}")))?;
            if execution.completed
                && !execution.recovered_finalized_founding
                && execution.market.is_some()
                && prior.get("cluster").and_then(Value::as_str) == Some("devnet")
                && prior.get("mode").and_then(Value::as_str) == Some("execute")
            {
                // The canonical parser beside the emitter is the only owner of
                // terminal-consumable report shape. A rerun returns these exact
                // bytes rather than replacing them with a preflight downgrade.
                parse_campaign_terminal_evidence_v1(&bytes)?;
                Some(bytes)
            } else {
                None
            }
        }
    };
    Ok(PriorCampaignEvidenceV1 {
        checkpoint,
        founding_submission_journals,
        terminal_consumable_source,
    })
}

fn authenticate_keypair_paths(
    keypairs: &BTreeMap<String, PathBuf>,
    required: &[&str],
    allowed: &[&str],
) -> Result<()> {
    let missing = required
        .iter()
        .filter(|role| !keypairs.contains_key(**role))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(Error::new(format!(
            "campaign omitted required keypair paths: {}",
            missing.join(", ")
        )));
    }
    let unexpected = keypairs
        .keys()
        .filter(|role| !allowed.contains(&role.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return Err(Error::new(format!(
            "campaign supplied keypair paths outside this mode: {}",
            unexpected.join(", ")
        )));
    }
    let mut paths = BTreeSet::new();
    for (role, path) in keypairs {
        if !path.is_absolute() {
            return Err(Error::new(format!("{role} keypair path must be absolute")));
        }
        if !paths.insert(path) {
            return Err(Error::new(
                "campaign keypair paths must be distinct before any file is read",
            ));
        }
    }
    Ok(())
}

fn founding_keypair_roles_v1(market: &crate::model::MarketRunInput) -> Vec<&'static str> {
    let mut roles = FOUNDING_REQUIRED_ROLES.to_vec();
    if market.local_participant_fixture_liquidity_atoms != 0 {
        roles.extend([
            crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
            crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
        ]);
    }
    roles
}

fn authenticate_founding_actor_partition_v1(
    founding_founder: Pubkey,
    substituted_founder: Pubkey,
    secrets: &BTreeMap<String, [u8; 32]>,
) -> Result<()> {
    if founding_founder == Pubkey::default()
        || substituted_founder == Pubkey::default()
        || founding_founder == substituted_founder
    {
        return Err(Error::new(
            "founding and substituted founder must be nonzero, distinct public identities",
        ));
    }
    let mut identities = BTreeSet::from([founding_founder, substituted_founder]);
    for (role, secret) in secrets {
        let signer = Keypair::new_from_array(*secret).pubkey();
        if !identities.insert(signer) {
            return Err(Error::new(format!(
                "{role} keypair aliases a founding actor or another retained signer"
            )));
        }
    }
    Ok(())
}

fn authenticate_founding_only_prerequisites_v1(states: &[(StageV1, StageStateV1)]) -> Result<()> {
    for stage in [
        StageV1::Substrate,
        StageV1::Publication,
        StageV1::Initialize,
        StageV1::Succession,
        StageV1::Activation,
    ] {
        let state = states
            .iter()
            .find(|(candidate, _)| *candidate == stage)
            .map(|(_, state)| state)
            .ok_or_else(|| Error::new(format!("founding-only gate omitted {}", stage.name())))?;
        if state != &StageStateV1::Complete {
            return Err(Error::new(format!(
                "--founding-only requires {} Complete before any key is read; observed {}{}",
                stage.name(),
                state.label(),
                state
                    .detail()
                    .map(|detail| format!(": {detail}"))
                    .unwrap_or_default(),
            )));
        }
    }
    Ok(())
}

/// One semantic owner for the administration ceiling.  The CLI parser and
/// the executor both call this function, so adding a stage cannot leave one
/// admitting it while the other silently refuses it.
pub(crate) fn authenticate_administration_through_v1(through: StageV1) -> Result<()> {
    if through > StageV1::Activation {
        return Err(Error::new(
            "administration mode is infrastructure-only through activation",
        ));
    }
    Ok(())
}

fn administration_required_roles_v1(
    states: &[(StageV1, StageStateV1)],
    through: StageV1,
) -> Vec<&'static str> {
    let mut roles = ADMIN_REQUIRED_ROLES.to_vec();
    if states.iter().any(|(stage, state)| {
        *stage == StageV1::Succession
            && *stage <= through
            && matches!(state, StageStateV1::Absent | StageStateV1::Partial(_))
    }) {
        roles.push(role::CAMPAIGN_PAYER);
    }
    roles
}

fn administration_requires_authority_v1(
    states: &[(StageV1, StageStateV1)],
    through: StageV1,
) -> bool {
    states.iter().any(|(stage, state)| {
        *stage > StageV1::Substrate
            && *stage <= through
            && matches!(state, StageStateV1::Absent | StageStateV1::Partial(_))
    })
}

fn assemble_infrastructure_stage_states_v1(
    substrate: StageStateV1,
    publication: StageStateV1,
    initialize: StageStateV1,
    succession: StageStateV1,
    mut activation: StageStateV1,
) -> Vec<(StageV1, StageStateV1)> {
    if succession != StageStateV1::Complete && activation != StageStateV1::Absent {
        activation = StageStateV1::Conflict(format!(
            "activation is {} while succession is {}; the chain is half-flipped and no resumed campaign may write over it",
            activation.label(),
            succession.label(),
        ));
    }
    vec![
        (StageV1::Substrate, substrate),
        (StageV1::Publication, publication),
        (StageV1::Initialize, initialize),
        (StageV1::Succession, succession),
        (StageV1::Activation, activation),
    ]
}

fn authenticate_local_participant_fixture_policy_v1(
    origin: &ClusterOriginV1,
    market: Option<&crate::model::MarketRunInput>,
    keypairs: &BTreeMap<String, PathBuf>,
) -> Result<()> {
    let Some(market) = market else {
        return Ok(());
    };
    if market.local_participant_fixture_liquidity_atoms == 0 {
        return Ok(());
    }
    crate::cluster::ExpectedClusterV1::OwnedLoopback.authenticate(origin)?;
    if market.local_participant_fixture_liquidity_atoms
        != crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
    {
        return Err(Error::new(
            "local participant fixture liquidity changed from its exact profile",
        ));
    }
    for role in [
        crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
        crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
    ] {
        if !keypairs.contains_key(role) {
            return Err(Error::new(format!(
                "local participant fixture liquidity requires --keypair-{role}"
            )));
        }
    }
    Ok(())
}

fn load_campaign_keypairs(paths: &BTreeMap<String, PathBuf>) -> Result<BTreeMap<String, [u8; 32]>> {
    let mut secrets = BTreeMap::new();
    let mut pubkeys = BTreeSet::new();
    for (role, path) in paths {
        let secret = read_keypair_file(path, role)?;
        let pubkey = Keypair::new_from_array(secret).pubkey();
        if !pubkeys.insert(pubkey) {
            return Err(Error::new(format!(
                "{role} keypair reuses another campaign role's public key"
            )));
        }
        secrets.insert(role.clone(), secret);
    }
    Ok(secrets)
}

/// Read one Solana CLI keypair file and return its 32-byte secret seed.
///
/// The CLI's format is a JSON array of 64 bytes: the ed25519 secret seed
/// followed by the public key it expands to. Both halves are read and the
/// expansion is **re-derived and compared**, so a truncated, reordered, or
/// hand-edited file is a refusal here rather than a signature the cluster
/// rejects for reasons that look like something else.
pub(crate) fn read_keypair_file(path: &Path, label: &str) -> Result<[u8; 32]> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} keypair path must be absolute")));
    }
    let bytes: Vec<u8> = serde_json::from_slice(&fs::read(path).map_err(|error| {
        Error::new(format!("read {label} keypair {}: {error}", path.display()))
    })?)
    .map_err(|error| {
        Error::new(format!(
            "{label} keypair {} is not a JSON byte array: {error}",
            path.display()
        ))
    })?;
    if bytes.len() != 64 {
        return Err(Error::new(format!(
            "{label} keypair {} holds {} bytes; a Solana CLI keypair file is 64 (32-byte secret \
             seed then its 32-byte public key)",
            path.display(),
            bytes.len()
        )));
    }
    // Split by value rather than by `copy_from_slice` on a `get(..).unwrap_or`:
    // the width was checked above, but a slice copy whose panic is prevented by
    // a check thirty lines away is a panic waiting for someone to move the
    // check. `try_into` carries its own proof.
    let (secret, declared): ([u8; 32], [u8; 32]) = match (bytes.get(..32), bytes.get(32..)) {
        (Some(secret), Some(declared)) => (
            secret
                .try_into()
                .map_err(|_| Error::new("keypair secret half was not 32 bytes"))?,
            declared
                .try_into()
                .map_err(|_| Error::new("keypair public half was not 32 bytes"))?,
        ),
        _ => return Err(Error::new("keypair file could not be split into halves")),
    };
    let derived = Keypair::new_from_array(secret);
    if derived.pubkey().to_bytes() != declared {
        return Err(Error::new(format!(
            "{label} keypair {} is inconsistent: the public key it declares is not the one its \
             secret seed expands to. This file is damaged; do not fund the address it prints.",
            path.display()
        )));
    }
    Ok(secret)
}

/// Read every role's deployment off the cluster and compare it to the plan.
///
/// Read-only, and the one stage that is read-only even under `--execute`.
pub(crate) fn substrate_state(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<(StageStateV1, Vec<ObservedRoleV1>)> {
    let mut observed = Vec::new();
    let mut absent = Vec::new();
    let mut drifted = Vec::new();
    for (role, pin) in runtime::role_pins(plan) {
        let programdata = pubkey(&pin.programdata_id)?;
        let account = rpc.account(programdata)?;
        let (slot, authority, owner, executable, live_elf_sha256, data_len) = match &account {
            None => (None, None, None, None, None, None),
            Some(account) => {
                let view = ProgramDataV3View::parse(&account.data).map_err(|error| {
                    Error::new(format!(
                        "{role} ProgramData at {programdata} does not parse as a Loader V3 \
                         ProgramData account: {error:?}"
                    ))
                })?;
                (
                    Some(view.deployment_slot()),
                    view.upgrade_authority()
                        .map(|key| Pubkey::from(key).to_string()),
                    Some(account.owner.to_string()),
                    Some(account.executable),
                    Some(hex(&<sha2::Sha256 as sha2::Digest>::digest(view.elf()))),
                    Some(account.data.len()),
                )
            }
        };
        let row = ObservedRoleV1 {
            role: role.to_owned(),
            program_id: pin.program_id.clone(),
            programdata_id: pin.programdata_id.clone(),
            observed_slot: slot,
            pinned_slot: pin.deployment_slot,
            observed_authority: authority,
            pinned_authority: pin.upgrade_authority.clone(),
            observed_owner: owner,
            observed_executable: executable,
            observed_live_elf_sha256: live_elf_sha256,
            pinned_live_elf_sha256: pin.live_elf_sha256.clone(),
            checked_candidate_elf_sha256: pin.checked_candidate_elf_sha256.clone(),
            live_elf_padding_bytes: pin.live_elf_padding_bytes,
            observed_data_len: data_len,
        };
        if account.is_none() {
            absent.push(row.role.clone());
        } else {
            let allow_succession = plan.infrastructure_succession.is_some() && role == "registry";
            drifted.extend(row.pin_conflicts_allowing_forward_slot(allow_succession));
        }
        observed.push(row);
    }
    let state = if absent.len() == observed.len() {
        StageStateV1::Absent
    } else if !absent.is_empty() {
        StageStateV1::Partial(format!("not deployed: {}", absent.join(", ")))
    } else if !drifted.is_empty() {
        // Decision 0012's fail-closed conditions, stated as themselves. Slot,
        // authority, owner, and executable shape are all authenticated by the
        // artifact release; no one coordinate substitutes for the others.
        StageStateV1::Conflict(format!(
            "SUBSTRATE DRIFT (decision 0012 fail-closed): {}. The current Loader deployment no \
             longer matches every fact this plan observed. Re-mint this plan's release bodies \
             from the CURRENT observed ProgramData before publishing anything.",
            drifted.join("; ")
        ))
    } else {
        StageStateV1::Complete
    };
    Ok((state, observed))
}

/// Are the nine infrastructure record bodies finalized where the plan says?
pub(crate) fn publication_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let mut present = Vec::new();
    let mut missing = Vec::new();
    let mut partial = Vec::new();
    let mut wrong = Vec::new();
    for (label, pair) in &plan.records {
        let (raw, staging) = runtime::record(plan, label)?;
        let body = runtime::decode_hex(&pair.body_hex)?;
        let raw_account = rpc.account(raw)?;
        let staging_account = rpc.account(staging)?;
        match runtime::existing_finalized_record_is_exact(
            registry,
            raw_account.as_ref(),
            staging_account.as_ref(),
            &body,
            rpc.minimum_balance(body.len())?,
        ) {
            Ok(true) => present.push(label.clone()),
            Ok(false) if staging_account.is_some() => partial.push(label.clone()),
            Ok(false) => missing.push(label.clone()),
            Err(_) => wrong.push(label.clone()),
        }
    }
    Ok(if !wrong.is_empty() {
        StageStateV1::Conflict(format!(
            "records exist at their derived addresses with bytes that are not this plan's: {}",
            wrong.join(", ")
        ))
    } else if missing.is_empty() && partial.is_empty() {
        StageStateV1::Complete
    } else if present.is_empty() && partial.is_empty() {
        StageStateV1::Absent
    } else {
        let mut remaining = missing;
        remaining.extend(partial.iter().map(|label| format!("{label} (in flight)")));
        StageStateV1::Partial(format!(
            "{} of {} finalized; still missing or in flight: {}",
            present.len(),
            plan.records.len(),
            remaining.join(", ")
        ))
    })
}

/// Does Core's infrastructure profile exist, with this plan's exact body?
pub(crate) fn initialize_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let address = pubkey(&plan.infrastructure_profile.address)?;
    let Some(account) = rpc.account(address)? else {
        return Ok(StageStateV1::Absent);
    };
    let expected = runtime::decode_hex(&plan.infrastructure_profile.body_hex)?;
    Ok(if account.data == expected {
        StageStateV1::Complete
    } else {
        StageStateV1::Conflict(format!(
            "an infrastructure profile exists at {address} whose {} bytes are not this plan's {}",
            account.data.len(),
            expected.len()
        ))
    })
}

struct SuccessionProjectionV1 {
    registry_release: ArtifactReleaseV1,
    registry_artifact_id: ArtifactReleaseIdV1,
    profile: ProtocolInfrastructureProfileV2,
}

fn succession_projection_v1(
    plan: &SuccessorPlan,
    predecessor: ProtocolInfrastructureProfileV1,
    observed_registry_slot: u64,
) -> Result<SuccessionProjectionV1> {
    let pair = plan
        .records
        .get("registry_artifact_release")
        .ok_or_else(|| Error::new("plan omitted predecessor Registry artifact record"))?;
    let predecessor_bytes = runtime::decode_hex(&pair.body_hex)?;
    let predecessor_release = ArtifactReleaseV1::decode(&predecessor_bytes)
        .map_err(|error| Error::new(format!("predecessor Registry artifact: {error:?}")))?;
    if sha256_bytes(&predecessor_bytes) != predecessor.registry().artifact_release().to_bytes()
        || observed_registry_slot <= predecessor_release.deployment_slot()
    {
        return Err(Error::new(
            "Registry succession did not move strictly forward from V1's pinned artifact",
        ));
    }
    let registry_release = ArtifactReleaseV1::new(
        predecessor_release.program(),
        predecessor_release.loader_program(),
        predecessor_release.programdata(),
        predecessor_release.semantic_release_id(),
        predecessor_release.elf_digest(),
        observed_registry_slot,
        predecessor_release.upgrade_policy(),
        predecessor_release.upgrade_authority(),
    )
    .map_err(|error| Error::new(format!("successor Registry artifact: {error:?}")))?;
    let registry_artifact_id = ArtifactReleaseIdV1::new(sha256_bytes(&registry_release.to_bytes()))
        .map_err(|error| Error::new(format!("successor Registry artifact id: {error:?}")))?;
    let profile = ProtocolInfrastructureProfileV2::new(
        ExecutionRoleBindingV1::new(predecessor.registry().program(), registry_artifact_id),
        predecessor.rent(),
        predecessor.registry().artifact_release(),
        predecessor.rent().artifact_release(),
    )
    .map_err(|error| Error::new(format!("successor infrastructure profile: {error:?}")))?;
    Ok(SuccessionProjectionV1 {
        registry_release,
        registry_artifact_id,
        profile,
    })
}

/// Detect the complete three-write succession stage.  A plan with no checked
/// local pin reads Complete/not-applicable, so neither a legacy immutable plan
/// nor permanent devnet silently acquires a Loader write.
pub(crate) fn succession_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let Some(pin) = plan.infrastructure_succession.as_ref() else {
        return Ok(StageStateV1::Complete);
    };
    if plan.checked_local_mutable_set.is_none() || plan.checked_upgrade_set.is_some() {
        return Ok(StageStateV1::Conflict(
            "infrastructure succession is admitted only by one checked-local mutable plan".into(),
        ));
    }
    let core = pubkey(&plan.core.program_id)?;
    let v1_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
    let v1_account = rpc.account(v1_address)?;
    let Some(v1_account) = v1_account else {
        return Ok(StageStateV1::Absent);
    };
    let expected_v1 = runtime::decode_hex(&plan.infrastructure_profile.body_hex)?;
    if v1_account.owner != core || v1_account.data != expected_v1 {
        return Ok(StageStateV1::Conflict(
            "V1 predecessor profile changed before succession".into(),
        ));
    }
    let predecessor = ProtocolInfrastructureProfileV1::decode(&v1_account.data)
        .map_err(|error| Error::new(format!("V1 predecessor profile: {error:?}")))?;
    if pin.predecessor_registry_artifact_release_id
        != hex(predecessor.registry().artifact_release().as_bytes())
        || pin.predecessor_rent_artifact_release_id
            != hex(predecessor.rent().artifact_release().as_bytes())
    {
        return Ok(StageStateV1::Conflict(
            "succession pin does not name V1's two predecessor artifacts".into(),
        ));
    }

    let registry_programdata = pubkey(&plan.registry.programdata_id)?;
    let Some(programdata) = rpc.account(registry_programdata)? else {
        return Ok(StageStateV1::Conflict(
            "Registry ProgramData disappeared during succession".into(),
        ));
    };
    let view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|error| Error::new(format!("Registry ProgramData: {error:?}")))?;
    let expected_authority = plan
        .registry
        .upgrade_authority
        .as_deref()
        .map(pubkey)
        .transpose()?
        .map(|authority| authority.to_bytes());
    if programdata.owner != bpf_loader_upgradeable::ID
        || programdata.executable
        || view.upgrade_authority() != expected_authority
        || hex(&sha256_bytes(view.elf())) != plan.registry.live_elf_sha256
    {
        return Ok(StageStateV1::Conflict(
            "Registry ProgramData changed outside the pinned one-upgrade succession".into(),
        ));
    }

    let buffer = pubkey(&pin.registry_upgrade_buffer)?;
    let buffer_account = rpc.account(buffer)?;
    let v2_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    let v2_account = rpc.account(v2_address)?;
    if view.deployment_slot() == plan.registry.deployment_slot {
        if v2_account.is_some() {
            return Ok(StageStateV1::Conflict(
                "V2 profile exists although Registry never moved from V1's slot".into(),
            ));
        }
        let genesis = plan
            .genesis_accounts
            .get(crate::plan::REGISTRY_SUCCESSION_BUFFER_LABEL_V1)
            .ok_or_else(|| Error::new("plan omitted Registry succession Buffer account pin"))?;
        return Ok(match buffer_account {
            Some(account)
                if account.owner == bpf_loader_upgradeable::ID
                    && !account.executable
                    && account.lamports == genesis.lamports
                    && account.data.len() == genesis.data_len
                    && hex(&sha256_bytes(&account.data)) == genesis.data_sha256 =>
            {
                StageStateV1::Absent
            }
            _ => StageStateV1::Conflict(
                "Registry is still at V1 but its exact planted upgrade Buffer is absent or changed"
                    .into(),
            ),
        });
    }
    if view.deployment_slot() < plan.registry.deployment_slot {
        return Ok(StageStateV1::Conflict(
            "Registry deployment slot moved backward from the predecessor pin".into(),
        ));
    }
    if buffer_account.is_some() {
        return Ok(StageStateV1::Conflict(
            "Registry moved but the one-use succession Buffer still exists".into(),
        ));
    }

    let projection = succession_projection_v1(plan, predecessor, view.deployment_slot())?;
    let registry = pubkey(&plan.registry.program_id)?;
    let body = projection.registry_release.to_bytes();
    let digest = sha256_bytes(&body);
    let raw = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry,
    )
    .0;
    let raw_account = rpc.account(raw)?;
    let staging_account = rpc.account(staging)?;
    let record_complete = match runtime::existing_finalized_record_is_exact(
        registry,
        raw_account.as_ref(),
        staging_account.as_ref(),
        &body,
        rpc.minimum_balance(body.len())?,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Ok(StageStateV1::Conflict(format!(
                "successor Registry record conflicts: {error}"
            )));
        }
    };
    if !record_complete {
        return Ok(if v2_account.is_some() {
            StageStateV1::Conflict(
                "V2 profile exists before the successor Registry record is finalized".into(),
            )
        } else {
            StageStateV1::Partial(
                "Registry upgrade landed; successor record and V2 ceremony remain".into(),
            )
        });
    }
    let Some(v2_account) = v2_account else {
        return Ok(StageStateV1::Partial(
            "Registry upgrade and successor record landed; V2 ceremony remains".into(),
        ));
    };
    Ok(
        if v2_account.owner == core
            && v2_account.data.len() == PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
            && v2_account.data == projection.profile.to_bytes()
        {
            StageStateV1::Complete
        } else {
            StageStateV1::Conflict(
                "V2 profile exists but is not the exact chain-derived succession selection".into(),
            )
        },
    )
}

fn infrastructure_role_label_v1(role: ExecutionRoleV1) -> &'static str {
    match role {
        ExecutionRoleV1::Core => "core",
        ExecutionRoleV1::Claims => "claims",
        ExecutionRoleV1::Trading => "trading",
        ExecutionRoleV1::Resolution => "resolution",
        ExecutionRoleV1::Custody => "custody",
    }
}

fn artifact_release_evidence_v1(id: ArtifactReleaseIdV1, release: ArtifactReleaseV1) -> Value {
    let policy = match release.upgrade_policy() {
        ArtifactUpgradePolicyV1::Immutable => "immutable",
        ArtifactUpgradePolicyV1::ExactAuthority => "exact-authority",
    };
    json!({
        "artifactReleaseId": hex(id.as_bytes()),
        "program": Pubkey::new_from_array(release.program().to_bytes()).to_string(),
        "loaderProgram": Pubkey::new_from_array(release.loader_program().to_bytes()).to_string(),
        "programData": Pubkey::new_from_array(release.programdata()).to_string(),
        "semanticReleaseId": hex(release.semantic_release_id().as_bytes()),
        "elfSha256": hex(&release.elf_digest()),
        "deploymentSlot": release.deployment_slot(),
        "upgradePolicy": policy,
        "upgradeAuthority": release
            .upgrade_authority()
            .map(Pubkey::new_from_array)
            .map(|value| value.to_string()),
    })
}

fn finalized_artifact_release_evidence_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    id: ArtifactReleaseIdV1,
    label: &str,
) -> Result<(ArtifactReleaseV1, Value)> {
    let raw = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            id.as_bytes(),
        ],
        &registry,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            id.as_bytes(),
        ],
        &registry,
    )
    .0;
    let raw_account = rpc.account(raw)?.ok_or_else(|| {
        Error::new(format!(
            "infrastructure lineage cannot read finalized {label} record {raw}"
        ))
    })?;
    let staging_account = rpc.account(staging)?;
    let digest: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&raw_account.data).into();
    if digest != *id.as_bytes()
        || !runtime::existing_finalized_record_is_exact(
            registry,
            Some(&raw_account),
            staging_account.as_ref(),
            &raw_account.data,
            rpc.minimum_balance(raw_account.data.len())?,
        )?
    {
        return Err(Error::new(format!(
            "infrastructure lineage {label} record is absent, partial, or not at its content identity"
        )));
    }
    let release = ArtifactReleaseV1::decode(&raw_account.data)
        .map_err(|error| Error::new(format!("infrastructure lineage {label}: {error:?}")))?;
    Ok((
        release,
        json!({
            "record": artifact_release_evidence_v1(id, release),
            "rawAccount": account_evidence(raw, &raw_account),
            "stagingAddress": staging.to_string(),
            "stagingAbsentAfterFinalize": staging_account.is_none(),
        }),
    ))
}

/// Authenticate the one carrier byte in a current Registry-written activation
/// cache without dropping byte-exact authentication of the release projection.
///
/// `ActivatedExecutionReleaseSetV1::to_bytes` cannot author the PDA bump: that
/// byte is a fact about the account address, not the release set. Registry does
/// author it while creating the account. The current-source lineage therefore
/// requires the nonzero bump re-derived from the exact Registry program and
/// release-set ID, then compares every other byte to the canonical projection.
fn authenticate_current_activation_cache_body_v1(
    registry_program: Pubkey,
    activation_address: Pubkey,
    observed: &[u8],
    expected: ActivatedExecutionReleaseSetV1,
) -> Result<()> {
    let release_set_id = expected.execution_release_set_id();
    let (expected_address, expected_bump) = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &registry_program,
    );
    let observed_bump = observed
        .get(ACTIVATION_CACHE_BUMP_OFFSET_V1)
        .copied()
        .ok_or_else(|| Error::new("current activation cache has the wrong width"))?;
    if activation_address != expected_address
        || expected_bump == 0
        || observed_bump == 0
        || observed_bump != expected_bump
    {
        return Err(Error::new(
            "current activation cache address-derived bump is not exact and nonzero",
        ));
    }
    let progress = activation_cache_progress_v1(observed, expected).map_err(|error| {
        Error::new(format!(
            "current activation cache differs outside its address-derived bump: {error:?}"
        ))
    })?;
    if !progress.is_complete() {
        return Err(Error::new(format!(
            "current activation cache has only {} of {} exact roles",
            progress.written_count(),
            EXECUTION_ROLE_ORDER_V1.len(),
        )));
    }
    Ok(())
}

/// Re-derive the complete checked-local infrastructure lineage from finalized
/// chain state. This is deliberately a post-execution projection: no caller
/// supplies a successor slot, profile body, record id, or activation role.
fn infrastructure_lineage_evidence_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    genesis_hash: &str,
    plan_sha256: &str,
    campaign_evidence_path: &Path,
) -> Result<Value> {
    let set = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("standalone infrastructure lineage requires a checked-local mutable release set")
    })?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(plan)?;
    if succession_state(rpc, plan)? != StageStateV1::Complete {
        return Err(Error::new(
            "standalone infrastructure lineage requires completed V1-to-V2 succession",
        ));
    }

    let core = pubkey(&plan.core.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let v1_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
    let v2_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    if v1_address.to_string() != plan.infrastructure_profile.address {
        return Err(Error::new(
            "plan infrastructure profile is not Core's canonical V1 coordinate",
        ));
    }
    let v1_account = rpc
        .account(v1_address)?
        .ok_or_else(|| Error::new("completed succession omitted the V1 predecessor profile"))?;
    let expected_v1 = runtime::decode_hex(&plan.infrastructure_profile.body_hex)?;
    if v1_account.owner != core
        || v1_account.data != expected_v1
        || hex(&<sha2::Sha256 as sha2::Digest>::digest(&v1_account.data))
            != plan.infrastructure_profile.body_sha256
    {
        return Err(Error::new(
            "completed succession did not preserve the exact planned V1 profile",
        ));
    }
    let v1 = ProtocolInfrastructureProfileV1::decode(&v1_account.data)
        .map_err(|error| Error::new(format!("lineage V1 profile: {error:?}")))?;
    let v2_account = rpc
        .account(v2_address)?
        .ok_or_else(|| Error::new("completed succession omitted the V2 profile"))?;
    if v2_account.owner != core {
        return Err(Error::new("V2 profile is not Core-owned"));
    }
    let v2 = ProtocolInfrastructureProfileV2::decode(&v2_account.data)
        .map_err(|error| Error::new(format!("lineage V2 profile: {error:?}")))?;
    if v2.registry().program() != v1.registry().program()
        || v2.rent().program() != v1.rent().program()
        || v2.predecessor_registry_artifact() != v1.registry().artifact_release()
        || v2.predecessor_rent_artifact() != v1.rent().artifact_release()
        || v2.rent().artifact_release() != v1.rent().artifact_release()
        || v2.registry().artifact_release() == v1.registry().artifact_release()
    {
        return Err(Error::new(
            "V2 profile does not form the exact one-Registry-move successor of V1",
        ));
    }

    let (predecessor_registry, predecessor_registry_evidence) =
        finalized_artifact_release_evidence_v1(
            rpc,
            registry,
            v1.registry().artifact_release(),
            "predecessor Registry",
        )?;
    let (successor_registry, successor_registry_evidence) = finalized_artifact_release_evidence_v1(
        rpc,
        registry,
        v2.registry().artifact_release(),
        "successor Registry",
    )?;
    let (rent_release, rent_evidence) = finalized_artifact_release_evidence_v1(
        rpc,
        registry,
        v2.rent().artifact_release(),
        "carried Rent",
    )?;
    if predecessor_registry.program() != successor_registry.program()
        || predecessor_registry.programdata() != successor_registry.programdata()
        || predecessor_registry.semantic_release_id() != successor_registry.semantic_release_id()
        || predecessor_registry.elf_digest() != successor_registry.elf_digest()
        || predecessor_registry.upgrade_authority() != successor_registry.upgrade_authority()
        || successor_registry.deployment_slot() <= predecessor_registry.deployment_slot()
        || rent_release.program().to_bytes() != v2.rent().program().to_bytes()
    {
        return Err(Error::new(
            "profile lineage artifact records do not prove one forward Registry deployment with carried Rent",
        ));
    }

    let activation_address = pubkey(&plan.activation)?;
    let activation_account = rpc.account(activation_address)?.ok_or_else(|| {
        Error::new("standalone infrastructure lineage requires the completed activation cache")
    })?;
    if activation_account.owner != registry || activation_account.executable {
        return Err(Error::new(
            "completed activation cache is not readonly Registry state",
        ));
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_account.data)
        .map_err(|error| Error::new(format!("infrastructure activation cache: {error:?}")))?;
    let activated_release_set_id = activated
        .execution_release_set_id()
        .map_err(|error| Error::new(format!("activation release-set id: {error:?}")))?;
    if hex(activated_release_set_id.as_bytes()) != plan.release_set_id {
        return Err(Error::new(
            "live activation cache differs from the plan's exact execution release set",
        ));
    }
    let expected_activation = runtime::expected_activation(plan)?;
    authenticate_current_activation_cache_body_v1(
        registry,
        activation_address,
        &activation_account.data,
        expected_activation,
    )?;
    let activated_roles = EXECUTION_ROLE_ORDER_V1
        .into_iter()
        .map(|role| {
            let row = activated.role(role).map_err(|error| {
                Error::new(format!(
                    "decode activated {} role: {error:?}",
                    infrastructure_role_label_v1(role)
                ))
            })?;
            Ok(json!({
                "role": infrastructure_role_label_v1(role),
                "release": artifact_release_evidence_v1(
                    row.artifact_release_id(),
                    row.release(),
                ),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let checked_artifacts = set
        .roles
        .iter()
        .map(|role| {
            json!({
                "role": role.role,
                "program": role.program_id,
                "programData": role.programdata_id,
                "checkedCandidateElfPath": role.checked_candidate_elf_path,
                "checkedCandidateElfSha256": role.checked_candidate_elf_sha256,
                "genesisLiveElfSha256": role.live_elf_sha256,
                "genesisProgramDataAccountSha256": role.programdata_account_sha256,
                "genesisDeploymentSlot": role.deployment_slot,
                "semanticReleaseId": role.semantic_release_id,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "schema": "dclutch-current-source-infrastructure-lineage-v1",
        "evidenceLevel": "local-validator-finalized-chain-state",
        "cluster": "owned-loopback",
        "genesisHash": genesis_hash,
        "planSha256": plan_sha256,
        "campaignEvidencePath": campaign_evidence_path.display().to_string(),
        "source": {
            "revision": set.source_revision,
            "treeSha256": set.source_tree_sha256,
            "checkedReleaseGatePath": set.checked_release_gate_path,
            "checkedReleaseGateSha256": set.checked_release_gate_sha256,
            "checkedLocalMutableSetSha256": set.set_sha256,
            "solanaCliVersion": set.solana_cli_version,
        },
        "checkedArtifacts": checked_artifacts,
        "profiles": {
            "predecessorV1": {
                "address": v1_address.to_string(),
                "account": account_evidence(v1_address, &v1_account),
                "registryArtifactReleaseId": hex(v1.registry().artifact_release().as_bytes()),
                "rentArtifactReleaseId": hex(v1.rent().artifact_release().as_bytes()),
            },
            "successorV2": {
                "address": v2_address.to_string(),
                "account": account_evidence(v2_address, &v2_account),
                "registryArtifactReleaseId": hex(v2.registry().artifact_release().as_bytes()),
                "rentArtifactReleaseId": hex(v2.rent().artifact_release().as_bytes()),
                "predecessorRegistryArtifactReleaseId": hex(v2.predecessor_registry_artifact().as_bytes()),
                "predecessorRentArtifactReleaseId": hex(v2.predecessor_rent_artifact().as_bytes()),
            },
            "v1PreservedByteIdentical": true,
        },
        "artifactLineage": {
            "registry": {
                "movedForward": true,
                "predecessor": predecessor_registry_evidence,
                "successor": successor_registry_evidence,
            },
            "rent": {
                "carriedForward": true,
                "release": rent_evidence,
            },
        },
        "activation": {
            "releaseSetId": plan.release_set_id,
            "checkedExecutionReleaseSetId": set.execution_release_set.checked_execution_release_set_id,
            "checkedMultiprogramEnvelopeSha256": hex(&<sha2::Sha256 as sha2::Digest>::digest(
                base64::engine::general_purpose::STANDARD
                    .decode(&set.execution_release_set.checked_execution_release_set_base64)
                    .map_err(|error| Error::new(format!("checked execution release envelope: {error}")))?
            )),
            "account": account_evidence(activation_address, &activation_account),
            "roles": activated_roles,
        },
        "migration": {
            "preexistingMarketsMigrated": 0,
            "marketsSilentlyRebound": false,
            "scope": "fresh-validator administration precedes all market founding",
            "consumerRule": "redeployed infrastructure consumers select V2; immutable V1 remains lineage evidence",
        },
    }))
}

fn write_stable_lineage_evidence_v1(path: &Path, value: &Value) -> Result<String> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("infrastructure lineage output requires a parent"))?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
        Error::new(format!(
            "inspect infrastructure lineage parent {}: {error}",
            parent.display()
        ))
    })?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(Error::new(
            "infrastructure lineage parent must be an existing non-symlink directory",
        ));
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let digest = hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes));
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(digest),
        Ok(_) => {
            return Err(Error::new(
                "existing infrastructure lineage artifact differs; it was not replaced",
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Error::new(format!(
                "read infrastructure lineage artifact {}: {error}",
                path.display()
            )));
        }
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            Error::new(format!(
                "create infrastructure lineage artifact {}: {error}",
                path.display()
            ))
        })?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    fs::File::open(parent)?.sync_all()?;
    Ok(digest)
}

/// Is this market input's founding already on the chain?
///
/// Complete is the market-account core of the executor's own
/// `authenticate_open_market_poststate_v1`: the DCLTGMF3 Market exists at its
/// derived address, Core-owned, Open, readiness consumed, identity equal.
/// Partial is anything the founding creates short of that — and the executor
/// REFUSES a partial founding rather than resuming into it, because the
/// founding ladder is not idempotent past record publication and a half-founded
/// market has real principal behind it. Absent means none of the derived
/// accounts exist and the founding may run.
pub(crate) fn founding_state(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &crate::model::MarketRunInput,
    collateral_mint: Pubkey,
    collateral_wallet: Pubkey,
) -> Result<(StageStateV1, crate::market::FoundingTargetsV1)> {
    let targets = crate::market::derive_founding_targets(plan, input, collateral_mint)?;
    let state = match crate::market::observe_open_market(rpc, plan, &targets)? {
        crate::market::OpenMarketObservationV1::Open => StageStateV1::Complete,
        crate::market::OpenMarketObservationV1::Other(detail) => StageStateV1::Conflict(detail),
        crate::market::OpenMarketObservationV1::Absent => {
            let mut present = Vec::new();
            for (label, key) in [
                ("collateral mint", targets.collateral_mint),
                // The wallet is created in the same transaction as the mint
                // and is a distinct forge role, so a half-founding can leave
                // it existing while the peeked mint does not (measured: the
                // first devnet attempt burned wallet[0] against mint[1], and
                // a retry without this probe collided on it mid-transaction
                // instead of refusing here with the account named).
                ("collateral wallet", collateral_wallet),
                ("realm record", targets.realm_record),
                ("Found31 Market", targets.found31_market),
                ("abort-lane Market", targets.abort_market),
            ] {
                if rpc.account(key)?.is_some() {
                    present.push(format!("{label} {key}"));
                }
            }
            if present.is_empty() {
                StageStateV1::Absent
            } else {
                StageStateV1::Partial(format!(
                    "the Open Market does not exist at {} but this founding has started: {}",
                    targets.open_market,
                    present.join(", ")
                ))
            }
        }
    };
    Ok((state, targets))
}

/// Project one exact activation-cache observation into campaign stage state.
/// One through four byte-identical role slots are an inert resume point; a
/// complete cache is done; any mismatched header, role, owner, privilege, or
/// width is a conflict.
fn activation_state_from_progress(
    address: Pubkey,
    progress: Result<Option<ActivationCacheProgressV1>>,
) -> StageStateV1 {
    match progress {
        Ok(None) => StageStateV1::Absent,
        Ok(Some(progress)) if progress.is_complete() => StageStateV1::Complete,
        Ok(Some(progress)) => StageStateV1::Partial(format!(
            "{} of {} exact release roles activated; resume the missing roles",
            progress.written_count(),
            runtime::ACTIVATION_ROLE_COUNT_V1
        )),
        Err(error) => StageStateV1::Conflict(format!(
            "a release activation cache exists at {address} that this plan does not \
             authenticate: {error}"
        )),
    }
}

/// The payer's balance against what the remaining stages will cost.
///
/// Rent is read from the cluster rather than assumed: SMOKE-0 §1.2 re-derived
/// devnet's affine `min_balance(n) = 890,880 + 6,960·n` live, and a driver that
/// hardcoded it would be carrying a fourth copy of a number the chain will tell
/// it.
#[derive(Clone, Debug)]
pub(crate) struct WalletArithmeticV1 {
    pub(crate) payer: String,
    pub(crate) balance_lamports: u64,
    pub(crate) record_rent_lamports: u64,
    pub(crate) profile_rent_lamports: u64,
    pub(crate) activation_rent_lamports: u64,
    pub(crate) estimated_fee_lamports: u64,
    pub(crate) required_lamports: u64,
}

impl WalletArithmeticV1 {
    pub(crate) fn shortfall(&self) -> u64 {
        self.required_lamports.saturating_sub(self.balance_lamports)
    }
}

/// Fee estimate per transaction, at the base signature price.
///
/// Measured-profile: SMOKE-0 §5.3 read the recent-prioritization-fee page as
/// all zeros immediately before its ladder and paid no priority fee anywhere,
/// so the base 5,000 lamports per signature is the whole cost today. It is an
/// estimate and is labelled as one; the driver prints it beside the real fees
/// it then pays.
const LAMPORTS_PER_SIGNATURE: u64 = 5_000;

/// A generous per-stage transaction count for the estimate.
///
/// Publication is `Begin -> Append… -> Finalize` per record and the founding
/// ladder is the ~116-transaction one SMOKE-0's charter names. Rounded up: an
/// estimate that under-states the requirement is the one that strands a run.
const ESTIMATED_TRANSACTIONS: u64 = 200;

pub(crate) fn wallet_arithmetic(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    payer: Pubkey,
) -> Result<WalletArithmeticV1> {
    let balance = rpc
        .call(
            "getBalance",
            &json!([payer.to_string(), {"commitment":"finalized"}]),
        )?
        .get("value")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Error::new("getBalance omitted a u64 value"))?;
    let mut record_rent = 0_u64;
    for label in plan.records.keys() {
        record_rent = record_rent.saturating_add(runtime::remaining_record_publication_rent(
            rpc, plan, label, payer,
        )?);
    }
    let profile_address = pubkey(&plan.infrastructure_profile.address)?;
    let profile_body = runtime::decode_hex(&plan.infrastructure_profile.body_hex)?;
    let profile_minimum = rpc.minimum_balance(profile_body.len())?;
    let profile_account = rpc.account(profile_address)?;
    let profile_rent = remaining_profile_rent(
        profile_account.as_ref(),
        pubkey(&plan.core.program_id)?,
        &profile_body,
        profile_minimum,
    )?;
    let activation_rent = runtime::remaining_activation_rent(rpc, plan)?;
    let fees = ESTIMATED_TRANSACTIONS.saturating_mul(LAMPORTS_PER_SIGNATURE);
    let required = record_rent
        .saturating_add(profile_rent)
        .saturating_add(activation_rent)
        .saturating_add(fees);
    Ok(WalletArithmeticV1 {
        payer: payer.to_string(),
        balance_lamports: balance,
        record_rent_lamports: record_rent,
        profile_rent_lamports: profile_rent,
        activation_rent_lamports: activation_rent,
        estimated_fee_lamports: fees,
        required_lamports: required,
    })
}

fn remaining_profile_rent(
    account: Option<&crate::rpc::RpcAccount>,
    core: Pubkey,
    expected_body: &[u8],
    minimum_balance: u64,
) -> Result<u64> {
    let Some(account) = account else {
        return Ok(minimum_balance);
    };
    if account.owner != core
        || account.executable
        || account.data != expected_body
        || account.lamports < minimum_balance
    {
        return Err(Error::new(
            "existing infrastructure profile conflicts with the exact plan coordinate",
        ));
    }
    Ok(0)
}

/// Authenticate the committed devnet Pyth release row against live accounts.
///
/// The row (`dclutch_pyth_svm::devnet_release_v1`, minted by SMOKE-0 at
/// `11f249ff`) states five keys, two deployment slots, and a config digest as
/// measured facts. This re-reads all eight off the cluster and compares — the
/// same joins `provider_instruction_v3::authenticate_pyth_release` makes on
/// chain, run *before* a market is founded against them rather than discovered
/// as a refusal at resolution.
pub(crate) fn authenticate_pyth_row(rpc: &mut Rpc) -> Result<Vec<(String, bool, String)>> {
    let release = devnet_release_v1().map_err(|error| {
        Error::new(format!(
            "the committed devnet Pyth row is invalid: {error:?}"
        ))
    })?;
    let receiver = Pubkey::from(release.receiver_program());
    let receiver_programdata = Pubkey::from(release.receiver_programdata());
    let receiver_config = Pubkey::from(release.receiver_config());
    let router = Pubkey::from(release.router_program());
    let router_programdata = Pubkey::from(release.router_programdata());
    let mut rows = Vec::new();

    for (label, program, programdata, expected_slot) in [
        (
            "receiver",
            receiver,
            receiver_programdata,
            release.receiver_deployment_slot(),
        ),
        (
            "router",
            router,
            router_programdata,
            release.router_deployment_slot(),
        ),
    ] {
        match rpc.account(program)? {
            None => rows.push((
                format!("{label} program {program}"),
                false,
                "account absent".into(),
            )),
            Some(account) => rows.push((
                format!("{label} program {program}"),
                account.executable,
                format!("executable={} owner={}", account.executable, account.owner),
            )),
        }
        match rpc.account(programdata)? {
            None => rows.push((
                format!("{label} programdata {programdata}"),
                false,
                "account absent".into(),
            )),
            Some(account) => {
                let view = ProgramDataMetadataV3View::parse(&account.data).map_err(|error| {
                    Error::new(format!("{label} ProgramData does not parse: {error:?}"))
                })?;
                let slot = view.deployment_slot();
                rows.push((
                    format!("{label} deployment slot"),
                    slot == expected_slot,
                    format!("observed {slot}, row binds {expected_slot}"),
                ));
                let authority = view
                    .upgrade_authority()
                    .map(|key| Pubkey::from(key).to_string())
                    .unwrap_or_else(|| "revoked".into());
                rows.push((
                    format!("{label} upgrade authority"),
                    true,
                    format!("observed {authority} (disclosed, not bound by the row)"),
                ));
            }
        }
    }

    match rpc.account(receiver_config)? {
        None => rows.push((
            format!("receiver Config {receiver_config}"),
            false,
            "account absent".into(),
        )),
        Some(account) => {
            let digest = hex(&<sha2::Sha256 as sha2::Digest>::digest(&account.data));
            let expected = hex(&release.config_digest());
            rows.push((
                "receiver Config digest".into(),
                digest == expected,
                format!("observed {digest}, row binds {expected}"),
            ));
            rows.push((
                "receiver Config owner".into(),
                account.owner == receiver,
                format!(
                    "observed {}, must be the receiver {receiver}",
                    account.owner
                ),
            ));
        }
    }
    Ok(rows)
}

/// The `solana program` ladder a deploy would run, emitted and never executed.
///
/// TPU is the default because SMOKE-0 §3.1's A/B measured it moving Trading's
/// 1.32 MB in 23 seconds against `--use-rpc`'s ~350 B/s and `Max retries
/// exceeded`. `--use-rpc` keeps its stated role as the fallback for a machine
/// whose TPU egress is blocked, which is what the runbook's advice should have
/// said all along.
pub(crate) fn deploy_ladder(plan: &SuccessorPlan, origin: &ClusterOriginV1) -> Vec<String> {
    let mut lines = vec![
        "# This driver never deploys. These are the commands a deploy would run.".into(),
        "# Transport: TPU by default (SMOKE-0 §3.1 measured ~100x over --use-rpc for buffer".into(),
        "# writes); add --use-rpc only if this machine's TPU egress is blocked, and expect".into(),
        "# minutes per hundred KB plus Max-retries resumes if you do.".into(),
        "# Run ONE of these at a time: one write-buffer saturates the whole per-IP RPC".into(),
        "# budget (SMOKE-0 friction 1), so nothing else may share this machine's IP.".into(),
    ];
    for (role, pin) in runtime::role_pins(plan) {
        lines.push(format!(
            "solana program deploy --url {} --keypair <PAYER> --program-id <{}-KEYPAIR> {}  \
             # {role}, pins slot {}",
            origin.redacted_url(),
            role.to_uppercase(),
            pin.checked_candidate_elf_path,
            pin.deployment_slot
        ));
    }
    lines.push(
        "# Then re-read each ProgramData and re-run `prepare --ROLE-observed-programdata`: the"
            .into(),
    );
    lines.push(
        "# deployment slot the release binds is decoded out of the resulting account, never".into(),
    );
    lines.push("# supplied by a caller.".into());
    lines
}

fn checked_role<'a>(
    plan: &'a SuccessorPlan,
    set_role: &'a CheckedUpgradeRolePinV1,
) -> Result<(&'a ProgramPin, &'static str)> {
    checked_plan_role(plan, &set_role.role)
}

fn checked_plan_role<'a>(
    plan: &'a SuccessorPlan,
    role: &str,
) -> Result<(&'a ProgramPin, &'static str)> {
    Ok(match role {
        "registry" => (&plan.registry, "registry_artifact_release"),
        "rent" => (&plan.rent_credit, "rent_artifact_release"),
        "custody" => (&plan.custody, "custody_artifact_release"),
        "resolution" => (&plan.resolution, "resolution_artifact_release"),
        "claims" => (&plan.claims, "claims_artifact_release"),
        "trading" => (&plan.trading, "trading_artifact_release"),
        "core" => (&plan.core, "core_artifact_release"),
        other => {
            return Err(Error::new(format!(
                "unknown checked deployment role {other}"
            )));
        }
    })
}

#[derive(Clone, Copy)]
enum CheckedPlanRoleEvidenceV1<'a> {
    PermanentDevnet(&'a CheckedUpgradeRolePinV1),
    OwnedLoopback(&'a CheckedLocalMutableRolePinV1),
}

impl<'a> CheckedPlanRoleEvidenceV1<'a> {
    fn role(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.role,
            Self::OwnedLoopback(role) => &role.role,
        }
    }

    fn program_id(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.program_id,
            Self::OwnedLoopback(role) => &role.program_id,
        }
    }

    fn programdata_id(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.programdata_id,
            Self::OwnedLoopback(role) => &role.programdata_id,
        }
    }

    fn checked_candidate_elf_path(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.checked_candidate_elf_path,
            Self::OwnedLoopback(role) => &role.checked_candidate_elf_path,
        }
    }

    fn checked_candidate_elf_sha256(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.checked_candidate_elf_sha256,
            Self::OwnedLoopback(role) => &role.checked_candidate_elf_sha256,
        }
    }

    fn live_elf_sha256(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.live_elf_sha256,
            Self::OwnedLoopback(role) => &role.live_elf_sha256,
        }
    }

    fn programdata_account_sha256(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.programdata_account_sha256,
            Self::OwnedLoopback(role) => &role.programdata_account_sha256,
        }
    }

    fn deployment_slot(self) -> u64 {
        match self {
            Self::PermanentDevnet(role) => role.deployment_slot,
            Self::OwnedLoopback(role) => role.deployment_slot,
        }
    }

    fn semantic_release_id(self) -> &'a str {
        match self {
            Self::PermanentDevnet(role) => &role.semantic_release_id,
            Self::OwnedLoopback(role) => &role.semantic_release_id,
        }
    }

    fn carried_artifact(self) -> Result<Option<(&'a str, &'a str)>> {
        match self {
            Self::PermanentDevnet(role)
                if role.disposition == CheckedDeploymentDispositionV1::CarryForward =>
            {
                Ok(Some((
                    role.artifact_release_body_hex.as_deref().ok_or_else(|| {
                        Error::new(format!(
                            "carried {} role omitted ArtifactRelease body",
                            role.role
                        ))
                    })?,
                    role.artifact_release_id.as_deref().ok_or_else(|| {
                        Error::new(format!(
                            "carried {} role omitted ArtifactRelease identity",
                            role.role
                        ))
                    })?,
                )))
            }
            Self::PermanentDevnet(_) | Self::OwnedLoopback(_) => Ok(None),
        }
    }
}

fn authenticate_checked_plan_role_projection(
    plan: &SuccessorPlan,
    evidence: CheckedPlanRoleEvidenceV1<'_>,
    retained_authority: Pubkey,
) -> Result<()> {
    let role = evidence.role();
    let (pin, record_label) = checked_plan_role(plan, role)?;
    let candidate = fs::read(&pin.checked_candidate_elf_path)?;
    let candidate_sha = hex(&<sha2::Sha256 as sha2::Digest>::digest(&candidate));
    if pin.program_id != evidence.program_id()
        || pin.programdata_id != evidence.programdata_id()
        || pin.elf_path != pin.checked_candidate_elf_path
        || pin.elf_sha256 != pin.checked_candidate_elf_sha256
        || pin.checked_candidate_elf_path != evidence.checked_candidate_elf_path()
        || pin.checked_candidate_elf_sha256 != evidence.checked_candidate_elf_sha256()
        || candidate_sha != evidence.checked_candidate_elf_sha256()
        || pin.live_elf_sha256 != evidence.live_elf_sha256()
        || pin.semantic_release_id != evidence.semantic_release_id()
        || pin.deployment_slot != evidence.deployment_slot()
        || pin.programdata_sha256 != evidence.programdata_account_sha256()
        || pin.upgrade_authority.as_deref() != Some(retained_authority.to_string().as_str())
        || pin.deployment_source != "observed-programdata-account"
    {
        return Err(Error::new(format!(
            "saved plan role {} differs from its authenticated deployment-set evidence",
            role
        )));
    }
    let pair = plan
        .records
        .get(record_label)
        .ok_or_else(|| Error::new(format!("saved plan omitted {record_label}")))?;
    if pair.schema_id != hex(&ARTIFACT_RELEASE_SCHEMA_ID_V1) {
        return Err(Error::new(format!(
            "saved plan {record_label} substituted the ArtifactRelease schema"
        )));
    }
    let body = runtime::decode_hex(&pair.body_hex)?;
    let body_sha = hex(&<sha2::Sha256 as sha2::Digest>::digest(&body));
    let release = ArtifactReleaseV1::decode(&body).map_err(|error| {
        Error::new(format!(
            "saved plan {record_label} is not an ArtifactRelease: {error:?}"
        ))
    })?;
    if pair.content_sha256 != body_sha
        || pin.artifact_release_id != body_sha
        || release.program().to_bytes() != pubkey(&pin.program_id)?.to_bytes()
        || release.programdata() != pubkey(&pin.programdata_id)?.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.semantic_release_id().as_bytes()
            != &crate::plan::hex32(&pin.semantic_release_id)?
        || release.elf_digest() != crate::plan::hex32(&pin.live_elf_sha256)?
        || release.deployment_slot() != pin.deployment_slot
        || release.upgrade_authority() != Some(retained_authority.to_bytes())
    {
        return Err(Error::new(format!(
            "saved plan {record_label} differs from its checked mutable slot pin"
        )));
    }
    if let Some((carried_body, carried_id)) = evidence.carried_artifact()? {
        if carried_body != pair.body_hex || carried_id != body_sha {
            return Err(Error::new(format!(
                "saved plan replaced carried {role} ArtifactRelease bytes"
            )));
        }
    }
    Ok(())
}

/// A saved public-cluster plan is an untrusted projection. Before any keypair
/// file is loaded, rehash its mixed deployment-set evidence and bind every
/// mutable Program pin and artifact body back to that owner.
pub(crate) fn authenticate_checked_campaign_plan(
    plan: &SuccessorPlan,
    origin: &ClusterOriginV1,
) -> Result<()> {
    let mutable = runtime::role_pins(plan)
        .into_iter()
        .any(|(_, pin)| pin.upgrade_authority.is_some());
    let every_role_observed = runtime::role_pins(plan)
        .into_iter()
        .all(|(_, pin)| pin.deployment_source == "observed-programdata-account");
    require_checked_mutable_binding(
        mutable,
        plan.checked_upgrade_set.is_some(),
        plan.checked_local_mutable_set.is_some(),
        every_role_observed,
    )?;
    if plan.checked_local_mutable_set.is_some() {
        crate::cluster::ExpectedClusterV1::OwnedLoopback.authenticate(origin)?;
        crate::local_mutable::authenticate_checked_local_mutable_plan_v1(plan)?;
        let set = plan
            .checked_local_mutable_set
            .as_ref()
            .ok_or_else(|| Error::new("checked local mutable set disappeared"))?;
        let retained = pubkey(&set.retained_upgrade_authority)?;
        for role in &set.roles {
            authenticate_checked_plan_role_projection(
                plan,
                CheckedPlanRoleEvidenceV1::OwnedLoopback(role),
                retained,
            )?;
            let (_, record_label) = checked_plan_role(plan, &role.role)?;
            let _ = runtime::record(plan, record_label)?;
        }
        runtime::authenticate_infrastructure_profile_projection(plan)?;
        return runtime::authenticate_checked_activation_projection(plan);
    }
    let Some(set) = plan.checked_upgrade_set.as_ref() else {
        return Ok(());
    };
    crate::cluster::ExpectedClusterV1::Devnet.authenticate(origin)?;
    crate::upgrade::reauthenticate_checked_deployment_set_pin(set)?;
    if set.devnet_genesis_hash != crate::cluster::DEVNET_GENESIS_HASH
        || plan.record_publication != "transaction"
        || plan.core_bootstrap.release_recognition_requires_revoke
        || plan.core_bootstrap.upgrade_authority != set.retained_upgrade_authority
        || set.roles.len() != 7
    {
        return Err(Error::new(
            "saved checked plan header differs from its permanent-devnet deployment set",
        ));
    }
    let retained = pubkey(&set.retained_upgrade_authority)?;
    for role in &set.roles {
        authenticate_checked_plan_role_projection(
            plan,
            CheckedPlanRoleEvidenceV1::PermanentDevnet(role),
            retained,
        )?;
        let (_, record_label) = checked_role(plan, role)?;
        let _ = runtime::record(plan, record_label)?;
    }
    let carry = &set.infrastructure_carry_forward;
    if plan.infrastructure_profile.address != carry.profile_address
        || plan.infrastructure_profile.body_hex != carry.profile_body_hex
        || plan.infrastructure_profile.body_sha256 != carry.profile_body_sha256
        || plan.infrastructure_profile.registry_artifact_release_id
            != plan.registry.artifact_release_id
        || plan.infrastructure_profile.rent_artifact_release_id
            != plan.rent_credit.artifact_release_id
    {
        return Err(Error::new(
            "saved plan replaced the authenticated carried infrastructure profile",
        ));
    }
    runtime::authenticate_infrastructure_profile_projection(plan)?;
    runtime::authenticate_checked_activation_projection(plan)
}

fn require_checked_mutable_binding(
    mutable: bool,
    has_checked_upgrade_set: bool,
    has_checked_local_set: bool,
    every_role_observed: bool,
) -> Result<()> {
    if has_checked_upgrade_set && has_checked_local_set {
        return Err(Error::new(
            "saved plan mixed permanent-devnet and owned-loopback checked deployment evidence",
        ));
    }
    // A mutable plan must be BOUND to checked evidence, and a founded cohort
    // binds to the third kind: the observed ProgramData accounts themselves.
    // The two deployment sets are second copies of facts an upgrade produced;
    // a cohort that succeeds nothing produced no such copy, and its binding is
    // the observation `prepare` already authenticated against the declared
    // authority and that `substrate_state` re-verifies on chain before any key
    // is read. The requirement is unchanged -- evidence must be bound -- only
    // the admissible source is widened, and only for a plan in which EVERY
    // role is observed, so a half-observed plan still refuses.
    if mutable && !has_checked_upgrade_set && !has_checked_local_set && !every_role_observed {
        return Err(Error::new(
            "mutable saved plan is not bound to checked deployment-set evidence",
        ));
    }
    Ok(())
}

fn authenticate_live_checked_role(
    role: &str,
    pin: &ProgramPin,
    program: &crate::rpc::RpcAccount,
    programdata: &crate::rpc::RpcAccount,
    candidate: &[u8],
) -> Result<()> {
    authenticate_live_checked_role_with_succession(
        role,
        pin,
        program,
        programdata,
        candidate,
        false,
    )
}

fn authenticate_live_checked_role_with_succession(
    role: &str,
    pin: &ProgramPin,
    program: &crate::rpc::RpcAccount,
    programdata: &crate::rpc::RpcAccount,
    candidate: &[u8],
    allow_forward_slot: bool,
) -> Result<()> {
    let program_key = pubkey(&pin.program_id)?;
    let programdata_key = pubkey(&pin.programdata_id)?;
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|error| Error::new(format!("{role} Program account: {error:?}")))?;
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|error| Error::new(format!("{role} ProgramData account: {error:?}")))?;
    let authority = pin
        .upgrade_authority
        .as_deref()
        .map(pubkey)
        .transpose()?
        .map(|key| key.to_bytes());
    let live = programdata_view.elf();
    if program.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || program_view.programdata() != programdata_key.to_bytes()
        || Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0
            != programdata_key
        || programdata.owner != bpf_loader_upgradeable::ID
        || programdata.executable
        || (!allow_forward_slot
            && hex(&<sha2::Sha256 as sha2::Digest>::digest(&programdata.data))
                != pin.programdata_sha256)
        || (programdata_view.deployment_slot() != pin.deployment_slot
            && !(allow_forward_slot && programdata_view.deployment_slot() > pin.deployment_slot))
        || programdata_view.upgrade_authority() != authority
        || hex(&<sha2::Sha256 as sha2::Digest>::digest(live)) != pin.live_elf_sha256
        || live.get(..candidate.len()) != Some(candidate)
        || live.get(candidate.len()..).is_none_or(|padding| {
            padding.len() != pin.live_elf_padding_bytes || padding.iter().any(|byte| *byte != 0)
        })
    {
        return Err(Error::new(format!(
            "{role} live Program/ProgramData/link/slot/authority differs from the checked saved plan"
        )));
    }
    Ok(())
}

pub(crate) fn authenticate_checked_live_substrate(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<()> {
    let (roles, floor): (Vec<(&str, &ProgramPin)>, u64) =
        if let Some(set) = plan.checked_upgrade_set.as_ref() {
            let floor = set
                .roles
                .iter()
                .map(|role| role.deployment_slot)
                .chain(std::iter::once(
                    set.infrastructure_carry_forward.context_slot,
                ))
                .max()
                .ok_or_else(|| Error::new("checked deployment set omitted roles"))?;
            let roles = set
                .roles
                .iter()
                .map(|role| {
                    let (pin, _) = checked_role(plan, role)?;
                    Ok((role.role.as_str(), pin))
                })
                .collect::<Result<Vec<_>>>()?;
            (roles, floor)
        } else if let Some(set) = plan.checked_local_mutable_set.as_ref() {
            let floor = set
                .roles
                .iter()
                .map(|role| role.deployment_slot)
                .max()
                .ok_or_else(|| Error::new("checked local mutable set omitted roles"))?;
            let roles = set
                .roles
                .iter()
                .map(|role| {
                    let (pin, _) = checked_plan_role(plan, &role.role)?;
                    Ok((role.role.as_str(), pin))
                })
                .collect::<Result<Vec<_>>>()?;
            (roles, floor)
        } else {
            return Ok(());
        };
    let mut addresses = Vec::with_capacity(14);
    for (_, pin) in &roles {
        addresses.push(pubkey(&pin.program_id)?);
        addresses.push(pubkey(&pin.programdata_id)?);
    }
    let (_, accounts) = rpc.finalized_accounts(&addresses, floor)?;
    for (index, (role, pin)) in roles.into_iter().enumerate() {
        let program = accounts[index * 2]
            .as_ref()
            .ok_or_else(|| Error::new(format!("{role} Program is absent")))?;
        let programdata = accounts[index * 2 + 1]
            .as_ref()
            .ok_or_else(|| Error::new(format!("{role} ProgramData is absent")))?;
        let candidate = fs::read(&pin.checked_candidate_elf_path)?;
        authenticate_live_checked_role_with_succession(
            role,
            pin,
            program,
            programdata,
            &candidate,
            plan.infrastructure_succession.is_some() && role == "registry",
        )?;
    }
    Ok(())
}

/// Run the driver.
pub(crate) fn execute(args: CampaignArgsV1) -> Result<()> {
    let _evidence_lease = args
        .evidence_path
        .as_deref()
        .map(CampaignEvidenceLeaseV1::acquire)
        .transpose()?;
    execute_with_evidence_lease(args)
}

fn execute_with_evidence_lease(args: CampaignArgsV1) -> Result<()> {
    if args.execute && args.evidence_path.is_none() {
        return Err(Error::new(
            "--execute requires --evidence ABSOLUTE_JSON so intent is durable before any mutation",
        ));
    }
    if args.infrastructure_lineage_path.is_some()
        && (!args.execute
            || args.mode != CampaignModeV1::Administration
            || args.through != StageV1::Activation)
    {
        return Err(Error::new(
            "standalone infrastructure lineage requires executed administration through activation",
        ));
    }
    if args.infrastructure_lineage_path.as_ref() == args.evidence_path.as_ref() {
        return Err(Error::new(
            "campaign report and standalone infrastructure lineage require distinct output paths",
        ));
    }
    match args.mode {
        CampaignModeV1::Administration => {
            authenticate_keypair_paths(&args.keypairs, &[], ADMIN_ALLOWED_ROLES)?;
            if args.market_path.is_some()
                || args.founding_founder.is_some()
                || args.substituted_founder.is_some()
            {
                return Err(Error::new(
                    "administration mode is infrastructure-only through activation",
                ));
            }
            authenticate_administration_through_v1(args.through)?;
        }
        CampaignModeV1::FoundingOnly => {
            let mut allowed = FOUNDING_REQUIRED_ROLES.to_vec();
            allowed.extend([
                crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
                crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
            ]);
            authenticate_keypair_paths(&args.keypairs, FOUNDING_REQUIRED_ROLES, &allowed)?;
            if args.market_path.is_none()
                || args.founding_founder.is_none()
                || args.substituted_founder.is_none()
                || args.through != StageV1::Founding
            {
                return Err(Error::new(
                    "founding-only mode requires its Market, two public founder identities, and through=founding",
                ));
            }
        }
    }
    let plan_source = fs::read(&args.plan_path)?;
    let plan: SuccessorPlan =
        serde_json::from_value(parse_json_without_duplicate_keys_v1(&plan_source)?)?;
    authenticate_checked_campaign_plan(&plan, &args.origin)?;
    let plan_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(&plan_source));
    // Decode the complete dossier before connection or key access. The same
    // bytes are hashed into both the campaign report and its outer CLI journal.
    let market_source = args.market_path.as_ref().map(fs::read).transpose()?;
    let market_sha256 = market_source
        .as_ref()
        .map(|bytes| hex(&<sha2::Sha256 as sha2::Digest>::digest(bytes)));
    let market: Option<crate::model::MarketRunInput> = market_source
        .as_ref()
        .map(|bytes| load_market_input(bytes))
        .transpose()?;
    authenticate_local_participant_fixture_policy_v1(
        &args.origin,
        market.as_ref(),
        &args.keypairs,
    )?;
    if args.mode == CampaignModeV1::FoundingOnly {
        let input = market
            .as_ref()
            .ok_or_else(|| Error::new("founding-only Market disappeared after parsing"))?;
        let roles = founding_keypair_roles_v1(input);
        authenticate_keypair_paths(&args.keypairs, &roles, &roles)?;
    }
    let prior = match &args.evidence_path {
        None => PriorCampaignEvidenceV1 {
            checkpoint: None,
            founding_submission_journals: BTreeMap::new(),
            terminal_consumable_source: None,
        },
        Some(path) => load_prior_campaign_evidence(
            path,
            &plan_sha256,
            market_sha256.as_deref(),
            args.origin.label(),
            &args.origin.redacted_url(),
        )?,
    };
    if let Some(source) = prior.terminal_consumable_source {
        eprintln!(
            "campaign: exact terminal-consumable completion already exists; preserved byte-for-byte"
        );
        let mut stdout = std::io::stdout();
        stdout.write_all(&source)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }
    let mut compatible_checkpoint = prior.checkpoint;
    let mut founding_submission_journals = prior.founding_submission_journals;
    let policy = if args.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&args.origin, policy)?;
    let observed_genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    args.origin.authenticate_genesis(&observed_genesis_hash)?;
    authenticate_checked_live_substrate(&mut rpc, &plan)?;

    // Key-free detectors first. Their authenticated result is fsynced before
    // the process opens any keypair file.
    let (substrate, observed_roles) = substrate_state(&mut rpc, &plan)?;
    // The activated release must be the one actually running. Costs no extra
    // RPC: substrate_state already read every role's live slot and ELF digest.
    crate::release_identity::authenticate_activated_release_is_live_v1(&plan, &observed_roles)?;
    let publication = publication_state(&mut rpc, &plan)?;
    let initialize = initialize_state(&mut rpc, &plan)?;
    let succession = succession_state(&mut rpc, &plan)?;
    let activation_progress = runtime::activation_progress(&mut rpc, &plan);
    let activated_prefix = activation_progress
        .as_ref()
        .ok()
        .and_then(|progress| *progress)
        .map(ActivationCacheProgressV1::written_count)
        .unwrap_or(0);
    let activation_compute = if activation_progress.is_ok() {
        activation_compute_preflight_v1(&observed_roles, activated_prefix)?
    } else {
        Vec::new()
    };
    let activation_address = pubkey(&plan.activation)?;
    let activation = activation_state_from_progress(activation_address, activation_progress);
    let mut states = assemble_infrastructure_stage_states_v1(
        substrate,
        publication,
        initialize,
        succession,
        activation,
    );
    let pyth = match &args.origin {
        ClusterOriginV1::AcknowledgedDevnet { .. } => Some(authenticate_pyth_row(&mut rpc)?),
        // The committed row is a devnet fact. Authenticating it against a
        // local ledger that has never seen the Pyth programs would produce a
        // page of false negatives and teach nobody anything.
        ClusterOriginV1::Loopback { .. } => None,
    };
    if args.mode == CampaignModeV1::FoundingOnly {
        authenticate_founding_only_prerequisites_v1(&states)?;
    }

    let mut report = json!({
        "schema": "dclutch-successor-campaign-report-v1",
        "cluster": args.origin.label(),
        "genesis_hash": observed_genesis_hash,
        "rpc_url": args.origin.redacted_url(),
        "mode": if args.execute { "execute" } else { "preflight (reads only, enforced)" },
        "plan": args.plan_path.display().to_string(),
        "plan_sha256": plan_sha256,
        "market_input": args.market_path.as_ref().map(|path| path.display().to_string()),
        "market_sha256": market_sha256,
        "evidence_output": args.evidence_path.as_ref().map(|path| path.display().to_string()),
        "infrastructureLineageEvidence": Value::Null,
        "through_stage": args.through.name(),
        "execution_intent": {
            "authorized_mutation": args.execute,
            "campaign_mode": args.mode.name(),
            "through_stage": args.through.name(),
            "plan": args.plan_path.display().to_string(),
            "market": args.market_path.as_ref().map(|path| path.display().to_string()),
            "founding_founder": args.founding_founder.map(|value| value.to_string()),
            "substituted_founder": args.substituted_founder.map(|value| value.to_string()),
        },
        "pre_key_checkpoint": {
            "durable": args.evidence_path.is_some(),
            "plan_authenticated": true,
            "live_substrate_authenticated": true,
            "keypair_files_read": false,
        },
        "payer": Value::Null,
        "keypair_derivation": "not-read",
        "private_key_persisted": true,
        "stages": states.iter().map(|(stage, state)| json!({
            "stage": stage.name(),
            "state": state.label(),
            "detail": state.detail(),
        })).collect::<Vec<_>>(),
        "roles": observed_roles.iter().map(|row| json!({
            "role": row.role,
            "program_id": row.program_id,
            "programdata_id": row.programdata_id,
            "observed_deployment_slot": row.observed_slot,
            "release_binds_deployment_slot": row.pinned_slot,
            "slot_pin_holds": row.slot_pin_holds(),
            "observed_upgrade_authority": row.observed_authority,
            "plan_upgrade_authority": row.pinned_authority,
            "upgrade_authority_pin_holds": row.authority_pin_holds(),
            "observed_programdata_owner": row.observed_owner,
            "loader_owner_holds": row.loader_owner_holds(),
            "observed_programdata_executable": row.observed_executable,
            "observed_live_elf_sha256": row.observed_live_elf_sha256,
            "release_binds_live_elf_sha256": row.pinned_live_elf_sha256,
            "checked_candidate_elf_sha256": row.checked_candidate_elf_sha256,
            "live_elf_padding_bytes": row.live_elf_padding_bytes,
            "observed_programdata_bytes": row.observed_data_len,
            "loader_metadata_bytes": LOADER_V3_PROGRAMDATA_METADATA_BYTES,
        })).collect::<Vec<_>>(),
        "activation_compute_preflight": {
            "agave_runtime": "4.0.2",
            "transaction_ceiling_compute_units": runtime::ACTIVATION_TRANSACTION_CU_LIMIT_V1,
            "maximum_live_elf_bytes": runtime::MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1,
            "size_only_not_a_measured_cu_substitute": true,
            "roles": activation_compute.iter().map(|row| json!({
                "role": row.role,
                "pending": row.pending,
                "live_elf_bytes": row.live_elf_bytes,
                "conservative_compute_units": row.conservative_compute_units,
                "headroom_compute_units": row.headroom_compute_units,
            })).collect::<Vec<_>>(),
        },
        "wallet": Value::Null,
        "pyth_devnet_release_authentication": pyth.as_ref().map(|rows| rows.iter().map(|(what, ok, detail)| json!({
            "fact": what,
            "holds": ok,
            "observed": detail,
        })).collect::<Vec<_>>()),
        "founding_targets": Value::Null,
        "foundingSubmissionJournals": founding_submission_journals.values().cloned().collect::<Vec<_>>(),
        "deploy_ladder": deploy_ladder(&plan, &args.origin),
        "transport_policy": "driver traffic: paced RPC (SMOKE-0 §6.4 -- the founding ladder and \
                             life are RPC-shaped end to end). Buffer writes: TPU, via the solana \
                             CLI, never this process (SMOKE-0 §3.1 -- ~100x, and it is the CLI's \
                             ladder, not the driver's).",
    });
    if let Some(checkpoint) = &compatible_checkpoint {
        report["foundingCheckpoint"] = serde_json::to_value(checkpoint)?;
    }
    if let Some(path) = &args.evidence_path {
        write_evidence_atomically(path, &report)?;
    } else {
        // A stdout-only read rehearsal stays completely key-free. Supplying a
        // durable evidence path opts into the key-dependent payer/founding
        // detector pass while still remaining mutation-free without execute.
        let mut stdout = std::io::stdout();
        stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }

    if args.mode == CampaignModeV1::Administration {
        if compatible_checkpoint.is_some() || !founding_submission_journals.is_empty() {
            return Err(Error::new(
                "administration evidence must not carry a founding checkpoint or submission journal",
            ));
        }
        if !args.execute {
            let mut stdout = std::io::stdout();
            stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
            stdout.write_all(b"\n")?;
            return Ok(());
        }
        for (stage, state) in states.iter().filter(|(stage, _)| *stage <= args.through) {
            if *stage == StageV1::Substrate && state != &StageStateV1::Complete {
                return Err(Error::new(
                    "administration cannot open a signer while the substrate is not Complete",
                ));
            }
            if let StageStateV1::Conflict(detail) = state {
                return Err(Error::new(format!(
                    "administration stage {} conflicts before any key read: {detail}",
                    stage.name()
                )));
            }
        }
        let execution = if administration_requires_authority_v1(&states, args.through) {
            let required_roles = administration_required_roles_v1(&states, args.through);
            authenticate_keypair_paths(&args.keypairs, &required_roles, &required_roles)?;
            let forge =
                KeyForge::persisted(load_campaign_keypairs(&args.keypairs)?, &required_roles)?;
            let authority = forge.keypair(role::CORE_UPGRADE_AUTHORITY);
            if required_roles.contains(&role::CAMPAIGN_PAYER)
                && forge.peek_pubkey(role::CAMPAIGN_PAYER)? == authority.pubkey()
            {
                return Err(Error::new(
                    "succession fee payer aliases the Core/Registry consent authority; Loader and ceremony privileges would merge",
                ));
            }
            let wallet = wallet_arithmetic(&mut rpc, &plan, authority.pubkey())?;
            report["pre_key_checkpoint"]["keypair_files_read"] = json!(true);
            report["payer"] = json!(authority.pubkey().to_string());
            report["keypair_derivation"] = json!(forge.derivation_label());
            report["private_key_persisted"] = json!(forge.persists_private_keys());
            report["wallet"] = serde_json::to_value(&json!({
                "payer": wallet.payer,
                "balance_lamports": wallet.balance_lamports,
                "record_rent_lamports": wallet.record_rent_lamports,
                "profile_rent_lamports": wallet.profile_rent_lamports,
                "activation_rent_lamports": wallet.activation_rent_lamports,
                "estimated_fee_lamports": wallet.estimated_fee_lamports,
                "required_lamports": wallet.required_lamports,
                "shortfall_lamports": wallet.shortfall(),
                "may_airdrop": args.origin.may_airdrop(),
                "funding": if args.origin.may_airdrop() {
                    "this origin's faucet is the campaign's own, so a shortfall is not a blocker"
                } else {
                    "this driver never airdrops: fund the payer before --execute so a shortfall refuses before the ladder"
                },
            }))?;
            if let Some(path) = &args.evidence_path {
                write_evidence_atomically(path, &report)?;
            }
            execute_stages(
                &mut rpc,
                &plan,
                &authority,
                &forge,
                None,
                None,
                None,
                &states,
                args.through,
                None,
                &mut |_| Ok(()),
                None,
            )?
        } else {
            for (stage, state) in states.iter().filter(|(stage, _)| *stage <= args.through) {
                if state != &StageStateV1::Complete {
                    return Err(Error::new(format!(
                        "administration stage {} is {} but has no admissible mutation route",
                        stage.name(),
                        state.label()
                    )));
                }
            }
            CampaignExecutionEvidenceV1 {
                transactions: Vec::new(),
                market: None,
                recovered_finalized_founding: false,
            }
        };
        report["execution"] = json!({
            "completed": true,
            "recoveredFinalizedFounding": execution.recovered_finalized_founding,
            "transactions": execution.transactions,
            "market": execution.market,
        });
        if let Some(lineage_path) = args.infrastructure_lineage_path.as_deref() {
            let campaign_evidence_path = args.evidence_path.as_deref().ok_or_else(|| {
                Error::new("standalone infrastructure lineage omitted its campaign evidence path")
            })?;
            let lineage = infrastructure_lineage_evidence_v1(
                &mut rpc,
                &plan,
                &observed_genesis_hash,
                &plan_sha256,
                campaign_evidence_path,
            )?;
            let digest = write_stable_lineage_evidence_v1(lineage_path, &lineage)?;
            report["infrastructureLineageEvidence"] = json!({
                "path": lineage_path.display().to_string(),
                "sha256": digest,
                "schema": "dclutch-current-source-infrastructure-lineage-v1",
            });
        }
        if let Some(path) = &args.evidence_path {
            write_evidence_atomically(path, &report)?;
        }
        let mut stdout = std::io::stdout();
        stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }

    // Recover an ambiguous founding packet before opening any key file. A
    // Prepared advances only to durable Dispatching. Dispatching may resend
    // only its already-signed bytes after exact prestate reauthentication; a
    // Submitted journal is strictly poll-only.
    if !founding_submission_journals.is_empty() {
        let payer = founding_submission_journals
            .values()
            .next()
            .ok_or_else(|| Error::new("founding journal set disappeared"))?
            .payer
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("founding journal payer: {error}")))?;
        let evidence_path = args
            .evidence_path
            .as_deref()
            .ok_or_else(|| Error::new("founding journal recovery omitted evidence path"))?;
        let market_digest = market_sha256
            .as_deref()
            .ok_or_else(|| Error::new("founding journal recovery omitted Market digest"))?;
        // The post-Open funding-readiness frames are two accounts narrower
        // without a recovery policy, so the journal cannot re-check its own
        // recorded geometry without knowing which market it belongs to.
        let market_has_recovery_policy = !market
            .as_ref()
            .ok_or_else(|| Error::new("founding journal recovery omitted the Market input"))?
            .recovery_policy_hex
            .is_empty();
        let binding = crate::market::founding_submission_journal::FoundingSubmissionBindingV1::new(
            args.origin.label(),
            &observed_genesis_hash,
            evidence_path,
            args.origin.redacted_url(),
            plan_sha256.clone(),
            market_digest,
            payer,
            market_has_recovery_policy,
        )?;
        let operations = founding_submission_journals
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for operation in operations {
            let mut journal = founding_submission_journals
                .get(&operation)
                .cloned()
                .ok_or_else(|| Error::new("founding recovery journal disappeared"))?;
            let mut action =
                crate::market::founding_submission_journal::founding_submission_recovery_v1(
                    &binding, &journal,
                )?;
            let mut completed_locally = false;
            let finalized_packet = match action {
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::SignOnce
                | crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::BeginDispatch => None,
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::Complete => {
                    crate::market::authenticate_completed_founding_submission_v1(
                        &mut rpc,
                        operation.label(),
                        &binding,
                        &journal,
                    )?;
                    completed_locally = true;
                    None
                }
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::ResendIdenticalPacket
                | crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::PollOnly => {
                    let signature = journal
                        .expected_signature
                        .as_deref()
                        .ok_or_else(|| Error::new("ambiguous founding journal omitted signature"))?
                        .parse::<solana_sdk::signature::Signature>()
                        .map_err(|error| Error::new(format!("ambiguous founding signature: {error}")))?;
                    rpc.finalized_signed_packet(operation.label(), signature, false)?
                }
            };
            if let Some(finalized) = finalized_packet {
                let persistence = observed_finalization_persistence_v1(journal.phase)?;
                // A chain-finalized Dispatching packet first advances through
                // the adjacent Submitted phase locally. Neither transition
                // opens a key or a send path, and each is fsynced separately.
                if persistence
                    == ObservedFinalizationPersistenceV1::SubmittedThenFinalizedThenCheckpoint
                {
                    let signature = journal.expected_signature.clone().ok_or_else(|| {
                        Error::new("Dispatching founding journal omitted signature")
                    })?;
                    journal =
                        crate::market::founding_submission_journal::submit_founding_submission_v1(
                            &binding, &journal, &signature,
                        )?;
                    founding_submission_journals.insert(operation, journal.clone());
                    report["foundingSubmissionJournals"] = serde_json::to_value(
                        founding_submission_journals
                            .values()
                            .cloned()
                            .collect::<Vec<_>>(),
                    )?;
                    write_evidence_atomically(evidence_path, &report)?;
                }
                journal = crate::market::finalize_observed_founding_submission_v1(
                    &mut rpc, &binding, &journal, &finalized,
                )?;
                founding_submission_journals.insert(operation, journal.clone());
                report["foundingSubmissionJournals"] = serde_json::to_value(
                    founding_submission_journals
                        .values()
                        .cloned()
                        .collect::<Vec<_>>(),
                )?;
                write_evidence_atomically(evidence_path, &report)?;
                crate::market::authenticate_completed_founding_submission_v1(
                    &mut rpc,
                    operation.label(),
                    &binding,
                    &journal,
                )?;
                completed_locally = true;
                action = crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::Complete;
            }
            if completed_locally {
                let materialized = match operation {
                    crate::market::founding_submission_journal::FoundingSubmissionOperationV1::Dcltcfq1
                        if compatible_checkpoint.is_none() =>
                    {
                        Some(crate::market::materialize_dcltcfq1_checkpoint_v1(
                            &mut rpc,
                            &binding,
                            &journal,
                        )?)
                    }
                    crate::market::founding_submission_journal::FoundingSubmissionOperationV1::Dcltpcb2
                        if compatible_checkpoint.as_ref().is_none_or(|checkpoint| {
                            checkpoint.schema
                                != crate::market::DCLTPCB2_CHECKPOINT_SCHEMA_V1
                        }) =>
                    {
                        Some(crate::market::materialize_dcltpcb2_checkpoint_v1(
                            &mut rpc,
                            &binding,
                            &journal,
                        )?)
                    }
                    _ => None,
                };
                if let Some(checkpoint) = materialized {
                    report["foundingCheckpoint"] = serde_json::to_value(&checkpoint)?;
                    compatible_checkpoint = Some(checkpoint);
                    // The journal is already durably Finalized. Checkpoint
                    // materialization is always the strictly later fsync.
                    write_evidence_atomically(evidence_path, &report)?;
                }
                continue;
            }
            if action
                == crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::BeginDispatch
                && args.execute
            {
                authenticate_founding_dispatch_ready_v1(&mut rpc, &binding, &journal)?;
                let dispatching = crate::market::founding_submission_journal::dispatch_founding_submission_v1(
                    &binding,
                    &journal,
                )?;
                founding_submission_journals.insert(operation, dispatching.clone());
                report["foundingSubmissionJournals"] = serde_json::to_value(
                    founding_submission_journals.values().cloned().collect::<Vec<_>>(),
                )?;
                // Dispatching, including the exact packet/signature, is
                // atomically replaced and fsynced before the native pre-send hook.
                write_evidence_atomically(evidence_path, &report)?;
                journal = dispatching;
                action = crate::market::founding_submission_journal::founding_submission_recovery_v1(
                    &binding,
                    &journal,
                )?;
            }
            match action {
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::ResendIdenticalPacket
                    if args.execute =>
                {
                    authenticate_founding_dispatch_ready_v1(&mut rpc, &binding, &journal)?;
                    let packet = crate::market::founding_submission_journal::founding_submission_packet_v1(
                        &binding,
                        &journal,
                    )?;
                    let signature = journal
                        .expected_signature
                        .as_deref()
                        .ok_or_else(|| Error::new("Dispatching founding journal omitted signature"))?
                        .parse::<solana_sdk::signature::Signature>()
                        .map_err(|error| Error::new(format!("Dispatching founding signature: {error}")))?;
                    let projection = crate::market::founding_submission_journal::founding_pre_send_projection_v1(
                        &binding,
                        &journal,
                    )?;
                    if projection.signature != signature.to_string() {
                        return Err(Error::new(
                            "Dispatching recovery pre-send projection changed signature",
                        ));
                    }
                    // Dispatching, including the exact packet/signature, was
                    // fsynced in the native campaign report before this sole
                    // send. A kill here or before Submitted is persisted may
                    // retry only the identical signature; Submitted is poll-only.
                    let returned = rpc.submit_signed_packet_once(
                        operation.label(),
                        &packet,
                        signature,
                        false,
                    )?;
                    let submitted = crate::market::founding_submission_journal::submit_founding_submission_v1(
                        &binding,
                        &journal,
                        &returned.to_string(),
                    )?;
                    founding_submission_journals.insert(operation, submitted);
                    report["foundingSubmissionJournals"] = serde_json::to_value(
                        founding_submission_journals.values().cloned().collect::<Vec<_>>(),
                    )?;
                    write_evidence_atomically(evidence_path, &report)?;
                    let mut stdout = std::io::stdout();
                    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
                    stdout.write_all(b"\n")?;
                    return Ok(());
                }
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::BeginDispatch
                | crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::ResendIdenticalPacket => {
                    let mut stdout = std::io::stdout();
                    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
                    stdout.write_all(b"\n")?;
                    return Ok(());
                }
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::PollOnly => {
                    let mut stdout = std::io::stdout();
                    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
                    stdout.write_all(b"\n")?;
                    return Ok(());
                }
                crate::market::founding_submission_journal::FoundingSubmissionRecoveryV1::SignOnce => {
                    // A persisted Planned recovery reauthenticates expiry and
                    // exact prestate before the campaign may reopen key files.
                    authenticate_founding_dispatch_ready_v1(&mut rpc, &binding, &journal)?;
                }
                _ => {}
            }
        }
    }

    // This is the first private-key read in the campaign. Every parser,
    // mutable-plan pin, live ProgramData observation, and key-free stage
    // detector above is already represented by an fsynced report.
    let market_input = market
        .as_ref()
        .ok_or_else(|| Error::new("founding-only execution omitted its Market input"))?;
    let founding_roles = founding_keypair_roles_v1(market_input);
    let secrets = load_campaign_keypairs(&args.keypairs)?;
    let founding_founder = args
        .founding_founder
        .ok_or_else(|| Error::new("founding-only execution omitted its founder"))?;
    let substituted_founder = args
        .substituted_founder
        .ok_or_else(|| Error::new("founding-only execution omitted its substituted founder"))?;
    authenticate_founding_actor_partition_v1(founding_founder, substituted_founder, &secrets)?;
    let actors = crate::market::FoundingActorsV1::new(founding_founder, substituted_founder)?;
    let forge = KeyForge::persisted(secrets, &founding_roles)?;
    let payer = forge.keypair(role::CAMPAIGN_PAYER);
    // PEEKED, never drawn: the detector must name the exact next key without
    // advancing the forge's issuance index.
    let founding_keys = match &market {
        None => None,
        Some(_) => Some((
            forge.peek_pubkey(role::COLLATERAL_MINT)?,
            forge.peek_pubkey(role::COLLATERAL_WALLET)?,
        )),
    };
    let founding_targets = match (&market, founding_keys) {
        (Some(input), Some((mint, wallet))) => {
            let (state, targets) = founding_state(&mut rpc, &plan, input, mint, wallet)?;
            states.push((StageV1::Founding, state));
            Some(targets)
        }
        _ => None,
    };
    let wallet = wallet_arithmetic(&mut rpc, &plan, payer.pubkey())?;
    report["pre_key_checkpoint"]["keypair_files_read"] = json!(true);
    report["payer"] = json!(payer.pubkey().to_string());
    report["keypair_derivation"] = json!(forge.derivation_label());
    report["private_key_persisted"] = json!(forge.persists_private_keys());
    report["stages"] = json!(
        states
            .iter()
            .map(|(stage, state)| json!({
                "stage": stage.name(),
                "state": state.label(),
                "detail": state.detail(),
            }))
            .collect::<Vec<_>>()
    );
    report["wallet"] = json!({
        "payer": wallet.payer,
        "balance_lamports": wallet.balance_lamports,
        "record_rent_lamports": wallet.record_rent_lamports,
        "profile_rent_lamports": wallet.profile_rent_lamports,
        "activation_rent_lamports": wallet.activation_rent_lamports,
        "estimated_fee_lamports": wallet.estimated_fee_lamports,
        "required_lamports": wallet.required_lamports,
        "shortfall_lamports": wallet.shortfall(),
        "may_airdrop": args.origin.may_airdrop(),
        "funding": if args.origin.may_airdrop() {
            "this origin's faucet is the campaign's own, so a shortfall is not a blocker"
        } else {
            "this driver never airdrops: fund the payer before --execute so a shortfall refuses before the ladder"
        },
    });
    report["founding_targets"] = founding_targets.as_ref().map_or(Value::Null, |targets| {
        json!({
            "market_input": args.market_path.as_ref().map(|path| path.display().to_string()),
            "collateral_mint": targets.collateral_mint.to_string(),
            "realm_record": targets.realm_record.to_string(),
            "found31_market": targets.found31_market.to_string(),
            "open_market": targets.open_market.to_string(),
            "abort_market": targets.abort_market.to_string(),
        })
    });
    // Full key-dependent preflight is also durable before the first send.
    if let Some(path) = &args.evidence_path {
        write_evidence_atomically(path, &report)?;
    }

    if !args.execute {
        let mut stdout = std::io::stdout();
        stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }
    let report_cell = RefCell::new(report);
    let sealing_binding;
    let execution = {
        let mut checkpoint = |value: &crate::market::MarketExecutionCheckpointV1| -> Result<()> {
            let mut report = report_cell.borrow_mut();
            report["foundingCheckpoint"] = serde_json::to_value(value)?;
            if let Some(path) = &args.evidence_path {
                write_evidence_atomically(path, &report)?;
            }
            Ok(())
        };
        let mut persist_journals = |journals: &[crate::market::founding_submission_journal::FoundingSubmissionJournalV1]| -> Result<()> {
            let mut report = report_cell.borrow_mut();
            report["foundingSubmissionJournals"] = serde_json::to_value(journals)?;
            if let Some(path) = &args.evidence_path {
                write_evidence_atomically(path, &report)?;
            }
            Ok(())
        };
        let evidence_path = args
            .evidence_path
            .as_deref()
            .ok_or_else(|| Error::new("founding execution omitted evidence path"))?;
        let market_sha256 = market_sha256
            .as_deref()
            .ok_or_else(|| Error::new("founding execution omitted Market digest"))?;
        let binding = crate::market::founding_submission_journal::FoundingSubmissionBindingV1::new(
            args.origin.label(),
            &observed_genesis_hash,
            evidence_path,
            args.origin.redacted_url(),
            plan_sha256.clone(),
            market_sha256,
            payer.pubkey(),
            !market_input.recovery_policy_hex.is_empty(),
        )?;
        // The binding is moved into the recorder and dropped with it; the
        // sealing totality pass below needs the same identity to annotate any
        // journal the run never observed finalize.
        sealing_binding = binding.clone();
        let mut recorder = Some(crate::market::FoundingSubmissionRecorderV1::new(
            binding,
            &mut founding_submission_journals,
            &mut persist_journals,
        )?);
        execute_stages(
            &mut rpc,
            &plan,
            &payer,
            &forge,
            Some(actors),
            market.as_ref(),
            founding_keys,
            &states,
            args.through,
            compatible_checkpoint.as_ref(),
            &mut checkpoint,
            recorder.as_mut(),
        )?
    };
    let mut report = report_cell.into_inner();
    // Sealing-time fee totality: any journal the run never observed finalize
    // either resolves against the chain now or gains an explicit
    // unresolved-fee marker, so the report accounts every send it made. This
    // pass may not abort the seal; every refusal degrades inside it.
    if crate::market::resolve_stranded_founding_submissions_v1(
        &mut rpc,
        &sealing_binding,
        &mut founding_submission_journals,
    ) {
        report["foundingSubmissionJournals"] = serde_json::to_value(
            founding_submission_journals
                .values()
                .cloned()
                .collect::<Vec<_>>(),
        )?;
        if let Some(path) = &args.evidence_path {
            write_evidence_atomically(path, &report)?;
        }
    }
    let local_participant_fixture_liquidity = execution
        .market
        .as_ref()
        .and_then(|market| market.local_participant_fixture_liquidity.as_ref())
        .map(serde_json::to_value)
        .transpose()?;
    report["execution"] = json!({
        "completed": true,
        "recoveredFinalizedFounding": execution.recovered_finalized_founding,
        "transactions": execution.transactions,
        "market": execution.market,
    });
    if let Some(receipt) = local_participant_fixture_liquidity {
        report["execution"]["localParticipantFixtureLiquidity"] = receipt;
    }
    if let Some(path) = &args.evidence_path {
        write_evidence_atomically(path, &report)?;
    }
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn authenticate_founding_dispatch_ready_v1(
    rpc: &mut Rpc,
    binding: &crate::market::founding_submission_journal::FoundingSubmissionBindingV1,
    journal: &crate::market::founding_submission_journal::FoundingSubmissionJournalV1,
) -> Result<()> {
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("founding recovery block height was not u64"))?;
    crate::market::founding_submission_journal::authenticate_founding_packet_fresh_v1(
        binding, journal, height,
    )?;
    let prestate_accounts = journal
        .prestate_accounts
        .iter()
        .map(|value| {
            value
                .parse::<Pubkey>()
                .map_err(|error| Error::new(format!("founding prestate account: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if crate::market::founding_account_set_digest_v1(rpc, &prestate_accounts)?
        != journal.prestate_sha256
    {
        return Err(Error::new(format!(
            "{} recovery found changed prestate; poll the exact signature and do not dispatch or resend",
            journal.operation.label()
        )));
    }
    Ok(())
}

/// Advance the chain through the requested stages, skipping what is done.
///
/// The stages through activation sign with the Core authority alone; the
/// founding is the one that needs the forge's other roles and the market
/// input, and it refuses by name when the input is absent.
pub(crate) struct CampaignExecutionEvidenceV1 {
    pub(crate) transactions: Vec<crate::model::TransactionEvidence>,
    pub(crate) market: Option<crate::market::MarketExecutionEvidence>,
    pub(crate) recovered_finalized_founding: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoundingCheckpointResumeV1 {
    PreparedControllerFunding,
    CustodyStaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObservedFinalizationPersistenceV1 {
    SubmittedThenFinalizedThenCheckpoint,
    FinalizedThenCheckpoint,
}

fn observed_finalization_persistence_v1(
    phase: crate::market::founding_submission_journal::FoundingSubmissionPhaseV1,
) -> Result<ObservedFinalizationPersistenceV1> {
    match phase {
        crate::market::founding_submission_journal::FoundingSubmissionPhaseV1::Dispatching => {
            Ok(ObservedFinalizationPersistenceV1::SubmittedThenFinalizedThenCheckpoint)
        }
        crate::market::founding_submission_journal::FoundingSubmissionPhaseV1::Submitted => {
            Ok(ObservedFinalizationPersistenceV1::FinalizedThenCheckpoint)
        }
        _ => Err(Error::new(
            "observed finalized packet did not start from Dispatching or Submitted",
        )),
    }
}

fn founding_checkpoint_resume_v1(
    checkpoint: &crate::market::MarketExecutionCheckpointV1,
) -> Result<FoundingCheckpointResumeV1> {
    match checkpoint.schema.as_str() {
        crate::market::DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1 => {
            Ok(FoundingCheckpointResumeV1::PreparedControllerFunding)
        }
        crate::market::DCLTPCB2_CHECKPOINT_SCHEMA_V1 => {
            Ok(FoundingCheckpointResumeV1::CustodyStaged)
        }
        schema => Err(Error::new(format!(
            "partial founding checkpoint uses unsupported schema {schema}"
        ))),
    }
}

fn execute_succession_stage_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    authority: &Keypair,
    forge: &KeyForge,
    initial_state: &StageStateV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    let pin = plan.infrastructure_succession.as_ref().ok_or_else(|| {
        Error::new("succession executor reached a plan with no checked-local succession pin")
    })?;
    let core = pubkey(&plan.core.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let v1_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
    let predecessor_bytes = rpc
        .account(v1_address)?
        .ok_or_else(|| Error::new("succession executor cannot read the V1 predecessor profile"))?
        .data;
    if predecessor_bytes != runtime::decode_hex(&plan.infrastructure_profile.body_hex)? {
        return Err(Error::new(
            "succession executor observed changed V1 predecessor bytes",
        ));
    }

    if initial_state == &StageStateV1::Absent {
        let buffer = pubkey(&pin.registry_upgrade_buffer)?;
        let instruction = solana_loader_v3_interface::instruction::upgrade(
            &registry,
            &buffer,
            &authority.pubkey(),
            &authority.pubkey(),
        );
        transactions.push(rpc.send(
            "upgrade Registry for infrastructure succession",
            &[instruction],
            authority,
        )?);
        match succession_state(rpc, plan)? {
            StageStateV1::Partial(_) => {}
            state => {
                return Err(Error::new(format!(
                    "Registry Upgrade landed but succession detector read {}{}",
                    state.label(),
                    state
                        .detail()
                        .map(|detail| format!(": {detail}"))
                        .unwrap_or_default()
                )));
            }
        }
    }

    let programdata = rpc
        .account(pubkey(&plan.registry.programdata_id)?)?
        .ok_or_else(|| Error::new("Registry ProgramData disappeared after Upgrade"))?;
    let view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|error| Error::new(format!("Registry ProgramData after Upgrade: {error:?}")))?;
    let predecessor = ProtocolInfrastructureProfileV1::decode(&predecessor_bytes)
        .map_err(|error| Error::new(format!("V1 predecessor profile: {error:?}")))?;
    let projection = succession_projection_v1(plan, predecessor, view.deployment_slot())?;
    let successor_bytes = projection.registry_release.to_bytes();
    runtime::publish_record(
        rpc,
        registry,
        authority,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        &successor_bytes,
        None,
        transactions,
    )?;
    match succession_state(rpc, plan)? {
        StageStateV1::Partial(_) => {}
        StageStateV1::Complete => return Ok(()),
        state => {
            return Err(Error::new(format!(
                "successor Registry record landed but succession detector read {}{}",
                state.label(),
                state
                    .detail()
                    .map(|detail| format!(": {detail}"))
                    .unwrap_or_default()
            )));
        }
    }

    let payer = forge.keypair(role::CAMPAIGN_PAYER);
    if payer.pubkey() == authority.pubkey() {
        return Err(Error::new(
            "succession fee payer aliases the consenting authority",
        ));
    }
    let report = crate::infrastructure_succession::plan_for_campaign_v1(
        rpc,
        core,
        projection.registry_artifact_id.to_bytes(),
        predecessor.rent().artifact_release().to_bytes(),
        payer.pubkey(),
    )?;
    let expected_signers = [payer.pubkey(), authority.pubkey()];
    if report.required_signers.as_slice() != expected_signers {
        return Err(Error::new(format!(
            "shipped succession builder required {:?}, expected the distinct payer then the one retained Core/Registry authority",
            report.required_signers
        )));
    }
    let simulation = rpc.simulate_v0(
        "infrastructure-succession",
        &[report.instruction.clone()],
        payer.pubkey(),
        report.observation,
        &[],
    )?;
    if !simulation.accepted() {
        return Err(Error::new(format!(
            "succession ceremony refused in simulation: {:?}",
            simulation.error
        )));
    }
    transactions.push(rpc.send_v0_inline_with_signers(
        "infrastructure-succession",
        &[report.instruction],
        &payer,
        &[authority],
        report.observation,
    )?);
    let landed = rpc
        .account(report.profile)?
        .ok_or_else(|| Error::new("succession ceremony landed but V2 is absent"))?;
    if landed.owner != core || landed.data != report.record.to_bytes() {
        return Err(Error::new(
            "landed V2 profile differs from the shipped builder's exact projection",
        ));
    }
    let predecessor_after = rpc
        .account(v1_address)?
        .ok_or_else(|| Error::new("succession ceremony removed the V1 profile"))?;
    if predecessor_after.owner != core || predecessor_after.data != predecessor_bytes {
        return Err(Error::new(
            "succession ceremony changed the write-once V1 predecessor profile",
        ));
    }
    let poststate = succession_state(rpc, plan)?;
    if poststate != StageStateV1::Complete {
        return Err(Error::new(format!(
            "succession executed but its own detector reads {}{}",
            poststate.label(),
            poststate
                .detail()
                .map(|detail| format!(": {detail}"))
                .unwrap_or_default()
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_stages(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    authority: &Keypair,
    forge: &KeyForge,
    founding_actors: Option<crate::market::FoundingActorsV1>,
    market: Option<&crate::model::MarketRunInput>,
    founding_keys: Option<(Pubkey, Pubkey)>,
    states: &[(StageV1, StageStateV1)],
    through: StageV1,
    compatible_checkpoint: Option<&crate::market::MarketExecutionCheckpointV1>,
    checkpoint: &mut dyn FnMut(&crate::market::MarketExecutionCheckpointV1) -> Result<()>,
    mut submission_recorder: Option<&mut crate::market::FoundingSubmissionRecorderV1<'_>>,
) -> Result<CampaignExecutionEvidenceV1> {
    for (stage, state) in states {
        if let StageStateV1::Conflict(detail) = state {
            return Err(Error::new(format!(
                "stage {} is in conflict and a resumed run must never write over it: {detail}",
                stage.name()
            )));
        }
    }
    let substrate = states
        .iter()
        .find(|(stage, _)| *stage == StageV1::Substrate)
        .map(|(_, state)| state);
    if substrate != Some(&StageStateV1::Complete) {
        return Err(Error::new(
            "the substrate stage is not complete, and this driver never deploys. Deploy the seven \
             roles (the ladder is printed in the report above), re-run `prepare` with each role's \
             observed ProgramData, and run this again.",
        ));
    }
    let mut transactions = Vec::new();
    let mut market_evidence = None;
    let recovered_finalized_founding = false;
    for (stage, state) in states {
        if *stage > through {
            break;
        }
        if *state == StageStateV1::Complete {
            eprintln!("campaign stage {}: already complete, skipped", stage.name());
            if *stage == StageV1::Founding {
                let actors = founding_actors.ok_or_else(|| {
                    Error::new("completed founding recovery omitted its two public actors")
                })?;
                let input = market.ok_or_else(|| {
                    Error::new("completed founding recovery requires the exact Market input")
                })?;
                let saved = compatible_checkpoint.ok_or_else(|| {
                    Error::new(
                        "the chain proves this founding complete, but no compatible durable DCLTPCB2 checkpoint can reconstruct caller-consumable Market evidence",
                    )
                })?;
                market_evidence = Some(crate::market::recover_completed_market_from_checkpoint(
                    rpc,
                    plan,
                    input,
                    authority,
                    forge,
                    actors,
                    &mut transactions,
                    saved,
                    submission_recorder.as_deref_mut(),
                )?);
                eprintln!(
                    "campaign stage founding: reconstructed exact Open poststate from durable DCLTPCB2 checkpoint"
                );
            }
            continue;
        }
        match stage {
            StageV1::Substrate => {}
            StageV1::Publication => {
                let count = runtime::publish_infrastructure_records(
                    rpc,
                    plan,
                    authority,
                    &mut transactions,
                )?;
                eprintln!("campaign stage publication: {count} record bodies finalized");
            }
            StageV1::Initialize => {
                transactions.push(rpc.send(
                    "initialize Core infrastructure profile",
                    &[runtime::initialize_instruction(
                        plan,
                        authority.pubkey(),
                        authority.pubkey(),
                    )?],
                    authority,
                )?);
                runtime::verify_profile(rpc, plan)?;
            }
            StageV1::Succession => {
                execute_succession_stage_v1(rpc, plan, authority, forge, state, &mut transactions)?;
            }
            StageV1::Activation => {
                for (label, instruction) in
                    runtime::pending_activation_instructions(rpc, plan, authority.pubkey())?
                {
                    transactions.push(rpc.send(label, &[instruction], authority)?);
                }
                runtime::verify_activation(rpc, plan)?;
            }
            StageV1::Founding => {
                let actors = founding_actors.ok_or_else(|| {
                    Error::new("founding execution omitted its two public actors")
                })?;
                let Some(input) = market else {
                    return Err(Error::new(
                        "the founding stage needs a market input: pass --market ABSOLUTE_JSON \
                         carrying the exact output of `devnet-market` or `graduation-market`. \
                         Every earlier stage runs without one.",
                    ));
                };
                let (mint, wallet) = founding_keys.ok_or_else(|| {
                    Error::new("the founding stage reached execution without peeked keys")
                })?;
                let evidence = match state {
                    StageStateV1::Partial(detail) => {
                        let saved = compatible_checkpoint.ok_or_else(|| {
                            Error::new(format!(
                                "this founding has STARTED on this chain ({detail}), but no compatible durable DCLTPCB2 checkpoint authenticates a safe suffix resume"
                            ))
                        })?;
                        match founding_checkpoint_resume_v1(saved)? {
                            FoundingCheckpointResumeV1::PreparedControllerFunding => {
                                crate::market::resume_found_market_from_prepared_checkpoint(
                                    rpc,
                                    plan,
                                    input,
                                    authority,
                                    forge,
                                    actors,
                                    &mut transactions,
                                    saved,
                                    checkpoint,
                                    submission_recorder.as_deref_mut(),
                                )?
                            }
                            FoundingCheckpointResumeV1::CustodyStaged => {
                                crate::market::resume_found_market_from_checkpoint(
                                    rpc,
                                    plan,
                                    input,
                                    authority,
                                    forge,
                                    actors,
                                    &mut transactions,
                                    saved,
                                    submission_recorder.as_deref_mut(),
                                )?
                            }
                        }
                    }
                    StageStateV1::Absent if compatible_checkpoint.is_some() => {
                        return Err(Error::new(
                            "a compatible DCLTPCB2 checkpoint exists but the chain detector reads founding Absent; the journal and live chain disagree",
                        ));
                    }
                    StageStateV1::Absent => {
                        crate::market::execute_found_market_with_checkpoint_and_journal(
                            rpc,
                            plan,
                            input,
                            authority,
                            forge,
                            actors,
                            &mut transactions,
                            checkpoint,
                            submission_recorder.as_deref_mut(),
                        )?
                    }
                    StageStateV1::Complete | StageStateV1::Conflict(_) => {
                        return Err(Error::new(
                            "founding stage changed state after the campaign preflight",
                        ));
                    }
                };
                // Detector == verifier: the same read that would have skipped
                // this stage must pass now that it executed — against the SAME
                // peeked mint, never a fresh draw (the executor advanced the
                // forge's counter; a fresh peek would name the next founding's
                // mint and report this one absent).
                let (poststate, targets) = founding_state(rpc, plan, input, mint, wallet)?;
                if poststate != StageStateV1::Complete {
                    return Err(Error::new(format!(
                        "the founding executed but its own detector does not read Complete \
                         ({}): {}",
                        poststate.label(),
                        poststate.detail().unwrap_or("no detail")
                    )));
                }
                eprintln!(
                    "campaign stage founding: Open Market {} ({} steps)",
                    targets.open_market,
                    evidence.completed.len()
                );
                market_evidence = Some(evidence);
            }
        }
    }
    eprintln!(
        "campaign: {} transactions submitted this run",
        transactions.len()
    );
    Ok(CampaignExecutionEvidenceV1 {
        transactions,
        market: market_evidence,
        recovered_finalized_founding,
    })
}

/// The acknowledgment text the usage line prints.
pub(crate) fn acknowledgment_help() -> String {
    format!(
        "{DEVNET_ACKNOWLEDGMENT_FLAG} <GENESIS_HASH> names the cluster by identity rather than \
         by a boolean, so a command line copied to another cluster stops being true. Mainnet is \
         refused unconditionally and no flag admits it."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use dclutch_core_contract::ContentId;
    use dclutch_registry_contract::{
        ArtifactActivationInputV1, DeploymentObservationV1, ExecutionReleaseActivationInputsV1,
        activate_execution_release_set_v1,
    };
    use dclutch_release_set_contract::{ExecutionReleaseSetV1, ProgramIdentityV1};

    fn activation_test_content(seed: u8) -> ContentId {
        ContentId::new([seed; 32]).expect("nonzero content identity")
    }

    fn activation_test_program(seed: u8) -> ProgramIdentityV1 {
        ProgramIdentityV1::new([seed; 32]).expect("nonzero program identity")
    }

    fn activation_test_artifact(seed: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new([seed; 32]).expect("nonzero artifact identity")
    }

    fn activation_test_release(seed: u8) -> ArtifactReleaseV1 {
        ArtifactReleaseV1::new(
            activation_test_program(seed),
            activation_test_program(200),
            [seed.wrapping_add(20); 32],
            activation_test_content(seed.wrapping_add(40)),
            [seed.wrapping_add(60); 32],
            u64::from(seed) * 100,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("valid immutable release")
    }

    fn activation_test_input(
        artifact: ArtifactReleaseIdV1,
        release: ArtifactReleaseV1,
    ) -> ArtifactActivationInputV1 {
        let loader = release.loader_program().to_bytes();
        let observation = DeploymentObservationV1::new(
            release.program().to_bytes(),
            loader,
            true,
            release.programdata(),
            loader,
            false,
            release.programdata(),
            loader,
            release.deployment_slot(),
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("valid immutable deployment observation");
        ArtifactActivationInputV1::new(artifact, release, observation)
    }

    fn activation_test_binding(
        artifact: ArtifactReleaseIdV1,
        release: ArtifactReleaseV1,
    ) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(release.program(), artifact)
    }

    fn current_activation_cache_carrier_fixture()
    -> (Pubkey, Pubkey, ActivatedExecutionReleaseSetV1, u8) {
        let artifacts = [
            activation_test_artifact(21),
            activation_test_artifact(22),
            activation_test_artifact(23),
            activation_test_artifact(24),
            activation_test_artifact(25),
        ];
        let releases = [
            activation_test_release(1),
            activation_test_release(2),
            activation_test_release(3),
            activation_test_release(4),
            activation_test_release(5),
        ];
        let release_set = ExecutionReleaseSetV1::new(
            activation_test_binding(artifacts[0], releases[0]),
            activation_test_binding(artifacts[1], releases[1]),
            activation_test_binding(artifacts[2], releases[2]),
            activation_test_binding(artifacts[3], releases[3]),
            activation_test_binding(artifacts[4], releases[4]),
        )
        .expect("valid execution release set");
        let inputs = ExecutionReleaseActivationInputsV1::new(
            activation_test_input(artifacts[0], releases[0]),
            activation_test_input(artifacts[1], releases[1]),
            activation_test_input(artifacts[2], releases[2]),
            activation_test_input(artifacts[3], releases[3]),
            activation_test_input(artifacts[4], releases[4]),
        );
        let expected =
            activate_execution_release_set_v1(activation_test_content(0x71), &release_set, &inputs)
                .expect("valid complete activation");
        let registry = Pubkey::new_from_array([0x61; 32]);
        let (address, bump) = Pubkey::find_program_address(
            &[
                ACTIVATION_PDA_DOMAIN_V1,
                expected.execution_release_set_id().as_bytes(),
            ],
            &registry,
        );
        assert_ne!(bump, 0, "current Registry route cannot persist a zero bump");
        (registry, address, expected, bump)
    }

    #[test]
    fn current_activation_cache_accepts_the_exact_registry_written_carrier() {
        let (registry, address, expected, bump) = current_activation_cache_carrier_fixture();
        let mut observed = expected.to_bytes();
        observed[ACTIVATION_CACHE_BUMP_OFFSET_V1] = bump;
        authenticate_current_activation_cache_body_v1(registry, address, &observed, expected)
            .expect("exact Registry-written carrier and canonical projection");
    }

    #[test]
    fn current_activation_cache_refuses_wrong_or_zero_bumps_and_addresses() {
        let (registry, address, expected, bump) = current_activation_cache_carrier_fixture();
        let wrong_bump = if bump == 1 { 2 } else { 1 };
        for (label, hostile_address, hostile_bump) in [
            ("zero bump", address, 0),
            ("wrong bump", address, wrong_bump),
            ("wrong address", Pubkey::new_unique(), bump),
        ] {
            let mut observed = expected.to_bytes();
            observed[ACTIVATION_CACHE_BUMP_OFFSET_V1] = hostile_bump;
            let refusal = authenticate_current_activation_cache_body_v1(
                registry,
                hostile_address,
                &observed,
                expected,
            )
            .expect_err(label);
            assert!(refusal.0.contains("address-derived bump"), "{refusal:?}");
        }
    }

    #[test]
    fn current_activation_cache_refuses_adjacent_and_full_body_substitutions() {
        let (registry, address, expected, bump) = current_activation_cache_carrier_fixture();
        for offset in [
            ACTIVATION_CACHE_BUMP_OFFSET_V1 - 1,
            ACTIVATION_CACHE_BUMP_OFFSET_V1 + 1,
        ] {
            let mut observed = expected.to_bytes();
            observed[ACTIVATION_CACHE_BUMP_OFFSET_V1] = bump;
            observed[offset] ^= 0xff;
            let refusal = authenticate_current_activation_cache_body_v1(
                registry, address, &observed, expected,
            )
            .expect_err("adjacent authored byte substitution");
            assert!(refusal.0.contains("outside"), "{refusal:?}");
        }

        let mut substituted = vec![0xa5; expected.to_bytes().len()];
        substituted[ACTIVATION_CACHE_BUMP_OFFSET_V1] = bump;
        let refusal = authenticate_current_activation_cache_body_v1(
            registry,
            address,
            &substituted,
            expected,
        )
        .expect_err("full-body substitution");
        assert!(refusal.0.contains("outside"), "{refusal:?}");
    }

    fn rpc_account(
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: &[u8],
    ) -> crate::rpc::RpcAccount {
        crate::rpc::RpcAccount {
            lamports,
            owner,
            executable,
            rent_epoch: 0,
            data: data.to_vec(),
        }
    }

    #[test]
    fn profile_budget_prices_only_a_missing_exact_coordinate() {
        let core = Pubkey::new_unique();
        let body = [7_u8; 144];
        let minimum = 1_893_120;
        assert_eq!(
            remaining_profile_rent(None, core, &body, minimum).expect("missing profile"),
            minimum
        );
        let exact = rpc_account(core, minimum, false, &body);
        assert_eq!(
            remaining_profile_rent(Some(&exact), core, &body, minimum).expect("exact profile"),
            0,
            "an authenticated carried singleton must not be budgeted a second time"
        );
    }

    #[test]
    fn profile_budget_refuses_every_conflicting_existing_shape() {
        let core = Pubkey::new_unique();
        let body = [9_u8; 144];
        let minimum = 1_893_120;
        let hostiles = [
            rpc_account(Pubkey::new_unique(), minimum, false, &body),
            rpc_account(core, minimum, true, &body),
            rpc_account(core, minimum, false, &[8_u8; 144]),
            rpc_account(core, minimum - 1, false, &body),
        ];
        for hostile in &hostiles {
            let refusal = remaining_profile_rent(Some(hostile), core, &body, minimum)
                .expect_err("conflicting profile must refuse");
            assert!(refusal.0.contains("conflicts"), "{}", refusal.0);
        }
    }

    #[test]
    fn mutable_saved_plan_requires_checked_set_before_key_loading() {
        require_checked_mutable_binding(false, false, false, false).expect("legacy immutable plan");
        require_checked_mutable_binding(true, true, false, false)
            .expect("checked devnet mutable plan");
        require_checked_mutable_binding(true, false, true, false)
            .expect("checked local mutable plan");
        // A founded cohort binds to its own observations, and only when EVERY
        // role is one: a half-observed mutable plan is still unbound.
        require_checked_mutable_binding(true, false, false, true)
            .expect("founded devnet plan bound by observation");
        require_checked_mutable_binding(true, false, false, false)
            .expect_err("a mutable plan bound by nothing must still refuse");
        let refusal = require_checked_mutable_binding(true, false, false, false)
            .expect_err("unbound mutable plan must refuse");
        assert!(refusal.0.contains("not bound"), "{}", refusal.0);
        assert!(require_checked_mutable_binding(true, true, true, false).is_err());
        // Observation never rescues a plan that mixed both deployment sets.
        assert!(require_checked_mutable_binding(true, true, true, true).is_err());
    }

    #[test]
    fn checked_live_role_binds_program_link_full_programdata_slot_authority_and_payload() {
        let program_key = Pubkey::new_unique();
        let programdata_key =
            Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
        let authority = Pubkey::new_unique();
        let candidate = b"\x7fELFchecked-live";
        let mut live = candidate.to_vec();
        live.extend_from_slice(&[0; 7]);
        let programdata_bytes = crate::plan::loader_programdata_bytes(&live, 818, Some(authority));
        let mut program_bytes = vec![0; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(programdata_key.as_ref());
        let mut pin = pin();
        pin.program_id = program_key.to_string();
        pin.programdata_id = programdata_key.to_string();
        pin.checked_candidate_elf_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(candidate));
        pin.elf_sha256 = pin.checked_candidate_elf_sha256.clone();
        pin.live_elf_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(&live));
        pin.live_elf_padding_bytes = 7;
        pin.upgrade_authority = Some(authority.to_string());
        pin.deployment_slot = 818;
        pin.deployment_source = "observed-programdata-account".into();
        pin.programdata_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(&programdata_bytes));
        let program = rpc_account(bpf_loader_upgradeable::ID, 1, true, &program_bytes);
        let programdata = rpc_account(bpf_loader_upgradeable::ID, 1, false, &programdata_bytes);
        authenticate_live_checked_role("core", &pin, &program, &programdata, candidate)
            .expect("exact live role");

        let mut stale = pin.clone();
        stale.deployment_slot += 1;
        assert!(
            authenticate_live_checked_role("core", &stale, &program, &programdata, candidate)
                .is_err()
        );
        let mut substituted_authority = pin.clone();
        substituted_authority.upgrade_authority = Some(Pubkey::new_unique().to_string());
        assert!(
            authenticate_live_checked_role(
                "core",
                &substituted_authority,
                &program,
                &programdata,
                candidate,
            )
            .is_err()
        );
        let mut wrong_link_bytes = program_bytes;
        wrong_link_bytes[4..].copy_from_slice(Pubkey::new_unique().as_ref());
        let wrong_link = rpc_account(bpf_loader_upgradeable::ID, 1, true, &wrong_link_bytes);
        assert!(
            authenticate_live_checked_role("core", &pin, &wrong_link, &programdata, candidate)
                .is_err()
        );
        let mut tampered_plan_pin = pin;
        tampered_plan_pin.programdata_sha256 = "11".repeat(32);
        assert!(
            authenticate_live_checked_role(
                "core",
                &tampered_plan_pin,
                &program,
                &programdata,
                candidate,
            )
            .is_err(),
            "saved-plan ProgramData digest substitution must refuse"
        );
    }

    fn test_direct_compiler(registry: Pubkey) -> crate::direct_market::DirectMarketCompilerOwnedV1 {
        crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("deployment widths"),
        )
    }

    fn graduation_market_value() -> Value {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let relayer = Pubkey::new_from_array([0x42; 32]);
        let direct = test_direct_compiler(registry);
        let venue = crate::relayed::RelayedVenueFactsV1 {
            program: [0x51; 32],
            programdata: [0x52; 32],
            pool: [0x53; 32],
            elf_digest: [0x54; 32],
            deployment_slot: 99,
            upgrade_authority: [0x55; 32],
        };
        let window = crate::relayed::WindowChoiceV1 {
            start_unix_seconds: 1_800_000_000,
            end_unix_seconds: 1_800_003_600,
            max_age_seconds: 900,
        };
        let facts = crate::relayed::relayed_market_input(
            registry,
            relayer.to_bytes(),
            &window,
            &venue,
            direct.compiler(),
        )
        .expect("graduation producer");
        json!({
            "schema": GRADUATION_MARKET_INPUT_SCHEMA_V1,
            "market": facts.input,
            "account_set_id": hex(&facts.account_set_id),
            "relayer_attestation": relayer.to_string(),
            "relayer_key_set_hex": hex(&facts.relayer_key_set_bytes),
            "relayer_key_set_digest": hex(&facts.relayer_key_set_digest),
            "venue_release_digest": hex(&facts.venue_release_digest),
            "relayed_adapter_config_digest": hex(&facts.relayed_adapter_config_digest),
            "source_spec_digest": hex(&facts.source_spec_digest),
            "window": {
                "start_unix_seconds": window.start_unix_seconds,
                "end_unix_seconds": window.end_unix_seconds,
                "max_age_seconds": window.max_age_seconds,
            },
            "walk_bounty_lamports": crate::relayed::WALK_BOUNTY_LAMPORTS,
            "admitted_principal_atoms": facts.admitted_principal_atoms.to_string(),
            "admitted_principal_cap_atoms": facts.admitted_principal_cap_atoms.to_string(),
            "disclosed_failure_conflation": crate::relayed::DISCLOSED_FAILURE_CONFLATION,
        })
    }

    #[test]
    fn market_loader_accepts_bare_and_authenticated_graduation_inputs() {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = test_direct_compiler(registry);
        let bare = crate::market::demo_market_input(registry, direct.compiler())
            .expect("bare devnet market");
        assert_eq!(
            load_market_input(&serde_json::to_vec(&bare).expect("bare JSON"))
                .expect("bare input")
                .product_id,
            bare.product_id
        );

        let wrapped = graduation_market_value();
        let loaded = load_market_input(&serde_json::to_vec(&wrapped).expect("wrapper JSON"))
            .expect("authenticated graduation input");
        assert_eq!(loaded.product_id, wrapped["market"]["product_id"]);
    }

    #[test]
    fn participant_fixture_liquidity_is_loopback_only_and_requires_exact_roles() {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = test_direct_compiler(registry);
        let mut market =
            crate::market::demo_market_input(registry, direct.compiler()).expect("local market");
        assert_eq!(
            market.local_participant_fixture_liquidity_atoms,
            crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
        );
        let loopback = ClusterOriginV1::Loopback {
            url: "http://127.0.0.1:18899/".into(),
            port: 18_899,
        };
        let devnet = ClusterOriginV1::AcknowledgedDevnet {
            url: "https://api.devnet.solana.com".into(),
        };
        let paths = BTreeMap::from([
            (
                crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1.into(),
                PathBuf::from("/tmp/participant.json"),
            ),
            (
                crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1.into(),
                PathBuf::from("/tmp/direct-buyer.json"),
            ),
        ]);
        authenticate_local_participant_fixture_policy_v1(&loopback, Some(&market), &paths)
            .expect("exact local fixture");
        assert!(
            authenticate_local_participant_fixture_policy_v1(&devnet, Some(&market), &paths)
                .is_err(),
            "public devnet must refuse local fixture supply"
        );
        let missing = BTreeMap::from([(
            crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1.into(),
            PathBuf::from("/tmp/participant.json"),
        )]);
        assert!(
            authenticate_local_participant_fixture_policy_v1(&loopback, Some(&market), &missing)
                .is_err(),
            "both exact local-prepare roles are required"
        );
        market.local_participant_fixture_liquidity_atoms = 99_999_999;
        assert!(crate::market::validate_market_input(&market).is_err());
        assert!(
            authenticate_local_participant_fixture_policy_v1(&loopback, Some(&market), &paths)
                .is_err(),
            "caller-chosen fixture quantities are not a hidden multiplier knob"
        );
    }

    #[test]
    fn participant_fixture_receipt_binds_supply_accounts_and_finalized_transaction() {
        let source = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let signature = Keypair::new().sign_message(b"local fixture").to_string();
        let receipt = crate::market::LocalParticipantFixtureLiquidityEvidenceV1 {
            source_token_account: source.to_string(),
            source_owner: owner.to_string(),
            quantity_atoms: crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1,
            founding_collateral_atoms: 1_000_000_000,
            total_supply_atoms: 1_100_000_000,
            mint: mint.to_string(),
            mint_authority_removed: true,
            transaction_signature: signature.clone(),
            finalized_slot: 77,
            compute_units_consumed: 88_000,
        };
        let transaction = TerminalTransactionEvidenceV1 {
            label: "create local fixture".into(),
            signature,
            slot: 77,
            transaction_metadata_available: true,
            fee_lamports: NullableV1(Some(5_000)),
            fee_only_balance_change: NullableV1(Some(false)),
            compute_units_consumed: NullableV1(Some(88_000)),
            error: Value::Null,
            logs: Vec::new(),
        };
        let account = |address: Pubkey| CampaignAccountEvidenceV1 {
            address: address.to_string(),
            owner: Pubkey::new_unique().to_string(),
            lamports: 1,
            executable: false,
            data_len: 1,
            data_sha256: "11".repeat(32),
            account_sha256: "22".repeat(32),
        };
        let market = TerminalMarketEvidenceV1 {
            completed: vec!["founding".into()],
            accounts: BTreeMap::from([
                ("local_participant_fixture_source".into(), account(source)),
                ("collateral_mint".into(), account(mint)),
            ]),
            founding_custody_context: "33".repeat(32),
            direct_selected_manifest_entry_index: 1,
        };
        authenticate_local_participant_fixture_evidence_v1(
            crate::cluster::ExpectedClusterV1::OwnedLoopback,
            Some(&receipt),
            &[transaction],
            &market,
        )
        .expect("exact fixture receipt");
        assert!(
            authenticate_local_participant_fixture_evidence_v1(
                crate::cluster::ExpectedClusterV1::Devnet,
                Some(&receipt),
                &[],
                &market,
            )
            .is_err(),
            "public devnet must reject even a structurally exact local receipt"
        );
        let mut retained = receipt.clone();
        retained.mint_authority_removed = false;
        assert!(
            authenticate_local_participant_fixture_evidence_v1(
                crate::cluster::ExpectedClusterV1::OwnedLoopback,
                Some(&retained),
                &[],
                &market,
            )
            .is_err(),
            "a receipt cannot bless retained mint authority"
        );

        // A MARKET FOUNDED AT ANOTHER STAKE IS STILL THIS MARKET. The founding
        // collateral used to be pinned to the literal the compiler hard-coded,
        // which made the stake knob foundable but not consumable: the market
        // opened and then every driver reading its campaign report refused it
        // here. What still binds is the arithmetic -- total supply is the
        // founding collateral plus the fixture liquidity and nothing else.
        let mut widened = receipt.clone();
        widened.founding_collateral_atoms = 5_073_807_456;
        widened.total_supply_atoms =
            5_073_807_456 + crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1;
        authenticate_local_participant_fixture_evidence_v1(
            crate::cluster::ExpectedClusterV1::OwnedLoopback,
            Some(&widened),
            &[transaction_for(&receipt)],
            &market,
        )
        .expect("a market founded at another stake is still authenticated");

        let mut lying = widened.clone();
        lying.total_supply_atoms += 1;
        assert!(
            authenticate_local_participant_fixture_evidence_v1(
                crate::cluster::ExpectedClusterV1::OwnedLoopback,
                Some(&lying),
                &[],
                &market,
            )
            .is_err(),
            "a receipt cannot claim a supply its own arithmetic does not carry"
        );

        let mut stakeless = widened;
        stakeless.founding_collateral_atoms = 0;
        stakeless.total_supply_atoms = crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1;
        assert!(
            authenticate_local_participant_fixture_evidence_v1(
                crate::cluster::ExpectedClusterV1::OwnedLoopback,
                Some(&stakeless),
                &[],
                &market,
            )
            .is_err(),
            "a market with no stake at all is still refused"
        );
    }

    /// The transaction evidence the fixture receipt binds, rebuilt for a second
    /// authentication in the same test.
    fn transaction_for(
        receipt: &crate::market::LocalParticipantFixtureLiquidityEvidenceV1,
    ) -> TerminalTransactionEvidenceV1 {
        TerminalTransactionEvidenceV1 {
            label: "create local fixture".into(),
            signature: receipt.transaction_signature.clone(),
            slot: receipt.finalized_slot,
            transaction_metadata_available: true,
            fee_lamports: NullableV1(Some(5_000)),
            fee_only_balance_change: NullableV1(Some(false)),
            compute_units_consumed: NullableV1(Some(receipt.compute_units_consumed)),
            error: Value::Null,
            logs: Vec::new(),
        }
    }

    #[test]
    fn graduation_loader_refuses_unknown_schema_and_unknown_fields() {
        let exact = graduation_market_value();

        let mut wrong_schema = exact.clone();
        wrong_schema["schema"] = json!("dclutch-graduation-market-input-v2");
        let refusal = load_market_input(&serde_json::to_vec(&wrong_schema).expect("JSON"))
            .err()
            .expect("unknown schema refuses");
        assert!(
            refusal.0.contains("unsupported graduation"),
            "{}",
            refusal.0
        );

        let mut wrapper_unknown = exact.clone();
        wrapper_unknown["shadow_market"] = wrapper_unknown["market"].clone();
        assert!(load_market_input(&serde_json::to_vec(&wrapper_unknown).expect("JSON")).is_err());

        let mut bare_unknown = exact["market"].clone();
        bare_unknown["schema_shadow"] = json!(GRADUATION_MARKET_INPUT_SCHEMA_V1);
        assert!(load_market_input(&serde_json::to_vec(&bare_unknown).expect("JSON")).is_err());
    }

    #[test]
    fn graduation_loader_refuses_digest_window_and_inner_market_substitution() {
        let exact = graduation_market_value();

        let mut digest = exact.clone();
        digest["relayer_key_set_digest"] = json!("11".repeat(32));
        assert!(load_market_input(&serde_json::to_vec(&digest).expect("JSON")).is_err());

        let mut window = exact.clone();
        window["window"]["start_unix_seconds"] = json!(1_799_999_999_i64);
        assert!(load_market_input(&serde_json::to_vec(&window).expect("JSON")).is_err());

        let mut market = exact;
        market["market"]["product_id"] = json!("22".repeat(32));
        assert!(load_market_input(&serde_json::to_vec(&market).expect("JSON")).is_err());
    }

    #[test]
    fn graduation_loader_refuses_unbounded_and_substituted_kappa_graphs() {
        let exact = graduation_market_value();

        // Empty is a meaningful bare-input shape: the market compiler turns
        // it into SourceMaterialV3::explicitly_unbounded. The graduation
        // envelope must not retain its bounded disclosure around that graph.
        let mut unbounded = exact.clone();
        unbounded["market"]["manipulation_floor_hex"] = json!("");
        let refusal = load_market_input(&serde_json::to_vec(&unbounded).expect("JSON"))
            .expect_err("unbounded graduation graph must refuse");
        assert!(
            refusal.0.contains("bounded manipulation floor"),
            "{refusal:?}"
        );

        // The capacity body is content-addressed by SourceSpec. A same-width
        // body with its kappa tail erased remains decodable but is not the
        // selected provisional 1/4 record.
        let mut unstated = exact.clone();
        let mut capacity = runtime::decode_hex(
            unstated["market"]["source_capacity_profile_hex"]
                .as_str()
                .expect("capacity hex"),
        )
        .expect("capacity bytes");
        capacity[dclutch_source_contract::SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1
            ..dclutch_source_contract::SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1 + 4]
            .fill(0);
        capacity[dclutch_source_contract::SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1
            ..dclutch_source_contract::SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1 + 4]
            .fill(0);
        unstated["market"]["source_capacity_profile_hex"] = json!(hex(&capacity));
        let refusal = load_market_input(&serde_json::to_vec(&unstated).expect("JSON"))
            .expect_err("unstated graduation kappa must refuse");
        assert!(refusal.0.contains("capacity"), "{refusal:?}");

        // A canonical floor for another Source is still a valid record. Its
        // exact identity binding, not decode, is what must refuse it.
        let floor_bytes = runtime::decode_hex(
            exact["market"]["manipulation_floor_hex"]
                .as_str()
                .expect("floor hex"),
        )
        .expect("floor bytes");
        let floor = dclutch_source_contract::ManipulationFloorV1::decode(&floor_bytes)
            .expect("canonical floor");
        let substituted_floor = dclutch_source_contract::ManipulationFloorV1::new(
            floor.basis(),
            dclutch_source_contract::ContentId::new([0x91; 32]).expect("hostile source"),
            floor.adapter_config_id(),
            floor.collateral_unit_id(),
            floor.derivation_release_id(),
            floor.floor_atoms(),
        );
        let mut substituted = exact;
        substituted["market"]["manipulation_floor_hex"] = json!(hex(&substituted_floor.to_bytes()));
        let refusal = load_market_input(&serde_json::to_vec(&substituted).expect("JSON"))
            .expect_err("substituted floor binding must refuse");
        assert!(refusal.0.contains("changed its source"), "{refusal:?}");
    }

    fn duplicate_field_before_original(json: &str, field: &str, value: &str) -> String {
        let original = format!("\"{field}\":");
        assert!(json.contains(&original), "fixture omitted {field}");
        json.replacen(&original, &format!("\"{field}\":{value},{original}"), 1)
    }

    fn assert_duplicate_refused(json: &str, field: &str) {
        let refusal = load_market_input(json.as_bytes())
            .err()
            .expect("duplicate object key must refuse");
        assert!(
            refusal.0.contains("duplicate JSON object key") && refusal.0.contains(field),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn market_loader_recursively_refuses_duplicate_object_keys_before_normalization() {
        let wrapped = serde_json::to_string(&graduation_market_value()).expect("wrapper JSON");
        assert_duplicate_refused(
            &duplicate_field_before_original(&wrapped, "schema", "\"shadow-schema\""),
            "schema",
        );
        assert_duplicate_refused(
            &duplicate_field_before_original(&wrapped, "start_unix_seconds", "0"),
            "start_unix_seconds",
        );
        assert_duplicate_refused(
            &duplicate_field_before_original(&wrapped, "generation", "0"),
            "generation",
        );

        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = test_direct_compiler(registry);
        let bare = crate::market::demo_market_input(registry, direct.compiler())
            .expect("bare devnet market");
        let bare = serde_json::to_string(&bare).expect("bare JSON");
        assert_duplicate_refused(
            &duplicate_field_before_original(&bare, "generation", "0"),
            "generation",
        );
    }

    fn checkpoint_value() -> Value {
        json!({
            "schema": "dclutch-market-dcltpcb2-checkpoint-v1",
            "market": Pubkey::new_unique().to_string(),
            "foundingCustodyContext": "33".repeat(32),
            "directSelectedManifestEntryIndex": 2,
            "direct_capability_root": Pubkey::new_unique().to_string(),
            "direct_trading_funding_ledger": Pubkey::new_unique().to_string(),
            "expiry_slot": 91,
            "found_record": Pubkey::new_unique().to_string(),
            "lock_record": Pubkey::new_unique().to_string(),
            "accounts": {},
            "completed": ["DCLTPCB2 finalized"],
        })
    }

    #[test]
    fn partial_founding_routes_only_the_two_durable_checkpoint_schemas() {
        let custody: crate::market::MarketExecutionCheckpointV1 =
            serde_json::from_value(checkpoint_value()).expect("custody checkpoint");
        assert_eq!(
            founding_checkpoint_resume_v1(&custody).expect("custody route"),
            FoundingCheckpointResumeV1::CustodyStaged
        );

        let mut prepared = custody.clone();
        prepared.schema = crate::market::DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1.into();
        assert_eq!(
            founding_checkpoint_resume_v1(&prepared).expect("Prepared route"),
            FoundingCheckpointResumeV1::PreparedControllerFunding
        );

        prepared.schema = "dclutch-market-shadow-checkpoint-v1".into();
        let refusal = founding_checkpoint_resume_v1(&prepared)
            .expect_err("unknown checkpoint schema must refuse");
        assert!(refusal.0.contains("unsupported schema"), "{refusal:?}");
    }

    #[test]
    fn observed_finalization_preserves_adjacent_fsync_order_without_a_send_route() {
        use crate::market::founding_submission_journal::FoundingSubmissionPhaseV1;

        assert_eq!(
            observed_finalization_persistence_v1(FoundingSubmissionPhaseV1::Dispatching)
                .expect("Dispatching recovery"),
            ObservedFinalizationPersistenceV1::SubmittedThenFinalizedThenCheckpoint
        );
        assert_eq!(
            observed_finalization_persistence_v1(FoundingSubmissionPhaseV1::Submitted)
                .expect("Submitted recovery"),
            ObservedFinalizationPersistenceV1::FinalizedThenCheckpoint
        );
        for phase in [
            FoundingSubmissionPhaseV1::Planned,
            FoundingSubmissionPhaseV1::Prepared,
            FoundingSubmissionPhaseV1::Finalized,
        ] {
            assert!(
                observed_finalization_persistence_v1(phase).is_err(),
                "{phase:?} must not acquire an observed-finalization path"
            );
        }
    }

    fn terminal_consumable_report(path: &Path, checkpoint: Value) -> Value {
        json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "cluster": "devnet",
            "genesis_hash": DEVNET_GENESIS_HASH,
            "rpc_url": "https://api.devnet.solana.com/",
            "mode": "execute",
            "plan_sha256": "11".repeat(32),
            "market_sha256": "22".repeat(32),
            "evidence_output": path.display().to_string(),
            "foundingCheckpoint": checkpoint,
            "execution": {
                "completed": true,
                "recoveredFinalizedFounding": false,
                "transactions": [],
                "market": {
                    "completed": ["DCLTGMF3 finalized"],
                    "accounts": {
                        "founding_market": {
                            "address": Pubkey::new_unique().to_string(),
                            "owner": Pubkey::new_unique().to_string(),
                            "lamports": 1,
                            "executable": false,
                            "data_len": 1,
                            "data_sha256": "44".repeat(32),
                            "account_sha256": "55".repeat(32),
                        },
                    },
                    "founding_custody_context": "33".repeat(32),
                    "direct_selected_manifest_entry_index": 2,
                },
            },
        })
    }

    #[test]
    fn terminal_campaign_evidence_authenticates_exact_market_digest() {
        let path = std::env::temp_dir().join(format!(
            "dclutch-campaign-terminal-market-digest-{}.json",
            Pubkey::new_unique()
        ));
        let report = terminal_consumable_report(&path, checkpoint_value());
        let encoded = serde_json::to_vec(&report).expect("terminal report JSON");
        let evidence = parse_campaign_terminal_evidence_v1(&encoded)
            .expect("exact Market digest is terminal-consumable");
        assert_eq!(evidence.market_sha256, "22".repeat(32));

        let mut wrong_genesis = report.clone();
        wrong_genesis["genesis_hash"] = json!(Hash::new_unique().to_string());
        let refusal = parse_campaign_terminal_evidence_v1(
            &serde_json::to_vec(&wrong_genesis).expect("wrong genesis JSON"),
        )
        .expect_err("terminal devnet evidence must bind the exact devnet genesis");
        assert!(refusal.0.contains("genesis_hash"), "{}", refusal.0);

        let mut missing = report.clone();
        missing
            .as_object_mut()
            .expect("report object")
            .remove("market_sha256");
        let refusal = parse_campaign_terminal_evidence_v1(
            &serde_json::to_vec(&missing).expect("missing digest JSON"),
        )
        .expect_err("missing Market digest must refuse");
        assert!(refusal.0.contains("omitted market_sha256"), "{}", refusal.0);

        let mut malformed = report;
        malformed["market_sha256"] = json!("22".repeat(31));
        let refusal = parse_campaign_terminal_evidence_v1(
            &serde_json::to_vec(&malformed).expect("malformed digest JSON"),
        )
        .expect_err("malformed Market digest must refuse");
        assert!(
            refusal.0.contains("expected 64 lowercase hex characters"),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn campaign_evidence_lease_refuses_a_racer_and_releases_only_its_owned_link() {
        let evidence = std::env::temp_dir().join(format!(
            "dclutch-campaign-evidence-lease-{}.json",
            Pubkey::new_unique()
        ));
        let first = CampaignEvidenceLeaseV1::acquire(&evidence).expect("first lease");
        let lock_path = first.path.clone();
        assert!(lock_path.exists(), "durable lock must exist while owned");
        let refusal = CampaignEvidenceLeaseV1::acquire(&evidence)
            .err()
            .expect("simultaneous campaign must refuse");
        assert!(refusal.0.contains("locked"), "{}", refusal.0);
        assert!(
            refusal.0.contains("never removed automatically"),
            "{}",
            refusal.0
        );
        drop(first);
        assert!(
            !lock_path.exists(),
            "owner must release its exact lock inode"
        );

        let next = CampaignEvidenceLeaseV1::acquire(&evidence).expect("released lease reacquires");
        drop(next);
        assert!(!lock_path.exists());
    }

    #[test]
    fn crash_restart_preserves_terminal_completion_and_mismatch_never_clobbers() {
        let path = std::env::temp_dir().join(format!(
            "dclutch-campaign-crash-restart-{}.json",
            Pubkey::new_unique()
        ));
        let mut first_runner = json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "cluster": "devnet",
            "rpc_url": "https://api.devnet.solana.com/",
            "plan_sha256": "11".repeat(32),
            "market_sha256": "22".repeat(32),
            "evidence_output": path.display().to_string(),
            "intent": { "execute": true, "through": "founding" },
        });
        write_evidence_atomically(&path, &first_runner).expect("durable prewrite");
        first_runner["foundingCheckpoint"] = checkpoint_value();
        write_evidence_atomically(&path, &first_runner).expect("pre-mutation checkpoint");
        drop(first_runner); // fake process death after mutation, before final evidence update

        let prior = load_prior_campaign_evidence(
            &path,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
            "devnet",
            "https://api.devnet.solana.com/",
        )
        .expect("restart reads durable evidence");
        assert!(prior.checkpoint.is_some());
        assert!(prior.terminal_consumable_source.is_none());

        let restarted_runner = terminal_consumable_report(
            &path,
            serde_json::to_value(prior.checkpoint).expect("checkpoint JSON"),
        );
        write_evidence_atomically(&path, &restarted_runner).expect("atomic final evidence");
        let finalized = fs::read(&path).expect("read final evidence");
        let completed = load_prior_campaign_evidence(
            &path,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
            "devnet",
            "https://api.devnet.solana.com/",
        )
        .expect("authenticate completed evidence");
        assert_eq!(
            completed.terminal_consumable_source.as_deref(),
            Some(finalized.as_slice())
        );
        assert_eq!(
            fs::read(&path).expect("preserved complete report"),
            finalized
        );

        let refusal = load_prior_campaign_evidence(
            &path,
            &"44".repeat(32),
            Some(&"22".repeat(32)),
            "devnet",
            "https://api.devnet.solana.com/",
        )
        .err()
        .expect("mismatched dossier refuses");
        assert!(refusal.0.contains("was not replaced"), "{}", refusal.0);
        assert_eq!(
            fs::read(&path).expect("mismatch did not clobber"),
            finalized
        );
        fs::remove_file(path).expect("remove isolated test evidence");
    }

    fn observed_role() -> ObservedRoleV1 {
        ObservedRoleV1 {
            role: "Trading".into(),
            program_id: Pubkey::new_unique().to_string(),
            programdata_id: Pubkey::new_unique().to_string(),
            observed_slot: Some(700),
            pinned_slot: 700,
            observed_authority: Some(Pubkey::new_from_array([9; 32]).to_string()),
            pinned_authority: Some(Pubkey::new_from_array([9; 32]).to_string()),
            observed_owner: Some(bpf_loader_upgradeable::ID.to_string()),
            observed_executable: Some(false),
            observed_live_elf_sha256: Some("ab".repeat(32)),
            pinned_live_elf_sha256: "ab".repeat(32),
            checked_candidate_elf_sha256: "cd".repeat(32),
            live_elf_padding_bytes: 17,
            observed_data_len: Some(45),
        }
    }

    #[test]
    fn substrate_pin_requires_slot_authority_loader_owner_and_data_shape() {
        let exact = observed_role();
        assert!(exact.pin_conflicts().is_empty());

        let mut stale_slot = exact.clone();
        stale_slot.observed_slot = Some(701);
        assert!(
            stale_slot
                .pin_conflicts()
                .iter()
                .any(|detail| detail.contains("observed slot"))
        );

        let mut changed_authority = exact.clone();
        changed_authority.observed_authority = Some(Pubkey::new_unique().to_string());
        assert!(
            changed_authority
                .pin_conflicts()
                .iter()
                .any(|detail| detail.contains("upgrade authority"))
        );

        let mut wrong_owner = exact.clone();
        wrong_owner.observed_owner = Some(Pubkey::new_unique().to_string());
        assert!(
            wrong_owner
                .pin_conflicts()
                .iter()
                .any(|detail| detail.contains("ProgramData owner"))
        );

        let mut executable = exact;
        executable.observed_executable = Some(true);
        assert!(
            executable
                .pin_conflicts()
                .iter()
                .any(|detail| detail.contains("executable flag"))
        );

        let mut changed_live_payload = observed_role();
        changed_live_payload.observed_live_elf_sha256 = Some("ef".repeat(32));
        assert!(
            changed_live_payload
                .pin_conflicts()
                .iter()
                .any(|detail| detail.contains("complete live ELF SHA-256"))
        );
    }

    fn activation_compute_rows(widths: [u64; 5]) -> Vec<ObservedRoleV1> {
        ["core", "claims", "trading", "resolution", "custody"]
            .into_iter()
            .zip(widths)
            .map(|(role, width)| {
                let mut row = observed_role();
                row.role = role.into();
                row.observed_data_len = Some(
                    usize::try_from(width)
                        .expect("fixture ELF width")
                        .checked_add(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
                        .expect("fixture ProgramData width"),
                );
                row
            })
            .collect()
    }

    #[test]
    fn activation_compute_preflight_reports_headroom_and_refuses_source_as_resolution() {
        let canonical = activation_compute_rows([934_088, 1_010_496, 1_325_848, 588_336, 360_328]);
        let projection = activation_compute_preflight_v1(&canonical, 0)
            .expect("canonical five-role activation set has headroom");
        assert_eq!(projection.len(), 5);
        let trading = projection
            .iter()
            .find(|row| row.role == "trading")
            .expect("Trading projection");
        assert_eq!(trading.live_elf_bytes, 1_325_848);
        assert!(trading.pending);
        assert!(trading.headroom_compute_units > 500_000);

        let hostile_source_as_resolution =
            activation_compute_rows([934_088, 1_010_496, 1_325_848, 9_034_536, 360_328]);
        let error = activation_compute_preflight_v1(&hostile_source_as_resolution, 0)
            .expect_err("9 MB dclutch_sbf.so substitution cannot reach activation");
        assert!(error.to_string().contains("pending resolution activation"));
        assert!(error.to_string().contains("9034536 bytes"));
        assert!(
            error
                .to_string()
                .contains(&runtime::MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1.to_string())
        );

        let completed = activation_compute_preflight_v1(&hostile_source_as_resolution, 5)
            .expect("already complete cache never schedules another activation");
        assert!(completed.iter().all(|row| !row.pending));
    }

    #[test]
    fn the_stage_order_is_the_only_order_a_chain_accepts() {
        // Publication before Initialize is not a preference: Core's
        // infrastructure initialization READS the Registry and Rent artifact
        // records, and activation reads the five role records plus the release
        // set. The enum's Ord is what `execute_stages` uses to stop at
        // `--through`, so the declaration order is load-bearing.
        assert!(StageV1::Substrate < StageV1::Publication);
        assert!(StageV1::Publication < StageV1::Initialize);
        assert!(StageV1::Initialize < StageV1::Succession);
        assert!(StageV1::Succession < StageV1::Activation);
        assert!(StageV1::Activation < StageV1::Founding);
        assert_eq!(StageV1::ORDER.len(), 6);
        for (index, stage) in StageV1::ORDER.into_iter().enumerate() {
            assert_eq!(StageV1::parse(stage.name()).expect("round trip"), stage);
            assert_eq!(
                StageV1::ORDER.get(index).copied(),
                Some(stage),
                "ORDER must be sorted"
            );
        }
        assert!(StageV1::parse("revoke").is_err());
        let refusal = StageV1::parse("nonsense").err().expect("must refuse");
        assert!(refusal.0.contains("substrate"), "{}", refusal.0);
    }

    #[test]
    fn a_damaged_keypair_file_is_refused_before_it_is_funded() {
        let dir = std::env::temp_dir().join(format!(
            "dclutch-driver-keys-{}-{}",
            std::process::id(),
            "a"
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let good = Keypair::new();
        let mut bytes = good.to_bytes().to_vec();
        let path = dir.join("good.json");
        std::fs::write(&path, serde_json::to_vec(&bytes).expect("json")).expect("write");
        assert_eq!(
            Keypair::new_from_array(read_keypair_file(&path, "test").expect("good")).pubkey(),
            good.pubkey(),
            "the secret seed must expand to the file's own address"
        );

        // A file whose declared public key is not the one its secret expands
        // to. This is the case that would otherwise be discovered as a
        // signature failure on a funded address.
        if let Some(byte) = bytes.get_mut(63) {
            *byte ^= 0xff;
        }
        let tampered = dir.join("tampered.json");
        std::fs::write(&tampered, serde_json::to_vec(&bytes).expect("json")).expect("write");
        let refusal = read_keypair_file(&tampered, "test")
            .err()
            .expect("must refuse");
        assert!(refusal.0.contains("do not fund"), "{}", refusal.0);

        // Wrong width.
        let short = dir.join("short.json");
        std::fs::write(&short, b"[1,2,3]").expect("write");
        assert!(read_keypair_file(&short, "test").is_err());
        // Not absolute.
        assert!(read_keypair_file(Path::new("relative.json"), "test").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keypair_path_authentication_is_key_free_and_duplicate_safe() {
        let root = std::env::temp_dir().join(format!(
            "dclutch-absent-campaign-keys-{}",
            Pubkey::new_unique()
        ));
        let paths = FOUNDING_REQUIRED_ROLES
            .iter()
            .enumerate()
            .map(|(index, role)| (role.to_string(), root.join(format!("{index}.json"))))
            .collect::<BTreeMap<_, _>>();
        authenticate_keypair_paths(&paths, FOUNDING_REQUIRED_ROLES, FOUNDING_REQUIRED_ROLES)
            .expect("path-only authentication must not open absent key files");
        assert!(load_campaign_keypairs(&paths).is_err());

        let mut duplicate = paths;
        let first = duplicate
            .get(FOUNDING_REQUIRED_ROLES[0])
            .expect("first path")
            .clone();
        duplicate.insert(FOUNDING_REQUIRED_ROLES[1].into(), first);
        let refusal = authenticate_keypair_paths(
            &duplicate,
            FOUNDING_REQUIRED_ROLES,
            FOUNDING_REQUIRED_ROLES,
        )
        .err()
        .expect("duplicate path refuses");
        assert!(refusal.0.contains("distinct"), "{}", refusal.0);
    }

    #[test]
    fn every_required_role_is_one_a_keypair_flag_can_name() {
        for role in ADMIN_REQUIRED_ROLES.iter().chain(FOUNDING_REQUIRED_ROLES) {
            assert!(
                KEYPAIR_ROLES.contains(role),
                "{role} is required but no flag names it"
            );
        }
        // The hostile authority is deliberately NOT required: proving a refusal
        // costs a second funded wallet and two fees the operator did not ask
        // for.
        assert!(!ADMIN_REQUIRED_ROLES.contains(&role::HOSTILE_AUTHORITY));
        assert!(!FOUNDING_REQUIRED_ROLES.contains(&role::HOSTILE_AUTHORITY));
        assert!(KEYPAIR_ROLES.contains(&role::HOSTILE_AUTHORITY));
        assert!(!KEYPAIR_ROLES.contains(&role::FOUNDING_FOUNDER));
        assert!(!KEYPAIR_ROLES.contains(&role::SUBSTITUTED_FOUNDER));
        assert!(FOUNDING_REQUIRED_ROLES.contains(&role::CAMPAIGN_PAYER));
        assert!(!FOUNDING_REQUIRED_ROLES.contains(&role::CORE_UPGRADE_AUTHORITY));
    }

    #[test]
    fn founding_actor_partition_preserves_the_old_all_identity_nonalias_rule() {
        let actor_keypair = Keypair::new();
        let founder = actor_keypair.pubkey();
        let substituted = Pubkey::new_from_array([0x62; 32]);
        let signer = Keypair::new();
        let mut secrets = BTreeMap::from([(
            role::CAMPAIGN_PAYER.to_owned(),
            signer.to_bytes()[..32].try_into().expect("secret seed"),
        )]);
        authenticate_founding_actor_partition_v1(founder, substituted, &secrets)
            .expect("disjoint actors and signer");

        secrets.insert(
            role::CAMPAIGN_PAYER.to_owned(),
            actor_keypair.to_bytes()[..32]
                .try_into()
                .expect("actor secret seed"),
        );
        let refusal = authenticate_founding_actor_partition_v1(founder, substituted, &secrets)
            .expect_err("signer alias must refuse");
        assert!(
            refusal.0.contains("aliases a founding actor"),
            "{refusal:?}"
        );
        assert!(
            authenticate_founding_actor_partition_v1(founder, founder, &BTreeMap::new()).is_err()
        );
    }

    fn exact_prerequisite_states_v1() -> Vec<(StageV1, StageStateV1)> {
        [
            StageV1::Substrate,
            StageV1::Publication,
            StageV1::Initialize,
            StageV1::Succession,
            StageV1::Activation,
        ]
        .into_iter()
        .map(|stage| (stage, StageStateV1::Complete))
        .collect()
    }

    #[test]
    fn founding_only_gate_requires_every_infrastructure_stage_exactly_complete() {
        let exact = exact_prerequisite_states_v1();
        authenticate_founding_only_prerequisites_v1(&exact).expect("exact Complete prefix");
        for hostile in [
            StageStateV1::Absent,
            StageStateV1::Partial("in flight".into()),
            StageStateV1::Conflict("substituted".into()),
        ] {
            for index in 0..exact.len() {
                let mut states = exact.clone();
                states[index].1 = hostile.clone();
                let refusal = authenticate_founding_only_prerequisites_v1(&states)
                    .expect_err("non-Complete prerequisite must refuse");
                assert!(
                    refusal.0.contains(states[index].0.name())
                        && refusal.0.contains("before any key is read"),
                    "{refusal:?}"
                );
            }
        }
        let without_succession = exact
            .iter()
            .filter(|(stage, _)| *stage != StageV1::Succession)
            .cloned()
            .collect::<Vec<_>>();
        let refusal = authenticate_founding_only_prerequisites_v1(&without_succession)
            .expect_err("a literal prerequisite list that forgets succession must fail red");
        assert!(refusal.0.contains("succession"), "{refusal:?}");
    }

    #[test]
    fn detector_assembly_is_the_exact_prefounding_prefix() {
        let states = assemble_infrastructure_stage_states_v1(
            StageStateV1::Complete,
            StageStateV1::Complete,
            StageStateV1::Complete,
            StageStateV1::Complete,
            StageStateV1::Complete,
        );
        assert_eq!(
            states.iter().map(|(stage, _)| *stage).collect::<Vec<_>>(),
            StageV1::ORDER[..5],
            "the detector list must include succession between initialize and activation"
        );
    }

    #[test]
    fn activation_can_never_precede_complete_succession() {
        for activation in [
            StageStateV1::Partial("one role".into()),
            StageStateV1::Complete,
        ] {
            let states = assemble_infrastructure_stage_states_v1(
                StageStateV1::Complete,
                StageStateV1::Complete,
                StageStateV1::Complete,
                StageStateV1::Partial("Registry moved; V2 absent".into()),
                activation,
            );
            assert!(matches!(
                states
                    .iter()
                    .find(|(stage, _)| *stage == StageV1::Activation)
                    .map(|(_, state)| state),
                Some(StageStateV1::Conflict(detail)) if detail.contains("half-flipped")
            ));
        }
    }

    #[test]
    fn one_administration_ceiling_admits_succession_and_refuses_founding() {
        authenticate_administration_through_v1(StageV1::Succession)
            .expect("succession is an administration stage");
        authenticate_administration_through_v1(StageV1::Activation)
            .expect("activation remains the ceiling");
        let refusal = authenticate_administration_through_v1(StageV1::Founding)
            .expect_err("founding crosses the shared ceiling");
        assert!(refusal.0.contains("activation"), "{refusal:?}");
    }

    #[test]
    fn incomplete_succession_requires_a_distinct_campaign_payer_role() {
        let mut states = exact_prerequisite_states_v1();
        states
            .iter_mut()
            .find(|(stage, _)| *stage == StageV1::Succession)
            .expect("succession state")
            .1 = StageStateV1::Partial("Registry upgraded".into());
        assert_eq!(
            administration_required_roles_v1(&states, StageV1::Succession),
            vec![role::CORE_UPGRADE_AUTHORITY, role::CAMPAIGN_PAYER]
        );
        assert_eq!(
            administration_required_roles_v1(&states, StageV1::Initialize),
            vec![role::CORE_UPGRADE_AUTHORITY]
        );
    }

    #[test]
    fn administration_authority_is_required_only_for_an_incomplete_write_stage() {
        let exact = exact_prerequisite_states_v1();
        assert!(!administration_requires_authority_v1(
            &exact,
            StageV1::Activation
        ));
        let mut partial = exact.clone();
        partial[1].1 = StageStateV1::Partial("one record".into());
        assert!(administration_requires_authority_v1(
            &partial,
            StageV1::Activation
        ));
        assert!(!administration_requires_authority_v1(
            &partial,
            StageV1::Substrate
        ));
    }

    #[test]
    fn the_deploy_ladder_defaults_to_tpu_and_never_executes() {
        // The ladder is text. There is no code path in this module that runs a
        // deploy, and this test is the statement of that: what `deploy_ladder`
        // returns is strings, and every one of them names the transport policy
        // the measurement supports.
        let joined = ["--use-rpc", "TPU"];
        let lines = deploy_ladder(
            &SuccessorPlan {
                schema: String::new(),
                genesis_boundary: Vec::new(),
                bootstrap_order: Vec::new(),
                execution_blocker: String::new(),
                account_dir: String::new(),
                registry: pin(),
                core: pin(),
                claims: pin(),
                trading: pin(),
                resolution: pin(),
                custody: pin(),
                rent_credit: pin(),
                activation: String::new(),
                release_set_id: String::new(),
                core_bootstrap: crate::model::CoreBootstrapPin {
                    upgrade_authority: String::new(),
                    genesis_programdata_sha256: String::new(),
                    post_revoke_programdata_sha256: String::new(),
                    release_recognition_requires_revoke: false,
                },
                checked_upgrade_set: None,
                checked_local_mutable_set: None,
                infrastructure_succession: None,
                infrastructure_profile: crate::model::InfrastructureProfilePin {
                    address: String::new(),
                    schema_id: String::new(),
                    body_sha256: String::new(),
                    body_hex: String::new(),
                    registry_artifact_release_id: String::new(),
                    rent_artifact_release_id: String::new(),
                },
                records: BTreeMap::new(),
                record_publication: String::new(),
                provider_release_id: String::new(),
                fixture_publish_time: 0,
                genesis_accounts: BTreeMap::new(),
            },
            &ClusterOriginV1::parse(
                "https://api.devnet.solana.com/",
                Some(crate::cluster::DEVNET_GENESIS_HASH),
            )
            .expect("devnet"),
        );
        let text = lines.join("\n");
        for needle in joined {
            assert!(text.contains(needle), "the ladder must name {needle}");
        }
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("solana program deploy"))
                .count(),
            7,
            "one deploy per role, no more"
        );
    }

    fn pin() -> crate::model::ProgramPin {
        crate::model::ProgramPin {
            program_id: String::new(),
            programdata_id: String::new(),
            elf_path: "/dev/null".into(),
            elf_sha256: String::new(),
            checked_candidate_elf_path: "/dev/null".into(),
            checked_candidate_elf_sha256: String::new(),
            live_elf_sha256: String::new(),
            live_elf_padding_bytes: 0,
            semantic_release_id: String::new(),
            artifact_release_id: String::new(),
            upgrade_authority: None,
            deployment_slot: 0,
            deployment_source: String::new(),
            programdata_sha256: String::new(),
        }
    }
}
