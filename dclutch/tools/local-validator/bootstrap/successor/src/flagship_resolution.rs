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
use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_claims_svm::{
    founding_v5::ClaimsFoundingAggregateSeedsV5,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::{
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_market_core_codec::{CoreState, Identity, Phase as CorePhase, Readiness};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    provider_transport_v3::{
        ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
        ProviderReclaimDeploymentV3, ProviderSubmitDeploymentV3, ProviderSubmitIntentV3,
        ProviderSubmitSnapshotV3, ProviderTransportReportV3, ProviderTransportTransactionErrorV3,
        build_provider_execute_v3, build_provider_reclaim_v3, build_provider_submit_v3,
        compile_provider_execute_v0, compile_provider_reclaim_v0, compile_provider_submit_v0,
        provider_execute_caller_authority_v3,
    },
    resolution_core_v3::{
        ResolutionAdmitTerminalReportV3, ResolutionAdmitTerminalSnapshotV3,
        build_resolution_admit_terminal_v3,
    },
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, GuardianSetV1, PostUpdateParamsView, ProgramDataV3View, ProgramV3View,
    PythReleaseV1, ReceiverConfigV2View, VerifiedEncodedVaaV1, devnet_release_v1,
    local_validator_release_v1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
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
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, SOURCE_SPEC_SCHEMA_ID_V1,
    STATISTIC_SPEC_SCHEMA_ID_V1, WINDOW_SPEC_SCHEMA_ID_V1,
};
use dclutch_source_contract::{
    ProviderReleaseV1, PythAdapterConfigV1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1, SourceResolutionRouteV1,
    SourceResolutionStateV2, WindowSpecV1,
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
    chaos_fault::{self, BoundaryV1},
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, ExpectedClusterV1,
    },
    evidence_refresh,
    model::{AccountEvidence, CheckedDeploymentDispositionV1, SuccessorPlan},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, account_evidence, bounded_instructions},
    runtime::decode_hex,
    upgrade::{CHECKED_SET_PREPARE_SCHEMA, SEMANTIC_DERIVATION_V1},
    wallet_terminal::authenticate_role,
};

const INPUT_FORMAT: &str = "dclutch-flagship-resolution-input-v1";
const LOCAL_INPUT_FORMAT: &str = "dclutch-owned-loopback-flagship-resolution-input-v1";
const CHECKPOINT_FORMAT: &str = "dclutch-flagship-resolution-checkpoint-v3";
const LOCAL_CHECKPOINT_FORMAT: &str = "dclutch-owned-loopback-flagship-resolution-checkpoint-v3";
const PRODUCER_FACTS_FORMAT: &str = "dclutch-flagship-pyth-update-facts-v1";
const PRODUCER_CHECKPOINT_FORMAT: &str = "dclutch-flagship-resolution-producer-v1";
const LOCAL_PRODUCER_CHECKPOINT_FORMAT: &str =
    "dclutch-owned-loopback-flagship-resolution-producer-v1";
const TABLE_PROVISION_JOURNAL_FORMAT: &str = "dclutch-flagship-resolution-alt-journal-v3";
const LOCAL_TABLE_PROVISION_JOURNAL_FORMAT: &str =
    "dclutch-owned-loopback-flagship-resolution-alt-journal-v3";
const CAMPAIGN_FORMAT: &str = "dclutch-successor-campaign-report-v1";
const PLAN_FORMAT: &str = crate::model::SUCCESSOR_PLAN_SCHEMA_V3;
/// Provisional operator delay after the terminal window. It is not a protocol
/// liveness bound; a measured finalized reclaim campaign can lift or narrow it.
const FLAGSHIP_RECLAIM_DELAY_SECONDS_V1: i64 = 3_600;
/// Chain-derived Pyth accumulator PriceFeedMessage V1 wire width.
const PYTH_PRICE_FEED_MESSAGE_BYTES_V1: usize = 85;
/// Exact bincode wire width of the `Clock` sysvar: five little-endian 8-byte
/// fields, no padding.
const CLOCK_SYSVAR_BYTES_V1: usize = 40;
const GEOMETRY_BLOCKHASH: [u8; 32] = [0x6d; 32];

const fn input_format(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => INPUT_FORMAT,
        ExpectedClusterV1::OwnedLoopback => LOCAL_INPUT_FORMAT,
    }
}

const fn checkpoint_format(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => CHECKPOINT_FORMAT,
        ExpectedClusterV1::OwnedLoopback => LOCAL_CHECKPOINT_FORMAT,
    }
}

const fn producer_checkpoint_format(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => PRODUCER_CHECKPOINT_FORMAT,
        ExpectedClusterV1::OwnedLoopback => LOCAL_PRODUCER_CHECKPOINT_FORMAT,
    }
}

const fn table_journal_format(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => TABLE_PROVISION_JOURNAL_FORMAT,
        ExpectedClusterV1::OwnedLoopback => LOCAL_TABLE_PROVISION_JOURNAL_FORMAT,
    }
}

fn expected_pyth_release(expected: ExpectedClusterV1) -> Result<PythReleaseV1> {
    match expected {
        ExpectedClusterV1::Devnet => devnet_release_v1()
            .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}"))),
        ExpectedClusterV1::OwnedLoopback => local_validator_release_v1()
            .map(|local| *local.release())
            .map_err(|error| {
                Error::new(format!("compiled owned-loopback Pyth release: {error:?}"))
            }),
    }
}

const fn expected_receiver_minimum_signatures(expected: ExpectedClusterV1) -> u8 {
    match expected {
        // The current five-guardian public release happens to use the same
        // value as strict majority, but it remains a distinct Config fact.
        ExpectedClusterV1::Devnet => 3,
        // The pinned 19-guardian lab Config accepts five signatures while
        // Router verification requires strict-majority ten.
        ExpectedClusterV1::OwnedLoopback => 5,
    }
}

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
    source_material_staging: String,
    source_spec: String,
    /// The six Execute-only staging cursors. `default` is carried so a producer
    /// checkpoint minted before §7.9 named them still deserializes; an empty
    /// value never reaches a message, because `nonzero_pubkey` refuses it.
    #[serde(default)]
    source_spec_staging: String,
    source_provider_release: String,
    #[serde(default)]
    source_provider_release_staging: String,
    adapter_config: String,
    #[serde(default)]
    adapter_config_staging: String,
    window: String,
    #[serde(default)]
    window_staging: String,
    statistic: String,
    #[serde(default)]
    statistic_staging: String,
    pyth_release: String,
    #[serde(default)]
    pyth_release_staging: String,
    product: String,
    product_staging: String,
    result_domain: String,
    result_domain_staging: String,
    portfolio: String,
    portfolio_staging: String,
    capability_manifest: String,
    capability_manifest_staging: String,
    funding_ledger: String,
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
    /// The Execute and Reclaim fee payer, which must not be the resolver.
    ///
    /// Both frames pin their sole instruction signer — the resolver — readonly,
    /// and the message compiler promotes whoever pays into a *writable* signer.
    /// A payer of the driver's own choosing is what keeps the two compatible
    /// (§7.13). `default` is carried so a producer checkpoint minted before wall
    /// 10 still deserializes; an empty value never reaches a message, because
    /// `nonzero_pubkey` refuses it in [`SelectedInputV1::parse`].
    #[serde(default)]
    payer: String,
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
    cluster: String,
    mode: String,
    plan_sha256: String,
    execution: CampaignExecutionEnvelopeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StableAddressClassV1 {
    Beneficiary,
    MarketState,
    SourceState,
    FundingLedger,
    ActivationCache,
    Infrastructure,
    Program,
    ProgramData,
    FinalizedRecord,
    FinalizedRecordStaging,
    ProviderConfig,
    ProviderObservation,
    CallerAuthority,
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
    /// The refresh digest this producer authenticated against, when one was
    /// supplied. A resume that changes which refresh is in play changes which
    /// rows authenticated, so it is bound here like every other input digest.
    #[serde(default)]
    refreshed_evidence_sha256: Option<String>,
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
    Dispatching,
    Submitted,
    Finalized,
}

