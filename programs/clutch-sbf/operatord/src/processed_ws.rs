//! One ordered, bounded owner for the processed Solana subscription plane.
//!
//! A connection generation is never partially published. Notifications are
//! buffered from registration through a release-bracketed finalized scan,
//! replayed in wire order, and only then exposed. Any transport, decode, fork,
//! capacity, or release error withdraws the whole processed generation.

use crate::{
    chain_server::refresh_finalized_projection,
    index_api::{ProcessedTransportState, SharedProcessedTransport},
    Result,
};
use clutch_local_real_pyth::index_service::{RpcIndexEngine, RpcIndexEngineEvent};
use clutch_local_real_pyth::rpc_index::{
    public_rpc_endpoint_binding, PlannedRpcRequest, RpcIndexPlan,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{client_tls_with_config, Error as WebSocketError, Message, WebSocket};

const MAX_RESOLVED_ADDRESSES: usize = 16;
const IO_POLL_INTERVAL: Duration = Duration::from_millis(250);
const READ_BUFFER_BYTES: usize = 16 * 1024;
const WRITE_BUFFER_BYTES: usize = 16 * 1024;
const MAX_WRITE_BUFFER_BYTES: usize = 128 * 1024;
const GENESIS_CHALLENGE_REQUEST_ID: u64 = 9_100_000;

type RpcSocket = WebSocket<MaybeTlsStream<TcpStream>>;

enum Incoming {
    Timeout,
    Control,
    Json(Value, usize),
}

pub fn spawn(
    engine: Arc<RwLock<RpcIndexEngine>>,
    plan: RpcIndexPlan,
    timeout_seconds: u64,
    reconnect_initial: Duration,
    reconnect_maximum: Duration,
    scan_gate: Arc<Mutex<()>>,
    release_ready: Arc<RwLock<bool>>,
    state: SharedProcessedTransport,
) {
    thread::spawn(move || {
        let initial_rollback = match withdraw_engine(&engine) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("operatord processed transport cannot initialize: {error}");
                return;
            }
        };
        if update_state(&state, |state| state.withdraw_generation(initial_rollback)).is_err() {
            return;
        }
        let mut backoff = reconnect_initial;
        loop {
            if update_state(&state, ProcessedTransportState::begin_generation).is_err() {
                return;
            }
            let outcome = run_generation(
                &engine,
                &plan,
                timeout_seconds,
                &scan_gate,
                &release_ready,
                &state,
            );
            let error = match outcome {
                Ok(()) => {
                    "processed WebSocket generation ended without an explicit error".to_string()
                }
                Err(error) => {
                    redacted_error_detail(&error.to_string(), &plan.cluster.rpc_websocket_url)
                }
            };
            if state
                .read()
                .ok()
                .is_some_and(|state| state.snapshot().phase == "live-nonfinal")
            {
                backoff = reconnect_initial;
            }
            if update_state(&state, |state| {
                state.mark_withdrawing(&error);
                Ok(())
            })
            .is_err()
            {
                return;
            }
            let rollback = match withdraw_engine(&engine) {
                Ok(value) => value,
                Err(withdrawal_error) => {
                    eprintln!(
                        "operatord processed transport cannot withdraw failed generation: {withdrawal_error}"
                    );
                    return;
                }
            };
            let backoff_milliseconds = match u64::try_from(backoff.as_millis()) {
                Ok(value) => value,
                Err(_) => return,
            };
            if update_state(&state, |state| {
                state.withdraw_generation(rollback)?;
                state.mark_backoff(&error, backoff_milliseconds)
            })
            .is_err()
            {
                return;
            }
            eprintln!("operatord processed transport: {error}; retry in {backoff_milliseconds}ms");
            thread::sleep(backoff);
            backoff = backoff
                .checked_mul(2)
                .unwrap_or(reconnect_maximum)
                .min(reconnect_maximum);
        }
    });
}

fn redacted_error_detail(_detail: &str, websocket_url: &str) -> String {
    let websocket = public_rpc_endpoint_binding(websocket_url);
    format!(
        "processed WebSocket generation failed at {}; endpoint credentials and transport detail are withheld from the browser projection",
        websocket.redacted
    )
}

