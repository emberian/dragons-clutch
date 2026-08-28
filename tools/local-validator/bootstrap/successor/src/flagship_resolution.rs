//! Finalized, devnet-only exterior driver for one Pyth-resolved flagship Market.
//!
//! This module owns orchestration, durable stage receipts, and hostile resume
//! classification. It deliberately does not own any protocol wire: provider
//! submit/execute/reclaim and Core terminal admission are constructed by the
//! corresponding `dclutch-operator` builders.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims_svm::{
    founding_v5::ClaimsFoundingAggregateSeedsV5,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::{
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, Readiness};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    provider_transport_v3::{
        ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
        ProviderReclaimDeploymentV3, ProviderSubmitDeploymentV3, ProviderSubmitIntentV3,
        ProviderSubmitSnapshotV3, ProviderTransportReportV3, build_provider_execute_v3,
        build_provider_reclaim_v3, build_provider_submit_v3, compile_provider_execute_v0,
        compile_provider_reclaim_v0, compile_provider_submit_v0,
    },
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, GuardianSetV1, PostUpdateParamsView, ProgramDataV3View, ProgramV3View,
    PythReleaseV1, ReceiverConfigV2View, VerifiedEncodedVaaV1, devnet_release_v1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_resolution_core_v3_operator::provider_finalized_projection_v3::{
    ProviderExecuteFinalizedInputV3, ProviderExecuteWritableAccountsV3,
    ProviderReclaimFinalizedInputV3, ProviderReclaimWritableAccountsV3,
    ProviderSubmitFinalizedInputV3, ProviderSubmitWritableAccountsV3,
    project_finalized_provider_execute_v3, project_finalized_provider_reclaim_v3,
    project_finalized_provider_submit_v3,
};
use dclutch_source_contract::{
    PythAdapterConfigV1, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1,
    SourceResolutionRouteV1, SourceResolutionStateV2, WindowSpecV1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    instruction::{create_lookup_table, extend_lookup_table, freeze_lookup_table},
    program as lookup_table_program,
    state::AddressLookupTable,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    slot_hashes::SlotHashes,
};
use solana_sdk::{
    message::{Message, VersionedMessage},
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH},
    model::{CheckedDeploymentDispositionV1, SuccessorPlan},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, account_evidence, bounded_instructions},
    runtime::decode_hex,
    upgrade::{CHECKED_SET_PREPARE_SCHEMA, SEMANTIC_DERIVATION_V1},
    wallet_terminal::authenticate_role,
};

const INPUT_FORMAT: &str = "dclutch-flagship-resolution-input-v1";
const CHECKPOINT_FORMAT: &str = "dclutch-flagship-resolution-checkpoint-v1";
const PRODUCER_FACTS_FORMAT: &str = "dclutch-flagship-pyth-update-facts-v1";
const PRODUCER_CHECKPOINT_FORMAT: &str = "dclutch-flagship-resolution-producer-v1";
const TABLE_PROVISION_JOURNAL_FORMAT: &str = "dclutch-flagship-resolution-alt-journal-v1";
const CAMPAIGN_FORMAT: &str = "dclutch-successor-campaign-report-v1";
const PLAN_FORMAT: &str = "dclutch-local-successor-infrastructure-plan-v2";
/// Provisional operator delay after the terminal window. It is not a protocol
/// liveness bound; a measured finalized reclaim campaign can lift or narrow it.
const FLAGSHIP_RECLAIM_DELAY_SECONDS_V1: i64 = 3_600;
/// Chain-derived Pyth accumulator PriceFeedMessage V1 wire width.
const PYTH_PRICE_FEED_MESSAGE_BYTES_V1: usize = 85;
const GEOMETRY_BLOCKHASH: [u8; 32] = [0x6d; 32];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LookupTablesV1 {
    submit: String,
    execute: String,
    reclaim: String,
}

/// Routing hints only. Every field is rejoined to a finalized Market, record,
/// PDA, activation cache, or provider release before it may reach a message.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountSelectorsV1 {
    market: String,
    source_state: String,
    source_material: String,
    source_spec: String,
    source_provider_release: String,
    adapter_config: String,
    window: String,
    statistic: String,
    pyth_release: String,
    product: String,
    result_domain: String,
    portfolio: String,
    certificate: String,
    activation_cache: String,
    infrastructure: String,
    registry_program: String,
    registry_programdata: String,
    registry_artifact: String,
    registry_artifact_staging: String,
    core_program: String,
    core_programdata: String,
    claims_program: String,
    claims_programdata: String,
    claims_aggregate: String,
    resolver_position: String,
    claims_admission: String,
    trading_program: String,
    trading_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
    receiver_program: String,
    receiver_programdata: String,
    receiver_config: String,
    router_program: String,
    router_programdata: String,
    guardian_set: String,
    encoded_vaa: String,
    update_account: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanInputV1 {
    format: String,
    generation: u64,
    release_set: String,
    submitter: String,
    resolver: String,
    refund_recipient: String,
    terminal_sequence: u64,
    reclaim_after_unix_seconds: i64,
    post_update_body_base64: String,
    accounts: AccountSelectorsV1,
    lookup_tables: LookupTablesV1,
}

/// The two ephemeral provider coordinates that cannot be protocol PDAs.
///
/// `encodedVaa` is accepted only as a routing hint to an already-finalized,
/// cryptographically verified Router account. `updateAccount` is the public
/// half of the fresh Receiver signer and must still be vacant. Every other
/// address in [`PlanInputV1`] is derived from the plan, campaign, or these
/// accounts' hostile-decoded bodies.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProducerPythFactsV1 {
    format: String,
    encoded_vaa: String,
    update_account: String,
    post_update_body_base64: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CampaignAccountEvidenceV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CampaignMarketEvidenceV1 {
    completed: Vec<String>,
    accounts: BTreeMap<String, CampaignAccountEvidenceV1>,
    founding_custody_context: String,
    direct_selected_manifest_entry_index: u16,
}

#[derive(Clone, Debug, Deserialize)]
struct CampaignExecutionEnvelopeV1 {
    completed: bool,
    market: Option<CampaignMarketEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize)]
