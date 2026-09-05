//! Strict JSON transport over the authoritative provider-transport owner.

use std::collections::BTreeSet;
use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_provider_transport_v3_operator::{
    Finality, Observation, ObservedAccount, ProviderReclaimDeploymentV3,
    ProviderSubmitDeploymentV3, ProviderSubmitIntentV3, ProviderSubmitSnapshotV3,
    ProviderTransportReportV3, build_provider_reclaim_v3, build_provider_submit_v3,
    compile_provider_reclaim_v0, compile_provider_submit_with_lifecycle_prepay_v0,
    derive_provider_submit_base_coordinates_v3, derive_provider_submit_fresh_coordinates_v3,
    derive_provider_submit_material_coordinates_v3,
    derive_provider_submit_provider_release_coordinates_v3,
    derive_provider_submit_pyth_coordinates_v3, derive_provider_submit_pyth_release_coordinates_v3,
};
use dclutch_registry::record::RAW_RECORD_PDA_SEED_V1;
use dclutch_source::pyth::{ProgramV3View, PythReleaseV1};
use dclutch_source::resolution::{
    PROVIDER_SUBMIT_REQUEST_BYTES_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderSubmitRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, ResolutionCertificateV2,
};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_hash::Hash;
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk_ids::bpf_loader_upgradeable;

pub(crate) const RECLAIM_INPUT_FORMAT_V1: &str = "dclutch-source-provider-reclaim-input-v1";
pub(crate) const PLAN_FORMAT_V1: &str = "dclutch-source-provider-plan-v1";
pub(crate) const COORDINATES_INPUT_FORMAT_V1: &str =
    "dclutch-source-provider-reclaim-coordinates-input-v1";
pub(crate) const COORDINATES_FORMAT_V1: &str = "dclutch-source-provider-reclaim-coordinates-v1";
pub(crate) const PROGRAM_INPUT_FORMAT_V1: &str = "dclutch-source-provider-program-input-v1";
pub(crate) const PRICE_INPUT_FORMAT_V1: &str = "dclutch-source-provider-price-input-v1";
pub(crate) const PRICE_FORMAT_V1: &str = "dclutch-source-provider-price-v1";
pub(crate) const PROGRAM_FORMAT_V1: &str = "dclutch-source-provider-program-v1";
pub(crate) const SUBMIT_BASE_INPUT_FORMAT_V1: &str = "dclutch-source-provider-submit-base-input-v1";
pub(crate) const SUBMIT_BASE_FORMAT_V1: &str = "dclutch-source-provider-submit-base-v1";
pub(crate) const SUBMIT_MATERIAL_INPUT_FORMAT_V1: &str =
    "dclutch-source-provider-submit-material-input-v1";
pub(crate) const SUBMIT_MATERIAL_FORMAT_V1: &str = "dclutch-source-provider-submit-material-v1";
pub(crate) const SUBMIT_RECORD_INPUT_FORMAT_V1: &str =
    "dclutch-source-provider-submit-record-input-v1";
pub(crate) const SUBMIT_RECORD_FORMAT_V1: &str = "dclutch-source-provider-submit-record-v1";
pub(crate) const SUBMIT_PYTH_INPUT_FORMAT_V1: &str = "dclutch-source-provider-submit-pyth-input-v1";
pub(crate) const SUBMIT_PYTH_FORMAT_V1: &str = "dclutch-source-provider-submit-pyth-v1";
pub(crate) const SUBMIT_FRESH_INPUT_FORMAT_V1: &str =
    "dclutch-source-provider-submit-fresh-input-v1";
pub(crate) const SUBMIT_FRESH_FORMAT_V1: &str = "dclutch-source-provider-submit-fresh-v1";
pub(crate) const SUBMIT_INPUT_FORMAT_V1: &str = "dclutch-source-provider-submit-input-v1";
pub(crate) const SUBMIT_PLAN_FORMAT_V1: &str = "dclutch-source-provider-submit-plan-v1";
pub(crate) const SUBMIT_POSTSTATE_INPUT_FORMAT_V1: &str =
    "dclutch-source-provider-submit-poststate-input-v1";
pub(crate) const SUBMIT_POSTSTATE_FORMAT_V1: &str = "dclutch-source-provider-submit-poststate-v1";
#[allow(dead_code)]
pub(crate) const SUBMIT_LIFECYCLE_BYTES_V1: usize =
    dclutch_source::resolution::PROVIDER_UPDATE_LIFECYCLE_BYTES_V3;