fn run_generation(
    engine: &Arc<RwLock<RpcIndexEngine>>,
    plan: &RpcIndexPlan,
    timeout_seconds: u64,
    scan_gate: &Arc<Mutex<()>>,
    release_ready: &Arc<RwLock<bool>>,
    state: &SharedProcessedTransport,
) -> Result<()> {
    let timeout = Duration::from_secs(timeout_seconds);
    let mut socket = connect(&plan.cluster.rpc_websocket_url, plan, timeout)?;
    update_state(state, |state| {
        state.mark_authenticating_genesis();
        Ok(())
    })?;
    authenticate_websocket_genesis(&mut socket, plan, timeout)?;
    update_state(state, ProcessedTransportState::mark_genesis_matched)?;
    update_state(state, ProcessedTransportState::mark_registering)?;
    let requests = subscription_requests(engine)?;
    if requests.len() != plan.releases.len().saturating_add(3) {
        return Err("processed generation does not own the complete program/block/slot/root subscription set".into());
    }
    for request in &requests {
        let body = serde_json::to_string(&request.body)?;
        if body.len() > plan.bounds.maximum_total_response_bytes {
            return Err("WebSocket subscription request exceeds the configured byte bound".into());
        }
        socket.send(Message::text(body))?;
    }

    let mut pending = requests
        .iter()
        .map(|request| request.request_id)
        .collect::<BTreeSet<_>>();
    let mut buffered = Vec::new();
    let mut buffered_bytes = 0_usize;
    let mut last_message = Instant::now();
    while !pending.is_empty() {
        match read_json(&mut socket, plan.bounds.maximum_total_response_bytes)? {
            Incoming::Timeout => require_not_idle(last_message, timeout)?,
            Incoming::Control => last_message = Instant::now(),
            Incoming::Json(value, bytes) => {
                last_message = Instant::now();
                if let Some(request_id) = response_id(&value)? {
                    if !pending.remove(&request_id) {
                        return Err("WebSocket returned an unexpected or duplicate subscription response id".into());
                    }
                    engine
                        .write()
                        .map_err(|_| "operator index write lock is unavailable")?
                        .admit_subscription_response(request_id, &value)?;
                } else {
                    buffer_notification(&mut buffered, &mut buffered_bytes, value, bytes, plan)?;
                }
            }
        }
    }

    let (scan_sender, scan_receiver) = mpsc::sync_channel(1);
    let scan_engine = Arc::clone(engine);
    let scan_plan = plan.clone();
    let scan_gate = Arc::clone(scan_gate);
    let scan_ready = Arc::clone(release_ready);
    thread::spawn(move || {
        let result = refresh_finalized_projection(
            &scan_engine,
            &scan_plan,
            timeout_seconds,
            &scan_gate,
            &scan_ready,
        )
        .map_err(|error| error.to_string());
        let _ignored = scan_sender.send(result);
    });

    let scan_result = loop {
        match scan_receiver.try_recv() {
            Ok(result) => break result,
            Err(mpsc::TryRecvError::Disconnected) => {
                break Err("finalized scan worker ended without a result".to_string())
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match read_json(&mut socket, plan.bounds.maximum_total_response_bytes) {
            Ok(Incoming::Timeout) => {
                if let Err(error) = require_not_idle(last_message, timeout) {
                    return wait_for_scan_then_error(scan_receiver, &error.to_string());
                }
            }
            Ok(Incoming::Control) => last_message = Instant::now(),
            Ok(Incoming::Json(value, bytes)) => {
                last_message = Instant::now();
                let response = match response_id(&value) {
                    Ok(response) => response,
                    Err(error) => {
                        return wait_for_scan_then_error(scan_receiver, &error.to_string())
                    }
                };
                if response.is_some() {
                    return wait_for_scan_then_error(
                        scan_receiver,
                        "unexpected JSON-RPC response after subscription registration",
                    );
                }
                if let Err(error) =
                    buffer_notification(&mut buffered, &mut buffered_bytes, value, bytes, plan)
                {
                    return wait_for_scan_then_error(scan_receiver, &error.to_string());
                }
            }
            Err(error) => return wait_for_scan_then_error(scan_receiver, &error.to_string()),
        }
    };
    scan_result.map_err(|error| format!("processed bootstrap finalized scan failed: {error}"))?;

    update_state(state, |state| {
        state.mark_replaying();
        Ok(())
    })?;
    for notification in buffered {
        admit_notification(engine, state, &notification)?;
    }
    update_state(state, |state| state.mark_live())?;

    last_message = Instant::now();
    loop {
        match read_json(&mut socket, plan.bounds.maximum_total_response_bytes)? {
            Incoming::Timeout => require_not_idle(last_message, timeout)?,
            Incoming::Control => last_message = Instant::now(),
            Incoming::Json(value, _) => {
                last_message = Instant::now();
                if response_id(&value)?.is_some() {
                    return Err(
                        "unexpected JSON-RPC response on a live processed subscription generation"
                            .into(),
                    );
                }
                admit_notification(engine, state, &value)?;
            }
        }
    }
}

fn authenticate_websocket_genesis(
    socket: &mut RpcSocket,
    plan: &RpcIndexPlan,
    timeout: Duration,
) -> Result<()> {
    let body = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": GENESIS_CHALLENGE_REQUEST_ID,
        "method": "getGenesisHash",
        "params": []
    }))?;
    if body.len() > plan.bounds.maximum_total_response_bytes {
        return Err("WebSocket genesis challenge exceeds the configured byte bound".into());
    }
    socket.send(Message::text(body))?;
    let started = Instant::now();
    loop {
        match read_json(socket, plan.bounds.maximum_total_response_bytes)? {
            Incoming::Timeout => require_not_idle(started, timeout)?,
            Incoming::Control => {}
            Incoming::Json(value, _) => {
                return validate_genesis_response(&value, &plan.cluster.genesis_hash);
            }
        }
    }
}