struct CampaignEvidenceV1 {
    schema: String,
    plan_sha256: String,
    execution: CampaignExecutionEnvelopeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StableAddressClassV1 {
    Beneficiary,
    MarketState,
    SourceState,
    ActivationCache,
    Infrastructure,
    Program,
    ProgramData,
    FinalizedRecord,
    FinalizedRecordStaging,
    ProviderConfig,
    ProviderObservation,
    Sysvar,
    SystemProgram,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StableAddressV1 {
    label: String,
    class: StableAddressClassV1,
    address: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LookupTablePlanV1 {
    stage: StageV1,
    creation_slot: u64,
    lookup_table: String,
    payer: String,
    authority: String,
    stable_union: Vec<StableAddressV1>,
    create: InstructionPlanV1,
    ordered_extensions: Vec<InstructionPlanV1>,
    freeze: InstructionPlanV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
enum LookupTableRouteV1 {
    Create {
        instruction: InstructionPlanV1,
    },
    Extend {
        page_index: usize,
        instruction: InstructionPlanV1,
    },
    Freeze {
        instruction: InstructionPlanV1,
    },
    Complete {
        last_extended_slot: u64,
        account_sha256: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProducerCheckpointV1 {
    format: String,
    plan_sha256: String,
    campaign_evidence_sha256: String,
    pyth_facts_sha256: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    market: String,
    generation: u64,
    payer: String,
    authority: String,
    tables: BTreeMap<StageV1, LookupTablePlanV1>,
    routes: BTreeMap<StageV1, LookupTableRouteV1>,
    planned_input: PlanInputV1,
    flagship_input: Option<PlanInputV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
enum TableProvisionActionV1 {
    Create,
    Extend { page_index: usize },
    Freeze,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TableProvisionIntentV1 {
    stage: StageV1,
    action: TableProvisionActionV1,
    lookup_table: String,
    instruction: InstructionPlanV1,
    observation_slot: u64,
    recent_blockhash: String,
    last_valid_block_height: u64,
    unsigned_message_base64: String,
    unsigned_message_sha256: String,
    exact_fee_lamports: u64,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    pre_accounts: BTreeMap<String, DurableAccountStateV1>,
    payer_pre_lamports: u64,
    table_pre_lamports: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DurablePhaseV1 {
    Planned,
    SignedNotSubmitted,
    Submitted,
    Finalized,
}

fn authenticate_send_boundary(phase: DurablePhaseV1) -> Result<()> {
    if phase != DurablePhaseV1::Submitted {
        return Err(Error::new(
            "durable packet send requires a fsynced Submitted phase",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TableProvisionReceiptV1 {
    stage: StageV1,
    action: TableProvisionActionV1,
    lookup_table: String,
    signature: String,
    slot: u64,
    fee_lamports: u64,
    payer_pre_lamports: u64,
    payer_post_lamports: u64,
    table_pre_lamports: u64,
    table_post_lamports: u64,
    table_post_account_sha256: String,
    signed_transaction_sha256: String,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    post_route: LookupTableRouteV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TableProvisionJournalV1 {
    format: String,
    producer_identity_sha256: String,
    phase: DurablePhaseV1,
    intent: Option<TableProvisionIntentV1>,
    intent_sha256: Option<String>,
    signed_transaction_base64: Option<String>,
    signed_transaction_sha256: Option<String>,
    expected_signature: Option<String>,
    finalized: Option<TableProvisionReceiptV1>,
    receipts: Vec<TableProvisionReceiptV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PriceFeedMessageV1 {
    feed_id: [u8; 32],
    price: i64,
    confidence: u64,
    exponent: i32,
    publish_time: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StageV1 {
    Submit,
    Execute,
    Reclaim,
    Complete,
}

impl StageV1 {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "submit" => Ok(Self::Submit),
            "execute" => Ok(Self::Execute),
            "reclaim" => Ok(Self::Reclaim),
            "complete" => Ok(Self::Complete),
            _ => Err(Error::new(
                "--through must be submit, execute, reclaim, or complete",
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Execute => "execute",
            Self::Reclaim => "reclaim",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug)]
struct SelectedInputV1 {
    generation: u64,
    release_set: [u8; 32],
    submitter: Pubkey,
    resolver: Pubkey,
    refund_recipient: Pubkey,
    terminal_sequence: u64,
    reclaim_after_unix_seconds: i64,
    post_update_body: Vec<u8>,
    accounts: BTreeMap<&'static str, Pubkey>,
    lookup_tables: BTreeMap<StageV1, Pubkey>,
}

impl SelectedInputV1 {
    fn parse(input: &PlanInputV1) -> Result<Self> {
        if input.format != INPUT_FORMAT {
            return Err(Error::new(format!("input format must be {INPUT_FORMAT}")));
        }
        if input.generation == 0 || input.terminal_sequence == 0 {
            return Err(Error::new(
                "generation and terminalSequence must be positive",
            ));
        }
        let release_set = hex32(&input.release_set)?;
        if release_set == [0; 32] {
            return Err(Error::new("releaseSet must be nonzero"));
        }
        let submitter = nonzero_pubkey(&input.submitter, "submitter")?;
        let resolver = nonzero_pubkey(&input.resolver, "resolver")?;
        let refund_recipient = nonzero_pubkey(&input.refund_recipient, "refundRecipient")?;
        let post_update_body = BASE64
            .decode(&input.post_update_body_base64)
            .map_err(|error| Error::new(format!("postUpdateBodyBase64: {error}")))?;
        if BASE64.encode(&post_update_body) != input.post_update_body_base64 {
            return Err(Error::new("postUpdateBodyBase64 is not canonical base64"));
        }
        PostUpdateParamsView::parse(&post_update_body)
            .map_err(|error| Error::new(format!("postUpdateBodyBase64: {error:?}")))?;
        let mut accounts = BTreeMap::new();
        macro_rules! account {
            ($label:literal, $field:ident) => {{
                let value = nonzero_pubkey(&input.accounts.$field, $label)?;
                if accounts.insert($label, value).is_some() {
                    return Err(Error::new(format!("duplicate selector label {}", $label)));
                }
            }};
        }
        account!("market", market);
        account!("source_state", source_state);
        account!("source_material", source_material);
        account!("source_spec", source_spec);
        account!("source_provider_release", source_provider_release);
        account!("adapter_config", adapter_config);
        account!("window", window);
        account!("statistic", statistic);
        account!("pyth_release", pyth_release);
        account!("product", product);
        account!("result_domain", result_domain);
        account!("portfolio", portfolio);
        account!("certificate", certificate);
        account!("activation_cache", activation_cache);
        account!("infrastructure", infrastructure);
        account!("registry_program", registry_program);
        account!("registry_programdata", registry_programdata);
        account!("registry_artifact", registry_artifact);
        account!("registry_artifact_staging", registry_artifact_staging);
        account!("core_program", core_program);
        account!("core_programdata", core_programdata);
        account!("claims_program", claims_program);
        account!("claims_programdata", claims_programdata);
        account!("claims_aggregate", claims_aggregate);
        account!("resolver_position", resolver_position);
        account!("claims_admission", claims_admission);
        account!("trading_program", trading_program);
        account!("trading_programdata", trading_programdata);
        account!("resolution_program", resolution_program);
        account!("resolution_programdata", resolution_programdata);
        account!("receiver_program", receiver_program);
        account!("receiver_programdata", receiver_programdata);
        account!("receiver_config", receiver_config);
        account!("router_program", router_program);
        account!("router_programdata", router_programdata);
        account!("guardian_set", guardian_set);
        account!("encoded_vaa", encoded_vaa);
        account!("update_account", update_account);
        let mut lookup_tables = BTreeMap::new();
        for (stage, value, label) in [
            (
                StageV1::Submit,
                &input.lookup_tables.submit,
                "lookupTables.submit",
            ),
            (
                StageV1::Execute,
                &input.lookup_tables.execute,
                "lookupTables.execute",
            ),
            (
                StageV1::Reclaim,
                &input.lookup_tables.reclaim,
                "lookupTables.reclaim",
            ),
        ] {
            lookup_tables.insert(stage, nonzero_pubkey(value, label)?);
        }
        let selected = Self {
            generation: input.generation,
            release_set,
            submitter,
            resolver,
            refund_recipient,
            terminal_sequence: input.terminal_sequence,
            reclaim_after_unix_seconds: input.reclaim_after_unix_seconds,
            post_update_body,
            accounts,
            lookup_tables,
        };
        selected.require_distinct()?;
        Ok(selected)
    }

    fn account(&self, label: &'static str) -> Result<Pubkey> {
        self.accounts
            .get(label)
            .copied()
            .ok_or_else(|| Error::new(format!("internal missing selector {label}")))
    }

    fn table(&self, stage: StageV1) -> Result<Pubkey> {
        self.lookup_tables
            .get(&stage)
            .copied()
            .ok_or_else(|| Error::new(format!("internal missing {} lookup table", stage.label())))
    }

    fn require_distinct(&self) -> Result<()> {
        let mut seen = BTreeMap::<Pubkey, &'static str>::new();
        for (&label, &key) in &self.accounts {
            if let Some(other) = seen.insert(key, label) {
                return Err(Error::new(format!(
                    "address-book substitution: {label} and {other} both name {key}"
                )));
            }
        }
        for (label, key) in [
            ("lookupTables.submit", self.table(StageV1::Submit)?),
            ("lookupTables.execute", self.table(StageV1::Execute)?),
            ("lookupTables.reclaim", self.table(StageV1::Reclaim)?),
        ] {
            if let Some(other) = seen.insert(key, label) {
                return Err(Error::new(format!(
                    "address-book substitution: {label} and {other} both name {key}"
                )));
            }
        }
        for (label, key) in [
            ("submitter", self.submitter),
            ("resolver", self.resolver),
            ("refundRecipient", self.refund_recipient),
        ] {
            if let Some(other) = seen.get(&key) {
                return Err(Error::new(format!(
                    "address-book substitution: {label} and {other} both name {key}"
                )));
            }
            seen.insert(key, label);
        }
        Ok(())
    }
}

fn nonzero_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    let key = pubkey(value)?;
    if key == Pubkey::default() {
        return Err(Error::new(format!("{label} must be nonzero")));
    }
    Ok(key)
}

fn parse_price_feed_message(body: &[u8]) -> Result<PriceFeedMessageV1> {
    let params = PostUpdateParamsView::parse(body)
        .map_err(|error| Error::new(format!("Pyth PostUpdateParams: {error:?}")))?;
    let message = params.message();
    if message.len() != PYTH_PRICE_FEED_MESSAGE_BYTES_V1 || message.first() != Some(&0) {
        return Err(Error::new(
            "Pyth update must carry one exact 85-byte PriceFeedMessage variant",
        ));
    }
    let array = |start: usize, width: usize| -> Result<&[u8]> {
        message
            .get(start..start.saturating_add(width))
            .ok_or_else(|| Error::new("Pyth PriceFeedMessage field exceeded its exact body"))
    };
    Ok(PriceFeedMessageV1 {
        feed_id: array(1, 32)?
            .try_into()
            .map_err(|_| Error::new("Pyth feed id width changed"))?,
        price: i64::from_be_bytes(
            array(33, 8)?
                .try_into()
                .map_err(|_| Error::new("Pyth price width changed"))?,
        ),
        confidence: u64::from_be_bytes(
            array(41, 8)?
                .try_into()
                .map_err(|_| Error::new("Pyth confidence width changed"))?,
        ),
        exponent: i32::from_be_bytes(
            array(49, 4)?
                .try_into()
                .map_err(|_| Error::new("Pyth exponent width changed"))?,
        ),
        publish_time: i64::from_be_bytes(
            array(53, 8)?
                .try_into()
                .map_err(|_| Error::new("Pyth publication width changed"))?,
        ),
    })
}

fn parse_producer_facts(bytes: &[u8]) -> Result<(ProducerPythFactsV1, Pubkey, Pubkey, Vec<u8>)> {
    let facts: ProducerPythFactsV1 = serde_json::from_slice(bytes)?;
    if facts.format != PRODUCER_FACTS_FORMAT {
        return Err(Error::new(format!(
            "Pyth facts format must be {PRODUCER_FACTS_FORMAT}"
        )));
    }
    let encoded_vaa = nonzero_pubkey(&facts.encoded_vaa, "encodedVaa")?;
    let update_account = nonzero_pubkey(&facts.update_account, "updateAccount")?;
    if encoded_vaa == update_account {
        return Err(Error::new(
            "encodedVaa and the fresh Receiver update signer must be distinct",
        ));
    }
    let body = BASE64
        .decode(&facts.post_update_body_base64)
        .map_err(|error| Error::new(format!("postUpdateBodyBase64: {error}")))?;
    if BASE64.encode(&body) != facts.post_update_body_base64 {
        return Err(Error::new("postUpdateBodyBase64 is not canonical base64"));
    }
    parse_price_feed_message(&body)?;
    Ok((facts, encoded_vaa, update_account, body))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotKindV1 {
    Vacant,
    Submitted,
    Consumed,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChainFactsV1 {
    market_phase: CorePhase,
    market_readiness: Readiness,
    source_phase: SourceResolutionPhaseV1,
    lifecycle: SlotKindV1,
    update: SlotKindV1,
    certificate: SlotKindV1,
}

fn classify(facts: ChainFactsV1) -> Result<StageV1> {
    use SlotKindV1::{Consumed, Other, Submitted, Vacant};
    match (
        facts.market_phase,
        facts.market_readiness,
        facts.source_phase,
        facts.lifecycle,
        facts.update,
        facts.certificate,
    ) {
        (
            CorePhase::Open,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Primary,
            Vacant,
            Vacant,
            Vacant,
        ) => Ok(StageV1::Submit),
        (
            CorePhase::Open,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Primary,
            Submitted,
            Submitted,
            Vacant,
        ) => Ok(StageV1::Execute),
        (
            CorePhase::Terminal,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted,
            Consumed,
            Submitted,
            Submitted,
        ) => Ok(StageV1::Reclaim),
        (
            CorePhase::Terminal,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted,
            Vacant,
            Vacant,
            Submitted,
        ) => Ok(StageV1::Complete),
        (_, _, _, Other, _, _) | (_, _, _, _, Other, _) | (_, _, _, _, _, Other) => Err(
            Error::new("ambiguous submitted state: an output account has an unknown owner or body"),
        ),
        _ => Err(Error::new(
            "ambiguous submitted state: Market, Source, lifecycle, update, and certificate do not form one canonical stage",
        )),
    }
}

#[derive(Clone)]
struct FinalizedSnapshotV1 {
    observation: Observation,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl FinalizedSnapshotV1 {
    fn account(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.accounts
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("finalized snapshot is missing {label} {key}")))
    }

    fn optional(&self, key: Pubkey) -> Option<&RpcAccount> {
        self.accounts.get(&key).and_then(Option::as_ref)
    }

    fn observed(&self, key: Pubkey, label: &str) -> Result<ObservedAccount> {
        let account = self.account(key, label)?;
        Ok(ObservedAccount {
            observation: self.observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data.clone(),
        })
    }

    fn observed_or_vacant(&self, key: Pubkey) -> ObservedAccount {
        let account = self.optional(key).cloned().unwrap_or(RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        });
        ObservedAccount {
            observation: self.observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data,
        }
    }
}

fn observe(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    _stage: StageV1,
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let mut keys = BTreeSet::new();
    keys.extend(selected.accounts.values().copied());
    keys.insert(lifecycle_address(selected)?);
    keys.extend(selected.lookup_tables.values().copied());
    keys.insert(sysvar::rent::ID);
    if keys.len() > 100 {
        return Err(Error::new(
            "flagship finalized snapshot exceeds the 100-account RPC bound",
        ));
    }
    observe_keys(rpc, keys, minimum_slot)
}

fn snapshot_rent(snapshot: &FinalizedSnapshotV1) -> Result<Rent> {
    let account = snapshot.account(sysvar::rent::ID, "Rent sysvar")?;
    if account.owner != sysvar::ID || account.executable {
        return Err(Error::new(
            "Rent sysvar owner or executable flag is not canonical",
        ));
    }
    bincode::deserialize(&account.data).map_err(|error| Error::new(format!("Rent sysvar: {error}")))
}

fn observe_keys(
    rpc: &mut Rpc,
    keys: BTreeSet<Pubkey>,
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    if keys.len() > 100 {
        return Err(Error::new(
            "flagship finalized snapshot exceeds the 100-account RPC bound",
        ));
    }
    let ordered = keys.into_iter().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&ordered, minimum_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    Ok(FinalizedSnapshotV1 {
        observation,
        accounts: ordered.into_iter().zip(values).collect(),
    })
}

fn chain_facts(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<ChainFactsV1> {
    let market_key = selected.account("market")?;
    let core = selected.account("core_program")?;
    let market_account = snapshot.account(market_key, "Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if market_account.owner != core
        || market_account.executable
        || market.identity.market_id.to_bytes() != market_key.to_bytes()
        || market.identity.generation != selected.generation
        || market.identity.selected_release_set.to_bytes() != selected.release_set
        || market.identity.registry_program.to_bytes()
            != selected.account("registry_program")?.to_bytes()
        || market.identity.resolution_policy.to_bytes()
            != hash(
                &snapshot
                    .account(selected.account("source_material")?, "SourceMaterial")?
                    .data,
            )
            .to_bytes()
    {
        return Err(Error::new(
            "wrong release, Market, generation, or source material",
        ));
    }
    authenticate_selected_resolver(selected, snapshot, market)?;
    let source_key = selected.account("source_state")?;
    let resolution = selected.account("resolution_program")?;
    let source_account = snapshot.account(source_key, "Source state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("Source state: {error:?}")))?;
    if source_account.owner != resolution
        || source_account.executable
        || source.market() != market_key.to_bytes()
        || source.generation() != selected.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
    {
        return Err(Error::new(
            "Source state does not belong to this Market generation",
        ));
    }
    let lifecycle = lifecycle_kind(selected, snapshot, market_key, source_key)?;
    let update = update_kind(selected, snapshot)?;
    let certificate = certificate_kind(selected, snapshot, market_key)?;
    Ok(ChainFactsV1 {
        market_phase: market.phase,
        market_readiness: market.readiness,
        source_phase: source.phase(),
        lifecycle,
        update,
        certificate,
    })
}

fn authenticate_selected_resolver(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    market: CoreState,
) -> Result<()> {
    let market_key = selected.account("market")?;
    let claims = selected.account("claims_program")?;
    let aggregate_key = selected.account("claims_aggregate")?;
    let position_key = selected.account("resolver_position")?;
    let admission_key = selected.account("claims_admission")?;
    let aggregate_account = snapshot.account(aggregate_key, "Claims aggregate")?;
    let position_account = snapshot.account(position_key, "resolver Position")?;
    let admission_account = snapshot.account(admission_key, "Claims admission")?;
    if aggregate_account.owner != claims
        || aggregate_account.executable
        || position_account.owner != claims
        || position_account.executable
        || admission_account.owner != claims
        || admission_account.executable
    {
        return Err(Error::new(
            "resolver is not carried by current non-executable Claims state",
        ));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("resolver Position: {error:?}")))?;
    let admission = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .map_err(|error| Error::new(format!("Claims admission: {error:?}")))?;
    let expected_aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(market_key.to_bytes())
            .map_err(|error| Error::new(format!("Claims aggregate seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let expected_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), selected.resolver.to_bytes())
            .map_err(|error| Error::new(format!("resolver Position seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let expected_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            aggregate_key.to_bytes(),
            selected.resolver.to_bytes(),
        )
        .map_err(|error| Error::new(format!("Claims admission seeds: {error:?}")))?
        .as_slices(),
        &claims,
    )
    .0;
    if aggregate_key != expected_aggregate
        || position_key != expected_position
        || admission_key != expected_admission
        || position.owner != selected.resolver.to_bytes()
        || position.market_account != aggregate_key.to_bytes()
        || position.basis_id != aggregate.basis_id
        || position.claim_count != aggregate.claim_count
        || aggregate.logical_market != market_key.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market.identity.registry_program.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.generation != market.identity.generation
        || admission.owner_kind() != ProtocolPositionOwnerKindV2::User
        || admission.market() != market_key.to_bytes()
        || admission.position_owner() != selected.resolver.to_bytes()
        || admission.release_set() != selected.release_set
        || admission.generation() != selected.generation
        || admission.product_record_digest() != market.identity.product_record.to_bytes()
        || admission.semantic_basis_id() != aggregate.basis_id
        || admission.outcome_count() != aggregate.claim_count
        || admission.claims_program() != claims.to_bytes()
        || admission.trading_program() != selected.account("trading_program")?.to_bytes()
        || admission.rent_credit() != market.rent_beneficiary.to_bytes()
        || admission.capability_descriptor() != [0; 32]
        || admission.capability_outcome() != 0
        || admission.position_lamports() != position_account.lamports
        || admission.admission_lamports() != admission_account.lamports
        || admission.position_rent_principal() > position_account.lamports
        || admission.admission_rent_principal() > admission_account.lamports
    {
        return Err(Error::new(
            "resolver is not the current canonical admitted Position owner for this Market",
        ));
    }
    Ok(())
}

fn is_vacant(account: Option<&RpcAccount>) -> bool {
    match account {
        None => true,
        Some(account) => {
            account.owner == system_program::ID && !account.executable && account.data.is_empty()
        }
    }
}

fn lifecycle_kind(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    market: Pubkey,
    source_state: Pubkey,
) -> Result<SlotKindV1> {
    let update = selected.account("update_account")?;
    let resolution = selected.account("resolution_program")?;
    let lifecycle = lifecycle_address(selected)?;
    let account = snapshot.optional(lifecycle);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != resolution || account.executable {
        return Ok(SlotKindV1::Other);
    }
    let lifecycle = match ProviderUpdateLifecycleV3::decode(&account.data) {
        Ok(value) => value,
        Err(_) => return Ok(SlotKindV1::Other),
    };
    if lifecycle.market != market.to_bytes()
        || lifecycle.source_state != source_state.to_bytes()
        || lifecycle.generation != selected.generation
        || lifecycle.release_set != selected.release_set
        || lifecycle.update_account != update.to_bytes()
        || lifecycle.provider_submitter != selected.submitter.to_bytes()
        || lifecycle.refund_recipient != selected.refund_recipient.to_bytes()
        || lifecycle.post_body_digest != hash(&selected.post_update_body).to_bytes()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(match lifecycle.status {
        ProviderUpdateStatusV3::Submitted => SlotKindV1::Submitted,
        ProviderUpdateStatusV3::Consumed => SlotKindV1::Consumed,
    })
}

fn lifecycle_address(selected: &SelectedInputV1) -> Result<Pubkey> {
    Ok(Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            selected.account("update_account")?.as_ref(),
        ],
        &selected.account("resolution_program")?,
    )
    .0)
}

fn stable_lookup_union(selected: &SelectedInputV1, stage: StageV1) -> Result<Vec<StableAddressV1>> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push =
        |label: &'static str, class: StableAddressClassV1, address: Pubkey| -> Result<()> {
            if !seen.insert(address) {
                return Err(Error::new(format!(
                    "{} stable ALT union aliases {label} at {address}",
                    stage.label()
                )));
            }
            rows.push(StableAddressV1 {
                label: label.to_owned(),
                class,
                address: address.to_string(),
            });
            Ok(())
        };
    macro_rules! selected {
        ($label:literal, $class:ident) => {
            push(
                $label,
                StableAddressClassV1::$class,
                selected.account($label)?,
            )?
        };
    }
    macro_rules! common_release {
        () => {
            selected!("activation_cache", ActivationCache);
            selected!("infrastructure", Infrastructure);
            selected!("registry_program", Program);
            selected!("registry_programdata", ProgramData);
            selected!("registry_artifact", FinalizedRecord);
            selected!("registry_artifact_staging", FinalizedRecordStaging);
            selected!("core_program", Program);
            selected!("core_programdata", ProgramData);
            selected!("resolution_program", Program);
            selected!("resolution_programdata", ProgramData);
        };
    }
    macro_rules! provider_programs {
        () => {
            selected!("receiver_program", Program);
            selected!("receiver_programdata", ProgramData);
            selected!("receiver_config", ProviderConfig);
            selected!("router_program", Program);
            selected!("router_programdata", ProgramData);
        };
    }
    match stage {
        StageV1::Submit => {
            push(
                "refund_recipient",
                StableAddressClassV1::Beneficiary,
                selected.refund_recipient,
            )?;
            selected!("market", MarketState);
            common_release!();
            selected!("source_state", SourceState);
            selected!("source_material", FinalizedRecord);
            selected!("source_spec", FinalizedRecord);
            selected!("source_provider_release", FinalizedRecord);
            selected!("pyth_release", FinalizedRecord);
            selected!("window", FinalizedRecord);
            provider_programs!();
            selected!("encoded_vaa", ProviderObservation);
            selected!("guardian_set", ProviderObservation);
        }
        StageV1::Execute => {
            common_release!();
            selected!("trading_program", Program);
            selected!("trading_programdata", ProgramData);
            selected!("source_material", FinalizedRecord);
            selected!("source_spec", FinalizedRecord);
            selected!("source_provider_release", FinalizedRecord);
            selected!("adapter_config", FinalizedRecord);
            selected!("window", FinalizedRecord);
            selected!("statistic", FinalizedRecord);
            selected!("pyth_release", FinalizedRecord);
            selected!("product", FinalizedRecord);
            selected!("result_domain", FinalizedRecord);
            selected!("portfolio", FinalizedRecord);
            selected!("update_account", ProviderObservation);
            provider_programs!();
        }
        StageV1::Reclaim => {
            push(
                "refund_recipient",
                StableAddressClassV1::Beneficiary,
                selected.refund_recipient,
            )?;
            selected!("certificate", SourceState);
            selected!("activation_cache", ActivationCache);
            selected!("registry_program", Program);
            selected!("registry_programdata", ProgramData);
            selected!("resolution_program", Program);
            selected!("resolution_programdata", ProgramData);
            selected!("pyth_release", FinalizedRecord);
            selected!("receiver_program", Program);
            selected!("receiver_programdata", ProgramData);
        }
        StageV1::Complete => {
            return Err(Error::new("complete has no lookup-table union"));
        }
    }
    push(
        "clock_sysvar",
        StableAddressClassV1::Sysvar,
        sysvar::clock::ID,
    )?;
    push(
        "rent_sysvar",
        StableAddressClassV1::Sysvar,
        sysvar::rent::ID,
    )?;
    push(
        "system_program",
        StableAddressClassV1::SystemProgram,
        system_program::ID,
    )?;
    Ok(rows)
}

fn stable_union_addresses(rows: &[StableAddressV1]) -> Result<Vec<Pubkey>> {
    rows.iter()
        // The System Program is canonically Pubkey::default(); every other
        // row was already nonzero-authenticated while deriving the union.
        .map(|row| pubkey(&row.address))
        .collect()
}

fn build_lookup_table_plan(
    selected: &SelectedInputV1,
    stage: StageV1,
    creation_slot: u64,
    authority: Pubkey,
) -> Result<LookupTablePlanV1> {
    let stable_union = stable_lookup_union(selected, stage)?;
    let addresses = stable_union_addresses(&stable_union)?;
    let (create, lookup_table) = create_lookup_table(authority, authority, creation_slot);
    if lookup_table != selected.table(stage)? {
        return Err(Error::new(format!(
            "{} lookup table differs from its durable creation slot",
            stage.label()
        )));
    }
    let ordered_extensions = addresses
        .chunks(dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1)
        .map(|page| {
            InstructionPlanV1::from_instruction(&extend_lookup_table(
                lookup_table,
                authority,
                Some(authority),
                page.to_vec(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LookupTablePlanV1 {
        stage,
        creation_slot,
        lookup_table: lookup_table.to_string(),
        payer: authority.to_string(),
        authority: authority.to_string(),
        stable_union,
        create: InstructionPlanV1::from_instruction(&create)?,
        ordered_extensions,
        freeze: InstructionPlanV1::from_instruction(&freeze_lookup_table(lookup_table, authority))?,
    })
}

fn authenticate_lookup_table_plan(
    selected: &SelectedInputV1,
    plan: &LookupTablePlanV1,
) -> Result<()> {
    if plan.stage == StageV1::Complete {
        return Err(Error::new("complete cannot own a provider lookup table"));
    }
    let expected =
        build_lookup_table_plan(selected, plan.stage, plan.creation_slot, selected.resolver)?;
    if *plan != expected {
        return Err(Error::new(format!(
            "{} lookup-table plan differs from its exact typed union or durable instruction sequence",
            plan.stage.label()
        )));
    }
    Ok(())
}

fn decoded_lookup_table<'a>(
    key: Pubkey,
    owner: Pubkey,
    executable: bool,
    data: &'a [u8],
    purpose: &str,
) -> Result<AddressLookupTable<'a>> {
    if owner != lookup_table_program::ID || executable {
        return Err(Error::new(format!(
            "{purpose} is not a non-executable Address Lookup Table account"
        )));
    }
    AddressLookupTable::deserialize(data)
        .map_err(|_| Error::new(format!("{purpose} {key} has noncanonical table bytes")))
}

fn expected_last_extension_start(address_count: usize) -> Result<u8> {
    if address_count == 0 {
        return Ok(0);
    }
    let page = dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
    let start = address_count.saturating_sub(1) / page * page;
    u8::try_from(start).map_err(|_| Error::new("lookup table extension start exceeded u8"))
}

fn authenticate_frozen_lookup_table(
    selected: &SelectedInputV1,
    stage: StageV1,
    table: &ObservedAccount,
    rent: &Rent,
) -> Result<()> {
    if table.key != selected.table(stage)? {
        return Err(Error::new(format!(
            "{} lookup table address substitution refused",
            stage.label()
        )));
    }
    let decoded = decoded_lookup_table(
        table.key,
        table.owner,
        table.executable,
        &table.data,
        stage.label(),
    )?;
    let expected = stable_union_addresses(&stable_lookup_union(selected, stage)?)?;
    if decoded.meta.authority.is_some()
        || table.lamports != rent.minimum_balance(table.data.len())
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || expected_last_extension_start(expected.len())?
            != decoded.meta.last_extended_slot_start_index
        || decoded.addresses.as_ref() != expected.as_slice()
    {
        return Err(Error::new(format!(
            "{} lookup table is not the fresh frozen exact typed stable union",
            stage.label()
        )));
    }
    Ok(())
}

fn route_lookup_table(
    plan: &LookupTablePlanV1,
    account: Option<&RpcAccount>,
    observation_slot: u64,
    rent: &Rent,
) -> Result<LookupTableRouteV1> {
    let table_key = pubkey(&plan.lookup_table)?;
    let authority = pubkey(&plan.authority)?;
    let expected = stable_union_addresses(&plan.stable_union)?;
    let Some(account) = account else {
        return Ok(LookupTableRouteV1::Create {
            instruction: plan.create.clone(),
        });
    };
    if account.lamports != rent.minimum_balance(account.data.len()) {
        return Err(Error::new(format!(
            "{} table lamports differ from the exact current rent minimum",
            plan.stage.label()
        )));
    }
    let decoded = decoded_lookup_table(
        table_key,
        account.owner,
        account.executable,
        &account.data,
        plan.stage.label(),
    )?;
    if decoded.meta.deactivation_slot != u64::MAX
        || decoded.addresses.len() > expected.len()
        || decoded.addresses.as_ref() != &expected[..decoded.addresses.len()]
        || expected_last_extension_start(decoded.addresses.len())?
            != decoded.meta.last_extended_slot_start_index
        || (!decoded.addresses.is_empty() && decoded.meta.last_extended_slot < plan.creation_slot)
    {
        return Err(Error::new(format!(
            "{} table contains a reordered, substituted, extra, or deactivated address",
            plan.stage.label()
        )));
    }
    if decoded.meta.authority.is_none() {
        if decoded.addresses.as_ref() != expected.as_slice()
            || decoded.meta.last_extended_slot >= observation_slot
        {
            return Err(Error::new(format!(
                "{} frozen table is stale or not the exact stable union",
                plan.stage.label()
            )));
        }
        return Ok(LookupTableRouteV1::Complete {
            last_extended_slot: decoded.meta.last_extended_slot,
            account_sha256: account_evidence(table_key, account).account_sha256,
        });
    }
    if decoded.meta.authority != Some(authority) {
        return Err(Error::new(format!(
            "{} table authority substitution refused",
            plan.stage.label()
        )));
    }
    if decoded.addresses.len() == expected.len() {
        return Ok(LookupTableRouteV1::Freeze {
            instruction: plan.freeze.clone(),
        });
    }
    let page_size = dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
    let page_index = decoded.addresses.len() / page_size;
    let expected_prefix = page_index.saturating_mul(page_size);
    if decoded.addresses.len() != expected_prefix {
        return Err(Error::new(format!(
            "{} table stopped between canonical extension pages",
            plan.stage.label()
        )));
    }
    let instruction = plan
        .ordered_extensions
        .get(page_index)
        .cloned()
        .ok_or_else(|| Error::new("lookup table extension page exceeded its durable plan"))?;
    Ok(LookupTableRouteV1::Extend {
        page_index,
        instruction,
    })
}

fn update_kind(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<SlotKindV1> {
    let key = selected.account("update_account")?;
    let account = snapshot.optional(key);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != selected.account("receiver_program")?
        || account.executable
        || FullPriceUpdateV2::parse(&account.data).is_err()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(SlotKindV1::Submitted)
}

fn certificate_kind(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    market: Pubkey,
) -> Result<SlotKindV1> {
    let key = selected.account("certificate")?;
    let account = snapshot.optional(key);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != selected.account("resolution_program")? || account.executable {
        return Ok(SlotKindV1::Other);
    }
    let certificate = match ResolutionCertificateV2::decode(&account.data) {
        Ok(value) => value,
        Err(_) => return Ok(SlotKindV1::Other),
    };
    if certificate.market != market.to_bytes()
        || certificate.generation != selected.generation
        || certificate.receipt_account != key.to_bytes()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(SlotKindV1::Submitted)
}

fn authenticate_current_deployments(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<()> {
    let registry = snapshot.observed(selected.account("registry_program")?, "Registry program")?;
    let activation =
        snapshot.observed(selected.account("activation_cache")?, "activation cache")?;
    for (role, program_label, programdata_label) in [
        (ExecutionRoleV1::Core, "core_program", "core_programdata"),
        (
            ExecutionRoleV1::Trading,
            "trading_program",
            "trading_programdata",
        ),
        (
            ExecutionRoleV1::Claims,
            "claims_program",
            "claims_programdata",
        ),
        (
            ExecutionRoleV1::Resolution,
            "resolution_program",
            "resolution_programdata",
        ),
    ] {
        authenticate_role(
            &registry,
            &activation,
            selected.release_set,
            role,
            &snapshot.observed(selected.account(program_label)?, program_label)?,
            &snapshot.observed(selected.account(programdata_label)?, programdata_label)?,
        )?;
    }
    Ok(())
}

fn authenticate_devnet_pyth(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    require_provider_observation: bool,
) -> Result<PythReleaseV1> {
    let release = PythReleaseV1::decode(
        &snapshot
            .account(selected.account("pyth_release")?, "Pyth release record")?
            .data,
    )
    .map_err(|error| Error::new(format!("Pyth release: {error:?}")))?;
    let expected = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    if release.to_bytes() != expected.to_bytes() {
        return Err(Error::new(
            "Pyth release record is not the exact devnet production row",
        ));
    }
    for (program_label, programdata_label, expected_slot, expected_program, expected_data) in [
        (
            "receiver_program",
            "receiver_programdata",
            release.receiver_deployment_slot(),
            release.receiver_program(),
            release.receiver_programdata(),
        ),
        (
            "router_program",
            "router_programdata",
            release.router_deployment_slot(),
            release.router_program(),
            release.router_programdata(),
        ),
    ] {
        let program_key = selected.account(program_label)?;
        let data_key = selected.account(programdata_label)?;
        let program = snapshot.account(program_key, program_label)?;
        let programdata = snapshot.account(data_key, programdata_label)?;
        let view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{program_label}: {error:?}")))?;
        let data_view = ProgramDataV3View::parse(&programdata.data)
            .map_err(|error| Error::new(format!("{programdata_label}: {error:?}")))?;
        if program_key.to_bytes() != expected_program
            || data_key.to_bytes() != expected_data
            || program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || view.programdata() != data_key.to_bytes()
            || programdata.owner != bpf_loader_upgradeable::ID
            || programdata.executable
            || data_view.deployment_slot() != expected_slot
        {
            return Err(Error::new(format!(
                "current {program_label} Program/ProgramData link, slot, owner, or executable bit refused"
            )));
        }
    }
    if selected.account("receiver_config")?.to_bytes() != release.receiver_config() {
        return Err(Error::new("Receiver Config address substitution refused"));
    }
    let config = snapshot.account(selected.account("receiver_config")?, "Receiver Config")?;
    let config_view = ReceiverConfigV2View::parse(&config.data)
        .map_err(|error| Error::new(format!("Receiver Config body: {error:?}")))?;
    if config.owner.to_bytes() != release.receiver_program()
        || config.executable
        || hash(&config.data).to_bytes() != release.config_digest()
        || config_view.router_program() != release.router_program()
        || config_view.minimum_signatures() != release.required_guardian_count()
    {
        return Err(Error::new(
            "Receiver Config owner, body, router, or threshold refused",
        ));
    }
    if !require_provider_observation {
        return Ok(release);
    }
    let encoded = snapshot.account(selected.account("encoded_vaa")?, "verified EncodedVaa")?;
    let encoded_view = VerifiedEncodedVaaV1::parse(&encoded.data)
        .map_err(|error| Error::new(format!("verified EncodedVaa: {error:?}")))?;
    let guardian = snapshot.account(selected.account("guardian_set")?, "GuardianSet")?;
    let guardian_view = GuardianSetV1::parse(&guardian.data)
        .map_err(|error| Error::new(format!("GuardianSet: {error:?}")))?;
    let expected_guardian = Pubkey::find_program_address(
        &[
            b"GuardianSet",
            &encoded_view.guardian_set_index().to_be_bytes(),
        ],
        &selected.account("router_program")?,
    )
    .0;
    if encoded.owner != selected.account("router_program")?
        || encoded.executable
        || encoded_view.write_authority() != selected.submitter.to_bytes()
        || guardian.owner != selected.account("router_program")?
        || guardian.executable
        || selected.account("guardian_set")? != expected_guardian
        || guardian_view
            .authenticate(
                encoded_view,
                release.guardian_set_count(),
                release.required_guardian_count(),
            )
            .is_err()
    {
        return Err(Error::new(
            "EncodedVaa/GuardianSet account, authority, PDA, or signature threshold refused",
        ));
    }
    Ok(release)
}

fn preflight_posted_observation(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<()> {
    let update = FullPriceUpdateV2::parse(
        &snapshot
            .account(selected.account("update_account")?, "Receiver update")?
            .data,
    )
    .map_err(|error| Error::new(format!("Receiver update: {error:?}")))?;
    let window = WindowSpecV1::decode(
        &snapshot
            .account(selected.account("window")?, "WindowSpec")?
            .data,
    )
    .map_err(|error| Error::new(format!("WindowSpec: {error:?}")))?;
    let adapter = PythAdapterConfigV1::decode(
        &snapshot
            .account(selected.account("adapter_config")?, "PythAdapterConfig")?
            .data,
    )
    .map_err(|error| Error::new(format!("PythAdapterConfig: {error:?}")))?;
    validate_observation_fields(
        update.publish_time(),
        update.feed_id(),
        update.price(),
        update.confidence(),
        update.exponent(),
        snapshot.observation.unix_timestamp,
        window,
        adapter,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_observation_fields(
    publication: i64,
    feed_id: [u8; 32],
    price: i64,
    confidence: u64,
    exponent: i32,
    finalized_now: i64,
    window: WindowSpecV1,
    adapter: PythAdapterConfigV1,
) -> Result<()> {
    let oldest = finalized_now
        .checked_sub(i64::from(window.max_age_seconds()))
        .ok_or_else(|| Error::new("Pyth freshness lower bound overflow"))?;
    let newest = finalized_now
        .checked_add(i64::from(window.max_future_skew_seconds()))
        .ok_or_else(|| Error::new("Pyth freshness upper bound overflow"))?;
    let in_schedule = window
        .contains_observation(publication)
        .map_err(|error| Error::new(format!("Pyth observation schedule: {error:?}")))?;
    if !in_schedule || publication < oldest || publication > newest {
        return Err(Error::new(format!(
            "stale or wrong-period Pyth observation: publication {publication}, Market window [{}, {}], finalized freshness band [{oldest}, {newest}]",
            window.start_unix_seconds(),
            window.end_unix_seconds()
        )));
    }
    let confidence_limit = u128::from(price.unsigned_abs())
        .checked_mul(u128::from(adapter.max_confidence_bps()))
        .ok_or_else(|| Error::new("Pyth confidence limit overflow"))?;
    let observed_confidence = u128::from(confidence)
        .checked_mul(10_000)
        .ok_or_else(|| Error::new("Pyth observed confidence overflow"))?;
    if feed_id != adapter.provider_feed_id()
        || exponent != adapter.expected_exponent()
        || observed_confidence > confidence_limit
    {
        return Err(Error::new(
            "Pyth update feed, exponent, or confidence differs from the finalized adapter record",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountMetaPlanV1 {
    pubkey: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionPlanV1 {
    program_id: String,
    accounts: Vec<AccountMetaPlanV1>,
    data_base64: String,
    sha256: String,
}

impl InstructionPlanV1 {
    fn from_instruction(instruction: &Instruction) -> Result<Self> {
        let serialized = bincode::serialize(instruction)
            .map_err(|error| Error::new(format!("serialize stage instruction: {error}")))?;
        Ok(Self {
            program_id: instruction.program_id.to_string(),
            accounts: instruction
                .accounts
                .iter()
                .map(|meta| AccountMetaPlanV1 {
                    pubkey: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                })
                .collect(),
            data_base64: BASE64.encode(&instruction.data),
            sha256: hex(&Sha256::digest(serialized)),
        })
    }

    fn instruction(&self) -> Result<Instruction> {
        let instruction = Instruction {
            program_id: pubkey(&self.program_id)?,
            accounts: self
                .accounts
                .iter()
                .map(|meta| {
                    Ok(if meta.writable {
                        AccountMeta::new(pubkey(&meta.pubkey)?, meta.signer)
                    } else {
                        AccountMeta::new_readonly(pubkey(&meta.pubkey)?, meta.signer)
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            data: BASE64
                .decode(&self.data_base64)
                .map_err(|error| Error::new(format!("checkpoint instruction body: {error}")))?,
        };
        if InstructionPlanV1::from_instruction(&instruction)?.sha256 != self.sha256 {
            return Err(Error::new("checkpoint instruction digest mismatch"));
        }
        Ok(instruction)
    }
}

impl StagePlanV1 {
    fn validate(&self) -> Result<()> {
        if self.stage == StageV1::Complete
            || self.observation_slot == 0
            || self.required_signers.is_empty()
            || self.compiled_wire_bytes > 1_232
        {
            return Err(Error::new("durable stage header is not executable"));
        }
        let payer = pubkey(
            self.required_signers
                .first()
                .ok_or_else(|| Error::new("durable stage omitted payer"))?,
        )?;
        let action = self.action.instruction()?;
        let mut unbounded = Vec::with_capacity(self.transfers.len() + 1);
        for top_up in &self.transfers {
            unbounded.push(transfer(
                &payer,
                &pubkey(&top_up.destination)?,
                top_up.lamports,
            ));
        }
        unbounded.push(action);
        let bounded = bounded_instructions(&unbounded, None)?;
        let expected = bounded
            .iter()
            .map(InstructionPlanV1::from_instruction)
            .collect::<Result<Vec<_>>>()?;
        if expected != self.transaction_instructions {
            return Err(Error::new(
                "durable transaction instructions differ from transfers, action, or compute policy",
            ));
        }
        pubkey(&self.lookup_table)?;
        pubkey(&self.mutation_account)?;
        for signer in &self.required_signers {
            pubkey(signer)?;
        }
        let message_bytes = BASE64
            .decode(&self.message_base64)
            .map_err(|error| Error::new(format!("durable provider message base64: {error}")))?;
        if BASE64.encode(&message_bytes) != self.message_base64
            || hex(&Sha256::digest(&message_bytes)) != self.message_sha256
        {
            return Err(Error::new("durable provider message digest changed"));
        }
        let message: VersionedMessage = bincode::deserialize(&message_bytes)
            .map_err(|error| Error::new(format!("durable provider message: {error}")))?;
        let blockhash = self
            .recent_blockhash
            .parse::<Hash>()
            .map_err(|error| Error::new(format!("durable provider blockhash: {error}")))?;
        let VersionedMessage::V0(v0) = &message else {
            return Err(Error::new("durable provider message was not v0"));
        };
        if v0.recent_blockhash.to_string() != self.recent_blockhash
            || v0.address_table_lookups.len() != 1
            || v0.address_table_lookups[0].account_key.to_string() != self.lookup_table
            || usize::from(v0.header.num_required_signatures) != self.required_signers.len()
            || v0.account_keys[..self.required_signers.len()]
                .iter()
                .map(ToString::to_string)
                .ne(self.required_signers.iter().cloned())
        {
            return Err(Error::new(
                "durable provider blockhash, table, or signer boundary changed",
            ));
        }
        let table_data = BASE64
            .decode(&self.lookup_table_account.data_base64)
            .map_err(|error| Error::new(format!("durable provider table base64: {error}")))?;
        if BASE64.encode(&table_data) != self.lookup_table_account.data_base64
            || hex(&Sha256::digest(&table_data)) != self.lookup_table_account.data_sha256
        {
            return Err(Error::new(
                "durable provider lookup-table account bytes changed",
            ));
        }
        let table = ObservedAccount {
            observation: Observation {
                slot: self.observation_slot,
                unix_timestamp: self.observation_unix_timestamp,
                finality: Finality::Finalized,
            },
            key: pubkey(&self.lookup_table)?,
            owner: pubkey(&self.lookup_table_account.owner)?,
            lamports: self.lookup_table_account.lamports,
            executable: self.lookup_table_account.executable,
            data: table_data,
        };
        if table_account_digest(&table) != self.lookup_table_account_sha256 {
            return Err(Error::new(
                "durable provider lookup-table account digest changed",
            ));
        }
        let canonical = dclutch_versioned_message_operator::compile_v0_message(
            payer,
            &bounded,
            blockhash,
            table.observation,
            std::slice::from_ref(&table),
        )
        .map_err(|error| Error::new(format!("canonical durable provider message: {error:?}")))?;
        if canonical.message != message
            || canonical.wire_bytes != self.compiled_wire_bytes
            || canonical.loaded_addresses != self.compiled_loaded_addresses
            || canonical.lookup_tables != vec![table.key]
        {
            return Err(Error::new(
                "durable provider message differs from the canonical transfer/action compilation",
            ));
        }
        let lookup_addresses = self
            .lookup_table_addresses
            .iter()
            .map(|key| pubkey(key))
            .collect::<Result<Vec<_>>>()?;
        let mut hasher = Sha256::new();
        for address in &lookup_addresses {
            hasher.update(address.as_ref());
        }
        if hex(&hasher.finalize()) != self.lookup_table_addresses_sha256 {
            return Err(Error::new("durable provider lookup address digest changed"));
        }
        let lookup = &v0.address_table_lookups[0];
        let resolve = |index: &u8| {
            lookup_addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| Error::new("durable provider lookup index exceeded table"))
        };
        let writable = lookup
            .writable_indexes
            .iter()
            .map(resolve)
            .collect::<Result<Vec<_>>>()?;
        let readonly = lookup
            .readonly_indexes
            .iter()
            .map(resolve)
            .collect::<Result<Vec<_>>>()?;
        let mut resolved = v0.account_keys.clone();
        resolved.extend(writable.iter().copied());
        resolved.extend(readonly.iter().copied());
        if writable.iter().map(ToString::to_string).collect::<Vec<_>>() != self.loaded_writable
            || readonly.iter().map(ToString::to_string).collect::<Vec<_>>() != self.loaded_readonly
            || resolved.iter().map(ToString::to_string).collect::<Vec<_>>()
                != self.resolved_account_keys
            || self.pre_balances.len() != resolved.len()
            || self.pre_accounts.len() != resolved.len()
        {
            return Err(Error::new(
                "durable provider loaded/resolved key or balance vector changed",
            ));
        }
        for (index, key) in self.resolved_account_keys.iter().enumerate() {
            let state = self
                .pre_accounts
                .get(key)
                .ok_or_else(|| Error::new("durable provider prestate omitted resolved account"))?;
            pubkey(&state.owner)?;
            let data = BASE64
                .decode(&state.data_base64)
                .map_err(|error| Error::new(format!("provider prestate base64: {error}")))?;
            if BASE64.encode(&data) != state.data_base64
                || hex(&Sha256::digest(&data)) != state.data_sha256
                || self.pre_balances.get(index).copied() != Some(state.lamports)
            {
                return Err(Error::new("durable provider account prestate changed"));
            }
        }
        match self.phase {
            DurablePhaseV1::Planned
                if self.signed_transaction_base64.is_none()
                    && self.signed_transaction_sha256.is_none()
                    && self.expected_signature.is_none()
                    && self.finalized.is_none() => {}
            DurablePhaseV1::SignedNotSubmitted | DurablePhaseV1::Submitted
                if self.signed_transaction_base64.is_some()
                    && self.signed_transaction_sha256.is_some()
                    && self.expected_signature.is_some()
                    && self.finalized.is_none() =>
            {
                let packet = BASE64
                    .decode(
                        self.signed_transaction_base64
                            .as_deref()
                            .ok_or_else(|| Error::new("signed provider plan omitted packet"))?,
                    )
                    .map_err(|error| {
                        Error::new(format!("signed provider packet base64: {error}"))
                    })?;
                if BASE64.encode(&packet)
                    != self
                        .signed_transaction_base64
                        .as_deref()
                        .unwrap_or_default()
                    || hex(&Sha256::digest(&packet))
                        != self
                            .signed_transaction_sha256
                            .as_deref()
                            .unwrap_or_default()
                {
                    return Err(Error::new("signed provider packet digest changed"));
                }
                let transaction: VersionedTransaction = bincode::deserialize(&packet)
                    .map_err(|error| Error::new(format!("signed provider packet: {error}")))?;
                transaction.verify_and_hash_message().map_err(|error| {
                    Error::new(format!("signed provider packet signature: {error}"))
                })?;
                if transaction.message != message
                    || transaction
                        .signatures
                        .first()
                        .map(ToString::to_string)
                        .as_deref()
                        != self.expected_signature.as_deref()
                {
                    return Err(Error::new(
                        "signed provider packet message or payer signature changed",
                    ));
                }
            }
            DurablePhaseV1::Finalized
                if self.signed_transaction_base64.is_some()
                    && self.signed_transaction_sha256.is_some()
                    && self.expected_signature.is_some()
                    && self.finalized.is_some() =>
            {
                let packet = BASE64
                    .decode(
                        self.signed_transaction_base64
                            .as_deref()
                            .ok_or_else(|| Error::new("signed provider plan omitted packet"))?,
                    )
                    .map_err(|error| {
                        Error::new(format!("signed provider packet base64: {error}"))
                    })?;
                if BASE64.encode(&packet)
                    != self
                        .signed_transaction_base64
                        .as_deref()
                        .unwrap_or_default()
                    || hex(&Sha256::digest(&packet))
                        != self
                            .signed_transaction_sha256
                            .as_deref()
                            .unwrap_or_default()
                {
                    return Err(Error::new("signed provider packet digest changed"));
                }
                let transaction: VersionedTransaction = bincode::deserialize(&packet)
                    .map_err(|error| Error::new(format!("signed provider packet: {error}")))?;
                transaction.verify_and_hash_message().map_err(|error| {
                    Error::new(format!("signed provider packet signature: {error}"))
                })?;
                if transaction.message != message
                    || transaction
                        .signatures
                        .first()
                        .map(ToString::to_string)
                        .as_deref()
                        != self.expected_signature.as_deref()
                {
                    return Err(Error::new(
                        "signed provider packet message or payer signature changed",
                    ));
                }
            }
            _ => {
                return Err(Error::new(
                    "durable provider phase/evidence shape is noncanonical",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransferPlanV1 {
    destination: String,
    lamports: u64,
    purpose: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArithmeticPlanV1 {
    lifecycle_rent_lamports: u64,
    update_rent_lamports: u64,
    certificate_rent_lamports: u64,
    provider_fee_lamports: u64,
    expected_reclaim_update_lamports: u64,
    expected_reclaim_lifecycle_lamports: u64,
    expected_reclaim_total_lamports: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DurableAccountStateV1 {
    owner: String,
    lamports: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StagePlanV1 {
    stage: StageV1,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    action: InstructionPlanV1,
    transaction_instructions: Vec<InstructionPlanV1>,
    lookup_table: String,
    lookup_table_account: DurableAccountStateV1,
    lookup_table_account_sha256: String,
    compiled_wire_bytes: usize,
    compiled_loaded_addresses: usize,
    required_signers: Vec<String>,
    transfers: Vec<TransferPlanV1>,
    arithmetic: ArithmeticPlanV1,
    mutation_account: String,
    phase: DurablePhaseV1,
    recent_blockhash: String,
    last_valid_block_height: u64,
    exact_fee_lamports: u64,
    message_base64: String,
    message_sha256: String,
    lookup_table_addresses: Vec<String>,
    lookup_table_addresses_sha256: String,
    loaded_writable: Vec<String>,
    loaded_readonly: Vec<String>,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    pre_accounts: BTreeMap<String, DurableAccountStateV1>,
    signed_transaction_base64: Option<String>,
    signed_transaction_sha256: Option<String>,
    expected_signature: Option<String>,
    finalized: Option<StageReceiptV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageReceiptV1 {
    stage: StageV1,
    signature: String,
    slot: u64,
    fee_lamports: u64,
    transfer_fee_lamports: u64,
    arithmetic: ArithmeticPlanV1,
    signed_transaction_sha256: String,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    return_data_base64: String,
    return_data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointV1 {
    format: String,
    input_sha256: String,
    stage_plan: Option<StagePlanV1>,
    receipts: Vec<StageReceiptV1>,
    verified_terminal: bool,
}

struct PreparedStageV1 {
    plan: StagePlanV1,
}

#[derive(Default)]
struct ProducerCommandArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    plan: Option<PathBuf>,
    campaign_evidence: Option<PathBuf>,
    pyth_facts: Option<PathBuf>,
    producer_checkpoint: Option<PathBuf>,
    output: Option<PathBuf>,
}

impl ProducerCommandArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        let mut mode = false;
        while let Some(argument) = iterator.next() {
            if argument == "--produce-input" {
                if mode {
                    return Err(Error::new("--produce-input may be supplied only once"));
                }
                mode = true;
                continue;
            }
            let value = iterator
                .next()
                .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
            match argument.as_str() {
                "--rpc-url" => set_once(&mut parsed.rpc_url, value, "--rpc-url")?,
                flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG => set_once(
                    &mut parsed.acknowledgment,
                    value,
                    DEVNET_ACKNOWLEDGMENT_FLAG,
                )?,
                "--plan" => set_once(&mut parsed.plan, PathBuf::from(value), "--plan")?,
                "--campaign-evidence" => set_once(
                    &mut parsed.campaign_evidence,
                    PathBuf::from(value),
                    "--campaign-evidence",
                )?,
                "--pyth-facts" => {
                    set_once(&mut parsed.pyth_facts, PathBuf::from(value), "--pyth-facts")?
                }
                "--producer-checkpoint" => set_once(
                    &mut parsed.producer_checkpoint,
                    PathBuf::from(value),
                    "--producer-checkpoint",
                )?,
                "--output" => set_once(&mut parsed.output, PathBuf::from(value), "--output")?,
                _ => {
                    return Err(Error::new(format!(
                        "unknown flagship producer argument: {argument}"
                    )));
                }
            }
        }
        if !mode {
            return Err(Error::new("flagship producer requires --produce-input"));
        }
        Ok(parsed)
    }
}

#[derive(Default)]
struct TableProvisionArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    producer_checkpoint: Option<PathBuf>,
    table_journal: Option<PathBuf>,
    authority_keypair: Option<PathBuf>,
    execute: bool,
}

impl TableProvisionArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        let mut mode = false;
        while let Some(argument) = iterator.next() {
            match argument.as_str() {
                "--provision-tables" => {
                    if mode {
                        return Err(Error::new("--provision-tables may be supplied only once"));
                    }
                    mode = true;
                }
                "--execute" => {
                    if parsed.execute {
                        return Err(Error::new("--execute may be supplied only once"));
                    }
                    parsed.execute = true;
                }
                _ => {
                    let value = iterator
                        .next()
                        .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
                    match argument.as_str() {
                        "--rpc-url" => set_once(&mut parsed.rpc_url, value, "--rpc-url")?,
                        flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG => set_once(
                            &mut parsed.acknowledgment,
                            value,
                            DEVNET_ACKNOWLEDGMENT_FLAG,
                        )?,
                        "--producer-checkpoint" => set_once(
                            &mut parsed.producer_checkpoint,
                            PathBuf::from(value),
                            "--producer-checkpoint",
                        )?,
                        "--table-journal" => set_once(
                            &mut parsed.table_journal,
                            PathBuf::from(value),
                            "--table-journal",
                        )?,
                        "--authority-keypair" => set_once(
                            &mut parsed.authority_keypair,
                            PathBuf::from(value),
                            "--authority-keypair",
                        )?,
                        _ => {
                            return Err(Error::new(format!(
                                "unknown flagship table provision argument: {argument}"
                            )));
                        }
                    }
                }
            }
        }
        if !mode {
            return Err(Error::new(
                "flagship table provisioner requires --provision-tables",
            ));
        }
        if !parsed.execute && parsed.authority_keypair.is_some() {
            return Err(Error::new(
                "authority keypair is refused in read-only table preflight; add it only with --execute",
            ));
        }
        Ok(parsed)
    }
}

fn campaign_account(evidence: &CampaignMarketEvidenceV1, label: &str) -> Result<Pubkey> {
    let row = evidence
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("completed campaign omitted {label}")))?;
    nonzero_pubkey(&row.address, label)
}

fn authenticate_campaign_account(
    evidence: &CampaignMarketEvidenceV1,
    label: &str,
    expected: Pubkey,
    snapshot: &FinalizedSnapshotV1,
) -> Result<()> {
    let row = evidence
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("completed campaign omitted {label}")))?;
    if nonzero_pubkey(&row.address, label)? != expected {
        return Err(Error::new(format!(
            "completed campaign substituted {label} address"
        )));
    }
    let account = snapshot.account(expected, label)?;
    let actual = account_evidence(expected, account);
    if row.owner != actual.owner
        || row.lamports != actual.lamports
        || row.executable != actual.executable
        || row.data_len != actual.data_len
        || row.data_sha256 != actual.data_sha256
        || row.account_sha256 != actual.account_sha256
    {
        return Err(Error::new(format!(
            "completed campaign {label} evidence differs from the current finalized account"
        )));
    }
    Ok(())
}

fn admitted_campaign_resolver(
    plan: &SuccessorPlan,
    evidence: &CampaignMarketEvidenceV1,
    snapshot: &FinalizedSnapshotV1,
    market: Pubkey,
    market_state: &CoreState,
) -> Result<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    let claims = nonzero_pubkey(&plan.claims.program_id, "Claims program")?;
    let aggregate_key = campaign_account(evidence, "claims_aggregate")?;
    let position_key = campaign_account(evidence, "founder_position")?;
    let admission_key = campaign_account(evidence, "claims_admission")?;
    let aggregate_account = snapshot.account(aggregate_key, "Claims aggregate")?;
    let position_account = snapshot.account(position_key, "founder Position")?;
    let admission_account = snapshot.account(admission_key, "Claims admission")?;
    if aggregate_account.owner != claims
        || aggregate_account.executable
        || position_account.owner != claims
        || position_account.executable
        || admission_account.owner != claims
        || admission_account.executable
    {
        return Err(Error::new(
            "campaign resolver is not carried by current non-executable Claims state",
        ));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("founder Position: {error:?}")))?;
    let admission = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .map_err(|error| Error::new(format!("Claims admission: {error:?}")))?;
    let resolver = Pubkey::new_from_array(position.owner);
    let expected_aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(market.to_bytes())
            .map_err(|error| Error::new(format!("Claims aggregate seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let expected_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), position.owner)
            .map_err(|error| Error::new(format!("founder Position seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let expected_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate_key.to_bytes(), position.owner)
            .map_err(|error| Error::new(format!("Claims admission seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    if aggregate_key != expected_aggregate
        || position_key != expected_position
        || admission_key != expected_admission
        || position.market_account != aggregate_key.to_bytes()
        || position.basis_id != aggregate.basis_id
        || position.claim_count != aggregate.claim_count
        || aggregate.logical_market != market.to_bytes()
        || aggregate.release_set != market_state.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market_state.identity.registry_program.to_bytes()
        || aggregate.product_instance_id != market_state.identity.product_id.to_bytes()
        || aggregate.generation != market_state.identity.generation
        || admission.owner_kind() != ProtocolPositionOwnerKindV2::User
        || admission.market() != market.to_bytes()
        || admission.position_owner() != position.owner
        || admission.release_set() != market_state.identity.selected_release_set.to_bytes()
        || admission.generation() != market_state.identity.generation
        || admission.product_record_digest() != market_state.identity.product_record.to_bytes()
        || admission.semantic_basis_id() != aggregate.basis_id
        || admission.outcome_count() != aggregate.claim_count
        || admission.claims_program() != claims.to_bytes()
        || admission.trading_program()
            != nonzero_pubkey(&plan.trading.program_id, "Trading program")?.to_bytes()
        || admission.rent_credit() != market_state.rent_beneficiary.to_bytes()
        || admission.capability_descriptor() != [0; 32]
        || admission.capability_outcome() != 0
        || admission.position_lamports() != position_account.lamports
        || admission.admission_lamports() != admission_account.lamports
        || admission.position_rent_principal() > position_account.lamports
        || admission.admission_rent_principal() > admission_account.lamports
    {
        return Err(Error::new(
            "campaign resolver is not the current canonical founding Position owner for this Market",
        ));
    }
    authenticate_campaign_account(evidence, "claims_admission", admission_key, snapshot)?;
    Ok((resolver, aggregate_key, position_key, admission_key))
}

fn completed_campaign<'a>(
    evidence: &'a CampaignEvidenceV1,
    plan_sha256: &str,
) -> Result<&'a CampaignMarketEvidenceV1> {
    if evidence.schema != CAMPAIGN_FORMAT
        || evidence.plan_sha256 != plan_sha256
        || !evidence.execution.completed
    {
        return Err(Error::new(
            "campaign schema, exact plan digest, or completed execution proof refused",
        ));
    }
    let market = evidence
        .execution
        .market
        .as_ref()
        .ok_or_else(|| Error::new("completed campaign omitted Market evidence"))?;
    if market.completed.is_empty()
        || hex32(&market.founding_custody_context).is_err()
        || market.direct_selected_manifest_entry_index == u16::MAX
    {
        return Err(Error::new(
            "completed campaign Market evidence has a hostile shape",
        ));
    }
    Ok(market)
}

fn load_producer_checkpoint(
    path: &Path,
    plan_sha256: &str,
    campaign_sha256: &str,
    facts_sha256: &str,
) -> Result<Option<ProducerCheckpointV1>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::new(format!(
                "read flagship producer checkpoint {}: {error}",
                path.display()
            )));
        }
    };
    let checkpoint: ProducerCheckpointV1 = serde_json::from_slice(&bytes)?;
    if checkpoint.format != PRODUCER_CHECKPOINT_FORMAT
        || checkpoint.plan_sha256 != plan_sha256
        || checkpoint.campaign_evidence_sha256 != campaign_sha256
        || checkpoint.pyth_facts_sha256 != facts_sha256
    {
        return Err(Error::new(
            "producer checkpoint format or source digest changed; cross-market resume refused",
        ));
    }
    if checkpoint.tables.len() != 3
        || checkpoint.tables.contains_key(&StageV1::Complete)
        || checkpoint.routes.len() != 3
        || checkpoint.routes.contains_key(&StageV1::Complete)
    {
        return Err(Error::new(
            "producer checkpoint does not contain exactly the three provider tables",
        ));
    }
    Ok(Some(checkpoint))
}

fn authenticate_producer_checkpoint(checkpoint: &ProducerCheckpointV1) -> Result<SelectedInputV1> {
    if checkpoint.format != PRODUCER_CHECKPOINT_FORMAT
        || checkpoint.tables.len() != 3
        || checkpoint.tables.contains_key(&StageV1::Complete)
        || checkpoint.routes.len() != 3
        || checkpoint.routes.contains_key(&StageV1::Complete)
    {
        return Err(Error::new(
            "producer checkpoint does not contain exactly three provider table plans",
        ));
    }
    for digest in [
        &checkpoint.plan_sha256,
        &checkpoint.campaign_evidence_sha256,
        &checkpoint.pyth_facts_sha256,
    ] {
        hex32(digest)?;
    }
    let selected = SelectedInputV1::parse(&checkpoint.planned_input)?;
    if checkpoint.market != selected.account("market")?.to_string()
        || checkpoint.generation != selected.generation
        || checkpoint.payer != selected.resolver.to_string()
        || checkpoint.authority != selected.resolver.to_string()
        || checkpoint
            .flagship_input
            .as_ref()
            .is_some_and(|input| input != &checkpoint.planned_input)
    {
        return Err(Error::new(
            "producer checkpoint Market, generation, authority, or final input changed",
        ));
    }
    for stage in [StageV1::Submit, StageV1::Execute, StageV1::Reclaim] {
        let table = checkpoint
            .tables
            .get(&stage)
            .ok_or_else(|| Error::new("producer checkpoint omitted a provider table"))?;
        authenticate_lookup_table_plan(&selected, table)?;
        if pubkey(&table.lookup_table)? != selected.table(stage)? {
            return Err(Error::new(format!(
                "{} checkpoint table differs from the planned input",
                stage.label()
            )));
        }
    }
    Ok(selected)
}

fn producer_identity_sha256(checkpoint: &ProducerCheckpointV1) -> Result<String> {
    let identity = serde_json::to_vec(&(
        &checkpoint.plan_sha256,
        &checkpoint.campaign_evidence_sha256,
        &checkpoint.pyth_facts_sha256,
        &checkpoint.market,
        checkpoint.generation,
        &checkpoint.payer,
        &checkpoint.authority,
        &checkpoint.tables,
        &checkpoint.planned_input,
    ))?;
    Ok(hex(&Sha256::digest(identity)))
}

fn load_table_journal(
    path: &Path,
    producer_identity_sha256: &str,
) -> Result<TableProvisionJournalV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TableProvisionJournalV1 {
                format: TABLE_PROVISION_JOURNAL_FORMAT.to_owned(),
                producer_identity_sha256: producer_identity_sha256.to_owned(),
                phase: DurablePhaseV1::Finalized,
                intent: None,
                intent_sha256: None,
                signed_transaction_base64: None,
                signed_transaction_sha256: None,
                expected_signature: None,
                finalized: None,
                receipts: Vec::new(),
            });
        }
        Err(error) => {
            return Err(Error::new(format!(
                "read table provision journal {}: {error}",
                path.display()
            )));
        }
    };
    let journal: TableProvisionJournalV1 = serde_json::from_slice(&bytes)?;
    if journal.format != TABLE_PROVISION_JOURNAL_FORMAT
        || journal.producer_identity_sha256 != producer_identity_sha256
    {
        return Err(Error::new(
            "table journal format or immutable producer identity changed",
        ));
    }
    let intent_shape = match journal.phase {
        DurablePhaseV1::Planned => {
            journal.intent.is_some()
                && journal.intent_sha256.is_some()
                && journal.signed_transaction_base64.is_none()
                && journal.signed_transaction_sha256.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none()
        }
        DurablePhaseV1::SignedNotSubmitted | DurablePhaseV1::Submitted => {
            journal.intent.is_some()
                && journal.intent_sha256.is_some()
                && journal.signed_transaction_base64.is_some()
                && journal.signed_transaction_sha256.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_none()
        }
        DurablePhaseV1::Finalized => {
            (journal.intent.is_none()
                && journal.intent_sha256.is_none()
                && journal.signed_transaction_base64.is_none()
                && journal.signed_transaction_sha256.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none())
                || (journal.intent.is_some()
                    && journal.intent_sha256.is_some()
                    && journal.signed_transaction_base64.is_some()
                    && journal.signed_transaction_sha256.is_some()
                    && journal.expected_signature.is_some()
                    && journal.finalized.is_some())
        }
    };
    if !intent_shape {
        return Err(Error::new(
            "table journal phase/evidence shape is noncanonical",
        ));
    }
    if let Some(intent) = &journal.intent
        && journal.intent_sha256.as_deref()
            != Some(hex(&Sha256::digest(serde_json::to_vec(intent)?)).as_str())
    {
        return Err(Error::new("table journal intent digest changed"));
    }
    if journal
        .receipts
        .iter()
        .any(|receipt| receipt.stage == StageV1::Complete)
        || journal.receipts.iter().enumerate().any(|(index, receipt)| {
            journal
                .receipts
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other.signature == receipt.signature)
        })
    {
        return Err(Error::new(
            "table journal contains a complete-stage or duplicate receipt",
        ));
    }
    Ok(journal)
}

fn lookup_creation_slots(
    prior: Option<&ProducerCheckpointV1>,
    observation_slot: u64,
) -> Result<BTreeMap<StageV1, u64>> {
    if let Some(prior) = prior {
        return prior
            .tables
            .iter()
            .map(|(stage, table)| Ok((*stage, table.creation_slot)))
            .collect();
    }
    if observation_slot < 3 {
        return Err(Error::new(
            "finalized slot is too small to derive three distinct lookup tables",
        ));
    }
    Ok(BTreeMap::from([
        (StageV1::Submit, observation_slot),
        (StageV1::Execute, observation_slot - 1),
        (StageV1::Reclaim, observation_slot - 2),
    ]))
}

fn table_keys(authority: Pubkey, slots: &BTreeMap<StageV1, u64>) -> Result<LookupTablesV1> {
    let derive = |stage: StageV1| -> Result<String> {
        let slot = *slots
            .get(&stage)
            .ok_or_else(|| Error::new("producer checkpoint omitted a table creation slot"))?;
        Ok(create_lookup_table(authority, authority, slot)
            .1
            .to_string())
    };
    let tables = LookupTablesV1 {
        submit: derive(StageV1::Submit)?,
        execute: derive(StageV1::Execute)?,
        reclaim: derive(StageV1::Reclaim)?,
    };
    if tables.submit == tables.execute
        || tables.submit == tables.reclaim
        || tables.execute == tables.reclaim
    {
        return Err(Error::new(
            "three provider stages derived an aliased lookup table",
        ));
    }
    Ok(tables)
}

fn no_clobber_input(path: &Path, input: &PlanInputV1) -> Result<()> {
    match fs::read(path) {
        Ok(bytes) => {
            let existing: PlanInputV1 = serde_json::from_slice(&bytes)?;
            if existing != *input {
                return Err(Error::new(format!(
                    "refusing to overwrite different flagship input {}",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_json(path, input),
        Err(error) => Err(Error::new(format!(
            "read flagship input output {}: {error}",
            path.display()
        ))),
    }
}

fn plan_record<'a>(plan: &'a SuccessorPlan, label: &str) -> Result<(&'a str, &'a str)> {
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("current plan omitted {label}")))?;
    Ok((&pair.raw, &pair.staging))
}

fn authenticate_checked_upgrade_plan(plan: &SuccessorPlan) -> Result<()> {
    let checked = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        Error::new(
            "devnet flagship production requires a plan minted from the complete checked seven-role Upgrade set",
        )
    })?;
    if checked.schema != CHECKED_SET_PREPARE_SCHEMA
        || checked.devnet_genesis_hash != DEVNET_GENESIS_HASH
        || checked.roles.len() != 7
        || checked.source_revision.is_empty()
        || checked.solana_cli_version.is_empty()
        || checked.semantic_derivation != SEMANTIC_DERIVATION_V1
    {
        return Err(Error::new(
            "checked Upgrade set schema, devnet identity, role width, or provenance refused",
        ));
    }
    for digest in [
        &checked.journal_sha256,
        &checked.final_set_sha256,
        &checked.checked_release_gate_sha256,
        &checked.source_tree_sha256,
    ] {
        hex32(digest)?;
    }
    for ((expected_role, expected, expected_disposition), observed) in [
        (
            "registry",
            &plan.registry,
            CheckedDeploymentDispositionV1::CarryForward,
        ),
        (
            "rent",
            &plan.rent_credit,
            CheckedDeploymentDispositionV1::CarryForward,
        ),
        (
            "custody",
            &plan.custody,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "resolution",
            &plan.resolution,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "claims",
            &plan.claims,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        (
            "trading",
            &plan.trading,
            CheckedDeploymentDispositionV1::Upgrade,
        ),
        ("core", &plan.core, CheckedDeploymentDispositionV1::Upgrade),
    ]
    .into_iter()
    .zip(&checked.roles)
    {
        for digest in [
            observed.dump_sha256.as_str(),
            observed.checked_candidate_elf_sha256.as_str(),
            observed.live_elf_sha256.as_str(),
            observed.programdata_account_sha256.as_str(),
            observed.semantic_release_id.as_str(),
        ] {
            hex32(digest)?;
        }
        let tagged_fields_are_closed = match observed.disposition {
            CheckedDeploymentDispositionV1::Upgrade => {
                observed
                    .baseline_sha256
                    .as_deref()
                    .is_some_and(|digest| hex32(digest).is_ok())
                    && observed
                        .receipt_sha256
                        .as_deref()
                        .is_some_and(|digest| hex32(digest).is_ok())
                    && observed
                        .baseline_path
                        .as_deref()
                        .is_some_and(|path| !path.is_empty())
                    && observed
                        .receipt_path
                        .as_deref()
                        .is_some_and(|path| !path.is_empty())
                    && observed.artifact_release_body_hex.is_none()
                    && observed.artifact_release_id.is_none()
                    && observed.carried_programdata_base64.is_none()
            }
            CheckedDeploymentDispositionV1::CarryForward => {
                observed.baseline_path.is_none()
                    && observed.baseline_sha256.is_none()
                    && observed.receipt_path.is_none()
                    && observed.receipt_sha256.is_none()
                    && observed
                        .artifact_release_body_hex
                        .as_deref()
                        .is_some_and(|body| decode_hex(body).is_ok_and(|bytes| !bytes.is_empty()))
                    && observed
                        .artifact_release_id
                        .as_deref()
                        .is_some_and(|digest| {
                            hex32(digest).is_ok() && digest == expected.artifact_release_id
                        })
                    && observed
                        .carried_programdata_base64
                        .as_deref()
                        .is_some_and(|body| {
                            BASE64.decode(body).is_ok_and(|bytes| {
                                !bytes.is_empty() && BASE64.encode(bytes) == body
                            })
                        })
            }
        };
        if observed.role != expected_role
            || observed.disposition != expected_disposition
            || !tagged_fields_are_closed
            || observed.dump_path.is_empty()
            || observed.checked_candidate_elf_path.is_empty()
            || observed.program_id != expected.program_id
            || observed.programdata_id != expected.programdata_id
            || observed.checked_candidate_elf_sha256 != expected.checked_candidate_elf_sha256
            || observed.live_elf_sha256 != expected.live_elf_sha256
            || observed.deployment_slot != expected.deployment_slot
            || observed.programdata_account_sha256 != expected.programdata_sha256
            || observed.semantic_release_id != expected.semantic_release_id
        {
            return Err(Error::new(format!(
                "checked Upgrade role {expected_role} differs from the exact current plan pin"
            )));
        }
    }
    let carry = &checked.infrastructure_carry_forward;
    for digest in [
        carry.snapshot_sha256.as_str(),
        carry.profile_account_sha256.as_str(),
        carry.profile_body_sha256.as_str(),
    ] {
        hex32(digest)?;
    }
    let (registry_raw, registry_staging) = plan_record(plan, "registry_artifact_release")?;
    let (rent_raw, rent_staging) = plan_record(plan, "rent_artifact_release")?;
    if carry.context_slot == 0
        || nonzero_pubkey(&carry.profile_address, "carried infrastructure profile")?.to_string()
            != plan.infrastructure_profile.address
        || carry.profile_body_sha256 != plan.infrastructure_profile.body_sha256
        || carry.profile_body_hex != plan.infrastructure_profile.body_hex
        || carry.registry_raw_address != registry_raw
        || carry.registry_staging_address != registry_staging
        || carry.rent_raw_address != rent_raw
        || carry.rent_staging_address != rent_staging
    {
        return Err(Error::new(
            "checked infrastructure carry-forward differs from the exact current plan",
        ));
    }
    Ok(())
}

fn producer_selected_input(
    plan: &SuccessorPlan,
    campaign: &CampaignMarketEvidenceV1,
    facts: &ProducerPythFactsV1,
    post_update_body: &[u8],
    coherent: &FinalizedSnapshotV1,
    slots: &BTreeMap<StageV1, u64>,
) -> Result<PlanInputV1> {
    if plan.schema != PLAN_FORMAT {
        return Err(Error::new(format!("plan schema must be {PLAN_FORMAT}")));
    }
    authenticate_checked_upgrade_plan(plan)?;
    let registry_program = nonzero_pubkey(&plan.registry.program_id, "Registry program")?;
    let core_program = nonzero_pubkey(&plan.core.program_id, "Core program")?;
    let trading_program = nonzero_pubkey(&plan.trading.program_id, "Trading program")?;
    let resolution_program = nonzero_pubkey(&plan.resolution.program_id, "Resolution program")?;
    let market = campaign_account(campaign, "founding_market")?;
    let market_account = coherent.account(market, "founding Market")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("founding Market: {error:?}")))?;
    let release_set = hex32(&plan.release_set_id)?;
    if market_account.owner != core_program
        || market_account.executable
        || market_state.phase != CorePhase::Open
        || market_state.readiness != Readiness::Consumed
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.selected_release_set.to_bytes() != release_set
        || market_state.identity.registry_program.to_bytes() != registry_program.to_bytes()
    {
        return Err(Error::new(
            "completed campaign Market is not the exact current Open/Consumed plan generation",
        ));
    }
    let (resolver, claims_aggregate, resolver_position, claims_admission) =
        admitted_campaign_resolver(plan, campaign, coherent, market, &market_state)?;
    let generation = market_state.identity.generation;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let source_account = coherent.account(source_state, "Source state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("Source state: {error:?}")))?;
    if source_account.owner != resolution_program
        || source_account.executable
        || source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != market.to_bytes()
        || source.generation() != generation
        || source.material_id().to_bytes() != market_state.identity.resolution_policy.to_bytes()
    {
        return Err(Error::new(
            "Source state is not this Open Market's canonical fresh primary child",
        ));
    }
    let beneficiary = Pubkey::new_from_array(source.rent_beneficiary());
    let encoded_vaa = nonzero_pubkey(&facts.encoded_vaa, "encodedVaa")?;
    let encoded_account = coherent.account(encoded_vaa, "verified EncodedVaa")?;
    let encoded = VerifiedEncodedVaaV1::parse(&encoded_account.data)
        .map_err(|error| Error::new(format!("verified EncodedVaa: {error:?}")))?;
    let submitter = Pubkey::new_from_array(encoded.write_authority());
    let update_account = nonzero_pubkey(&facts.update_account, "updateAccount")?;
    if submitter == Pubkey::default()
        || beneficiary == Pubkey::default()
        || !is_vacant(coherent.optional(update_account))
    {
        return Err(Error::new(
            "provider authorities must be nonzero and the fresh Receiver update must be vacant",
        ));
    }
    let pyth = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    let router_program = Pubkey::new_from_array(pyth.router_program());
    let guardian_set = Pubkey::find_program_address(
        &[b"GuardianSet", &encoded.guardian_set_index().to_be_bytes()],
        &router_program,
    )
    .0;
    let window_key = campaign_account(campaign, "window_spec_record")?;
    let adapter_key = campaign_account(campaign, "pyth_adapter_config_record")?;
    let window = WindowSpecV1::decode(&coherent.account(window_key, "WindowSpec")?.data)
        .map_err(|error| Error::new(format!("WindowSpec: {error:?}")))?;
    let adapter =
        PythAdapterConfigV1::decode(&coherent.account(adapter_key, "PythAdapterConfig")?.data)
            .map_err(|error| Error::new(format!("PythAdapterConfig: {error:?}")))?;
    let price = parse_price_feed_message(post_update_body)?;
    validate_observation_fields(
        price.publish_time,
        price.feed_id,
        price.price,
        price.confidence,
        price.exponent,
        coherent.observation.unix_timestamp,
        window,
        adapter,
    )?;
    let reclaim_after_unix_seconds = coherent
        .observation
        .unix_timestamp
        .max(window.end_unix_seconds())
        .checked_add(FLAGSHIP_RECLAIM_DELAY_SECONDS_V1)
        .ok_or_else(|| Error::new("provider reclaim time overflow"))?;
    // A fresh Primary state hostile-decodes only with terminal sequence zero;
    // the first terminal decision is therefore the canonical next sequence.
    let terminal_sequence = 1_u64;
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[1],
            &terminal_sequence.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let lookup_tables = table_keys(resolver, slots)?;
    let (registry_artifact, registry_artifact_staging) =
        plan_record(plan, "registry_artifact_release")?;
    let (pyth_release, _) = plan_record(plan, "pyth_release")?;
    Ok(PlanInputV1 {
        format: INPUT_FORMAT.to_owned(),
        generation,
        release_set: plan.release_set_id.clone(),
        submitter: submitter.to_string(),
        resolver: resolver.to_string(),
        refund_recipient: beneficiary.to_string(),
        terminal_sequence,
        reclaim_after_unix_seconds,
        post_update_body_base64: BASE64.encode(post_update_body),
        accounts: AccountSelectorsV1 {
            market: market.to_string(),
            source_state: source_state.to_string(),
            source_material: campaign_account(campaign, "source_material_record")?.to_string(),
            source_spec: campaign_account(campaign, "source_spec_record")?.to_string(),
            source_provider_release: campaign_account(campaign, "provider_release_record")?
                .to_string(),
            adapter_config: adapter_key.to_string(),
            window: window_key.to_string(),
            statistic: campaign_account(campaign, "statistic_spec_record")?.to_string(),
            pyth_release: pyth_release.to_owned(),
            product: campaign_account(campaign, "product_record")?.to_string(),
            result_domain: campaign_account(campaign, "result_domain_record")?.to_string(),
            portfolio: campaign_account(campaign, "portfolio_record")?.to_string(),
            certificate: certificate.to_string(),
            activation_cache: plan.activation.clone(),
            infrastructure: plan.infrastructure_profile.address.clone(),
            registry_program: registry_program.to_string(),
            registry_programdata: plan.registry.programdata_id.clone(),
            registry_artifact: registry_artifact.to_owned(),
            registry_artifact_staging: registry_artifact_staging.to_owned(),
            core_program: core_program.to_string(),
            core_programdata: plan.core.programdata_id.clone(),
            claims_program: plan.claims.program_id.clone(),
            claims_programdata: plan.claims.programdata_id.clone(),
            claims_aggregate: claims_aggregate.to_string(),
            resolver_position: resolver_position.to_string(),
            claims_admission: claims_admission.to_string(),
            trading_program: trading_program.to_string(),
            trading_programdata: plan.trading.programdata_id.clone(),
            resolution_program: resolution_program.to_string(),
            resolution_programdata: plan.resolution.programdata_id.clone(),
            receiver_program: Pubkey::new_from_array(pyth.receiver_program()).to_string(),
            receiver_programdata: Pubkey::new_from_array(pyth.receiver_programdata()).to_string(),
            receiver_config: Pubkey::new_from_array(pyth.receiver_config()).to_string(),
            router_program: router_program.to_string(),
            router_programdata: Pubkey::new_from_array(pyth.router_programdata()).to_string(),
            guardian_set: guardian_set.to_string(),
            encoded_vaa: encoded_vaa.to_string(),
            update_account: update_account.to_string(),
        },
        lookup_tables,
    })
}

fn run_producer(arguments: Vec<String>) -> Result<()> {
    let arguments = ProducerCommandArgumentsV1::parse(arguments)?;
    let plan_path = absolute(arguments.plan, "--plan")?;
    let campaign_path = absolute(arguments.campaign_evidence, "--campaign-evidence")?;
    let facts_path = absolute(arguments.pyth_facts, "--pyth-facts")?;
    let checkpoint_path = absolute(arguments.producer_checkpoint, "--producer-checkpoint")?;
    let output_path = absolute(arguments.output, "--output")?;
    let plan_bytes = fs::read(&plan_path)?;
    let campaign_bytes = fs::read(&campaign_path)?;
    let facts_bytes = fs::read(&facts_path)?;
    let plan_sha256 = hex(&Sha256::digest(&plan_bytes));
    let campaign_sha256 = hex(&Sha256::digest(&campaign_bytes));
    let facts_sha256 = hex(&Sha256::digest(&facts_bytes));
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let campaign_envelope: CampaignEvidenceV1 = serde_json::from_slice(&campaign_bytes)?;
    let campaign = completed_campaign(&campaign_envelope, &plan_sha256)?;
    let (facts, encoded_vaa, update_account, post_update_body) =
        parse_producer_facts(&facts_bytes)?;
    let prior = load_producer_checkpoint(
        &checkpoint_path,
        &plan_sha256,
        &campaign_sha256,
        &facts_sha256,
    )?;
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    if origin.loopback_port().is_some() {
        return Err(Error::new(
            "flagship input production is devnet-only and refuses loopback origins",
        ));
    }
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let market = campaign_account(campaign, "founding_market")?;
    let window = campaign_account(campaign, "window_spec_record")?;
    let adapter = campaign_account(campaign, "pyth_adapter_config_record")?;
    let claims_aggregate = campaign_account(campaign, "claims_aggregate")?;
    let resolver_position = campaign_account(campaign, "founder_position")?;
    let claims_admission = campaign_account(campaign, "claims_admission")?;
    let first = observe_keys(
        &mut rpc,
        BTreeSet::from([market, encoded_vaa, window, adapter]),
        0,
    )?;
    let market_state = CoreState::decode(&first.account(market, "founding Market")?.data)
        .map_err(|error| Error::new(format!("founding Market: {error:?}")))?;
    let resolution_program = nonzero_pubkey(&plan.resolution.program_id, "Resolution program")?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &market_state.identity.generation.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let coherent = observe_keys(
        &mut rpc,
        BTreeSet::from([
            market,
            encoded_vaa,
            update_account,
            window,
            adapter,
            source_state,
            claims_aggregate,
            resolver_position,
            claims_admission,
        ]),
        first.observation.slot,
    )?;
    let slots = lookup_creation_slots(prior.as_ref(), coherent.observation.slot)?;
    let input = producer_selected_input(
        &plan,
        campaign,
        &facts,
        &post_update_body,
        &coherent,
        &slots,
    )?;
    let selected = SelectedInputV1::parse(&input)?;
    let snapshot = observe(
        &mut rpc,
        &selected,
        StageV1::Submit,
        coherent.observation.slot,
    )?;
    if classify(chain_facts(&selected, &snapshot)?)? != StageV1::Submit {
        return Err(Error::new(
            "flagship producer requires the canonical pre-submit Open/Primary state",
        ));
    }
    authenticate_current_deployments(&selected, &snapshot)?;
    authenticate_devnet_pyth(&selected, &snapshot, true)?;
    // The production builder is the final address/record join. Producing its
    // exact Submit instruction proves no locally-derived selector is merely a
    // plausible address.
    let submit_report = provider_submit_report(&selected, &snapshot)?;
    let posted = parse_price_feed_message(&post_update_body)?;
    let actual_window = WindowSpecV1::decode(
        &snapshot
            .account(selected.account("window")?, "WindowSpec")?
            .data,
    )
    .map_err(|error| Error::new(format!("WindowSpec: {error:?}")))?;
    let actual_adapter = PythAdapterConfigV1::decode(
        &snapshot
            .account(selected.account("adapter_config")?, "PythAdapterConfig")?
            .data,
    )
    .map_err(|error| Error::new(format!("PythAdapterConfig: {error:?}")))?;
    validate_observation_fields(
        posted.publish_time,
        posted.feed_id,
        posted.price,
        posted.confidence,
        posted.exponent,
        snapshot.observation.unix_timestamp,
        actual_window,
        actual_adapter,
    )?;
    for (label, selector) in [
        ("founding_market", "market"),
        ("source_material_record", "source_material"),
        ("source_spec_record", "source_spec"),
        ("provider_release_record", "source_provider_release"),
        ("pyth_adapter_config_record", "adapter_config"),
        ("window_spec_record", "window"),
        ("statistic_spec_record", "statistic"),
        ("product_record", "product"),
        ("result_domain_record", "result_domain"),
        ("portfolio_record", "portfolio"),
    ] {
        authenticate_campaign_account(campaign, label, selected.account(selector)?, &snapshot)?;
    }
    let mut tables = BTreeMap::new();
    let mut routes = BTreeMap::new();
    let rent = snapshot_rent(&snapshot)?;
    for stage in [StageV1::Submit, StageV1::Execute, StageV1::Reclaim] {
        let table = build_lookup_table_plan(
            &selected,
            stage,
            *slots
                .get(&stage)
                .ok_or_else(|| Error::new("missing table creation slot"))?,
            selected.resolver,
        )?;
        authenticate_lookup_table_plan(&selected, &table)?;
        let key = pubkey(&table.lookup_table)?;
        let route = route_lookup_table(
            &table,
            snapshot.optional(key),
            snapshot.observation.slot,
            &rent,
        )?;
        tables.insert(stage, table);
        routes.insert(stage, route);
    }
    if let Some(prior) = prior.as_ref()
        && (prior.market != selected.account("market")?.to_string()
            || prior.generation != selected.generation
            || prior.payer != selected.resolver.to_string()
            || prior.authority != selected.resolver.to_string()
            || prior.tables != tables
            || prior.planned_input != input)
    {
        return Err(Error::new(
            "producer checkpoint immutable Market, authority, or typed table plan changed",
        ));
    }
    let complete = routes
        .values()
        .all(|route| matches!(route, LookupTableRouteV1::Complete { .. }));
    let flagship_input = complete.then_some(input.clone());
    if complete {
        for stage in [StageV1::Submit, StageV1::Execute, StageV1::Reclaim] {
            authenticate_frozen_lookup_table(
                &selected,
                stage,
                &snapshot.observed(selected.table(stage)?, "provider lookup table")?,
                &rent,
            )?;
        }
        let submit_table = snapshot.observed(selected.table(StageV1::Submit)?, "submit table")?;
        let compiled = compile_provider_submit_v0(
            &submit_report,
            Hash::new_from_array(GEOMETRY_BLOCKHASH),
            std::slice::from_ref(&submit_table),
        )
        .map_err(|error| Error::new(format!("producer submit v0 geometry: {error:?}")))?;
        if compiled.required_signers.as_slice()
            != [selected.submitter, selected.account("update_account")?]
        {
            return Err(Error::new(
                "producer submit table changed the exact signer boundary",
            ));
        }
        no_clobber_input(&output_path, &input)?;
    } else if output_path.exists() {
        return Err(Error::new(format!(
            "flagship input {} exists before all three exact tables are frozen; refusing stale output",
            output_path.display()
        )));
    }
    let checkpoint = ProducerCheckpointV1 {
        format: PRODUCER_CHECKPOINT_FORMAT.to_owned(),
        plan_sha256,
        campaign_evidence_sha256: campaign_sha256,
        pyth_facts_sha256: facts_sha256,
        observation_slot: snapshot.observation.slot,
        observation_unix_timestamp: snapshot.observation.unix_timestamp,
        market: selected.account("market")?.to_string(),
        generation: selected.generation,
        payer: selected.resolver.to_string(),
        authority: selected.resolver.to_string(),
        tables,
        routes,
        planned_input: input.clone(),
        flagship_input,
    };
    write_json(&checkpoint_path, &checkpoint)?;
    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
    Ok(())
}

fn table_action_instruction<'a>(
    plan: &'a LookupTablePlanV1,
    action: &TableProvisionActionV1,
) -> Result<&'a InstructionPlanV1> {
    match action {
        TableProvisionActionV1::Create => Ok(&plan.create),
        TableProvisionActionV1::Extend { page_index } => plan
            .ordered_extensions
            .get(*page_index)
            .ok_or_else(|| Error::new("table provision page exceeded its durable plan")),
        TableProvisionActionV1::Freeze => Ok(&plan.freeze),
    }
}

fn route_action(route: &LookupTableRouteV1) -> Option<(TableProvisionActionV1, InstructionPlanV1)> {
    match route {
        LookupTableRouteV1::Create { instruction } => {
            Some((TableProvisionActionV1::Create, instruction.clone()))
        }
        LookupTableRouteV1::Extend {
            page_index,
            instruction,
        } => Some((
            TableProvisionActionV1::Extend {
                page_index: *page_index,
            },
            instruction.clone(),
        )),
        LookupTableRouteV1::Freeze { instruction } => {
            Some((TableProvisionActionV1::Freeze, instruction.clone()))
        }
        LookupTableRouteV1::Complete { .. } => None,
    }
}

fn next_table_provision(
    checkpoint: &ProducerCheckpointV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<Option<(StageV1, TableProvisionActionV1, InstructionPlanV1)>> {
    let mut routed = Vec::with_capacity(3);
    let rent = snapshot_rent(snapshot)?;
    for stage in [StageV1::Submit, StageV1::Execute, StageV1::Reclaim] {
        let plan = checkpoint
            .tables
            .get(&stage)
            .ok_or_else(|| Error::new("producer checkpoint omitted a table plan"))?;
        let key = pubkey(&plan.lookup_table)?;
        let route = route_lookup_table(
            plan,
            snapshot.optional(key),
            snapshot.observation.slot,
            &rent,
        )?;
        routed.push((stage, route));
    }
    Ok(select_next_table_action(&routed))
}

fn select_next_table_action(
    routed: &[(StageV1, LookupTableRouteV1)],
) -> Option<(StageV1, TableProvisionActionV1, InstructionPlanV1)> {
    // All three creates consume a recent SlotHashes entry. Provision them
    // before any extension/freeze sequence so a long first-table sequence
    // cannot strand the still-vacant tables behind an expired creation slot.
    for (stage, route) in routed {
        if let LookupTableRouteV1::Create { instruction } = route {
            return Some((*stage, TableProvisionActionV1::Create, instruction.clone()));
        }
    }
    for (stage, route) in routed {
        if let Some((action, instruction)) = route_action(route) {
            return Some((*stage, action, instruction));
        }
    }
    None
}

fn authenticate_table_creation_slot(
    rpc: &mut Rpc,
    plan: &LookupTablePlanV1,
    minimum_slot: u64,
) -> Result<()> {
    let (_, accounts) = rpc.finalized_accounts(&[sysvar::slot_hashes::ID], minimum_slot)?;
    let account = accounts
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| Error::new("finalized SlotHashes sysvar is missing"))?;
    if account.owner != sysvar::ID || account.executable {
        return Err(Error::new(
            "SlotHashes is not the canonical non-executable sysvar account",
        ));
    }
    let hashes: SlotHashes = bincode::deserialize(&account.data)
        .map_err(|error| Error::new(format!("SlotHashes sysvar: {error}")))?;
    if hashes.get(&plan.creation_slot).is_none() {
        return Err(Error::new(format!(
            "{} lookup-table creation slot {} expired before signing; produce a fresh three-table plan",
            plan.stage.label(),
            plan.creation_slot
        )));
    }
    Ok(())
}

fn provision_action_advanced(action: &TableProvisionActionV1, route: &LookupTableRouteV1) -> bool {
    match (action, route) {
        (TableProvisionActionV1::Create, LookupTableRouteV1::Extend { page_index: 0, .. })
        | (TableProvisionActionV1::Freeze, LookupTableRouteV1::Complete { .. }) => true,
        (
            TableProvisionActionV1::Extend { page_index },
            LookupTableRouteV1::Extend {
                page_index: next, ..
            },
        ) => page_index.checked_add(1) == Some(*next),
        (TableProvisionActionV1::Extend { .. }, LookupTableRouteV1::Freeze { .. }) => true,
        _ => false,
    }
}

fn latest_table_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("table blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((blockhash, last_valid))
}

fn validate_table_intent(
    checkpoint: &ProducerCheckpointV1,
    selected: &SelectedInputV1,
    intent: &TableProvisionIntentV1,
) -> Result<Message> {
    if intent.stage == StageV1::Complete
        || intent.observation_slot == 0
        || intent.lookup_table != selected.table(intent.stage)?.to_string()
    {
        return Err(Error::new(
            "table submission stage or lookup-table coordinate changed",
        ));
    }
    let plan = checkpoint
        .tables
        .get(&intent.stage)
        .ok_or_else(|| Error::new("table submission stage has no durable plan"))?;
    let expected_instruction = table_action_instruction(plan, &intent.action)?;
    if expected_instruction != &intent.instruction {
        return Err(Error::new(
            "table submission instruction differs from its exact durable plan",
        ));
    }
    let bytes = BASE64
        .decode(&intent.unsigned_message_base64)
        .map_err(|error| Error::new(format!("unsigned table message base64: {error}")))?;
    if BASE64.encode(&bytes) != intent.unsigned_message_base64
        || hex(&Sha256::digest(&bytes)) != intent.unsigned_message_sha256
    {
        return Err(Error::new("unsigned table message digest changed"));
    }
    let message: Message = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("unsigned table message: {error}")))?;
    let blockhash = intent
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("journaled table blockhash: {error}")))?;
    let instruction = intent.instruction.instruction()?;
    let bounded = bounded_instructions(std::slice::from_ref(&instruction), None)?;
    let expected_message =
        Message::new_with_blockhash(&bounded, Some(&selected.resolver), &blockhash);
    let resolved = message
        .account_keys
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if message != expected_message
        || resolved != intent.resolved_account_keys
        || intent.pre_balances.len() != resolved.len()
        || intent.pre_accounts.len() != resolved.len()
        || intent.resolved_account_keys.first() != Some(&selected.resolver.to_string())
    {
        return Err(Error::new(
            "unsigned table message, payer, blockhash, resolved keys, or balances changed",
        ));
    }
    for (index, key) in intent.resolved_account_keys.iter().enumerate() {
        let state = intent
            .pre_accounts
            .get(key)
            .ok_or_else(|| Error::new("table intent omitted resolved account prestate"))?;
        pubkey(&state.owner)?;
        let data = BASE64
            .decode(&state.data_base64)
            .map_err(|error| Error::new(format!("table prestate base64: {error}")))?;
        if BASE64.encode(&data) != state.data_base64
            || hex(&Sha256::digest(&data)) != state.data_sha256
            || intent.pre_balances.get(index).copied() != Some(state.lamports)
        {
            return Err(Error::new("table intent account prestate changed"));
        }
    }
    Ok(message)
}

fn validate_table_signed_packet(journal: &TableProvisionJournalV1) -> Result<Transaction> {
    let intent = journal
        .intent
        .as_ref()
        .ok_or_else(|| Error::new("signed table journal omitted intent"))?;
    let packet = BASE64
        .decode(
            journal
                .signed_transaction_base64
                .as_deref()
                .ok_or_else(|| Error::new("signed table journal omitted packet"))?,
        )
        .map_err(|error| Error::new(format!("signed table packet base64: {error}")))?;
    if BASE64.encode(&packet)
        != journal
            .signed_transaction_base64
            .as_deref()
            .unwrap_or_default()
        || hex(&Sha256::digest(&packet))
            != journal
                .signed_transaction_sha256
                .as_deref()
                .unwrap_or_default()
    {
        return Err(Error::new("signed table packet digest changed"));
    }
    let transaction: Transaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("signed table packet: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("signed table packet signature: {error}")))?;
    let message_bytes = BASE64
        .decode(&intent.unsigned_message_base64)
        .map_err(|error| Error::new(format!("unsigned table message base64: {error}")))?;
    let expected_message: Message = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("unsigned table message: {error}")))?;
    if transaction.message != expected_message
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != journal.expected_signature.as_deref()
    {
        return Err(Error::new(
            "signed table packet message or expected signature changed",
        ));
    }
    Ok(transaction)
}

enum TableTransactionStatusV1 {
    Pending,
    Finalized {
        slot: u64,
        fee_lamports: u64,
        post_balances: Vec<u64>,
    },
}

fn table_transaction_status(
    rpc: &mut Rpc,
    journal: &TableProvisionJournalV1,
) -> Result<TableTransactionStatusV1> {
    let intent = journal
        .intent
        .as_ref()
        .ok_or_else(|| Error::new("table poll omitted durable intent"))?;
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("table poll omitted expected signature"))?;
    let value = rpc.call(
        "getTransaction",
        &json!([signature, {
            "commitment":"finalized",
            "encoding":"base64",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if !value.is_null() {
        let transaction = value
            .get("transaction")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new("finalized table transaction omitted base64 tuple"))?;
        if transaction.len() != 2
            || transaction.first().and_then(Value::as_str)
                != journal.signed_transaction_base64.as_deref()
            || transaction.get(1).and_then(Value::as_str) != Some("base64")
        {
            return Err(Error::new(
                "finalized table transaction bytes differ from the pre-send journal",
            ));
        }
        let meta = value
            .get("meta")
            .ok_or_else(|| Error::new("finalized table transaction omitted meta"))?;
        if !meta.get("err").is_some_and(Value::is_null) {
            return Err(Error::new(
                "journaled table transaction finalized with a runtime error",
            ));
        }
        let slot = value
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("finalized table transaction omitted slot"))?;
        let fee_lamports = meta
            .get("fee")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("finalized table transaction omitted fee"))?;
        let pre_balances = meta
            .get("preBalances")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new("finalized table transaction omitted preBalances"))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| Error::new("invalid preBalance"))
            })
            .collect::<Result<Vec<_>>>()?;
        let post_balances = meta
            .get("postBalances")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new("finalized table transaction omitted postBalances"))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| Error::new("invalid postBalance"))
            })
            .collect::<Result<Vec<_>>>()?;
        if fee_lamports != intent.exact_fee_lamports
            || pre_balances != intent.pre_balances
            || post_balances.len() != intent.resolved_account_keys.len()
            || meta.get("returnData").is_some_and(|value| !value.is_null())
        {
            return Err(Error::new(
                "finalized table fee, balance vector, or returnData differed from the durable message",
            ));
        }
        return Ok(TableTransactionStatusV1::Finalized {
            slot,
            fee_lamports,
            post_balances,
        });
    }
    Ok(TableTransactionStatusV1::Pending)
}

fn submit_journaled_table_transaction(
    rpc: &mut Rpc,
    journal: &TableProvisionJournalV1,
) -> Result<()> {
    authenticate_send_boundary(journal.phase)?;
    let packet = journal
        .signed_transaction_base64
        .as_deref()
        .ok_or_else(|| Error::new("table send omitted durable signed packet"))?;
    let signature = rpc
        .call(
            "sendTransaction",
            &json!([packet, {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"confirmed",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .to_owned();
    if Some(signature.as_str()) != journal.expected_signature.as_deref() {
        return Err(Error::new(
            "RPC returned another signature for the exact journaled table bytes",
        ));
    }
    Ok(())
}

fn finish_table_submission(
    rpc: &mut Rpc,
    checkpoint: &ProducerCheckpointV1,
    selected: &SelectedInputV1,
    journal: &TableProvisionJournalV1,
    slot: u64,
    fee_lamports: u64,
    post_balances: Vec<u64>,
) -> Result<TableProvisionReceiptV1> {
    let submission = journal
        .intent
        .as_ref()
        .ok_or_else(|| Error::new("table finalization omitted durable intent"))?;
    let snapshot = observe(rpc, selected, StageV1::Submit, slot)?;
    if classify(chain_facts(selected, &snapshot)?)? != StageV1::Submit {
        return Err(Error::new(
            "Market resolution advanced while provisioning its routing tables",
        ));
    }
    let plan = checkpoint
        .tables
        .get(&submission.stage)
        .ok_or_else(|| Error::new("table receipt stage has no durable plan"))?;
    let table_key = pubkey(&plan.lookup_table)?;
    let route = route_lookup_table(
        plan,
        snapshot.optional(table_key),
        snapshot.observation.slot,
        &snapshot_rent(&snapshot)?,
    )?;
    if !provision_action_advanced(&submission.action, &route) {
        return Err(Error::new(
            "journaled table action finalized without its exact next canonical state",
        ));
    }
    let table_post_account = snapshot.account(table_key, "provisioned lookup table")?;
    authenticate_table_action_poststate(plan, submission, table_post_account, slot)?;
    let table_post = table_post_account.lamports;
    let table_index = submission
        .resolved_account_keys
        .iter()
        .position(|key| key == &table_key.to_string())
        .ok_or_else(|| Error::new("durable table message omitted lookup-table account"))?;
    let payer_post = *post_balances
        .first()
        .ok_or_else(|| Error::new("finalized table balances omitted payer"))?;
    if post_balances.get(table_index).copied() != Some(table_post) {
        return Err(Error::new(
            "finalized table balance vector differs from the exact observed table account",
        ));
    }
    let table_rent_delta = table_post
        .checked_sub(submission.table_pre_lamports)
        .ok_or_else(|| Error::new("table lamports decreased during provisioning"))?;
    let exact_debit = fee_lamports
        .checked_add(table_rent_delta)
        .ok_or_else(|| Error::new("table fee plus rent overflow"))?;
    if payer_post.checked_add(exact_debit) != Some(submission.payer_pre_lamports) {
        return Err(Error::new(
            "table authority debit differs from exact finalized fee plus table rent increase",
        ));
    }
    for (index, (pre, post)) in submission
        .pre_balances
        .iter()
        .zip(&post_balances)
        .enumerate()
    {
        if index != 0 && index != table_index && pre != post {
            return Err(Error::new(
                "readonly table-message account balance changed in finalized history",
            ));
        }
    }
    Ok(TableProvisionReceiptV1 {
        stage: submission.stage,
        action: submission.action.clone(),
        lookup_table: submission.lookup_table.clone(),
        signature: journal.expected_signature.clone().unwrap_or_default(),
        slot,
        fee_lamports,
        payer_pre_lamports: submission.payer_pre_lamports,
        payer_post_lamports: payer_post,
        table_pre_lamports: submission.table_pre_lamports,
        table_post_lamports: table_post,
        table_post_account_sha256: account_evidence(table_key, table_post_account).account_sha256,
        signed_transaction_sha256: journal
            .signed_transaction_sha256
            .clone()
            .unwrap_or_default(),
        resolved_account_keys: submission.resolved_account_keys.clone(),
        pre_balances: submission.pre_balances.clone(),
        post_balances,
        post_route: route,
    })
}

fn authenticate_table_action_poststate(
    plan: &LookupTablePlanV1,
    intent: &TableProvisionIntentV1,
    post: &RpcAccount,
    finalized_slot: u64,
) -> Result<()> {
    let table_key = pubkey(&intent.lookup_table)?;
    let pre = intent
        .pre_accounts
        .get(&intent.lookup_table)
        .ok_or_else(|| Error::new("table intent omitted exact table prestate"))?;
    let pre_data = BASE64
        .decode(&pre.data_base64)
        .map_err(|error| Error::new(format!("table prestate base64: {error}")))?;
    let post_table = decoded_lookup_table(
        table_key,
        post.owner,
        post.executable,
        &post.data,
        plan.stage.label(),
    )?;
    let expected = stable_union_addresses(&plan.stable_union)?;
    let authority = pubkey(&plan.authority)?;
    match intent.action {
        TableProvisionActionV1::Create => {
            if pre.owner != system_program::ID.to_string()
                || pre.executable
                || !pre_data.is_empty()
                || pre.lamports != 0
                || post_table.meta.authority != Some(authority)
                || post_table.meta.deactivation_slot != u64::MAX
                || post_table.meta.last_extended_slot != 0
                || post_table.meta.last_extended_slot_start_index != 0
                || !post_table.addresses.is_empty()
            {
                return Err(Error::new(
                    "finalized table Create did not produce the exact canonical vacant-table successor",
                ));
            }
        }
        TableProvisionActionV1::Extend { page_index } => {
            let pre_table = decoded_lookup_table(
                table_key,
                pubkey(&pre.owner)?,
                pre.executable,
                &pre_data,
                plan.stage.label(),
            )?;
            let page = dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
            let start = page_index
                .checked_mul(page)
                .ok_or_else(|| Error::new("table extension start overflow"))?;
            let end = start.saturating_add(page).min(expected.len());
            if pre_table.meta.authority != Some(authority)
                || pre_table.meta.deactivation_slot != u64::MAX
                || pre_table.addresses.as_ref() != &expected[..start]
                || post_table.meta.authority != Some(authority)
                || post_table.meta.deactivation_slot != u64::MAX
                || post_table.meta.last_extended_slot != finalized_slot
                || usize::from(post_table.meta.last_extended_slot_start_index) != start
                || post_table.addresses.as_ref() != &expected[..end]
            {
                return Err(Error::new(
                    "finalized table Extend differed from its exact slot-relative next prefix",
                ));
            }
        }
        TableProvisionActionV1::Freeze => {
            let pre_table = decoded_lookup_table(
                table_key,
                pubkey(&pre.owner)?,
                pre.executable,
                &pre_data,
                plan.stage.label(),
            )?;
            if pre_table.meta.authority != Some(authority)
                || pre_table.addresses.as_ref() != expected.as_slice()
                || post_table.meta.authority.is_some()
                || post_table.meta.deactivation_slot != pre_table.meta.deactivation_slot
                || post_table.meta.last_extended_slot != pre_table.meta.last_extended_slot
                || post_table.meta.last_extended_slot_start_index
                    != pre_table.meta.last_extended_slot_start_index
                || post_table.addresses != pre_table.addresses
            {
                return Err(Error::new(
                    "finalized table Freeze changed more than the exact authority field",
                ));
            }
        }
    }
    Ok(())
}

fn table_fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact table fee"))
}

fn table_message_balances(
    rpc: &mut Rpc,
    message: &Message,
    minimum_slot: u64,
) -> Result<(u64, Vec<u64>, BTreeMap<String, DurableAccountStateV1>)> {
    let (slot, accounts) = rpc.finalized_accounts(&message.account_keys, minimum_slot)?;
    let mut balances = Vec::with_capacity(accounts.len());
    let mut states = BTreeMap::new();
    for (key, account) in message.account_keys.iter().zip(accounts) {
        let account = account.unwrap_or(RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        });
        balances.push(account.lamports);
        states.insert(
            key.to_string(),
            DurableAccountStateV1 {
                owner: account.owner.to_string(),
                lamports: account.lamports,
                executable: account.executable,
                data_base64: BASE64.encode(&account.data),
                data_sha256: hex(&Sha256::digest(&account.data)),
            },
        );
    }
    Ok((slot, balances, states))
}

fn reconcile_table_attempt(
    rpc: &mut Rpc,
    path: &Path,
    checkpoint: &ProducerCheckpointV1,
    selected: &SelectedInputV1,
    journal: &mut TableProvisionJournalV1,
) -> Result<bool> {
    validate_table_signed_packet(journal)?;
    match table_transaction_status(rpc, journal)? {
        TableTransactionStatusV1::Pending => Ok(false),
        TableTransactionStatusV1::Finalized {
            slot,
            fee_lamports,
            post_balances,
        } => {
            let receipt = finish_table_submission(
                rpc,
                checkpoint,
                selected,
                journal,
                slot,
                fee_lamports,
                post_balances,
            )?;
            if let Some(prior) = &journal.finalized
                && prior != &receipt
            {
                return Err(Error::new(
                    "persisted table finalization differs from exact finalized history",
                ));
            }
            journal.phase = DurablePhaseV1::Finalized;
            journal.finalized = Some(receipt);
            write_json(path, journal)?;
            Ok(true)
        }
    }
}

fn run_table_provisioner(arguments: Vec<String>) -> Result<()> {
    let arguments = TableProvisionArgumentsV1::parse(arguments)?;
    let producer_path = absolute(arguments.producer_checkpoint, "--producer-checkpoint")?;
    let journal_path = absolute(arguments.table_journal, "--table-journal")?;
    let producer_bytes = fs::read(&producer_path).map_err(|error| {
        Error::new(format!(
            "read producer checkpoint {}: {error}",
            producer_path.display()
        ))
    })?;
    let checkpoint: ProducerCheckpointV1 = serde_json::from_slice(&producer_bytes)?;
    let selected = authenticate_producer_checkpoint(&checkpoint)?;
    let identity = producer_identity_sha256(&checkpoint)?;
    let mut journal = load_table_journal(&journal_path, &identity)?;
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    if origin.loopback_port().is_some() {
        return Err(Error::new(
            "flagship table provisioner is devnet-only and refuses loopback origins",
        ));
    }
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    if journal.intent.is_some() {
        let intent = journal.intent.as_ref().expect("checked intent");
        validate_table_intent(&checkpoint, &selected, intent)?;
        match journal.phase {
            DurablePhaseV1::SignedNotSubmitted | DurablePhaseV1::Submitted => {
                reconcile_table_attempt(
                    &mut rpc,
                    &journal_path,
                    &checkpoint,
                    &selected,
                    &mut journal,
                )?;
                println!("{}", serde_json::to_string_pretty(&journal)?);
                return Ok(());
            }
            DurablePhaseV1::Finalized => {
                reconcile_table_attempt(
                    &mut rpc,
                    &journal_path,
                    &checkpoint,
                    &selected,
                    &mut journal,
                )?;
                let receipt = journal
                    .finalized
                    .take()
                    .ok_or_else(|| Error::new("finalized table attempt omitted receipt"))?;
                journal.receipts.push(receipt);
                journal.intent = None;
                journal.intent_sha256 = None;
                journal.signed_transaction_base64 = None;
                journal.signed_transaction_sha256 = None;
                journal.expected_signature = None;
                write_json(&journal_path, &journal)?;
            }
            DurablePhaseV1::Planned if !arguments.execute => {
                println!("{}", serde_json::to_string_pretty(&journal)?);
                return Ok(());
            }
            DurablePhaseV1::Planned => {}
        }
    }
    if journal.intent.is_none() {
        let snapshot = observe(&mut rpc, &selected, StageV1::Submit, 0)?;
        if classify(chain_facts(&selected, &snapshot)?)? != StageV1::Submit {
            return Err(Error::new(
                "routing tables may be provisioned only before the flagship provider submission",
            ));
        }
        authenticate_current_deployments(&selected, &snapshot)?;
        authenticate_devnet_pyth(&selected, &snapshot, true)?;
        provider_submit_report(&selected, &snapshot)?;
        let Some((stage, action, instruction)) = next_table_provision(&checkpoint, &snapshot)?
        else {
            println!("{}", serde_json::to_string_pretty(&journal)?);
            return Ok(());
        };
        if action == TableProvisionActionV1::Create {
            authenticate_table_creation_slot(
                &mut rpc,
                checkpoint
                    .tables
                    .get(&stage)
                    .ok_or_else(|| Error::new("producer checkpoint omitted next table plan"))?,
                snapshot.observation.slot,
            )?;
        }
        let (blockhash, last_valid_block_height) = latest_table_blockhash(&mut rpc)?;
        let bounded =
            bounded_instructions(std::slice::from_ref(&instruction.instruction()?), None)?;
        let message = Message::new_with_blockhash(&bounded, Some(&selected.resolver), &blockhash);
        let message_bytes = bincode::serialize(&message)
            .map_err(|error| Error::new(format!("serialize unsigned table message: {error}")))?;
        let message_base64 = BASE64.encode(&message_bytes);
        let exact_fee_lamports = table_fee_for_message(&mut rpc, &message_base64)?;
        let (observation_slot, pre_balances, pre_accounts) =
            table_message_balances(&mut rpc, &message, snapshot.observation.slot)?;
        let table_key = selected.table(stage)?;
        let table_index = message
            .account_keys
            .iter()
            .position(|key| *key == table_key)
            .ok_or_else(|| Error::new("unsigned table message omitted table account"))?;
        let intent = TableProvisionIntentV1 {
            stage,
            action,
            lookup_table: table_key.to_string(),
            instruction,
            observation_slot,
            recent_blockhash: blockhash.to_string(),
            last_valid_block_height,
            unsigned_message_base64: message_base64,
            unsigned_message_sha256: hex(&Sha256::digest(&message_bytes)),
            exact_fee_lamports,
            resolved_account_keys: message
                .account_keys
                .iter()
                .map(ToString::to_string)
                .collect(),
            payer_pre_lamports: pre_balances[0],
            table_pre_lamports: pre_balances[table_index],
            pre_balances,
            pre_accounts,
        };
        validate_table_intent(&checkpoint, &selected, &intent)?;
        journal.phase = DurablePhaseV1::Planned;
        journal.intent_sha256 = Some(hex(&Sha256::digest(serde_json::to_vec(&intent)?)));
        journal.intent = Some(intent);
        // Complete unsigned message and exact fee/prestate are durable before key access.
        write_json(&journal_path, &journal)?;
        if !arguments.execute {
            println!("{}", serde_json::to_string_pretty(&journal)?);
            return Ok(());
        }
    }
    let intent = journal
        .intent
        .clone()
        .ok_or_else(|| Error::new("planned table intent vanished"))?;
    let message = validate_table_intent(&checkpoint, &selected, &intent)?;
    let current_height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if current_height > intent.last_valid_block_height {
        return Err(Error::new(
            "planned table blockhash expired before key access; preserve the journal and produce a new authorized attempt",
        ));
    }
    let (_, current_balances, current_accounts) =
        table_message_balances(&mut rpc, &message, intent.observation_slot)?;
    if current_balances != intent.pre_balances || current_accounts != intent.pre_accounts {
        return Err(Error::new("table message prestate changed before signing"));
    }
    if table_fee_for_message(&mut rpc, &intent.unsigned_message_base64)?
        != intent.exact_fee_lamports
    {
        return Err(Error::new(
            "table exact fee differs from the canonical durable message",
        ));
    }
    let authority = load_keypair(
        arguments.authority_keypair.as_ref(),
        "authority",
        selected.resolver,
    )?;
    let mut transaction = Transaction::new_unsigned(message);
    transaction
        .try_sign(&[&authority], transaction.message.recent_blockhash)
        .map_err(|error| Error::new(format!("sign exact durable table message: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("verify exact signed table message: {error}")))?;
    let packet = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize signed table transaction: {error}")))?;
    journal.signed_transaction_base64 = Some(BASE64.encode(&packet));
    journal.signed_transaction_sha256 = Some(hex(&Sha256::digest(&packet)));
    journal.expected_signature = Some(transaction.signatures[0].to_string());
    journal.phase = DurablePhaseV1::SignedNotSubmitted;
    validate_table_signed_packet(&journal)?;
    write_json(&journal_path, &journal)?;
    let (_, current_balances, current_accounts) =
        table_message_balances(&mut rpc, &transaction.message, intent.observation_slot)?;
    if current_balances != intent.pre_balances || current_accounts != intent.pre_accounts {
        return Err(Error::new("table message prestate changed after signing"));
    }
    // Submitted is durable before the sole send. A crash after this write is poll-only,
    // so the exact packet can never be replayed by recovery.
    journal.phase = DurablePhaseV1::Submitted;
    write_json(&journal_path, &journal)?;
    submit_journaled_table_transaction(&mut rpc, &journal)?;
    reconcile_table_attempt(
        &mut rpc,
        &journal_path,
        &checkpoint,
        &selected,
        &mut journal,
    )?;
    println!("{}", serde_json::to_string_pretty(&journal)?);
    Ok(())
}

fn provider_submit_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    let report = build_provider_submit_v3(
        &ProviderSubmitSnapshotV3 {
            market: snapshot.observed(selected.account("market")?, "Market")?,
            source_state: snapshot.observed(selected.account("source_state")?, "Source state")?,
            source_material: snapshot
                .observed(selected.account("source_material")?, "SourceMaterial")?,
            source_spec: snapshot.observed(selected.account("source_spec")?, "SourceSpec")?,
            source_provider_release: snapshot.observed(
                selected.account("source_provider_release")?,
                "ProviderRelease",
            )?,
            pyth_release: snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
            window: snapshot.observed(selected.account("window")?, "WindowSpec")?,
            encoded_vaa: snapshot
                .observed(selected.account("encoded_vaa")?, "verified EncodedVaa")?,
        },
        ProviderSubmitDeploymentV3 {
            infrastructure: selected.account("infrastructure")?,
            registry_programdata: selected.account("registry_programdata")?,
            registry_artifact: selected.account("registry_artifact")?,
            registry_artifact_staging: selected.account("registry_artifact_staging")?,
            core_programdata: selected.account("core_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
            receiver_config: selected.account("receiver_config")?,
            guardian_set: selected.account("guardian_set")?,
        },
        &ProviderSubmitIntentV3 {
            submitter: selected.submitter,
            refund_recipient: selected.refund_recipient,
            update_account: selected.account("update_account")?,
            reclaim_after_unix_seconds: selected.reclaim_after_unix_seconds,
            post_update_body: selected.post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("provider submit builder: {error:?}")))?;
    if report.lifecycle != lifecycle_address(selected)? {
        return Err(Error::new(
            "provider submit derived an unexpected lifecycle PDA",
        ));
    }
    Ok(report)
}

fn provider_execute_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    let report = build_provider_execute_v3(
        &ProviderExecuteSnapshotV3 {
            market: snapshot.observed(selected.account("market")?, "Market")?,
            source_state: snapshot.observed(selected.account("source_state")?, "Source state")?,
            lifecycle: snapshot.observed(lifecycle_address(selected)?, "provider lifecycle")?,
            update: snapshot.observed(selected.account("update_account")?, "Receiver update")?,
            source_material: snapshot
                .observed(selected.account("source_material")?, "SourceMaterial")?,
            source_spec: snapshot.observed(selected.account("source_spec")?, "SourceSpec")?,
            source_provider_release: snapshot.observed(
                selected.account("source_provider_release")?,
                "ProviderRelease",
            )?,
            adapter_config: snapshot
                .observed(selected.account("adapter_config")?, "PythAdapterConfig")?,
            window: snapshot.observed(selected.account("window")?, "WindowSpec")?,
            statistic: snapshot.observed(selected.account("statistic")?, "StatisticSpec")?,
            pyth_release: snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
            product: snapshot.observed(selected.account("product")?, "Product")?,
            result_domain: snapshot.observed(selected.account("result_domain")?, "ResultDomain")?,
            portfolio: snapshot.observed(selected.account("portfolio")?, "Portfolio")?,
        },
        ProviderExecuteDeploymentV3 {
            registry_programdata: selected.account("registry_programdata")?,
            registry_artifact: selected.account("registry_artifact")?,
            registry_artifact_staging: selected.account("registry_artifact_staging")?,
            core_programdata: selected.account("core_programdata")?,
            trading_program: selected.account("trading_program")?,
            trading_programdata: selected.account("trading_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
            receiver_config: selected.account("receiver_config")?,
        },
        &ProviderExecuteIntentV3 {
            resolver: selected.resolver,
            terminal_sequence: selected.terminal_sequence,
            post_update_body: selected.post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("provider execute builder: {error:?}")))?;
    let certificate = report
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("provider execute report lost certificate account"))?
        .pubkey;
    if certificate != selected.account("certificate")? {
        return Err(Error::new("certificate address substitution refused"));
    }
    Ok(report)
}

fn provider_reclaim_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    build_provider_reclaim_v3(
        &snapshot.observed(lifecycle_address(selected)?, "provider lifecycle")?,
        &snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
        ProviderReclaimDeploymentV3 {
            resolver: selected.resolver,
            registry_programdata: selected.account("registry_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
        },
    )
    .map_err(|error| Error::new(format!("provider reclaim builder: {error:?}")))
}

fn vacant_top_up(
    snapshot: &FinalizedSnapshotV1,
    destination: Pubkey,
    rent_minimum: u64,
    purpose: &str,
) -> Result<Option<TransferPlanV1>> {
    let current = snapshot.optional(destination);
    if !is_vacant(current) {
        return Err(Error::new(format!("{purpose} destination is not vacant")));
    }
    let lamports = current.map_or(0, |account| account.lamports);
    if lamports > rent_minimum {
        return Err(Error::new(format!(
            "{purpose} vacant account holds {lamports} lamports, above exact rent {rent_minimum}"
        )));
    }
    let missing = rent_minimum
        .checked_sub(lamports)
        .ok_or_else(|| Error::new(format!("{purpose} rent subtraction overflow")))?;
    Ok((missing != 0).then(|| TransferPlanV1 {
        destination: destination.to_string(),
        lamports: missing,
        purpose: purpose.to_owned(),
    }))
}

fn table_account_digest(account: &ObservedAccount) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account.key.as_ref());
    hasher.update(account.owner.as_ref());
    hasher.update(account.lamports.to_le_bytes());
    hasher.update([u8::from(account.executable)]);
    hasher.update(&account.data);
    hex(&hasher.finalize())
}

fn resolve_provider_v0_keys(
    message: &VersionedMessage,
    table: &ObservedAccount,
) -> Result<(Vec<Pubkey>, Vec<Pubkey>, Vec<Pubkey>, Vec<Pubkey>)> {
    let VersionedMessage::V0(message) = message else {
        return Err(Error::new("provider message was not v0"));
    };
    if message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != table.key
    {
        return Err(Error::new(
            "provider message did not use its one exact frozen table",
        ));
    }
    let decoded = decoded_lookup_table(
        table.key,
        table.owner,
        table.executable,
        &table.data,
        "provider",
    )?;
    let addresses = decoded.addresses.into_owned();
    let lookup = &message.address_table_lookups[0];
    let resolve = |index: &u8| {
        addresses
            .get(usize::from(*index))
            .copied()
            .ok_or_else(|| Error::new("provider lookup index exceeded frozen table"))
    };
    let writable = lookup
        .writable_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let readonly = lookup
        .readonly_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let mut resolved = message.account_keys.clone();
    resolved.extend(writable.iter().copied());
    resolved.extend(readonly.iter().copied());
    if resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len() {
        return Err(Error::new(
            "provider resolved key vector contained a duplicate",
        ));
    }
    Ok((addresses, writable, readonly, resolved))
}

fn versioned_message_balances(
    rpc: &mut Rpc,
    resolved: &[Pubkey],
    minimum_slot: u64,
) -> Result<(u64, Vec<u64>, BTreeMap<String, DurableAccountStateV1>)> {
    let (slot, accounts) = rpc.finalized_accounts(resolved, minimum_slot)?;
    let mut balances = Vec::with_capacity(resolved.len());
    let mut states = BTreeMap::new();
    for (key, account) in resolved.iter().zip(accounts) {
        let account = account.unwrap_or(RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        });
        balances.push(account.lamports);
        states.insert(
            key.to_string(),
            DurableAccountStateV1 {
                owner: account.owner.to_string(),
                lamports: account.lamports,
                executable: account.executable,
                data_base64: BASE64.encode(&account.data),
                data_sha256: hex(&Sha256::digest(&account.data)),
            },
        );
    }
    Ok((slot, balances, states))
}

struct CanonicalStageSemanticsV1 {
    action: Instruction,
    required_signers: Vec<Pubkey>,
    transfers: Vec<TransferPlanV1>,
    arithmetic: ArithmeticPlanV1,
    mutation_account: Pubkey,
}

fn canonical_stage_semantics(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    stage: StageV1,
    table: &ObservedAccount,
) -> Result<CanonicalStageSemanticsV1> {
    let mut arithmetic = ArithmeticPlanV1::default();
    let mut transfers = Vec::new();
    let (action, required_signers, mutation_account) = match stage {
        StageV1::Submit => {
            let report = provider_submit_report(selected, snapshot)?;
            let lifecycle_rent = rpc.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)?;
            let update_rent = rpc.minimum_balance(dclutch_pyth_svm::FULL_PRICE_UPDATE_V2_LEN)?;
            let config = ReceiverConfigV2View::parse(
                &snapshot
                    .account(selected.account("receiver_config")?, "Receiver Config")?
                    .data,
            )
            .map_err(|error| Error::new(format!("Receiver Config: {error:?}")))?;
            arithmetic.lifecycle_rent_lamports = lifecycle_rent;
            arithmetic.update_rent_lamports = update_rent;
            arithmetic.provider_fee_lamports = config.fee();
            if let Some(transfer) = vacant_top_up(
                snapshot,
                lifecycle_address(selected)?,
                lifecycle_rent,
                "provider lifecycle",
            )? {
                transfers.push(transfer);
            }
            let compiled = compile_provider_submit_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(table),
            )
            .map_err(|error| Error::new(format!("provider submit v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                selected.account("update_account")?,
            )
        }
        StageV1::Execute => {
            preflight_posted_observation(selected, snapshot)?;
            let report = provider_execute_report(selected, snapshot)?;
            let certificate_rent = rpc.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)?;
            arithmetic.certificate_rent_lamports = certificate_rent;
            let lifecycle = ProviderUpdateLifecycleV3::decode(
                &snapshot
                    .account(lifecycle_address(selected)?, "provider lifecycle")?
                    .data,
            )
            .map_err(|error| Error::new(format!("provider lifecycle: {error:?}")))?;
            arithmetic.update_rent_lamports = lifecycle.update_rent_lamports;
            arithmetic.provider_fee_lamports = lifecycle.provider_fee_lamports;
            if let Some(transfer) = vacant_top_up(
                snapshot,
                selected.account("certificate")?,
                certificate_rent,
                "terminal certificate",
            )? {
                transfers.push(transfer);
            }
            let compiled = compile_provider_execute_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(table),
            )
            .map_err(|error| Error::new(format!("provider execute v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                selected.account("source_state")?,
            )
        }
        StageV1::Reclaim => {
            let report = provider_reclaim_report(selected, snapshot)?;
            let lifecycle_account =
                snapshot.account(lifecycle_address(selected)?, "provider lifecycle")?;
            let lifecycle = ProviderUpdateLifecycleV3::decode(&lifecycle_account.data)
                .map_err(|error| Error::new(format!("provider lifecycle: {error:?}")))?;
            if snapshot.observation.unix_timestamp < lifecycle.reclaim_after_unix_seconds {
                return Err(Error::new(format!(
                    "reclaim is premature: finalized clock {} is before {}",
                    snapshot.observation.unix_timestamp, lifecycle.reclaim_after_unix_seconds
                )));
            }
            arithmetic.expected_reclaim_update_lamports = lifecycle.update_rent_lamports;
            arithmetic.expected_reclaim_lifecycle_lamports = lifecycle_account.lamports;
            arithmetic.expected_reclaim_total_lamports = lifecycle
                .update_rent_lamports
                .checked_add(lifecycle_account.lamports)
                .ok_or_else(|| Error::new("reclaim credit overflow"))?;
            let compiled = compile_provider_reclaim_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(table),
            )
            .map_err(|error| Error::new(format!("provider reclaim v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                selected.account("update_account")?,
            )
        }
        StageV1::Complete => return Err(Error::new("complete has no transaction plan")),
    };
    let expected_signers = match stage {
        StageV1::Submit => vec![selected.submitter, selected.account("update_account")?],
        StageV1::Execute | StageV1::Reclaim => vec![selected.resolver],
        StageV1::Complete => Vec::new(),
    };
    if required_signers != expected_signers {
        return Err(Error::new(format!(
            "{} compiler signer boundary changed",
            stage.label()
        )));
    }
    Ok(CanonicalStageSemanticsV1 {
        action,
        required_signers,
        transfers,
        arithmetic,
        mutation_account,
    })
}

fn authenticate_planned_stage_semantics(
    plan: &StagePlanV1,
    expected: &CanonicalStageSemanticsV1,
) -> Result<()> {
    if InstructionPlanV1::from_instruction(&expected.action)? != plan.action
        || expected
            .required_signers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            != plan.required_signers
        || expected.transfers != plan.transfers
        || expected.arithmetic != plan.arithmetic
        || expected.mutation_account.to_string() != plan.mutation_account
    {
        return Err(Error::new(
            "provider semantic owner rebuilt another action, signer, transfer, arithmetic, or mutation plan",
        ));
    }
    Ok(())
}

fn prepare_stage(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    stage: StageV1,
) -> Result<PreparedStageV1> {
    authenticate_current_deployments(selected, snapshot)?;
    match stage {
        StageV1::Submit | StageV1::Execute => {
            authenticate_devnet_pyth(selected, snapshot, true)?;
        }
        StageV1::Reclaim => {
            authenticate_devnet_pyth(selected, snapshot, false)?;
        }
        StageV1::Complete => {}
    }
    let observed_stage = classify(chain_facts(selected, snapshot)?)?;
    if observed_stage != stage {
        return Err(Error::new(format!(
            "stage changed across finalized observations: selected {}, now {}",
            stage.label(),
            observed_stage.label()
        )));
    }
    let table = snapshot.observed(selected.table(stage)?, "stage lookup table")?;
    authenticate_frozen_lookup_table(selected, stage, &table, &snapshot_rent(snapshot)?)?;
    let CanonicalStageSemanticsV1 {
        action,
        required_signers,
        transfers,
        arithmetic,
        mutation_account,
    } = canonical_stage_semantics(rpc, selected, snapshot, stage, &table)?;
    let mut instructions = Vec::with_capacity(transfers.len() + 1);
    for top_up in &transfers {
        instructions.push(transfer(
            required_signers
                .first()
                .ok_or_else(|| Error::new("stage has no fee payer"))?,
            &pubkey(&top_up.destination)?,
            top_up.lamports,
        ));
    }
    instructions.push(action.clone());
    let bounded = bounded_instructions(&instructions, None)?;
    let (blockhash, last_valid_block_height) = latest_table_blockhash(rpc)?;
    let routed = dclutch_versioned_message_operator::compile_v0_message(
        *required_signers
            .first()
            .ok_or_else(|| Error::new("stage has no fee payer"))?,
        &bounded,
        blockhash,
        snapshot.observation,
        std::slice::from_ref(&table),
    )
    .map_err(|error| {
        Error::new(format!(
            "{} atomic stage geometry: {error:?}",
            stage.label()
        ))
    })?;
    if usize::from(routed.required_signatures) != required_signers.len() {
        return Err(Error::new(format!(
            "{} atomic prepay changed the signer boundary",
            stage.label()
        )));
    }
    let (lookup_addresses, loaded_writable, loaded_readonly, resolved) =
        resolve_provider_v0_keys(&routed.message, &table)?;
    let message_bytes = bincode::serialize(&routed.message)
        .map_err(|error| Error::new(format!("serialize provider v0 message: {error}")))?;
    let message_base64 = BASE64.encode(&message_bytes);
    let exact_fee_lamports = table_fee_for_message(rpc, &message_base64)?;
    let (balance_slot, pre_balances, pre_accounts) =
        versioned_message_balances(rpc, &resolved, snapshot.observation.slot)?;
    let mut address_hasher = Sha256::new();
    for address in &lookup_addresses {
        address_hasher.update(address.as_ref());
    }
    let action_plan = InstructionPlanV1::from_instruction(&action)?;
    let transaction_instructions = bounded
        .iter()
        .map(InstructionPlanV1::from_instruction)
        .collect::<Result<Vec<_>>>()?;
    let plan = StagePlanV1 {
        stage,
        observation_slot: balance_slot,
        observation_unix_timestamp: snapshot.observation.unix_timestamp,
        action: action_plan,
        transaction_instructions,
        lookup_table: table.key.to_string(),
        lookup_table_account: DurableAccountStateV1 {
            owner: table.owner.to_string(),
            lamports: table.lamports,
            executable: table.executable,
            data_base64: BASE64.encode(&table.data),
            data_sha256: hex(&Sha256::digest(&table.data)),
        },
        lookup_table_account_sha256: table_account_digest(&table),
        compiled_wire_bytes: routed.wire_bytes,
        compiled_loaded_addresses: routed.loaded_addresses,
        required_signers: required_signers.iter().map(ToString::to_string).collect(),
        transfers,
        arithmetic,
        mutation_account: mutation_account.to_string(),
        phase: DurablePhaseV1::Planned,
        recent_blockhash: blockhash.to_string(),
        last_valid_block_height,
        exact_fee_lamports,
        message_base64,
        message_sha256: hex(&Sha256::digest(&message_bytes)),
        lookup_table_addresses: lookup_addresses.iter().map(ToString::to_string).collect(),
        lookup_table_addresses_sha256: hex(&address_hasher.finalize()),
        loaded_writable: loaded_writable.iter().map(ToString::to_string).collect(),
        loaded_readonly: loaded_readonly.iter().map(ToString::to_string).collect(),
        resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
        pre_balances,
        pre_accounts,
        signed_transaction_base64: None,
        signed_transaction_sha256: None,
        expected_signature: None,
        finalized: None,
    };
    // Persistence must round-trip the instruction before a secret can be read.
    if plan.action.instruction()? != action {
        return Err(Error::new(
            "durable stage instruction round-trip changed bytes or metas",
        ));
    }
    Ok(PreparedStageV1 { plan })
}

#[derive(Default)]
struct CommandArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    input: Option<PathBuf>,
    checkpoint: Option<PathBuf>,
    submitter_keypair: Option<PathBuf>,
    resolver_keypair: Option<PathBuf>,
    update_keypair: Option<PathBuf>,
    through: Option<StageV1>,
    execute: bool,
}

impl CommandArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            if argument == "--execute" {
                if parsed.execute {
                    return Err(Error::new("--execute may be supplied only once"));
                }
                parsed.execute = true;
                continue;
            }
            let value = iterator
                .next()
                .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
            match argument.as_str() {
                "--rpc-url" => set_once(&mut parsed.rpc_url, value, "--rpc-url")?,
                flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG => set_once(
                    &mut parsed.acknowledgment,
                    value,
                    DEVNET_ACKNOWLEDGMENT_FLAG,
                )?,
                "--input" => set_once(&mut parsed.input, PathBuf::from(value), "--input")?,
                "--checkpoint" => {
                    set_once(&mut parsed.checkpoint, PathBuf::from(value), "--checkpoint")?
                }
                "--submitter-keypair" => set_once(
                    &mut parsed.submitter_keypair,
                    PathBuf::from(value),
                    "--submitter-keypair",
                )?,
                "--resolver-keypair" => set_once(
                    &mut parsed.resolver_keypair,
                    PathBuf::from(value),
                    "--resolver-keypair",
                )?,
                "--update-keypair" => set_once(
                    &mut parsed.update_keypair,
                    PathBuf::from(value),
                    "--update-keypair",
                )?,
                "--through" => {
                    if parsed.through.replace(StageV1::parse(&value)?).is_some() {
                        return Err(Error::new("--through may be supplied only once"));
                    }
                }
                _ => return Err(Error::new(format!("unknown flagship argument: {argument}"))),
            }
        }
        if !parsed.execute
            && (parsed.submitter_keypair.is_some()
                || parsed.resolver_keypair.is_some()
                || parsed.update_keypair.is_some())
        {
            return Err(Error::new(
                "keypair paths are refused in read-only preflight; add them only with --execute",
            ));
        }
        Ok(parsed)
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!("{label} may be supplied only once")));
    }
    Ok(())
}

fn absolute(path: Option<PathBuf>, label: &str) -> Result<PathBuf> {
    let path = path.ok_or_else(|| Error::new(format!("{label} is required")))?;
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap flagship-resolution-v1 --produce-input \
     --rpc-url URL --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON \
     --campaign-evidence ABSOLUTE_JSON --pyth-facts ABSOLUTE_JSON \
     --producer-checkpoint ABSOLUTE_JSON --output ABSOLUTE_JSON\n\n  \
     dclutch-local-successor-bootstrap flagship-resolution-v1 --provision-tables \
     --rpc-url URL --i-mean-devnet DEVNET_GENESIS \
     --producer-checkpoint ABSOLUTE_JSON --table-journal ABSOLUTE_JSON \
     [--execute --authority-keypair ABSOLUTE_JSON]\n\n  \
     dclutch-local-successor-bootstrap flagship-resolution-v1 --rpc-url URL \
     --i-mean-devnet DEVNET_GENESIS --input ABSOLUTE_JSON \
     --checkpoint ABSOLUTE_JSON [--through submit|execute|reclaim|complete] \
     [--execute --submitter-keypair ABSOLUTE_JSON --resolver-keypair ABSOLUTE_JSON \
     --update-keypair ABSOLUTE_JSON]\n\nThe producer is key-free, read-only, and devnet-only. It \
     emits durable create/ordered-extend/freeze plans for the three exact typed tables, and \
     writes the flagship input only after a fresh finalized snapshot proves all three are frozen \
     and exact. The table provisioner defaults to key-free preflight and executes exactly one \
     journaled create, ordered extension, or freeze per invocation. The resolution executor \
     defaults to key-free finalized preflight. \
     With --execute, each next chain-derived stage is durably written before the minimum \
     necessary key file is opened; no signer bytes or key paths enter the checkpoint."
}

struct ProviderFinalizedTransactionV1 {
    slot: u64,
    fee_lamports: u64,
    post_balances: Vec<u64>,
    return_data_base64: String,
}

fn provider_transaction_status(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
) -> Result<Option<ProviderFinalizedTransactionV1>> {
    plan.validate()?;
    let signature = plan
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("provider poll omitted expected signature"))?;
    let value = rpc.call(
        "getTransaction",
        &json!([signature, {"commitment":"finalized","encoding":"base64","maxSupportedTransactionVersion":0}]),
    )?;
    if value.is_null() {
        return Ok(None);
    }
    let transaction = value
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized provider transaction omitted base64 tuple"))?;
    if transaction.len() != 2
        || transaction.first().and_then(Value::as_str) != plan.signed_transaction_base64.as_deref()
        || transaction.get(1).and_then(Value::as_str) != Some("base64")
    {
        return Err(Error::new(
            "finalized provider packet differs from durable packet",
        ));
    }
    let meta = value
        .get("meta")
        .ok_or_else(|| Error::new("finalized provider transaction omitted meta"))?;
    if !meta.get("err").is_some_and(Value::is_null) {
        return Err(Error::new("provider packet finalized with a runtime error"));
    }
    let fee_lamports = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized provider transaction omitted fee"))?;
    let parse_balances = |label: &str| -> Result<Vec<u64>> {
        meta.get(label)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("provider transaction omitted {label}")))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| Error::new(format!("invalid provider {label}")))
            })
            .collect()
    };
    let pre_balances = parse_balances("preBalances")?;
    let post_balances = parse_balances("postBalances")?;
    let loaded = meta
        .get("loadedAddresses")
        .ok_or_else(|| Error::new("provider transaction omitted loadedAddresses"))?;
    let parse_addresses = |label: &str| -> Result<Vec<String>> {
        loaded
            .get(label)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("loadedAddresses omitted {label}")))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| Error::new(format!("invalid loaded {label} address")))
            })
            .collect()
    };
    let return_data = meta
        .get("returnData")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("provider transaction omitted returnData"))?;
    let tuple = return_data
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("provider returnData omitted base64 tuple"))?;
    let return_data_base64 = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("provider returnData omitted bytes"))?
        .to_owned();
    let return_bytes = BASE64
        .decode(&return_data_base64)
        .map_err(|error| Error::new(format!("provider returnData base64: {error}")))?;
    if tuple.len() != 2
        || tuple.get(1).and_then(Value::as_str) != Some("base64")
        || return_data.get("programId").and_then(Value::as_str)
            != Some(selected.account("resolution_program")?.to_string().as_str())
        || BASE64.encode(return_bytes) != return_data_base64
        || fee_lamports != plan.exact_fee_lamports
        || pre_balances != plan.pre_balances
        || post_balances.len() != plan.resolved_account_keys.len()
        || parse_addresses("writable")? != plan.loaded_writable
        || parse_addresses("readonly")? != plan.loaded_readonly
    {
        return Err(Error::new(
            "provider finalized fee, balances, loaded addresses, or returnData changed",
        ));
    }
    Ok(Some(ProviderFinalizedTransactionV1 {
        slot: value
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("finalized provider transaction omitted slot"))?,
        fee_lamports,
        post_balances,
        return_data_base64,
    }))
}

