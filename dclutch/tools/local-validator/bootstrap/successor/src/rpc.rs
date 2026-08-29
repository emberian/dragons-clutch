use std::{
    net::IpAddr,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_versioned_message_operator::{Finality, Observation, ObservedAccount};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::{Transaction, VersionedTransaction},
};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, LOOPBACK_PACING, PacingV1},
    model::{AccountEvidence, TransactionEvidence},
    plan::{hex, pubkey},
};

const LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Heap frame the two founding routes request, and why it now does something.
///
/// A previous campaign requested 256 KiB here and **measured it to change
/// nothing**: the transaction carried the instruction and the runtime accepted
/// it, but the pre-V2 projected bootstrap still died out of memory at the same
/// point, because
/// `RequestHeapFrame` raises the region the runtime *grants* while the stock
/// `solana-program-entrypoint` allocator is constructed with the compile-time
/// constant `HEAP_LENGTH = 32 * 1024` and never asks how much it was given.
/// The request was withdrawn rather than left in place, because an instruction
/// that only looks like a fix costs compute and moves every measurement.
///
/// That is no longer the shape of the world. Trading now owns its entrypoint
/// and its allocator (`programs/dclutch-trading-sbf/src/entrypoint_adapter.rs`),
/// and `admit_heap_frame_v1` re-derives the grant from the instructions sysvar
/// the runtime itself serialized, applying agave's own
/// `sanitize_requested_heap_size`. Exactly two routes declare the extended
/// profile — `DCLTGMF3` and `DCLTPCB2` — and both now carry the instructions
/// sysvar in their frame so the adapter can find it. The Hot execution path is
/// deliberately **not** on that list and keeps the 32 KiB discipline, so this
/// constant is applied per transaction and never globally.
///
/// Chain-derived: agave's `MAX_HEAP_FRAME_BYTES`. The adapter refuses anything
/// outside `[32 KiB, 256 KiB]` or not a multiple of 1 KiB, which are the same
/// bounds `sanitize_requested_heap_size` enforces.
pub(crate) const FOUNDING_HEAP_FRAME_BYTES: u32 = 256 * 1024;

/// ComputeBudget program instruction discriminant for `RequestHeapFrame(u32)`.
const REQUEST_HEAP_FRAME_DISCRIMINANT: u8 = 1;

/// ComputeBudget program instruction discriminant for `SetComputeUnitLimit(u32)`.
const SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT: u8 = 2;

/// ComputeBudget discriminant for `SetComputeUnitPrice(u64)` (microlamports
/// per compute unit).
const SET_COMPUTE_UNIT_PRICE_DISCRIMINANT: u8 = 3;

/// The priority fee every campaign transaction now carries, in microlamports
/// per compute unit.
///
/// Measured on devnet 2026-08-28 against the pre-V2 founding wire: every small
/// transaction landed in seconds while the 1.4M-CU atomic founding was left
/// behind by leaders for a full blockhash lifetime, repeatedly, at priority zero —
/// block packing prefers paid-per-CU, and a zero-fee compute-heavy transaction
/// is the first thing a leader drops. 50,000 µlam/CU prices a 1.4M-CU
/// transaction's priority at 70,000 lamports (0.00007 SOL) — decisive on
/// devnet's shallow fee market, negligible in the wallet ledger.
const COMPUTE_UNIT_PRICE_MICROLAMPORTS: u64 = 50_000;

/// How many times a send rebuilds a dropped transaction with a fresh blockhash
/// before giving up. Each attempt gets a full blockhash lifetime, so this is a
/// bound on total patience against a genuinely wedged cluster, not a retry of a
/// transaction that failed on its merits (a failure is a confirmed status, not
/// a drop).
const REBUILD_ON_DROP_ATTEMPTS: u32 = 5;

/// How many times a durable legacy packet is polled for finalized history.
///
/// The blockhash's own last-valid height is the authority on whether the
/// packet can still land, and it is checked every pass; this bound only stops
/// an unresponsive endpoint from spinning forever. At 400ms a pass it is about
/// two minutes, which is far past finality on a 16-tick loopback slot.
const FINALIZED_POLL_ATTEMPTS: u32 = 300;