fn validate_genesis_response(value: &Value, expected_genesis: &str) -> Result<()> {
    if response_id(value)? != Some(GENESIS_CHALLENGE_REQUEST_ID) {
        return Err("WebSocket emitted a notification or unrelated response before its genesis challenge completed".into());
    }
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || value.get("error").is_some_and(|error| !error.is_null())
        || value.get("result").and_then(Value::as_str) != Some(expected_genesis)
    {
        return Err("WebSocket genesis challenge differs from the exact selected cluster".into());
    }
    Ok(())
}

fn connect(endpoint: &str, plan: &RpcIndexPlan, timeout: Duration) -> Result<RpcSocket> {
    let request = endpoint.into_client_request()?;
    let uri = request.uri();
    let host = uri.host().ok_or("WebSocket URL has no host")?;
    let port = uri
        .port_u16()
        .unwrap_or(if uri.scheme_str() == Some("wss") {
            443
        } else {
            80
        });
    let started = Instant::now();
    let addresses = (host, port)
        .to_socket_addrs()?
        .take(MAX_RESOLVED_ADDRESSES)
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("WebSocket hostname resolved to no address".into());
    }
    let mut stream = None;
    for address in addresses {
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            break;
        };
        if remaining.is_zero() {
            break;
        }
        if let Ok(candidate) = TcpStream::connect_timeout(&address, remaining) {
            stream = Some(candidate);
            break;
        }
    }
    let stream = stream.ok_or("WebSocket TCP connection did not complete within its bound")?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let websocket_config = WebSocketConfig::default()
        .read_buffer_size(READ_BUFFER_BYTES)
        .write_buffer_size(WRITE_BUFFER_BYTES)
        .max_write_buffer_size(MAX_WRITE_BUFFER_BYTES)
        .max_message_size(Some(plan.bounds.maximum_total_response_bytes))
        .max_frame_size(Some(plan.bounds.maximum_total_response_bytes));
    let (mut socket, response) =
        client_tls_with_config(request, stream, Some(websocket_config), None)
            .map_err(|error| format!("WebSocket handshake failed: {error:?}"))?;
    if response.status().as_u16() != 101 {
        return Err("WebSocket handshake did not return Switching Protocols".into());
    }
    set_read_timeout(&mut socket, IO_POLL_INTERVAL)?;
    Ok(socket)
}

fn set_read_timeout(socket: &mut RpcSocket, timeout: Duration) -> Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(Some(timeout))?,
        MaybeTlsStream::Rustls(stream) => stream.sock.set_read_timeout(Some(timeout))?,
        _ => return Err("unsupported WebSocket TLS stream variant".into()),
    }
    Ok(())
}

fn read_json(socket: &mut RpcSocket, maximum_bytes: usize) -> Result<Incoming> {
    let message = match socket.read() {
        Ok(message) => message,
        Err(WebSocketError::Io(error))
            if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
        {
            return Ok(Incoming::Timeout)
        }
        Err(error) => return Err(error.into()),
    };
    if message.len() > maximum_bytes {
        return Err("WebSocket message exceeds the configured raw byte bound".into());
    }
    match message {
        Message::Text(text) => {
            let bytes = text.len();
            let value: Value = serde_json::from_str(text.as_str())?;
            if !value.is_object() {
                return Err("WebSocket JSON-RPC message must be one object, never a batch".into());
            }
            Ok(Incoming::Json(value, bytes))
        }
        Message::Ping(_) | Message::Pong(_) => {
            socket.flush()?;
            Ok(Incoming::Control)
        }
        Message::Close(_) => Err("WebSocket peer closed the processed subscription stream".into()),
        Message::Binary(_) => Err("binary WebSocket messages are unsupported".into()),
        Message::Frame(_) => Err("raw WebSocket frames are unsupported".into()),
    }
}

fn response_id(value: &Value) -> Result<Option<u64>> {
    let id = value.get("id");
    let method = value.get("method");
    if id.is_some() == method.is_some() {
        return Err("JSON-RPC message must be exactly one response or notification".into());
    }
    match id {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| "JSON-RPC response id is not u64".into()),
        None => Ok(None),
    }
}