fn authenticate_send_boundary(phase: DurablePhaseV1) -> Result<()> {
    if phase != DurablePhaseV1::Dispatching {
        return Err(Error::new(
            "durable packet send requires a fsynced Dispatching phase",
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
    compute_units_consumed: u64,
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
    Accept,
    Reclaim,
    Complete,
}

impl StageV1 {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "submit" => Ok(Self::Submit),
            "execute" => Ok(Self::Execute),
            "accept" => Ok(Self::Accept),
            "reclaim" => Ok(Self::Reclaim),
            "complete" => Ok(Self::Complete),
            _ => Err(Error::new(
                "--through must be submit, execute, accept, reclaim, or complete",
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Execute => "execute",
            Self::Accept => "accept",
            Self::Reclaim => "reclaim",
            Self::Complete => "complete",
        }
    }

    const fn routing_stage(self) -> Self {
        match self {
            Self::Accept => Self::Execute,
            other => other,
        }
    }
}

#[derive(Clone, Debug)]
struct SelectedInputV1 {
    generation: u64,
    release_set: [u8; 32],
    submitter: Pubkey,
    resolver: Pubkey,
    payer: Pubkey,
    refund_recipient: Pubkey,
    terminal_sequence: u64,
    reclaim_after_unix_seconds: i64,
    post_update_body: Vec<u8>,
    accounts: BTreeMap<&'static str, Pubkey>,
    lookup_tables: BTreeMap<StageV1, Pubkey>,
}

impl SelectedInputV1 {
    fn parse(input: &PlanInputV1, expected_cluster: ExpectedClusterV1) -> Result<Self> {
        let expected_format = input_format(expected_cluster);
        if input.format != expected_format {
            return Err(Error::new(format!(
                "input format must be {expected_format}"
            )));
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
        let payer = nonzero_pubkey(&input.payer, "payer")?;
        if payer == resolver {
            return Err(Error::new(
                "payer must differ from resolver: Execute and Reclaim pin the resolver readonly, \
                 and the fee payer is always a writable signer",
            ));
        }
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
        account!("source_material_staging", source_material_staging);
        account!("source_spec", source_spec);
        account!("source_spec_staging", source_spec_staging);
        account!("source_provider_release", source_provider_release);
        account!(
            "source_provider_release_staging",
            source_provider_release_staging
        );
        account!("adapter_config", adapter_config);
        account!("adapter_config_staging", adapter_config_staging);
        account!("window", window);
        account!("window_staging", window_staging);
        account!("statistic", statistic);
        account!("statistic_staging", statistic_staging);
        account!("pyth_release", pyth_release);
        account!("pyth_release_staging", pyth_release_staging);
        account!("product", product);
        account!("product_staging", product_staging);
        account!("result_domain", result_domain);
        account!("result_domain_staging", result_domain_staging);
        account!("portfolio", portfolio);
        account!("portfolio_staging", portfolio_staging);
        account!("capability_manifest", capability_manifest);
        account!("capability_manifest_staging", capability_manifest_staging);
        account!("funding_ledger", funding_ledger);
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
            payer,
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
            .get(&stage.routing_stage())
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
            // The payer must alias nothing: as the fee payer it is a writable
            // signer, so aliasing any frame account flips that account's pinned
            // privilege and the packet is refused on chain, not here.
            ("payer", self.payer),
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
            CorePhase::Open,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted,
            Consumed,
            Submitted,
            Submitted,
        ) => Ok(StageV1::Accept),
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

    /// Read a key that is allowed to be vacant on chain.
    ///
    /// A key the snapshot never fetched is *not* the same fact as a key the
    /// chain reports vacant, and reading the first as the second fabricates a
    /// zero balance the projections then compare against. The map distinguishes
    /// them — a fetched-but-vacant key is present with `None` — so this refuses
    /// rather than inventing an observation it never made.
    fn observed_or_vacant(&self, key: Pubkey) -> Result<ObservedAccount> {
        let account = self
            .accounts
            .get(&key)
            .ok_or_else(|| {
                Error::new(format!(
                    "finalized snapshot never observed {key}; a vacant reading would be a fabrication"
                ))
            })?
            .clone()
            .unwrap_or(RpcAccount {
                lamports: 0,
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
                data: Vec::new(),
            });
        Ok(ObservedAccount {
            observation: self.observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data,
        })
    }
}

/// The Pyth Receiver's fee treasury, which every provider submit credits.
///
/// It is a derived address rather than a named input account, which is why the
/// input's `accounts` map does not carry it.
fn receiver_treasury_address(selected: &SelectedInputV1) -> Result<Pubkey> {
    Ok(
        Pubkey::find_program_address(&[b"treasury", &[0]], &selected.account("receiver_program")?)
            .0,
    )
}

/// Observe the post-state of a stage that actually ran.
///
/// [`observe`] answers for the accounts the *input* names. A finalized
/// projection asks about the accounts the *transaction* touched, and for
/// Reclaim those are not the same set: its writable frame includes a provider
/// authority PDA that no input label carries and that the stage itself closes.
/// Reading that as vacant is exactly the fabrication `observed_or_vacant`
/// refuses, so the fix is to fetch it rather than to assume it — §7.8's wall,
/// in the one stage that had never run.
///
/// The plan's resolved keys are the authority on what the message touched; they
/// are already pinned byte-exact in `pre_accounts` and re-derived from the
/// compiled message, so this widens what is *read*, never what is believed.
fn observe_stage_poststate_v1(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let mut keys = observation_keys_v1(selected)?;
    for key in &plan.resolved_account_keys {
        keys.insert(pubkey(key)?);
    }
    if keys.len() > 100 {
        return Err(Error::new(
            "flagship finalized snapshot exceeds the 100-account RPC bound",
        ));
    }
    observe_keys(rpc, keys, minimum_slot)
}

fn observe(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    _stage: StageV1,
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let keys = observation_keys_v1(selected)?;
    if keys.len() > 100 {
        return Err(Error::new(
            "flagship finalized snapshot exceeds the 100-account RPC bound",
        ));
    }
    observe_keys(rpc, keys, minimum_slot)
}

fn observation_keys_v1(selected: &SelectedInputV1) -> Result<BTreeSet<Pubkey>> {
    let mut keys = BTreeSet::new();
    keys.extend(selected.accounts.values().copied());
    keys.insert(lifecycle_address(selected)?);
    keys.extend(selected.lookup_tables.values().copied());
    keys.insert(sysvar::rent::ID);
    // The finalized projections read every writable the stage mutates, and four
    // of those are identities the input names outside its `accounts` map — the
    // three signing roles and the Receiver's derived fee treasury. Without them
    // the snapshot cannot answer for the accounts the projection is about.
    keys.insert(selected.submitter);
    keys.insert(selected.resolver);
    keys.insert(selected.refund_recipient);
    keys.insert(receiver_treasury_address(selected)?);
    Ok(keys)
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

/// The Core caller-authority PDA the Execute instruction carries at index 0.
///
/// The five seed coordinates are all named by the input and all pinned against
/// the finalized Market by `chain_facts` — `market_id == market`, `generation`,
/// `selected_release_set`, the Source state, and `market_account.owner ==
/// core_program` — so this derivation is authenticated by the same read that
/// authenticates the stage. The seeds themselves are never spelled here: the
/// transport builder's own `provider_execute_caller_authority_v3` does the work,
/// which is what makes this a lookup rather than a second implementation.
fn execute_caller_authority(selected: &SelectedInputV1) -> Result<Pubkey> {
    let market = selected.account("market")?;
    let market_id = Identity::new(market.to_bytes())
        .map_err(|error| Error::new(format!("Market identity: {error:?}")))?;
    provider_execute_caller_authority_v3(
        selected.release_set,
        market,
        market_id,
        selected.generation,
        selected.account("source_state")?,
        selected.account("core_program")?,
    )
    .map(|(_, caller_authority)| caller_authority)
    .map_err(|error| Error::new(format!("Execute caller authority: {error:?}")))
}

fn stable_lookup_union(selected: &SelectedInputV1, stage: StageV1) -> Result<Vec<StableAddressV1>> {
    let routed_stage = stage.routing_stage();
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
    match routed_stage {
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
            selected!("market", MarketState);
            common_release!();
            selected!("trading_program", Program);
            selected!("trading_programdata", ProgramData);
            selected!("source_state", SourceState);
            selected!("source_material", FinalizedRecord);
            selected!("source_material_staging", FinalizedRecordStaging);
            selected!("capability_manifest", FinalizedRecord);
            selected!("capability_manifest_staging", FinalizedRecordStaging);
            selected!("funding_ledger", FundingLedger);
            selected!("source_spec", FinalizedRecord);
            selected!("source_spec_staging", FinalizedRecordStaging);
            selected!("source_provider_release", FinalizedRecord);
            selected!("source_provider_release_staging", FinalizedRecordStaging);
            selected!("adapter_config", FinalizedRecord);
            selected!("adapter_config_staging", FinalizedRecordStaging);
            selected!("window", FinalizedRecord);
            selected!("window_staging", FinalizedRecordStaging);
            selected!("statistic", FinalizedRecord);
            selected!("statistic_staging", FinalizedRecordStaging);
            selected!("pyth_release", FinalizedRecord);
            selected!("pyth_release_staging", FinalizedRecordStaging);
            selected!("product", FinalizedRecord);
            selected!("product_staging", FinalizedRecordStaging);
            selected!("result_domain", FinalizedRecord);
            selected!("result_domain_staging", FinalizedRecordStaging);
            selected!("portfolio", FinalizedRecord);
            selected!("portfolio_staging", FinalizedRecordStaging);
            selected!("update_account", ProviderObservation);
            // Named by the input but historically pushed only into the Reclaim
            // union, and derived rather than named. Both are ordinary readonly
            // keys the Execute instruction carries; seating them here is what
            // takes the bare action under the packet limit (§7.9, §7.11).
            selected!("certificate", SourceState);
            push(
                "lifecycle",
                StableAddressClassV1::ProviderObservation,
                lifecycle_address(selected)?,
            )?;
            // The ninth key §7.11 measured and declined to seat, and the one
            // wall 10 cannot do without: a distinct fee payer costs 96 bytes and
            // unbundling the certificate top-up returns only 48, leaving 19 that
            // have to come from somewhere. This row is worth 31.
            //
            // §7.13 expected this to need Core's request encoding re-derived in
            // the producer or the builder's report plumbed out. Neither is
            // needed. The address is a PDA over five coordinates and
            // `chain_facts` already pins every one of them against the finalized
            // Market before any stage runs: `market_id == market`, `generation`,
            // `selected_release_set`, the Source state, and the Market's owner
            // being the named Core program. So the union may derive it from the
            // input alone — and it derives it by *calling the builder's own
            // function*, not by reimplementing it, so the two cannot drift.
            push(
                "caller_authority",
                StableAddressClassV1::CallerAuthority,
                execute_caller_authority(selected)?,
            )?;
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
        StageV1::Accept => unreachable!("accept routes through execute"),
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

fn authenticate_selected_pyth_release(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    require_provider_observation: bool,
    expected_cluster: ExpectedClusterV1,
) -> Result<PythReleaseV1> {
    let release = PythReleaseV1::decode(
        &snapshot
            .account(selected.account("pyth_release")?, "Pyth release record")?
            .data,
    )
    .map_err(|error| Error::new(format!("Pyth release: {error:?}")))?;
    let expected = expected_pyth_release(expected_cluster)?;
    if release.to_bytes() != expected.to_bytes() {
        return Err(Error::new(format!(
            "Pyth release record is not the exact {} row",
            expected_cluster.evidence_label()
        )));
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
        || config_view.minimum_signatures()
            != expected_receiver_minimum_signatures(expected_cluster)
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
        let (lookup_addresses, writable, readonly, resolved) =
            resolve_provider_v0_keys(&message, &table)?;
        let mut hasher = Sha256::new();
        for address in &lookup_addresses {
            hasher.update(address.as_ref());
        }
        if lookup_addresses
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            != self.lookup_table_addresses
            || hex(&hasher.finalize()) != self.lookup_table_addresses_sha256
        {
            return Err(Error::new("durable provider lookup address digest changed"));
        }
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
            DurablePhaseV1::SignedNotSubmitted
            | DurablePhaseV1::Dispatching
            | DurablePhaseV1::Submitted
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
enum ReceiptStageV1 {
    #[serde(rename = "submit")]
    Submit,
    #[serde(rename = "resolution-provider-execute-v1")]
    ProviderExecute,
    #[serde(rename = "core-terminal-accept-v1")]
    CoreAccept,
    #[serde(rename = "reclaim")]
    Reclaim,
}

impl ReceiptStageV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::ProviderExecute => "execute",
            Self::CoreAccept => "accept",
            Self::Reclaim => "reclaim",
        }
    }

    fn from_stage(stage: StageV1) -> Result<Self> {
        match stage {
            StageV1::Submit => Ok(Self::Submit),
            StageV1::Execute => Ok(Self::ProviderExecute),
            StageV1::Accept => Ok(Self::CoreAccept),
            StageV1::Reclaim => Ok(Self::Reclaim),
            StageV1::Complete => Err(Error::new("complete has no mutation receipt")),
        }
    }

    /// The routing stage whose lookup table this receipt's packet resolved
    /// against. Accept rides Execute's table (`StageV1::routing_stage`).
    const fn routing_stage(self) -> StageV1 {
        match self {
            Self::Submit => StageV1::Submit,
            Self::ProviderExecute | Self::CoreAccept => StageV1::Execute,
            Self::Reclaim => StageV1::Reclaim,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageReceiptV1 {
    stage: ReceiptStageV1,
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
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

/// Chain-reauthenticated facts handed from the flagship provider lifecycle to
/// the complete-life campaign.
///
/// The receipt bodies remain the flagship driver's own serialized values.  A
/// campaign may carry them, but it does not get a second DTO in which to
/// restate their arithmetic or account vectors.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectResolutionTerminalEvidenceV1 {
    pub(crate) input_sha256: String,
    pub(crate) checkpoint_sha256: String,
    pub(crate) market: String,
    pub(crate) source_state: String,
    pub(crate) source_state_sha256: String,
    pub(crate) certificate: String,
    pub(crate) certificate_sha256: String,
    pub(crate) resolution_program: String,
    pub(crate) generation: u64,
    pub(crate) terminal_sequence: u64,
    pub(crate) selector: u32,
    pub(crate) attempt_index: u32,
    pub(crate) route: &'static str,
    pub(crate) certificate_kind: &'static str,
    pub(crate) finalized_receipts: Vec<Value>,
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
    /// The optional post-activation evidence refresh
    /// (`docs/design/EVIDENCE_REFRESH_V1.md`). Absent, this producer behaves
    /// byte-for-byte as it did before the refresh existed.
    refreshed_evidence: Option<PathBuf>,
    pyth_facts: Option<PathBuf>,
    producer_checkpoint: Option<PathBuf>,
    output: Option<PathBuf>,
    /// The Execute and Reclaim fee payer. It is a caller choice, not a chain
    /// reading — nothing on chain names it — so the producer must be told, and
    /// what it produces is authenticated against it like any other coordinate.
    payer: Option<String>,
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
                "--refreshed-evidence" => set_once(
                    &mut parsed.refreshed_evidence,
                    PathBuf::from(value),
                    "--refreshed-evidence",
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
                "--payer" => set_once(&mut parsed.payer, value, "--payer")?,
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
    standing_checkpoint: Option<PathBuf>,
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
                        "--standing-checkpoint" => set_once(
                            &mut parsed.standing_checkpoint,
                            PathBuf::from(value),
                            "--standing-checkpoint",
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

/// The producer's rows in the crate's one canonical evidence-row shape.
///
/// Both spellings carry the identical seven fields; this exists so the refresh
/// admission can be written once, against one type, rather than once per
/// parser.
fn campaign_rows_as_model_v1(
    evidence: &CampaignMarketEvidenceV1,
) -> BTreeMap<String, AccountEvidence> {
    evidence
        .accounts
        .iter()
        .map(|(label, row)| {
            (
                label.clone(),
                AccountEvidence {
                    address: row.address.clone(),
                    owner: row.owner.clone(),
                    lamports: row.lamports,
                    executable: row.executable,
                    data_len: row.data_len,
                    data_sha256: row.data_sha256.clone(),
                    account_sha256: row.account_sha256.clone(),
                },
            )
        })
        .collect()
}

fn model_rows_as_campaign_v1(
    rows: BTreeMap<String, AccountEvidence>,
) -> BTreeMap<String, CampaignAccountEvidenceV1> {
    rows.into_iter()
        .map(|(label, row)| {
            (
                label,
                CampaignAccountEvidenceV1 {
                    address: row.address,
                    owner: row.owner,
                    lamports: row.lamports,
                    executable: row.executable,
                    data_len: row.data_len,
                    data_sha256: row.data_sha256,
                    account_sha256: row.account_sha256,
                },
            )
        })
        .collect()
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
    expected_cluster: ExpectedClusterV1,
) -> Result<&'a CampaignMarketEvidenceV1> {
    let expected_label = match expected_cluster {
        ExpectedClusterV1::Devnet => "devnet",
        ExpectedClusterV1::OwnedLoopback => "loopback",
    };
    if evidence.schema != CAMPAIGN_FORMAT
        || evidence.cluster != expected_label
        || evidence.mode != "execute"
        || evidence.plan_sha256 != plan_sha256
        || !evidence.execution.completed
    {
        return Err(Error::new(
            "campaign schema, typed cluster, execute mode, exact plan digest, or completed execution proof refused",
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
    refreshed_sha256: Option<&str>,
    facts_sha256: &str,
    expected_cluster: ExpectedClusterV1,
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
    if checkpoint.format != producer_checkpoint_format(expected_cluster)
        || checkpoint.plan_sha256 != plan_sha256
        || checkpoint.campaign_evidence_sha256 != campaign_sha256
        || checkpoint.refreshed_evidence_sha256.as_deref() != refreshed_sha256
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

fn authenticate_producer_checkpoint(
    checkpoint: &ProducerCheckpointV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<SelectedInputV1> {
    if checkpoint.format != producer_checkpoint_format(expected_cluster)
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
    let selected = SelectedInputV1::parse(&checkpoint.planned_input, expected_cluster)?;
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
    expected_cluster: ExpectedClusterV1,
) -> Result<TableProvisionJournalV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TableProvisionJournalV1 {
                format: table_journal_format(expected_cluster).to_owned(),
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
    authenticate_table_journal_identity(&journal, producer_identity_sha256, expected_cluster)?;
    let intent_shape = match journal.phase {
        DurablePhaseV1::Planned => {
            journal.intent.is_some()
                && journal.intent_sha256.is_some()
                && journal.signed_transaction_base64.is_none()
                && journal.signed_transaction_sha256.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none()
        }
        DurablePhaseV1::SignedNotSubmitted
        | DurablePhaseV1::Dispatching
        | DurablePhaseV1::Submitted => {
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

fn authenticate_table_journal_identity(
    journal: &TableProvisionJournalV1,
    producer_identity_sha256: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    if journal.format != table_journal_format(expected_cluster)
        || journal.producer_identity_sha256 != producer_identity_sha256
    {
        return Err(Error::new(
            "table journal format or immutable producer identity changed",
        ));
    }
    Ok(())
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

/// The reclaim floor is a checkpoint COMMITMENT, not a per-produce observation.
///
/// It is encoded into the provider submission the checkpoint plans, so a resume
/// that re-derived it from the wall clock would plan a different transaction
/// than the one the checkpoint already committed to. And because the derivation
/// is `max(observation, window_end) + delay`, once the observation passes the
/// window end it tracks the clock: `prior.planned_input != input` on every
/// subsequent produce, and the resume guard refuses forever. It is therefore
/// pinned from the prior checkpoint exactly as `lookup_creation_slots` pins the
/// table creation slots.
///
/// Pinning moves the value from derived to carried, so it acquires its own
/// bounds. A carried floor must lie inside the closed interval of values this
/// producer could legitimately have derived at any observation between founding
/// and now:
///
/// * at least `window_end + delay` — the floor a fresh derivation always clears,
///   and below which `dclutch-provider-transport-v3-operator` refuses the intent
///   outright (`intent.reclaim_after_unix_seconds < window.end_unix_seconds()`);
/// * at most this observation's own derivation — so a hand-edited checkpoint
///   cannot push the floor into the future and strand the reclaim.
///
/// Nothing is excluded from the resume comparison: `planned_input` is still
/// compared in full, this field included. What changed is only which document
/// the field is read from, and a carried value that fails either bound refuses.
fn pinned_reclaim_after_unix_seconds(
    prior: Option<&ProducerCheckpointV1>,
    observation_unix_timestamp: i64,
    window_end_unix_seconds: i64,
) -> Result<i64> {
    let derived = observation_unix_timestamp
        .max(window_end_unix_seconds)
        .checked_add(FLAGSHIP_RECLAIM_DELAY_SECONDS_V1)
        .ok_or_else(|| Error::new("provider reclaim time overflow"))?;
    let Some(prior) = prior else {
        return Ok(derived);
    };
    let pinned = prior.planned_input.reclaim_after_unix_seconds;
    let floor = window_end_unix_seconds
        .checked_add(FLAGSHIP_RECLAIM_DELAY_SECONDS_V1)
        .ok_or_else(|| Error::new("provider reclaim time overflow"))?;
    if pinned < floor {
        return Err(Error::new(format!(
            "producer checkpoint reclaim floor {pinned} is below the terminal window bound {floor}"
        )));
    }
    if pinned > derived {
        return Err(Error::new(format!(
            "producer checkpoint reclaim floor {pinned} is ahead of the derivation {derived} this observation admits"
        )));
    }
    Ok(pinned)
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
            // Baseline present (it fixed the width the equality was judged at),
            // receipt absent (there is no Upgrade), and none of the
            // carry-forward-only transport fields.
            CheckedDeploymentDispositionV1::AlreadyCurrent => {
                observed.already_current_closure().holds() && observed.carries_no_transport_fields()
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
        // The table says which KIND of row this is. Carry-forward is exact; a
        // role the cut owns may be satisfied by an Upgrade receipt or by proven
        // equality with the checked candidate, and both are admitted here.
        let disposition_matches = expected_disposition.admits(observed.disposition);
        if observed.role != expected_role
            || !disposition_matches
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

fn authenticate_checked_plan(
    plan: &SuccessorPlan,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    match expected_cluster {
        ExpectedClusterV1::Devnet => authenticate_checked_upgrade_plan(plan),
        ExpectedClusterV1::OwnedLoopback => {
            if plan.checked_upgrade_set.is_some() {
                return Err(Error::new(
                    "owned-loopback flagship production refuses permanent-devnet Upgrade evidence",
                ));
            }
            crate::local_mutable::authenticate_checked_local_mutable_plan_v1(plan)
        }
    }
}

fn campaign_record_staging(
    campaign: &CampaignMarketEvidenceV1,
    label: &str,
    schema: [u8; 32],
    registry_program: Pubkey,
) -> Result<Pubkey> {
    let row = campaign
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("completed campaign omitted {label}")))?;
    let raw = nonzero_pubkey(&row.address, label)?;
    let digest = hex32(&row.data_sha256)?;
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    if raw != expected_raw || staging == Pubkey::default() {
        return Err(Error::new(format!(
            "completed campaign {label} is not the canonical raw record coordinate"
        )));
    }
    Ok(staging)
}

fn producer_selected_input(
    plan: &SuccessorPlan,
    campaign: &CampaignMarketEvidenceV1,
    facts: &ProducerPythFactsV1,
    post_update_body: &[u8],
    coherent: &FinalizedSnapshotV1,
    slots: &BTreeMap<StageV1, u64>,
    prior: Option<&ProducerCheckpointV1>,
    payer: Pubkey,
    expected_cluster: ExpectedClusterV1,
) -> Result<PlanInputV1> {
    if plan.schema != PLAN_FORMAT {
        return Err(Error::new(format!("plan schema must be {PLAN_FORMAT}")));
    }
    authenticate_checked_plan(plan, expected_cluster)?;
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
    let pyth = expected_pyth_release(expected_cluster)?;
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
    let reclaim_after_unix_seconds = pinned_reclaim_after_unix_seconds(
        prior,
        coherent.observation.unix_timestamp,
        window.end_unix_seconds(),
    )?;
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
    let (pyth_release, pyth_release_staging) = plan_record(plan, "pyth_release")?;
    let source_material_staging = campaign_record_staging(
        campaign,
        "source_material_record",
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        registry_program,
    )?;
    let capability_manifest = campaign_account(campaign, "capability_manifest_record")?;
    let capability_manifest_staging = campaign_record_staging(
        campaign,
        "capability_manifest_record",
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        registry_program,
    )?;
    let product_staging = campaign_record_staging(
        campaign,
        "product_record",
        PRODUCT_RECORD_SCHEMA_ID_V2,
        registry_program,
    )?;
    let result_domain_staging = campaign_record_staging(
        campaign,
        "result_domain_record",
        RESULT_DOMAIN_SCHEMA_ID_V2,
        registry_program,
    )?;
    let portfolio_staging = campaign_record_staging(
        campaign,
        "portfolio_record",
        PORTFOLIO_SCHEMA_ID_V2,
        registry_program,
    )?;
    // The six Execute-only staging cursors. Each is the canonical staging PDA of
    // a campaign record whose raw coordinate `campaign_record_staging` re-derives
    // from the same (schema, digest) pair, so a wrong schema refuses here rather
    // than silently seating a junk address in the Execute lookup table.
    let source_spec_staging = campaign_record_staging(
        campaign,
        "source_spec_record",
        SOURCE_SPEC_SCHEMA_ID_V1,
        registry_program,
    )?;
    let source_provider_release_staging = campaign_record_staging(
        campaign,
        "provider_release_record",
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        registry_program,
    )?;
    let adapter_config_staging = campaign_record_staging(
        campaign,
        "pyth_adapter_config_record",
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        registry_program,
    )?;
    let window_staging = campaign_record_staging(
        campaign,
        "window_spec_record",
        WINDOW_SPEC_SCHEMA_ID_V1,
        registry_program,
    )?;
    let statistic_staging = campaign_record_staging(
        campaign,
        "statistic_spec_record",
        STATISTIC_SPEC_SCHEMA_ID_V1,
        registry_program,
    )?;
    let funding_ledger = campaign_account(campaign, "resolution_funding_ledger")?;
    Ok(PlanInputV1 {
        format: input_format(expected_cluster).to_owned(),
        generation,
        release_set: plan.release_set_id.clone(),
        submitter: submitter.to_string(),
        resolver: resolver.to_string(),
        payer: payer.to_string(),
        refund_recipient: beneficiary.to_string(),
        terminal_sequence,
        reclaim_after_unix_seconds,
        post_update_body_base64: BASE64.encode(post_update_body),
        accounts: AccountSelectorsV1 {
            market: market.to_string(),
            source_state: source_state.to_string(),
            source_material: campaign_account(campaign, "source_material_record")?.to_string(),
            source_material_staging: source_material_staging.to_string(),
            source_spec: campaign_account(campaign, "source_spec_record")?.to_string(),
            source_spec_staging: source_spec_staging.to_string(),
            source_provider_release: campaign_account(campaign, "provider_release_record")?
                .to_string(),
            source_provider_release_staging: source_provider_release_staging.to_string(),
            adapter_config: adapter_key.to_string(),
            adapter_config_staging: adapter_config_staging.to_string(),
            window: window_key.to_string(),
            window_staging: window_staging.to_string(),
            statistic: campaign_account(campaign, "statistic_spec_record")?.to_string(),
            statistic_staging: statistic_staging.to_string(),
            pyth_release: pyth_release.to_owned(),
            pyth_release_staging: pyth_release_staging.to_owned(),
            product: campaign_account(campaign, "product_record")?.to_string(),
            product_staging: product_staging.to_string(),
            result_domain: campaign_account(campaign, "result_domain_record")?.to_string(),
            result_domain_staging: result_domain_staging.to_string(),
            portfolio: campaign_account(campaign, "portfolio_record")?.to_string(),
            portfolio_staging: portfolio_staging.to_string(),
            capability_manifest: capability_manifest.to_string(),
            capability_manifest_staging: capability_manifest_staging.to_string(),
            funding_ledger: funding_ledger.to_string(),
            certificate: certificate.to_string(),
            activation_cache: plan.activation.clone(),
            // The V2 PDA, because that is the profile Core authenticates:
            // since `2951b226` every route reads `dclutch:infrastructure:v2`
            // and nothing else. The address is domain-derived under Core, so
            // it is the same account whether this cohort was born at V2 or
            // succeeded into it; only the bytes differ. The sealed V1 is
            // lineage evidence and is never an account in a live instruction.
            infrastructure: plan.genesis_infrastructure_profile.address.clone(),
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

fn run_producer(arguments: Vec<String>, expected_cluster: ExpectedClusterV1) -> Result<()> {
    let arguments = ProducerCommandArgumentsV1::parse(arguments)?;
    let plan_path = absolute(arguments.plan, "--plan")?;
    let campaign_path = absolute(arguments.campaign_evidence, "--campaign-evidence")?;
    let refreshed_path = arguments
        .refreshed_evidence
        .map(|path| absolute(Some(path), "--refreshed-evidence"))
        .transpose()?;
    let facts_path = absolute(arguments.pyth_facts, "--pyth-facts")?;
    let checkpoint_path = absolute(arguments.producer_checkpoint, "--producer-checkpoint")?;
    let output_path = absolute(arguments.output, "--output")?;
    let plan_bytes = fs::read(&plan_path)?;
    let campaign_bytes = fs::read(&campaign_path)?;
    let refreshed_bytes = refreshed_path.as_ref().map(fs::read).transpose()?;
    let facts_bytes = fs::read(&facts_path)?;
    let plan_sha256 = hex(&Sha256::digest(&plan_bytes));
    let campaign_sha256 = hex(&Sha256::digest(&campaign_bytes));
    let refreshed_sha256 = refreshed_bytes
        .as_deref()
        .map(|bytes| hex(&Sha256::digest(bytes)));
    let facts_sha256 = hex(&Sha256::digest(&facts_bytes));
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let campaign_envelope: CampaignEvidenceV1 = serde_json::from_slice(&campaign_bytes)?;
    let founding_campaign = completed_campaign(&campaign_envelope, &plan_sha256, expected_cluster)?;
    let (facts, encoded_vaa, update_account, post_update_body) =
        parse_producer_facts(&facts_bytes)?;
    let prior = load_producer_checkpoint(
        &checkpoint_path,
        &plan_sha256,
        &campaign_sha256,
        refreshed_sha256.as_deref(),
        &facts_sha256,
        expected_cluster,
    )?;
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    expected_cluster.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    // The refresh, if one was supplied, is admitted here and merged into the
    // effective evidence. Nothing below is weakened by it: every label the
    // producer pins is still pinned byte-exact against the live finalized
    // account by `authenticate_campaign_account`. What the refresh changes is
    // only WHICH document was allowed to carry the row, and it may carry one
    // only after reproducing the founding's immutable records exactly.
    // `docs/design/EVIDENCE_REFRESH_V1.md`.
    let refreshed_campaign = match refreshed_bytes.as_deref() {
        None => None,
        Some(bytes) => {
            let refresh = evidence_refresh::parse_refresh_v1(bytes)?;
            let effective = evidence_refresh::effective_accounts_v1(
                &refresh,
                &campaign_bytes,
                &campaign_rows_as_model_v1(founding_campaign),
                &plan_sha256,
                expected_cluster,
                rpc.finalized_slot()?,
            )?;
            Some(CampaignMarketEvidenceV1 {
                completed: founding_campaign.completed.clone(),
                accounts: model_rows_as_campaign_v1(effective),
                // The same §3 selection the terminal sequence makes. The
                // producer only bounds this field, but it must not hand two
                // different documents two different custody contexts.
                founding_custody_context: evidence_refresh::effective_custody_context_v1(
                    Some(&refresh),
                    &founding_campaign.founding_custody_context,
                )?,
                direct_selected_manifest_entry_index: founding_campaign
                    .direct_selected_manifest_entry_index,
            })
        }
    };
    let campaign = refreshed_campaign.as_ref().unwrap_or(founding_campaign);
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
        prior.as_ref(),
        nonzero_pubkey(
            arguments
                .payer
                .as_deref()
                .ok_or_else(|| Error::new("--payer is required"))?,
            "--payer",
        )?,
        expected_cluster,
    )?;
    let selected = SelectedInputV1::parse(&input, expected_cluster)?;
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
    authenticate_selected_pyth_release(&selected, &snapshot, true, expected_cluster)?;
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
        ("capability_manifest_record", "capability_manifest"),
        ("resolution_funding_ledger", "funding_ledger"),
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
        format: producer_checkpoint_format(expected_cluster).to_owned(),
        plan_sha256,
        campaign_evidence_sha256: campaign_sha256,
        refreshed_evidence_sha256: refreshed_sha256,
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
        compute_units_consumed: u64,
        post_balances: Vec<u64>,
    },
}

fn finalized_compute_units(meta: &Value, label: &str) -> Result<u64> {
    meta.get("computeUnitsConsumed")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("finalized {label} omitted computeUnitsConsumed")))
}

fn table_transaction_status(
    rpc: &mut Rpc,
    journal: &TableProvisionJournalV1,
    expected_cluster: ExpectedClusterV1,
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
        expected_cluster
            .authenticate_finalized_fee(fee_lamports, "Resolution table transaction")?;
        let compute_units_consumed = finalized_compute_units(meta, "table transaction")?;
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
            compute_units_consumed,
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
    compute_units_consumed: u64,
    post_balances: Vec<u64>,
) -> Result<TableProvisionReceiptV1> {
    let submission = journal
        .intent
        .as_ref()
        .ok_or_else(|| Error::new("table finalization omitted durable intent"))?;
    let snapshot = observe(rpc, selected, StageV1::Submit, slot)?;
    // §7.12 Ruling 6: the plan-time gate reads the chain before the packet is
    // sent, and the life can advance between send and finalize. A table action
    // must not *land* after the life has passed the stage its table serves —
    // Ruling 1's clause, re-evaluated at the finalized slot.
    let position = classify(chain_facts(selected, &snapshot)?)?;
    if position > submission.stage {
        return Err(Error::new(format!(
            "the {} routing table action finalized after the life advanced to {}",
            submission.stage.label(),
            position.label()
        )));
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
        compute_units_consumed,
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

fn table_fee_for_message(
    rpc: &mut Rpc,
    message_base64: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<u64> {
    let fee = rpc
        .call(
            "getFeeForMessage",
            &json!([message_base64, {"commitment":"finalized"}]),
        )?
        .get("value")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getFeeForMessage omitted exact table fee"))?;
    expected_cluster.authenticate_finalized_fee(fee, "Resolution table fee quote")?;
    Ok(fee)
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

fn authenticate_table_dispatch_prestate(
    rpc: &mut Rpc,
    intent: &TableProvisionIntentV1,
    message: &Message,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let current_height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if current_height > intent.last_valid_block_height {
        return Err(Error::new(
            "durable table packet expired before dispatch or identical-byte recovery",
        ));
    }
    let (_, current_balances, current_accounts) =
        table_message_balances(rpc, message, intent.observation_slot)?;
    if current_balances != intent.pre_balances || current_accounts != intent.pre_accounts {
        return Err(Error::new(
            "table message prestate changed before dispatch or identical-byte recovery",
        ));
    }
    if table_fee_for_message(rpc, &intent.unsigned_message_base64, expected_cluster)?
        != intent.exact_fee_lamports
    {
        return Err(Error::new(
            "table exact fee differs from the canonical durable message",
        ));
    }
    Ok(())
}

fn reconcile_table_attempt(
    rpc: &mut Rpc,
    path: &Path,
    checkpoint: &ProducerCheckpointV1,
    selected: &SelectedInputV1,
    journal: &mut TableProvisionJournalV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<bool> {
    validate_table_signed_packet(journal)?;
    match table_transaction_status(rpc, journal, expected_cluster)? {
        TableTransactionStatusV1::Pending => Ok(false),
        TableTransactionStatusV1::Finalized {
            slot,
            fee_lamports,
            compute_units_consumed,
            post_balances,
        } => {
            let receipt = finish_table_submission(
                rpc,
                checkpoint,
                selected,
                journal,
                slot,
                fee_lamports,
                compute_units_consumed,
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

/// The freshness proof that accompanies a table provision: rebuild the action of
/// the stage the life is at *now*, from the same finalized snapshot the routing
/// decision reads (`docs/design/EVIDENCE_REFRESH_V1.md` §7.12 Ruling 5).
///
/// Its subject is the chain's position, not the table being written. Provisioning
/// is a sequence — a fresh life writes all three tables while still at Submit —
/// and a later stage's action is unbuildable from a snapshot that predates the
/// accounts that stage's predecessor creates. At the mid-life case this exists
/// for, the position and the table's stage coincide.
///
/// `Complete` is unreachable here: Ruling 1 refuses every routing stage first.
fn authenticate_stage_freshness(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    position: StageV1,
) -> Result<()> {
    match position {
        StageV1::Submit => provider_submit_report(selected, snapshot).map(drop),
        StageV1::Execute => provider_execute_report(selected, snapshot).map(drop),
        StageV1::Accept => core_terminal_accept_report(selected, snapshot).map(drop),
        StageV1::Reclaim => provider_reclaim_report(selected, snapshot).map(drop),
        StageV1::Complete => Ok(()),
    }
}

/// A life already under way must name itself before a routing table is written
/// for it, and the name is §7.10's receipt chain, byte-identical: each standing
/// receipt is re-derived from the cluster's own finalized packet and bound to
/// *this* input's market (§7.12 Rulings 3 and 4).
///
/// The binding cannot be the input digest — a re-plan changes `inputSha256` by
/// construction, which is the whole reason §7.10 exists.
fn authenticate_standing_life(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    path: Option<&PathBuf>,
    position: StageV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<Option<CheckpointV1>> {
    let Some(path) = path else {
        if position == StageV1::Submit {
            return Ok(None);
        }
        return Err(Error::new(format!(
            "a life already at {} may be provisioned only against its standing checkpoint; pass --standing-checkpoint",
            position.label()
        )));
    };
    let path = absolute(Some(path.clone()), "--standing-checkpoint")?;
    let bytes = fs::read(&path).map_err(|error| {
        Error::new(format!(
            "read standing checkpoint {}: {error}",
            path.display()
        ))
    })?;
    let standing: CheckpointV1 = serde_json::from_slice(&bytes)?;
    if standing.format != checkpoint_format(expected_cluster) {
        return Err(Error::new(
            "standing checkpoint format differs from this cluster",
        ));
    }
    // The chain, not the operator, decides how many receipts the standing
    // checkpoint owes: exactly the stage prefix below where the chain is.
    require_adoption_coverage(&standing.receipts, position)?;
    for receipt in &standing.receipts {
        authenticate_adopted_receipt(rpc, selected, receipt)?;
    }
    authenticate_receipt_prefix(&standing, expected_cluster)?;
    Ok(Some(standing))
}

/// A routing table may be written only for a stage the life has not passed and
/// whose packet is neither landed nor planned (§7.12 Rulings 1 and 2).
///
/// A v0 message names most of its accounts by index into a lookup table, so a
/// table that could change after a packet referencing it exists could change
/// what that packet means. `classify` answers for landed packets; the standing
/// checkpoint answers for signed-but-unsent ones, which chain cannot see.
fn require_table_stage_open(
    position: StageV1,
    stage: StageV1,
    standing: Option<&CheckpointV1>,
) -> Result<()> {
    let landed = standing.map_or_else(Vec::new, |checkpoint| {
        checkpoint
            .receipts
            .iter()
            .map(|receipt| receipt.stage)
            .collect()
    });
    let planned = standing
        .and_then(|checkpoint| checkpoint.stage_plan.as_ref())
        .map(|plan| plan.stage);
    table_stage_open(position, stage, &landed, planned)
}

/// The clause itself, over exactly the stage facts it reads: where the chain
/// is, which stages the standing checkpoint has landed, and which stage it has
/// a signed-but-unsent packet for.
fn table_stage_open(
    position: StageV1,
    stage: StageV1,
    landed: &[ReceiptStageV1],
    planned: Option<StageV1>,
) -> Result<()> {
    if position > stage {
        return Err(Error::new(format!(
            "the {} routing table may not be provisioned: the life is already at {}",
            stage.label(),
            position.label()
        )));
    }
    if let Some(receipt) = landed
        .iter()
        .find(|receipt| receipt.routing_stage() == stage)
    {
        return Err(Error::new(format!(
            "the {} routing table may not be provisioned: the standing checkpoint holds a landed {} receipt",
            stage.label(),
            receipt.label()
        )));
    }
    if let Some(plan) = planned.filter(|plan| plan.routing_stage() == stage) {
        return Err(Error::new(format!(
            "the {} routing table may not be provisioned: the standing checkpoint already plans an {} packet",
            stage.label(),
            plan.label()
        )));
    }
    Ok(())
}

fn run_table_provisioner(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
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
    let selected = authenticate_producer_checkpoint(&checkpoint, expected_cluster)?;
    let identity = producer_identity_sha256(&checkpoint)?;
    let mut journal = load_table_journal(&journal_path, &identity, expected_cluster)?;
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    expected_cluster.authenticate(&origin)?;
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
            DurablePhaseV1::SignedNotSubmitted => {
                if !arguments.execute {
                    println!("{}", serde_json::to_string_pretty(&journal)?);
                    return Ok(());
                }
                validate_table_signed_packet(&journal)?;
                let message = validate_table_intent(&checkpoint, &selected, intent)?;
                authenticate_table_dispatch_prestate(&mut rpc, intent, &message, expected_cluster)?;
                journal.phase = DurablePhaseV1::Dispatching;
                write_json(&journal_path, &journal)?;
            }
            DurablePhaseV1::Dispatching => {
                if reconcile_table_attempt(
                    &mut rpc,
                    &journal_path,
                    &checkpoint,
                    &selected,
                    &mut journal,
                    expected_cluster,
                )? {
                    println!("{}", serde_json::to_string_pretty(&journal)?);
                    return Ok(());
                }
                if !arguments.execute {
                    println!("{}", serde_json::to_string_pretty(&journal)?);
                    return Ok(());
                }
            }
            DurablePhaseV1::Submitted => {
                reconcile_table_attempt(
                    &mut rpc,
                    &journal_path,
                    &checkpoint,
                    &selected,
                    &mut journal,
                    expected_cluster,
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
                    expected_cluster,
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
        if journal.phase == DurablePhaseV1::Dispatching {
            let intent = journal.intent.as_ref().expect("checked intent");
            let message = validate_table_intent(&checkpoint, &selected, intent)?;
            authenticate_table_dispatch_prestate(&mut rpc, intent, &message, expected_cluster)?;
            submit_journaled_table_transaction(&mut rpc, &journal)?;
            journal.phase = DurablePhaseV1::Submitted;
            write_json(&journal_path, &journal)?;
            reconcile_table_attempt(
                &mut rpc,
                &journal_path,
                &checkpoint,
                &selected,
                &mut journal,
                expected_cluster,
            )?;
            println!("{}", serde_json::to_string_pretty(&journal)?);
            return Ok(());
        }
    }
    if journal.intent.is_none() {
        let snapshot = observe(&mut rpc, &selected, StageV1::Submit, 0)?;
        // §7.12 replaces the per-life gate with a per-stage one. The life's
        // position on chain, the standing checkpoint that names the life, and
        // the freshness proof for that position are all read before any stage
        // is chosen; the stage-open clause then guards the write itself.
        let position = classify(chain_facts(&selected, &snapshot)?)?;
        let standing = authenticate_standing_life(
            &mut rpc,
            &selected,
            arguments.standing_checkpoint.as_ref(),
            position,
            expected_cluster,
        )?;
        authenticate_current_deployments(&selected, &snapshot)?;
        authenticate_selected_pyth_release(&selected, &snapshot, true, expected_cluster)?;
        authenticate_stage_freshness(&selected, &snapshot, position)?;
        let Some((stage, action, instruction)) = next_table_provision(&checkpoint, &snapshot)?
        else {
            println!("{}", serde_json::to_string_pretty(&journal)?);
            return Ok(());
        };
        require_table_stage_open(position, stage, standing.as_ref())?;
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
        let exact_fee_lamports =
            table_fee_for_message(&mut rpc, &message_base64, expected_cluster)?;
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
    authenticate_table_dispatch_prestate(&mut rpc, &intent, &message, expected_cluster)?;
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
    authenticate_table_dispatch_prestate(
        &mut rpc,
        &intent,
        &transaction.message,
        expected_cluster,
    )?;
    // Dispatching is durable before transport. Recovery polls first and may
    // resend only these identical authenticated bytes under the same signature.
    journal.phase = DurablePhaseV1::Dispatching;
    write_json(&journal_path, &journal)?;
    submit_journaled_table_transaction(&mut rpc, &journal)?;
    // Submitted is written only after the transport call returned the exact
    // durable signature. Recovery from here is permanently poll-only.
    journal.phase = DurablePhaseV1::Submitted;
    write_json(&journal_path, &journal)?;
    reconcile_table_attempt(
        &mut rpc,
        &journal_path,
        &checkpoint,
        &selected,
        &mut journal,
        expected_cluster,
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

fn core_terminal_accept_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ResolutionAdmitTerminalReportV3> {
    let market = selected.account("market")?;
    let report = build_resolution_admit_terminal_v3(&ResolutionAdmitTerminalSnapshotV3 {
        market: snapshot.observed(market, "Market")?,
        activation_cache: snapshot
            .observed(selected.account("activation_cache")?, "activation cache")?,
        registry_program: snapshot
            .observed(selected.account("registry_program")?, "Registry program")?,
        core_program: snapshot.observed(selected.account("core_program")?, "Core program")?,
        core_programdata: snapshot
            .observed(selected.account("core_programdata")?, "Core ProgramData")?,
        resolution_program: snapshot.observed(
            selected.account("resolution_program")?,
            "Resolution program",
        )?,
        resolution_programdata: snapshot.observed(
            selected.account("resolution_programdata")?,
            "Resolution ProgramData",
        )?,
        source_material: snapshot
            .observed(selected.account("source_material")?, "SourceMaterial")?,
        source_material_staging: snapshot
            .observed_or_vacant(selected.account("source_material_staging")?)?,
        capability_manifest: snapshot.observed(
            selected.account("capability_manifest")?,
            "CapabilityManifest",
        )?,
        capability_manifest_staging: snapshot
            .observed_or_vacant(selected.account("capability_manifest_staging")?)?,
        source_state: snapshot.observed(selected.account("source_state")?, "Source state")?,
        funding_ledger: snapshot.observed(
            selected.account("funding_ledger")?,
            "Resolution funding ledger",
        )?,
        certificate: snapshot.observed(selected.account("certificate")?, "terminal certificate")?,
        rent_sysvar: snapshot.observed(sysvar::rent::ID, "Rent sysvar")?,
        product_raw: snapshot.observed(selected.account("product")?, "Product")?,
        product_staging: snapshot.observed_or_vacant(selected.account("product_staging")?)?,
        result_domain_raw: snapshot.observed(selected.account("result_domain")?, "ResultDomain")?,
        result_domain_staging: snapshot
            .observed_or_vacant(selected.account("result_domain_staging")?)?,
        portfolio_raw: snapshot.observed(selected.account("portfolio")?, "Portfolio")?,
        portfolio_staging: snapshot.observed_or_vacant(selected.account("portfolio_staging")?)?,
    })
    .map_err(|error| Error::new(format!("Core terminal accept builder: {error:?}")))?;
    if report.instruction.program_id != selected.account("core_program")?
        || report
            .instruction
            .accounts
            .first()
            .is_none_or(|meta| meta.pubkey != report.caller_authority || meta.is_signer)
        || report
            .instruction
            .accounts
            .get(1)
            .is_none_or(|meta| meta.pubkey != market)
        || report
            .instruction
            .accounts
            .iter()
            .any(|meta| meta.is_signer)
        || report.terminal_sequence != selected.terminal_sequence
    {
        return Err(Error::new(
            "Core terminal accept report changed its producer, caller, Market, signer, or sequence boundary",
        ));
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

/// Give a provider-transport geometry probe the margin its error cannot carry.
///
/// `ProviderTransportTransactionErrorV3::Routing(PacketTooLarge)` says the bare
/// action does not fit and cannot say by how much, which is the difference
/// between a wall that can be acted on and one that can only be reported.
/// EVIDENCE_REFRESH_V1 §7.4 fixed exactly this for the packet `prepare_stage`
/// ships and named the same blind spot in these probes; this is that remedy
/// applied where the probe lives.
fn sized_provider_geometry_refusal(
    stage: StageV1,
    payer: Pubkey,
    report: &ProviderTransportReportV3,
    table: &ObservedAccount,
    error: &ProviderTransportTransactionErrorV3,
) -> Error {
    if matches!(
        error,
        ProviderTransportTransactionErrorV3::Routing(
            dclutch_versioned_message_operator::Error::PacketTooLarge
        )
    ) {
        if let Ok(wire_bytes) = dclutch_versioned_message_operator::measure_v0_wire_bytes(
            payer,
            std::slice::from_ref(&report.instruction),
            Hash::new_from_array(GEOMETRY_BLOCKHASH),
            report.observation,
            std::slice::from_ref(table),
        ) {
            return Error::new(format!(
                "provider {} v0 geometry: the bare action alone is {wire_bytes} wire bytes, \
                 {} over the {} packet limit, before the ComputeBudget prefix the sent packet adds",
                stage.label(),
                wire_bytes.saturating_sub(dclutch_versioned_message_operator::PACKET_DATA_BYTES),
                dclutch_versioned_message_operator::PACKET_DATA_BYTES,
            ));
        }
    }
    Error::new(format!("provider {} v0 geometry: {error:?}", stage.label()))
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
            .map_err(|error| {
                sized_provider_geometry_refusal(stage, selected.submitter, &report, table, &error)
            })?;
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
            // Execute cannot carry its own top-up, and the reason is not only
            // bytes. The transfer's source has to be the account that owes the
            // certificate its rent — the resolver — and a System transfer makes
            // its source a writable signer, which is exactly the privilege
            // Core's frame refuses at that index. Bundling therefore keeps the
            // resolver writable no matter who pays the fee (§7.13, measured:
            // the distinct-payer bundled variant still reports
            // `numReadonlySigned: 0`, and at 1,299 bytes it does not fit
            // either). So the top-up leaves the packet and becomes its own
            // accounted act, and this refusal says exactly what that act is.
            if let Some(transfer) = vacant_top_up(
                snapshot,
                selected.account("certificate")?,
                certificate_rent,
                "terminal certificate",
            )? {
                return Err(Error::new(format!(
                    "execute requires the terminal certificate funded ahead of the packet: \
                     send exactly {} lamports to {} in its own transaction. The Execute frame \
                     pins the resolver readonly, and a bundled top-up would make it writable",
                    transfer.lamports, transfer.destination,
                )));
            }
            let compiled = compile_provider_execute_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(table),
                selected.payer,
            )
            .map_err(|error| {
                sized_provider_geometry_refusal(stage, selected.payer, &report, table, &error)
            })?;
            (
                report.instruction,
                compiled.required_signers,
                selected.account("source_state")?,
            )
        }
        StageV1::Accept => {
            let report = core_terminal_accept_report(selected, snapshot)?;
            (
                report.instruction,
                vec![selected.resolver],
                selected.account("market")?,
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
                selected.payer,
            )
            .map_err(|error| {
                sized_provider_geometry_refusal(stage, selected.payer, &report, table, &error)
            })?;
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
        // Execute and Reclaim name the resolver as a readonly instruction
        // signer, so the fee payer is a second, separate key and sorts first.
        // Accept names no signer at all — its caller authority is a readonly
        // non-signer PDA — so the resolver may pay there and the frame is
        // indifferent.
        StageV1::Execute | StageV1::Reclaim => vec![selected.payer, selected.resolver],
        StageV1::Accept => vec![selected.resolver],
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
    expected_cluster: ExpectedClusterV1,
) -> Result<PreparedStageV1> {
    authenticate_current_deployments(selected, snapshot)?;
    match stage {
        StageV1::Submit | StageV1::Execute => {
            authenticate_selected_pyth_release(selected, snapshot, true, expected_cluster)?;
        }
        StageV1::Accept | StageV1::Reclaim => {
            authenticate_selected_pyth_release(selected, snapshot, false, expected_cluster)?;
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
    let payer = *required_signers
        .first()
        .ok_or_else(|| Error::new("stage has no fee payer"))?;
    let routed = dclutch_versioned_message_operator::compile_v0_message(
        payer,
        &bounded,
        blockhash,
        snapshot.observation,
        std::slice::from_ref(&table),
    )
    .map_err(|error| {
        // A packet refusal that cannot name its own margin cannot be acted on:
        // it does not say whether the route is a few bytes over or structurally
        // impossible. The operator's error carries no payload, so measure the
        // identical message and report the overage.
        if error == dclutch_versioned_message_operator::Error::PacketTooLarge {
            if let Ok(wire_bytes) = dclutch_versioned_message_operator::measure_v0_wire_bytes(
                payer,
                &bounded,
                blockhash,
                snapshot.observation,
                std::slice::from_ref(&table),
            ) {
                return Error::new(format!(
                    "{} atomic stage geometry: {wire_bytes} wire bytes is {} over the {} packet limit, \
                     carrying {} bundled top-up transfer(s)",
                    stage.label(),
                    wire_bytes.saturating_sub(
                        dclutch_versioned_message_operator::PACKET_DATA_BYTES
                    ),
                    dclutch_versioned_message_operator::PACKET_DATA_BYTES,
                    transfers.len(),
                ));
            }
        }
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
    let exact_fee_lamports = table_fee_for_message(rpc, &message_base64, expected_cluster)?;
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
    payer_keypair: Option<PathBuf>,
    update_keypair: Option<PathBuf>,
    through: Option<StageV1>,
    adopt_receipts: Option<PathBuf>,
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
                "--payer-keypair" => set_once(
                    &mut parsed.payer_keypair,
                    PathBuf::from(value),
                    "--payer-keypair",
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
                "--adopt-receipts" => set_once(
                    &mut parsed.adopt_receipts,
                    PathBuf::from(value),
                    "--adopt-receipts",
                )?,
                _ => return Err(Error::new(format!("unknown flagship argument: {argument}"))),
            }
        }
        if !parsed.execute
            && (parsed.submitter_keypair.is_some()
                || parsed.resolver_keypair.is_some()
                || parsed.payer_keypair.is_some()
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
     [--standing-checkpoint ABSOLUTE_JSON] \
     [--execute --authority-keypair ABSOLUTE_JSON]\n\n  \
     dclutch-local-successor-bootstrap flagship-resolution-v1 --rpc-url URL \
     --i-mean-devnet DEVNET_GENESIS --input ABSOLUTE_JSON \
     --checkpoint ABSOLUTE_JSON [--through submit|execute|reclaim|complete] \
     [--adopt-receipts ABSOLUTE_JSON] \
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

pub(crate) fn owned_loopback_usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap \
     local-private-validator-flagship-resolution-v1 --produce-input \
     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \
     --campaign-evidence ABSOLUTE_JSON --pyth-facts ABSOLUTE_JSON \
     --producer-checkpoint ABSOLUTE_JSON --output ABSOLUTE_JSON\n\n  \
     dclutch-local-successor-bootstrap \
     local-private-validator-flagship-resolution-v1 --provision-tables \
     --rpc-url http://127.0.0.1:PORT --producer-checkpoint ABSOLUTE_JSON \
     --table-journal ABSOLUTE_JSON [--standing-checkpoint ABSOLUTE_JSON] \
     [--execute --authority-keypair ABSOLUTE_JSON]\n\n  \
     dclutch-local-successor-bootstrap \
     local-private-validator-flagship-resolution-v1 \
     --rpc-url http://127.0.0.1:PORT --input ABSOLUTE_JSON \
     --checkpoint ABSOLUTE_JSON [--through submit|execute|reclaim|complete] \
     [--execute --submitter-keypair ABSOLUTE_JSON --resolver-keypair ABSOLUTE_JSON \
     --update-keypair ABSOLUTE_JSON]\n\nThis command exposes the same authenticated provider \
     lifecycle to a validator launched and owned by the private lifecycle runner. It accepts \
     only 127.0.0.1 with an explicit permitted port, requires the distinct owned-loopback input, \
     checkpoint, table-journal, and pinned local Pyth release domains, and refuses every external \
     origin, including devnet and mainnet-beta. Without --execute it remains key-free and \
     read-only."
}

struct ProviderFinalizedTransactionV1 {
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
    post_balances: Vec<u64>,
    return_data_base64: String,
}

/// May this stage plan be discarded and re-planned?
///
/// Only when all three hold: the phase is one that has not handed a packet to
/// the permanently poll-only recovery path, the cluster's finalized block
/// height is past the plan's `last_valid_block_height`, and — for a plan that
/// was actually signed — the packet is not on chain.
///
/// The last two together are what make this safe rather than convenient. An
/// expired blockhash is refused by the network, not by this tool, so a packet
/// that is both expired and absent from finalized history has no future in
/// which it lands.
fn discardable_expired_plan_v1(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
) -> Result<bool> {
    if matches!(
        plan.phase,
        DurablePhaseV1::Submitted | DurablePhaseV1::Finalized
    ) {
        return Ok(false);
    }
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height <= plan.last_valid_block_height {
        return Ok(false);
    }
    // An unsigned plan has no packet at all, so there is nothing to look for.
    if plan.expected_signature.is_none() {
        return Ok(true);
    }
    Ok(provider_transaction_status(rpc, selected, plan)?.is_none())
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
    let compute_units_consumed = finalized_compute_units(meta, "provider transaction")?;
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
    let return_data_base64 = if plan.stage == StageV1::Accept {
        if meta.get("returnData").is_some_and(|value| !value.is_null()) {
            return Err(Error::new(
                "Core terminal accept unexpectedly emitted returnData",
            ));
        }
        String::new()
    } else {
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
        {
            return Err(Error::new(
                "provider finalized returnData changed producer, encoding, or bytes",
            ));
        }
        return_data_base64
    };
    if fee_lamports != plan.exact_fee_lamports
        || pre_balances != plan.pre_balances
        || post_balances.len() != plan.resolved_account_keys.len()
        || parse_addresses("writable")? != plan.loaded_writable
        || parse_addresses("readonly")? != plan.loaded_readonly
    {
        return Err(Error::new(
            "provider finalized fee, balances, or loaded addresses changed",
        ));
    }
    Ok(Some(ProviderFinalizedTransactionV1 {
        slot: value
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("finalized provider transaction omitted slot"))?,
        fee_lamports,
        compute_units_consumed,
        post_balances,
        return_data_base64,
    }))
}

/// The Clock sysvar's five decoded fields.
///
/// Every one of them advances, so a durable plan cannot pin any of them by
/// bytes and still be sendable on a cluster whose slots move. Each earns a
/// bound instead (EVIDENCE_REFRESH_V1 §7.6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClockReadingV1 {
    slot: u64,
    epoch_start_timestamp: i64,
    epoch: u64,
    leader_schedule_epoch: u64,
    unix_timestamp: i64,
}

impl ClockReadingV1 {
    fn decode(stage: StageV1, data: &[u8]) -> Result<Self> {
        let field = |start: usize| -> Result<[u8; 8]> {
            data.get(start..start.saturating_add(8))
                .and_then(|slice| <[u8; 8]>::try_from(slice).ok())
                .ok_or_else(|| Self::width_refusal(stage))
        };
        if data.len() != CLOCK_SYSVAR_BYTES_V1 {
            return Err(Self::width_refusal(stage));
        }
        Ok(Self {
            slot: u64::from_le_bytes(field(0)?),
            epoch_start_timestamp: i64::from_le_bytes(field(8)?),
            epoch: u64::from_le_bytes(field(16)?),
            leader_schedule_epoch: u64::from_le_bytes(field(24)?),
            unix_timestamp: i64::from_le_bytes(field(32)?),
        })
    }

    fn width_refusal(stage: StageV1) -> Error {
        Error::new(format!(
            "provider {} clock sysvar is not the exact {CLOCK_SYSVAR_BYTES_V1}-byte Clock layout",
            stage.label()
        ))
    }
}

/// The interval of clock readings under which this exact plan is still the plan
/// the producer's `validate_observation_fields` admitted.
///
/// `upper` is open where the stage reads no clock at all; the transaction's own
/// freshness is bounded there by `last_valid_block_height`, not by this band
/// (EVIDENCE_REFRESH_V1 §7.6, Ruling 4).
#[derive(Clone, Copy, Debug)]
struct ClockBandV1 {
    lower: i64,
    upper: Option<i64>,
}

/// Read one band endpoint's account out of the plan's own prestate.
///
/// Band endpoints come only from rows that still carry a byte-exact pin, which
/// is what makes the band un-widenable: the comparison this change keeps has
/// already refused any alteration of these rows on the original string before
/// the band is derived (EVIDENCE_REFRESH_V1 §7.6, Ruling 3).
fn pinned_band_row(plan: &StagePlanV1, key: Pubkey, label: &str) -> Result<Vec<u8>> {
    let state = plan.pre_accounts.get(&key.to_string()).ok_or_else(|| {
        Error::new(format!(
            "provider {} clock band lost its pinned {label} row",
            plan.stage.label()
        ))
    })?;
    BASE64
        .decode(&state.data_base64)
        .map_err(|error| Error::new(format!("provider prestate base64: {error}")))
}

fn admissible_clock_band(selected: &SelectedInputV1, plan: &StagePlanV1) -> Result<ClockBandV1> {
    let planned = plan.observation_unix_timestamp;
    match plan.stage {
        // `validate_observation_fields` admits when the publication sits inside
        // `[now - max_age, now + max_future_skew]`. Solved for the clock rather
        // than the publication, that is the closed interval below, and it is
        // exactly the band this plan was admitted under.
        StageV1::Submit | StageV1::Execute => {
            let window = WindowSpecV1::decode(&pinned_band_row(
                plan,
                selected.account("window")?,
                "WindowSpec",
            )?)
            .map_err(|error| Error::new(format!("WindowSpec: {error:?}")))?;
            let publication = parse_price_feed_message(&selected.post_update_body)?.publish_time;
            let lower = publication
                .checked_sub(i64::from(window.max_future_skew_seconds()))
                .ok_or_else(|| Error::new("provider clock band lower bound overflow"))?;
            let upper = publication
                .checked_add(i64::from(window.max_age_seconds()))
                .ok_or_else(|| Error::new("provider clock band upper bound overflow"))?;
            Ok(ClockBandV1 {
                lower: lower.max(planned),
                upper: Some(upper),
            })
        }
        // Core terminal accept reads no clock, so monotonicity is the whole of
        // its band; inventing an upper bound would be improvisation.
        StageV1::Accept => Ok(ClockBandV1 {
            lower: planned,
            upper: None,
        }),
        StageV1::Reclaim => {
            let lifecycle = ProviderUpdateLifecycleV3::decode(&pinned_band_row(
                plan,
                lifecycle_address(selected)?,
                "provider lifecycle",
            )?)
            .map_err(|error| Error::new(format!("provider lifecycle: {error:?}")))?;
            Ok(ClockBandV1 {
                lower: lifecycle.reclaim_after_unix_seconds.max(planned),
                upper: None,
            })
        }
        StageV1::Complete => Err(Error::new("complete has no stage plan")),
    }
}

/// Authenticate the one resolved row the runtime advances unconditionally.
///
/// Owner, lamports, executable and data width keep the original byte-pin and
/// the original refusal; only the Clock's five decoded fields are released, and
/// each is replaced by a bound rather than dropped.
/// Compare the resolved prestate the plan recorded against a fresh reading.
///
/// Every row but the Clock is byte-exact on the original refusal. The band is
/// taken lazily so a stage whose message never resolves the Clock never derives
/// one.
fn authenticate_prestate_rows(
    plan: &StagePlanV1,
    observed: BTreeMap<String, DurableAccountStateV1>,
    band: impl FnOnce() -> Result<ClockBandV1>,
) -> Result<()> {
    let clock_key = sysvar::clock::ID.to_string();
    let mut observed_rows = observed;
    let mut planned_rows = plan.pre_accounts.clone();
    let observed_clock = observed_rows.remove(&clock_key);
    let planned_clock = planned_rows.remove(&clock_key);
    if observed_rows != planned_rows {
        // The relation is unchanged — every row is still required byte-exact.
        // It now names the rows that moved. A prestate pin spanning dozens of
        // accounts behind one string is a refusal that can be read and not
        // acted on, which is the lesson §7.4 recorded and §7.14.1 had to learn
        // a second time.
        return Err(Error::new(format!(
            "provider full resolved account prestate changed: {}",
            describe_prestate_difference_v1(&planned_rows, &observed_rows)
        )));
    }
    match (planned_clock, observed_clock) {
        (Some(planned_clock), Some(observed_clock)) => {
            authenticate_prestate_clock(plan, &planned_clock, &observed_clock, band()?)
        }
        // A stage whose message never resolves the Clock has nothing to release.
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Error::new(
            "provider full resolved account prestate changed: the Clock is resolved now and was \
             not when this stage was planned",
        )),
        (Some(_), None) => Err(Error::new(
            "provider full resolved account prestate changed: the Clock was resolved when this \
             stage was planned and is absent now",
        )),
    }
}

/// Name the rows a prestate comparison refused on, and what moved in each.
///
/// Byte-equality is the whole relation and it is not relaxed here; this only
/// reports which of the pinned accounts stopped satisfying it, so a refusal can
/// be acted on rather than merely reported.
fn describe_prestate_difference_v1(
    planned: &BTreeMap<String, DurableAccountStateV1>,
    observed: &BTreeMap<String, DurableAccountStateV1>,
) -> String {
    let mut notes = Vec::new();
    for (address, planned_row) in planned {
        let Some(observed_row) = observed.get(address) else {
            notes.push(format!(
                "{address} was pinned when planned and is absent now"
            ));
            continue;
        };
        if planned_row == observed_row {
            continue;
        }
        let mut fields = Vec::new();
        if planned_row.owner != observed_row.owner {
            fields.push(format!(
                "owner {} -> {}",
                planned_row.owner, observed_row.owner
            ));
        }
        if planned_row.lamports != observed_row.lamports {
            fields.push(format!(
                "lamports {} -> {}",
                planned_row.lamports, observed_row.lamports
            ));
        }
        if planned_row.executable != observed_row.executable {
            fields.push(format!(
                "executable {} -> {}",
                planned_row.executable, observed_row.executable
            ));
        }
        if planned_row.data_base64 != observed_row.data_base64 {
            fields.push(format!(
                "data ({} -> {} base64 chars)",
                planned_row.data_base64.len(),
                observed_row.data_base64.len()
            ));
        }
        notes.push(format!("{address}: {}", fields.join(", ")));
    }
    for address in observed.keys() {
        if !planned.contains_key(address) {
            notes.push(format!("{address} is pinned now and was not when planned"));
        }
    }
    if notes.is_empty() {
        return "the row maps differ but no row reports a difference".into();
    }
    notes.join("; ")
}

fn authenticate_prestate_clock(
    plan: &StagePlanV1,
    planned: &DurableAccountStateV1,
    observed: &DurableAccountStateV1,
    band: ClockBandV1,
) -> Result<()> {
    let decode = |state: &DurableAccountStateV1| -> Result<Vec<u8>> {
        BASE64
            .decode(&state.data_base64)
            .map_err(|error| Error::new(format!("provider prestate base64: {error}")))
    };
    let planned_data = decode(planned)?;
    let observed_data = decode(observed)?;
    if planned.owner != observed.owner
        || planned.lamports != observed.lamports
        || planned.executable != observed.executable
        || planned_data.len() != observed_data.len()
    {
        return Err(Error::new(
            "provider full resolved account prestate changed",
        ));
    }
    let before = ClockReadingV1::decode(plan.stage, &planned_data)?;
    let after = ClockReadingV1::decode(plan.stage, &observed_data)?;
    let monotone = |field: &str, planned_value: i128, observed_value: i128| -> Result<()> {
        if observed_value < planned_value {
            return Err(Error::new(format!(
                "provider {} clock rewound: {field} {observed_value} is behind the planned {planned_value}",
                plan.stage.label()
            )));
        }
        Ok(())
    };
    monotone(
        "slot",
        i128::from(before.slot.max(plan.observation_slot)),
        i128::from(after.slot),
    )?;
    monotone("epoch", i128::from(before.epoch), i128::from(after.epoch))?;
    monotone(
        "leaderScheduleEpoch",
        i128::from(before.leader_schedule_epoch),
        i128::from(after.leader_schedule_epoch),
    )?;
    monotone(
        "epochStartTimestamp",
        i128::from(before.epoch_start_timestamp),
        i128::from(after.epoch_start_timestamp),
    )?;
    monotone(
        "unixTimestamp",
        i128::from(before.unix_timestamp),
        i128::from(after.unix_timestamp),
    )?;
    if after.unix_timestamp < band.lower
        || band.upper.is_some_and(|upper| after.unix_timestamp > upper)
    {
        return Err(Error::new(format!(
            "provider {} clock {} is outside the admissible band [{}, {}] this plan was admitted under",
            plan.stage.label(),
            after.unix_timestamp,
            band.lower,
            band.upper
                .map_or_else(|| "unbounded".to_string(), |upper| upper.to_string()),
        )));
    }
    Ok(())
}

fn authenticate_provider_prestate(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    plan: &StagePlanV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let snapshot = observe(rpc, selected, plan.stage, plan.observation_slot)?;
    if classify(chain_facts(selected, &snapshot)?)? != plan.stage {
        return Err(Error::new(
            "provider stage changed before signing or sending",
        ));
    }
    authenticate_current_deployments(selected, &snapshot)?;
    authenticate_selected_pyth_release(
        selected,
        &snapshot,
        matches!(plan.stage, StageV1::Submit | StageV1::Execute),
        expected_cluster,
    )?;
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
    // Thirty-eight of the thirty-nine rows keep byte-equality on the original
    // refusal. The Clock is the one account no actor controls and the runtime
    // advances every slot, so pinning its contents buys no anti-forgery value
    // and guarantees a liveness failure anywhere slots move; it is released
    // here and bounded instead (EVIDENCE_REFRESH_V1 §7.6).
    authenticate_prestate_rows(plan, states, || admissible_clock_band(selected, plan))?;
    // Expiry is checked before the fee probe deliberately. `getFeeForMessage`
    // answers null for a blockhash the cluster has forgotten, so an expired
    // durable plan otherwise refuses with `getFeeForMessage omitted exact table
    // fee` — a refusal that names the probe rather than the cause.
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height > plan.last_valid_block_height {
        return Err(Error::new(
            "durable provider blockhash expired before key access",
        ));
    }
    if table_fee_for_message(rpc, &plan.message_base64, expected_cluster)?
        != plan.exact_fee_lamports
    {
        return Err(Error::new(
            "provider exact fee differs from the canonical durable message",
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

fn require_same_account_state(
    before: &ObservedAccount,
    after: &ObservedAccount,
    purpose: &str,
) -> Result<()> {
    if before.key != after.key
        || before.owner != after.owner
        || before.lamports != after.lamports
        || before.executable != after.executable
        || before.data != after.data
    {
        return Err(Error::new(format!(
            "Core terminal accept changed immutable {purpose} state"
        )));
    }
    Ok(())
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
            let after0 = post.observed_or_vacant(key(0)?)?;
            let after1 = post.observed_or_vacant(key(1)?)?;
            let after2 = post.observed_or_vacant(key(2)?)?;
            let after34 = post.observed_or_vacant(key(34)?)?;
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
            let after2 = post.observed_or_vacant(key(2)?)?;
            let after3 = post.observed_or_vacant(key(3)?)?;
            let after4 = post.observed_or_vacant(key(4)?)?;
            let after37 = post.observed_or_vacant(key(37)?)?;
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
        StageV1::Accept => {
            if !return_data.is_empty() {
                return Err(Error::new(
                    "Core terminal accept carried prohibited return data",
                ));
            }
            let before_market = durable_pre_account(plan, key(1)?)?;
            let after_market = post.observed_or_vacant(key(1)?)?;
            if before_market.owner != after_market.owner
                || before_market.executable != after_market.executable
                || before_market.data == after_market.data
            {
                return Err(Error::new(
                    "Core terminal accept did not exclusively advance its Market state",
                ));
            }
            for (index, purpose) in [
                (12, "Source"),
                (13, "Resolution funding ledger"),
                (14, "Resolution certificate"),
            ] {
                require_same_account_state(
                    &durable_pre_account(plan, key(index)?)?,
                    &post.observed_or_vacant(key(index)?)?,
                    purpose,
                )?;
            }
            let replay = core_terminal_accept_report(selected, post)?;
            if replay.instruction.program_id != instruction.program_id
                || replay.instruction.accounts != instruction.accounts
                || replay.terminal_sequence != selected.terminal_sequence
                || replay.role_request_digest == [0; 32]
                || replay.outcome_count == 0
            {
                return Err(Error::new(
                    "Core terminal accept replay changed its account frame or semantic receipt",
                ));
            }
        }
        StageV1::Reclaim => {
            let before1 = durable_pre_account(plan, key(1)?)?;
            let before2 = durable_pre_account(plan, key(2)?)?;
            let before3 = durable_pre_account(plan, key(3)?)?;
            let before4 = durable_pre_account(plan, key(4)?)?;
            let after1 = post.observed_or_vacant(key(1)?)?;
            let after2 = post.observed_or_vacant(key(2)?)?;
            let after3 = post.observed_or_vacant(key(3)?)?;
            let after4 = post.observed_or_vacant(key(4)?)?;
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
    expected_cluster: ExpectedClusterV1,
) -> Result<StageReceiptV1> {
    expected_cluster
        .authenticate_finalized_fee(finalized.fee_lamports, "Resolution finalized transaction")?;
    let post = observe_stage_poststate_v1(rpc, selected, plan, finalized.slot)?;
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
        StageV1::Execute => StageV1::Accept,
        StageV1::Accept => StageV1::Reclaim,
        StageV1::Reclaim => StageV1::Complete,
        StageV1::Complete => return Err(Error::new("complete has no provider finalization")),
    };
    if classify(chain_facts(selected, &post)?)? != expected {
        return Err(Error::new(
            "provider packet did not produce its exact next stage",
        ));
    }
    if matches!(
        plan.stage,
        StageV1::Execute | StageV1::Accept | StageV1::Reclaim
    ) {
        authenticate_current_deployments(selected, &post)?;
        authenticate_selected_pyth_release(
            selected,
            &post,
            plan.stage == StageV1::Execute,
            expected_cluster,
        )?;
    }
    if matches!(plan.stage, StageV1::Accept | StageV1::Reclaim) {
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
            StageV1::Accept => 0,
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
        stage: ReceiptStageV1::from_stage(plan.stage)?,
        signature: plan.expected_signature.clone().unwrap_or_default(),
        slot: finalized.slot,
        fee_lamports: finalized.fee_lamports,
        compute_units_consumed: finalized.compute_units_consumed,
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

/// Expose only Core's terminal-accept packet to the owned-loopback crash
/// campaign. The plan's message digest is its effective intent digest: plan
/// validation re-derives that message from the exact transfer/action list,
/// ALT, account frame, blockhash, fee, and prestate before this helper runs.
fn park_core_terminal_accept_chaos_boundary_v1(
    expected_cluster: ExpectedClusterV1,
    checkpoint_path: &Path,
    plan: &StagePlanV1,
    boundary: BoundaryV1,
) -> Result<()> {
    if !is_core_terminal_accept_chaos_target_v1(expected_cluster, plan.stage) {
        return Ok(());
    }
    plan.validate()?;
    chaos_fault::park_if_armed_v1(
        expected_cluster.evidence_label(),
        "core-terminal-accept",
        boundary,
        checkpoint_path,
        &plan.message_sha256,
        plan.signed_transaction_sha256
            .as_deref()
            .ok_or_else(|| Error::new("Core terminal accept chaos seam omitted packet digest"))?,
        plan.expected_signature
            .as_deref()
            .ok_or_else(|| Error::new("Core terminal accept chaos seam omitted signature"))?,
    )
}

const fn is_core_terminal_accept_chaos_target_v1(
    expected_cluster: ExpectedClusterV1,
    stage: StageV1,
) -> bool {
    matches!(expected_cluster, ExpectedClusterV1::OwnedLoopback) && matches!(stage, StageV1::Accept)
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    run_with_expected_cluster(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run_with_expected_cluster(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn run_with_expected_cluster(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    if arguments
        .iter()
        .any(|argument| argument == "--produce-input")
    {
        return run_producer(arguments, expected_cluster);
    }
    if arguments
        .iter()
        .any(|argument| argument == "--provision-tables")
    {
        return run_table_provisioner(arguments, expected_cluster);
    }
    if arguments
        .iter()
        .any(|argument| argument == "--reprovision-execute-table")
    {
        return run_execute_table_reprovision(arguments, expected_cluster);
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
    let selected = SelectedInputV1::parse(&input, expected_cluster)?;
    let input_sha256 = hex(&Sha256::digest(&input_bytes));
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    expected_cluster.authenticate(&origin)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    let mut checkpoint = load_checkpoint(&checkpoint_path, &input_sha256, expected_cluster)?;
    if arguments.adopt_receipts.is_some() {
        let prior = absolute(arguments.adopt_receipts.clone(), "--adopt-receipts")?;
        // The chain, not the operator, decides how many receipts this resume
        // owes (§7.10 Ruling 5). Classify first, then demand exactly that prefix.
        let initial = observe(&mut rpc, &selected, StageV1::Submit, 0)?;
        let stage = classify(chain_facts(&selected, &initial)?)?;
        adopt_prior_receipts(
            &mut rpc,
            &selected,
            &mut checkpoint,
            &prior,
            stage,
            expected_cluster,
        )?;
        write_checkpoint(&checkpoint_path, &checkpoint)?;
    }
    let through = arguments.through.unwrap_or(StageV1::Complete);
    loop {
        if let Some(plan) = checkpoint.stage_plan.as_mut() {
            plan.validate()?;
            // A plan whose blockhash the cluster will no longer accept, and
            // whose packet is not on chain, can never land: Solana refuses a
            // transaction past its `lastValidBlockHeight` permanently. Such a
            // plan is not a packet in flight, it is a dead one, and every
            // authentication below it — the prestate pin first — refuses
            // forever while naming something other than the cause. That is
            // §7.7's lesson one frame further out than §7.7 applied it: the
            // expiry is checked before the fee probe there, and it belongs
            // before the prestate here for exactly the same reason.
            //
            // Discarding it re-plans the same stage against current chain. It
            // cannot double-send, because the packet being discarded is
            // unlandable by consensus rule rather than by local intent.
            //
            // Submitted is deliberately excluded and stays permanently
            // poll-only: that phase's whole contract is that it never reasons
            // about a packet it may have broadcast.
            if discardable_expired_plan_v1(&mut rpc, &selected, plan)? {
                let stage = plan.stage;
                println!(
                    "{} plan expired unlanded at block height {}; re-planning against current chain",
                    stage.label(),
                    plan.last_valid_block_height
                );
                checkpoint.stage_plan = None;
                write_checkpoint(&checkpoint_path, &checkpoint)?;
                continue;
            }
            match plan.phase {
                DurablePhaseV1::Finalized => {
                    let observed = provider_transaction_status(&mut rpc, &selected, plan)?
                        .ok_or_else(|| Error::new("persisted provider finalization disappeared"))?;
                    let receipt = finish_provider_stage(
                        &mut rpc,
                        &selected,
                        plan,
                        observed,
                        expected_cluster,
                    )?;
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
                DurablePhaseV1::SignedNotSubmitted => {
                    if !arguments.execute {
                        println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                        return Ok(());
                    }
                    authenticate_provider_prestate(&mut rpc, &selected, plan, expected_cluster)?;
                    plan.phase = DurablePhaseV1::Dispatching;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    continue;
                }
                DurablePhaseV1::Dispatching => {
                    if let Some(finalized) = provider_transaction_status(&mut rpc, &selected, plan)?
                    {
                        let receipt = finish_provider_stage(
                            &mut rpc,
                            &selected,
                            plan,
                            finalized,
                            expected_cluster,
                        )?;
                        park_core_terminal_accept_chaos_boundary_v1(
                            expected_cluster,
                            &checkpoint_path,
                            plan,
                            BoundaryV1::LandedBeforeFinalizationFsync,
                        )?;
                        plan.phase = DurablePhaseV1::Finalized;
                        plan.finalized = Some(receipt);
                        write_checkpoint(&checkpoint_path, &checkpoint)?;
                        continue;
                    }
                    if !arguments.execute {
                        println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                        return Ok(());
                    }
                    authenticate_provider_prestate(&mut rpc, &selected, plan, expected_cluster)?;
                    park_core_terminal_accept_chaos_boundary_v1(
                        expected_cluster,
                        &checkpoint_path,
                        plan,
                        BoundaryV1::DispatchingBeforeSend,
                    )?;
                    send_provider_packet_once(&mut rpc, plan)?;
                    plan.phase = DurablePhaseV1::Submitted;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    continue;
                }
                DurablePhaseV1::Submitted => {
                    if let Some(finalized) = provider_transaction_status(&mut rpc, &selected, plan)?
                    {
                        let receipt = finish_provider_stage(
                            &mut rpc,
                            &selected,
                            plan,
                            finalized,
                            expected_cluster,
                        )?;
                        park_core_terminal_accept_chaos_boundary_v1(
                            expected_cluster,
                            &checkpoint_path,
                            plan,
                            BoundaryV1::LandedBeforeFinalizationFsync,
                        )?;
                        plan.phase = DurablePhaseV1::Finalized;
                        plan.finalized = Some(receipt);
                        write_checkpoint(&checkpoint_path, &checkpoint)?;
                    }
                    // Submitted recovery is permanently poll-only and never
                    // accesses a key or retransmits a packet.
                    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    return Ok(());
                }
                DurablePhaseV1::Planned if !arguments.execute => {
                    println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    return Ok(());
                }
                DurablePhaseV1::Planned => {
                    authenticate_provider_prestate(&mut rpc, &selected, plan, expected_cluster)?;
                    let stage = plan.stage;
                    let (payer, update) = load_stage_signers(&selected, stage, &arguments)?;
                    sign_provider_plan(plan, &payer, update.as_ref())?;
                    // Signed packet and local transaction ID are durable before the sole send.
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    let plan = checkpoint
                        .stage_plan
                        .as_ref()
                        .ok_or_else(|| Error::new("signed provider plan disappeared"))?;
                    authenticate_provider_prestate(&mut rpc, &selected, plan, expected_cluster)?;
                    checkpoint
                        .stage_plan
                        .as_mut()
                        .ok_or_else(|| Error::new("dispatching provider plan disappeared"))?
                        .phase = DurablePhaseV1::Dispatching;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    let plan = checkpoint
                        .stage_plan
                        .as_ref()
                        .ok_or_else(|| Error::new("dispatching provider plan disappeared"))?;
                    park_core_terminal_accept_chaos_boundary_v1(
                        expected_cluster,
                        &checkpoint_path,
                        plan,
                        BoundaryV1::DispatchingBeforeSend,
                    )?;
                    send_provider_packet_once(&mut rpc, plan)?;
                    checkpoint
                        .stage_plan
                        .as_mut()
                        .ok_or_else(|| Error::new("submitted provider plan disappeared"))?
                        .phase = DurablePhaseV1::Submitted;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                    if let Some(finalized) = provider_transaction_status(
                        &mut rpc,
                        &selected,
                        checkpoint.stage_plan.as_ref().expect("submitted plan"),
                    )? {
                        let plan = checkpoint.stage_plan.as_ref().expect("submitted plan");
                        let receipt = finish_provider_stage(
                            &mut rpc,
                            &selected,
                            plan,
                            finalized,
                            expected_cluster,
                        )?;
                        park_core_terminal_accept_chaos_boundary_v1(
                            expected_cluster,
                            &checkpoint_path,
                            plan,
                            BoundaryV1::LandedBeforeFinalizationFsync,
                        )?;
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
            authenticate_selected_pyth_release(&selected, &snapshot, false, expected_cluster)?;
            verify_terminal(&selected, &snapshot)?;
            require_terminal_receipts(&checkpoint, expected_cluster)?;
            checkpoint.verified_terminal = true;
            write_checkpoint(&checkpoint_path, &checkpoint)?;
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        if stage > through {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        let prepared = prepare_stage(&mut rpc, &selected, &snapshot, stage, expected_cluster)?;
        checkpoint.stage_plan = Some(prepared.plan);
        // Complete real v0 message, exact fee, ALT resolution, and prebalances are durable first.
        write_checkpoint(&checkpoint_path, &checkpoint)?;
        if !arguments.execute {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
    }
}

fn load_checkpoint(
    path: &Path,
    input_sha256: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<CheckpointV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CheckpointV1 {
                format: checkpoint_format(expected_cluster).to_owned(),
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
    authenticate_checkpoint_identity(&checkpoint, input_sha256, expected_cluster)?;
    if let Some(plan) = &checkpoint.stage_plan {
        plan.validate()?;
    }
    authenticate_receipt_prefix(&checkpoint, expected_cluster)?;
    Ok(checkpoint)
}

/// Reopen the exact owned-loopback flagship artifacts and the finalized chain
/// before handing Resolution facts to a wider lifecycle campaign.
///
/// `verifiedTerminal` is not accepted as a shortcut.  This repeats the current
/// deployment, selected provider release, thirty-three-clause terminal join,
/// and exact four-receipt checks against a fresh finalized observation.  The
/// route/attempt fields returned below are decoded from the persisted Source
/// and certificate rather than asserted by the caller.
pub(crate) fn authenticate_direct_resolution_terminal_v1(
    rpc: &mut Rpc,
    input_path: &Path,
    checkpoint_path: &Path,
) -> Result<DirectResolutionTerminalEvidenceV1> {
    let input_bytes = fs::read(input_path).map_err(|error| {
        Error::new(format!(
            "read direct-life Resolution input {}: {error}",
            input_path.display()
        ))
    })?;
    let input: PlanInputV1 = serde_json::from_slice(&input_bytes)?;
    let selected = SelectedInputV1::parse(&input, ExpectedClusterV1::OwnedLoopback)?;
    let input_sha256 = hex(&Sha256::digest(&input_bytes));
    let checkpoint_bytes = fs::read(checkpoint_path).map_err(|error| {
        Error::new(format!(
            "read direct-life Resolution checkpoint {}: {error}",
            checkpoint_path.display()
        ))
    })?;
    let checkpoint = load_checkpoint(
        checkpoint_path,
        &input_sha256,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    if !checkpoint.verified_terminal || checkpoint.stage_plan.is_some() {
        return Err(Error::new(
            "direct-life Resolution checkpoint has not reached its exact terminal receipt prefix",
        ));
    }
    require_terminal_receipts(&checkpoint, ExpectedClusterV1::OwnedLoopback)?;

    let snapshot = observe(rpc, &selected, StageV1::Complete, 0)?;
    if classify(chain_facts(&selected, &snapshot)?)? != StageV1::Complete {
        return Err(Error::new(
            "direct-life Resolution chain state no longer classifies as provider lifecycle Complete",
        ));
    }
    authenticate_current_deployments(&selected, &snapshot)?;
    authenticate_selected_pyth_release(
        &selected,
        &snapshot,
        false,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    verify_terminal(&selected, &snapshot)?;

    let market = selected.account("market")?;
    let source_state = selected.account("source_state")?;
    let certificate = selected.account("certificate")?;
    let resolution_program = selected.account("resolution_program")?;
    let source_account = snapshot.account(source_state, "terminal Source state")?;
    let certificate_account = snapshot.account(certificate, "terminal certificate")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("terminal Source state: {error:?}")))?;
    let terminal = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("terminal Source projection: {error:?}")))?;
    let certificate_value = ResolutionCertificateV2::decode(&certificate_account.data)
        .map_err(|error| Error::new(format!("terminal certificate: {error:?}")))?;
    if terminal.route() != SourceResolutionRouteV1::Primary
        || certificate_value.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate_value.attempt_index != 0
        || terminal.selector() != certificate_value.selector
    {
        return Err(Error::new(
            "direct-life provider completion was not the exact primary first-attempt terminal",
        ));
    }
    let finalized_receipts = checkpoint
        .receipts
        .iter()
        .map(serde_json::to_value)
        .collect::<core::result::Result<Vec<_>, _>>()?;
    Ok(DirectResolutionTerminalEvidenceV1 {
        input_sha256,
        checkpoint_sha256: hex(&Sha256::digest(&checkpoint_bytes)),
        market: market.to_string(),
        source_state: source_state.to_string(),
        source_state_sha256: hex(&Sha256::digest(&source_account.data)),
        certificate: certificate.to_string(),
        certificate_sha256: hex(&Sha256::digest(&certificate_account.data)),
        resolution_program: resolution_program.to_string(),
        generation: selected.generation,
        terminal_sequence: terminal.terminal_sequence(),
        selector: terminal.selector(),
        attempt_index: certificate_value.attempt_index,
        route: "primary",
        certificate_kind: "resolution-success",
        finalized_receipts,
    })
}

fn authenticate_receipt_prefix(
    checkpoint: &CheckpointV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let expected = [
        ReceiptStageV1::Submit,
        ReceiptStageV1::ProviderExecute,
        ReceiptStageV1::CoreAccept,
        ReceiptStageV1::Reclaim,
    ];
    let signatures = checkpoint
        .receipts
        .iter()
        .map(|receipt| receipt.signature.as_str())
        .collect::<BTreeSet<_>>();
    if checkpoint.receipts.len() > expected.len()
        || checkpoint
            .receipts
            .iter()
            .zip(expected)
            .any(|(receipt, expected_stage)| {
                receipt.stage != expected_stage
                    || receipt.signature.is_empty()
                    || receipt.slot == 0
                    || receipt.compute_units_consumed == 0
                    || hex32(&receipt.signed_transaction_sha256).is_err()
                    || hex32(&receipt.return_data_sha256).is_err()
                    || (receipt.stage == ReceiptStageV1::CoreAccept
                        && (!receipt.return_data_base64.is_empty()
                            || receipt.return_data_sha256 != hex(&Sha256::digest([]))))
            })
        || checkpoint
            .receipts
            .windows(2)
            .any(|pair| pair[0].slot >= pair[1].slot)
        || signatures.len() != checkpoint.receipts.len()
        || (checkpoint.verified_terminal
            && (checkpoint.receipts.len() != expected.len() || checkpoint.stage_plan.is_some()))
    {
        return Err(Error::new(
            "checkpoint receipts are missing, substituted, out of order, or disagree with terminal completion",
        ));
    }
    for receipt in &checkpoint.receipts {
        expected_cluster
            .authenticate_finalized_fee(receipt.fee_lamports, "Resolution finalized mutation")?;
    }
    Ok(())
}

/// Carry the finalized receipts of every stage before `stage` into a fresh
/// checkpoint, each one re-derived from chain rather than trusted from the file
/// (`docs/design/EVIDENCE_REFRESH_V1.md` §7.10). This exists so a new
/// `input.json` — a new `inputSha256`, and so a checkpoint
/// `authenticate_checkpoint_identity` will not load — can still reach the
/// unchanged exactly-four gate honestly.
fn adopt_prior_receipts(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    checkpoint: &mut CheckpointV1,
    prior_path: &Path,
    stage: StageV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    if !checkpoint.receipts.is_empty()
        || checkpoint.stage_plan.is_some()
        || checkpoint.verified_terminal
    {
        return Err(Error::new(
            "receipt adoption refused into a checkpoint that already has history",
        ));
    }
    let bytes = fs::read(prior_path).map_err(|error| {
        Error::new(format!(
            "read adopted checkpoint {}: {error}",
            prior_path.display()
        ))
    })?;
    let prior: CheckpointV1 = serde_json::from_slice(&bytes)?;
    require_adoption_coverage(&prior.receipts, stage)?;
    for receipt in &prior.receipts {
        authenticate_adopted_receipt(rpc, selected, receipt)?;
    }
    checkpoint.receipts = prior.receipts;
    // Every structural clause the driver-minted vector answers, unchanged.
    authenticate_receipt_prefix(checkpoint, expected_cluster)
}

/// The chain decides how many receipts a resume owes: exactly the stage prefix
/// below `stage`, in order. This is the clause that keeps a resume from
/// skipping a stage — no operator input can change the count (§7.10 Ruling 5).
fn require_adoption_coverage(receipts: &[StageReceiptV1], stage: StageV1) -> Result<()> {
    let owed = [
        StageV1::Submit,
        StageV1::Execute,
        StageV1::Accept,
        StageV1::Reclaim,
    ]
    .into_iter()
    .filter(|candidate| *candidate < stage)
    .map(ReceiptStageV1::from_stage)
    .collect::<Result<Vec<_>>>()?;
    if receipts.len() != owed.len()
        || receipts
            .iter()
            .zip(&owed)
            .any(|(receipt, expected)| receipt.stage != *expected)
    {
        return Err(Error::new(format!(
            "adopted receipts do not cover exactly the stages before {}",
            stage.label()
        )));
    }
    Ok(())
}

/// Re-derive one adopted receipt from the finalized transaction it names. The
/// anchor is the packet digest: the receipt names a byte string, and the cluster
/// hands back the bytes it executed.
fn authenticate_adopted_receipt(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    receipt: &StageReceiptV1,
) -> Result<()> {
    let label = receipt.stage.label();
    let value = rpc.call(
        "getTransaction",
        &json!([receipt.signature, {"commitment":"finalized","encoding":"base64","maxSupportedTransactionVersion":0}]),
    )?;
    if value.is_null() {
        return Err(Error::new(format!(
            "adopted {label} receipt has no finalized transaction on this cluster"
        )));
    }
    let tuple = value
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted base64 tuple")))?;
    let encoded = tuple.first().and_then(Value::as_str).unwrap_or_default();
    let packet = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("adopted {label} packet base64: {error}")))?;
    let transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("adopted {label} packet: {error}")))?;
    if tuple.len() != 2
        || tuple.get(1).and_then(Value::as_str) != Some("base64")
        || hex(&Sha256::digest(&packet)) != receipt.signed_transaction_sha256
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != Some(receipt.signature.as_str())
    {
        return Err(Error::new(format!(
            "adopted {label} receipt does not authenticate against the finalized packet"
        )));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("adopted {label} packet signature: {error}")))?;
    let meta = value
        .get("meta")
        .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted meta")))?;
    if !meta.get("err").is_some_and(Value::is_null) {
        return Err(Error::new(format!(
            "adopted {label} receipt finalized with a runtime error"
        )));
    }
    let numbers = |parent: &Value, key: &str| -> Result<Vec<u64>> {
        parent
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted {key}")))?
            .iter()
            .map(|entry| {
                entry
                    .as_u64()
                    .ok_or_else(|| Error::new(format!("adopted {label} receipt invalid {key}")))
            })
            .collect()
    };
    let strings = |parent: &Value, key: &str| -> Result<Vec<String>> {
        parent
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted {key}")))?
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| Error::new(format!("adopted {label} receipt invalid {key}")))
            })
            .collect()
    };
    let loaded = meta
        .get("loadedAddresses")
        .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted loadedAddresses")))?;
    let mut resolved = transaction
        .message
        .static_account_keys()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let statics = resolved.len();
    resolved.extend(strings(loaded, "writable")?);
    resolved.extend(strings(loaded, "readonly")?);
    if resolved != receipt.resolved_account_keys {
        return Err(Error::new(format!(
            "adopted {label} receipt resolved a different account vector"
        )));
    }
    // The packet may be real, finalized, and correctly digested and still belong
    // to another market. Bind it to *this* input: the certificate and lifecycle
    // it touched, and the Resolution program it invoked (§7.10 Ruling 4).
    let mut bound = vec![
        lifecycle_address(selected)?,
        selected.account("market")?,
        selected.account("source_state")?,
        selected.account("update_account")?,
    ];
    // Submit CREATES the certificate's preconditions but never names it; every
    // later stage does. Demand it exactly where it belongs.
    if receipt.stage != ReceiptStageV1::Submit {
        bound.push(selected.account("certificate")?);
    }
    let resolution = selected.account("resolution_program")?.to_string();
    if bound
        .into_iter()
        .any(|key| !resolved.contains(&key.to_string()))
        || !resolved[..statics].contains(&resolution)
    {
        return Err(Error::new(format!(
            "adopted {label} receipt belongs to a different market"
        )));
    }
    let return_data_base64 = match meta.get("returnData") {
        Some(data) if !data.is_null() => data
            .get("data")
            .and_then(Value::as_array)
            .and_then(|tuple| tuple.first())
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("adopted {label} receipt omitted returnData bytes")))?
            .to_owned(),
        _ => String::new(),
    };
    let return_bytes = BASE64
        .decode(&return_data_base64)
        .map_err(|error| Error::new(format!("adopted {label} returnData base64: {error}")))?;
    for (field, chain, named) in [
        (
            "slot",
            value.get("slot").and_then(Value::as_u64),
            Some(receipt.slot),
        ),
        (
            "fee",
            meta.get("fee").and_then(Value::as_u64),
            Some(receipt.fee_lamports),
        ),
        (
            "computeUnitsConsumed",
            finalized_compute_units(meta, "adopted receipt").ok(),
            Some(receipt.compute_units_consumed),
        ),
    ] {
        if chain.is_none() || chain != named {
            return Err(Error::new(format!(
                "adopted {label} receipt {field} differs from the finalized transaction"
            )));
        }
    }
    if numbers(meta, "preBalances")? != receipt.pre_balances
        || numbers(meta, "postBalances")? != receipt.post_balances
        || return_data_base64 != receipt.return_data_base64
        || hex(&Sha256::digest(&return_bytes)) != receipt.return_data_sha256
    {
        return Err(Error::new(format!(
            "adopted {label} receipt balances or return data differ from the finalized transaction"
        )));
    }
    Ok(())
}

#[derive(Default)]
struct ReprovisionArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    producer_checkpoint: Option<PathBuf>,
    campaign_evidence: Option<PathBuf>,
    plan: Option<PathBuf>,
    output: Option<PathBuf>,
    input_output: Option<PathBuf>,
    payer: Option<String>,
}

impl ReprovisionArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        let mut mode = false;
        while let Some(argument) = iterator.next() {
            if argument == "--reprovision-execute-table" {
                if mode {
                    return Err(Error::new(
                        "--reprovision-execute-table may be supplied only once",
                    ));
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
                "--producer-checkpoint" => set_once(
                    &mut parsed.producer_checkpoint,
                    PathBuf::from(value),
                    "--producer-checkpoint",
                )?,
                "--campaign-evidence" => set_once(
                    &mut parsed.campaign_evidence,
                    PathBuf::from(value),
                    "--campaign-evidence",
                )?,
                "--plan" => set_once(&mut parsed.plan, PathBuf::from(value), "--plan")?,
                "--output" => set_once(&mut parsed.output, PathBuf::from(value), "--output")?,
                "--input-output" => set_once(
                    &mut parsed.input_output,
                    PathBuf::from(value),
                    "--input-output",
                )?,
                "--payer" => set_once(&mut parsed.payer, value, "--payer")?,
                _ => {
                    return Err(Error::new(format!(
                        "unknown reprovision argument: {argument}"
                    )));
                }
            }
        }
        Ok(parsed)
    }
}

/// Re-plan the Execute lookup table for a life already under way.
///
/// The producer cannot run here — it asserts a fresh life, and the Receiver
/// update this market already created is not vacant. But nothing about the
/// planned input is stale except the Execute union, which grew by the eight
/// rows §7.9 measured as missing. So this reads an already-authenticated
/// producer checkpoint, names the six Execute-only staging cursors the input
/// never carried, and re-plans **only** the Execute table at a fresh creation
/// slot. Submit and Reclaim keep their frozen tables, because neither union
/// changed.
///
/// It writes nothing to chain, and it re-runs `authenticate_producer_checkpoint`
/// on its own output before writing: a re-plan that does not agree with the
/// union derived from its own planned input refuses here.
fn run_execute_table_reprovision(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let arguments = ReprovisionArgumentsV1::parse(arguments)?;
    let producer_path = absolute(arguments.producer_checkpoint, "--producer-checkpoint")?;
    let campaign_path = absolute(arguments.campaign_evidence, "--campaign-evidence")?;
    let plan_path = absolute(arguments.plan, "--plan")?;
    let output_path = absolute(arguments.output, "--output")?;
    let input_path = absolute(arguments.input_output, "--input-output")?;
    let producer_bytes = fs::read(&producer_path).map_err(|error| {
        Error::new(format!(
            "read producer checkpoint {}: {error}",
            producer_path.display()
        ))
    })?;
    let prior: ProducerCheckpointV1 = serde_json::from_slice(&producer_bytes)?;
    // The prior checkpoint cannot authenticate itself under the widened
    // selector set — it was minted before the six cursors were named, so its
    // planned input carries empty strings where they belong. The output is
    // authenticated in full instead, and the two are tied together by the
    // exact-diff assertion below, which is strictly stronger: it admits a change
    // to the six cursors and the Execute table address, and nothing else.
    let campaign_bytes = fs::read(&campaign_path).map_err(|error| {
        Error::new(format!(
            "read campaign evidence {}: {error}",
            campaign_path.display()
        ))
    })?;
    if hex(&Sha256::digest(&campaign_bytes)) != prior.campaign_evidence_sha256 {
        return Err(Error::new(
            "campaign evidence digest differs from the producer checkpoint it re-plans",
        ));
    }
    let campaign_envelope: CampaignEvidenceV1 = serde_json::from_slice(&campaign_bytes)?;
    let plan_bytes = fs::read(&plan_path)
        .map_err(|error| Error::new(format!("read plan {}: {error}", plan_path.display())))?;
    if hex(&Sha256::digest(&plan_bytes)) != prior.plan_sha256 {
        return Err(Error::new(
            "plan digest differs from the producer checkpoint it re-plans",
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let campaign = completed_campaign(&campaign_envelope, &prior.plan_sha256, expected_cluster)?;
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    expected_cluster.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let registry_program = pubkey(&prior.planned_input.accounts.registry_program)?;
    let mut input = prior.planned_input.clone();
    // The six Execute-only staging cursors, each re-derived from the campaign
    // record whose raw coordinate pins its (schema, digest) pair.
    // Each cursor is tied to the raw record the input ALREADY names. That makes
    // the derivation self-checking: `campaign_record_staging` pins the (schema,
    // digest) pair by re-deriving the raw coordinate, and this pins that
    // coordinate to the authenticated selector. A campaign document that no
    // longer describes this input's records refuses rather than seating a stale
    // cursor in the frozen table.
    let cursor = |label: &str, selector: &str, raw: &str, schema: [u8; 32]| -> Result<String> {
        let staging = campaign_record_staging(campaign, label, schema, registry_program)?;
        if campaign_account(campaign, label)?.to_string() != raw {
            return Err(Error::new(format!(
                "campaign {label} is not the record this input names as {selector}"
            )));
        }
        Ok(staging.to_string())
    };
    input.accounts.source_spec_staging = cursor(
        "source_spec_record",
        "sourceSpec",
        &prior.planned_input.accounts.source_spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
    )?;
    input.accounts.source_provider_release_staging = cursor(
        "provider_release_record",
        "sourceProviderRelease",
        &prior.planned_input.accounts.source_provider_release,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
    )?;
    input.accounts.adapter_config_staging = cursor(
        "pyth_adapter_config_record",
        "adapterConfig",
        &prior.planned_input.accounts.adapter_config,
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
    )?;
    input.accounts.window_staging = cursor(
        "window_spec_record",
        "window",
        &prior.planned_input.accounts.window,
        WINDOW_SPEC_SCHEMA_ID_V1,
    )?;
    input.accounts.statistic_staging = cursor(
        "statistic_spec_record",
        "statistic",
        &prior.planned_input.accounts.statistic,
        STATISTIC_SPEC_SCHEMA_ID_V1,
    )?;
    // Pyth's staging cursor was always derivable — the producer computed it from
    // the plan and discarded it.
    input.accounts.pyth_release_staging = plan_record(&plan, "pyth_release")?.1.to_owned();
    let authority = pubkey(&prior.authority)?;
    let value = rpc.call("getSlot", &json!([{"commitment":"finalized"}]))?;
    let creation_slot = value
        .as_u64()
        .ok_or_else(|| Error::new("getSlot omitted a finalized slot"))?;
    let table_key = create_lookup_table(authority, authority, creation_slot).1;
    input.lookup_tables.execute = table_key.to_string();
    // The fee payer is the eighth movable field, and it may only be *named*,
    // never *changed*: a checkpoint minted before wall 10 carries an empty
    // payer, and naming one is how such a life adopts the fix. A checkpoint
    // that already names a payer keeps it — re-pointing the payer of a life
    // whose Execute packet may already be signed is a substitution, not a
    // re-plan, and refuses below on byte equality like any other drift.
    let named_payer = nonzero_pubkey(
        arguments
            .payer
            .as_deref()
            .ok_or_else(|| Error::new("--payer is required"))?,
        "--payer",
    )?
    .to_string();
    if !prior.planned_input.payer.is_empty() && prior.planned_input.payer != named_payer {
        return Err(Error::new(format!(
            "the producer checkpoint already names payer {}; --payer {named_payer} would \
             substitute it",
            prior.planned_input.payer,
        )));
    }
    input.payer = named_payer;
    // Exactly eight fields may move. Rebase the new input back onto the prior
    // one and demand byte equality: any other drift — a substituted Market, a
    // moved refund recipient, a different post body — refuses here.
    let mut rebased = input.clone();
    rebased.payer = prior.planned_input.payer.clone();
    rebased.accounts.source_spec_staging = prior.planned_input.accounts.source_spec_staging.clone();
    rebased.accounts.source_provider_release_staging = prior
        .planned_input
        .accounts
        .source_provider_release_staging
        .clone();
    rebased.accounts.adapter_config_staging =
        prior.planned_input.accounts.adapter_config_staging.clone();
    rebased.accounts.window_staging = prior.planned_input.accounts.window_staging.clone();
    rebased.accounts.statistic_staging = prior.planned_input.accounts.statistic_staging.clone();
    rebased.accounts.pyth_release_staging =
        prior.planned_input.accounts.pyth_release_staging.clone();
    rebased.lookup_tables.execute = prior.planned_input.lookup_tables.execute.clone();
    if rebased != prior.planned_input {
        return Err(Error::new(
            "Execute table re-plan changed more than the six staging cursors, the fee payer, \
             and the Execute table",
        ));
    }
    let selected = SelectedInputV1::parse(&input, expected_cluster)?;
    let table = build_lookup_table_plan(&selected, StageV1::Execute, creation_slot, authority)?;
    authenticate_lookup_table_plan(&selected, &table)?;
    let route = route_lookup_table(&table, None, creation_slot, &Rent::default())?;
    let mut tables = prior.tables.clone();
    let mut routes = prior.routes.clone();
    tables.insert(StageV1::Execute, table);
    routes.insert(StageV1::Execute, route);
    let checkpoint = ProducerCheckpointV1 {
        tables,
        routes,
        planned_input: input.clone(),
        flagship_input: Some(input.clone()),
        ..prior
    };
    // The output is admitted by the same authentication the producer's own
    // output is: the Execute plan must agree with the union its planned input
    // derives, and every table address must agree with `lookupTables`.
    authenticate_producer_checkpoint(&checkpoint, expected_cluster)?;
    write_json(&output_path, &checkpoint)?;
    write_json(&input_path, &input)?;
    println!("{}", serde_json::to_string_pretty(&checkpoint.routes)?);
    Ok(())
}

fn require_terminal_receipts(
    checkpoint: &CheckpointV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    authenticate_receipt_prefix(checkpoint, expected_cluster)?;
    if checkpoint.receipts.len() != 4 || checkpoint.stage_plan.is_some() {
        return Err(Error::new(
            "Core terminal state cannot finalize an incomplete four-mutation checkpoint",
        ));
    }
    Ok(())
}

fn authenticate_checkpoint_identity(
    checkpoint: &CheckpointV1,
    input_sha256: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    if checkpoint.format != checkpoint_format(expected_cluster)
        || checkpoint.input_sha256 != input_sha256
    {
        return Err(Error::new(
            "checkpoint format or input digest differs; cross-market resume refused",
        ));
    }
    Ok(())
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
            load_keypair(arguments.payer_keypair.as_ref(), "payer", selected.payer)?,
            Some(load_keypair(
                arguments.resolver_keypair.as_ref(),
                "resolver",
                selected.resolver,
            )?),
        )),
        StageV1::Accept => Ok((
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
    // Wall 11: two records, one word. `certificate.route` is the *Pyth* release
    // record's content digest — `provider_finalized_projection_v3` writes
    // `route: request.provider_release`, and the transport builder sets that to
    // `pyth_id`, which it reads *out of* the Source's ProviderRelease record and
    // then pins to the Pyth release account with `authenticate_raw`. Digesting
    // the Source's ProviderRelease here compared the wrong one of the two, and
    // nothing caught it because no market had ever passed Execute.
    //
    // The join is checked whole rather than merely repointed: the Source's
    // ProviderRelease must name the Pyth release the certificate routed
    // through, and that Pyth release account must be the record it names. Both
    // records stay read; neither is dropped to make a clause pass.
    let source_provider_release = snapshot.account(
        selected.account("source_provider_release")?,
        "ProviderRelease",
    )?;
    let pyth_release = snapshot.account(selected.account("pyth_release")?, "PythRelease")?;
    let source_provider = ProviderReleaseV1::decode(&source_provider_release.data)
        .map_err(|error| Error::new(format!("ProviderRelease: {error:?}")))?;
    let product = snapshot.account(selected.account("product")?, "Product")?;
    // Thirty-four conjuncts behind one string is a refusal that can be reported
    // and not acted on: it says the terminal join failed and nothing about
    // *which* coordinate disagreed, which is the difference between a wall that
    // can be driven and one that can only be described (§7.4). The relation is
    // unchanged — every clause below is the same clause, in the same order, and
    // all of them must hold. Only the payload is new.
    let certificate_rent = snapshot_rent(snapshot)?.minimum_balance(certificate_account.data.len());
    let refused: Vec<&str> = [
        (
            "market phase is Terminal",
            market.phase == CorePhase::Terminal,
        ),
        (
            "market readiness is Consumed",
            market.readiness == Readiness::Consumed,
        ),
        (
            "market identity names this market",
            market.identity.market_id.to_bytes() == market_key.to_bytes(),
        ),
        (
            "market generation",
            market.identity.generation == selected.generation,
        ),
        (
            "market release set",
            market.identity.selected_release_set.to_bytes() == selected.release_set,
        ),
        (
            "market terminal receipt names the certificate",
            market.terminal_receipt.map(|value| value.to_bytes())
                == Some(certificate_key.to_bytes()),
        ),
        (
            "certificate market",
            certificate.market == market_key.to_bytes(),
        ),
        (
            "certificate generation",
            certificate.generation == selected.generation,
        ),
        (
            "certificate receipt account is itself",
            certificate.receipt_account == certificate_key.to_bytes(),
        ),
        (
            "certificate kind is ResolutionSuccess",
            certificate.kind == ResolutionCertificateKindV2::ResolutionSuccess,
        ),
        (
            "certificate selector is the market's terminal winner",
            certificate.selector == market.terminal_winner,
        ),
        (
            "certificate route digests the Pyth release record",
            certificate.route == hash(&pyth_release.data).to_bytes(),
        ),
        (
            "the Source ProviderRelease names that Pyth release",
            source_provider.provider_deployment_release_id().to_bytes() == certificate.route,
        ),
        (
            "certificate source material is the market's resolution policy",
            certificate.source_material == market.identity.resolution_policy.to_bytes(),
        ),
        (
            "certificate product-record digest",
            certificate.product_record_digest == market.identity.product_record.to_bytes(),
        ),
        (
            "certificate provider evidence is the Source terminal evidence",
            certificate.provider_evidence == source_terminal.resolution_evidence_id().to_bytes(),
        ),
        (
            "certificate funding allocation is zero",
            certificate.funding_allocation == [0; 32],
        ),
        (
            "certificate attempt index is zero",
            certificate.attempt_index == 0,
        ),
        (
            "certificate result denominator is one",
            certificate.result_denominator == 1,
        ),
        (
            "certificate observed_at is nonzero",
            certificate.observed_at != 0,
        ),
        (
            "Source phase is Resolved",
            source.phase() == SourceResolutionPhaseV1::Resolved,
        ),
        ("Source market", source.market() == market_key.to_bytes()),
        (
            "Source generation",
            source.generation() == selected.generation,
        ),
        (
            "Source terminal route is Primary",
            source_terminal.route() == SourceResolutionRouteV1::Primary,
        ),
        (
            "Source terminal selector matches the certificate",
            source_terminal.selector() == certificate.selector,
        ),
        (
            "Source terminal sequence matches the input",
            source_terminal.terminal_sequence() == selected.terminal_sequence,
        ),
        (
            "Source state is its canonical PDA",
            source_key == expected_source,
        ),
        (
            "certificate is its canonical PDA",
            certificate_key == expected_certificate,
        ),
        (
            "Source state is Resolution-owned",
            source_account.owner == resolution && !source_account.executable,
        ),
        (
            "certificate is Resolution-owned",
            certificate_account.owner == resolution && !certificate_account.executable,
        ),
        (
            "certificate is exactly rent-exempt",
            certificate_account.lamports == certificate_rent,
        ),
        (
            "SourceMaterial digests to the market's resolution policy",
            hash(&source_material.data).to_bytes() == market.identity.resolution_policy.to_bytes(),
        ),
        (
            "Product digests to the market's product record",
            hash(&product.data).to_bytes() == market.identity.product_record.to_bytes(),
        ),
    ]
    .into_iter()
    .filter_map(|(label, held)| (!held).then_some(label))
    .collect();
    if !refused.is_empty() {
        return Err(Error::new(format!(
            "finalized Core Terminal, receipt, winner, Source, Market, generation, or release \
             join refused on {} of 33 clauses: {}",
            refused.len(),
            refused.join("; "),
        )));
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
            .is_ok_and(|stage| stage == StageV1::Accept),
            "provider execution must stop at the durable Core-accept boundary"
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

    /// A sponsored-push market that resolved by the failure walk is NOT an
    /// `accept` stage, and `--through accept` cannot reach one.
    ///
    /// The Core builder this stage calls — `build_resolution_admit_terminal_v3`
    /// in `core_terminal_accept_report` — names no relayed-VAA coordinate at
    /// all: its snapshot is Market, activation cache, the four deployment
    /// programs, SourceMaterial, capability manifest, Source state, funding
    /// ledger, certificate, Rent and the three product record pairs. So the
    /// *capability* is separable. The *command* is not, and this test is the
    /// cheap half of why: cohort-13's chain state — Market Open/Consumed,
    /// Source `FailureCommitted`, no provider lifecycle, no Receiver update,
    /// certificate written — does not classify as `Accept`, because `Accept`
    /// demands a Consumed lifecycle and a Submitted update account that a
    /// snapshot-route market never creates. `--through` caps the LAST stage
    /// run; the entry stage always comes from `classify`, so no flag reaches
    /// past this refusal.
    ///
    /// Three further refusals stand in front of it in `SelectedInputV1::parse`,
    /// which is why the answer is a design fact and not a missing flag:
    /// `encodedVaa` and `updateAccount` are `nonzero_pubkey`, and
    /// `postUpdateBodyBase64` must parse as `PostUpdateParamsView`. A market
    /// resolved from a sponsored snapshot has none of the three and never will.
    ///
    /// The arm that does reach Core for such a market is
    /// `devnet-sponsored-push-v1 --action admit-terminal`.
    #[test]
    fn sponsored_push_failure_state_is_not_an_accept_stage() {
        use SlotKindV1::{Submitted, Vacant};
        let cohort_thirteen = facts(
            CorePhase::Open,
            SourceResolutionPhaseV1::FailureCommitted,
            Vacant,
            Vacant,
            Submitted,
        );
        let refusal = classify(cohort_thirteen).expect_err("sponsored-push state is not a stage");
        assert!(
            refusal.0.contains(
                "Market, Source, lifecycle, update, and certificate do not form one canonical stage"
            ),
            "expected the canonical-stage refusal, got: {refusal}"
        );
        // The same Source phase with the relayed ladder's own accounts IS an
        // accept stage, so the refusal above is about the missing VAA route and
        // not about failure resolution being unrepresentable.
        assert_eq!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::FailureCommitted,
                SlotKindV1::Consumed,
                Submitted,
                Submitted,
            ))
            .expect("relayed failure accept"),
            StageV1::Accept
        );
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
            source_material_staging: key(),
            source_spec: key(),
            source_spec_staging: key(),
            source_provider_release: key(),
            source_provider_release_staging: key(),
            adapter_config: key(),
            adapter_config_staging: key(),
            window: key(),
            window_staging: key(),
            statistic: key(),
            statistic_staging: key(),
            pyth_release: key(),
            pyth_release_staging: key(),
            product: key(),
            product_staging: key(),
            result_domain: key(),
            result_domain_staging: key(),
            portfolio: key(),
            portfolio_staging: key(),
            capability_manifest: key(),
            capability_manifest_staging: key(),
            funding_ledger: key(),
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
            payer: Pubkey::new_from_array([71; 32]).to_string(),
            refund_recipient: Pubkey::new_from_array([81; 32]).to_string(),
            terminal_sequence: 1,
            reclaim_after_unix_seconds: 1_000,
            post_update_body_base64: BASE64.encode(price_body([3; 32], 10, 1, -8, 100)),
            accounts,
            lookup_tables: tables,
        }
    }

    fn sample_selected() -> SelectedInputV1 {
        SelectedInputV1::parse(&sample_input(), ExpectedClusterV1::Devnet).expect("selected input")
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

        let mut hostile_lookup = plan.clone();
        let original = hostile_lookup
            .lookup_table_addresses
            .first()
            .cloned()
            .expect("fixture lookup address");
        let substituted = Pubkey::new_from_array([96; 32]).to_string();
        *hostile_lookup
            .lookup_table_addresses
            .first_mut()
            .expect("fixture lookup address") = substituted.clone();
        for vector in [
            &mut hostile_lookup.loaded_writable,
            &mut hostile_lookup.loaded_readonly,
            &mut hostile_lookup.resolved_account_keys,
        ] {
            for key in vector {
                if key == &original {
                    key.clone_from(&substituted);
                }
            }
        }
        if let Some(state) = hostile_lookup.pre_accounts.remove(&original) {
            hostile_lookup
                .pre_accounts
                .insert(substituted.clone(), state);
        }
        let mut lookup_hasher = Sha256::new();
        for key in &hostile_lookup.lookup_table_addresses {
            lookup_hasher.update(pubkey(key).expect("hostile lookup key").as_ref());
        }
        hostile_lookup.lookup_table_addresses_sha256 = hex(&lookup_hasher.finalize());
        assert!(hostile_lookup.validate().is_err());

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
        let selected = SelectedInputV1::parse(&exact, ExpectedClusterV1::Devnet)
            .expect("exact selected input");
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
        assert!(SelectedInputV1::parse(&aliased, ExpectedClusterV1::Devnet).is_err());
        let mut aliased = sample_input();
        aliased.resolver = aliased.accounts.update_account.clone();
        assert!(SelectedInputV1::parse(&aliased, ExpectedClusterV1::Devnet).is_err());
        let mut aliased = sample_input();
        aliased.refund_recipient.clone_from(&aliased.resolver);
        assert!(SelectedInputV1::parse(&aliased, ExpectedClusterV1::Devnet).is_err());
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
            "cluster": "devnet",
            "mode": "execute",
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
                "cluster": "devnet",
                "mode": "execute",
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
            cluster: "devnet".into(),
            mode: "execute".into(),
            plan_sha256: "aa".repeat(32),
            execution: CampaignExecutionEnvelopeV1 {
                completed: true,
                market: Some(market),
            },
        };
        let selected_market =
            completed_campaign(&envelope, &"aa".repeat(32), ExpectedClusterV1::Devnet)
                .expect("campaign Market");
        assert!(
            completed_campaign(&envelope, &"bb".repeat(32), ExpectedClusterV1::Devnet).is_err()
        );
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
        let accept = stable_lookup_union(&selected, StageV1::Accept).expect("accept union");
        let reclaim = stable_lookup_union(&selected, StageV1::Reclaim).expect("reclaim union");
        assert_ne!(submit, execute);
        assert_ne!(submit, reclaim);
        assert_ne!(execute, reclaim);
        assert_eq!(accept, execute);
        assert_eq!(
            selected.table(StageV1::Accept).expect("accept table"),
            selected.table(StageV1::Execute).expect("execute table")
        );
        assert_eq!(submit[0].label, "refund_recipient");
        assert_eq!(submit[0].class, StableAddressClassV1::Beneficiary);
        assert!(execute.iter().any(|row| {
            row.label == "trading_program" && row.class == StableAddressClassV1::Program
        }));
        // Wall 10's margin is 12 bytes and it is bought by the 49th row. If this
        // count moves, the Execute packet no longer fits and the frozen table on
        // chain is stale — both are refusals, but neither is a surprise worth
        // discovering on a cluster.
        assert_eq!(execute.len(), 49);
        let caller = execute
            .iter()
            .find(|row| row.label == "caller_authority")
            .expect("Execute union seats the caller authority");
        assert_eq!(caller.class, StableAddressClassV1::CallerAuthority);
        // The address is bound to this life, not a constant: every seed
        // coordinate `chain_facts` pins must move it.
        let mut elsewhere = sample_input();
        elsewhere.generation += 1;
        let elsewhere = SelectedInputV1::parse(&elsewhere, ExpectedClusterV1::Devnet)
            .expect("another generation");
        let other = stable_lookup_union(&elsewhere, StageV1::Execute).expect("other union");
        assert_ne!(
            caller.address,
            other
                .iter()
                .find(|row| row.label == "caller_authority")
                .expect("caller authority")
                .address,
            "a caller authority that survived a generation change would not be this life's",
        );
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

    /// Wall 10: the fee payer cannot be any account whose privilege a frame pins.
    ///
    /// Before this, `compile_provider_execute_v0` took the payer from
    /// `required_signers.first()` — the resolver — and Solana's compiler
    /// promoted it to a writable signer. The packet compiled, fit, reached the
    /// cluster, and was refused with `0x3001` after 20,517 compute units. Every
    /// refusal below now happens locally, before a key is opened.
    #[test]
    fn the_stage_fee_payer_must_be_distinct_from_the_readonly_resolver() {
        let mut same = sample_input();
        same.payer = same.resolver.clone();
        let error = SelectedInputV1::parse(&same, ExpectedClusterV1::Devnet)
            .expect_err("a resolver that pays is the wall-10 defect");
        assert!(
            format!("{error}").contains("payer must differ from resolver"),
            "{error}",
        );

        let mut absent = sample_input();
        absent.payer = String::new();
        assert!(
            SelectedInputV1::parse(&absent, ExpectedClusterV1::Devnet).is_err(),
            "an input minted before wall 10 deserializes, but never reaches a message",
        );

        for alias in ["market", "source_state", "certificate", "update_account"] {
            let mut aliased = sample_input();
            let selected =
                SelectedInputV1::parse(&sample_input(), ExpectedClusterV1::Devnet).expect("sample");
            aliased.payer = selected.account(alias).expect("selector").to_string();
            let error = SelectedInputV1::parse(&aliased, ExpectedClusterV1::Devnet)
                .expect_err("a payer that aliases the frame flips a pinned privilege");
            assert!(
                format!("{error}").contains("address-book substitution"),
                "{alias}: {error}",
            );
        }

        // And the honest shape still parses, with the payer sorting first.
        let selected =
            SelectedInputV1::parse(&sample_input(), ExpectedClusterV1::Devnet).expect("sample");
        assert_ne!(selected.payer, selected.resolver);
    }

    #[test]
    fn owned_loopback_formats_and_provider_release_are_disjoint_from_devnet() {
        let devnet = sample_input();
        SelectedInputV1::parse(&devnet, ExpectedClusterV1::Devnet).expect("public devnet input");
        assert!(SelectedInputV1::parse(&devnet, ExpectedClusterV1::OwnedLoopback).is_err());

        let mut local = devnet.clone();
        local.format = LOCAL_INPUT_FORMAT.into();
        SelectedInputV1::parse(&local, ExpectedClusterV1::OwnedLoopback)
            .expect("private owned-loopback input");
        assert!(SelectedInputV1::parse(&local, ExpectedClusterV1::Devnet).is_err());

        let public_release =
            expected_pyth_release(ExpectedClusterV1::Devnet).expect("compiled devnet release");
        let local_release = expected_pyth_release(ExpectedClusterV1::OwnedLoopback)
            .expect("compiled local release");
        assert_ne!(public_release.to_bytes(), local_release.to_bytes());
        assert_ne!(public_release.receiver_deployment_slot(), 0);
        assert_ne!(public_release.router_deployment_slot(), 0);
        assert_eq!(local_release.receiver_deployment_slot(), 0);
        assert_eq!(local_release.router_deployment_slot(), 0);
        assert_ne!(
            producer_checkpoint_format(ExpectedClusterV1::Devnet),
            producer_checkpoint_format(ExpectedClusterV1::OwnedLoopback)
        );
        assert_ne!(
            table_journal_format(ExpectedClusterV1::Devnet),
            table_journal_format(ExpectedClusterV1::OwnedLoopback)
        );
        assert_ne!(
            checkpoint_format(ExpectedClusterV1::Devnet),
            checkpoint_format(ExpectedClusterV1::OwnedLoopback)
        );
    }

    #[test]
    fn chaos_target_is_only_owned_loopback_core_terminal_accept() {
        for stage in [
            StageV1::Submit,
            StageV1::Execute,
            StageV1::Accept,
            StageV1::Reclaim,
            StageV1::Complete,
        ] {
            assert_eq!(
                is_core_terminal_accept_chaos_target_v1(ExpectedClusterV1::OwnedLoopback, stage,),
                stage == StageV1::Accept,
            );
            assert!(!is_core_terminal_accept_chaos_target_v1(
                ExpectedClusterV1::Devnet,
                stage,
            ));
        }
    }

    #[test]
    fn resolution_v3_receipts_require_exact_finalized_compute_units() {
        assert_eq!(
            finalized_compute_units(&json!({"computeUnitsConsumed": 123}), "fixture")
                .expect("finalized CU"),
            123
        );
        for hostile in [
            json!({}),
            json!({"computeUnitsConsumed": null}),
            json!({"computeUnitsConsumed": "123"}),
            json!({"computeUnitsConsumed": -1}),
        ] {
            assert!(finalized_compute_units(&hostile, "fixture").is_err());
        }

        let stage_receipt = StageReceiptV1 {
            stage: ReceiptStageV1::Submit,
            signature: Pubkey::new_from_array([1; 32]).to_string(),
            slot: 7,
            fee_lamports: 5_000,
            compute_units_consumed: 123,
            transfer_fee_lamports: 0,
            arithmetic: ArithmeticPlanV1::default(),
            signed_transaction_sha256: "11".repeat(32),
            resolved_account_keys: vec![Pubkey::new_from_array([2; 32]).to_string()],
            pre_balances: vec![10_000],
            post_balances: vec![5_000],
            return_data_base64: BASE64.encode([3]),
            return_data_sha256: hex(&Sha256::digest([3])),
        };
        let checkpoint = CheckpointV1 {
            format: CHECKPOINT_FORMAT.into(),
            input_sha256: "22".repeat(32),
            stage_plan: None,
            receipts: vec![stage_receipt.clone()],
            verified_terminal: false,
        };
        let mut omitted_checkpoint =
            serde_json::to_value(&checkpoint).expect("checkpoint JSON fixture");
        omitted_checkpoint["receipts"][0]
            .as_object_mut()
            .expect("receipt object")
            .remove("computeUnitsConsumed");
        assert!(serde_json::from_value::<CheckpointV1>(omitted_checkpoint).is_err());
        let mut substituted_checkpoint =
            serde_json::to_value(&checkpoint).expect("checkpoint JSON fixture");
        substituted_checkpoint["receipts"][0]["computeUnitsConsumed"] = json!(124);
        let substituted_checkpoint: CheckpointV1 =
            serde_json::from_value(substituted_checkpoint).expect("substituted CU shape");
        assert_ne!(substituted_checkpoint.receipts[0], stage_receipt);

        let table_receipt = TableProvisionReceiptV1 {
            stage: StageV1::Submit,
            action: TableProvisionActionV1::Create,
            lookup_table: Pubkey::new_from_array([4; 32]).to_string(),
            signature: Pubkey::new_from_array([5; 32]).to_string(),
            slot: 8,
            fee_lamports: 5_000,
            compute_units_consumed: 456,
            payer_pre_lamports: 10_000,
            payer_post_lamports: 5_000,
            table_pre_lamports: 0,
            table_post_lamports: 0,
            table_post_account_sha256: "33".repeat(32),
            signed_transaction_sha256: "44".repeat(32),
            resolved_account_keys: vec![Pubkey::new_from_array([6; 32]).to_string()],
            pre_balances: vec![10_000],
            post_balances: vec![5_000],
            post_route: LookupTableRouteV1::Complete {
                last_extended_slot: 7,
                account_sha256: "55".repeat(32),
            },
        };
        let journal = TableProvisionJournalV1 {
            format: TABLE_PROVISION_JOURNAL_FORMAT.into(),
            producer_identity_sha256: "66".repeat(32),
            phase: DurablePhaseV1::Finalized,
            intent: None,
            intent_sha256: None,
            signed_transaction_base64: None,
            signed_transaction_sha256: None,
            expected_signature: None,
            finalized: None,
            receipts: vec![table_receipt.clone()],
        };
        let mut omitted_journal = serde_json::to_value(&journal).expect("table journal fixture");
        omitted_journal["receipts"][0]
            .as_object_mut()
            .expect("receipt object")
            .remove("computeUnitsConsumed");
        assert!(serde_json::from_value::<TableProvisionJournalV1>(omitted_journal).is_err());
        let mut substituted_journal =
            serde_json::to_value(&journal).expect("table journal fixture");
        substituted_journal["receipts"][0]["computeUnitsConsumed"] = json!(457);
        let substituted_journal: TableProvisionJournalV1 =
            serde_json::from_value(substituted_journal).expect("substituted CU shape");
        assert_ne!(substituted_journal.receipts[0], table_receipt);
    }

    #[test]
    fn resolution_v3_checkpoint_requires_exact_four_mutation_completion() {
        let receipt = |stage: ReceiptStageV1, byte: u8, slot: u64| StageReceiptV1 {
            stage,
            signature: Pubkey::new_from_array([byte; 32]).to_string(),
            slot,
            fee_lamports: 5_000,
            compute_units_consumed: 100_000 + u64::from(byte),
            transfer_fee_lamports: 0,
            arithmetic: ArithmeticPlanV1::default(),
            signed_transaction_sha256: hex(&[byte; 32]),
            resolved_account_keys: vec![Pubkey::new_from_array([byte + 10; 32]).to_string()],
            pre_balances: vec![10_000],
            post_balances: vec![5_000],
            return_data_base64: if stage == ReceiptStageV1::CoreAccept {
                String::new()
            } else {
                BASE64.encode([byte])
            },
            return_data_sha256: if stage == ReceiptStageV1::CoreAccept {
                hex(&Sha256::digest([]))
            } else {
                hex(&Sha256::digest([byte]))
            },
        };
        let receipts = vec![
            receipt(ReceiptStageV1::Submit, 1, 10),
            receipt(ReceiptStageV1::ProviderExecute, 2, 11),
            receipt(ReceiptStageV1::CoreAccept, 3, 12),
            receipt(ReceiptStageV1::Reclaim, 4, 13),
        ];
        assert_eq!(
            receipts
                .iter()
                .map(|row| serde_json::to_value(row).expect("receipt")["stage"]
                    .as_str()
                    .expect("stage")
                    .to_owned())
                .collect::<Vec<_>>(),
            [
                "submit",
                "resolution-provider-execute-v1",
                "core-terminal-accept-v1",
                "reclaim",
            ]
        );
        let complete = CheckpointV1 {
            format: CHECKPOINT_FORMAT.into(),
            input_sha256: "22".repeat(32),
            stage_plan: None,
            receipts: receipts.clone(),
            verified_terminal: true,
        };
        assert!(authenticate_receipt_prefix(&complete, ExpectedClusterV1::Devnet).is_ok());
        assert!(require_terminal_receipts(&complete, ExpectedClusterV1::Devnet).is_ok());

        let mut zero_fee = complete.clone();
        zero_fee.receipts[0].fee_lamports = 0;
        assert!(authenticate_receipt_prefix(&zero_fee, ExpectedClusterV1::OwnedLoopback).is_ok());
        assert!(authenticate_receipt_prefix(&zero_fee, ExpectedClusterV1::Devnet).is_err());

        let mut duplicate_signature = complete.clone();
        duplicate_signature.receipts[2].signature =
            duplicate_signature.receipts[1].signature.clone();
        assert!(
            authenticate_receipt_prefix(&duplicate_signature, ExpectedClusterV1::Devnet).is_err()
        );
        let mut same_slot = complete.clone();
        same_slot.receipts[2].slot = same_slot.receipts[1].slot;
        assert!(authenticate_receipt_prefix(&same_slot, ExpectedClusterV1::Devnet).is_err());
        let mut reordered = complete.clone();
        reordered.receipts.swap(1, 2);
        assert!(authenticate_receipt_prefix(&reordered, ExpectedClusterV1::Devnet).is_err());
        let mut missing_accept = complete.clone();
        missing_accept.receipts.remove(2);
        missing_accept.verified_terminal = false;
        assert!(authenticate_receipt_prefix(&missing_accept, ExpectedClusterV1::Devnet).is_err());
        assert!(require_terminal_receipts(&missing_accept, ExpectedClusterV1::Devnet).is_err());

        // §7.10 Ruling 5: the chain's stage fixes the count, and only the exact
        // prefix below it is admissible. Nothing here can be widened by input.
        for (stage, owed) in [
            (StageV1::Submit, 0),
            (StageV1::Execute, 1),
            (StageV1::Accept, 2),
            (StageV1::Reclaim, 3),
            (StageV1::Complete, 4),
        ] {
            assert!(require_adoption_coverage(&receipts[..owed], stage).is_ok());
            for wrong in 0..=4 {
                if wrong != owed {
                    assert!(
                        require_adoption_coverage(&receipts[..wrong], stage).is_err(),
                        "{} admitted {wrong} receipts, owes {owed}",
                        stage.label()
                    );
                }
            }
        }
        // A later stage's receipt may never stand in for an earlier one.
        let skipped = vec![receipts[1].clone()];
        assert!(require_adoption_coverage(&skipped, StageV1::Execute).is_err());
        let reordered_prefix = vec![receipts[1].clone(), receipts[0].clone()];
        assert!(require_adoption_coverage(&reordered_prefix, StageV1::Accept).is_err());
    }

    #[test]
    fn resolution_v3_headers_refuse_old_checkpoints_and_v1_alt_journal() {
        for (expected_cluster, old_checkpoint, old_journal) in [
            (
                ExpectedClusterV1::Devnet,
                "dclutch-flagship-resolution-checkpoint-v1",
                "dclutch-flagship-resolution-alt-journal-v1",
            ),
            (
                ExpectedClusterV1::OwnedLoopback,
                "dclutch-owned-loopback-flagship-resolution-checkpoint-v1",
                "dclutch-owned-loopback-flagship-resolution-alt-journal-v1",
            ),
        ] {
            let checkpoint = CheckpointV1 {
                format: old_checkpoint.into(),
                input_sha256: "77".repeat(32),
                stage_plan: None,
                receipts: Vec::new(),
                verified_terminal: false,
            };
            assert!(
                authenticate_checkpoint_identity(&checkpoint, &"77".repeat(32), expected_cluster,)
                    .is_err()
            );
            let journal = TableProvisionJournalV1 {
                format: old_journal.into(),
                producer_identity_sha256: "88".repeat(32),
                phase: DurablePhaseV1::Finalized,
                intent: None,
                intent_sha256: None,
                signed_transaction_base64: None,
                signed_transaction_sha256: None,
                expected_signature: None,
                finalized: None,
                receipts: Vec::new(),
            };
            assert!(
                authenticate_table_journal_identity(&journal, &"88".repeat(32), expected_cluster,)
                    .is_err()
            );
        }
        let old_v2 = CheckpointV1 {
            format: "dclutch-owned-loopback-flagship-resolution-checkpoint-v2".into(),
            input_sha256: "99".repeat(32),
            stage_plan: None,
            receipts: Vec::new(),
            verified_terminal: false,
        };
        assert!(
            authenticate_checkpoint_identity(
                &old_v2,
                &"99".repeat(32),
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );
        let old_v2_journal = TableProvisionJournalV1 {
            format: "dclutch-owned-loopback-flagship-resolution-alt-journal-v2".into(),
            producer_identity_sha256: "aa".repeat(32),
            phase: DurablePhaseV1::Finalized,
            intent: None,
            intent_sha256: None,
            signed_transaction_base64: None,
            signed_transaction_sha256: None,
            expected_signature: None,
            finalized: None,
            receipts: Vec::new(),
        };
        assert!(
            authenticate_table_journal_identity(
                &old_v2_journal,
                &"aa".repeat(32),
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );
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
    fn durable_send_boundary_refuses_every_phase_except_fsynced_dispatching() {
        assert!(authenticate_send_boundary(DurablePhaseV1::Planned).is_err());
        assert!(authenticate_send_boundary(DurablePhaseV1::SignedNotSubmitted).is_err());
        assert!(authenticate_send_boundary(DurablePhaseV1::Dispatching).is_ok());
        assert!(authenticate_send_boundary(DurablePhaseV1::Submitted).is_err());
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

    fn checkpoint_with_reclaim_floor(reclaim_after_unix_seconds: i64) -> ProducerCheckpointV1 {
        let mut planned_input = sample_input();
        planned_input.reclaim_after_unix_seconds = reclaim_after_unix_seconds;
        ProducerCheckpointV1 {
            format: PRODUCER_CHECKPOINT_FORMAT.into(),
            plan_sha256: hex(&[1; 32]),
            campaign_evidence_sha256: hex(&[2; 32]),
            refreshed_evidence_sha256: None,
            pyth_facts_sha256: hex(&[3; 32]),
            observation_slot: 100,
            observation_unix_timestamp: 10_000,
            market: Pubkey::new_from_array([60; 32]).to_string(),
            generation: 1,
            payer: Pubkey::new_from_array([80; 32]).to_string(),
            authority: Pubkey::new_from_array([80; 32]).to_string(),
            tables: BTreeMap::new(),
            routes: BTreeMap::new(),
            planned_input,
            flagship_input: None,
        }
    }

    /// The first produce derives the floor from its own observation.
    #[test]
    fn absent_prior_derives_the_reclaim_floor_from_this_observation() {
        // Observation past the window end: the observation is the max.
        assert_eq!(
            pinned_reclaim_after_unix_seconds(None, 10_000, 5_000).expect("derived"),
            10_000 + FLAGSHIP_RECLAIM_DELAY_SECONDS_V1
        );
        // Observation inside the window: the window end is the max.
        assert_eq!(
            pinned_reclaim_after_unix_seconds(None, 4_000, 5_000).expect("derived"),
            5_000 + FLAGSHIP_RECLAIM_DELAY_SECONDS_V1
        );
    }

    /// The deadlock: with the floor re-derived, a later observation would move
    /// it and the resume comparison would refuse forever. Pinned, it does not
    /// move, no matter how far the clock has run past the window.
    #[test]
    fn resume_pins_the_reclaim_floor_against_an_advancing_clock() {
        let committed = 10_000 + FLAGSHIP_RECLAIM_DELAY_SECONDS_V1;
        let prior = checkpoint_with_reclaim_floor(committed);
        for observation in [10_000, 20_000, 400_000, 10_000_000] {
            assert_eq!(
                pinned_reclaim_after_unix_seconds(Some(&prior), observation, 5_000)
                    .expect("pinned floor"),
                committed,
                "the carried floor moved at observation {observation}"
            );
        }
    }

    /// Carried, not believed. A floor below the terminal window bound is what
    /// `dclutch-provider-transport-v3-operator` refuses outright, so the
    /// producer refuses to carry one.
    #[test]
    fn carried_reclaim_floor_below_the_window_bound_refuses() {
        let prior = checkpoint_with_reclaim_floor(5_000 + FLAGSHIP_RECLAIM_DELAY_SECONDS_V1 - 1);
        let error = pinned_reclaim_after_unix_seconds(Some(&prior), 10_000, 5_000)
            .expect_err("floor below the window bound must refuse");
        assert!(
            format!("{error:?}").contains("is below the terminal window bound"),
            "unexpected refusal: {error:?}"
        );
    }

    /// The other direction: a hand-edited floor cannot be pushed past what this
    /// observation would itself derive, so a resume cannot strand the reclaim.
    #[test]
    fn carried_reclaim_floor_ahead_of_this_observation_refuses() {
        let prior = checkpoint_with_reclaim_floor(10_000 + FLAGSHIP_RECLAIM_DELAY_SECONDS_V1 + 1);
        let error = pinned_reclaim_after_unix_seconds(Some(&prior), 10_000, 5_000)
            .expect_err("floor ahead of the derivation must refuse");
        assert!(
            format!("{error:?}").contains("is ahead of the derivation"),
            "unexpected refusal: {error:?}"
        );
        // The same floor is admitted once the clock legitimately reaches it.
        assert!(pinned_reclaim_after_unix_seconds(Some(&prior), 10_001, 5_000).is_ok());
    }

    fn clock_row(
        slot: u64,
        epoch_start: i64,
        epoch: u64,
        leader: u64,
        unix: i64,
    ) -> DurableAccountStateV1 {
        let mut data = Vec::with_capacity(CLOCK_SYSVAR_BYTES_V1);
        data.extend_from_slice(&slot.to_le_bytes());
        data.extend_from_slice(&epoch_start.to_le_bytes());
        data.extend_from_slice(&epoch.to_le_bytes());
        data.extend_from_slice(&leader.to_le_bytes());
        data.extend_from_slice(&unix.to_le_bytes());
        DurableAccountStateV1 {
            owner: sysvar::ID.to_string(),
            lamports: 1_169_280,
            executable: false,
            data_base64: BASE64.encode(&data),
            data_sha256: hex(&Sha256::digest(&data)),
        }
    }

    /// EVIDENCE_REFRESH_V1 §7.5/§7.6: the Clock is released from byte-equality
    /// and bounded instead, and the release must not leak to any other row.
    #[test]
    fn prestate_releases_only_the_clock_and_bounds_every_field_it_releases() {
        let (mut plan, _) = sample_durable_stage_plan();
        let clock_key = sysvar::clock::ID.to_string();
        let planned_clock = clock_row(111, 200, 3, 5, 222);
        plan.pre_accounts
            .insert(clock_key.clone(), planned_clock.clone());
        let band = ClockBandV1 {
            lower: 222,
            upper: Some(500),
        };
        let rows = |clock: &DurableAccountStateV1| {
            let mut observed = plan.pre_accounts.clone();
            observed.insert(clock_key.clone(), clock.clone());
            observed
        };
        let refusal = |observed: BTreeMap<String, DurableAccountStateV1>| {
            authenticate_prestate_rows(&plan, observed, || Ok(band))
                .expect_err("must refuse")
                .to_string()
        };

        // Green: the clock advanced, inside the band, and the plan still holds.
        // This is the direction the byte-pin made impossible on a live cluster.
        assert!(
            authenticate_prestate_rows(&plan, rows(&clock_row(9_001, 200, 3, 5, 499)), || Ok(band))
                .is_ok()
        );
        assert!(
            authenticate_prestate_rows(&plan, rows(&planned_clock), || Ok(band)).is_ok(),
            "an unmoved clock is still inside its own band"
        );

        // Past the band, on a refusal of its own that names the band.
        let outside = refusal(rows(&clock_row(9_001, 200, 3, 5, 501)));
        assert!(
            outside.contains("is outside the admissible band [222, 500]"),
            "unexpected refusal: {outside}"
        );

        // Every released field keeps a bound: nothing is simply dropped.
        for (field, hostile) in [
            ("unixTimestamp", clock_row(9_001, 200, 3, 5, 221)),
            ("slot", clock_row(110, 200, 3, 5, 300)),
            ("epoch", clock_row(9_001, 200, 2, 5, 300)),
            ("epochStartTimestamp", clock_row(9_001, 199, 3, 5, 300)),
            ("leaderScheduleEpoch", clock_row(9_001, 200, 3, 4, 300)),
        ] {
            let error = refusal(rows(&hostile));
            assert!(
                error.contains(&format!("clock rewound: {field}")),
                "unexpected refusal for {field}: {error}"
            );
        }

        // The release is per-field: the Clock row's own non-advancing
        // attributes keep the byte-pin and the ORIGINAL refusal.
        let mut reparented = planned_clock.clone();
        reparented.owner = system_program::ID.to_string();
        assert_eq!(
            refusal(rows(&reparented)),
            "provider full resolved account prestate changed"
        );
        let mut refunded = planned_clock.clone();
        refunded.lamports = planned_clock.lamports + 1;
        assert_eq!(
            refusal(rows(&refunded)),
            "provider full resolved account prestate changed"
        );
        let mut truncated = planned_clock.clone();
        let short = vec![0_u8; CLOCK_SYSVAR_BYTES_V1 - 1];
        truncated.data_base64 = BASE64.encode(&short);
        truncated.data_sha256 = hex(&Sha256::digest(&short));
        assert_eq!(
            refusal(rows(&truncated)),
            "provider full resolved account prestate changed",
            "a re-widthed Clock row is caught by the surviving data-length pin"
        );

        // The band must not leak generality: a single byte moved on any OTHER
        // row still refuses on the original string, even while the clock is
        // legitimately advancing inside its band.
        let victim = plan
            .resolved_account_keys
            .first()
            .cloned()
            .expect("fixture resolved key");
        let mut tampered = rows(&clock_row(9_001, 200, 3, 5, 499));
        let row = tampered.get_mut(&victim).expect("fixture row");
        let moved_to = row.lamports + 1;
        let was = row.lamports;
        row.lamports = moved_to;
        // The relation is unchanged and still byte-exact. What it now does is
        // say WHICH row stopped satisfying it, and in what field — a prestate
        // pin spans dozens of accounts, and one string for all of them is a
        // refusal that can be read and not acted on.
        assert_eq!(
            refusal(tampered),
            format!(
                "provider full resolved account prestate changed: {victim}: lamports {was} -> {moved_to}"
            )
        );

        // A Clock the plan pinned but the reading never produced is structural,
        // and the refusal says which side is missing it rather than leaving the
        // reader to guess between the two asymmetric cases.
        let mut absent = rows(&planned_clock);
        absent.remove(&clock_key);
        assert_eq!(
            refusal(absent),
            "provider full resolved account prestate changed: the Clock was resolved when this \
             stage was planned and is absent now"
        );
    }

    /// EVIDENCE_REFRESH_V1 §7.6, Ruling 3/4: each stage derives its own band,
    /// and only from rows that still carry a byte-exact pin.
    #[test]
    fn admissible_clock_band_is_stage_specific_and_reads_only_pinned_rows() {
        let selected = sample_selected();
        let (mut plan, _) = sample_durable_stage_plan();

        // Core terminal accept reads no clock: monotonicity is the whole band.
        plan.stage = StageV1::Accept;
        let accept = admissible_clock_band(&selected, &plan).expect("accept band");
        assert_eq!(accept.lower, plan.observation_unix_timestamp);
        assert_eq!(accept.upper, None);

        // Complete never carries a plan, so it can never derive a band.
        plan.stage = StageV1::Complete;
        assert!(admissible_clock_band(&selected, &plan).is_err());

        // A band endpoint whose account the plan did not resolve is a
        // structural break, refused by name rather than silently widened.
        plan.stage = StageV1::Reclaim;
        let error = admissible_clock_band(&selected, &plan)
            .expect_err("reclaim band without a pinned lifecycle row")
            .to_string();
        assert!(
            error.contains("clock band lost its pinned provider lifecycle row"),
            "unexpected refusal: {error}"
        );
        plan.stage = StageV1::Submit;
        let error = admissible_clock_band(&selected, &plan)
            .expect_err("submit band without a pinned WindowSpec row")
            .to_string();
        assert!(
            error.contains("clock band lost its pinned WindowSpec row"),
            "unexpected refusal: {error}"
        );
    }

    /// `docs/design/EVIDENCE_REFRESH_V1.md` §7.12 Ruling 1: the routing-table
    /// gate is per-stage reachability, not per-life freshness. Driving chain can
    /// only exhibit the positions the substrate happens to be in; the clause is
    /// a pure function of two stages, so prove it over the whole lattice.
    #[test]
    fn routing_table_stage_gate_is_per_stage_reachability() {
        let routed = [StageV1::Submit, StageV1::Execute, StageV1::Reclaim];
        let positions = [
            StageV1::Submit,
            StageV1::Execute,
            StageV1::Accept,
            StageV1::Reclaim,
            StageV1::Complete,
        ];
        for position in positions {
            for stage in routed {
                let admitted = table_stage_open(position, stage, &[], None).is_ok();
                assert_eq!(
                    admitted,
                    position <= stage,
                    "position {} against the {} table",
                    position.label(),
                    stage.label()
                );
            }
        }

        // The fresh-life path does not move: a life at Submit still writes all
        // three tables, which is the whole flagship founding sequence.
        for stage in routed {
            table_stage_open(StageV1::Submit, stage, &[], None).expect("fresh life");
        }

        // Submit's meaning stays frozen the instant the life leaves Submit, and
        // the refusal names the stage and the position rather than the feature.
        let error = table_stage_open(StageV1::Execute, StageV1::Submit, &[], None)
            .expect_err("submit table mid-life")
            .to_string();
        assert!(
            error.contains(
                "the submit routing table may not be provisioned: the life is already at execute"
            ),
            "unexpected refusal: {error}"
        );

        // A finished life writes no table at all.
        for stage in routed {
            let error = table_stage_open(StageV1::Complete, stage, &[], None)
                .expect_err("complete life")
                .to_string();
            assert!(
                error.contains("the life is already at complete"),
                "unexpected refusal: {error}"
            );
        }
    }

    /// §7.12 Ruling 2: chain answers for landed packets, the standing checkpoint
    /// answers for signed-but-unsent ones. A stage plan is invisible to
    /// `classify`, so Ruling 1 alone does not buy the property.
    #[test]
    fn routing_table_stage_gate_reads_planned_packets_from_the_standing_checkpoint() {
        // A landed Execute receipt freezes the Execute table even where Ruling 1
        // would still admit it. Accept rides Execute's table, so its receipt
        // freezes the same one.
        for landed in [ReceiptStageV1::ProviderExecute, ReceiptStageV1::CoreAccept] {
            let error = table_stage_open(StageV1::Execute, StageV1::Execute, &[landed], None)
                .expect_err("landed receipt")
                .to_string();
            assert!(
                error.contains(
                    "the execute routing table may not be provisioned: the standing checkpoint holds a landed"
                ),
                "unexpected refusal: {error}"
            );
        }

        // A Submit receipt says nothing about Execute's table.
        table_stage_open(
            StageV1::Execute,
            StageV1::Execute,
            &[ReceiptStageV1::Submit],
            None,
        )
        .expect("a landed submit does not freeze the execute table");

        // A signed-but-unsent Execute packet does, and chain cannot see it:
        // `classify` reads accounts, and an unsent packet has touched none.
        let error = table_stage_open(
            StageV1::Execute,
            StageV1::Execute,
            &[ReceiptStageV1::Submit],
            Some(StageV1::Execute),
        )
        .expect_err("planned packet")
        .to_string();
        assert!(
            error.contains(
                "the execute routing table may not be provisioned: the standing checkpoint already plans an execute packet"
            ),
            "unexpected refusal: {error}"
        );

        // An Accept packet is planned against Execute's table too.
        assert!(
            table_stage_open(
                StageV1::Execute,
                StageV1::Execute,
                &[ReceiptStageV1::Submit],
                Some(StageV1::Accept)
            )
            .is_err()
        );

        // …and neither says anything about Reclaim's.
        table_stage_open(
            StageV1::Execute,
            StageV1::Reclaim,
            &[ReceiptStageV1::Submit],
            Some(StageV1::Execute),
        )
        .expect("a planned execute packet does not freeze the reclaim table");
    }
}