fn authenticate_provider_prestate(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
) -> Result<()> {
    let snapshot = observe(rpc, selected, plan.stage, plan.observation_slot)?;
    if classify(chain_facts(selected, &snapshot)?)? != plan.stage {
        return Err(Error::new(
            "provider stage changed before signing or sending",
        ));
    }
    authenticate_current_deployments(selected, &snapshot)?;
    authenticate_devnet_pyth(selected, &snapshot, plan.stage != StageV1::Reclaim)?;
    let table = snapshot.observed(selected.table(plan.stage)?, "provider lookup table")?;
    authenticate_frozen_lookup_table(selected, plan.stage, &table, &snapshot_rent(&snapshot)?)?;
    let current_table_state = DurableAccountStateV1 {
        owner: table.owner.to_string(),
        lamports: table.lamports,
        executable: table.executable,
        data_base64: BASE64.encode(&table.data),
        data_sha256: hex(&Sha256::digest(&table.data)),
    };
    if current_table_state != plan.lookup_table_account
        || table_account_digest(&table) != plan.lookup_table_account_sha256
    {
        return Err(Error::new(
            "frozen provider lookup table changed after planning",
        ));
    }
    let expected = canonical_stage_semantics(rpc, selected, &snapshot, plan.stage, &table)?;
    authenticate_planned_stage_semantics(plan, &expected)?;
    let resolved = plan
        .resolved_account_keys
        .iter()
        .map(|key| pubkey(key))
        .collect::<Result<Vec<_>>>()?;
    let (_, balances, states) = versioned_message_balances(rpc, &resolved, plan.observation_slot)?;
    if balances != plan.pre_balances {
        return Err(Error::new(
            "provider full resolved prebalance vector changed",
        ));
    }
    if states != plan.pre_accounts {
        return Err(Error::new(
            "provider full resolved account prestate changed",
        ));
    }
    if table_fee_for_message(rpc, &plan.message_base64)? != plan.exact_fee_lamports {
        return Err(Error::new(
            "provider exact fee differs from the canonical durable message",
        ));
    }
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height > plan.last_valid_block_height {
        return Err(Error::new(
            "durable provider blockhash expired before key access",
        ));
    }
    Ok(())
}