/// One exact signed versioned packet persisted before its first submission.
///
/// The last-valid height is liveness metadata returned beside the blockhash;
/// it never participates in protocol semantics.  The packet digest and
/// signature let a restarting exterior reject a changed journal before it
/// polls or submits anything.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SignedVersionedPacketV1 {
    pub(crate) signature: String,
    pub(crate) packet_base64: String,
    pub(crate) packet_sha256: String,
    pub(crate) last_valid_block_height: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RpcAccount {
    pub(crate) lamports: u64,
    pub(crate) owner: Pubkey,
    pub(crate) executable: bool,
    pub(crate) rent_epoch: u64,
    pub(crate) data: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcSuccessEnvelopeV1 {
    jsonrpc: String,
    id: u64,
    result: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorEnvelopeV1 {
    jsonrpc: String,
    id: u64,
    error: RpcErrorBodyV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorBodyV1 {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcContextValueV1<T> {
    context: RpcContextV1,
    value: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcContextV1 {
    slot: u64,
    #[serde(rename = "apiVersion", default)]
    api_version: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcAccountWireV1 {
    lamports: u64,
    owner: String,
    executable: bool,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
    data: [String; 2],
    space: u64,
}

#[derive(Clone, Copy)]
struct ExactJsonValueSeedV1;

impl<'de> DeserializeSeed<'de> for ExactJsonValueSeedV1 {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> core::result::Result<Self::Value, D::Error>
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
        formatter.write_str("one JSON value with no duplicate object keys")
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
            .ok_or_else(|| E::custom("JSON number was not finite"))
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
        ExactJsonValueSeedV1.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(ExactJsonValueSeedV1)? {
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
            let value = map.next_value_seed(ExactJsonValueSeedV1)?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

pub(crate) fn parse_json_without_duplicate_keys_v1(bytes: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| Error::new(format!("JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| Error::new(format!("JSON trailing bytes: {error}")))?;
    Ok(value)
}

fn parse_rpc_response_v1(method: &str, request_id: u64, bytes: &[u8]) -> Result<Value> {
    let body = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{method} {error}")))?;
    let object = body
        .as_object()
        .ok_or_else(|| Error::new(format!("{method} RPC response was not an object")))?;
    match (object.contains_key("result"), object.contains_key("error")) {
        (true, false) => {
            let envelope: RpcSuccessEnvelopeV1 = serde_json::from_value(body)
                .map_err(|error| Error::new(format!("{method} RPC response shape: {error}")))?;
            require_rpc_envelope_v1(method, request_id, &envelope.jsonrpc, envelope.id)?;
            Ok(envelope.result)
        }
        (false, true) => {
            let envelope: RpcErrorEnvelopeV1 = serde_json::from_value(body)
                .map_err(|error| Error::new(format!("{method} RPC error shape: {error}")))?;
            require_rpc_envelope_v1(method, request_id, &envelope.jsonrpc, envelope.id)?;
            let data = envelope
                .error
                .data
                .map(|value| format!(" data {value}"))
                .unwrap_or_default();
            Err(Error::new(format!(
                "{method} RPC error: code {} message {}{data}",
                envelope.error.code, envelope.error.message
            )))
        }
        _ => Err(Error::new(format!(
            "{method} RPC response must carry exactly one of result or error"
        ))),
    }
}

fn require_rpc_envelope_v1(
    method: &str,
    request_id: u64,
    jsonrpc: &str,
    response_id: u64,
) -> Result<()> {
    if jsonrpc != "2.0" || response_id != request_id {
        return Err(Error::new(format!(
            "{method} RPC response version or request ID differed"
        )));
    }
    Ok(())
}

/// Whether this connection is allowed to change anything on the cluster.
///
/// `ReadsOnly` is not a promise in a doc comment; it is enforced at
/// [`Rpc::call`], the single point every request in this tool passes through,
/// by an allowlist of read methods. That is the same shape
/// `tools/release/devnet-observe.sh` uses for the same reason: a preflight that
/// *cannot* write is worth more than one that intends not to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WritePolicyV1 {
    /// The default for every campaign that means to change the chain.
    Writes,
    /// Reads only. Any other method is a refusal, not a request.
    ReadsOnly,
}

/// Every JSON-RPC method this tool may issue under [`WritePolicyV1::ReadsOnly`].
///
/// Deliberately a literal list rather than a "does the name start with get"
/// rule: `getFeeForMessage` is a read and `requestAirdrop` is not, and neither
/// is decidable from the prefix.
const READ_METHODS: &[&str] = &[
    "getAccountInfo",
    "getBalance",
    "getBlockHeight",
    "getBlockTime",
    "getEpochInfo",
    "getFeeForMessage",
    "getGenesisHash",
    "getHealth",
    "getLatestBlockhash",
    "getMinimumBalanceForRentExemption",
    "getMultipleAccounts",
    "getRecentPrioritizationFees",
    "getSignatureStatuses",
    "getSignaturesForAddress",
    "getSlot",
    "getTransaction",
    "getVersion",
];

pub(crate) struct Rpc {
    url: Url,
    client: Client,
    request_id: u64,
    pacing: PacingV1,
    policy: WritePolicyV1,
    /// When the last request went out, so the next one can wait its turn.
    last_call: Option<Instant>,
    /// The finalized slot of this connection's newest confirmed transaction.
    ///
    /// Read-your-writes, made structural: a public endpoint load-balances
    /// across replicas, and a single-account read served by a replica still
    /// catching up can answer from BEFORE a transaction this same connection
    /// already confirmed as finalized. On loopback the floor is always
    /// already met and nothing changes; against devnet it turned a passed
    /// hostile-probe rollback check into a false accusation (measured
    /// 2026-08-28, the first driven devnet founding: the payer-balance
    /// equality read a stale replica and refused a rollback that had in fact
    /// rolled back). Every single-account read waits this floor out.
    read_floor: u64,
}

/// What one submit+confirm attempt found.
enum ConfirmOutcomeV1 {
    /// The transaction reached finalized history; its evidence is here.
    Confirmed(TransactionEvidence),
    /// The transaction can never land on these bytes (its blockhash expired
    /// with no status, or the confirm deadline passed). The caller rebuilds it
    /// with a fresh blockhash and submits again.
    Dropped,
}

/// The chain's sealing-time answer about a durable signature the campaign
/// never observed finalize -- [`Rpc::finalized_signed_packet`]'s `None`,
/// distinguished into the three facts an accounting marker must separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LateSignatureProbeV1 {
    /// A finalized status is served but the transaction metadata is not: the
    /// send LANDED at this slot, and its deterministic fee was charged.
    StatusWithoutMetadata { slot: u64 },
    /// Neither status nor metadata: purged history, or a send that never
    /// landed. The fee stays two-point.
    Unserved,
    /// The endpoint could not answer; nothing may be concluded.
    Refused,
}

/// One exact finalized transaction recovered without any submission attempt.
pub(crate) struct FinalizedSignedPacketV1 {
    pub(crate) evidence: TransactionEvidence,
    pub(crate) packet: Vec<u8>,
    /// Exact top-level program return data when the finalized transaction
    /// published it. Durable family callers authenticate this at the same
    /// boundary as the signed packet rather than accepting a log projection.
    ///
    /// A log line is a projection the validator is free to truncate; the
    /// `returnData` field is the datum the program actually set. A family ACK
    /// is commit-last evidence, so reading it from anywhere but here would let
    /// a truncated log silently weaken the strongest claim the caller makes.
    pub(crate) return_data: Option<FinalizedReturnDataV1>,
}

/// Canonical finalized transaction return-data projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedReturnDataV1 {
    /// Program that called `set_return_data` last.
    pub(crate) program: Pubkey,
    /// Canonically decoded base64 payload.
    pub(crate) data: Vec<u8>,
}

impl Rpc {
    pub(crate) fn connect(value: &str) -> Result<Self> {
        let url = validate_loopback_url(value)?;
        let mut rpc = Self::build(url, LOOPBACK_PACING, WritePolicyV1::Writes)?;
        let health = rpc.call("getHealth", &json!([]))?;
        if health != Value::String("ok".into()) {
            return Err(Error::new(format!("local RPC health refused: {health}")));
        }
        Ok(rpc)
    }

    /// Connect to an already-admitted cluster origin and prove which chain it is.
    ///
    /// The genesis check is not decoration. `ClusterOriginV1::parse` can only
    /// judge the spelling of a URL; this asks the cluster what it is and hands
    /// the answer back to the origin to accept or refuse. It runs here, at
    /// connect, so that no caller can reach a `send` on a chain nobody
    /// authenticated.
    pub(crate) fn connect_cluster(origin: &ClusterOriginV1, policy: WritePolicyV1) -> Result<Self> {
        let url = Url::parse(origin.url())
            .map_err(|error| Error::new(format!("cluster RPC URL: {error}")))?;
        let mut rpc = Self::build(url, origin.pacing(), policy)?;
        let health = rpc.call("getHealth", &json!([]))?;
        if health != Value::String("ok".into()) {
            return Err(Error::new(format!(
                "{} RPC health refused: {health}",
                origin.redacted_url()
            )));
        }
        let genesis = rpc
            .call("getGenesisHash", &json!([]))?
            .as_str()
            .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
            .to_owned();
        origin.authenticate_genesis(&genesis)?;
        Ok(rpc)
    }

    fn build(url: Url, pacing: PacingV1, policy: WritePolicyV1) -> Result<Self> {
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| Error::new(format!("build RPC client: {error}")))?;
        Ok(Self {
            url,
            client,
            request_id: 0,
            pacing,
            policy,
            last_call: None,
            read_floor: 0,
        })
    }

    pub(crate) fn url(&self) -> &str {
        self.url.as_str()
    }

    pub(crate) fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.call_with_transport_attempt_limit(method, params, 3)
    }

    /// Issue exactly one HTTP request, including across a transport-ambiguous
    /// failure.
    ///
    /// A durable exterior uses this only after the exact signed packet and its
    /// expected signature are fsynced in a Dispatching journal. Retrying at this
    /// layer would bypass that exterior's poll-only recovery state machine.
    pub(crate) fn call_once(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.call_with_transport_attempt_limit(method, params, 1)
    }

    /// Submit exactly one already-authenticated signed packet.
    ///
    /// The caller must have fsynced the packet and signature before entering
    /// this method. A transport error is intentionally not retried here: the
    /// durable exterior retains its Dispatching phase and may only resend these
    /// identical bytes on recovery.
    pub(crate) fn submit_signed_packet_once(
        &mut self,
        label: &str,
        packet: &[u8],
        expected_signature: Signature,
        skip_preflight: bool,
    ) -> Result<Signature> {
        let returned = self
            .call_once(
                "sendTransaction",
                &json!([BASE64.encode(packet), {
                    "encoding":"base64",
                    "skipPreflight":skip_preflight,
                    "preflightCommitment":"confirmed",
                    "maxRetries":8
                }]),
            )
            .map_err(|error| Error::new(format!("{label}: {error}")))?
            .as_str()
            .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
            .parse::<Signature>()
            .map_err(|error| Error::new(format!("transaction signature: {error}")))?;
        if returned != expected_signature {
            return Err(Error::new(format!(
                "{label}: sendTransaction returned {returned}, not durable signature {expected_signature}"
            )));
        }
        Ok(returned)
    }

    /// One late, single-shot question at campaign sealing: did the chain ever
    /// see this durable signature? Errors are ANSWERS here, not fatalities --
    /// the caller is writing an accounting marker for a submission it never
    /// observed finalize, and a refused endpoint must degrade the marker to
    /// "refused", never abort the seal. Returns the verdict and the finalized
    /// slot at which the chain was asked (0 when even that is unknowable).
    pub(crate) fn late_signature_probe_v1(
        &mut self,
        signature: Signature,
    ) -> (LateSignatureProbeV1, u64) {
        let result = match self.call(
            "getSignatureStatuses",
            &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
        ) {
            Ok(result) => result,
            Err(_) => return (LateSignatureProbeV1::Refused, 0),
        };
        let checked_at_slot = result
            .get("context")
            .and_then(|context| context.get("slot"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let Some(status) = result
            .get("value")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .filter(|value| !value.is_null())
        else {
            return (LateSignatureProbeV1::Unserved, checked_at_slot);
        };
        // Only a FINALIZED status proves the fee is settled history; a lesser
        // commitment at sealing time stays an unserved two-point unknown
        // rather than becoming a claim the chain could still roll back.
        if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
            return (LateSignatureProbeV1::Unserved, checked_at_slot);
        }
        match status.get("slot").and_then(Value::as_u64) {
            Some(slot) if slot > 0 => (
                LateSignatureProbeV1::StatusWithoutMetadata { slot },
                checked_at_slot,
            ),
            _ => (LateSignatureProbeV1::Refused, checked_at_slot),
        }
    }

    /// Poll one exact signature and, only when finalized, authenticate and
    /// return the chain's complete signed packet and transaction metadata.
    /// No key and no send method is reachable from this path.
    pub(crate) fn finalized_signed_packet(
        &mut self,
        label: &str,
        signature: Signature,
        expect_failure: bool,
    ) -> Result<Option<FinalizedSignedPacketV1>> {
        let result = self.call(
            "getSignatureStatuses",
            &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
        )?;
        let Some(status) = result
            .get("value")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .filter(|value| !value.is_null())
        else {
            return Ok(None);
        };
        if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
            return Ok(None);
        }
        let status_error = status.get("err").cloned().filter(|value| !value.is_null());
        if expect_failure != status_error.is_some() {
            return Err(Error::new(format!(
                "{label} finalized status contradicted expectation: {}",
                status.get("err").unwrap_or(&Value::Null)
            )));
        }
        let transaction = self.call(
            "getTransaction",
            &json!([signature.to_string(), {
                "encoding":"base64",
                "commitment":"finalized",
                "maxSupportedTransactionVersion":0
            }]),
        )?;
        if transaction.is_null() {
            return Ok(None);
        }
        let slot = transaction
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new(format!("{label} transaction omitted slot")))?;
        let meta = transaction
            .get("meta")
            .ok_or_else(|| Error::new(format!("{label} transaction omitted meta")))?;
        let meta_error = meta.get("err").cloned().filter(|value| !value.is_null());
        if meta_error != status_error {
            return Err(Error::new(format!(
                "{label} status and transaction errors differ"
            )));
        }
        let packet_base64 = transaction
            .get("transaction")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("{label} transaction omitted base64 packet")))?;
        let packet = BASE64
            .decode(packet_base64)
            .map_err(|error| Error::new(format!("{label} finalized packet base64: {error}")))?;
        if BASE64.encode(&packet) != packet_base64 {
            return Err(Error::new(format!(
                "{label} finalized packet was not canonical base64"
            )));
        }
        let decoded: VersionedTransaction = bincode::deserialize(&packet)
            .map_err(|error| Error::new(format!("{label} finalized packet: {error}")))?;
        decoded
            .verify_and_hash_message()
            .map_err(|error| Error::new(format!("{label} finalized packet signature: {error}")))?;
        if decoded.signatures.first() != Some(&signature)
            || bincode::serialize(&decoded)
                .map_err(|error| Error::new(format!("{label} packet reencode: {error}")))?
                != packet
        {
            return Err(Error::new(format!(
                "{label} finalized packet signature or canonical bytes changed"
            )));
        }
        let fee_lamports = u64_field(meta, "fee")?;
        let compute_units_consumed = meta.get("computeUnitsConsumed").and_then(Value::as_u64);
        let fee_only_balance_change = (|| {
            let pre = meta.get("preBalances")?.as_array()?;
            let post = meta.get("postBalances")?.as_array()?;
            if pre.len() != post.len() || pre.is_empty() {
                return None;
            }
            let payer_pre = pre.first()?.as_u64()?;
            let payer_post = post.first()?.as_u64()?;
            let others_unmoved = pre
                .iter()
                .zip(post.iter())
                .skip(1)
                .all(|(before, after)| before.as_u64() == after.as_u64());
            Some(payer_post.checked_add(fee_lamports)? == payer_pre && others_unmoved)
        })();
        let logs = meta
            .get("logMessages")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        // Absent and JSON null both mean the transaction published no return
        // data. Anything present must be the canonical `[base64, "base64"]`
        // pair; a noncanonical encoding is refused rather than coerced,
        // because the bytes it decodes to are what a family ACK is checked
        // against and a lenient decode would admit two spellings of one ACK.
        let return_data = match meta.get("returnData") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let program = value
                    .get("programId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::new(format!("{label} return data omitted programId")))?
                    .parse::<Pubkey>()
                    .map_err(|error| {
                        Error::new(format!("{label} return data programId: {error}"))
                    })?;
                let pair = value
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| Error::new(format!("{label} return data omitted data pair")))?;
                if pair.len() != 2 || pair.get(1).and_then(Value::as_str) != Some("base64") {
                    return Err(Error::new(format!(
                        "{label} return data encoding was not canonical base64"
                    )));
                }
                let encoded = pair
                    .first()
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        Error::new(format!("{label} return data payload was not a string"))
                    })?;
                let data = BASE64
                    .decode(encoded)
                    .map_err(|error| Error::new(format!("{label} return data base64: {error}")))?;
                if BASE64.encode(&data) != encoded {
                    return Err(Error::new(format!(
                        "{label} return data was not canonical base64"
                    )));
                }
                Some(FinalizedReturnDataV1 { program, data })
            }
        };
        self.read_floor = self.read_floor.max(slot);
        Ok(Some(FinalizedSignedPacketV1 {
            evidence: TransactionEvidence {
                label: label.into(),
                signature: signature.to_string(),
                slot,
                transaction_metadata_available: true,
                fee_lamports: Some(fee_lamports),
                fee_only_balance_change,
                compute_units_consumed,
                error: meta_error,
                logs,
            },
            packet,
            return_data,
        }))
    }

    fn call_with_transport_attempt_limit(
        &mut self,
        method: &str,
        params: &Value,
        transport_attempt_limit: u32,
    ) -> Result<Value> {
        if transport_attempt_limit == 0 {
            return Err(Error::new("RPC transport attempt limit must be positive"));
        }
        if self.policy == WritePolicyV1::ReadsOnly && !READ_METHODS.contains(&method) {
            return Err(Error::new(format!(
                "REFUSED: {method} is not a read method and this connection is read-only. A \
                 preflight that could write is not a preflight."
            )));
        }
        // SMOKE-0 friction 1: one busy process saturates a public endpoint's
        // whole per-IP budget, so this waits its turn rather than discovering
        // the budget as a 429 mid-ladder. Zero on loopback, where there is no
        // shared budget and the wait would only slow a local campaign down.
        if !self.pacing.minimum_call_interval.is_zero()
            && let Some(previous) = self.last_call
        {
            let elapsed = previous.elapsed();
            if elapsed < self.pacing.minimum_call_interval {
                thread::sleep(self.pacing.minimum_call_interval - elapsed);
            }
        }
        self.last_call = Some(Instant::now());
        self.request_id = self
            .request_id
            .checked_add(1)
            .ok_or_else(|| Error::new("RPC request ID overflow"))?;
        // A TRANSPORT failure against a loopback validator is retried a
        // bounded number of times before it kills a multi-minute campaign: a
        // local RPC can refuse one connection under snapshot or accept-queue
        // pressure while the validator is healthy (observed 2026-08-27, a
        // seven-minute founding dead at one getSignatureStatuses blip). Only
        // the failure to SEND is retried — an HTTP error status or an RPC
        // error object is an answer, and answers are never retried here.
        // Retrying sendTransaction is admissible for the same reason it is
        // safe on expiry: the bytes are already signed, so a duplicate lands
        // as the same signature and the chain deduplicates it.
        let mut attempt = 0_u32;
        let response = loop {
            let sent = self
                .client
                .post(self.url.clone())
                .json(&json!({
                    "jsonrpc": "2.0",
                    "id": self.request_id,
                    "method": method,
                    "params": params,
                }))
                .send();
            match sent {
                Ok(response) => break response,
                Err(error) => {
                    attempt = attempt.saturating_add(1);
                    if attempt >= transport_attempt_limit {
                        return Err(Error::new(format!("{method} transport: {error}")));
                    }
                    eprintln!(
                        "rpc: {method} transport failure (attempt {attempt} of {transport_attempt_limit}): {error}; \
                         retrying"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        };
        if !response.status().is_success() {
            return Err(Error::new(format!(
                "{method} returned HTTP {}",
                response.status()
            )));
        }
        let body = response
            .bytes()
            .map_err(|error| Error::new(format!("{method} response body: {error}")))?;
        parse_rpc_response_v1(method, self.request_id, &body)
    }

    pub(crate) fn account(&mut self, address: Pubkey) -> Result<Option<RpcAccount>> {
        // `minContextSlot` is this connection's own read floor: the node must
        // answer from a snapshot at or past the newest transaction this
        // connection confirmed, or say it cannot yet (-32016), which is
        // retried within the confirmation deadline rather than surfaced as a
        // stale answer. A node that never catches up still fails loudly.
        let deadline = Instant::now() + self.pacing.confirm_timeout;
        let value = loop {
            let result = self.call(
                "getAccountInfo",
                &json!([address.to_string(), {
                    "encoding":"base64",
                    "commitment":"finalized",
                    "minContextSlot": self.read_floor
                }]),
            );
            match result {
                Ok(value) => break value,
                Err(error) => {
                    let text = error.to_string();
                    let behind = text.contains("-32016")
                        || text.contains("Minimum context slot has not been reached");
                    if behind && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                    return Err(error);
                }
            }
        };
        parse_account_info_result_v1(value, self.read_floor)
    }

    pub(crate) fn required_account(&mut self, address: Pubkey, label: &str) -> Result<RpcAccount> {
        self.account(address)?
            .ok_or_else(|| Error::new(format!("missing {label} account {address}")))
    }

    pub(crate) fn finalized_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(u64, Vec<Option<RpcAccount>>)> {
        if addresses.is_empty() || addresses.len() > 100 {
            return Err(Error::new(
                "getMultipleAccounts requires one through 100 exact addresses",
            ));
        }
        let value = self.call(
            "getMultipleAccounts",
            &json!([addresses.iter().map(ToString::to_string).collect::<Vec<_>>(), {
                "encoding":"base64",
                "commitment":"finalized",
                "minContextSlot":minimum_slot
            }]),
        )?;
        parse_multiple_accounts_result_v1(value, addresses.len(), minimum_slot)
    }

    /// Reacquire one finalized account as an exact routing observation.
    ///
    /// Address lookup tables are transaction routing data, never protocol
    /// authority. The observation is still finalized and slot-pinned so the
    /// shared compiler can refuse a table extended in the observed slot.
    pub(crate) fn finalized_observed_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(Observation, Vec<ObservedAccount>)> {
        let (slot, accounts) = self.finalized_accounts(addresses, minimum_slot)?;
        let observation = Observation {
            slot,
            unix_timestamp: self.block_time(slot)?,
            finality: Finality::Finalized,
        };
        let mut observed = Vec::with_capacity(addresses.len());
        for (key, account) in addresses.iter().copied().zip(accounts) {
            let account = account
                .ok_or_else(|| Error::new(format!("finalized observation missing {key}")))?;
            observed.push(ObservedAccount {
                observation,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            });
        }
        Ok((observation, observed))
    }

    pub(crate) fn block_time(&mut self, slot: u64) -> Result<i64> {
        self.call("getBlockTime", &json!([slot]))?
            .as_i64()
            .ok_or_else(|| Error::new("getBlockTime result was not an integer"))
    }

    pub(crate) fn finalized_slot(&mut self) -> Result<u64> {
        self.call("getSlot", &json!([{"commitment":"finalized"}]))?
            .as_u64()
            .ok_or_else(|| Error::new("getSlot result was not a u64"))
    }

    pub(crate) fn minimum_balance(&mut self, data_len: usize) -> Result<u64> {
        self.call(
            "getMinimumBalanceForRentExemption",
            &json!([data_len, {"commitment":"finalized"}]),
        )?
        .as_u64()
        .ok_or_else(|| Error::new("rent minimum result was not a u64"))
    }

    pub(crate) fn airdrop(
        &mut self,
        label: &str,
        address: Pubkey,
        lamports: u64,
    ) -> Result<TransactionEvidence> {
        let signature = self
            .call("requestAirdrop", &json!([address.to_string(), lamports]))?
            .as_str()
            .ok_or_else(|| Error::new("requestAirdrop result was not a signature"))?
            .parse::<Signature>()
            .map_err(|error| Error::new(format!("airdrop signature: {error}")))?;
        self.confirm_airdrop(label, signature)
    }

    pub(crate) fn send(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<TransactionEvidence> {
        self.send_inner(label, instructions, payer, false)
    }

    /// Sign one exact legacy packet without submitting it.
    ///
    /// The v0 path above is the right one whenever a frame needs routing. A
    /// family whose whole frame already fits the 1,232-byte ceiling does not,
    /// and making it publish an address lookup table first would add an
    /// activation round trip and a table the campaign then has to own. The
    /// durability rule is identical and is the entire point of both: persist
    /// these exact bytes, then only ever resend them.
    pub(crate) fn prepare_signed_legacy_packet(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<SignedVersionedPacketV1> {
        let bounded = bounded_instructions(instructions, None)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
        let (blockhash, last_valid_block_height) = self.latest_blockhash_with_height()?;
        let signers: Vec<&dyn Signer> = vec![payer];
        let transaction = Transaction::new_signed_with_payer(
            &bounded,
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let packet = bincode::serialize(&transaction)
            .map_err(|error| Error::new(format!("{label}: serialize transaction: {error}")))?;
        if packet.len() > 1_232 {
            return Err(Error::new(format!(
                "{label}: legacy transaction is {} bytes, above the 1,232-byte packet ceiling; \
                 this frame needs v0 routing",
                packet.len()
            )));
        }
        let signature = transaction
            .signatures
            .first()
            .ok_or_else(|| Error::new(format!("{label}: signed packet omitted signature")))?
            .to_string();
        Ok(SignedVersionedPacketV1 {
            signature,
            packet_base64: BASE64.encode(&packet),
            packet_sha256: hex(&Sha256::digest(&packet)),
            last_valid_block_height,
        })
    }

    /// Submit the already-persisted legacy packet exactly once.
    pub(crate) fn submit_signed_legacy_packet(
        &mut self,
        label: &str,
        packet: &SignedVersionedPacketV1,
    ) -> Result<()> {
        let expected = Self::authenticate_signed_legacy_packet(label, packet)?;
        let observed = self.submit_encoded(label, &packet.packet_base64, false)?;
        if observed != expected {
            return Err(Error::new(format!(
                "{label}: RPC returned a signature other than the persisted packet signature"
            )));
        }
        Ok(())
    }

    /// Poll one submitted, persisted legacy signature through finalized history.
    ///
    /// Poll-only by construction, exactly like the v0 sibling: a `Submitted`
    /// journal can never fan out into a second transaction identity.
    pub(crate) fn confirm_signed_legacy_packet(
        &mut self,
        label: &str,
        packet: &SignedVersionedPacketV1,
    ) -> Result<FinalizedSignedPacketV1> {
        let signature = Self::authenticate_signed_legacy_packet(label, packet)?;
        for _ in 0..FINALIZED_POLL_ATTEMPTS {
            if let Some(finalized) = self.finalized_signed_packet(label, signature, false)? {
                return Ok(finalized);
            }
            let height = self.block_height()?;
            if height > packet.last_valid_block_height {
                return Err(Error::new(format!(
                    "{label}: persisted signature {signature} expired at block height {height} \
                     without a finalized status; retain the journal as evidence and prepare a new \
                     action under a new output path"
                )));
            }
            thread::sleep(Duration::from_millis(400));
        }
        Err(Error::new(format!(
            "{label}: persisted signature {signature} did not reach finalized history within \
             {FINALIZED_POLL_ATTEMPTS} polls"
        )))
    }

    /// Reauthenticate one persisted legacy packet against its own digest.
    pub(crate) fn authenticate_signed_legacy_packet(
        label: &str,
        packet: &SignedVersionedPacketV1,
    ) -> Result<Signature> {
        let bytes = BASE64
            .decode(&packet.packet_base64)
            .map_err(|error| Error::new(format!("{label}: persisted packet base64: {error}")))?;
        if BASE64.encode(&bytes) != packet.packet_base64
            || hex(&Sha256::digest(&bytes)) != packet.packet_sha256
        {
            return Err(Error::new(format!(
                "{label}: persisted packet digest changed"
            )));
        }
        let transaction: Transaction = bincode::deserialize(&bytes)
            .map_err(|error| Error::new(format!("{label}: persisted transaction: {error}")))?;
        transaction
            .verify()
            .map_err(|error| Error::new(format!("{label}: persisted signature: {error}")))?;
        let signature = transaction
            .signatures
            .first()
            .copied()
            .ok_or_else(|| Error::new(format!("{label}: persisted transaction is unsigned")))?;
        if signature.to_string() != packet.signature {
            return Err(Error::new(format!("{label}: persisted signature changed")));
        }
        Ok(signature)
    }

    /// Sign one exact routed v0 packet without submitting it.
    ///
    /// The caller must durably persist the returned value before invoking
    /// [`Self::submit_signed_v0_packet`]. Unlike [`Self::send`], this path
    /// never rebuilds a different signature after the persistence boundary.
    pub(crate) fn prepare_signed_v0_packet(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        table: &ObservedAccount,
    ) -> Result<SignedVersionedPacketV1> {
        self.prepare_signed_v0_packet_with_signers(label, instructions, payer, &[], table)
    }

    /// Sign one exact routed v0 packet with its complete signer set without
    /// submitting it. The additional signatures are covered by the same
    /// durable packet digest and are reauthenticated on every restart.
    pub(crate) fn prepare_signed_v0_packet_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        table: &ObservedAccount,
    ) -> Result<SignedVersionedPacketV1> {
        let bounded = bounded_instructions(instructions, None)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
        let (blockhash, last_valid_block_height) = self.latest_blockhash_with_height()?;
        let routed = dclutch_versioned_message_operator::compile_v0_message(
            payer.pubkey(),
            &bounded,
            solana_hash::Hash::new_from_array(blockhash.to_bytes()),
            table.observation,
            std::slice::from_ref(table),
        )
        .map_err(|error| Error::new(format!("{label}: v0 message: {error:?}")))?;
        if routed.wire_bytes > 1_232 {
            return Err(Error::new(format!(
                "{label}: routed transaction is {} bytes, above the 1,232-byte packet ceiling",
                routed.wire_bytes
            )));
        }
        let mut signers = Vec::with_capacity(additional_signers.len() + 1);
        signers.push(payer);
        signers.extend_from_slice(additional_signers);
        let transaction = VersionedTransaction::try_new(routed.message, &signers)
            .map_err(|error| Error::new(format!("{label}: sign v0 transaction: {error}")))?;
        let signature = transaction
            .signatures
            .first()
            .ok_or_else(|| Error::new(format!("{label}: signed packet omitted signature")))?
            .to_string();
        let packet = bincode::serialize(&transaction)
            .map_err(|error| Error::new(format!("{label}: serialize transaction: {error}")))?;
        Ok(SignedVersionedPacketV1 {
            signature,
            packet_base64: BASE64.encode(&packet),
            packet_sha256: hex(&Sha256::digest(&packet)),
            last_valid_block_height,
        })
    }

    /// Submit the already-persisted packet exactly once.
    ///
    /// A crash after this call but before the caller records `Submitted` is
    /// recovered by submitting the same bytes again: Solana deduplicates the
    /// identical signature. No fresh blockhash or signature is created here.
    pub(crate) fn submit_signed_v0_packet(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: Pubkey,
        table: &ObservedAccount,
        packet: &SignedVersionedPacketV1,
    ) -> Result<()> {
        Self::authenticate_signed_v0_packet(label, instructions, payer, table, packet)?;
        let observed = self.submit_encoded(label, &packet.packet_base64, false)?;
        if observed.to_string() != packet.signature {
            return Err(Error::new(format!(
                "{label}: RPC returned a signature other than the persisted packet signature"
            )));
        }
        Ok(())
    }

    /// Submit one persisted v0 packet that is EXPECTED to fail on chain.
    ///
    /// A hostile is only evidence if it reaches consensus. With preflight on,
    /// the RPC simulates the transaction and rejects it before it is ever a
    /// block entry, which proves what a simulator thinks rather than what the
    /// chain did. This skips preflight so the refusal is committed, paid for,
    /// and readable in finalized history like any other outcome.
    pub(crate) fn submit_signed_v0_packet_expecting_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: Pubkey,
        table: &ObservedAccount,
        packet: &SignedVersionedPacketV1,
    ) -> Result<()> {
        Self::authenticate_signed_v0_packet(label, instructions, payer, table, packet)?;
        let observed = self.submit_encoded(label, &packet.packet_base64, true)?;
        if observed.to_string() != packet.signature {
            return Err(Error::new(format!(
                "{label}: RPC returned a signature other than the persisted packet signature"
            )));
        }
        Ok(())
    }

    /// Poll one persisted v0 signature that is expected to have failed.
    ///
    /// Returns the finalized evidence with its error intact. A transaction
    /// that SUCCEEDED here is itself a refusal: a hostile that landed is the
    /// loudest possible failure of the property it was meant to defend.
    pub(crate) fn confirm_signed_v0_packet_expecting_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: Pubkey,
        table: &ObservedAccount,
        packet: &SignedVersionedPacketV1,
    ) -> Result<TransactionEvidence> {
        let signature =
            Self::authenticate_signed_v0_packet(label, instructions, payer, table, packet)?;
        for _ in 0..FINALIZED_POLL_ATTEMPTS {
            if let Some(finalized) = self.finalized_signed_packet(label, signature, true)? {
                return Ok(finalized.evidence);
            }
            let height = self.block_height()?;
            if height > packet.last_valid_block_height {
                return Err(Error::new(format!(
                    "{label}: hostile signature {signature} expired at block height {height} \
                     without a finalized status"
                )));
            }
            thread::sleep(Duration::from_millis(400));
        }
        Err(Error::new(format!(
            "{label}: hostile signature {signature} did not reach finalized history"
        )))
    }

    /// Poll one submitted, persisted signature through finalized history.
    ///
    /// This is deliberately poll-only. It neither sends the packet nor
    /// rebuilds it after blockhash expiry, so a `Submitted` journal can never
    /// fan out into a second transaction identity on restart.
    pub(crate) fn confirm_signed_v0_packet(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: Pubkey,
        table: &ObservedAccount,
        packet: &SignedVersionedPacketV1,
    ) -> Result<TransactionEvidence> {
        let signature =
            Self::authenticate_signed_v0_packet(label, instructions, payer, table, packet)?;
        match self.confirm_inner(
            label,
            signature,
            None,
            false,
            packet.last_valid_block_height,
        )? {
            ConfirmOutcomeV1::Confirmed(evidence) => Ok(evidence),
            ConfirmOutcomeV1::Dropped => Err(Error::new(format!(
                "{label}: persisted signature {} expired without a finalized status; retain the journal as evidence and prepare a new action under a new output path",
                packet.signature
            ))),
        }
    }

    pub(crate) fn authenticate_signed_v0_packet(
        label: &str,
        instructions: &[Instruction],
        payer: Pubkey,
        table: &ObservedAccount,
        packet: &SignedVersionedPacketV1,
    ) -> Result<Signature> {
        let bytes = BASE64
            .decode(&packet.packet_base64)
            .map_err(|error| Error::new(format!("{label}: persisted packet base64: {error}")))?;
        if BASE64.encode(&bytes) != packet.packet_base64
            || hex(&Sha256::digest(&bytes)) != packet.packet_sha256
        {
            return Err(Error::new(format!(
                "{label}: persisted packet digest changed"
            )));
        }
        let transaction: VersionedTransaction = bincode::deserialize(&bytes)
            .map_err(|error| Error::new(format!("{label}: persisted transaction: {error}")))?;
        transaction
            .verify_and_hash_message()
            .map_err(|error| Error::new(format!("{label}: persisted signature: {error}")))?;
        let signature = transaction
            .signatures
            .first()
            .copied()
            .ok_or_else(|| Error::new(format!("{label}: persisted transaction is unsigned")))?;
        if signature.to_string() != packet.signature {
            return Err(Error::new(format!("{label}: persisted signature changed")));
        }
        let bounded = bounded_instructions(instructions, None)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
        let expected = dclutch_versioned_message_operator::compile_v0_message(
            payer,
            &bounded,
            *transaction.message.recent_blockhash(),
            table.observation,
            std::slice::from_ref(table),
        )
        .map_err(|error| Error::new(format!("{label}: canonical v0 message: {error:?}")))?;
        if transaction.message != expected.message {
            return Err(Error::new(format!(
                "{label}: persisted transaction no longer matches the authenticated instruction"
            )));
        }
        Ok(signature)
    }

    pub(crate) fn send_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
    ) -> Result<TransactionEvidence> {
        self.send_inner_with_signers(label, instructions, payer, additional_signers, false)
    }

    pub(crate) fn send_expected_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<TransactionEvidence> {
        self.send_inner(label, instructions, payer, true)
    }

    /// Submit one packet-safe v0 transaction routed through finalized tables.
    ///
    /// The canonical Found and generic-founding frames exceed the 1,232-byte
    /// legacy packet with their account keys inline; the shared versioned
    /// message operator owns table admission and packet geometry.
    pub(crate) fn send_v0(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            &[],
            observation,
            tables,
            false,
            None,
        )
    }

    pub(crate) fn send_v0_expected_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            &[],
            observation,
            tables,
            true,
            None,
        )
    }

    /// Submit one routed v0 transaction carrying additional exact signers.
    ///
    /// A routed frame can still require a signature that is not the fee
    /// payer's: the projected-Custody abort needs the principal's owner to sign
    /// while remaining non-writable, which the fee payer cannot do.
    ///
    /// This path carries **no heap-frame request**, deliberately. Only the two
    /// founding routes declare the extended profile, and a route that does not
    /// need the grant does not ask for one.
    pub(crate) fn send_v0_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            false,
            None,
        )
    }

    /// Submit one routed v0 transaction expected to refuse, carrying the exact
    /// signatures its frame requires.
    ///
    /// A hostile case must differ from the honest one in exactly the coordinate
    /// under test. If it also drops a signature the frame needs, the
    /// transaction never reaches the chain and the refusal proves nothing.
    pub(crate) fn send_v0_expected_failure_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            true,
            None,
        )
    }

    /// Submit one routed v0 transaction on a runtime-granted extended heap.
    ///
    /// Only the two founding routes may use this: `DCLTGMF3` and `DCLTPCB2`
    /// are the exhaustive list in
    /// `entrypoint_adapter::declares_extended_heap_profile_v1`, and each
    /// presents the instructions sysvar in its own frame so the program can
    /// re-derive the grant. A route not on that list keeps the 32 KiB
    /// structural discipline and the instruction would be dead weight.
    pub(crate) fn send_v0_on_founding_heap_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            false,
            Some(FOUNDING_HEAP_FRAME_BYTES),
        )
    }

    /// Submit one routed v0 founding transaction expected to refuse.
    ///
    /// Carries the identical ComputeBudget declarations as the honest
    /// transaction. A hostile case that also withheld the heap frame would
    /// differ from the honest one in two coordinates, and its refusal would
    /// not be attributable to the one under test.
    pub(crate) fn send_v0_on_founding_heap_expected_failure_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            true,
            Some(FOUNDING_HEAP_FRAME_BYTES),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn send_v0_inner(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
        expect_failure: bool,
        heap_frame_bytes: Option<u32>,
    ) -> Result<TransactionEvidence> {
        let bounded = bounded_instructions(instructions, heap_frame_bytes)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(additional_signers.len() + 1);
        signers.push(payer);
        signers.extend(
            additional_signers
                .iter()
                .map(|signer| *signer as &dyn Signer),
        );
        for attempt in 0..REBUILD_ON_DROP_ATTEMPTS {
            let (blockhash, last_valid) = self.latest_blockhash_with_height()?;
            let plan = dclutch_versioned_message_operator::compile_v0_message(
                payer.pubkey(),
                &bounded,
                solana_hash::Hash::new_from_array(blockhash.to_bytes()),
                observation,
                tables,
            )
            .map_err(|error| Error::new(format!("{label}: v0 message compilation: {error:?}")))?;
            let transaction = VersionedTransaction::try_new(plan.message, &signers)
                .map_err(|error| Error::new(format!("{label}: sign v0 transaction: {error}")))?;
            let (signature, encoded) = self.submit(label, &transaction, expect_failure)?;
            match self.confirm(label, signature, &encoded, expect_failure, last_valid)? {
                ConfirmOutcomeV1::Confirmed(evidence) => return Ok(evidence),
                ConfirmOutcomeV1::Dropped => {
                    eprintln!(
                        "rpc: {label} dropped (blockhash expired, attempt {} of {}); \
                         rebuilding with a fresh blockhash",
                        attempt + 1,
                        REBUILD_ON_DROP_ATTEMPTS
                    );
                }
            }
        }
        Err(Error::new(format!(
            "{label} was dropped {REBUILD_ON_DROP_ATTEMPTS} times running; the cluster is not \
             landing this transaction within a blockhash lifetime"
        )))
    }

    fn submit<T: serde::Serialize>(
        &mut self,
        label: &str,
        transaction: &T,
        expect_failure: bool,
    ) -> Result<(Signature, String)> {
        let encoded = BASE64.encode(
            bincode::serialize(transaction)
                .map_err(|error| Error::new(format!("serialize transaction: {error}")))?,
        );
        let signature = self.submit_encoded(label, &encoded, expect_failure)?;
        Ok((signature, encoded))
    }

    /// Send one already-encoded transaction. Idempotent by signature: the same
    /// signed bytes resubmitted after a devnet drop land as the same signature
    /// and the chain deduplicates, which is what makes [`confirm`]'s resubmit
    /// loop safe.
    fn submit_encoded(
        &mut self,
        label: &str,
        encoded: &str,
        expect_failure: bool,
    ) -> Result<Signature> {
        self.call(
            "sendTransaction",
            &json!([encoded, {
                "encoding":"base64",
                "skipPreflight": expect_failure,
                "preflightCommitment":"confirmed",
                "maxRetries": 8
            }]),
        )
        .map_err(|error| Error::new(format!("{label}: {error}")))?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("transaction signature: {error}")))
    }

    fn send_inner(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        self.send_inner_with_signers(label, instructions, payer, &[], expect_failure)
    }

    fn send_inner_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        if additional_signers
            .iter()
            .any(|signer| signer.pubkey() == payer.pubkey())
        {
            return Err(Error::new("transaction signer list duplicated its payer"));
        }
        for (index, signer) in additional_signers.iter().enumerate() {
            if additional_signers
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other.pubkey() == signer.pubkey())
            {
                return Err(Error::new("transaction signer list contained duplicates"));
            }
        }
        let bounded_instructions = bounded_instructions(instructions, None)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(additional_signers.len() + 1);
        signers.push(payer);
        signers.extend(
            additional_signers
                .iter()
                .map(|signer| *signer as &dyn Signer),
        );
        for attempt in 0..REBUILD_ON_DROP_ATTEMPTS {
            let (blockhash, last_valid) = self.latest_blockhash_with_height()?;
            let transaction = Transaction::new_signed_with_payer(
                &bounded_instructions,
                Some(&payer.pubkey()),
                &signers,
                blockhash,
            );
            let (signature, encoded) = self.submit(label, &transaction, expect_failure)?;
            match self.confirm(label, signature, &encoded, expect_failure, last_valid)? {
                ConfirmOutcomeV1::Confirmed(evidence) => return Ok(evidence),
                ConfirmOutcomeV1::Dropped => {
                    eprintln!(
                        "rpc: {label} dropped (blockhash expired, attempt {} of {}); \
                         rebuilding with a fresh blockhash",
                        attempt + 1,
                        REBUILD_ON_DROP_ATTEMPTS
                    );
                }
            }
        }
        Err(Error::new(format!(
            "{label} was dropped {REBUILD_ON_DROP_ATTEMPTS} times running; the cluster is not \
             landing this transaction within a blockhash lifetime"
        )))
    }

    /// A recent blockhash and the last block height at which it is still valid.
    ///
    /// The height is what lets [`confirm`] tell a transaction that is merely
    /// slow to land from one that is definitively dropped: once the chain's
    /// block height passes `last_valid_block_height` with no status, no
    /// validator will ever accept those bytes again, and the only recovery is
    /// to rebuild with a fresh blockhash. `finalized` commitment (rather than
    /// the usual `confirmed`) buys the longest validity window, which is what a
    /// paced, sequential founding wants.
    fn latest_blockhash_with_height(&mut self) -> Result<(Hash, u64)> {
        let value = self.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
        let inner = value
            .get("value")
            .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
        let hash = inner
            .get("blockhash")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
            .parse::<Hash>()
            .map_err(|error| Error::new(format!("blockhash: {error}")))?;
        let last_valid = inner
            .get("lastValidBlockHeight")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
        Ok((hash, last_valid))
    }

    fn block_height(&mut self) -> Result<u64> {
        self.call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
            .as_u64()
            .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))
    }

    fn confirm(
        &mut self,
        label: &str,
        signature: Signature,
        encoded: &str,
        expect_failure: bool,
        last_valid_block_height: u64,
    ) -> Result<ConfirmOutcomeV1> {
        self.confirm_inner(
            label,
            signature,
            Some(encoded),
            expect_failure,
            last_valid_block_height,
        )
    }

    fn confirm_inner(
        &mut self,
        label: &str,
        signature: Signature,
        resubmit_packet: Option<&str>,
        expect_failure: bool,
        last_valid_block_height: u64,
    ) -> Result<ConfirmOutcomeV1> {
        // A DEADLINE, not an iteration count. The count was equivalent while
        // every connection polled an unpaced loopback validator at 100 ms; on a
        // paced connection each iteration also waits out the call interval, so
        // a fixed count silently becomes a different — and unstated — amount of
        // patience. Loopback keeps its 60 seconds exactly; devnet gets the five
        // minutes its profile names.
        let deadline = Instant::now() + self.pacing.confirm_timeout;
        let submitted_at = Instant::now();
        let mut status = None;
        // Devnet drops transactions: a valid blockhash can expire before the
        // transaction lands, and then no status ever appears (measured
        // 2026-08-28, a founding died at a dropped hostile probe after the full
        // 300 s). Resubmitting the SAME signed bytes is idempotent by
        // signature, so the loop re-sends periodically until a status appears
        // or the deadline passes. Loopback never needs this and its interval is
        // long enough never to fire inside the 60 s local budget.
        let mut next_resubmit = Instant::now() + self.pacing.resubmit_interval;
        while Instant::now() < deadline {
            if Instant::now() >= next_resubmit && resubmit_packet.is_some() {
                // Resubmit against a transient drop while the blockhash is
                // still valid. A resubmit failure is not fatal: the status
                // poll below is the authority on whether it landed, and the
                // block-height check is the authority on whether it ever can.
                let _ = self.submit_encoded(
                    label,
                    resubmit_packet.ok_or_else(|| Error::new("resubmit packet disappeared"))?,
                    expect_failure,
                );
                next_resubmit = Instant::now() + self.pacing.resubmit_interval;
            }
            let result = self.call(
                "getSignatureStatuses",
                &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
            )?;
            status = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|value| !value.is_null())
                .cloned();
            if status
                .as_ref()
                .and_then(|value| value.get("confirmationStatus"))
                == Some(&Value::String("finalized".into()))
            {
                break;
            }
            if status.is_none() {
                // Definitive-drop check: once the chain's block height passes
                // the blockhash's last valid height with still no status, these
                // bytes can never land — resubmitting them is futile and the
                // caller must rebuild with a fresh blockhash.
                //
                // TWO guards against a false verdict, both measured on devnet
                // 2026-08-28: the height comparison alone declared "expired"
                // ~20 seconds after a finalized blockhash whose real margin was
                // 148 blocks (~60 s), because a load-balanced endpoint served
                // the blockhash from one replica and the height from a fresher
                // one. So expiry is only believed when (a) at least a full
                // blockhash lifetime of WALL CLOCK — this process's own clock,
                // no replica's — has passed since submit, and (b) the height
                // exceeds the bound by a finalization-depth margin.
                let aged = submitted_at.elapsed() >= Duration::from_secs(75);
                if aged && self.block_height()? > last_valid_block_height.saturating_add(32) {
                    return Ok(ConfirmOutcomeV1::Dropped);
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        let Some(status) = status else {
            // Timed out without a status and without provable expiry: treat as
            // a drop so the caller rebuilds, rather than failing a founding for
            // a transaction that simply never landed.
            return Ok(ConfirmOutcomeV1::Dropped);
        };
        let status_error = status.get("err").cloned().filter(|value| !value.is_null());
        if expect_failure != status_error.is_some() {
            return Err(Error::new(format!(
                "{label} status contradicted expectation: {}",
                status.get("err").unwrap_or(&Value::Null)
            )));
        }
        let deadline = Instant::now() + self.pacing.confirm_timeout;
        let mut transaction = None;
        while Instant::now() < deadline {
            let candidate = self.call(
                "getTransaction",
                &json!([signature.to_string(), {
                    "encoding":"json",
                    "commitment":"finalized",
                    "maxSupportedTransactionVersion":0
                }]),
            )?;
            if candidate.get("meta").is_some() {
                transaction = Some(candidate);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let transaction = transaction.ok_or_else(|| {
            Error::new(format!(
                "{label} {signature} did not reach finalized transaction history"
            ))
        })?;
        let meta = transaction
            .get("meta")
            .ok_or_else(|| Error::new(format!("{label} transaction omitted meta")))?;
        let meta_error = meta.get("err").cloned().filter(|value| !value.is_null());
        if meta_error != status_error {
            return Err(Error::new(format!(
                "{label} status and transaction errors differ"
            )));
        }
        let slot = transaction
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new(format!("{label} transaction omitted slot")))?;
        let fee_lamports = u64_field(meta, "fee")?;
        let compute_units_consumed = meta.get("computeUnitsConsumed").and_then(Value::as_u64);
        let fee_only_balance_change = (|| {
            let pre = meta.get("preBalances")?.as_array()?;
            let post = meta.get("postBalances")?.as_array()?;
            if pre.len() != post.len() || pre.is_empty() {
                return None;
            }
            let payer_pre = pre.first()?.as_u64()?;
            let payer_post = post.first()?.as_u64()?;
            let others_unmoved = pre
                .iter()
                .zip(post.iter())
                .skip(1)
                .all(|(before, after)| before.as_u64() == after.as_u64());
            Some(payer_post.checked_add(fee_lamports)? == payer_pre && others_unmoved)
        })();
        self.read_floor = self.read_floor.max(slot);
        eprintln!(
            "campaign transaction: slot={slot} fee={fee_lamports} compute_units={} {label}",
            compute_units_consumed
                .map(|units| units.to_string())
                .unwrap_or_else(|| "unavailable".into())
        );
        let logs = meta
            .get("logMessages")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Ok(ConfirmOutcomeV1::Confirmed(TransactionEvidence {
            label: label.into(),
            signature: signature.to_string(),
            slot,
            transaction_metadata_available: true,
            fee_lamports: Some(fee_lamports),
            fee_only_balance_change,
            compute_units_consumed,
            error: meta_error,
            logs,
        }))
    }

    fn confirm_airdrop(
        &mut self,
        label: &str,
        signature: Signature,
    ) -> Result<TransactionEvidence> {
        let mut status = None;
        for _ in 0..120 {
            let result = self.call(
                "getSignatureStatuses",
                &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
            )?;
            status = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|value| !value.is_null())
                .cloned();
            if status
                .as_ref()
                .and_then(|value| value.get("confirmationStatus"))
                == Some(&Value::String("finalized".into()))
            {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let status = status.ok_or_else(|| {
            Error::new(format!(
                "{label} {signature} did not reach a visible status"
            ))
        })?;
        if let Some(error) = status.get("err").filter(|value| !value.is_null()) {
            return Err(Error::new(format!("{label} airdrop failed: {error}")));
        }
        Ok(TransactionEvidence {
            label: label.into(),
            signature: signature.to_string(),
            slot: u64_field(&status, "slot")?,
            transaction_metadata_available: false,
            fee_lamports: None,
            fee_only_balance_change: None,
            compute_units_consumed: None,
            error: None,
            logs: Vec::new(),
        })
    }
}

/// Prepend this campaign's ComputeBudget declarations to one instruction list.
///
/// Every transaction carries the compute-unit limit. A transaction whose route
/// declares the extended heap profile additionally carries `RequestHeapFrame`;
/// see [`FOUNDING_HEAP_FRAME_BYTES`] for why that is no longer inert.
///
/// **The prepend is a refusal surface, not a formatting step.** A signature
/// precompile (ed25519, secp256k1, secp256r1) carries the *instruction index*
/// of the instruction whose data it verifies, inside its own payload, so
/// inserting anything ahead of one silently re-points it at a different
/// instruction. This campaign builds no such transaction today, and this
/// refusal is what keeps that true: a precompile appearing in a bounded list
/// is a defect to fix at the call site, never something to prepend past.
pub(crate) fn bounded_instructions(
    instructions: &[Instruction],
    heap_frame_bytes: Option<u32>,
) -> Result<Vec<Instruction>> {
    for instruction in instructions {
        if instruction.program_id == solana_sdk_ids::ed25519_program::ID
            || instruction.program_id == solana_sdk_ids::secp256k1_program::ID
            || instruction.program_id == solana_sdk_ids::secp256r1_program::ID
        {
            return Err(Error::new(
                "a signature precompile carries instruction indices in its payload and cannot be prepended past",
            ));
        }
        if instruction.program_id == solana_sdk_ids::compute_budget::ID {
            return Err(Error::new(
                "the ComputeBudget declarations are owned by bounded_instructions; a duplicate is a transaction error",
            ));
        }
    }
    let mut bounded = Vec::with_capacity(instructions.len().saturating_add(3));
    let mut compute_limit_data = Vec::with_capacity(5);
    compute_limit_data.push(SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT);
    compute_limit_data.extend_from_slice(&LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT.to_le_bytes());
    bounded.push(Instruction {
        program_id: solana_sdk_ids::compute_budget::ID,
        accounts: Vec::new(),
        data: compute_limit_data,
    });
    let mut compute_price_data = Vec::with_capacity(9);
    compute_price_data.push(SET_COMPUTE_UNIT_PRICE_DISCRIMINANT);
    compute_price_data.extend_from_slice(&COMPUTE_UNIT_PRICE_MICROLAMPORTS.to_le_bytes());
    bounded.push(Instruction {
        program_id: solana_sdk_ids::compute_budget::ID,
        accounts: Vec::new(),
        data: compute_price_data,
    });
    if let Some(bytes) = heap_frame_bytes {
        let mut heap_frame_data = Vec::with_capacity(5);
        heap_frame_data.push(REQUEST_HEAP_FRAME_DISCRIMINANT);
        heap_frame_data.extend_from_slice(&bytes.to_le_bytes());
        bounded.push(Instruction {
            program_id: solana_sdk_ids::compute_budget::ID,
            accounts: Vec::new(),
            data: heap_frame_data,
        });
    }
    bounded.extend_from_slice(instructions);
    Ok(bounded)
}

fn parse_account_info_result_v1(value: Value, minimum_slot: u64) -> Result<Option<RpcAccount>> {
    let result: RpcContextValueV1<Option<RpcAccountWireV1>> = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("getAccountInfo result shape: {error}")))?;
    require_rpc_context_v1("getAccountInfo", &result.context, minimum_slot)?;
    result.value.map(parse_account_wire_v1).transpose()
}

fn parse_multiple_accounts_result_v1(
    value: Value,
    expected_width: usize,
    minimum_slot: u64,
) -> Result<(u64, Vec<Option<RpcAccount>>)> {
    let result: RpcContextValueV1<Vec<Option<RpcAccountWireV1>>> = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("getMultipleAccounts result shape: {error}")))?;
    require_rpc_context_v1("getMultipleAccounts", &result.context, minimum_slot)?;
    if result.value.len() != expected_width {
        return Err(Error::new(
            "getMultipleAccounts response width differed from request",
        ));
    }
    let accounts = result
        .value
        .into_iter()
        .map(|account| account.map(parse_account_wire_v1).transpose())
        .collect::<Result<Vec<_>>>()?;
    Ok((result.context.slot, accounts))
}

