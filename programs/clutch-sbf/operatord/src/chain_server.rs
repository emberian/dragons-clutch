//! Explicit, bounded, read-only RPC acquisition for the Glass chain console.
//!
//! This mode checks the selected genesis and every Program/ProgramData/slot/
//! ELF tuple through the configured untrusted RPC before exposing a release,
//! then repeatedly admits finalized `getProgramAccounts` scans through
//! `RpcIndexEngine`. A separately bounded, ordered WebSocket owner publishes a
//! processed generation only after complete subscription registration,
//! release-bracketed scan, and notification replay.

use crate::{
    bus::Bus,
    index_api::{ProcessedTransportState, SharedIndexApi},
    payoff_compiler, processed_ws, Result,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use clutch_local_real_pyth::account_index::{
    CanonicalDecoderContext, IndexCapacity, CANONICAL_ACCOUNT_DECODER_SET,
};
use clutch_local_real_pyth::index_service::RpcIndexEngine;
use clutch_local_real_pyth::operatord::ResumableKeeperSelector;
use clutch_local_real_pyth::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, CompiledSourceProfile, IndexedProgramRelease,
    PlannedRpcRequest, RpcAcquisitionBounds, RpcClusterBinding, RpcIndexPlan,
};
use clutch_sbf::loader_state::{
    decode_loader_pair_v1, LoaderAccountViewV1, PROGRAMDATA_METADATA_LEN,
};
use clutch_source_plane_v3_runtime::RuntimeKey;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_address::Address;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