fn durable_pre_account(plan: &StagePlanV1, key: Pubkey) -> Result<ObservedAccount> {
    let state = plan
        .pre_accounts
        .get(&key.to_string())
        .ok_or_else(|| Error::new("durable provider prestate omitted required writable"))?;
    let data = BASE64
        .decode(&state.data_base64)
        .map_err(|error| Error::new(format!("provider prestate base64: {error}")))?;
    if hex(&Sha256::digest(&data)) != state.data_sha256 {
        return Err(Error::new("provider prestate data digest changed"));
    }
    Ok(ObservedAccount {
        observation: Observation {
            slot: plan.observation_slot,
            unix_timestamp: plan.observation_unix_timestamp,
            finality: Finality::Finalized,
        },
        key,
        owner: pubkey(&state.owner)?,
        lamports: state.lamports,
        executable: state.executable,
        data,
    })
}

fn authenticate_provider_finalized_projection(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
    post: &FinalizedSnapshotV1,
    finalized_slot: u64,
    fee_lamports: u64,
    return_data: &[u8],
) -> Result<()> {
    let instruction = plan.action.instruction()?;
    let key = |index: usize| -> Result<Pubkey> {
        instruction
            .accounts
            .get(index)
            .map(|meta| meta.pubkey)
            .ok_or_else(|| Error::new("provider projection account index exceeded instruction"))
    };
    let resolution = selected.account("resolution_program")?;
    let rent = snapshot_rent(post)?;
    match plan.stage {
        StageV1::Submit => {
            let before0 = durable_pre_account(plan, key(0)?)?;
            let before1 = durable_pre_account(plan, key(1)?)?;
            let before2 = durable_pre_account(plan, key(2)?)?;
            let before34 = durable_pre_account(plan, key(34)?)?;
            let after0 = post.observed_or_vacant(key(0)?);
            let after1 = post.observed_or_vacant(key(1)?);
            let after2 = post.observed_or_vacant(key(2)?);
            let after34 = post.observed_or_vacant(key(34)?);
            let lifecycle_top_up_lamports = plan
                .transfers
                .iter()
                .filter(|transfer| {
                    transfer.destination == key(2).map(|key| key.to_string()).unwrap_or_default()
                })
                .try_fold(0_u64, |sum, transfer| {
                    sum.checked_add(transfer.lamports)
                        .ok_or_else(|| Error::new("lifecycle top-up overflow"))
                })?;
            project_finalized_provider_submit_v3(ProviderSubmitFinalizedInputV3 {
                instruction: &instruction,
                return_data_program: resolution,
                return_data,
                finalized_slot,
                transaction_fee_lamports: fee_lamports,
                lifecycle_top_up_lamports,
                expected_provider_fee_lamports: plan.arithmetic.provider_fee_lamports,
                rent: &rent,
                writable: ProviderSubmitWritableAccountsV3 {
                    submitter_before: &before0,
                    update_before: &before1,
                    lifecycle_before: &before2,
                    treasury_before: &before34,
                    submitter_after: &after0,
                    update_after: &after1,
                    lifecycle_after: &after2,
                    treasury_after: &after34,
                },
            })
            .map_err(|error| {
                Error::new(format!("finalized provider submit projection: {error:?}"))
            })?;
        }
        StageV1::Execute => {
            let before2 = durable_pre_account(plan, key(2)?)?;
            let before3 = durable_pre_account(plan, key(3)?)?;
            let before4 = durable_pre_account(plan, key(4)?)?;
            let before37 = durable_pre_account(plan, key(37)?)?;
            let after2 = post.observed_or_vacant(key(2)?);
            let after3 = post.observed_or_vacant(key(3)?);
            let after4 = post.observed_or_vacant(key(4)?);
            let after37 = post.observed_or_vacant(key(37)?);
            let source_material = durable_pre_account(plan, selected.account("source_material")?)?;
            let result_domain = durable_pre_account(plan, selected.account("result_domain")?)?;
            let update = durable_pre_account(plan, selected.account("update_account")?)?;
            let certificate_top_up_lamports = plan
                .transfers
                .iter()
                .filter(|transfer| {
                    transfer.destination == key(3).map(|key| key.to_string()).unwrap_or_default()
                })
                .try_fold(0_u64, |sum, transfer| {
                    sum.checked_add(transfer.lamports)
                        .ok_or_else(|| Error::new("certificate top-up overflow"))
                })?;
            project_finalized_provider_execute_v3(ProviderExecuteFinalizedInputV3 {
                instruction: &instruction,
                return_data_program: resolution,
                return_data,
                finalized_slot,
                execution_unix_timestamp: rpc.block_time(finalized_slot)?,
                rent: &rent,
                certificate_top_up_lamports,
                source_material: &source_material,
                result_domain: &result_domain,
                update: &update,
                writable: ProviderExecuteWritableAccountsV3 {
                    source_before: &before2,
                    certificate_before: &before3,
                    market_before: &before4,
                    lifecycle_before: &before37,
                    source_after: &after2,
                    certificate_after: &after3,
                    market_after: &after4,
                    lifecycle_after: &after37,
                },
            })
            .map_err(|error| {
                Error::new(format!("finalized provider execute projection: {error:?}"))
            })?;
        }
        StageV1::Reclaim => {
            let before1 = durable_pre_account(plan, key(1)?)?;
            let before2 = durable_pre_account(plan, key(2)?)?;
            let before3 = durable_pre_account(plan, key(3)?)?;
            let before4 = durable_pre_account(plan, key(4)?)?;
            let after1 = post.observed_or_vacant(key(1)?);
            let after2 = post.observed_or_vacant(key(2)?);
            let after3 = post.observed_or_vacant(key(3)?);
            let after4 = post.observed_or_vacant(key(4)?);
            let certificate = durable_pre_account(plan, key(5)?)?;
            project_finalized_provider_reclaim_v3(ProviderReclaimFinalizedInputV3 {
                instruction: &instruction,
                return_data_program: resolution,
                return_data,
                finalized_slot,
                execution_unix_timestamp: rpc.block_time(finalized_slot)?,
                rent: &rent,
                certificate: &certificate,
                writable: ProviderReclaimWritableAccountsV3 {
                    lifecycle_before: &before1,
                    update_before: &before2,
                    authority_before: &before3,
                    refund_before: &before4,
                    lifecycle_after: &after1,
                    update_after: &after2,
                    authority_after: &after3,
                    refund_after: &after4,
                },
            })
            .map_err(|error| {
                Error::new(format!("finalized provider reclaim projection: {error:?}"))
            })?;
        }
        StageV1::Complete => return Err(Error::new("complete has no provider projection")),
    }
    Ok(())
}