fn require_rpc_context_v1(method: &str, context: &RpcContextV1, minimum_slot: u64) -> Result<()> {
    if context.slot < minimum_slot {
        return Err(Error::new(format!(
            "{method} returned a snapshot before the required transaction"
        )));
    }
    if context.api_version.as_deref().is_some_and(str::is_empty) {
        return Err(Error::new(format!("{method} returned an empty apiVersion")));
    }
    Ok(())
}

fn parse_account_wire_v1(value: RpcAccountWireV1) -> Result<RpcAccount> {
    if value.data[1] != "base64" {
        return Err(Error::new(
            "account data must be the exact [base64, \"base64\"] tuple",
        ));
    }
    let data = BASE64
        .decode(&value.data[0])
        .map_err(|error| Error::new(format!("account base64: {error}")))?;
    if BASE64.encode(&data) != value.data[0] || u64::try_from(data.len()).ok() != Some(value.space)
    {
        return Err(Error::new("account base64 or declared space was not exact"));
    }
    Ok(RpcAccount {
        lamports: value.lamports,
        owner: pubkey(&value.owner)?,
        executable: value.executable,
        rent_epoch: value.rent_epoch,
        data,
    })
}

pub(crate) fn account_evidence(address: Pubkey, account: &RpcAccount) -> AccountEvidence {
    let data_sha256 = Sha256::digest(&account.data);
    let mut exact = Sha256::new();
    exact.update(account.owner.as_ref());
    exact.update(account.lamports.to_le_bytes());
    exact.update([u8::from(account.executable)]);
    exact.update(account.rent_epoch.to_le_bytes());
    exact.update(
        u64::try_from(account.data.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    exact.update(&account.data);
    AccountEvidence {
        address: address.to_string(),
        owner: account.owner.to_string(),
        lamports: account.lamports,
        executable: account.executable,
        data_len: account.data.len(),
        data_sha256: hex(&data_sha256),
        account_sha256: hex(&exact.finalize()),
    }
}

pub(crate) fn validate_loopback_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|error| Error::new(format!("RPC URL: {error}")))?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port().is_none()
    {
        return Err(Error::new(
            "RPC URL must be a credential-free explicit-port loopback HTTP origin",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::new("RPC URL omitted host"))?;
    let normalized = host.trim_start_matches('[').trim_end_matches(']');
    if !normalized.eq_ignore_ascii_case("localhost")
        && !normalized
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    {
        return Err(Error::new("RPC URL host is not loopback"));
    }
    Ok(url)
}

fn u64_field(value: &Value, name: &str) -> Result<u64> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("JSON omitted u64 {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read as _,
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    fn account_value_v1() -> Value {
        json!({
            "lamports": 9_u64,
            "owner": solana_sdk_ids::system_program::ID.to_string(),
            "executable": false,
            "rentEpoch": 4_u64,
            "data": ["AQID", "base64"],
            "space": 3_u64
        })
    }

    fn account_result_v1(account: Value) -> Value {
        json!({
            "context": {"slot": 17_u64, "apiVersion": "2.2.7"},
            "value": account
        })
    }

    #[test]
    fn persisted_v0_packet_binds_signature_bytes_payer_table_and_instruction() {
        use std::borrow::Cow;

        use solana_address_lookup_table_interface::{
            program as lookup_table_program,
            state::{AddressLookupTable, LookupTableMeta},
        };

        let payer = Keypair::new();
        let destination = Pubkey::new_unique();
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![solana_sdk::instruction::AccountMeta::new(
                destination,
                false,
            )],
            data: vec![1, 2, 3],
        };
        let observation = Observation {
            slot: 20,
            unix_timestamp: 30,
            finality: Finality::Finalized,
        };
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                deactivation_slot: u64::MAX,
                last_extended_slot: 19,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(vec![destination]),
        };
        let table = ObservedAccount {
            observation,
            key: Pubkey::new_unique(),
            owner: lookup_table_program::ID,
            lamports: 1,
            executable: false,
            data: table.serialize_for_tests().expect("table bytes"),
        };
        let bounded = bounded_instructions(std::slice::from_ref(&instruction), None)
            .expect("bounded instruction");
        let routed = dclutch_versioned_message_operator::compile_v0_message(
            payer.pubkey(),
            &bounded,
            Hash::new_unique(),
            observation,
            std::slice::from_ref(&table),
        )
        .expect("v0 message");
        let transaction = VersionedTransaction::try_new(routed.message, &[&payer])
            .expect("signed v0 transaction");
        let packet_bytes = bincode::serialize(&transaction).expect("packet bytes");
        let packet = SignedVersionedPacketV1 {
            signature: transaction.signatures[0].to_string(),
            packet_base64: BASE64.encode(&packet_bytes),
            packet_sha256: hex(&Sha256::digest(&packet_bytes)),
            last_valid_block_height: 99,
        };
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&instruction),
                payer.pubkey(),
                &table,
                &packet,
            )
            .is_ok()
        );

        let mut changed_digest = packet.clone();
        changed_digest.packet_sha256 = "00".repeat(32);
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&instruction),
                payer.pubkey(),
                &table,
                &changed_digest,
            )
            .is_err()
        );
        let mut changed_signature = packet.clone();
        changed_signature.signature = Signature::default().to_string();
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&instruction),
                payer.pubkey(),
                &table,
                &changed_signature,
            )
            .is_err()
        );
        let mut changed_instruction = instruction.clone();
        changed_instruction.data.push(4);
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&changed_instruction),
                payer.pubkey(),
                &table,
                &packet,
            )
            .is_err()
        );
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&instruction),
                Pubkey::new_unique(),
                &table,
                &packet,
            )
            .is_err()
        );
        let mut changed_table = table.clone();
        changed_table.key = Pubkey::new_unique();
        assert!(
            Rpc::authenticate_signed_v0_packet(
                "test",
                std::slice::from_ref(&instruction),
                payer.pubkey(),
                &changed_table,
                &packet,
            )
            .is_err()
        );
    }

    #[test]
    fn rpc_json_refuses_duplicate_keys_at_every_depth_before_value_normalization() {
        for bytes in [
            br#"{"jsonrpc":"2.0","id":7,"id":8,"result":null}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":7,"result":{"context":{"slot":17,"slot":18}}}"#,
            br#"{"jsonrpc":"2.0","id":7,"result":[{"owner":"a","owner":"b"}]}"#,
        ] {
            let error = parse_rpc_response_v1("test", 7, bytes)
                .expect_err("duplicate JSON object key unexpectedly normalized")
                .to_string();
            assert!(error.contains("duplicate JSON object key"), "{error}");
        }
    }

    #[test]
    fn rpc_envelope_is_exact_versioned_and_request_bound() {
        let valid = br#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#;
        assert_eq!(
            parse_rpc_response_v1("test", 7, valid).expect("exact response"),
            json!({"ok": true})
        );
        for bytes in [
            br#"{"jsonrpc":"2.0","id":7,"result":null,"extra":0}"#.as_slice(),
            br#"{"jsonrpc":"1.0","id":7,"result":null}"#,
            br#"{"jsonrpc":"2.0","id":8,"result":null}"#,
            br#"{"jsonrpc":"2.0","id":7,"result":null,"error":{"code":-1,"message":"x"}}"#,
            br#"{"jsonrpc":"2.0","id":7}"#,
            br#"[]"#,
        ] {
            assert!(
                parse_rpc_response_v1("test", 7, bytes).is_err(),
                "hostile RPC envelope unexpectedly accepted: {}",
                String::from_utf8_lossy(bytes)
            );
        }
        let error = parse_rpc_response_v1(
            "test",
            7,
            br#"{"jsonrpc":"2.0","id":7,"error":{"code":-32016,"message":"Minimum context slot has not been reached","data":{"slot":16}}}"#,
        )
        .expect_err("RPC error unexpectedly became a result")
        .to_string();
        assert!(error.contains("-32016"), "{error}");
        assert!(
            error.contains("Minimum context slot has not been reached"),
            "{error}"
        );
    }

    #[test]
    fn account_result_requires_exact_shape_base64_tuple_space_and_slot_floor() {
        let account = parse_account_info_result_v1(account_result_v1(account_value_v1()), 17)
            .expect("exact account result")
            .expect("present account");
        assert_eq!(account.lamports, 9);
        assert_eq!(account.rent_epoch, 4);
        assert_eq!(account.data, [1, 2, 3]);

        let mut cases = Vec::new();
        let mut unknown_account = account_value_v1();
        unknown_account["unknown"] = json!(true);
        cases.push(account_result_v1(unknown_account));
        let mut unknown_context = account_result_v1(account_value_v1());
        unknown_context["context"]["unknown"] = json!(true);
        cases.push(unknown_context);
        let mut unknown_result = account_result_v1(account_value_v1());
        unknown_result["unknown"] = json!(true);
        cases.push(unknown_result);
        for data in [
            json!(["AQID"]),
            json!(["AQID", "base64", "extra"]),
            json!(["AQID", "base58"]),
            json!([1, "base64"]),
            json!(["AQID", 1]),
        ] {
            let mut value = account_value_v1();
            value["data"] = data;
            cases.push(account_result_v1(value));
        }
        let mut wrong_space = account_value_v1();
        wrong_space["space"] = json!(4_u64);
        cases.push(account_result_v1(wrong_space));
        let mut wrong_width = account_value_v1();
        wrong_width["lamports"] = json!(-1_i64);
        cases.push(account_result_v1(wrong_width));
        let mut wrong_encoding = account_value_v1();
        wrong_encoding["data"] = json!(["AQI", "base64"]);
        wrong_encoding["space"] = json!(2_u64);
        cases.push(account_result_v1(wrong_encoding));
        for value in cases {
            assert!(
                parse_account_info_result_v1(value, 17).is_err(),
                "hostile account result unexpectedly accepted"
            );
        }

        assert!(
            parse_account_info_result_v1(account_result_v1(account_value_v1()), 18).is_err(),
            "context slot below the requested floor unexpectedly accepted"
        );
        assert!(
            parse_account_info_result_v1(json!({"context":{"slot":17_u64},"value":null}), 17,)
                .expect("exact absent account")
                .is_none()
        );
    }

    #[test]
    fn multiple_accounts_preserves_width_nulls_and_the_same_exact_account_parser() {
        let (slot, accounts) = parse_multiple_accounts_result_v1(
            json!({
                "context": {"slot": 19_u64},
                "value": [account_value_v1(), null]
            }),
            2,
            19,
        )
        .expect("exact multiple-account result");
        assert_eq!(slot, 19);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].as_ref().expect("present").data, [1, 2, 3]);
        assert!(accounts[1].is_none());
        assert!(
            parse_multiple_accounts_result_v1(
                json!({"context":{"slot":19_u64},"value":[account_value_v1()]}),
                2,
                19,
            )
            .is_err(),
            "wrong response width unexpectedly accepted"
        );
    }

    #[test]
    fn only_explicit_loopback_origins_are_admitted() {
        for value in ["http://127.0.0.1:20890/", "http://[::1]:20890/"] {
            assert!(validate_loopback_url(value).is_ok(), "{value}");
        }
        for value in [
            "https://127.0.0.1:20890/",
            "http://127.0.0.1/",
            "http://127.0.0.1:20890/path",
            "http://example.com:20890/",
            "http://user@127.0.0.1:20890/",
        ] {
            assert!(validate_loopback_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn one_shot_transport_ambiguity_never_retries_the_http_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind hostile RPC");
        listener
            .set_nonblocking(true)
            .expect("nonblocking hostile RPC");
        let address = listener.local_addr().expect("hostile RPC address");
        let accepted = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&accepted);
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        observed.fetch_add(1, Ordering::SeqCst);
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .expect("hostile stream timeout");
                        let mut request = [0_u8; 4096];
                        let _ = stream.read(&mut request);
                        // Drop without an HTTP response. The request may have
                        // reached a validator, so retrying would be ambiguous.
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("hostile RPC accept: {error}"),
                }
            }
        });
        let url = Url::parse(&format!("http://{address}/")).expect("hostile RPC URL");
        let mut rpc =
            Rpc::build(url, LOOPBACK_PACING, WritePolicyV1::Writes).expect("hostile RPC client");
        assert!(
            rpc.call_once("sendTransaction", &json!(["packet"]))
                .is_err(),
            "connection close without a response unexpectedly succeeded"
        );
        server.join().expect("hostile RPC server");
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }
}