const CONFIG_SCHEMA: &str = "dragons-clutch/operatord-chain-config/v3";
const MAX_CONFIG_BYTES: usize = 262_144;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ChainConfigWire {
    schema: String,
    decoder_set: String,
    cluster: ClusterWire,
    releases: Vec<ReleaseWire>,
    source_neutral_sink: String,
    workflow_id: String,
    maximum_keeper_actions: String,
    bounds: BoundsWire,
    polling_interval_milliseconds: String,
    rpc_timeout_seconds: String,
    websocket_reconnect_initial_milliseconds: String,
    websocket_reconnect_maximum_milliseconds: String,
    compiler_release_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ClusterWire {
    name: String,
    genesis_hash: String,
    rpc_http_url: String,
    rpc_websocket_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReleaseWire {
    program_id: String,
    program_data: String,
    elf_sha256: String,
    deployment_slot: String,
    release_manifest_sha256: String,
    capability_profile_id: String,
    source_commit: String,
    source_profile: String,
    registered_source_release_count: String,
    enabled_intents: Vec<IntentWire>,
    families: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct IntentWire {
    family_tag: String,
    family_version: String,
    local_action: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BoundsWire {
    maximum_accounts_per_scan: String,
    maximum_account_data_bytes: String,
    maximum_total_response_bytes: String,
    maximum_subscriptions: String,
    maximum_addresses: String,
    maximum_versions_per_address: String,
    maximum_fork_nodes: String,
}

struct ChainConfig {
    plan: RpcIndexPlan,
    capacity: IndexCapacity,
    source_neutral_sink: RuntimeKey,
    selector: ResumableKeeperSelector,
    polling_interval: Duration,
    rpc_timeout_seconds: u64,
    websocket_reconnect_initial: Duration,
    websocket_reconnect_maximum: Duration,
    compiler_release_sha256: String,
}

#[derive(Debug)]
struct RpcAccount {
    owner: Address,
    executable: bool,
    data: Vec<u8>,
}

fn canonical_unsigned(text: &str, name: &str) -> Result<()> {
    if text.is_empty()
        || (text != "0" && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{name} must be a canonical unsigned decimal string").into());
    }
    Ok(())
}

fn parse_unsigned<T>(text: &str, name: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    canonical_unsigned(text, name)?;
    text.parse::<T>()
        .map_err(|_| format!("{name} exceeds its exact integer width").into())
}

fn decode_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hash32(text: &str, name: &str) -> Result<[u8; 32]> {
    if text.len() != 64 {
        return Err(format!("{name} must contain exactly 32 lowercase hexadecimal bytes").into());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_nibble(pair[0])
            .ok_or_else(|| format!("{name} must be lowercase hexadecimal"))?;
        let low = decode_nibble(pair[1])
            .ok_or_else(|| format!("{name} must be lowercase hexadecimal"))?;
        output[index] = (high << 4) | low;
    }
    if output == [0; 32] {
        return Err(format!("{name} must be nonzero").into());
    }
    Ok(output)
}

fn address(text: &str, name: &str) -> Result<Address> {
    let value = Address::from_str(text).map_err(|_| format!("{name} is not a Solana address"))?;
    if value == Address::default() {
        return Err(format!("{name} must be nonzero").into());
    }
    Ok(value)
}

fn family(text: &str) -> Result<CanonicalFamily> {
    Ok(match text {
        "collateral" => CanonicalFamily::Collateral,
        "fractional" => CanonicalFamily::Fractional,
        "general" => CanonicalFamily::General,
        "product" => CanonicalFamily::Product,
        "source" => CanonicalFamily::Source,
        "series" => CanonicalFamily::Series,
        "fees" => CanonicalFamily::Fees,
        "liveness" => CanonicalFamily::Liveness,
        "position-v3" => CanonicalFamily::PositionV3,
        "replay-v3" => CanonicalFamily::ReplayV3,
        "structured-claim" => CanonicalFamily::StructuredClaim,
        "dealer" => CanonicalFamily::Dealer,
        "failure" => CanonicalFamily::Failure,
        _ => return Err(format!("unknown canonical decoder family {text}").into()),
    })
}

fn parse_config(path: &Path) -> Result<ChainConfig> {
    let bytes = std::fs::read(path)?;
    parse_config_bytes(&bytes)
}

pub(crate) fn validate_chain_config_bytes(bytes: &[u8]) -> Result<()> {
    parse_config_bytes(bytes).map(|_| ())
}

fn parse_config_bytes(bytes: &[u8]) -> Result<ChainConfig> {
    if bytes.is_empty() || bytes.len() > MAX_CONFIG_BYTES {
        return Err(format!("chain config must contain 1..={MAX_CONFIG_BYTES} bytes").into());
    }
    let wire: ChainConfigWire = serde_json::from_slice(&bytes)?;
    if wire.schema != CONFIG_SCHEMA {
        return Err("chain config schema is not operatord-chain-config/v3".into());
    }
    if wire.decoder_set != CANONICAL_ACCOUNT_DECODER_SET {
        return Err(format!(
            "chain config decoderSet must be exactly {CANONICAL_ACCOUNT_DECODER_SET}"
        )
        .into());
    }
    if wire.releases.is_empty() || wire.releases.len() > 64 {
        return Err("chain config must contain 1..=64 explicit releases".into());
    }
    let mut releases = Vec::with_capacity(wire.releases.len());
    for (index, release) in wire.releases.into_iter().enumerate() {
        let families = release
            .families
            .iter()
            .map(|name| family(name))
            .collect::<Result<Vec<_>>>()?;
        let enabled_intents = release
            .enabled_intents
            .into_iter()
            .map(|intent| {
                Ok(CanonicalIntentCoordinate {
                    family_tag: parse_unsigned(
                        &intent.family_tag,
                        &format!("releases[{index}].enabledIntents.familyTag"),
                    )?,
                    family_version: parse_unsigned(
                        &intent.family_version,
                        &format!("releases[{index}].enabledIntents.familyVersion"),
                    )?,
                    local_action: parse_unsigned(
                        &intent.local_action,
                        &format!("releases[{index}].enabledIntents.localAction"),
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        releases.push(IndexedProgramRelease {
            program_id: address(&release.program_id, &format!("releases[{index}].programId"))?,
            program_data: address(
                &release.program_data,
                &format!("releases[{index}].programData"),
            )?,
            elf_sha256: hash32(&release.elf_sha256, &format!("releases[{index}].elfSha256"))?,
            deployment_slot: parse_unsigned(
                &release.deployment_slot,
                &format!("releases[{index}].deploymentSlot"),
            )?,
            release_manifest_sha256: hash32(
                &release.release_manifest_sha256,
                &format!("releases[{index}].releaseManifestSha256"),
            )?,
            capability_profile_id: hash32(
                &release.capability_profile_id,
                &format!("releases[{index}].capabilityProfileId"),
            )?,
            source_commit: release.source_commit,
            source_profile: {
                let profile = CompiledSourceProfile::parse(&release.source_profile)
                    .map_err(|_| format!("releases[{index}].sourceProfile is unsupported"))?;
                if release.registered_source_release_count
                    != profile.registered_release_count().to_string()
                {
                    return Err(format!(
                        "releases[{index}].registeredSourceReleaseCount differs from the compiled Source profile"
                    )
                    .into());
                }
                profile
            },
            enabled_intents,
            families,
        });
    }
    let plan = RpcIndexPlan {
        cluster: RpcClusterBinding {
            cluster_name: wire.cluster.name,
            genesis_hash: wire.cluster.genesis_hash,
            rpc_http_url: wire.cluster.rpc_http_url,
            rpc_websocket_url: wire.cluster.rpc_websocket_url,
        },
        releases,
        bounds: RpcAcquisitionBounds {
            maximum_accounts_per_scan: parse_unsigned(
                &wire.bounds.maximum_accounts_per_scan,
                "bounds.maximumAccountsPerScan",
            )?,
            maximum_account_data_bytes: parse_unsigned(
                &wire.bounds.maximum_account_data_bytes,
                "bounds.maximumAccountDataBytes",
            )?,
            maximum_total_response_bytes: parse_unsigned(
                &wire.bounds.maximum_total_response_bytes,
                "bounds.maximumTotalResponseBytes",
            )?,
            maximum_subscriptions: parse_unsigned(
                &wire.bounds.maximum_subscriptions,
                "bounds.maximumSubscriptions",
            )?,
        },
    };
    plan.validate()?;
    let sink = address(&wire.source_neutral_sink, "sourceNeutralSink")?;
    let capacity = IndexCapacity {
        maximum_addresses: parse_unsigned(
            &wire.bounds.maximum_addresses,
            "bounds.maximumAddresses",
        )?,
        maximum_versions_per_address: parse_unsigned(
            &wire.bounds.maximum_versions_per_address,
            "bounds.maximumVersionsPerAddress",
        )?,
        maximum_fork_nodes: parse_unsigned(
            &wire.bounds.maximum_fork_nodes,
            "bounds.maximumForkNodes",
        )?,
    };
    if capacity.maximum_addresses > 262_144
        || capacity.maximum_versions_per_address > 64
        || capacity.maximum_fork_nodes > 262_144
    {
        return Err("index capacity exceeds maximumAddresses=262144, maximumVersionsPerAddress=64, or maximumForkNodes=262144".into());
    }
    let polling_interval_ms: u64 = parse_unsigned(
        &wire.polling_interval_milliseconds,
        "pollingIntervalMilliseconds",
    )?;
    if !(1_000..=3_600_000).contains(&polling_interval_ms) {
        return Err("pollingIntervalMilliseconds must be in 1000..=3600000".into());
    }
    let rpc_timeout_seconds: u64 = parse_unsigned(&wire.rpc_timeout_seconds, "rpcTimeoutSeconds")?;
    if !(1..=120).contains(&rpc_timeout_seconds) {
        return Err("rpcTimeoutSeconds must be in 1..=120".into());
    }
    let reconnect_initial_ms: u64 = parse_unsigned(
        &wire.websocket_reconnect_initial_milliseconds,
        "websocketReconnectInitialMilliseconds",
    )?;
    let reconnect_maximum_ms: u64 = parse_unsigned(
        &wire.websocket_reconnect_maximum_milliseconds,
        "websocketReconnectMaximumMilliseconds",
    )?;
    if !(100..=10_000).contains(&reconnect_initial_ms)
        || reconnect_maximum_ms < reconnect_initial_ms
        || reconnect_maximum_ms > 60_000
    {
        return Err("WebSocket reconnect bounds require initial 100..=10000ms and initial<=maximum<=60000ms".into());
    }
    let selector = ResumableKeeperSelector {
        workflow_id: hash32(&wire.workflow_id, "workflowId")?,
        maximum_actions: parse_unsigned(&wire.maximum_keeper_actions, "maximumKeeperActions")?,
    };
    selector.validate()?;
    // Validate the compiler pin before any RPC process is opened.
    hash32(&wire.compiler_release_sha256, "compilerReleaseSha256")?;
    Ok(ChainConfig {
        plan,
        capacity,
        source_neutral_sink: RuntimeKey::from_bytes(sink.to_bytes()),
        selector,
        polling_interval: Duration::from_millis(polling_interval_ms),
        rpc_timeout_seconds,
        websocket_reconnect_initial: Duration::from_millis(reconnect_initial_ms),
        websocket_reconnect_maximum: Duration::from_millis(reconnect_maximum_ms),
        compiler_release_sha256: wire.compiler_release_sha256,
    })
}

fn rpc_call(url: &str, body: &Value, maximum_bytes: usize, timeout_seconds: u64) -> Result<Value> {
    let timeout = timeout_seconds.to_string();
    let maximum = maximum_bytes.to_string();
    let payload = body.to_string();
    let output = Command::new("curl")
        .args([
            "-q",
            "--fail-with-body",
            "--silent",
            "--show-error",
            "--max-time",
            &timeout,
            "--connect-timeout",
            &timeout,
            "--max-filesize",
            &maximum,
            "--max-redirs",
            "0",
            "--noproxy",
            "*",
            "--proxy",
            "",
            "--proto",
            "=https,http",
            "-H",
            "Content-Type: application/json",
            "-X",
            "POST",
            "--data-binary",
            &payload,
            url,
        ])
        .output()?;
    if output.stdout.len() > maximum_bytes {
        return Err("RPC response exceeded the configured byte bound".into());
    }
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.chars().take(1_024).collect::<String>();
        return Err(format!("bounded read-only RPC request failed: {detail}").into());
    }
    let response: Value = serde_json::from_slice(&output.stdout)?;
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err("RPC response has an invalid jsonrpc version".into());
    }
    let has_result = response.get("result").is_some();
    let has_error = response.get("error").is_some_and(|error| !error.is_null());
    if has_result == has_error {
        return Err("RPC response must contain exactly one result or error".into());
    }
    if let Some(error) = response.get("error").filter(|error| !error.is_null()) {
        return Err(format!("read-only RPC returned an error: {error}").into());
    }
    Ok(response)
}

fn rpc_result<'a>(response: &'a Value, expected_id: u64) -> Result<&'a Value> {
    if response.get("id").and_then(Value::as_u64) != Some(expected_id) {
        return Err("RPC response id differs from the exact request id".into());
    }
    response
        .get("result")
        .ok_or_else(|| "RPC response is missing result".into())
}

fn rpc_account(value: &Value, maximum_data_bytes: usize, name: &str) -> Result<RpcAccount> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name} has no data tuple"))?;
    if data.len() != 2 || data.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(format!("{name} data is not an exact base64 tuple").into());
    }
    let encoded = data
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} has no base64 body"))?;
    let body = BASE64.decode(encoded)?;
    if body.len() > maximum_data_bytes {
        return Err(format!("{name} exceeds maximumAccountDataBytes").into());
    }
    Ok(RpcAccount {
        owner: address(
            value
                .get("owner")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{name} has no owner"))?,
            &format!("{name}.owner"),
        )?,
        executable: value
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("{name} has no executable bit"))?,
        data: body,
    })
}