fn finish_provider_stage(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
    finalized: ProviderFinalizedTransactionV1,
) -> Result<StageReceiptV1> {
    let post = observe(rpc, selected, plan.stage, finalized.slot)?;
    let return_bytes = BASE64
        .decode(&finalized.return_data_base64)
        .map_err(|error| Error::new(format!("provider returnData base64: {error}")))?;
    authenticate_provider_finalized_projection(
        rpc,
        selected,
        plan,
        &post,
        finalized.slot,
        finalized.fee_lamports,
        &return_bytes,
    )?;
    let expected = match plan.stage {
        StageV1::Submit => StageV1::Execute,
        StageV1::Execute => StageV1::Reclaim,
        StageV1::Reclaim => StageV1::Complete,
        StageV1::Complete => return Err(Error::new("complete has no provider finalization")),
    };
    if classify(chain_facts(selected, &post)?)? != expected {
        return Err(Error::new(
            "provider packet did not produce its exact next stage",
        ));
    }
    if matches!(plan.stage, StageV1::Execute | StageV1::Reclaim) {
        authenticate_current_deployments(selected, &post)?;
        authenticate_devnet_pyth(selected, &post, plan.stage == StageV1::Execute)?;
        verify_terminal(selected, &post)?;
    }
    let payer_pre = plan.pre_balances[0];
    let payer_post = finalized.post_balances[0];
    let top_ups = plan.transfers.iter().try_fold(0_u64, |sum, transfer| {
        sum.checked_add(transfer.lamports)
            .ok_or_else(|| Error::new("provider top-up sum overflow"))
    })?;
    if plan.stage == StageV1::Reclaim {
        let refund_index = plan
            .resolved_account_keys
            .iter()
            .position(|key| key == &selected.refund_recipient.to_string())
            .ok_or_else(|| Error::new("reclaim omitted refund recipient"))?;
        if payer_post.checked_add(finalized.fee_lamports) != Some(payer_pre)
            || finalized.post_balances[refund_index]
                != plan.pre_balances[refund_index]
                    .checked_add(plan.arithmetic.expected_reclaim_total_lamports)
                    .ok_or_else(|| Error::new("reclaim refund overflow"))?
        {
            return Err(Error::new("reclaim finalized balance vector changed"));
        }
    } else {
        let non_fee = match plan.stage {
            StageV1::Submit => top_ups
                .checked_add(plan.arithmetic.update_rent_lamports)
                .and_then(|value| value.checked_add(plan.arithmetic.provider_fee_lamports))
                .ok_or_else(|| Error::new("submit balance arithmetic overflow"))?,
            StageV1::Execute => top_ups,
            _ => 0,
        };
        if payer_post
            .checked_add(non_fee)
            .and_then(|value| value.checked_add(finalized.fee_lamports))
            != Some(payer_pre)
        {
            return Err(Error::new(
                "provider payer finalized balance vector changed",
            ));
        }
    }
    Ok(StageReceiptV1 {
        stage: plan.stage,
        signature: plan.expected_signature.clone().unwrap_or_default(),
        slot: finalized.slot,
        fee_lamports: finalized.fee_lamports,
        transfer_fee_lamports: 0,
        arithmetic: plan.arithmetic.clone(),
        signed_transaction_sha256: plan.signed_transaction_sha256.clone().unwrap_or_default(),
        resolved_account_keys: plan.resolved_account_keys.clone(),
        pre_balances: plan.pre_balances.clone(),
        post_balances: finalized.post_balances,
        return_data_base64: finalized.return_data_base64,
        return_data_sha256: hex(&Sha256::digest(&return_bytes)),
    })
}

