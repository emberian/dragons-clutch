//! Strict JSON transport over the native Source-readiness owner.

use std::collections::BTreeMap;
use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionRetirementReceiptFactsV3,
    authenticate_resolution_retirement_receipt_v3,
    derive_resolution_admit_terminal_base_coordinates_v3,
    derive_resolution_admit_terminal_detail_coordinates_v3,
    derive_resolution_admit_terminal_product_coordinates_v3,
    derive_resolution_close_fund_coordinates_v1, derive_resolution_funding_base_coordinates_v3,
    derive_resolution_funding_detail_coordinates_v3,
    derive_resolution_recovery_policy_coordinates_v3,
};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_program::{instruction::Instruction, pubkey::Pubkey};

use crate::{
    FundingReadinessCoordinatesV1, FundingReadinessFrameV1, FundingReadinessGeometryV1,
    FundingReadinessInstructionPlanV1, FundingReadinessPlanV1, FundingReadinessRecordCoordinatesV1,
    SourceCloseFundFrameV1, SourceCloseFundPlanV1, SourceTerminalFrameV1, SourceTerminalPlanV1,
    plan_funding_readiness_v1, plan_source_close_fund_v1, plan_source_terminal_v1,
};

const SNAPSHOT_FORMAT_V1: &str = "dclutch-source-readiness-snapshot-v1";
const PLAN_FORMAT_V1: &str = "dclutch-source-readiness-plan-v1";
const MARKET_FORMAT_V1: &str = "dclutch-source-readiness-market-v1";
const RECORDS_FORMAT_V1: &str = "dclutch-source-readiness-records-v1";
const SOURCE_FORMAT_V1: &str = "dclutch-source-readiness-source-v1";
const TERMINAL_BASE_FORMAT_V1: &str = "dclutch-source-terminal-base-v1";
const TERMINAL_PRODUCT_FORMAT_V1: &str = "dclutch-source-terminal-product-v1";
const TERMINAL_DETAIL_FORMAT_V1: &str = "dclutch-source-terminal-detail-v1";
const TERMINAL_SNAPSHOT_FORMAT_V1: &str = "dclutch-source-terminal-snapshot-v1";
const TERMINAL_PLAN_FORMAT_V1: &str = "dclutch-source-terminal-plan-v1";
const CLOSE_DETAIL_FORMAT_V1: &str = "dclutch-source-close-detail-v1";
const CLOSE_SNAPSHOT_FORMAT_V1: &str = "dclutch-source-close-snapshot-v1";
const CLOSE_PLAN_FORMAT_V1: &str = "dclutch-source-close-plan-v1";
const CLOSE_VERIFY_FORMAT_V1: &str = "dclutch-source-close-verify-v1";
const MAX_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_ACCOUNT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_ACCOUNT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordCoordinatesWireV1 {
    raw: String,
    staging: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoordinatesWireV1 {
    market: String,
    source_material: RecordCoordinatesWireV1,
    capability_manifest: RecordCoordinatesWireV1,
    recovery_policy: Option<RecordCoordinatesWireV1>,
    source_state: String,
    funding_ledger: String,
    beneficiary: String,
    activation_receipt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameWireV1 {
    coordinates: CoordinatesWireV1,
    activation_cache: String,
    registry_program: String,
    core_program: String,
    core_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountWireV1 {
    address: String,
    owner: String,
    lamports: String,
    executable: bool,
    data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    frame: FrameWireV1,
    accounts: Vec<AccountWireV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MarketCoordinateWireV1 {
    address: String,
    owner: String,
    executable: bool,
    data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MarketWireV1 {
    format: String,
    market: MarketCoordinateWireV1,
    core_program: String,
    registry_program: String,
    resolution_program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordsWireV1 {
    format: String,
    market_address: String,
    market_owner: String,
    market_executable: bool,
    market_data_base64: String,
    core_program: String,
    registry_program: String,
    resolution_program: String,
    source_material_data_base64: String,
    capability_manifest_data_base64: String,
    recovery_policy_data_base64: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceWireV1 {
    format: String,
    market_data_base64: String,
    registry_program: String,
    source_material_data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalBaseWireV1 {
    format: String,
    market: MarketCoordinateWireV1,
    core_program: String,
    registry_program: String,
    resolution_program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalProductWireV1 {
    format: String,
    market_address: String,
    market_data_base64: String,
    registry_program: String,
    product_data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalDetailWireV1 {
    format: String,
    market_address: String,
    market_data_base64: String,
    registry_program: String,
    resolution_program: String,
    source_state_address: String,
    source_state_data_base64: String,
    product_data_base64: String,
    result_domain_data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalFrameWireV1 {
    readiness: FrameWireV1,
    certificate: String,
    product_raw: String,
    product_staging: String,
    result_domain_raw: String,
    result_domain_staging: String,
    portfolio_raw: String,
    portfolio_staging: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalSnapshotWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    frame: TerminalFrameWireV1,
    accounts: Vec<AccountWireV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseDetailWireV1 {
    format: String,
    market_address: String,
    market_data_base64: String,
    resolution_program: String,
    source_state_address: String,
    source_state_data_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseFrameWireV1 {
    readiness: FrameWireV1,
    certificate: String,
    closure_receipt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseSnapshotWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    frame: CloseFrameWireV1,
    accounts: Vec<AccountWireV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseExpectedWireV1 {
    market: String,
    generation: String,
    closure_receipt: String,
    source_state: String,
    source_material: String,
    capability_manifest: String,
    terminal_certificate: String,
    beneficiary: String,
    selector: String,
    terminal_sequence: String,
    source_state_digest: String,
    terminal_certificate_digest: String,
    funding_set_digest: String,
    source_refund_lamports: String,
    ledger_remaining_native_principal: String,
    ledger_rent_lamports: String,
    ledger_lamport_surplus: String,
    refund_lamports: String,
    closed_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseVerifyWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    resolution_program: String,
    receipt: AccountWireV1,
    rent_sysvar: AccountWireV1,
    expected: CloseExpectedWireV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaOutputV1 {
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionOutputV1 {
    program: String,
    accounts: Vec<MetaOutputV1>,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepayOutputV1 {
    destination: String,
    lamports: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountSetsOutputV1 {
    protocol_writable: Vec<String>,
    completion: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryOutputV1 {
    protocol_account_count: usize,
    protocol_unique_account_count: usize,
    protocol_writable_count: usize,
    protocol_signer_count: usize,
    protocol_data_len: usize,
    transaction_instruction_count_without_compute_budget: usize,
    transaction_lock_count_without_payer: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanOutputV1 {
    format: &'static str,
    route: &'static str,
    observed_slot: String,
    instruction: Option<InstructionOutputV1>,
    prepay: Option<PrepayOutputV1>,
    accounts: Option<AccountSetsOutputV1>,
    geometry: Option<GeometryOutputV1>,
    facts: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaseCoordinatesOutputV1 {
    activation_cache: String,
    core_programdata: String,
    resolution_programdata: String,
    source_material: String,
    source_material_staging: String,
    capability_manifest: String,
    capability_manifest_staging: String,
    source_state: String,
    activation_receipt: String,
    beneficiary: String,
    generation: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DetailCoordinatesOutputV1 {
    recovery_policy: Option<String>,
    recovery_policy_staging: Option<String>,
    funding_ledger: String,
    funding_entry_indices: [u16; 3],
    frame: FrameOutputV1,
    addresses: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordCoordinatesOutputV1 {
    raw: String,
    staging: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinatesOutputV1 {
    market: String,
    source_material: RecordCoordinatesOutputV1,
    capability_manifest: RecordCoordinatesOutputV1,
    recovery_policy: Option<RecordCoordinatesOutputV1>,
    source_state: String,
    funding_ledger: String,
    beneficiary: String,
    activation_receipt: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameOutputV1 {
    coordinates: CoordinatesOutputV1,
    activation_cache: String,
    registry_program: String,
    core_program: String,
    core_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryCoordinatesOutputV1 {
    recovery_policy: Option<String>,
    recovery_policy_staging: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalBaseOutputV1 {
    readiness: BaseCoordinatesOutputV1,
    product_raw: String,
    product_staging: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalProductOutputV1 {
    result_domain_raw: String,
    result_domain_staging: String,
    portfolio_raw: String,
    portfolio_staging: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalDetailOutputV1 {
    result_domain_raw: String,
    result_domain_staging: String,
    portfolio_raw: String,
    portfolio_staging: String,
    certificate: String,
    outcome_count: u32,
    terminal_sequence: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloseDetailOutputV1 {
    certificate: String,
    closure_receipt: String,
    terminal_sequence: String,
    closure_sequence: String,
}

/// Derive the record and child addresses selected by one exact Core Market.
pub fn derive_source_readiness_base_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: MarketWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source readiness Market schema: {error}"))?;
    if wire.format != MARKET_FORMAT_V1 {
        return Err("Source readiness Market has another format".to_owned());
    }
    let market_data = exact_base64(&wire.market.data_base64, "Market data")?;
    let result = derive_resolution_funding_base_coordinates_v3(
        exact_key(&wire.market.address, "Market")?,
        exact_key(&wire.market.owner, "Market owner")?,
        wire.market.executable,
        &market_data,
        exact_key(&wire.core_program, "Core program")?,
        exact_key(&wire.registry_program, "Registry program")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
    )
    .map_err(|error| format!("Source readiness Market derivation refused: {error:?}"))?;
    serde_json::to_string(&BaseCoordinatesOutputV1 {
        activation_cache: result.activation_cache.to_string(),
        core_programdata: result.core_programdata.to_string(),
        resolution_programdata: result.resolution_programdata.to_string(),
        source_material: result.source_material.to_string(),
        source_material_staging: result.source_material_staging.to_string(),
        capability_manifest: result.capability_manifest.to_string(),
        capability_manifest_staging: result.capability_manifest_staging.to_string(),
        source_state: result.source_state.to_string(),
        activation_receipt: result.activation_receipt.to_string(),
        beneficiary: result.beneficiary.to_string(),
        generation: result.generation.to_string(),
    })
    .map_err(|error| format!("Source readiness base-coordinate encoding: {error}"))
}

/// Derive the optional recovery-policy pair after the Source record is read.
pub fn derive_source_readiness_recovery_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SourceWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source readiness Source schema: {error}"))?;
    if wire.format != SOURCE_FORMAT_V1 {
        return Err("Source readiness Source has another format".to_owned());
    }
    let result = derive_resolution_recovery_policy_coordinates_v3(
        &exact_base64(&wire.market_data_base64, "Market data")?,
        exact_key(&wire.registry_program, "Registry program")?,
        &exact_base64(&wire.source_material_data_base64, "Source material data")?,
    )
    .map_err(|error| format!("Source readiness recovery derivation refused: {error:?}"))?;
    let (recovery_policy, recovery_policy_staging) = match result {
        Some((raw, staging)) => (Some(raw.to_string()), Some(staging.to_string())),
        None => (None, None),
    };
    serde_json::to_string(&RecoveryCoordinatesOutputV1 {
        recovery_policy,
        recovery_policy_staging,
    })
    .map_err(|error| format!("Source readiness recovery-coordinate encoding: {error}"))
}

/// Derive the optional recovery record and subset-ledger address without a
/// caller-supplied mask or entry selection.
pub fn derive_source_readiness_detail_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: RecordsWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source readiness records schema: {error}"))?;
    if wire.format != RECORDS_FORMAT_V1 {
        return Err("Source readiness records have another format".to_owned());
    }
    let market_data = exact_base64(&wire.market_data_base64, "Market data")?;
    let source_material = exact_base64(&wire.source_material_data_base64, "Source material data")?;
    let capability_manifest = exact_base64(
        &wire.capability_manifest_data_base64,
        "capability manifest data",
    )?;
    let recovery_policy = wire
        .recovery_policy_data_base64
        .as_deref()
        .map(|value| exact_base64(value, "recovery policy data"))
        .transpose()?;
    let market_address = exact_key(&wire.market_address, "Market")?;
    let market_owner = exact_key(&wire.market_owner, "Market owner")?;
    let core_program = exact_key(&wire.core_program, "Core program")?;
    let registry_program = exact_key(&wire.registry_program, "Registry program")?;
    let resolution_program = exact_key(&wire.resolution_program, "Resolution program")?;
    let result = derive_resolution_funding_detail_coordinates_v3(
        market_address,
        &market_data,
        registry_program,
        resolution_program,
        &source_material,
        &capability_manifest,
        recovery_policy.as_deref(),
    )
    .map_err(|error| format!("Source readiness record derivation refused: {error:?}"))?;
    let base = derive_resolution_funding_base_coordinates_v3(
        market_address,
        market_owner,
        wire.market_executable,
        &market_data,
        core_program,
        registry_program,
        resolution_program,
    )
    .map_err(|error| format!("Source readiness Market derivation refused: {error:?}"))?;
    let frame = FundingReadinessFrameV1 {
        coordinates: FundingReadinessCoordinatesV1 {
            market: market_address,
            source_material: FundingReadinessRecordCoordinatesV1 {
                raw: base.source_material,
                staging: base.source_material_staging,
            },
            capability_manifest: FundingReadinessRecordCoordinatesV1 {
                raw: base.capability_manifest,
                staging: base.capability_manifest_staging,
            },
            recovery_policy: result
                .recovery_policy
                .zip(result.recovery_policy_staging)
                .map(|(raw, staging)| FundingReadinessRecordCoordinatesV1 { raw, staging }),
            source_state: base.source_state,
            funding_ledger: result.funding_ledger,
            beneficiary: base.beneficiary,
            activation_receipt: base.activation_receipt,
        },
        activation_cache: base.activation_cache,
        registry_program,
        core_program,
        core_programdata: base.core_programdata,
        resolution_program,
        resolution_programdata: base.resolution_programdata,
    };
    let addresses = crate::funding_readiness_observation_addresses_v1(&frame)
        .map_err(|error| error.message().to_owned())?
        .into_iter()
        .map(|address| address.to_string())
        .collect();
    serde_json::to_string(&DetailCoordinatesOutputV1 {
        recovery_policy: result.recovery_policy.map(|value| value.to_string()),
        recovery_policy_staging: result
            .recovery_policy_staging
            .map(|value| value.to_string()),
        funding_ledger: result.funding_ledger.to_string(),
        funding_entry_indices: result.funding_entry_indices,
        frame: frame_output(&frame),
        addresses,
    })
    .map_err(|error| format!("Source readiness detail-coordinate encoding: {error}"))
}

/// Derive Source/record coordinates needed before terminal Product reads.
pub fn derive_source_terminal_base_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: TerminalBaseWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source terminal base schema: {error}"))?;
    if wire.format != TERMINAL_BASE_FORMAT_V1 {
        return Err("Source terminal base has another format".to_owned());
    }
    let market_data = exact_base64(&wire.market.data_base64, "Market data")?;
    let result = derive_resolution_admit_terminal_base_coordinates_v3(
        exact_key(&wire.market.address, "Market")?,
        exact_key(&wire.market.owner, "Market owner")?,
        wire.market.executable,
        &market_data,
        exact_key(&wire.core_program, "Core program")?,
        exact_key(&wire.registry_program, "Registry program")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
    )
    .map_err(|error| format!("Source terminal base derivation refused: {error:?}"))?;
    let funding = result.funding;
    serde_json::to_string(&TerminalBaseOutputV1 {
        readiness: BaseCoordinatesOutputV1 {
            activation_cache: funding.activation_cache.to_string(),
            core_programdata: funding.core_programdata.to_string(),
            resolution_programdata: funding.resolution_programdata.to_string(),
            source_material: funding.source_material.to_string(),
            source_material_staging: funding.source_material_staging.to_string(),
            capability_manifest: funding.capability_manifest.to_string(),
            capability_manifest_staging: funding.capability_manifest_staging.to_string(),
            source_state: funding.source_state.to_string(),
            activation_receipt: funding.activation_receipt.to_string(),
            beneficiary: funding.beneficiary.to_string(),
            generation: funding.generation.to_string(),
        },
        product_raw: result.product_raw.to_string(),
        product_staging: result.product_staging.to_string(),
    })
    .map_err(|error| format!("Source terminal base encoding: {error}"))
}

/// Derive Product child record addresses from the Market-selected root bytes.
pub fn derive_source_terminal_product_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: TerminalProductWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source terminal Product schema: {error}"))?;
    if wire.format != TERMINAL_PRODUCT_FORMAT_V1 {
        return Err("Source terminal Product has another format".to_owned());
    }
    let result = derive_resolution_admit_terminal_product_coordinates_v3(
        exact_key(&wire.market_address, "Market")?,
        &exact_base64(&wire.market_data_base64, "Market data")?,
        exact_key(&wire.registry_program, "Registry program")?,
        &exact_base64(&wire.product_data_base64, "Product data")?,
    )
    .map_err(|error| format!("Source terminal Product derivation refused: {error:?}"))?;
    serde_json::to_string(&TerminalProductOutputV1 {
        result_domain_raw: result.result_domain_raw.to_string(),
        result_domain_staging: result.result_domain_staging.to_string(),
        portfolio_raw: result.portfolio_raw.to_string(),
        portfolio_staging: result.portfolio_staging.to_string(),
    })
    .map_err(|error| format!("Source terminal Product encoding: {error}"))
}

/// Derive the terminal certificate from exact Source and Product-domain bytes.
pub fn derive_source_terminal_detail_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: TerminalDetailWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source terminal detail schema: {error}"))?;
    if wire.format != TERMINAL_DETAIL_FORMAT_V1 {
        return Err("Source terminal detail has another format".to_owned());
    }
    let result = derive_resolution_admit_terminal_detail_coordinates_v3(
        exact_key(&wire.market_address, "Market")?,
        &exact_base64(&wire.market_data_base64, "Market data")?,
        exact_key(&wire.registry_program, "Registry program")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
        exact_key(&wire.source_state_address, "Source state")?,
        &exact_base64(&wire.source_state_data_base64, "Source state data")?,
        &exact_base64(&wire.product_data_base64, "Product data")?,
        &exact_base64(&wire.result_domain_data_base64, "ResultDomain data")?,
    )
    .map_err(|error| format!("Source terminal detail derivation refused: {error:?}"))?;
    serde_json::to_string(&TerminalDetailOutputV1 {
        result_domain_raw: result.result_domain_raw.to_string(),
        result_domain_staging: result.result_domain_staging.to_string(),
        portfolio_raw: result.portfolio_raw.to_string(),
        portfolio_staging: result.portfolio_staging.to_string(),
        certificate: result.certificate.to_string(),
        outcome_count: result.outcome_count,
        terminal_sequence: result.terminal_sequence.to_string(),
    })
    .map_err(|error| format!("Source terminal detail encoding: {error}"))
}

/// Derive the admitted certificate and canonical closure receipt from exact
/// Market and Source bytes.
pub fn derive_source_close_detail_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: CloseDetailWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source close detail schema: {error}"))?;
    if wire.format != CLOSE_DETAIL_FORMAT_V1 {
        return Err("Source close detail has another format".to_owned());
    }
    let result = derive_resolution_close_fund_coordinates_v1(
        exact_key(&wire.market_address, "Market")?,
        &exact_base64(&wire.market_data_base64, "Market data")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
        exact_key(&wire.source_state_address, "Source state")?,
        &exact_base64(&wire.source_state_data_base64, "Source state data")?,
    )
    .map_err(|error| format!("Source close detail derivation refused: {error:?}"))?;
    serde_json::to_string(&CloseDetailOutputV1 {
        certificate: result.certificate.to_string(),
        closure_receipt: result.closure_receipt.to_string(),
        terminal_sequence: result.terminal_sequence.to_string(),
        closure_sequence: result.closure_sequence.to_string(),
    })
    .map_err(|error| format!("Source close detail encoding: {error}"))
}

fn frame_output(frame: &FundingReadinessFrameV1) -> FrameOutputV1 {
    let record = |value: FundingReadinessRecordCoordinatesV1| RecordCoordinatesOutputV1 {
        raw: value.raw.to_string(),
        staging: value.staging.to_string(),
    };
    FrameOutputV1 {
        coordinates: CoordinatesOutputV1 {
            market: frame.coordinates.market.to_string(),
            source_material: record(frame.coordinates.source_material),
            capability_manifest: record(frame.coordinates.capability_manifest),
            recovery_policy: frame.coordinates.recovery_policy.map(record),
            source_state: frame.coordinates.source_state.to_string(),
            funding_ledger: frame.coordinates.funding_ledger.to_string(),
            beneficiary: frame.coordinates.beneficiary.to_string(),
            activation_receipt: frame.coordinates.activation_receipt.to_string(),
        },
        activation_cache: frame.activation_cache.to_string(),
        registry_program: frame.registry_program.to_string(),
        core_program: frame.core_program.to_string(),
        core_programdata: frame.core_programdata.to_string(),
        resolution_program: frame.resolution_program.to_string(),
        resolution_programdata: frame.resolution_programdata.to_string(),
    }
}

/// Strictly decode one finalized snapshot, run the native owner, and encode
/// its canonical plan. The WASM boundary calls this function unchanged.
pub fn plan_funding_readiness_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SnapshotWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source readiness snapshot schema: {error}"))?;
    if wire.format != SNAPSHOT_FORMAT_V1 {
        return Err("Source readiness snapshot has another format".to_owned());
    }
    let observation = Observation {
        slot: exact_u64(&wire.observed_slot, "observed slot", false)?,
        unix_timestamp: exact_i64(&wire.unix_timestamp, "Unix timestamp")?,
        finality: Finality::Finalized,
    };
    let frame = decode_frame(wire.frame)?;
    let mut total = 0_usize;
    let accounts = wire
        .accounts
        .into_iter()
        .map(|account| {
            let data = exact_base64(&account.data_base64, "account data")?;
            total = total.checked_add(data.len()).ok_or_else(|| {
                "Source readiness account bytes overflowed their bound".to_owned()
            })?;
            if total > MAX_TOTAL_ACCOUNT_BYTES {
                return Err("Source readiness account bytes exceed their bounded total".to_owned());
            }
            Ok(ObservedAccount {
                observation,
                key: exact_key(&account.address, "account address")?,
                owner: exact_key(&account.owner, "account owner")?,
                lamports: exact_u64(&account.lamports, "account lamports", true)?,
                executable: account.executable,
                data,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let plan =
        plan_funding_readiness_v1(&frame, &accounts).map_err(|error| error.message().to_owned())?;
    let output = encode_plan(plan, observation.slot);
    serde_json::to_string(&output)
        .map_err(|error| format!("Source readiness plan encoding: {error}"))
}

/// Strictly decode and plan one terminal admission or exact completion.
pub fn plan_source_terminal_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: TerminalSnapshotWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source terminal snapshot schema: {error}"))?;
    if wire.format != TERMINAL_SNAPSHOT_FORMAT_V1 {
        return Err("Source terminal snapshot has another format".to_owned());
    }
    let observation = Observation {
        slot: exact_u64(&wire.observed_slot, "observed slot", false)?,
        unix_timestamp: exact_i64(&wire.unix_timestamp, "Unix timestamp")?,
        finality: Finality::Finalized,
    };
    let frame = SourceTerminalFrameV1 {
        readiness: decode_frame(wire.frame.readiness)?,
        certificate: exact_key(&wire.frame.certificate, "terminal certificate")?,
        product_raw: exact_key(&wire.frame.product_raw, "Product raw")?,
        product_staging: exact_key(&wire.frame.product_staging, "Product staging")?,
        result_domain_raw: exact_key(&wire.frame.result_domain_raw, "ResultDomain raw")?,
        result_domain_staging: exact_key(
            &wire.frame.result_domain_staging,
            "ResultDomain staging",
        )?,
        portfolio_raw: exact_key(&wire.frame.portfolio_raw, "Portfolio raw")?,
        portfolio_staging: exact_key(&wire.frame.portfolio_staging, "Portfolio staging")?,
    };
    let mut total = 0_usize;
    let accounts = wire
        .accounts
        .into_iter()
        .map(|account| {
            let data = exact_base64(&account.data_base64, "account data")?;
            total = total
                .checked_add(data.len())
                .ok_or_else(|| "Source terminal account bytes overflowed their bound".to_owned())?;
            if total > MAX_TOTAL_ACCOUNT_BYTES {
                return Err("Source terminal account bytes exceed their bounded total".to_owned());
            }
            Ok(ObservedAccount {
                observation,
                key: exact_key(&account.address, "account address")?,
                owner: exact_key(&account.owner, "account owner")?,
                lamports: exact_u64(&account.lamports, "account lamports", true)?,
                executable: account.executable,
                data,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let plan =
        plan_source_terminal_v1(&frame, &accounts).map_err(|error| error.message().to_owned())?;
    serde_json::to_string(&encode_terminal_plan(plan, observation.slot))
        .map_err(|error| format!("Source terminal plan encoding: {error}"))
}

/// Strictly decode and plan exact receipt prepayment or V7 direct close.
pub fn plan_source_close_fund_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: CloseSnapshotWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source close snapshot schema: {error}"))?;
    if wire.format != CLOSE_SNAPSHOT_FORMAT_V1 {
        return Err("Source close snapshot has another format".to_owned());
    }
    let observation = Observation {
        slot: exact_u64(&wire.observed_slot, "observed slot", false)?,
        unix_timestamp: exact_i64(&wire.unix_timestamp, "observation timestamp")?,
        finality: Finality::Finalized,
    };
    let frame = SourceCloseFundFrameV1 {
        readiness: decode_frame(wire.frame.readiness)?,
        certificate: exact_key(&wire.frame.certificate, "terminal certificate")?,
        closure_receipt: exact_key(&wire.frame.closure_receipt, "closure receipt")?,
    };
    let mut total = 0_usize;
    let mut accounts = Vec::with_capacity(wire.accounts.len());
    for value in wire.accounts {
        let data = exact_base64(&value.data_base64, "account data")?;
        if data.len() > MAX_ACCOUNT_BYTES {
            return Err("Source close account exceeded its byte bound".to_owned());
        }
        total = total
            .checked_add(data.len())
            .ok_or_else(|| "Source close account bytes overflowed their bound".to_owned())?;
        if total > MAX_TOTAL_ACCOUNT_BYTES {
            return Err("Source close account bytes exceed their bounded total".to_owned());
        }
        accounts.push(ObservedAccount {
            key: exact_key(&value.address, "account address")?,
            owner: exact_key(&value.owner, "account owner")?,
            lamports: exact_u64(&value.lamports, "account lamports", true)?,
            executable: value.executable,
            data,
            observation,
        });
    }
    let plan =
        plan_source_close_fund_v1(&frame, &accounts).map_err(|error| error.message().to_owned())?;
    serde_json::to_string(&encode_close_plan(plan, observation.slot))
        .map_err(|error| format!("Source close plan encoding: {error}"))
}

/// Authenticate the exact finalized closure receipt against facts persisted
/// from the Rust close plan.
pub fn verify_source_close_receipt_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: CloseVerifyWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source close verification schema: {error}"))?;
    if wire.format != CLOSE_VERIFY_FORMAT_V1 {
        return Err("Source close verification has another format".to_owned());
    }
    let observation = Observation {
        slot: exact_u64(&wire.observed_slot, "observed slot", false)?,
        unix_timestamp: exact_i64(&wire.unix_timestamp, "observation timestamp")?,
        finality: Finality::Finalized,
    };
    let account = |value: AccountWireV1, label: &str| -> Result<ObservedAccount, String> {
        Ok(ObservedAccount {
            key: exact_key(&value.address, &format!("{label} address"))?,
            owner: exact_key(&value.owner, &format!("{label} owner"))?,
            lamports: exact_u64(&value.lamports, &format!("{label} lamports"), true)?,
            executable: value.executable,
            data: exact_base64(&value.data_base64, &format!("{label} data"))?,
            observation,
        })
    };
    let receipt = account(wire.receipt, "closure receipt")?;
    let rent = account(wire.rent_sysvar, "Rent sysvar")?;
    let expected = wire.expected;
    let facts = ResolutionRetirementReceiptFactsV3 {
        market: exact_key(&expected.market, "expected Market")?.to_bytes(),
        generation: exact_u64(&expected.generation, "expected generation", true)?,
        resolution_closure_receipt: exact_key(
            &expected.closure_receipt,
            "expected closure receipt",
        )?
        .to_bytes(),
        source_state: exact_key(&expected.source_state, "expected Source state")?.to_bytes(),
        source_material: exact_hex_32(&expected.source_material, "expected Source material")?,
        capability_manifest: exact_hex_32(
            &expected.capability_manifest,
            "expected capability manifest",
        )?,
        terminal_certificate: exact_key(
            &expected.terminal_certificate,
            "expected terminal certificate",
        )?
        .to_bytes(),
        beneficiary: exact_key(&expected.beneficiary, "expected beneficiary")?.to_bytes(),
        selector: exact_u32(&expected.selector, "expected selector")?,
        terminal_sequence: exact_u64(
            &expected.terminal_sequence,
            "expected terminal sequence",
            true,
        )?,
        source_state_digest: exact_hex_32(&expected.source_state_digest, "expected Source digest")?,
        terminal_certificate_digest: exact_hex_32(
            &expected.terminal_certificate_digest,
            "expected certificate digest",
        )?,
        funding_set_digest: exact_hex_32(&expected.funding_set_digest, "expected funding digest")?,
        source_refund_lamports: exact_u64(
            &expected.source_refund_lamports,
            "expected Source refund",
            true,
        )?,
        ledger_remaining_native_principal: exact_u64(
            &expected.ledger_remaining_native_principal,
            "expected ledger principal",
            true,
        )?,
        ledger_rent_lamports: exact_u64(
            &expected.ledger_rent_lamports,
            "expected ledger rent",
            true,
        )?,
        ledger_lamport_surplus: exact_u64(
            &expected.ledger_lamport_surplus,
            "expected ledger surplus",
            true,
        )?,
        refund_lamports: exact_u64(&expected.refund_lamports, "expected refund", true)?,
        closed_at: exact_u64(&expected.closed_at, "expected close time", false)?,
    };
    authenticate_resolution_retirement_receipt_v3(
        &receipt,
        &rent,
        exact_key(&wire.resolution_program, "Resolution program")?,
        facts,
    )
    .map_err(|error| format!("Source closure receipt refused: {error:?}"))?;
    serde_json::to_string(&serde_json::json!({
        "format": CLOSE_VERIFY_FORMAT_V1,
        "complete": true,
        "observedSlot": observation.slot.to_string(),
        "receipt": receipt.key.to_string(),
    }))
    .map_err(|error| format!("Source close verification encoding: {error}"))
}

fn bounded_exact_json(source: &[u8]) -> Result<Value, String> {
    if source.is_empty() || source.len() > MAX_JSON_BYTES {
        return Err("Source readiness input is outside its bounded JSON size".to_owned());
    }
    parse_exact_json_v1(source)
}

fn decode_frame(wire: FrameWireV1) -> Result<FundingReadinessFrameV1, String> {
    let record = |value: RecordCoordinatesWireV1,
                  label: &str|
     -> Result<FundingReadinessRecordCoordinatesV1, String> {
        Ok(FundingReadinessRecordCoordinatesV1 {
            raw: exact_key(&value.raw, &format!("{label} raw"))?,
            staging: exact_key(&value.staging, &format!("{label} staging"))?,
        })
    };
    Ok(FundingReadinessFrameV1 {
        coordinates: FundingReadinessCoordinatesV1 {
            market: exact_key(&wire.coordinates.market, "Market")?,
            source_material: record(wire.coordinates.source_material, "Source material")?,
            capability_manifest: record(
                wire.coordinates.capability_manifest,
                "capability manifest",
            )?,
            recovery_policy: wire
                .coordinates
                .recovery_policy
                .map(|value| record(value, "recovery policy"))
                .transpose()?,
            source_state: exact_key(&wire.coordinates.source_state, "Source state")?,
            funding_ledger: exact_key(&wire.coordinates.funding_ledger, "funding ledger")?,
            beneficiary: exact_key(&wire.coordinates.beneficiary, "beneficiary")?,
            activation_receipt: exact_key(
                &wire.coordinates.activation_receipt,
                "activation receipt",
            )?,
        },
        activation_cache: exact_key(&wire.activation_cache, "activation cache")?,
        registry_program: exact_key(&wire.registry_program, "Registry program")?,
        core_program: exact_key(&wire.core_program, "Core program")?,
        core_programdata: exact_key(&wire.core_programdata, "Core ProgramData")?,
        resolution_program: exact_key(&wire.resolution_program, "Resolution program")?,
        resolution_programdata: exact_key(&wire.resolution_programdata, "Resolution ProgramData")?,
    })
}

fn encode_plan(plan: FundingReadinessPlanV1, observed_slot: u64) -> PlanOutputV1 {
    match plan {
        FundingReadinessPlanV1::Create(value) => {
            let mut facts = BTreeMap::new();
            facts.insert("callerAuthority", value.report.caller_authority.to_string());
            facts.insert("beneficiary", value.report.beneficiary.to_string());
            facts.insert(
                "fundingEntryIndices",
                indices(value.report.funding_entry_indices),
            );
            facts.insert(
                "sourceTopUpLamports",
                value.report.source_top_up_lamports.to_string(),
            );
            facts.insert("roleRequestDigest", hex(value.report.role_request_digest));
            executable_output("create", observed_slot, &value, facts)
        }
        FundingReadinessPlanV1::Activate(value) => {
            let mut facts = BTreeMap::new();
            facts.insert(
                "activationReceipt",
                value.report.activation_receipt.to_string(),
            );
            facts.insert(
                "receiptTopUpLamports",
                value.report.receipt_top_up_lamports.to_string(),
            );
            facts.insert(
                "expectedBeneficiaryCreditLamports",
                value
                    .report
                    .expected_beneficiary_credit_lamports
                    .to_string(),
            );
            facts.insert("requestDigest", hex(value.report.request_digest));
            facts.insert(
                "fundingEntryIndices",
                indices(value.report.funding_entry_indices),
            );
            executable_output("activate", observed_slot, &value, facts)
        }
        FundingReadinessPlanV1::Accept(value) => {
            let mut facts = verify_facts(&value.report);
            facts.insert("terminal", "false".to_owned());
            executable_output("accept", observed_slot, &value, facts)
        }
        FundingReadinessPlanV1::Complete(value) => PlanOutputV1 {
            format: PLAN_FORMAT_V1,
            route: "complete",
            observed_slot: observed_slot.to_string(),
            instruction: None,
            prepay: None,
            accounts: Some(account_sets(&value)),
            geometry: Some(geometry(value.geometry)),
            facts: verify_facts(&value.report),
        },
        FundingReadinessPlanV1::ConsumedByFounding => PlanOutputV1 {
            format: PLAN_FORMAT_V1,
            route: "consumed-by-founding",
            observed_slot: observed_slot.to_string(),
            instruction: None,
            prepay: None,
            accounts: None,
            geometry: None,
            facts: BTreeMap::new(),
        },
    }
}

fn encode_terminal_plan(plan: SourceTerminalPlanV1, observed_slot: u64) -> PlanOutputV1 {
    let value = match plan {
        SourceTerminalPlanV1::Admit(value) => {
            let mut facts = terminal_facts(&value.report);
            facts.insert("terminal", "false".to_owned());
            PlanOutputV1 {
                format: TERMINAL_PLAN_FORMAT_V1,
                route: "admit",
                observed_slot: observed_slot.to_string(),
                instruction: Some(instruction(&value.report.instruction)),
                prepay: None,
                accounts: Some(account_sets(&value)),
                geometry: Some(geometry(value.geometry)),
                facts,
            }
        }
        SourceTerminalPlanV1::Complete(value) => {
            let mut facts = terminal_facts(&value.report);
            facts.insert("terminal", "true".to_owned());
            PlanOutputV1 {
                format: TERMINAL_PLAN_FORMAT_V1,
                route: "complete",
                observed_slot: observed_slot.to_string(),
                instruction: None,
                prepay: None,
                accounts: Some(account_sets(&value)),
                geometry: Some(geometry(value.geometry)),
                facts,
            }
        }
    };
    value
}

fn encode_close_plan(plan: SourceCloseFundPlanV1, observed_slot: u64) -> PlanOutputV1 {
    match plan {
        SourceCloseFundPlanV1::Prepay {
            observation,
            receipt,
            current_lamports,
            exact_rent_lamports,
            top_up_lamports,
        } => {
            let mut facts = BTreeMap::new();
            facts.insert("currentLamports", current_lamports.to_string());
            facts.insert("exactRentLamports", exact_rent_lamports.to_string());
            facts.insert("receipt", receipt.to_string());
            PlanOutputV1 {
                format: CLOSE_PLAN_FORMAT_V1,
                route: "prepay",
                observed_slot: observation.slot.to_string(),
                instruction: None,
                prepay: Some(PrepayOutputV1 {
                    destination: receipt.to_string(),
                    lamports: top_up_lamports.to_string(),
                }),
                accounts: None,
                geometry: None,
                facts,
            }
        }
        SourceCloseFundPlanV1::Close(value) => {
            let facts = value.report.expected_retirement_facts;
            let mut output = BTreeMap::new();
            output.insert("closureReceipt", value.report.closure_receipt.to_string());
            output.insert("requestDigest", hex(value.report.request_digest));
            output.insert("market", Pubkey::new_from_array(facts.market).to_string());
            output.insert("generation", facts.generation.to_string());
            output.insert(
                "sourceState",
                Pubkey::new_from_array(facts.source_state).to_string(),
            );
            output.insert("sourceMaterial", hex(facts.source_material));
            output.insert("capabilityManifest", hex(facts.capability_manifest));
            output.insert(
                "terminalCertificate",
                Pubkey::new_from_array(facts.terminal_certificate).to_string(),
            );
            output.insert(
                "beneficiary",
                Pubkey::new_from_array(facts.beneficiary).to_string(),
            );
            output.insert("sourceStateDigest", hex(facts.source_state_digest));
            output.insert(
                "terminalCertificateDigest",
                hex(facts.terminal_certificate_digest),
            );
            output.insert("fundingSetDigest", hex(facts.funding_set_digest));
            output.insert("terminalSequence", facts.terminal_sequence.to_string());
            output.insert("selector", facts.selector.to_string());
            output.insert("refundLamports", facts.refund_lamports.to_string());
            output.insert(
                "sourceRefundLamports",
                facts.source_refund_lamports.to_string(),
            );
            output.insert(
                "ledgerRemainingNativePrincipal",
                facts.ledger_remaining_native_principal.to_string(),
            );
            output.insert("ledgerRentLamports", facts.ledger_rent_lamports.to_string());
            output.insert(
                "ledgerLamportSurplus",
                facts.ledger_lamport_surplus.to_string(),
            );
            output.insert("closedAt", facts.closed_at.to_string());
            PlanOutputV1 {
                format: CLOSE_PLAN_FORMAT_V1,
                route: "close",
                observed_slot: observed_slot.to_string(),
                instruction: Some(instruction(&value.report.instruction)),
                prepay: None,
                accounts: Some(account_sets(&value)),
                geometry: Some(geometry(value.geometry)),
                facts: output,
            }
        }
    }
}

fn terminal_facts(
    report: &dclutch_resolution_core_v3_operator::ResolutionAdmitTerminalReportV3,
) -> BTreeMap<&'static str, String> {
    let mut facts = BTreeMap::new();
    facts.insert("callerAuthority", report.caller_authority.to_string());
    facts.insert("terminalSequence", report.terminal_sequence.to_string());
    facts.insert("selector", report.selector.to_string());
    facts.insert("outcomeCount", report.outcome_count.to_string());
    facts.insert("roleRequestDigest", hex(report.role_request_digest));
    facts
}

fn executable_output<T>(
    route: &'static str,
    observed_slot: u64,
    value: &FundingReadinessInstructionPlanV1<T>,
    facts: BTreeMap<&'static str, String>,
) -> PlanOutputV1
where
    T: InstructionReportV1,
{
    PlanOutputV1 {
        format: PLAN_FORMAT_V1,
        route,
        observed_slot: observed_slot.to_string(),
        instruction: Some(instruction(value.report.instruction())),
        prepay: value.prepay.map(|prepay| PrepayOutputV1 {
            destination: prepay.destination.to_string(),
            lamports: prepay.lamports.to_string(),
        }),
        accounts: Some(account_sets(value)),
        geometry: Some(geometry(value.geometry)),
        facts,
    }
}

trait InstructionReportV1 {
    fn instruction(&self) -> &Instruction;
}
impl InstructionReportV1 for dclutch_resolution_core_v3_operator::ResolutionCreateFundReportV3 {
    fn instruction(&self) -> &Instruction {
        &self.instruction
    }
}
impl InstructionReportV1 for dclutch_resolution_core_v3_operator::ResolutionActivateFundReportV1 {
    fn instruction(&self) -> &Instruction {
        &self.instruction
    }
}
impl InstructionReportV1
    for dclutch_resolution_core_v3_operator::ResolutionVerifyFundReadyReportV3
{
    fn instruction(&self) -> &Instruction {
        &self.instruction
    }
}

fn verify_facts(
    report: &dclutch_resolution_core_v3_operator::ResolutionVerifyFundReadyReportV3,
) -> BTreeMap<&'static str, String> {
    let mut facts = BTreeMap::new();
    facts.insert("callerAuthority", report.caller_authority.to_string());
    facts.insert("beneficiary", report.beneficiary.to_string());
    facts.insert("fundingEntryIndices", indices(report.funding_entry_indices));
    facts.insert("activationSlot", report.activation_slot.to_string());
    facts.insert(
        "expectedBeneficiaryCreditLamports",
        report.expected_beneficiary_credit_lamports.to_string(),
    );
    facts.insert("roleRequestDigest", hex(report.role_request_digest));
    facts
}

fn instruction(value: &Instruction) -> InstructionOutputV1 {
    InstructionOutputV1 {
        program: value.program_id.to_string(),
        accounts: value
            .accounts
            .iter()
            .map(|meta| MetaOutputV1 {
                address: meta.pubkey.to_string(),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data_base64: STANDARD.encode(&value.data),
    }
}

fn account_sets<T>(value: &FundingReadinessInstructionPlanV1<T>) -> AccountSetsOutputV1 {
    AccountSetsOutputV1 {
        protocol_writable: value
            .accounts
            .protocol_writable
            .iter()
            .map(ToString::to_string)
            .collect(),
        completion: value
            .accounts
            .completion
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}

const fn geometry(value: FundingReadinessGeometryV1) -> GeometryOutputV1 {
    GeometryOutputV1 {
        protocol_account_count: value.protocol_account_count,
        protocol_unique_account_count: value.protocol_unique_account_count,
        protocol_writable_count: value.protocol_writable_count,
        protocol_signer_count: value.protocol_signer_count,
        protocol_data_len: value.protocol_data_len,
        transaction_instruction_count_without_compute_budget: value
            .transaction_instruction_count_without_compute_budget,
        transaction_lock_count_without_payer: value.transaction_lock_count_without_payer,
    }
}

fn exact_key(value: &str, field: &str) -> Result<Pubkey, String> {
    let key = Pubkey::from_str(value)
        .map_err(|_| format!("{field} is not one canonical Solana address"))?;
    if key.to_string() != value {
        return Err(format!("{field} is not canonical base58 text"));
    }
    Ok(key)
}

fn exact_u64(value: &str, field: &str, zero: bool) -> Result<u64, String> {
    if ((!zero && !matches!(value.as_bytes().first(), Some(b'1'..=b'9')))
        || (zero && value != "0" && !matches!(value.as_bytes().first(), Some(b'1'..=b'9'))))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{field} is not canonical unsigned decimal"));
    }
    value.parse().map_err(|_| format!("{field} exceeds u64"))
}

fn exact_u32(value: &str, field: &str) -> Result<u32, String> {
    let parsed = exact_u64(value, field, true)?;
    u32::try_from(parsed).map_err(|_| format!("{field} exceeds u32"))
}

fn exact_hex_32(value: &str, field: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} is not canonical lowercase hex"));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{field} is not canonical lowercase hex"))?;
    }
    Ok(output)
}

fn exact_i64(value: &str, field: &str) -> Result<i64, String> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || (digits != "0" && !matches!(digits.as_bytes().first(), Some(b'1'..=b'9')))
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || value == "-0"
    {
        return Err(format!("{field} is not canonical signed decimal"));
    }
    value.parse().map_err(|_| format!("{field} exceeds i64"))
}

fn exact_base64(value: &str, field: &str) -> Result<Vec<u8>, String> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| format!("{field} is not canonical base64"))?;
    if bytes.len() > MAX_ACCOUNT_BYTES || STANDARD.encode(&bytes) != value {
        return Err(format!("{field} is not bounded canonical base64"));
    }
    Ok(bytes)
}

fn indices(value: [u16; 3]) -> String {
    format!("{},{},{}", value[0], value[1], value[2])
}
fn hex(value: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn parse_exact_json_v1(bytes: &[u8]) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| format!("Source readiness JSON: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("Source readiness JSON trailing bytes: {error}"))?;
    Ok(value)
}

struct ExactJsonValueSeedV1;
impl<'de> DeserializeSeed<'de> for ExactJsonValueSeedV1 {
    type Value = Value;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ExactJsonValueVisitorV1)
    }
}
struct ExactJsonValueVisitorV1;
impl<'de> Visitor<'de> for ExactJsonValueVisitorV1 {
    type Value = Value;
    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("one JSON value without duplicate object keys")
    }
    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E>(self, value: f64) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON number was not finite"))
    }
    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExactJsonValueSeedV1.deserialize(deserializer)
    }
    fn visit_seq<A>(self, mut sequence: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(ExactJsonValueSeedV1)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
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
            values.insert(key, map.next_value_seed(ExactJsonValueSeedV1)?);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_refuses_duplicate_unknown_and_noncanonical_scalars() {
        assert!(
            plan_funding_readiness_json_v1(br#"{"format":"x","format":"y"}"#)
                .expect_err("duplicate")
                .contains("duplicate JSON object key")
        );
        assert!(plan_funding_readiness_json_v1(br#"{"format":"dclutch-source-readiness-snapshot-v1","observedSlot":"01","unixTimestamp":"0","frame":{},"accounts":[],"extra":1}"#).is_err());
        assert!(exact_u64("01", "slot", false).is_err());
        assert!(exact_i64("-0", "timestamp").is_err());
        assert!(exact_base64("AA", "data").is_err());
        assert!(
            derive_source_terminal_base_json_v1(br#"{"format":"x","format":"y"}"#)
                .expect_err("duplicate terminal field")
                .contains("duplicate JSON object key")
        );
        assert!(derive_source_terminal_product_json_v1(br#"{"format":"dclutch-source-terminal-product-v1","marketAddress":"x","marketDataBase64":"","registryProgram":"x","productDataBase64":"","extra":1}"#).is_err());
        assert!(plan_source_terminal_json_v1(br#"{"format":"dclutch-source-terminal-snapshot-v1","observedSlot":"01","unixTimestamp":"0","frame":{},"accounts":[]}"#).is_err());
        assert!(
            derive_source_close_detail_json_v1(br#"{"format":"x","format":"y"}"#)
                .expect_err("duplicate close field")
                .contains("duplicate JSON object key")
        );
        assert!(derive_source_close_detail_json_v1(br#"{"format":"dclutch-source-close-detail-v1","marketAddress":"x","marketDataBase64":"","resolutionProgram":"x","sourceStateAddress":"x","sourceStateDataBase64":"","extra":1}"#).is_err());
        assert!(plan_source_close_fund_json_v1(br#"{"format":"dclutch-source-close-snapshot-v1","observedSlot":"01","unixTimestamp":"0","frame":{},"accounts":[]}"#).is_err());
    }
}