pub(crate) fn verify_chain_bindings(plan: &RpcIndexPlan, timeout_seconds: u64) -> Result<()> {
    let maximum = plan.bounds.maximum_total_response_bytes;
    let genesis_response = rpc_call(
        &plan.cluster.rpc_http_url,
        &json!({"jsonrpc":"2.0", "id":9000000, "method":"getGenesisHash", "params":[]}),
        maximum,
        timeout_seconds,
    )?;
    let genesis = rpc_result(&genesis_response, 9_000_000)?
        .as_str()
        .ok_or("getGenesisHash result is not text")?;
    if genesis != plan.cluster.genesis_hash {
        return Err("RPC genesis hash differs from the explicit chain configuration".into());
    }
    for (index, release) in plan.releases.iter().enumerate() {
        let request_id = 9_000_001_u64
            .checked_add(u64::try_from(index)?)
            .ok_or("release verification request id overflow")?;
        let response = rpc_call(
            &plan.cluster.rpc_http_url,
            &json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "method":"getMultipleAccounts",
                "params":[[release.program_id.to_string(), release.program_data.to_string()], {"commitment":"finalized", "encoding":"base64"}]
            }),
            maximum,
            timeout_seconds,
        )?;
        let values = rpc_result(&response, request_id)?
            .get("value")
            .and_then(Value::as_array)
            .ok_or("getMultipleAccounts result has no value array")?;
        if values.len() != 2 || values.iter().any(Value::is_null) {
            return Err(format!("release {index} Program/ProgramData account is absent").into());
        }
        let program = rpc_account(
            &values[0],
            plan.bounds.maximum_account_data_bytes,
            &format!("releases[{index}].program"),
        )?;
        let programdata = rpc_account(
            &values[1],
            plan.bounds.maximum_account_data_bytes,
            &format!("releases[{index}].programData"),
        )?;
        let decoded = decode_loader_pair_v1(
            LoaderAccountViewV1::new(
                release.program_id.to_bytes(),
                program.owner.to_bytes(),
                program.executable,
                &program.data,
            ),
            LoaderAccountViewV1::new(
                release.program_data.to_bytes(),
                programdata.owner.to_bytes(),
                programdata.executable,
                &programdata.data,
            ),
        )
        .map_err(|error| format!("release {index} loader binding refused: {error:?}"))?;
        if decoded.state.linked_programdata != release.program_data.to_bytes()
            || decoded.state.deployment_slot != release.deployment_slot
        {
            return Err(format!("release {index} ProgramData address or slot differs").into());
        }
        let elf = programdata
            .data
            .get(PROGRAMDATA_METADATA_LEN..)
            .ok_or_else(|| format!("release {index} ProgramData has no ELF body"))?;
        if elf.is_empty() || solana_sha256_hasher::hash(elf).to_bytes() != release.elf_sha256 {
            return Err(format!("release {index} ProgramData ELF SHA-256 differs").into());
        }
    }
    Ok(())
}