fn sign_provider_plan(
    plan: &mut StagePlanV1,
    payer: &Keypair,
    update: Option<&Keypair>,
) -> Result<()> {
    let message_bytes = BASE64
        .decode(&plan.message_base64)
        .map_err(|error| Error::new(format!("provider message base64: {error}")))?;
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("provider v0 message: {error}")))?;
    let mut signers: Vec<&dyn Signer> = vec![payer];
    if let Some(update) = update {
        signers.push(update);
    }
    let transaction = VersionedTransaction::try_new(message, &signers)
        .map_err(|error| Error::new(format!("sign exact provider message: {error}")))?;
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("verify exact provider signed packet: {error}")))?;
    let packet = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize provider packet: {error}")))?;
    plan.signed_transaction_base64 = Some(BASE64.encode(&packet));
    plan.signed_transaction_sha256 = Some(hex(&Sha256::digest(&packet)));
    plan.expected_signature = transaction.signatures.first().map(ToString::to_string);
    plan.phase = DurablePhaseV1::SignedNotSubmitted;
    plan.validate()
}

fn send_provider_packet_once(rpc: &mut Rpc, plan: &StagePlanV1) -> Result<()> {
    authenticate_send_boundary(plan.phase)?;
    let returned = rpc
        .call(
            "sendTransaction",
            &json!([plan.signed_transaction_base64, {"encoding":"base64","skipPreflight":false,"preflightCommitment":"finalized","maxRetries":0}]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("provider sendTransaction omitted signature"))?
        .to_owned();
    if Some(returned.as_str()) != plan.expected_signature.as_deref() {
        return Err(Error::new("provider RPC returned another packet signature"));
    }
    Ok(())
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    if arguments
        .iter()
        .any(|argument| argument == "--produce-input")
    {
        return run_producer(arguments);
    }
    if arguments
        .iter()
        .any(|argument| argument == "--provision-tables")
    {
        return run_table_provisioner(arguments);
    }
    let arguments = CommandArgumentsV1::parse(arguments)?;
    let input_path = absolute(arguments.input.clone(), "--input")?;
    let checkpoint_path = absolute(arguments.checkpoint.clone(), "--checkpoint")?;
    let input_bytes = fs::read(&input_path).map_err(|error| {
        Error::new(format!(
            "read flagship input {}: {error}",
            input_path.display()
        ))
    })?;
    let input: PlanInputV1 = serde_json::from_slice(&input_bytes)?;
    let selected = SelectedInputV1::parse(&input)?;
    let input_sha256 = hex(&Sha256::digest(&input_bytes));
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    if origin.loopback_port().is_some() {
        return Err(Error::new(
            "flagship-resolution-v1 is the devnet exterior and refuses loopback origins",
        ));
    }
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    let mut checkpoint = load_checkpoint(&checkpoint_path, &input_sha256)?;
    let through = arguments.through.unwrap_or(StageV1::Complete);
    loop {
        if let Some(plan) = checkpoint.stage_plan.as_mut() {
            plan.validate()?;
            match plan.phase {
                DurablePhaseV1::Finalized => {
                    let observed = provider_transaction_status(&mut rpc, &selected, plan)?
                        .ok_or_else(|| Error::new("persisted provider finalization disappeared"))?;
                    let receipt = finish_provider_stage(&mut rpc, &selected, plan, observed)?;
                    if plan.finalized.as_ref() != Some(&receipt) {
                        return Err(Error::new(
                            "persisted provider receipt differs from exact finalized history",
                        ));
                    }
                    let stage = plan.stage;
                    checkpoint.receipts.push(receipt);
                    checkpoint.stage_plan = None;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    if stage >= through {
                        println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                        return Ok(());
                    }
                    continue;
                }
                DurablePhaseV1::SignedNotSubmitted | DurablePhaseV1::Submitted => {
                    if let Some(finalized) = provider_transaction_status(&mut rpc, &selected, plan)?
                    {
                        let receipt = finish_provider_stage(&mut rpc, &selected, plan, finalized)?;
                        plan.phase = DurablePhaseV1::Finalized;
                        plan.finalized = Some(receipt);
                        write_checkpoint(&checkpoint_path, &checkpoint)?;
                    }
                    // Both ambiguous signed phases are permanently poll-only.
                    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    return Ok(());
                }
                DurablePhaseV1::Planned if !arguments.execute => {
                    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    return Ok(());
                }
                DurablePhaseV1::Planned => {
                    authenticate_provider_prestate(&mut rpc, &selected, plan)?;
                    let stage = plan.stage;
                    let (payer, update) = load_stage_signers(&selected, stage, &arguments)?;
                    sign_provider_plan(plan, &payer, update.as_ref())?;
                    // Signed packet and local transaction ID are durable before the sole send.
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    let plan = checkpoint
                        .stage_plan
                        .as_ref()
                        .ok_or_else(|| Error::new("signed provider plan disappeared"))?;
                    authenticate_provider_prestate(&mut rpc, &selected, plan)?;
                    checkpoint
                        .stage_plan
                        .as_mut()
                        .ok_or_else(|| Error::new("submitted provider plan disappeared"))?
                        .phase = DurablePhaseV1::Submitted;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    let plan = checkpoint
                        .stage_plan
                        .as_ref()
                        .ok_or_else(|| Error::new("submitted provider plan disappeared"))?;
                    send_provider_packet_once(&mut rpc, plan)?;
                    if let Some(finalized) = provider_transaction_status(
                        &mut rpc,
                        &selected,
                        checkpoint.stage_plan.as_ref().expect("submitted plan"),
                    )? {
                        let plan = checkpoint.stage_plan.as_ref().expect("submitted plan");
                        let receipt = finish_provider_stage(&mut rpc, &selected, plan, finalized)?;
                        let plan = checkpoint.stage_plan.as_mut().expect("submitted plan");
                        plan.phase = DurablePhaseV1::Finalized;
                        plan.finalized = Some(receipt);
                        write_checkpoint(&checkpoint_path, &checkpoint)?;
                    }
                    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    return Ok(());
                }
            }
        }
        let initial = observe(&mut rpc, &selected, StageV1::Submit, 0)?;
        let stage = classify(chain_facts(&selected, &initial)?)?;
        let snapshot = if stage == StageV1::Submit {
            initial
        } else {
            observe(&mut rpc, &selected, stage, initial.observation.slot)?
        };
        if stage == StageV1::Complete {
            authenticate_current_deployments(&selected, &snapshot)?;
            authenticate_devnet_pyth(&selected, &snapshot, false)?;
            verify_terminal(&selected, &snapshot)?;
            checkpoint.verified_terminal = true;
            write_checkpoint(&checkpoint_path, &checkpoint)?;
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        if stage > through {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        let prepared = prepare_stage(&mut rpc, &selected, &snapshot, stage)?;
        checkpoint.stage_plan = Some(prepared.plan);
        // Complete real v0 message, exact fee, ALT resolution, and prebalances are durable first.
        write_checkpoint(&checkpoint_path, &checkpoint)?;
        if !arguments.execute {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
    }
}

fn load_checkpoint(path: &Path, input_sha256: &str) -> Result<CheckpointV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CheckpointV1 {
                format: CHECKPOINT_FORMAT.to_owned(),
                input_sha256: input_sha256.to_owned(),
                stage_plan: None,
                receipts: Vec::new(),
                verified_terminal: false,
            });
        }
        Err(error) => {
            return Err(Error::new(format!(
                "read checkpoint {}: {error}",
                path.display()
            )));
        }
    };
    let checkpoint: CheckpointV1 = serde_json::from_slice(&bytes)?;
    if checkpoint.format != CHECKPOINT_FORMAT || checkpoint.input_sha256 != input_sha256 {
        return Err(Error::new(
            "checkpoint format or input digest differs; cross-market resume refused",
        ));
    }
    if let Some(plan) = &checkpoint.stage_plan {
        plan.validate()?;
    }
    if checkpoint
        .receipts
        .windows(2)
        .any(|pair| pair[0].stage >= pair[1].stage || pair[0].slot > pair[1].slot)
    {
        return Err(Error::new(
            "checkpoint receipts are duplicated, out of order, or cross-run substituted",
        ));
    }
    Ok(checkpoint)
}