const MAX_JSON_BYTES: usize = 24 * 1024 * 1024;
const MAX_ACCOUNT_BYTES: usize = 8 * 1024 * 1024;

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
struct ReclaimDeploymentWireV1 {
    payer: String,
    resolver: String,
    registry_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReclaimInputWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    recent_blockhash: String,
    lifecycle: AccountWireV1,
    pyth_release: AccountWireV1,
    update: AccountWireV1,
    update_authority: AccountWireV1,
    refund_recipient: AccountWireV1,
    certificate: AccountWireV1,
    deployment: ReclaimDeploymentWireV1,
    lookup_table: Option<AccountWireV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoordinatesInputWireV1 {
    format: String,
    lifecycle: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProgramInputWireV1 {
    format: String,
    program: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitBaseInputWireV1 {
    format: String,
    market: AccountWireV1,
    core_program: String,
    registry_program: String,
    resolution_program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitMaterialInputWireV1 {
    format: String,
    market: AccountWireV1,
    source_material: AccountWireV1,
    infrastructure: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitRecordInputWireV1 {
    format: String,
    registry_program: String,
    record: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitPythInputWireV1 {
    format: String,
    registry_program: String,
    pyth_release: AccountWireV1,
    encoded_vaa: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitFreshInputWireV1 {
    format: String,
    market: String,
    source_state: String,
    update_account: String,
    resolution_program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitDeploymentWireV1 {
    submitter: String,
    refund_recipient: String,
    update_account: String,
    infrastructure: String,
    registry_programdata: String,
    registry_artifact: String,
    registry_artifact_staging: String,
    core_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
    receiver_config: String,
    guardian_set: String,
    receiver_program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitInputWireV1 {
    format: String,
    observed_slot: String,
    unix_timestamp: String,
    recent_blockhash: String,
    reclaim_after_unix_seconds: String,
    post_update_body_base64: String,
    lifecycle_rent_minimum: String,
    market: AccountWireV1,
    source_state: AccountWireV1,
    source_material: AccountWireV1,
    source_spec: AccountWireV1,
    source_provider_release: AccountWireV1,
    pyth_release: AccountWireV1,
    window: AccountWireV1,
    encoded_vaa: AccountWireV1,
    update_prestate: AccountWireV1,
    lifecycle_prestate: AccountWireV1,
    deployment: SubmitDeploymentWireV1,
    lookup_table: AccountWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitPoststateExpectationWireV1 {
    lifecycle: String,
    update_account: String,
    update_authority: String,
    resolution_program: String,
    receiver_program: String,
    submit_request_base64: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitPoststateInputWireV1 {
    format: String,
    expectation: SubmitPoststateExpectationWireV1,
    lifecycle: Option<AccountWireV1>,
    update: Option<AccountWireV1>,
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
struct PlanOutputV1 {
    format: &'static str,
    route: &'static str,
    observed_slot: String,
    instruction: InstructionOutputV1,
    unsigned_message_base64: String,
    required_signers: Vec<String>,
    wire_bytes: usize,
    loaded_addresses: usize,
    lookup_tables: Vec<String>,
    lifecycle: String,
    update_authority: String,
    completion: Vec<String>,
    expected_poststates: Vec<AccountOutputV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountOutputV1 {
    address: String,
    owner: String,
    lamports: String,
    executable: bool,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinatesOutputV1 {
    format: &'static str,
    lifecycle: String,
    market: String,
    source_state: String,
    resolution_program: String,
    registry_program: String,
    pyth_release: String,
    update_account: String,
    update_authority: String,
    refund_recipient: String,
    certificate: String,
    release_set: String,
    generation: String,
    terminal_sequence: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramOutputV1 {
    format: &'static str,
    program: String,
    programdata: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitBaseOutputV1 {
    format: &'static str,
    source_state: String,
    source_material: String,
    refund_recipient: String,
    infrastructure: String,
    core_programdata: String,
    resolution_programdata: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitMaterialOutputV1 {
    format: &'static str,
    source_spec: String,
    source_spec_staging: String,
    window: String,
    window_staging: String,
    registry_artifact: String,
    registry_artifact_staging: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitRecordOutputV1 {
    format: &'static str,
    raw: String,
    staging: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitPythOutputV1 {
    format: &'static str,
    receiver_program: String,
    receiver_programdata: String,
    receiver_config: String,
    router_program: String,
    router_programdata: String,
    guardian_set: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitFreshOutputV1 {
    format: &'static str,
    lifecycle: String,
    update_authority: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitPoststateExpectationOutputV1 {
    lifecycle: String,
    update_account: String,
    update_authority: String,
    resolution_program: String,
    receiver_program: String,
    submit_request_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitPlanOutputV1 {
    format: &'static str,
    route: &'static str,
    observed_slot: String,
    instruction: InstructionOutputV1,
    unsigned_message_base64: String,
    required_signers: Vec<String>,
    wire_bytes: usize,
    loaded_addresses: usize,
    lookup_tables: Vec<String>,
    lifecycle_top_up_lamports: String,
    completion: Vec<String>,
    poststate: SubmitPoststateExpectationOutputV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitPoststateOutputV1 {
    format: &'static str,
    complete: bool,
}

/// Derive all reclaim coordinates available from one exact consumed lifecycle.
pub fn derive_provider_reclaim_coordinates_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: CoordinatesInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider reclaim coordinates schema: {error}"))?;
    if wire.format != COORDINATES_INPUT_FORMAT_V1 {
        return Err("Source provider reclaim coordinates input has another format".to_owned());
    }
    let address = exact_key(&wire.lifecycle.address, "provider lifecycle")?;
    let resolution_program = exact_key(&wire.lifecycle.owner, "Resolution program")?;
    if wire.lifecycle.executable {
        return Err("Source provider lifecycle is executable".to_owned());
    }
    let bytes = exact_base64(&wire.lifecycle.data_base64, "provider lifecycle data")?;
    let lifecycle = ProviderUpdateLifecycleV3::decode(&bytes)
        .map_err(|error| format!("Source provider lifecycle: {error:?}"))?;
    if lifecycle.status != ProviderUpdateStatusV3::Consumed {
        return Err("Source provider lifecycle is not consumed and reclaimable".to_owned());
    }
    let expected = Pubkey::find_program_address(
        &[
            dclutch_source::resolution::PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            &lifecycle.update_account,
        ],
        &resolution_program,
    )
    .0;
    if address != expected {
        return Err("Source provider lifecycle PDA changed".to_owned());
    }
    let registry_program = Pubkey::new_from_array(lifecycle.registry_program);
    let pyth_release = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
            &lifecycle.provider_release,
        ],
        &registry_program,
    )
    .0;
    serde_json::to_string(&CoordinatesOutputV1 {
        format: COORDINATES_FORMAT_V1,
        lifecycle: address.to_string(),
        market: Pubkey::new_from_array(lifecycle.market).to_string(),
        source_state: Pubkey::new_from_array(lifecycle.source_state).to_string(),
        resolution_program: resolution_program.to_string(),
        registry_program: registry_program.to_string(),
        pyth_release: pyth_release.to_string(),
        update_account: Pubkey::new_from_array(lifecycle.update_account).to_string(),
        update_authority: Pubkey::new_from_array(lifecycle.update_authority).to_string(),
        refund_recipient: Pubkey::new_from_array(lifecycle.refund_recipient).to_string(),
        certificate: Pubkey::new_from_array(lifecycle.certificate).to_string(),
        release_set: Pubkey::new_from_array(lifecycle.release_set).to_string(),
        generation: lifecycle.generation.to_string(),
        terminal_sequence: lifecycle.terminal_sequence.to_string(),
    })
    .map_err(|error| format!("Source provider reclaim coordinates output: {error}"))
}

/// Decode one exact Upgradeable Loader Program-to-ProgramData link.
pub fn derive_provider_programdata_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: ProgramInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider Program schema: {error}"))?;
    if wire.format != PROGRAM_INPUT_FORMAT_V1 {
        return Err("Source provider Program input has another format".to_owned());
    }
    let program = exact_key(&wire.program.address, "program")?;
    if exact_key(&wire.program.owner, "program owner")? != bpf_loader_upgradeable::ID
        || !wire.program.executable
    {
        return Err(
            "Source provider Program is not executable Upgradeable Loader state".to_owned(),
        );
    }
    let data = exact_base64(&wire.program.data_base64, "program data")?;
    let programdata = ProgramV3View::parse(&data)
        .map_err(|error| format!("Source provider Program: {error:?}"))?
        .programdata();
    serde_json::to_string(&ProgramOutputV1 {
        format: PROGRAM_FORMAT_V1,
        program: program.to_string(),
        programdata: Pubkey::new_from_array(programdata).to_string(),
    })
    .map_err(|error| format!("Source provider Program output: {error}"))
}

/// Derive the first provider-submit coordinates from one exact Market.
pub fn derive_provider_submit_base_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitBaseInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit base schema: {error}"))?;
    if wire.format != SUBMIT_BASE_INPUT_FORMAT_V1 {
        return Err("Source provider submit base has another format".to_owned());
    }
    let market = observed(wire.market, discovery_observation(), "Market")?;
    let result = derive_provider_submit_base_coordinates_v3(
        &market,
        exact_key(&wire.core_program, "Core program")?,
        exact_key(&wire.registry_program, "Registry program")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
    )
    .map_err(|error| format!("Source provider submit base owner: {error:?}"))?;
    serde_json::to_string(&SubmitBaseOutputV1 {
        format: SUBMIT_BASE_FORMAT_V1,
        source_state: result.source_state.to_string(),
        source_material: result.source_material.to_string(),
        refund_recipient: result.refund_recipient.to_string(),
        infrastructure: result.infrastructure.to_string(),
        core_programdata: result.core_programdata.to_string(),
        resolution_programdata: result.resolution_programdata.to_string(),
    })
    .map_err(|error| format!("Source provider submit base output: {error}"))
}

/// Continue submit discovery through SourceMaterial and infrastructure.
pub fn derive_provider_submit_material_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitMaterialInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit material schema: {error}"))?;
    if wire.format != SUBMIT_MATERIAL_INPUT_FORMAT_V1 {
        return Err("Source provider submit material has another format".to_owned());
    }
    let observation = discovery_observation();
    let result = derive_provider_submit_material_coordinates_v3(
        &observed(wire.market, observation, "Market")?,
        &observed(wire.source_material, observation, "SourceMaterial")?,
        &observed(wire.infrastructure, observation, "infrastructure")?,
    )
    .map_err(|error| format!("Source provider submit material owner: {error:?}"))?;
    serde_json::to_string(&SubmitMaterialOutputV1 {
        format: SUBMIT_MATERIAL_FORMAT_V1,
        source_spec: result.source_spec.to_string(),
        source_spec_staging: result.source_spec_staging.to_string(),
        window: result.window.to_string(),
        window_staging: result.window_staging.to_string(),
        registry_artifact: result.registry_artifact.to_string(),
        registry_artifact_staging: result.registry_artifact_staging.to_string(),
    })
    .map_err(|error| format!("Source provider submit material output: {error}"))
}

/// Derive the ProviderRelease pair from one SourceSpec.
pub fn derive_provider_submit_provider_release_json_v1(source: &[u8]) -> Result<String, String> {
    derive_submit_record(source, true)
}

/// Derive the Pyth release pair from one ProviderRelease.
pub fn derive_provider_submit_pyth_release_json_v1(source: &[u8]) -> Result<String, String> {
    derive_submit_record(source, false)
}

fn derive_submit_record(source: &[u8], source_spec: bool) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitRecordInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit record schema: {error}"))?;
    if wire.format != SUBMIT_RECORD_INPUT_FORMAT_V1 {
        return Err("Source provider submit record has another format".to_owned());
    }
    let registry = exact_key(&wire.registry_program, "Registry program")?;
    let record = observed(
        wire.record,
        discovery_observation(),
        "provider graph record",
    )?;
    let result = if source_spec {
        derive_provider_submit_provider_release_coordinates_v3(registry, &record)
    } else {
        derive_provider_submit_pyth_release_coordinates_v3(registry, &record)
    }
    .map_err(|error| format!("Source provider submit record owner: {error:?}"))?;
    serde_json::to_string(&SubmitRecordOutputV1 {
        format: SUBMIT_RECORD_FORMAT_V1,
        raw: result.raw.to_string(),
        staging: result.staging.to_string(),
    })
    .map_err(|error| format!("Source provider submit record output: {error}"))
}

/// Derive the Receiver and Router frame from Pyth release plus verified VAA.
pub fn derive_provider_submit_pyth_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitPythInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit Pyth schema: {error}"))?;
    if wire.format != SUBMIT_PYTH_INPUT_FORMAT_V1 {
        return Err("Source provider submit Pyth has another format".to_owned());
    }
    let observation = discovery_observation();
    let result = derive_provider_submit_pyth_coordinates_v3(
        exact_key(&wire.registry_program, "Registry program")?,
        &observed(wire.pyth_release, observation, "Pyth release")?,
        &observed(wire.encoded_vaa, observation, "verified EncodedVaa")?,
    )
    .map_err(|error| format!("Source provider submit Pyth owner: {error:?}"))?;
    serde_json::to_string(&SubmitPythOutputV1 {
        format: SUBMIT_PYTH_FORMAT_V1,
        receiver_program: result.receiver_program.to_string(),
        receiver_programdata: result.receiver_programdata.to_string(),
        receiver_config: result.receiver_config.to_string(),
        router_program: result.router_program.to_string(),
        router_programdata: result.router_programdata.to_string(),
        guardian_set: result.guardian_set.to_string(),
    })
    .map_err(|error| format!("Source provider submit Pyth output: {error}"))
}

/// Derive the lifecycle and Receiver authority for one fresh update signer.
pub fn derive_provider_submit_fresh_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitFreshInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit fresh schema: {error}"))?;
    if wire.format != SUBMIT_FRESH_INPUT_FORMAT_V1 {
        return Err("Source provider submit fresh input has another format".to_owned());
    }
    let result = derive_provider_submit_fresh_coordinates_v3(
        exact_key(&wire.market, "Market")?,
        exact_key(&wire.source_state, "Source state")?,
        exact_key(&wire.update_account, "Receiver update")?,
        exact_key(&wire.resolution_program, "Resolution program")?,
    );
    serde_json::to_string(&SubmitFreshOutputV1 {
        format: SUBMIT_FRESH_FORMAT_V1,
        lifecycle: result.lifecycle.to_string(),
        update_authority: result.update_authority.to_string(),
    })
    .map_err(|error| format!("Source provider submit fresh output: {error}"))
}

/// Decode, rebuild, and compile one exact provider submission.
pub fn plan_provider_submit_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit schema: {error}"))?;
    if wire.format != SUBMIT_INPUT_FORMAT_V1 {
        return Err("Source provider submit input has another format".to_owned());
    }
    let slot = exact_u64(&wire.observed_slot, "observed slot", false)?;
    let unix_timestamp = exact_i64(&wire.unix_timestamp, "Unix timestamp")?;
    if unix_timestamp <= 0 {
        return Err("Source provider submit Unix timestamp must be positive".to_owned());
    }
    let observation = Observation {
        slot,
        unix_timestamp,
        finality: Finality::Finalized,
    };
    let market = observed(wire.market, observation, "Market")?;
    let source_state = observed(wire.source_state, observation, "Source state")?;
    let source_material = observed(wire.source_material, observation, "SourceMaterial")?;
    let source_spec = observed(wire.source_spec, observation, "SourceSpec")?;
    let source_provider_release =
        observed(wire.source_provider_release, observation, "ProviderRelease")?;
    let pyth_release = observed(wire.pyth_release, observation, "Pyth release")?;
    let window = observed(wire.window, observation, "WindowSpec")?;
    let encoded_vaa = observed(wire.encoded_vaa, observation, "verified EncodedVaa")?;
    let update = observed(
        wire.update_prestate,
        observation,
        "Receiver update prestate",
    )?;
    let lifecycle = observed(
        wire.lifecycle_prestate,
        observation,
        "provider lifecycle prestate",
    )?;
    let submitter = exact_key(&wire.deployment.submitter, "submitter")?;
    let refund_recipient = exact_key(&wire.deployment.refund_recipient, "refund recipient")?;
    let update_account = exact_key(&wire.deployment.update_account, "Receiver update")?;
    let resolution_program = exact_key(&wire.deployment.resolution_program, "Resolution program")?;
    let receiver_program = exact_key(&wire.deployment.receiver_program, "Receiver program")?;
    let post_update_body = exact_base64(&wire.post_update_body_base64, "post-update body")?;
    let report = build_provider_submit_v3(
        &ProviderSubmitSnapshotV3 {
            market,
            source_state,
            source_material,
            source_spec,
            source_provider_release,
            pyth_release,
            window,
            encoded_vaa,
        },
        ProviderSubmitDeploymentV3 {
            infrastructure: exact_key(&wire.deployment.infrastructure, "infrastructure")?,
            registry_programdata: exact_key(
                &wire.deployment.registry_programdata,
                "Registry ProgramData",
            )?,
            registry_artifact: exact_key(&wire.deployment.registry_artifact, "Registry artifact")?,
            registry_artifact_staging: exact_key(
                &wire.deployment.registry_artifact_staging,
                "Registry artifact staging",
            )?,
            core_programdata: exact_key(&wire.deployment.core_programdata, "Core ProgramData")?,
            resolution_program,
            resolution_programdata: exact_key(
                &wire.deployment.resolution_programdata,
                "Resolution ProgramData",
            )?,
            receiver_config: exact_key(&wire.deployment.receiver_config, "Receiver Config")?,
            guardian_set: exact_key(&wire.deployment.guardian_set, "GuardianSet")?,
        },
        &ProviderSubmitIntentV3 {
            submitter,
            refund_recipient,
            update_account,
            reclaim_after_unix_seconds: exact_i64(
                &wire.reclaim_after_unix_seconds,
                "reclaim-after timestamp",
            )?,
            post_update_body,
        },
    )
    .map_err(|error| format!("Source provider submit owner: {error:?}"))?;
    if update.key != update_account || lifecycle.key != report.lifecycle {
        return Err(
            "Source provider submit prestates changed their derived coordinates".to_owned(),
        );
    }
    let lookup_table = observed(wire.lookup_table, observation, "lookup table")?;
    let recent_blockhash = Hash::from_str(&wire.recent_blockhash)
        .map_err(|_| "Source provider recent blockhash is not canonical base58".to_owned())?;
    let plan = compile_provider_submit_with_lifecycle_prepay_v0(
        &report,
        &update,
        &lifecycle,
        exact_u64(
            &wire.lifecycle_rent_minimum,
            "lifecycle rent minimum",
            false,
        )?,
        recent_blockhash,
        core::slice::from_ref(&lookup_table),
    )
    .map_err(|error| format!("Source provider submit transaction: {error:?}"))?;
    if plan.transaction.required_signers.as_slice() != [submitter, update_account] {
        return Err("Source provider submit signer boundary changed".to_owned());
    }
    let submit_request = report
        .instruction
        .data
        .get(..PROVIDER_SUBMIT_REQUEST_BYTES_V3)
        .ok_or_else(|| "Source provider submit request prefix disappeared".to_owned())?;
    serde_json::to_string(&SubmitPlanOutputV1 {
        format: SUBMIT_PLAN_FORMAT_V1,
        route: "submit",
        observed_slot: slot.to_string(),
        instruction: instruction(&report),
        unsigned_message_base64: STANDARD.encode(plan.transaction.message.message.serialize()),
        required_signers: plan
            .transaction
            .required_signers
            .iter()
            .map(ToString::to_string)
            .collect(),
        wire_bytes: plan.transaction.message.wire_bytes,
        loaded_addresses: plan.transaction.message.loaded_addresses,
        lookup_tables: plan
            .transaction
            .message
            .lookup_tables
            .iter()
            .map(ToString::to_string)
            .collect(),
        lifecycle_top_up_lamports: plan.lifecycle_top_up_lamports.to_string(),
        completion: vec![report.lifecycle.to_string(), update_account.to_string()],
        poststate: SubmitPoststateExpectationOutputV1 {
            lifecycle: report.lifecycle.to_string(),
            update_account: update_account.to_string(),
            update_authority: report.update_authority.to_string(),
            resolution_program: resolution_program.to_string(),
            receiver_program: receiver_program.to_string(),
            submit_request_base64: STANDARD.encode(submit_request),
        },
    })
    .map_err(|error| format!("Source provider submit output: {error}"))
}

/// Check the exact terminal submit poststate projected by the Rust request.
pub fn verify_provider_submit_poststate_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: SubmitPoststateInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider submit poststate schema: {error}"))?;
    if wire.format != SUBMIT_POSTSTATE_INPUT_FORMAT_V1 {
        return Err("Source provider submit poststate has another format".to_owned());
    }
    let Some(lifecycle_wire) = wire.lifecycle else {
        return submit_poststate_output(false);
    };
    let Some(update_wire) = wire.update else {
        return submit_poststate_output(false);
    };
    let observation = discovery_observation();
    let lifecycle = observed(lifecycle_wire, observation, "provider lifecycle poststate")?;
    let update = observed(update_wire, observation, "Receiver update poststate")?;
    let expected_lifecycle = exact_key(&wire.expectation.lifecycle, "expected lifecycle")?;
    let expected_update = exact_key(&wire.expectation.update_account, "expected update")?;
    let expected_authority = exact_key(
        &wire.expectation.update_authority,
        "expected update authority",
    )?;
    let resolution_program = exact_key(
        &wire.expectation.resolution_program,
        "expected Resolution program",
    )?;
    let receiver_program = exact_key(
        &wire.expectation.receiver_program,
        "expected Receiver program",
    )?;
    let request_bytes = exact_base64(
        &wire.expectation.submit_request_base64,
        "expected submit request",
    )?;
    let request = ProviderSubmitRequestV3::decode(&request_bytes)
        .map_err(|_| "Source provider expected submit request is malformed".to_owned())?;
    let state = ProviderUpdateLifecycleV3::decode(&lifecycle.data)
        .map_err(|_| "Source provider lifecycle poststate is malformed".to_owned())?;
    let complete = lifecycle.key == expected_lifecycle
        && lifecycle.owner == resolution_program
        && !lifecycle.executable
        && lifecycle.lamports > 0
        && update.key == expected_update
        && update.owner == receiver_program
        && !update.executable
        && update.lamports == state.update_rent_lamports
        && !update.data.is_empty()
        && state.status == ProviderUpdateStatusV3::Submitted
        && state.market == request.market
        && state.source_state == request.source_state
        && state.source_material == request.source_material
        && state.provider_release == request.provider_release
        && state.update_account == request.update_account
        && state.update_authority == expected_authority.to_bytes()
        && state.provider_submitter == request.provider_submitter
        && state.refund_recipient == request.refund_recipient
        && state.release_set == request.release_set
        && state.registry_program == request.registry_program
        && state.post_body_digest == request.post_body_digest
        && state.reclaim_after_unix_seconds == request.reclaim_after_unix_seconds
        && solana_program::hash::hash(&update.data).to_bytes() == state.update_digest;
    submit_poststate_output(complete)
}

fn submit_poststate_output(complete: bool) -> Result<String, String> {
    serde_json::to_string(&SubmitPoststateOutputV1 {
        format: SUBMIT_POSTSTATE_FORMAT_V1,
        complete,
    })
    .map_err(|error| format!("Source provider submit poststate output: {error}"))
}

/// Decode, rebuild, and compile one exact provider reclaim.
pub fn plan_provider_reclaim_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: ReclaimInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider reclaim schema: {error}"))?;
    if wire.format != RECLAIM_INPUT_FORMAT_V1 {
        return Err("Source provider reclaim input has another format".to_owned());
    }
    let slot = exact_u64(&wire.observed_slot, "observed slot", false)?;
    let unix_timestamp = exact_i64(&wire.unix_timestamp, "Unix timestamp")?;
    if unix_timestamp <= 0 {
        return Err("Source provider Unix timestamp must be positive".to_owned());
    }
    let observation = Observation {
        slot,
        unix_timestamp,
        finality: Finality::Finalized,
    };
    let lifecycle = observed(wire.lifecycle, observation, "provider lifecycle")?;
    let pyth_release = observed(wire.pyth_release, observation, "Pyth release")?;
    let update = observed(wire.update, observation, "Receiver update")?;
    let authority = observed(wire.update_authority, observation, "update authority")?;
    let refund = observed(wire.refund_recipient, observation, "refund recipient")?;
    let certificate = observed(wire.certificate, observation, "terminal certificate")?;
    let payer = exact_key(&wire.deployment.payer, "payer")?;
    let resolver = exact_key(&wire.deployment.resolver, "resolver")?;
    let resolution_program = exact_key(&wire.deployment.resolution_program, "Resolution program")?;
    let report = build_provider_reclaim_v3(
        &lifecycle,
        &pyth_release,
        ProviderReclaimDeploymentV3 {
            resolver,
            registry_programdata: exact_key(
                &wire.deployment.registry_programdata,
                "Registry ProgramData",
            )?,
            resolution_program,
            resolution_programdata: exact_key(
                &wire.deployment.resolution_programdata,
                "Resolution ProgramData",
            )?,
        },
    )
    .map_err(|error| format!("Source provider reclaim owner: {error:?}"))?;
    let lifecycle_state = ProviderUpdateLifecycleV3::decode(&lifecycle.data)
        .map_err(|error| format!("Source provider lifecycle: {error:?}"))?;
    let release = PythReleaseV1::decode(&pyth_release.data)
        .map_err(|error| format!("Source provider Pyth release: {error:?}"))?;
    let certificate_state = ResolutionCertificateV2::decode(&certificate.data)
        .map_err(|error| format!("Source provider terminal certificate: {error:?}"))?;
    if update.key.to_bytes() != lifecycle_state.update_account
        || update.owner.to_bytes() != release.receiver_program()
        || update.executable
        || update.lamports != lifecycle_state.update_rent_lamports
        || solana_program::hash::hash(&update.data).to_bytes() != lifecycle_state.update_digest
        || authority.key.to_bytes() != lifecycle_state.update_authority
        || authority.owner != solana_sdk_ids::system_program::ID
        || authority.lamports != 0
        || authority.executable
        || !authority.data.is_empty()
        || refund.key.to_bytes() != lifecycle_state.refund_recipient
        || refund.executable
        || certificate.key.to_bytes() != lifecycle_state.certificate
        || certificate.owner != resolution_program
        || certificate.executable
        || certificate_state.market != lifecycle_state.market
        || certificate_state.source_material != lifecycle_state.source_material
        || certificate_state.provider_evidence != lifecycle_state.provider_evidence
        || certificate_state.receipt_account != lifecycle_state.certificate
        || certificate_state.generation != lifecycle_state.generation
    {
        return Err("Source provider reclaim writable or certificate prestate changed".to_owned());
    }
    let expected_refund_lamports = refund
        .lamports
        .checked_add(lifecycle_state.update_rent_lamports)
        .and_then(|value| value.checked_add(lifecycle.lamports))
        .ok_or_else(|| "Source provider reclaim refund overflow".to_owned())?;
    let tables = wire
        .lookup_table
        .map(|account| observed(account, observation, "lookup table"))
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    let recent_blockhash = Hash::from_str(&wire.recent_blockhash)
        .map_err(|_| "Source provider recent blockhash is not canonical base58".to_owned())?;
    let plan = compile_provider_reclaim_v0(&report, recent_blockhash, &tables, payer)
        .map_err(|error| format!("Source provider reclaim transaction: {error:?}"))?;
    if plan.required_signers.as_slice() != [payer, resolver] {
        return Err("Source provider reclaim signer boundary changed".to_owned());
    }
    let completion = report
        .instruction
        .accounts
        .iter()
        .enumerate()
        .filter(|(index, _)| matches!(index, 1..=4))
        .map(|(_, account)| account.pubkey.to_string())
        .collect();
    serde_json::to_string(&PlanOutputV1 {
        format: PLAN_FORMAT_V1,
        route: "reclaim",
        observed_slot: slot.to_string(),
        instruction: instruction(&report),
        unsigned_message_base64: STANDARD.encode(plan.message.message.serialize()),
        required_signers: plan
            .required_signers
            .iter()
            .map(ToString::to_string)
            .collect(),
        wire_bytes: plan.message.wire_bytes,
        loaded_addresses: plan.message.loaded_addresses,
        lookup_tables: plan
            .message
            .lookup_tables
            .iter()
            .map(ToString::to_string)
            .collect(),
        lifecycle: report.lifecycle.to_string(),
        update_authority: report.update_authority.to_string(),
        completion,
        expected_poststates: vec![
            vacant_output(lifecycle.key),
            vacant_output(update.key),
            vacant_output(authority.key),
            AccountOutputV1 {
                address: refund.key.to_string(),
                owner: refund.owner.to_string(),
                lamports: expected_refund_lamports.to_string(),
                executable: refund.executable,
                data_base64: STANDARD.encode(&refund.data),
            },
        ],
    })
    .map_err(|error| format!("Source provider reclaim output: {error}"))
}

fn vacant_output(address: Pubkey) -> AccountOutputV1 {
    AccountOutputV1 {
        address: address.to_string(),
        owner: solana_sdk_ids::system_program::ID.to_string(),
        lamports: "0".to_owned(),
        executable: false,
        data_base64: String::new(),
    }
}

fn instruction(report: &ProviderTransportReportV3) -> InstructionOutputV1 {
    let Instruction {
        program_id,
        accounts,
        data,
    } = &report.instruction;
    InstructionOutputV1 {
        program: program_id.to_string(),
        accounts: accounts
            .iter()
            .map(|account| MetaOutputV1 {
                address: account.pubkey.to_string(),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data_base64: STANDARD.encode(data),
    }
}

fn discovery_observation() -> Observation {
    Observation {
        slot: 1,
        unix_timestamp: 1,
        finality: Finality::Finalized,
    }
}

fn observed(
    wire: AccountWireV1,
    observation: Observation,
    label: &str,
) -> Result<ObservedAccount, String> {
    let data = exact_base64(&wire.data_base64, &format!("{label} data"))?;
    Ok(ObservedAccount {
        observation,
        key: exact_key(&wire.address, &format!("{label} address"))?,
        owner: exact_owner(&wire.owner, &format!("{label} owner"))?,
        lamports: exact_u64(&wire.lamports, &format!("{label} lamports"), true)?,
        executable: wire.executable,
        data,
    })
}

fn exact_key(source: &str, label: &str) -> Result<Pubkey, String> {
    let key = Pubkey::from_str(source).map_err(|_| format!("{label} is not one Solana address"))?;
    if key.to_string() != source || key == Pubkey::default() {
        return Err(format!("{label} is not canonical nonzero base58"));
    }
    Ok(key)
}

fn exact_owner(source: &str, label: &str) -> Result<Pubkey, String> {
    let key = Pubkey::from_str(source).map_err(|_| format!("{label} is not one Solana address"))?;
    if key.to_string() != source {
        return Err(format!("{label} is not canonical base58"));
    }
    Ok(key)
}

fn exact_u64(source: &str, label: &str, allow_zero: bool) -> Result<u64, String> {
    if source.is_empty()
        || source.starts_with('+')
        || (source.len() > 1 && source.starts_with('0'))
        || !source.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{label} is not canonical unsigned decimal"));
    }
    let value = source
        .parse::<u64>()
        .map_err(|_| format!("{label} exceeds u64"))?;
    if !allow_zero && value == 0 {
        return Err(format!("{label} must be positive"));
    }
    Ok(value)
}

fn exact_i64(source: &str, label: &str) -> Result<i64, String> {
    if source.is_empty()
        || source.starts_with('+')
        || source == "-0"
        || (source.starts_with('0') && source.len() > 1)
        || (source.starts_with("-0") && source.len() > 2)
    {
        return Err(format!("{label} is not canonical signed decimal"));
    }
    source
        .parse::<i64>()
        .map_err(|_| format!("{label} exceeds i64"))
}

fn exact_base64(source: &str, label: &str) -> Result<Vec<u8>, String> {
    let decoded = STANDARD
        .decode(source)
        .map_err(|_| format!("{label} is not canonical base64"))?;
    if decoded.len() > MAX_ACCOUNT_BYTES || STANDARD.encode(&decoded) != source {
        return Err(format!(
            "{label} is outside its bound or not canonical base64"
        ));
    }
    Ok(decoded)
}

fn bounded_exact_json(source: &[u8]) -> Result<Value, String> {
    if source.is_empty() || source.len() > MAX_JSON_BYTES {
        return Err("Source provider input is outside its bounded JSON size".to_owned());
    }
    let mut deserializer = serde_json::Deserializer::from_slice(source);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| format!("Source provider JSON: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("Source provider JSON trailing bytes: {error}"))?;
    Ok(value)
}

struct ExactJsonValueSeedV1;
impl<'de> DeserializeSeed<'de> for ExactJsonValueSeedV1 {
    type Value = Value;
    fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
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
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
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
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use serde_json::json;

    #[test]
    fn strict_transport_refuses_duplicate_unknown_and_noncanonical_scalars() {
        assert!(
            plan_provider_reclaim_json_v1(br#"{"format":"x","format":"y"}"#)
                .expect_err("duplicate")
                .contains("duplicate JSON object key")
        );
        assert!(plan_provider_reclaim_json_v1(br#"{"format":"dclutch-source-provider-reclaim-input-v1","observedSlot":"01","unixTimestamp":"1","recentBlockhash":"x","lifecycle":{},"pythRelease":{},"deployment":{},"lookupTable":null,"extra":1}"#).is_err());
        assert!(exact_u64("01", "slot", false).is_err());
        assert!(exact_i64("-0", "timestamp").is_err());
        assert!(exact_base64("AA", "data").is_err());
    }

    #[test]
    fn submit_poststate_reauthenticates_request_and_update_digest() {
        let key = |byte: u8| Pubkey::new_from_array([byte; 32]);
        let update_data = vec![1, 2, 3, 4];
        let request = ProviderSubmitRequestV3 {
            generation: 7,
            reclaim_after_unix_seconds: 1_900_000_000,
            market: key(1).to_bytes(),
            source_state: key(2).to_bytes(),
            lifecycle: key(3).to_bytes(),
            source_material: key(4).to_bytes(),
            provider_release: key(5).to_bytes(),
            update_account: key(6).to_bytes(),
            provider_submitter: key(7).to_bytes(),
            refund_recipient: key(8).to_bytes(),
            release_set: key(9).to_bytes(),
            registry_program: key(10).to_bytes(),
            encoded_vaa: key(11).to_bytes(),
            post_body_digest: key(12).to_bytes(),
        };
        let authority = key(13);
        let resolution = key(14);
        let receiver = key(15);
        let lifecycle = ProviderUpdateLifecycleV3::submitted(
            request,
            1,
            authority.to_bytes(),
            request.registry_program,
            solana_program::hash::hash(&update_data).to_bytes(),
            1_800_000_000,
            90,
            2_000,
            1,
        )
        .expect("submitted lifecycle")
        .to_bytes()
        .expect("lifecycle bytes");
        let account = |address: Pubkey, owner: Pubkey, lamports: u64, data: &[u8]| {
            json!({
                "address": address.to_string(), "owner": owner.to_string(),
                "lamports": lamports.to_string(), "executable": false,
                "dataBase64": STANDARD.encode(data),
            })
        };
        let input = json!({
            "format": SUBMIT_POSTSTATE_INPUT_FORMAT_V1,
            "expectation": {
                "lifecycle": key(3).to_string(), "updateAccount": key(6).to_string(),
                "updateAuthority": authority.to_string(), "resolutionProgram": resolution.to_string(),
                "receiverProgram": receiver.to_string(),
                "submitRequestBase64": STANDARD.encode(request.to_bytes().expect("request")),
            },
            "lifecycle": account(key(3), resolution, 1_000, &lifecycle),
            "update": account(key(6), receiver, 2_000, &update_data),
        });
        let complete = verify_provider_submit_poststate_json_v1(input.to_string().as_bytes())
            .expect("complete poststate");
        assert!(complete.contains("\"complete\":true"));

        let mut changed = input;
        changed["update"]["dataBase64"] = json!(STANDARD.encode([9, 9, 9]));
        let incomplete = verify_provider_submit_poststate_json_v1(changed.to_string().as_bytes())
            .expect("incomplete poststate");
        assert!(incomplete.contains("\"complete\":false"));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PriceInputWireV1 {
    format: String,
    receiver_program: String,
    price_update: AccountWireV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PriceOutputV1 {
    format: &'static str,
    address: String,
    feed_id: String,
    price: String,
    confidence: String,
    exponent: i32,
    publish_time: String,
    posted_slot: String,
}

/// Read one sponsored `PriceUpdateV2` account, exactly as the Source family does.
///
/// THE DEFECT THIS CLOSES. The founding wizard's band was centred on a typed
/// number -- 15,000 ticks, a $150 SOL, three months stale while SOL traded near
/// $100 -- because the browser had no way to read a price and the only
/// alternative was to restate Pyth's account layout in TypeScript. That layout
/// already has one owner in this tree, `dclutch_source::pyth::FullPriceUpdateV2`,
/// and it is the same decoder the resolution path grades against. So the wizard
/// reads the feed through this boundary rather than growing a second reader,
/// and the number it centres on is the one the founding will resolve against.
///
/// The owner is checked against the receiver program the caller names, so a
/// well-formed account belonging to something else is refused rather than
/// decoded: a 134-byte account with the right discriminator is not a price
/// unless the program that maintains it says so.
pub fn read_sponsored_price_update_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: PriceInputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Source provider price schema: {error}"))?;
    if wire.format != PRICE_INPUT_FORMAT_V1 {
        return Err("Source provider price input has another format".to_owned());
    }
    let receiver = exact_key(&wire.receiver_program, "receiver program")?;
    let address = exact_key(&wire.price_update.address, "price update")?;
    if exact_key(&wire.price_update.owner, "price update owner")? != receiver {
        return Err(
            "Source provider price update is not owned by the named receiver program".to_owned(),
        );
    }
    if wire.price_update.executable {
        return Err("Source provider price update is executable".to_owned());
    }
    let data = exact_base64(&wire.price_update.data_base64, "price update data")?;
    let update = dclutch_source::pyth::FullPriceUpdateV2::parse(&data)
        .map_err(|error| format!("Source provider price update: {error:?}"))?;
    serde_json::to_string(&PriceOutputV1 {
        format: PRICE_FORMAT_V1,
        address: address.to_string(),
        feed_id: update
            .feed_id()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        price: update.price().to_string(),
        confidence: update.confidence().to_string(),
        exponent: update.exponent(),
        publish_time: update.publish_time().to_string(),
        posted_slot: update.posted_slot().to_string(),
    })
    .map_err(|error| format!("Source provider price output: {error}"))
}