fn pending_scan_requests(engine: &Arc<RwLock<RpcIndexEngine>>) -> Result<Vec<PlannedRpcRequest>> {
    let guard = engine
        .read()
        .map_err(|_| "operator index read lock is unavailable")?;
    Ok(guard.bootstrap_requests().into_iter().cloned().collect())
}

fn admit_scans(
    engine: &Arc<RwLock<RpcIndexEngine>>,
    rpc_url: &str,
    maximum_bytes: usize,
    timeout_seconds: u64,
) -> Result<()> {
    for request in pending_scan_requests(engine)? {
        let response = rpc_call(rpc_url, &request.body, maximum_bytes, timeout_seconds)?;
        engine
            .write()
            .map_err(|_| "operator index write lock is unavailable")?
            .admit_scan_response(request.request_id, &response)?;
    }
    Ok(())
}

fn spawn_finalized_poller(
    engine: Arc<RwLock<RpcIndexEngine>>,
    plan: RpcIndexPlan,
    timeout_seconds: u64,
    interval: Duration,
    scan_gate: Arc<Mutex<()>>,
    ready: Arc<RwLock<bool>>,
) {
    thread::spawn(move || loop {
        thread::sleep(interval);
        let outcome =
            refresh_finalized_projection(&engine, &plan, timeout_seconds, &scan_gate, &ready);
        if let Err(error) = outcome {
            eprintln!("operatord chain poller: {error}");
        }
    });
}