fn write_checkpoint(path: &Path, checkpoint: &CheckpointV1) -> Result<()> {
    write_json(path, checkpoint)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("JSON output path has no parent"))?;
    if !parent.is_dir() {
        return Err(Error::new("JSON output parent directory does not exist"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("JSON output filename is not UTF-8"))?;
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(&temporary).map_err(|error| {
        Error::new(format!(
            "create JSON output temporary {}: {error}",
            temporary.display()
        ))
    })?;
    use std::io::Write as _;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically install JSON output {}: {error}",
            path.display()
        ))
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            Error::new(format!(
                "fsync JSON output parent {}: {error}",
                parent.display()
            ))
        })?;
    Ok(())
}

fn load_keypair(path: Option<&PathBuf>, label: &str, expected: Pubkey) -> Result<Keypair> {
    let path = path.ok_or_else(|| Error::new(format!("--{label}-keypair is required")))?;
    let seed = read_keypair_file(path, label)?;
    let keypair = Keypair::new_from_array(seed);
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair public key {} differs from authenticated input {expected}",
            keypair.pubkey()
        )));
    }
    Ok(keypair)
}

fn load_stage_signers(
    selected: &SelectedInputV1,
    stage: StageV1,
    arguments: &CommandArgumentsV1,
) -> Result<(Keypair, Option<Keypair>)> {
    match stage {
        StageV1::Submit => Ok((
            load_keypair(
                arguments.submitter_keypair.as_ref(),
                "submitter",
                selected.submitter,
            )?,
            Some(load_keypair(
                arguments.update_keypair.as_ref(),
                "update",
                selected.account("update_account")?,
            )?),
        )),
        StageV1::Execute | StageV1::Reclaim => Ok((
            load_keypair(
                arguments.resolver_keypair.as_ref(),
                "resolver",
                selected.resolver,
            )?,
            None,
        )),
        StageV1::Complete => Err(Error::new("complete has no executable stage")),
    }
}