fn buffer_notification(
    buffered: &mut Vec<Value>,
    buffered_bytes: &mut usize,
    value: Value,
    bytes: usize,
    plan: &RpcIndexPlan,
) -> Result<()> {
    if buffered.len() >= plan.bounds.maximum_accounts_per_scan {
        return Err("processed bootstrap notification count exceeds maximumAccountsPerScan".into());
    }
    *buffered_bytes = buffered_bytes
        .checked_add(bytes)
        .ok_or("processed bootstrap notification byte count overflow")?;
    if *buffered_bytes > plan.bounds.maximum_total_response_bytes {
        return Err("processed bootstrap notifications exceed maximumTotalResponseBytes".into());
    }
    buffered.push(value);
    Ok(())
}

fn subscription_requests(engine: &Arc<RwLock<RpcIndexEngine>>) -> Result<Vec<PlannedRpcRequest>> {
    let guard = engine
        .read()
        .map_err(|_| "operator index read lock is unavailable")?;
    Ok(guard
        .unregistered_subscription_requests()
        .into_iter()
        .cloned()
        .collect())
}

fn admit_notification(
    engine: &Arc<RwLock<RpcIndexEngine>>,
    state: &SharedProcessedTransport,
    notification: &Value,
) -> Result<()> {
    // Keep the engine write guard until rollback counters are published. HTTP
    // readers sample transport state before and after their engine read, so
    // this lock order prevents a dead-branch mutation from escaping with its
    // prior rollback epoch.
    let mut engine = engine
        .write()
        .map_err(|_| "operator index write lock is unavailable")?;
    let events: Vec<RpcIndexEngineEvent> = engine.admit_notification(notification)?;
    update_state(state, |state| state.admit_events(&events))
}

fn withdraw_engine(
    engine: &Arc<RwLock<RpcIndexEngine>>,
) -> Result<clutch_local_real_pyth::index_service::ProcessedReconnectRollback> {
    Ok(engine
        .write()
        .map_err(|_| "operator index write lock is unavailable")?
        .begin_processed_reconnect())
}

fn update_state(
    state: &SharedProcessedTransport,
    update: impl FnOnce(&mut ProcessedTransportState) -> std::result::Result<(), &'static str>,
) -> Result<()> {
    update(
        &mut state
            .write()
            .map_err(|_| "processed transport state lock is unavailable")?,
    )
    .map_err(|error| -> Box<dyn std::error::Error> { error.into() })
}

fn require_not_idle(last_message: Instant, timeout: Duration) -> Result<()> {
    if last_message.elapsed() >= timeout {
        Err("processed WebSocket stream exceeded its idle timeout".into())
    } else {
        Ok(())
    }
}

fn wait_for_scan_then_error(
    receiver: mpsc::Receiver<std::result::Result<(), String>>,
    detail: &str,
) -> Result<()> {
    let _ignored = receiver.recv();
    Err(detail.to_string().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn response_discriminator_refuses_batches_and_ambiguous_envelopes() {
        assert_eq!(response_id(&json!({"id": 7})).unwrap(), Some(7));
        assert_eq!(
            response_id(&json!({"method": "rootNotification"})).unwrap(),
            None
        );
        assert!(response_id(&json!([])).is_err());
        assert!(response_id(&json!({})).is_err());
        assert!(response_id(&json!({"id": 7, "method": "rootNotification"})).is_err());
        assert!(response_id(&json!({"id": "7"})).is_err());
    }

    #[test]
    fn websocket_genesis_challenge_refuses_mismatch_error_and_notification() {
        let expected = "11111111111111111111111111111111";
        assert!(validate_genesis_response(
            &json!({"jsonrpc":"2.0", "id":GENESIS_CHALLENGE_REQUEST_ID, "result":expected}),
            expected
        )
        .is_ok());
        assert!(validate_genesis_response(
            &json!({"jsonrpc":"2.0", "id":GENESIS_CHALLENGE_REQUEST_ID, "result":"wrong"}),
            expected
        )
        .is_err());
        assert!(validate_genesis_response(
            &json!({"jsonrpc":"2.0", "id":GENESIS_CHALLENGE_REQUEST_ID, "error":{"code":-1}}),
            expected
        )
        .is_err());
        assert!(validate_genesis_response(
            &json!({"jsonrpc":"2.0", "method":"rootNotification"}),
            expected
        )
        .is_err());
    }

    #[test]
    fn public_transport_error_never_repeats_endpoint_path_or_query() {
        let endpoint = "wss://rpc.example/private/token?api-key=secret";
        let detail = redacted_error_detail(
            "connection to wss://rpc.example/private/token?api-key=secret failed",
            endpoint,
        );
        assert!(detail.contains("wss://rpc.example/<redacted>?<redacted>"));
        assert!(!detail.contains("private"));
        assert!(!detail.contains("token"));
        assert!(!detail.contains("secret"));
    }
}