/// Execute one serialized release-check → finalized scan → release-check
/// cycle. Every caller shares `scan_gate`, including the WebSocket generation
/// bootstrap that buffers notifications while this cycle runs.
pub(crate) fn refresh_finalized_projection(
    engine: &Arc<RwLock<RpcIndexEngine>>,
    plan: &RpcIndexPlan,
    timeout_seconds: u64,
    scan_gate: &Arc<Mutex<()>>,
    ready: &Arc<RwLock<bool>>,
) -> Result<()> {
    let _scan = scan_gate
        .lock()
        .map_err(|_| "finalized scan gate is unavailable")?;
    *ready
        .write()
        .map_err(|_| "release readiness lock is unavailable")? = false;
    verify_chain_bindings(plan, timeout_seconds)?;
    {
        let mut guard = engine
            .write()
            .map_err(|_| "operator index write lock is unavailable")?;
        if guard.bootstrap_complete() {
            guard.begin_finalized_rescan()?;
        }
    }
    admit_scans(
        engine,
        &plan.cluster.rpc_http_url,
        plan.bounds.maximum_total_response_bytes,
        timeout_seconds,
    )?;
    verify_chain_bindings(plan, timeout_seconds)?;
    *ready
        .write()
        .map_err(|_| "release readiness lock is unavailable")? = true;
    Ok(())
}