fn verify_terminal(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<()> {
    let market_key = selected.account("market")?;
    let market = CoreState::decode(&snapshot.account(market_key, "Terminal Market")?.data)
        .map_err(|error| Error::new(format!("Terminal Market: {error:?}")))?;
    let certificate_key = selected.account("certificate")?;
    let certificate_account = snapshot.account(certificate_key, "terminal certificate")?;
    let certificate = ResolutionCertificateV2::decode(&certificate_account.data)
        .map_err(|error| Error::new(format!("terminal certificate: {error:?}")))?;
    let source_key = selected.account("source_state")?;
    let source_account = snapshot.account(source_key, "terminal Source state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("terminal Source state: {error:?}")))?;
    let source_terminal = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("terminal Source projection: {error:?}")))?;
    let resolution = selected.account("resolution_program")?;
    let expected_source = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market_key.as_ref(),
            &selected.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let expected_certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_key.as_ref(),
            &[1],
            &selected.terminal_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source_material =
        snapshot.account(selected.account("source_material")?, "SourceMaterial")?;
    let provider_release = snapshot.account(
        selected.account("source_provider_release")?,
        "ProviderRelease",
    )?;
    let product = snapshot.account(selected.account("product")?, "Product")?;
    if market.phase != CorePhase::Terminal
        || market.readiness != Readiness::Consumed
        || market.identity.market_id.to_bytes() != market_key.to_bytes()
        || market.identity.generation != selected.generation
        || market.identity.selected_release_set.to_bytes() != selected.release_set
        || market.terminal_receipt.map(|value| value.to_bytes()) != Some(certificate_key.to_bytes())
        || certificate.market != market_key.to_bytes()
        || certificate.generation != selected.generation
        || certificate.receipt_account != certificate_key.to_bytes()
        || certificate.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate.selector != market.terminal_winner
        || certificate.route != hash(&provider_release.data).to_bytes()
        || certificate.source_material != market.identity.resolution_policy.to_bytes()
        || certificate.product_record_digest != market.identity.product_record.to_bytes()
        || certificate.provider_evidence != source_terminal.resolution_evidence_id().to_bytes()
        || certificate.funding_allocation != [0; 32]
        || certificate.attempt_index != 0
        || certificate.result_denominator != 1
        || certificate.observed_at == 0
        || source.phase() != SourceResolutionPhaseV1::Resolved
        || source.market() != market_key.to_bytes()
        || source.generation() != selected.generation
        || source_terminal.route() != SourceResolutionRouteV1::Primary
        || source_terminal.selector() != certificate.selector
        || source_terminal.terminal_sequence() != selected.terminal_sequence
        || source_key != expected_source
        || certificate_key != expected_certificate
        || source_account.owner != resolution
        || source_account.executable
        || certificate_account.owner != resolution
        || certificate_account.executable
        || certificate_account.lamports
            != snapshot_rent(snapshot)?.minimum_balance(certificate_account.data.len())
        || hash(&source_material.data).to_bytes() != market.identity.resolution_policy.to_bytes()
        || hash(&product.data).to_bytes() != market.identity.product_record.to_bytes()
    {
        return Err(Error::new(
            "finalized Core Terminal, receipt, winner, Source, Market, generation, or release join refused",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn base58_encode(bytes: &[u8]) -> Result<String> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    if bytes.is_empty() {
        return Ok(String::new());
    }
    let zeroes = bytes.iter().take_while(|byte| **byte == 0).count();
    let capacity = bytes
        .len()
        .checked_mul(138)
        .and_then(|value| value.checked_div(100))
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| Error::new("base58 capacity overflow"))?;
    let mut digits = vec![0_u8; capacity];
    let mut length = 0_usize;
    for &byte in bytes {
        let mut carry = u32::from(byte);
        for digit in digits.iter_mut().take(length).rev() {
            let value = u32::from(*digit).saturating_mul(256).saturating_add(carry);
            *digit = u8::try_from(value % 58)
                .map_err(|_| Error::new("base58 digit conversion refused"))?;
            carry = value / 58;
        }
        while carry != 0 {
            if let Some(digit) = digits.get_mut(length) {
                *digit = u8::try_from(carry % 58)
                    .map_err(|_| Error::new("base58 carry conversion refused"))?;
            }
            length = length
                .checked_add(1)
                .ok_or_else(|| Error::new("base58 length overflow"))?;
            carry /= 58;
        }
    }
    let mut output = String::with_capacity(
        zeroes
            .checked_add(length)
            .ok_or_else(|| Error::new("base58 output length overflow"))?,
    );
    output.extend(std::iter::repeat_n('1', zeroes));
    for digit in digits.iter().take(length).rev() {
        output.push(char::from(ALPHABET[usize::from(*digit)]));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        market_phase: CorePhase,
        source_phase: SourceResolutionPhaseV1,
        lifecycle: SlotKindV1,
        update: SlotKindV1,
        certificate: SlotKindV1,
    ) -> ChainFactsV1 {
        ChainFactsV1 {
            market_phase,
            market_readiness: Readiness::Consumed,
            source_phase,
            lifecycle,
            update,
            certificate,
        }
    }

    #[test]
    fn fake_rpc_canonical_stage_ladder_is_exhaustive() {
        use SlotKindV1::{Consumed, Submitted, Vacant};
        assert_eq!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Vacant,
                Vacant,
                Vacant,
            ))
            .expect("submit"),
            StageV1::Submit
        );
        assert_eq!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Submitted,
                Submitted,
                Vacant,
            ))
            .expect("execute"),
            StageV1::Execute
        );
        assert!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Submitted,
            ))
            .is_err(),
            "provider execution has no finalized pre-admission intermediate"
        );
        assert_eq!(
            classify(facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Submitted,
            ))
            .expect("reclaim"),
            StageV1::Reclaim
        );
        assert_eq!(
            classify(facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Vacant,
                Vacant,
                Submitted,
            ))
            .expect("complete"),
            StageV1::Complete
        );
    }

    #[test]
    fn fake_rpc_partial_and_ambiguous_states_refuse() {
        use SlotKindV1::{Consumed, Other, Submitted, Vacant};
        for hostile in [
            facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Submitted,
                Vacant,
                Vacant,
            ),
            facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Vacant,
            ),
            facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Other,
                Submitted,
                Submitted,
            ),
            facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Vacant,
                Submitted,
                Submitted,
            ),
        ] {
            assert!(classify(hostile).is_err());
        }
    }

    #[test]
    fn cli_refuses_keys_during_read_only_preflight_and_duplicate_flags() {
        assert!(
            CommandArgumentsV1::parse(vec!["--submitter-keypair".into(), "/tmp/key.json".into(),])
                .is_err()
        );
        assert!(CommandArgumentsV1::parse(vec!["--execute".into(), "--execute".into(),]).is_err());
    }

    #[test]
    fn fake_rpc_stale_wrong_feed_and_wide_confidence_refuse_before_execute() {
        let source = dclutch_source_contract::ContentId::new([1; 32]).expect("source identity");
        let schedule = dclutch_source_contract::ContentId::new([2; 32]).expect("schedule identity");
        let window = WindowSpecV1::new(
            source,
            dclutch_source_contract::WindowKind::Terminal,
            90,
            110,
            20,
            5,
            schedule,
        )
        .expect("window");
        let adapter = PythAdapterConfigV1::new([3; 32], -8, 100).expect("adapter");
        assert!(
            validate_observation_fields(100, [3; 32], 10_000, 50, -8, 105, window, adapter).is_ok()
        );
        assert!(
            validate_observation_fields(80, [3; 32], 10_000, 50, -8, 105, window, adapter).is_err()
        );
        assert!(
            validate_observation_fields(100, [4; 32], 10_000, 50, -8, 105, window, adapter)
                .is_err()
        );
        assert!(
            validate_observation_fields(100, [3; 32], 10_000, 101, -8, 105, window, adapter)
                .is_err()
        );
    }

    fn price_body(
        feed: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
        publish_time: i64,
    ) -> Vec<u8> {
        let mut message = Vec::with_capacity(PYTH_PRICE_FEED_MESSAGE_BYTES_V1);
        message.push(0);
        message.extend_from_slice(&feed);
        message.extend_from_slice(&price.to_be_bytes());
        message.extend_from_slice(&confidence.to_be_bytes());
        message.extend_from_slice(&exponent.to_be_bytes());
        message.extend_from_slice(&publish_time.to_be_bytes());
        message.extend_from_slice(&(publish_time - 1).to_be_bytes());
        message.extend_from_slice(&price.to_be_bytes());
        message.extend_from_slice(&confidence.to_be_bytes());
        assert_eq!(message.len(), PYTH_PRICE_FEED_MESSAGE_BYTES_V1);
        let mut body = Vec::new();
        body.extend_from_slice(&(message.len() as u32).to_le_bytes());
        body.extend_from_slice(&message);
        body.extend_from_slice(&0_u32.to_le_bytes());
        body.push(0);
        body
    }

    fn sample_input() -> PlanInputV1 {
        let mut next = 1_u8;
        let mut key = || {
            let value = Pubkey::new_from_array([next; 32]).to_string();
            next = next.checked_add(1).expect("test key space");
            value
        };
        let accounts = AccountSelectorsV1 {
            market: key(),
            source_state: key(),
            source_material: key(),
            source_spec: key(),
            source_provider_release: key(),
            adapter_config: key(),
            window: key(),
            statistic: key(),
            pyth_release: key(),
            product: key(),
            result_domain: key(),
            portfolio: key(),
            certificate: key(),
            activation_cache: key(),
            infrastructure: key(),
            registry_program: key(),
            registry_programdata: key(),
            registry_artifact: key(),
            registry_artifact_staging: key(),
            core_program: key(),
            core_programdata: key(),
            claims_program: key(),
            claims_programdata: key(),
            claims_aggregate: key(),
            resolver_position: key(),
            claims_admission: key(),
            trading_program: key(),
            trading_programdata: key(),
            resolution_program: key(),
            resolution_programdata: key(),
            receiver_program: key(),
            receiver_programdata: key(),
            receiver_config: key(),
            router_program: key(),
            router_programdata: key(),
            guardian_set: key(),
            encoded_vaa: key(),
            update_account: key(),
        };
        let resolver = Pubkey::new_from_array([80; 32]);
        let tables = table_keys(
            resolver,
            &BTreeMap::from([
                (StageV1::Submit, 100),
                (StageV1::Execute, 99),
                (StageV1::Reclaim, 98),
            ]),
        )
        .expect("table keys");
        PlanInputV1 {
            format: INPUT_FORMAT.into(),
            generation: 1,
            release_set: hex(&[90; 32]),
            submitter: Pubkey::new_from_array([70; 32]).to_string(),
            resolver: resolver.to_string(),
            refund_recipient: Pubkey::new_from_array([81; 32]).to_string(),
            terminal_sequence: 1,
            reclaim_after_unix_seconds: 1_000,
            post_update_body_base64: BASE64.encode(price_body([3; 32], 10, 1, -8, 100)),
            accounts,
            lookup_tables: tables,
        }
    }

    fn sample_selected() -> SelectedInputV1 {
        SelectedInputV1::parse(&sample_input()).expect("selected input")
    }

    fn table_account(
        addresses: Vec<Pubkey>,
        authority: Option<Pubkey>,
        last_extended_slot: u64,
    ) -> RpcAccount {
        use std::borrow::Cow;

        use solana_address_lookup_table_interface::state::LookupTableMeta;

        let last_extended_slot_start_index =
            expected_last_extension_start(addresses.len()).expect("extension start");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                deactivation_slot: u64::MAX,
                last_extended_slot,
                last_extended_slot_start_index,
                authority,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        let data = table.serialize_for_tests().expect("table bytes");
        RpcAccount {
            lamports: Rent::default().minimum_balance(data.len()),
            owner: lookup_table_program::ID,
            executable: false,
            rent_epoch: 0,
            data,
        }
    }

    fn sample_durable_stage_plan() -> (StagePlanV1, CanonicalStageSemanticsV1) {
        let payer = Pubkey::new_from_array([91; 32]);
        let destination = Pubkey::new_from_array([92; 32]);
        let table_key = Pubkey::new_from_array([93; 32]);
        let observation = Observation {
            slot: 111,
            unix_timestamp: 222,
            finality: Finality::Finalized,
        };
        let table_rpc = table_account(vec![destination], None, 110);
        let table = ObservedAccount {
            observation,
            key: table_key,
            owner: table_rpc.owner,
            lamports: table_rpc.lamports,
            executable: table_rpc.executable,
            data: table_rpc.data,
        };
        let action = transfer(&payer, &destination, 7);
        let bounded = bounded_instructions(std::slice::from_ref(&action), None)
            .expect("bounded fixture instructions");
        let blockhash = Hash::new_from_array([94; 32]);
        let routed = dclutch_versioned_message_operator::compile_v0_message(
            payer,
            &bounded,
            blockhash,
            observation,
            std::slice::from_ref(&table),
        )
        .expect("fixture v0 message");
        let (lookup_addresses, loaded_writable, loaded_readonly, resolved) =
            resolve_provider_v0_keys(&routed.message, &table).expect("fixture resolved keys");
        let message_bytes = bincode::serialize(&routed.message).expect("fixture message bytes");
        let empty_digest = hex(&Sha256::digest([]));
        let mut pre_balances = Vec::new();
        let mut pre_accounts = BTreeMap::new();
        for key in &resolved {
            let lamports = if *key == payer { 100_000 } else { 0 };
            pre_balances.push(lamports);
            pre_accounts.insert(
                key.to_string(),
                DurableAccountStateV1 {
                    owner: system_program::ID.to_string(),
                    lamports,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: empty_digest.clone(),
                },
            );
        }
        let mut address_hasher = Sha256::new();
        for address in &lookup_addresses {
            address_hasher.update(address.as_ref());
        }
        let table_state = DurableAccountStateV1 {
            owner: table.owner.to_string(),
            lamports: table.lamports,
            executable: table.executable,
            data_base64: BASE64.encode(&table.data),
            data_sha256: hex(&Sha256::digest(&table.data)),
        };
        let expected = CanonicalStageSemanticsV1 {
            action: action.clone(),
            required_signers: vec![payer],
            transfers: Vec::new(),
            arithmetic: ArithmeticPlanV1::default(),
            mutation_account: destination,
        };
        let plan = StagePlanV1 {
            stage: StageV1::Submit,
            observation_slot: observation.slot,
            observation_unix_timestamp: observation.unix_timestamp,
            action: InstructionPlanV1::from_instruction(&action).expect("action plan"),
            transaction_instructions: bounded
                .iter()
                .map(InstructionPlanV1::from_instruction)
                .collect::<Result<Vec<_>>>()
                .expect("instruction plans"),
            lookup_table: table.key.to_string(),
            lookup_table_account: table_state,
            lookup_table_account_sha256: table_account_digest(&table),
            compiled_wire_bytes: routed.wire_bytes,
            compiled_loaded_addresses: routed.loaded_addresses,
            required_signers: vec![payer.to_string()],
            transfers: Vec::new(),
            arithmetic: ArithmeticPlanV1::default(),
            mutation_account: destination.to_string(),
            phase: DurablePhaseV1::Planned,
            recent_blockhash: blockhash.to_string(),
            last_valid_block_height: 333,
            exact_fee_lamports: 5_000,
            message_base64: BASE64.encode(&message_bytes),
            message_sha256: hex(&Sha256::digest(&message_bytes)),
            lookup_table_addresses: lookup_addresses.iter().map(ToString::to_string).collect(),
            lookup_table_addresses_sha256: hex(&address_hasher.finalize()),
            loaded_writable: loaded_writable.iter().map(ToString::to_string).collect(),
            loaded_readonly: loaded_readonly.iter().map(ToString::to_string).collect(),
            resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
            pre_balances,
            pre_accounts,
            signed_transaction_base64: None,
            signed_transaction_sha256: None,
            expected_signature: None,
            finalized: None,
        };
        (plan, expected)
    }

    #[test]
    fn durable_provider_plan_refuses_full_message_and_semantic_plan_tampering() {
        let (plan, expected) = sample_durable_stage_plan();
        assert!(plan.validate().is_ok());
        assert!(authenticate_planned_stage_semantics(&plan, &expected).is_ok());

        let mut hostile_message = plan.clone();
        let bytes = BASE64
            .decode(&hostile_message.message_base64)
            .expect("message base64");
        let mut message: VersionedMessage =
            bincode::deserialize(&bytes).expect("versioned message");
        let VersionedMessage::V0(v0) = &mut message else {
            panic!("fixture is v0")
        };
        v0.instructions.swap(0, 1);
        let bytes = bincode::serialize(&message).expect("hostile message bytes");
        hostile_message.message_base64 = BASE64.encode(&bytes);
        hostile_message.message_sha256 = hex(&Sha256::digest(&bytes));
        assert!(hostile_message.validate().is_err());

        let mut hostile_transfer = plan.clone();
        hostile_transfer.transfers.push(TransferPlanV1 {
            destination: Pubkey::new_from_array([95; 32]).to_string(),
            lamports: 1,
            purpose: "hostile same-payer transfer".into(),
        });
        assert!(authenticate_planned_stage_semantics(&hostile_transfer, &expected).is_err());

        let mut hostile_arithmetic = plan;
        hostile_arithmetic.arithmetic.provider_fee_lamports = 1;
        assert!(authenticate_planned_stage_semantics(&hostile_arithmetic, &expected).is_err());
    }

    #[test]
    fn producer_price_message_is_exact_big_endian_and_shape_bound() {
        let body = price_body([3; 32], -12_345, 99, -8, 1_787_439_872);
        let parsed = parse_price_feed_message(&body).expect("price message");
        assert_eq!(parsed.feed_id, [3; 32]);
        assert_eq!(parsed.price, -12_345);
        assert_eq!(parsed.confidence, 99);
        assert_eq!(parsed.exponent, -8);
        assert_eq!(parsed.publish_time, 1_787_439_872);
        let mut trailing = body.clone();
        trailing.push(0);
        assert!(parse_price_feed_message(&trailing).is_err());
        let mut wrong_variant = body;
        wrong_variant[4] = 1;
        assert!(parse_price_feed_message(&wrong_variant).is_err());
    }

    #[test]
    fn producer_facts_refuse_unknown_shape_and_ephemeral_alias() {
        let body = price_body([3; 32], 10, 1, -8, 100);
        let encoded = Pubkey::new_from_array([4; 32]).to_string();
        let exact = json!({
            "format": PRODUCER_FACTS_FORMAT,
            "encodedVaa": encoded,
            "updateAccount": Pubkey::new_from_array([5; 32]).to_string(),
            "postUpdateBodyBase64": BASE64.encode(&body),
        });
        assert!(parse_producer_facts(&serde_json::to_vec(&exact).expect("facts")).is_ok());
        let mut unknown = exact.clone();
        unknown["callerAssertedMarket"] = Value::String(Pubkey::new_unique().to_string());
        assert!(parse_producer_facts(&serde_json::to_vec(&unknown).expect("facts")).is_err());
        let mut alias = exact;
        alias["updateAccount"] = alias["encodedVaa"].clone();
        assert!(parse_producer_facts(&serde_json::to_vec(&alias).expect("facts")).is_err());
        assert!(
            ProducerCommandArgumentsV1::parse(vec!["--produce-input".into(), "--execute".into(),])
                .is_err()
        );
    }

    #[test]
    fn producer_and_table_cli_refuse_signer_aliases_and_preflight_keys() {
        let exact = sample_input();
        let selected = SelectedInputV1::parse(&exact).expect("exact selected input");
        assert_eq!(
            selected.account("claims_program").expect("Claims program"),
            pubkey(&exact.accounts.claims_program).expect("Claims program key")
        );
        assert_eq!(
            selected
                .account("claims_aggregate")
                .expect("Claims aggregate"),
            pubkey(&exact.accounts.claims_aggregate).expect("Claims aggregate key")
        );
        assert_eq!(
            selected
                .account("resolver_position")
                .expect("resolver Position"),
            pubkey(&exact.accounts.resolver_position).expect("resolver Position key")
        );
        let mut aliased = sample_input();
        aliased.resolver.clone_from(&aliased.submitter);
        assert!(SelectedInputV1::parse(&aliased).is_err());
        let mut aliased = sample_input();
        aliased.resolver = aliased.accounts.update_account.clone();
        assert!(SelectedInputV1::parse(&aliased).is_err());
        let mut aliased = sample_input();
        aliased.refund_recipient.clone_from(&aliased.resolver);
        assert!(SelectedInputV1::parse(&aliased).is_err());
        assert!(
            TableProvisionArgumentsV1::parse(vec![
                "--provision-tables".into(),
                "--authority-keypair".into(),
                "/tmp/refused.json".into(),
            ])
            .is_err()
        );
        assert!(
            TableProvisionArgumentsV1::parse(vec![
                "--provision-tables".into(),
                "--execute".into(),
                "--authority-keypair".into(),
                "/tmp/accepted-only-after-authentication.json".into(),
            ])
            .is_ok()
        );
    }

    #[test]
    fn completed_campaign_digest_and_account_evidence_are_substitution_bound() {
        let exact_shape: CampaignEvidenceV1 = serde_json::from_value(json!({
            "schema": CAMPAIGN_FORMAT,
            "plan_sha256": "aa".repeat(32),
            "execution": {
                "completed": true,
                "market": {
                    "completed": ["opened"],
                    "accounts": {},
                    "founding_custody_context": "01".repeat(32),
                    "direct_selected_manifest_entry_index": 1,
                }
            }
        }))
        .expect("campaign's emitted snake-case shape");
        assert!(exact_shape.execution.market.is_some());
        assert!(
            serde_json::from_value::<CampaignEvidenceV1>(json!({
                "schema": CAMPAIGN_FORMAT,
                "plan_sha256": "aa".repeat(32),
                "execution": {
                    "completed": true,
                    "market": {
                        "completed": ["opened"],
                        "accounts": {},
                        "foundingCustodyContext": "01".repeat(32),
                        "directSelectedManifestEntryIndex": 1,
                    }
                }
            }))
            .is_err()
        );
        let key = Pubkey::new_from_array([44; 32]);
        let account = RpcAccount {
            lamports: 7,
            owner: Pubkey::new_from_array([45; 32]),
            executable: false,
            rent_epoch: 9,
            data: vec![1, 2, 3],
        };
        let exact = account_evidence(key, &account);
        let row = CampaignAccountEvidenceV1 {
            address: exact.address,
            owner: exact.owner,
            lamports: exact.lamports,
            executable: exact.executable,
            data_len: exact.data_len,
            data_sha256: exact.data_sha256,
            account_sha256: exact.account_sha256,
        };
        let market = CampaignMarketEvidenceV1 {
            completed: vec!["opened".into()],
            accounts: BTreeMap::from([("founding_market".into(), row)]),
            founding_custody_context: hex(&[1; 32]),
            direct_selected_manifest_entry_index: 1,
        };
        let envelope = CampaignEvidenceV1 {
            schema: CAMPAIGN_FORMAT.into(),
            plan_sha256: "aa".repeat(32),
            execution: CampaignExecutionEnvelopeV1 {
                completed: true,
                market: Some(market),
            },
        };
        let selected_market =
            completed_campaign(&envelope, &"aa".repeat(32)).expect("campaign Market");
        assert!(completed_campaign(&envelope, &"bb".repeat(32)).is_err());
        let snapshot = FinalizedSnapshotV1 {
            observation: Observation {
                slot: 10,
                unix_timestamp: 20,
                finality: Finality::Finalized,
            },
            accounts: BTreeMap::from([(key, Some(account))]),
        };
        assert!(
            authenticate_campaign_account(selected_market, "founding_market", key, &snapshot,)
                .is_ok()
        );
        assert!(
            authenticate_campaign_account(
                selected_market,
                "founding_market",
                Pubkey::new_from_array([46; 32]),
                &snapshot,
            )
            .is_err()
        );
    }

    #[test]
    fn typed_stable_unions_are_stage_specific_ordered_and_class_bound() {
        let selected = sample_selected();
        let submit = stable_lookup_union(&selected, StageV1::Submit).expect("submit union");
        let execute = stable_lookup_union(&selected, StageV1::Execute).expect("execute union");
        let reclaim = stable_lookup_union(&selected, StageV1::Reclaim).expect("reclaim union");
        assert_ne!(submit, execute);
        assert_ne!(submit, reclaim);
        assert_ne!(execute, reclaim);
        assert_eq!(submit[0].label, "refund_recipient");
        assert_eq!(submit[0].class, StableAddressClassV1::Beneficiary);
        assert!(execute.iter().any(|row| {
            row.label == "trading_program" && row.class == StableAddressClassV1::Program
        }));
        let plan = build_lookup_table_plan(&selected, StageV1::Submit, 100, selected.resolver)
            .expect("lookup plan");
        assert!(authenticate_lookup_table_plan(&selected, &plan).is_ok());
        let mut hostile_plan = plan.clone();
        let mut substituted_class = submit.clone();
        substituted_class[0].class = StableAddressClassV1::Program;
        hostile_plan.stable_union = substituted_class;
        assert!(authenticate_lookup_table_plan(&selected, &hostile_plan).is_err());
        let mut hostile_plan = plan;
        let mut reordered = submit.clone();
        reordered.swap(0, 1);
        hostile_plan.stable_union = reordered;
        assert!(authenticate_lookup_table_plan(&selected, &hostile_plan).is_err());
    }

    #[test]
    fn lookup_route_refuses_substitution_partial_page_and_stale_frozen_table() {
        let selected = sample_selected();
        let authority = selected.resolver;
        let plan = build_lookup_table_plan(&selected, StageV1::Submit, 100, authority)
            .expect("lookup plan");
        let expected = stable_union_addresses(&plan.stable_union).expect("addresses");
        let rent = Rent::default();
        assert!(matches!(
            route_lookup_table(&plan, None, 120, &rent).expect("vacant route"),
            LookupTableRouteV1::Create { .. }
        ));
        let page = dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
        let partial = table_account(expected[..page].to_vec(), Some(authority), 110);
        assert!(matches!(
            route_lookup_table(&plan, Some(&partial), 120, &rent).expect("extend route"),
            LookupTableRouteV1::Extend { page_index: 1, .. }
        ));
        let between_pages = table_account(expected[..page + 1].to_vec(), Some(authority), 110);
        assert!(route_lookup_table(&plan, Some(&between_pages), 120, &rent).is_err());
        let mut substituted = expected.clone();
        substituted[0] = Pubkey::new_from_array([99; 32]);
        let substituted = table_account(substituted, Some(authority), 110);
        assert!(route_lookup_table(&plan, Some(&substituted), 120, &rent).is_err());
        let stale = table_account(expected.clone(), None, 120);
        assert!(route_lookup_table(&plan, Some(&stale), 120, &rent).is_err());
        let frozen = table_account(expected, None, 119);
        assert!(matches!(
            route_lookup_table(&plan, Some(&frozen), 120, &rent).expect("complete route"),
            LookupTableRouteV1::Complete { .. }
        ));
        let mut wrong_rent = frozen.clone();
        wrong_rent.lamports = wrong_rent.lamports.checked_add(1).expect("one lamport");
        assert!(route_lookup_table(&plan, Some(&wrong_rent), 120, &rent).is_err());
        wrong_rent.lamports = frozen.lamports.checked_sub(1).expect("one lamport");
        assert!(route_lookup_table(&plan, Some(&wrong_rent), 120, &rent).is_err());
        assert!(provision_action_advanced(
            &TableProvisionActionV1::Create,
            &LookupTableRouteV1::Extend {
                page_index: 0,
                instruction: plan.ordered_extensions[0].clone(),
            },
        ));
        assert!(!provision_action_advanced(
            &TableProvisionActionV1::Create,
            &LookupTableRouteV1::Freeze {
                instruction: plan.freeze.clone(),
            },
        ));
        assert!(provision_action_advanced(
            &TableProvisionActionV1::Freeze,
            &LookupTableRouteV1::Complete {
                last_extended_slot: 119,
                account_sha256: "11".repeat(32),
            },
        ));
    }

    #[test]
    fn table_provisioning_creates_all_vacant_tables_before_extending_one() {
        let selected = sample_selected();
        let submit = build_lookup_table_plan(&selected, StageV1::Submit, 100, selected.resolver)
            .expect("submit plan");
        let execute = build_lookup_table_plan(&selected, StageV1::Execute, 99, selected.resolver)
            .expect("execute plan");
        let routed = vec![
            (
                StageV1::Submit,
                LookupTableRouteV1::Extend {
                    page_index: 0,
                    instruction: submit.ordered_extensions[0].clone(),
                },
            ),
            (
                StageV1::Execute,
                LookupTableRouteV1::Create {
                    instruction: execute.create.clone(),
                },
            ),
        ];
        let (stage, action, instruction) =
            select_next_table_action(&routed).expect("next table action");
        assert_eq!(stage, StageV1::Execute);
        assert_eq!(action, TableProvisionActionV1::Create);
        assert_eq!(instruction, execute.create);
    }

    #[test]
    fn durable_send_boundary_refuses_every_phase_except_fsynced_submitted() {
        assert!(authenticate_send_boundary(DurablePhaseV1::Planned).is_err());
        assert!(authenticate_send_boundary(DurablePhaseV1::SignedNotSubmitted).is_err());
        assert!(authenticate_send_boundary(DurablePhaseV1::Submitted).is_ok());
        assert!(authenticate_send_boundary(DurablePhaseV1::Finalized).is_err());
    }

    #[test]
    fn table_signed_packet_refuses_message_signature_and_txid_substitution() {
        let signer = Keypair::new();
        let destination = Pubkey::new_from_array([88; 32]);
        let instruction = transfer(&signer.pubkey(), &destination, 1);
        let blockhash = Hash::new_from_array([77; 32]);
        let message = Message::new_with_blockhash(
            std::slice::from_ref(&instruction),
            Some(&signer.pubkey()),
            &blockhash,
        );
        let message_bytes = bincode::serialize(&message).expect("message bytes");
        let mut transaction = Transaction::new_unsigned(message.clone());
        transaction
            .try_sign(&[&signer], blockhash)
            .expect("sign fixture");
        let packet = bincode::serialize(&transaction).expect("packet bytes");
        let intent = TableProvisionIntentV1 {
            stage: StageV1::Submit,
            action: TableProvisionActionV1::Create,
            lookup_table: destination.to_string(),
            instruction: InstructionPlanV1::from_instruction(&instruction).expect("instruction"),
            observation_slot: 1,
            recent_blockhash: blockhash.to_string(),
            last_valid_block_height: 2,
            unsigned_message_base64: BASE64.encode(&message_bytes),
            unsigned_message_sha256: hex(&Sha256::digest(&message_bytes)),
            exact_fee_lamports: 5_000,
            resolved_account_keys: message
                .account_keys
                .iter()
                .map(ToString::to_string)
                .collect(),
            pre_balances: vec![10_000, 0, 1],
            pre_accounts: BTreeMap::new(),
            payer_pre_lamports: 10_000,
            table_pre_lamports: 0,
        };
        let mut journal = TableProvisionJournalV1 {
            format: TABLE_PROVISION_JOURNAL_FORMAT.to_owned(),
            producer_identity_sha256: "11".repeat(32),
            phase: DurablePhaseV1::SignedNotSubmitted,
            intent: Some(intent),
            intent_sha256: None,
            signed_transaction_base64: Some(BASE64.encode(&packet)),
            signed_transaction_sha256: Some(hex(&Sha256::digest(&packet))),
            expected_signature: Some(transaction.signatures[0].to_string()),
            finalized: None,
            receipts: Vec::new(),
        };
        assert!(validate_table_signed_packet(&journal).is_ok());
        journal.expected_signature = Some(Keypair::new().pubkey().to_string());
        assert!(validate_table_signed_packet(&journal).is_err());
        journal.expected_signature = Some(transaction.signatures[0].to_string());
        let mut hostile = transaction;
        hostile.message.recent_blockhash = Hash::new_from_array([76; 32]);
        let hostile = bincode::serialize(&hostile).expect("hostile packet");
        journal.signed_transaction_base64 = Some(BASE64.encode(&hostile));
        journal.signed_transaction_sha256 = Some(hex(&Sha256::digest(&hostile)));
        assert!(validate_table_signed_packet(&journal).is_err());
    }

    #[test]
    fn table_finalizer_binds_exact_slot_relative_create_extend_and_freeze_states() {
        let selected = sample_selected();
        let authority = selected.resolver;
        let plan = build_lookup_table_plan(&selected, StageV1::Submit, 100, authority)
            .expect("lookup plan");
        let table_key = pubkey(&plan.lookup_table).expect("table key");
        let expected = stable_union_addresses(&plan.stable_union).expect("addresses");
        let pre_state = |account: &RpcAccount| DurableAccountStateV1 {
            owner: account.owner.to_string(),
            lamports: account.lamports,
            executable: account.executable,
            data_base64: BASE64.encode(&account.data),
            data_sha256: hex(&Sha256::digest(&account.data)),
        };
        let intent = |action, pre: &RpcAccount| TableProvisionIntentV1 {
            stage: StageV1::Submit,
            action,
            lookup_table: table_key.to_string(),
            instruction: plan.create.clone(),
            observation_slot: 110,
            recent_blockhash: Hash::new_from_array([1; 32]).to_string(),
            last_valid_block_height: 111,
            unsigned_message_base64: String::new(),
            unsigned_message_sha256: String::new(),
            exact_fee_lamports: 5_000,
            resolved_account_keys: Vec::new(),
            pre_balances: Vec::new(),
            pre_accounts: BTreeMap::from([(table_key.to_string(), pre_state(pre))]),
            payer_pre_lamports: 0,
            table_pre_lamports: pre.lamports,
        };
        let vacant = RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        };
        let created = table_account(Vec::new(), Some(authority), 0);
        assert!(
            authenticate_table_action_poststate(
                &plan,
                &intent(TableProvisionActionV1::Create, &vacant),
                &created,
                110,
            )
            .is_ok()
        );
        let page = dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
        let extended_once = table_account(expected[..page].to_vec(), Some(authority), 110);
        let second_page_end = page.saturating_mul(2).min(expected.len());
        let extended_twice =
            table_account(expected[..second_page_end].to_vec(), Some(authority), 120);
        assert!(
            authenticate_table_action_poststate(
                &plan,
                &intent(
                    TableProvisionActionV1::Extend { page_index: 1 },
                    &extended_once,
                ),
                &extended_twice,
                120,
            )
            .is_ok()
        );
        let mut wrong_slot = extended_twice.clone();
        let decoded = AddressLookupTable::deserialize(&wrong_slot.data).expect("table");
        wrong_slot = table_account(decoded.addresses.into_owned(), Some(authority), 119);
        assert!(
            authenticate_table_action_poststate(
                &plan,
                &intent(
                    TableProvisionActionV1::Extend { page_index: 1 },
                    &extended_once,
                ),
                &wrong_slot,
                120,
            )
            .is_err()
        );
        let full = table_account(expected.clone(), Some(authority), 121);
        let frozen = table_account(expected, None, 121);
        assert!(
            authenticate_table_action_poststate(
                &plan,
                &intent(TableProvisionActionV1::Freeze, &full),
                &frozen,
                122,
            )
            .is_ok()
        );
    }

    #[test]
    fn founding_v5_resolver_coordinates_refuse_owner_substitution() {
        let market = Pubkey::new_from_array([61; 32]);
        let claims = Pubkey::new_from_array([62; 32]);
        let founder = Pubkey::new_from_array([63; 32]);
        let substitute = Pubkey::new_from_array([64; 32]);
        let aggregate = Pubkey::find_program_address(
            &ClaimsFoundingAggregateSeedsV5::new(market.to_bytes())
                .expect("aggregate seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
                .expect("position seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let admission = Pubkey::find_program_address(
            &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
                .expect("admission seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let substituted_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), substitute.to_bytes())
                .expect("substituted position seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let substituted_admission = Pubkey::find_program_address(
            &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), substitute.to_bytes())
                .expect("substituted admission seeds")
                .as_slices(),
            &claims,
        )
        .0;
        assert_ne!(position, substituted_position);
        assert_ne!(admission, substituted_admission);
        assert_ne!(position, admission);
    }

    #[test]
    fn base58_matches_known_system_program_spelling() {
        assert_eq!(
            base58_encode(&[0; 32]).expect("base58"),
            "11111111111111111111111111111111"
        );
        assert_eq!(base58_encode(&[0, 1]).expect("base58"), "12");
    }
}