/// Verify explicit chain coordinates, bootstrap the untrusted index, and serve
/// only Glass static files, GET projections, and the pure payoff compiler.
pub fn serve(port: u16, statics: PathBuf, config_path: &Path) -> Result<()> {
    let config = parse_config(config_path)?;
    let polling_plan = config.plan.clone();
    let engine = Arc::new(RwLock::new(RpcIndexEngine::new(
        config.plan,
        CanonicalDecoderContext {
            source_neutral_sink: config.source_neutral_sink,
        },
        config.capacity,
    )?));
    let scan_gate = Arc::new(Mutex::new(()));
    let ready = Arc::new(RwLock::new(false));
    refresh_finalized_projection(
        &engine,
        &polling_plan,
        config.rpc_timeout_seconds,
        &scan_gate,
        &ready,
    )?;
    let processed = Arc::new(RwLock::new(ProcessedTransportState::default()));
    let base_read_api =
        SharedIndexApi::processed(Arc::clone(&engine), config.selector, Arc::clone(&processed))
            .read_api();
    let read_ready = Arc::clone(&ready);
    let read_api: crate::http::ReadApi = Arc::new(move |method, target| {
        if !read_ready.read().map(|state| *state).unwrap_or(false) {
            return Some(crate::http::JsonReadResponse {
                status: 503,
                body: json!({
                    "error": "chain projection is unavailable until release re-verification and a complete finalized scan succeed"
                }),
            });
        }
        base_read_api(method, target)
    });
    let post_api = payoff_compiler::post_api(config.compiler_release_sha256)?;
    processed_ws::spawn(
        Arc::clone(&engine),
        polling_plan.clone(),
        config.rpc_timeout_seconds,
        config.websocket_reconnect_initial,
        config.websocket_reconnect_maximum,
        Arc::clone(&scan_gate),
        Arc::clone(&ready),
        processed,
    );
    spawn_finalized_poller(
        engine,
        polling_plan,
        config.rpc_timeout_seconds,
        config.polling_interval,
        scan_gate,
        ready,
    );
    let server =
        crate::http::Server::bind_pure(port, Bus::new(), statics, Some(read_api), Some(post_api))?;
    println!(
        "Glass chain reader listening on http://127.0.0.1:{} (configured HTTP+WebSocket coordinates, finalized polling plus rollbackable processed subscriptions, untrusted projection, no wallet/sign/submit/persist)",
        server.port()?
    );
    server.serve_forever();
    Ok(())
}
