//! Construction-only RPC acquisition plans and hostile response decoding.
//!
//! This module has no HTTP or websocket client. It produces bounded request
//! bodies for an external transport and decodes responses only against the
//! exact cluster, program, and release that authorized the request.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;
use std::str::FromStr;

pub type Result<T> = core::result::Result<T, RpcIndexError>;

pub const RPC_ENDPOINT_BINDING_DOMAIN_V1: &[u8] = b"dragons-clutch/rpc-endpoint-binding/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcIndexError {
    InvalidCluster,
    InvalidRelease,
    DuplicateRelease,
    InvalidBound,
    MalformedResponse,
    WrongOwner,
    ResponseTooLarge,
    InvalidAccount,
    WrongRequest,
}

impl core::fmt::Display for RpcIndexError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCluster => "RPC index cluster binding is invalid",
            Self::InvalidRelease => "RPC index program release is invalid",
            Self::DuplicateRelease => "RPC index program release is duplicated",
            Self::InvalidBound => "RPC index acquisition bound is invalid",
            Self::MalformedResponse => "RPC index response is malformed",
            Self::WrongOwner => "RPC account owner differs from the requested release",
            Self::ResponseTooLarge => "RPC response exceeds its explicit acquisition bound",
            Self::InvalidAccount => "RPC response contains an invalid account",
            Self::WrongRequest => "RPC response was supplied to the wrong request plan",
        })
    }
}

impl std::error::Error for RpcIndexError {}

/// Semantic families a reviewed program release is allowed to own.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalFamily {
    Collateral,
    Fractional,
    General,
    Source,
    Series,
    Fees,
    Liveness,
    PositionV3,
    ReplayV3,
    StructuredClaim,
    Dealer,
    Failure,
    Direct,
}

impl CanonicalFamily {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Collateral => "collateral",
            Self::Fractional => "fractional",
            Self::General => "general",
            Self::Source => "source",
            Self::Series => "series",
            Self::Fees => "fees",
            Self::Liveness => "liveness",
            Self::PositionV3 => "position-v3",
            Self::ReplayV3 => "replay-v3",
            Self::StructuredClaim => "structured-claim",
            Self::Dealer => "dealer",
            Self::Failure => "failure",
            Self::Direct => "direct",
        }
    }
}

/// Exact network identity. URLs are transport coordinates, never cluster truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RpcClusterBinding {
    pub cluster_name: String,
    pub genesis_hash: String,
    pub rpc_http_url: String,
    pub rpc_websocket_url: String,
}

impl RpcClusterBinding {
    pub fn validate(&self) -> Result<()> {
        let local_validator = self.cluster_name == "local-validator";
        let local_endpoints = self.rpc_http_url.starts_with("http://127.0.0.1:")
            && self
                .rpc_websocket_url
                .starts_with("ws://127.0.0.1:");
        if self.cluster_name.trim().is_empty()
            || self.genesis_hash.len() < 32
            || self.genesis_hash.len() > 64
            || self.genesis_hash.chars().any(char::is_whitespace)
            || !safe_endpoint(&self.rpc_http_url, false)
            || !safe_endpoint(&self.rpc_websocket_url, true)
            || (local_validator && !local_endpoints)
        {
            return Err(RpcIndexError::InvalidCluster);
        }
        Ok(())
    }

    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.cluster_name, self.genesis_hash)
    }
}

fn safe_endpoint(url: &str, websocket: bool) -> bool {
    if url.is_empty()
        || url.len() > 2_048
        || url.contains('@')
        || url.contains('#')
        || url.chars().any(char::is_whitespace)
    {
        return false;
    }
    let public_prefix = if websocket { "wss://" } else { "https://" };
    let loopback_prefix = if websocket {
        "ws://127.0.0.1:"
    } else {
        "http://127.0.0.1:"
    };
    let public = url.strip_prefix(public_prefix).is_some_and(|remainder| {
        let authority_end = remainder
            .find(|character| matches!(character, '/' | '?'))
            .unwrap_or(remainder.len());
        authority_end > 0
    });
    public
        || url
            .strip_prefix(loopback_prefix)
            .is_some_and(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicRpcEndpointBinding {
    pub redacted: String,
    pub binding_sha256: [u8; 32],
}

/// Produce a stable exact-byte endpoint join without publishing a query value
/// or path token. Userinfo is rejected by `RpcClusterBinding::validate`; this
/// projection retains only the scheme/authority plus the presence of a hidden
/// path or query. The hash is domain-separated and covers the complete URL.
pub fn public_rpc_endpoint_binding(url: &str) -> PublicRpcEndpointBinding {
    let (scheme, remainder) = url.split_once("://").unwrap_or(("invalid", "invalid"));
    let authority_end = remainder
        .find(|character| matches!(character, '/' | '?'))
        .unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let suffix = &remainder[authority_end..];
    let (path, query) = suffix
        .split_once('?')
        .map_or((suffix, None), |(path, query)| (path, Some(query)));
    let redacted_path = match path {
        "" => "",
        "/" => "/",
        _ => "/<redacted>",
    };
    let redacted_query = query.map_or("", |_| "?<redacted>");
    let redacted = format!("{scheme}://{authority}{redacted_path}{redacted_query}");
    let mut hasher = Sha256::new();
    hasher.update(RPC_ENDPOINT_BINDING_DOMAIN_V1);
    hasher.update(url.as_bytes());
    PublicRpcEndpointBinding {
        redacted,
        binding_sha256: hasher.finalize().into(),
    }
}

/// One exact executable release and the account families it may own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedProgramRelease {
    pub program_id: Address,
    pub program_data: Address,
    pub elf_sha256: [u8; 32],
    pub deployment_slot: u64,
    /// Canonical digest of the checked capability-profile manifest that owns
    /// the semantic release description. This is not supplied by the browser.
    pub release_manifest_sha256: [u8; 32],
    pub capability_profile_id: [u8; 32],
    pub source_commit: String,
    /// Only centrally registered coordinates present in the checked manifest.
    /// A decoded family without a coordinate remains non-actionable.
    pub enabled_intents: Vec<CanonicalIntentCoordinate>,
    /// Checked payload-scoped capabilities whose coarse tuple stays disabled.
    pub enabled_intent_variants: Vec<CanonicalIntentVariantV1>,
    pub families: Vec<CanonicalFamily>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalIntentCoordinate {
    pub family_tag: u8,
    pub family_version: u8,
    pub local_action: u8,
}

/// One checked payload discriminator beneath an allocated intent coordinate.
///
/// This enum is closed so a browser cannot invent generic payload selectors.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalIntentVariantV1 {
    DealerRetireActiveFacilityCredit,
    DealerRetireUnusedFutureCredit,
}

impl CanonicalIntentVariantV1 {
    #[must_use]
    pub const fn coordinate(self) -> CanonicalIntentCoordinate {
        CanonicalIntentCoordinate {
            family_tag: clutch_solana_layout::registry::DEALER_FAMILY_TAG,
            family_version: clutch_solana_layout::registry::DEALER_FAMILY_VERSION,
            local_action: clutch_solana_layout::registry::DealerFacilityAction::Retire.tag(),
        }
    }

    #[must_use]
    pub const fn payload_discriminator(self) -> u8 {
        match self {
            Self::DealerRetireActiveFacilityCredit => 8,
            Self::DealerRetireUnusedFutureCredit => 9,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DealerRetireActiveFacilityCredit => "dealer-retire-active-facility-credit",
            Self::DealerRetireUnusedFutureCredit => "dealer-retire-unused-future-credit",
        }
    }
}

impl IndexedProgramRelease {
    pub fn validate(&self) -> Result<()> {
        if self.program_id == Address::default()
            || self.program_data == Address::default()
            || self.program_id == self.program_data
            || self.elf_sha256 == [0; 32]
            || self.release_manifest_sha256 == [0; 32]
            || self.capability_profile_id == [0; 32]
            || !matches!(self.source_commit.len(), 40 | 64)
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || self.source_commit.bytes().all(|byte| byte == b'0')
            || self.families.is_empty()
        {
            return Err(RpcIndexError::InvalidRelease);
        }
        let mut previous = None;
        for family in &self.families {
            if previous.is_some_and(|value| value >= *family) {
                return Err(RpcIndexError::InvalidRelease);
            }
            previous = Some(*family);
        }
        let mut previous_intent = None;
        for intent in &self.enabled_intents {
            if intent.family_tag == 0
                || intent.family_version == 0
                || previous_intent.is_some_and(|value| value >= *intent)
            {
                return Err(RpcIndexError::InvalidRelease);
            }
            previous_intent = Some(*intent);
        }
        let mut previous_variant = None;
        for variant in &self.enabled_intent_variants {
            if previous_variant.is_some_and(|value| value >= *variant)
                || self.enabled_intents.binary_search(&variant.coordinate()).is_ok()
            {
                return Err(RpcIndexError::InvalidRelease);
            }
            previous_variant = Some(*variant);
        }
        Ok(())
    }

    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.program_id,
            self.deployment_slot,
            hex(&self.elf_sha256),
            hex(&self.release_manifest_sha256)
        )
    }
}

/// Hard acquisition limits applied before any account reaches a decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RpcAcquisitionBounds {
    pub maximum_accounts_per_scan: usize,
    pub maximum_account_data_bytes: usize,
    pub maximum_total_response_bytes: usize,
    pub maximum_subscriptions: usize,
}

impl RpcAcquisitionBounds {
    pub fn validate(self) -> Result<()> {
        if self.maximum_accounts_per_scan == 0
            || self.maximum_accounts_per_scan > 65_536
            || self.maximum_account_data_bytes == 0
            || self.maximum_account_data_bytes > 1_048_576
            || self.maximum_total_response_bytes < self.maximum_account_data_bytes
            || self.maximum_total_response_bytes > 268_435_456
            || self.maximum_subscriptions == 0
            || self.maximum_subscriptions > 256
        {
            return Err(RpcIndexError::InvalidBound);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RpcCommitment {
    Processed,
    Finalized,
}

impl RpcCommitment {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Processed => "processed",
            Self::Finalized => "finalized",
        }
    }
}

/// Complete read-only index acquisition policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RpcIndexPlan {
    pub cluster: RpcClusterBinding,
    pub releases: Vec<IndexedProgramRelease>,
    pub bounds: RpcAcquisitionBounds,
}

impl RpcIndexPlan {
    pub fn validate(&self) -> Result<()> {
        self.cluster.validate()?;
        self.bounds.validate()?;
        if self.releases.is_empty() || self.releases.len() > self.bounds.maximum_subscriptions {
            return Err(RpcIndexError::InvalidBound);
        }
        let mut programs = BTreeSet::new();
        for release in &self.releases {
            release.validate()?;
            let slot_matches_cluster = if self.cluster.cluster_name == "local-validator" {
                release.deployment_slot == 0
            } else {
                release.deployment_slot != 0
            };
            if !slot_matches_cluster {
                return Err(RpcIndexError::InvalidRelease);
            }
            if !programs.insert(release.program_id) {
                return Err(RpcIndexError::DuplicateRelease);
            }
        }
        Ok(())
    }

    /// Construct one bounded finalized bootstrap scan per explicit release.
    pub fn finalized_scan_requests(&self) -> Result<Vec<PlannedRpcRequest>> {
        self.validate()?;
        Ok(self
            .releases
            .iter()
            .enumerate()
            .map(|(index, release)| PlannedRpcRequest {
                request_id: u64::try_from(index + 1).unwrap_or(u64::MAX),
                release_key: release.key(),
                program_id: release.program_id,
                commitment: RpcCommitment::Finalized,
                purpose: RpcRequestPurpose::ProgramScan,
                body: json!({
                    "jsonrpc": "2.0",
                    "id": index + 1,
                    "method": "getProgramAccounts",
                    "params": [release.program_id.to_string(), {
                        "commitment": "finalized",
                        "encoding": "base64",
                        "withContext": true
                    }]
                }),
            })
            .collect())
    }

    /// Construct processed account subscriptions plus slot/root topology feeds.
    pub fn subscription_requests(&self) -> Result<Vec<PlannedRpcRequest>> {
        self.validate()?;
        let required = self
            .releases
            .len()
            .checked_add(3)
            .ok_or(RpcIndexError::InvalidBound)?;
        if required > self.bounds.maximum_subscriptions {
            return Err(RpcIndexError::InvalidBound);
        }
        let mut output = Vec::with_capacity(required);
        for (index, release) in self.releases.iter().enumerate() {
            output.push(PlannedRpcRequest {
                request_id: u64::try_from(index + 1).unwrap_or(u64::MAX),
                release_key: release.key(),
                program_id: release.program_id,
                commitment: RpcCommitment::Processed,
                purpose: RpcRequestPurpose::ProgramSubscription,
                body: json!({
                    "jsonrpc": "2.0",
                    "id": index + 1,
                    "method": "programSubscribe",
                    "params": [release.program_id.to_string(), {
                        "commitment": "processed",
                        "encoding": "base64"
                    }]
                }),
            });
        }
        let base = self.releases.len() + 1;
        for (offset, purpose, method, params) in [
            (
                0usize,
                RpcRequestPurpose::BlockSubscription,
                "blockSubscribe",
                json!(["all", {
                    "commitment": "processed",
                    "encoding": "json",
                    "transactionDetails": "none",
                    "showRewards": false
                }]),
            ),
            (
                1usize,
                RpcRequestPurpose::SlotSubscription,
                "slotsUpdatesSubscribe",
                json!([]),
            ),
            (
                2usize,
                RpcRequestPurpose::RootSubscription,
                "rootSubscribe",
                json!([]),
            ),
        ] {
            output.push(PlannedRpcRequest {
                request_id: u64::try_from(base + offset).unwrap_or(u64::MAX),
                release_key: self.cluster.key(),
                program_id: Address::default(),
                commitment: RpcCommitment::Processed,
                purpose,
                body: json!({"jsonrpc": "2.0", "id": base + offset, "method": method, "params": params}),
            });
        }
        Ok(output)
    }

    pub fn release(&self, key: &str) -> Result<&IndexedProgramRelease> {
        self.releases
            .iter()
            .find(|release| release.key() == key)
            .ok_or(RpcIndexError::WrongRequest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcRequestPurpose {
    ProgramScan,
    ExactAccountSnapshot,
    ProgramSubscription,
    BlockSubscription,
    SlotSubscription,
    RootSubscription,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedRpcRequest {
    pub request_id: u64,
    pub release_key: String,
    pub program_id: Address,
    pub commitment: RpcCommitment,
    pub purpose: RpcRequestPurpose,
    pub body: Value,
}

/// Opaque bounded finalized `getMultipleAccounts` request. Its address set is
/// constructed only by a crate-owned semantic planner from hostile-decoded
/// chain identities; the transport can issue but cannot reshape it.
#[derive(Clone, Debug, PartialEq)]
pub struct FinalizedExactAccountSnapshotRequestV1 {
    request: PlannedRpcRequest,
    addresses: Vec<Address>,
}

/// Transport-decoded finalized context. Private fields prevent accidental
/// reshaping after the bounded decoder; this remains an untrusted offchain
/// observation and never replaces onchain reauthentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedSnapshotReceiptV1 {
    cluster_key: String,
    release_key: String,
    slot: u64,
    receipt_id: [u8; 32],
}

/// Opaque finalized account acquisition.  Current semantic planners consume
/// this value instead of caller-assembled `ObservedRpcAccount` vectors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAccountSnapshotV1 {
    receipt: FinalizedSnapshotReceiptV1,
    accounts: Vec<ObservedRpcAccount>,
}

impl FinalizedAccountSnapshotV1 {
    #[must_use]
    pub const fn receipt(&self) -> &FinalizedSnapshotReceiptV1 { &self.receipt }

    #[must_use]
    pub fn accounts(&self) -> &[ObservedRpcAccount] { &self.accounts }
}

impl FinalizedSnapshotReceiptV1 {
    #[must_use]
    pub fn cluster_key(&self) -> &str { &self.cluster_key }
    #[must_use]
    pub fn release_key(&self) -> &str { &self.release_key }
    #[must_use]
    pub const fn slot(&self) -> u64 { self.slot }
    #[must_use]
    pub const fn receipt_id(&self) -> [u8; 32] { self.receipt_id }
}

fn finalized_snapshot_receipt_v1(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    slot: u64,
    address_commitment: [u8; 32],
) -> Result<FinalizedSnapshotReceiptV1> {
    if request.commitment != RpcCommitment::Finalized
        || slot == 0
        || address_commitment == [0; 32]
    {
        return Err(RpcIndexError::WrongRequest);
    }
    let cluster_key = plan.cluster.key();
    let receipt_id = Sha256::new()
        .chain_update(b"dragons-clutch/operator/finalized-snapshot-receipt/v1\0")
        .chain_update(cluster_key.as_bytes())
        .chain_update(request.release_key.as_bytes())
        .chain_update(request.request_id.to_le_bytes())
        .chain_update(slot.to_le_bytes())
        .chain_update(address_commitment)
        .finalize()
        .into();
    Ok(FinalizedSnapshotReceiptV1 {
        cluster_key,
        release_key: request.release_key.clone(),
        slot,
        receipt_id,
    })
}

impl FinalizedExactAccountSnapshotRequestV1 {
    #[must_use]
    pub const fn request(&self) -> &PlannedRpcRequest { &self.request }

    #[must_use]
    pub fn addresses(&self) -> &[Address] { &self.addresses }
}

pub(crate) fn finalized_exact_account_snapshot_request_v1(
    plan: &RpcIndexPlan,
    release_key: &str,
    request_id: u64,
    minimum_context_slot: u64,
    mut addresses: Vec<Address>,
) -> Result<FinalizedExactAccountSnapshotRequestV1> {
    plan.validate()?;
    let release = plan.release(release_key)?;
    if request_id == 0
        || minimum_context_slot == 0
        || addresses.is_empty()
        || addresses.len() > plan.bounds.maximum_accounts_per_scan
    {
        return Err(RpcIndexError::InvalidBound);
    }
    addresses.sort();
    if addresses.iter().any(|address| *address == Address::default()) {
        return Err(RpcIndexError::InvalidAccount);
    }
    let width = addresses.len();
    addresses.dedup();
    if addresses.len() != width { return Err(RpcIndexError::InvalidAccount); }
    let request = PlannedRpcRequest {
        request_id,
        release_key: release.key(),
        program_id: release.program_id,
        commitment: RpcCommitment::Finalized,
        purpose: RpcRequestPurpose::ExactAccountSnapshot,
        body: json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "getMultipleAccounts",
            "params": [addresses.iter().map(ToString::to_string).collect::<Vec<_>>(), {
                "commitment": "finalized",
                "encoding": "base64",
                "minContextSlot": minimum_context_slot
            }]
        }),
    };
    Ok(FinalizedExactAccountSnapshotRequestV1 { request, addresses })
}

pub fn decode_response_result<'a>(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    response: &'a Value,
) -> Result<&'a Value> {
    require_bounded_json(plan, response)?;
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || response.get("id").and_then(Value::as_u64) != Some(request.request_id)
        || response.get("error").is_some_and(|error| !error.is_null())
    {
        return Err(RpcIndexError::WrongRequest);
    }
    response
        .get("result")
        .ok_or(RpcIndexError::MalformedResponse)
}

fn require_notification_envelope(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    notification: &Value,
) -> Result<()> {
    require_bounded_json(plan, notification)?;
    let method = match request.purpose {
        RpcRequestPurpose::ProgramSubscription => "programNotification",
        RpcRequestPurpose::BlockSubscription => "blockNotification",
        RpcRequestPurpose::SlotSubscription => "slotsUpdatesNotification",
        RpcRequestPurpose::RootSubscription => "rootNotification",
        RpcRequestPurpose::ProgramScan | RpcRequestPurpose::ExactAccountSnapshot => {
            return Err(RpcIndexError::WrongRequest)
        }
    };
    if notification.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || notification.get("method").and_then(Value::as_str) != Some(method)
    {
        return Err(RpcIndexError::WrongRequest);
    }
    notification_subscription_id(notification)?;
    Ok(())
}

/// Admit the server-assigned subscription coordinate for one exact planned
/// request. The engine retains the resulting binding for every notification.
pub fn decode_subscription_registration(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    response: &Value,
) -> Result<u64> {
    plan.validate()?;
    let result = decode_response_result(plan, request, response)?;
    match request.purpose {
        RpcRequestPurpose::ProgramSubscription => {
            let release = plan.release(&request.release_key)?;
            if request.commitment != RpcCommitment::Processed
                || request.program_id != release.program_id
            {
                return Err(RpcIndexError::WrongRequest);
            }
        }
        RpcRequestPurpose::BlockSubscription
        | RpcRequestPurpose::SlotSubscription
        | RpcRequestPurpose::RootSubscription => {
            require_topology_request(plan, request, request.purpose)?;
        }
        RpcRequestPurpose::ProgramScan | RpcRequestPurpose::ExactAccountSnapshot => {
            return Err(RpcIndexError::WrongRequest)
        }
    }
    result
        .as_u64()
        .filter(|subscription| *subscription > 0)
        .ok_or(RpcIndexError::MalformedResponse)
}

pub fn notification_subscription_id(notification: &Value) -> Result<u64> {
    notification
        .get("params")
        .and_then(|value| value.get("subscription"))
        .and_then(Value::as_u64)
        .filter(|subscription| *subscription > 0)
        .ok_or(RpcIndexError::MalformedResponse)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcObservationSource {
    FinalizedScan,
    /// Exact finalized address snapshot derived from decoded chain state.
    FinalizedExactAccountSnapshot { request_id: u64 },
    ProcessedSubscription { subscription_id: u64 },
}

/// Processed fork identity obtained from a block subscription. Slot alone is
/// never used as branch identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedSlot {
    pub cluster_key: String,
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub commitment: RpcCommitment,
    pub receive_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservedSlotUpdateKind {
    FirstShred,
    Completed,
    CreatedBank,
    Frozen,
    Dead,
    OptimisticConfirmation,
    Root,
}

/// Slot lifecycle evidence has no blockhash and therefore cannot identify a
/// fork by itself. It only refines branches learned from block notifications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedSlotUpdate {
    pub cluster_key: String,
    pub slot: u64,
    pub parent_slot: Option<u64>,
    pub kind: ObservedSlotUpdateKind,
    pub receive_sequence: u64,
}

fn require_topology_request(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    purpose: RpcRequestPurpose,
) -> Result<()> {
    plan.validate()?;
    if request.purpose != purpose
        || request.commitment != RpcCommitment::Processed
        || request.release_key != plan.cluster.key()
        || request.program_id != Address::default()
    {
        return Err(RpcIndexError::WrongRequest);
    }
    Ok(())
}

fn require_bounded_json(plan: &RpcIndexPlan, value: &Value) -> Result<()> {
    if serde_json::to_vec(value)
        .map_err(|_| RpcIndexError::MalformedResponse)?
        .len()
        > plan.bounds.maximum_total_response_bytes
    {
        Err(RpcIndexError::ResponseTooLarge)
    } else {
        Ok(())
    }
}

fn valid_blockhash(value: &str) -> bool {
    (32..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() && !matches!(byte, b'0' | b'O' | b'I' | b'l'))
}

pub fn decode_block_notification(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    notification: &Value,
    receive_sequence: u64,
) -> Result<ObservedSlot> {
    require_topology_request(plan, request, RpcRequestPurpose::BlockSubscription)?;
    require_notification_envelope(plan, request, notification)?;
    let value = notification
        .get("params")
        .and_then(|value| value.get("result"))
        .and_then(|value| value.get("value"))
        .ok_or(RpcIndexError::MalformedResponse)?;
    let slot = value
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let block = value.get("block").ok_or(RpcIndexError::MalformedResponse)?;
    let parent_slot = block
        .get("parentSlot")
        .and_then(Value::as_u64)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let blockhash = block
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let previous_blockhash = block
        .get("previousBlockhash")
        .and_then(Value::as_str)
        .ok_or(RpcIndexError::MalformedResponse)?;
    if slot == 0
        || parent_slot >= slot
        || !valid_blockhash(blockhash)
        || !valid_blockhash(previous_blockhash)
        || blockhash == previous_blockhash
    {
        return Err(RpcIndexError::MalformedResponse);
    }
    Ok(ObservedSlot {
        cluster_key: plan.cluster.key(),
        slot,
        parent_slot,
        blockhash: blockhash.to_string(),
        previous_blockhash: previous_blockhash.to_string(),
        commitment: RpcCommitment::Processed,
        receive_sequence,
    })
}

pub fn decode_slot_update_notification(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    notification: &Value,
    receive_sequence: u64,
) -> Result<ObservedSlotUpdate> {
    require_topology_request(plan, request, RpcRequestPurpose::SlotSubscription)?;
    require_notification_envelope(plan, request, notification)?;
    let result = notification
        .get("params")
        .and_then(|value| value.get("result"))
        .ok_or(RpcIndexError::MalformedResponse)?;
    let slot = result
        .get("slot")
        .and_then(Value::as_u64)
        .filter(|slot| *slot > 0)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let parent_slot = result.get("parent").and_then(Value::as_u64);
    if parent_slot.is_some_and(|parent| parent >= slot) {
        return Err(RpcIndexError::MalformedResponse);
    }
    let kind = match result.get("type").and_then(Value::as_str) {
        Some("firstShredReceived") => ObservedSlotUpdateKind::FirstShred,
        Some("completed") => ObservedSlotUpdateKind::Completed,
        Some("createdBank") => ObservedSlotUpdateKind::CreatedBank,
        Some("frozen") => ObservedSlotUpdateKind::Frozen,
        Some("dead") => ObservedSlotUpdateKind::Dead,
        Some("optimisticConfirmation") => ObservedSlotUpdateKind::OptimisticConfirmation,
        Some("root") => ObservedSlotUpdateKind::Root,
        _ => return Err(RpcIndexError::MalformedResponse),
    };
    Ok(ObservedSlotUpdate {
        cluster_key: plan.cluster.key(),
        slot,
        parent_slot,
        kind,
        receive_sequence,
    })
}

pub fn decode_root_notification(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    notification: &Value,
) -> Result<u64> {
    require_topology_request(plan, request, RpcRequestPurpose::RootSubscription)?;
    require_notification_envelope(plan, request, notification)?;
    notification
        .get("params")
        .and_then(|value| value.get("result"))
        .and_then(Value::as_u64)
        .filter(|slot| *slot > 0)
        .ok_or(RpcIndexError::MalformedResponse)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RpcObservationProvenance {
    pub cluster_key: String,
    pub release_key: String,
    pub slot: u64,
    pub commitment: RpcCommitment,
    pub source: RpcObservationSource,
    pub receive_sequence: u64,
}

/// Hostile wire account after bounded base64 decoding and exact owner check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRpcAccount {
    pub address: Address,
    pub owner: Address,
    pub lamports: u64,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub provenance: RpcObservationProvenance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcAccountRemovalKind {
    Closed,
    OwnerChanged,
}

impl RpcAccountRemovalKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::OwnerChanged => "owner-changed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRpcAccountRemoval {
    pub address: Address,
    pub observed_owner: Address,
    pub observed_lamports: u64,
    pub observed_executable: bool,
    pub observed_data_bytes: usize,
    pub kind: RpcAccountRemovalKind,
    pub provenance: RpcObservationProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservedRpcProgramUpdate {
    Present(ObservedRpcAccount),
    Removed(ObservedRpcAccountRemoval),
}

impl ObservedRpcProgramUpdate {
    #[must_use]
    pub const fn address(&self) -> Address {
        match self {
            Self::Present(account) => account.address,
            Self::Removed(removal) => removal.address,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> u64 {
        match self {
            Self::Present(account) => account.provenance.slot,
            Self::Removed(removal) => removal.provenance.slot,
        }
    }

    #[must_use]
    pub fn retained_data_bytes(&self) -> usize {
        match self {
            Self::Present(account) => account.data.len(),
            Self::Removed(_) => 0,
        }
    }
}

pub fn decode_program_scan_result(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    result: &Value,
    receive_sequence_start: u64,
) -> Result<Vec<ObservedRpcAccount>> {
    plan.validate()?;
    if request.purpose != RpcRequestPurpose::ProgramScan
        || request.commitment != RpcCommitment::Finalized
    {
        return Err(RpcIndexError::WrongRequest);
    }
    let release = plan.release(&request.release_key)?;
    if release.program_id != request.program_id {
        return Err(RpcIndexError::WrongRequest);
    }
    require_bounded_json(plan, result)?;
    let slot = result
        .get("context")
        .and_then(|value| value.get("slot"))
        .and_then(Value::as_u64)
        .filter(|slot| *slot > 0)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let values = result
        .get("value")
        .and_then(Value::as_array)
        .ok_or(RpcIndexError::MalformedResponse)?;
    if values.len() > plan.bounds.maximum_accounts_per_scan {
        return Err(RpcIndexError::ResponseTooLarge);
    }
    let mut total = 0usize;
    let mut output = Vec::with_capacity(values.len());
    let mut addresses = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let address = value
            .get("pubkey")
            .and_then(Value::as_str)
            .ok_or(RpcIndexError::MalformedResponse)?;
        let account = value
            .get("account")
            .ok_or(RpcIndexError::MalformedResponse)?;
        let data = decode_account_value(account, plan.bounds)?;
        if data.owner != release.program_id {
            return Err(RpcIndexError::WrongOwner);
        }
        total = total
            .checked_add(data.data.len())
            .ok_or(RpcIndexError::ResponseTooLarge)?;
        if total > plan.bounds.maximum_total_response_bytes {
            return Err(RpcIndexError::ResponseTooLarge);
        }
        let address = Address::from_str(address).map_err(|_| RpcIndexError::InvalidAccount)?;
        if !addresses.insert(address) {
            return Err(RpcIndexError::InvalidAccount);
        }
        output.push(ObservedRpcAccount {
            address,
            owner: data.owner,
            lamports: data.lamports,
            executable: data.executable,
            rent_epoch: data.rent_epoch,
            data: data.data,
            provenance: RpcObservationProvenance {
                cluster_key: plan.cluster.key(),
                release_key: request.release_key.clone(),
                slot,
                commitment: RpcCommitment::Finalized,
                source: RpcObservationSource::FinalizedScan,
                receive_sequence: receive_sequence_start
                    .checked_add(u64::try_from(index).map_err(|_| RpcIndexError::InvalidBound)?)
                    .ok_or(RpcIndexError::InvalidBound)?,
            },
        });
    }
    Ok(output)
}

/// Decode a finalized owner scan and retain its opaque bounded context receipt.
/// Existing index callers may keep using `decode_program_scan_result`; current
/// cross-owner semantic planners must retain this receipt.
pub fn decode_program_scan_snapshot_v1(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    result: &Value,
    receive_sequence_start: u64,
) -> Result<FinalizedAccountSnapshotV1> {
    let accounts = decode_program_scan_result(
        plan,
        request,
        result,
        receive_sequence_start,
    )?;
    let slot = program_scan_context_slot(result)?;
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/program-scan-addresses/v1\0");
    hash.update(
        u64::try_from(accounts.len())
            .map_err(|_| RpcIndexError::InvalidBound)?
            .to_le_bytes(),
    );
    for account in &accounts { hash.update(account.address.to_bytes()); }
    let receipt = finalized_snapshot_receipt_v1(plan, request, slot, hash.finalize().into())?;
    Ok(FinalizedAccountSnapshotV1 { receipt, accounts })
}

/// Decode the exact response to one opaque chain-derived address request.
/// Every returned row remains finalized, order-bound, and owner-unrestricted;
/// action-specific decoders enforce the required System/program owner later.
pub fn decode_finalized_exact_account_snapshot_v1(
    plan: &RpcIndexPlan,
    request: &FinalizedExactAccountSnapshotRequestV1,
    result: &Value,
    receive_sequence_start: u64,
) -> Result<FinalizedAccountSnapshotV1> {
    plan.validate()?;
    let wire = request.request();
    if wire.purpose != RpcRequestPurpose::ExactAccountSnapshot
        || wire.commitment != RpcCommitment::Finalized
        || wire.request_id == 0
        || plan.release(&wire.release_key)?.program_id != wire.program_id
    {
        return Err(RpcIndexError::WrongRequest);
    }
    require_bounded_json(plan, result)?;
    let slot = result
        .get("context")
        .and_then(|value| value.get("slot"))
        .and_then(Value::as_u64)
        .filter(|slot| *slot > 0)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let values = result
        .get("value")
        .and_then(Value::as_array)
        .ok_or(RpcIndexError::MalformedResponse)?;
    if values.len() != request.addresses.len() {
        return Err(RpcIndexError::MalformedResponse);
    }
    let mut total = 0usize;
    let mut output = Vec::with_capacity(values.len());
    for (index, (address, value)) in request.addresses.iter().zip(values).enumerate() {
        if value.is_null() { return Err(RpcIndexError::InvalidAccount); }
        let data = decode_account_value(value, plan.bounds)?;
        total = total
            .checked_add(data.data.len())
            .ok_or(RpcIndexError::ResponseTooLarge)?;
        if total > plan.bounds.maximum_total_response_bytes {
            return Err(RpcIndexError::ResponseTooLarge);
        }
        output.push(ObservedRpcAccount {
            address: *address,
            owner: data.owner,
            lamports: data.lamports,
            executable: data.executable,
            rent_epoch: data.rent_epoch,
            data: data.data,
            provenance: RpcObservationProvenance {
                cluster_key: plan.cluster.key(),
                release_key: wire.release_key.clone(),
                slot,
                commitment: RpcCommitment::Finalized,
                source: RpcObservationSource::FinalizedExactAccountSnapshot {
                    request_id: wire.request_id,
                },
                receive_sequence: receive_sequence_start
                    .checked_add(
                        u64::try_from(index).map_err(|_| RpcIndexError::InvalidBound)?,
                    )
                    .ok_or(RpcIndexError::InvalidBound)?,
            },
        });
    }
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/exact-snapshot-addresses/v1\0");
    hash.update(
        u64::try_from(request.addresses.len())
            .map_err(|_| RpcIndexError::InvalidBound)?
            .to_le_bytes(),
    );
    for address in &request.addresses { hash.update(address.to_bytes()); }
    let receipt = finalized_snapshot_receipt_v1(plan, wire, slot, hash.finalize().into())?;
    Ok(FinalizedAccountSnapshotV1 {
        receipt,
        accounts: output,
    })
}

pub fn program_scan_context_slot(result: &Value) -> Result<u64> {
    result
        .get("context")
        .and_then(|value| value.get("slot"))
        .and_then(Value::as_u64)
        .ok_or(RpcIndexError::MalformedResponse)
}

pub fn decode_program_notification(
    plan: &RpcIndexPlan,
    request: &PlannedRpcRequest,
    notification: &Value,
    receive_sequence: u64,
) -> Result<ObservedRpcProgramUpdate> {
    plan.validate()?;
    if request.purpose != RpcRequestPurpose::ProgramSubscription
        || request.commitment != RpcCommitment::Processed
    {
        return Err(RpcIndexError::WrongRequest);
    }
    require_notification_envelope(plan, request, notification)?;
    let release = plan.release(&request.release_key)?;
    if release.program_id != request.program_id {
        return Err(RpcIndexError::WrongRequest);
    }
    let params = notification
        .get("params")
        .ok_or(RpcIndexError::MalformedResponse)?;
    let subscription_id = params
        .get("subscription")
        .and_then(Value::as_u64)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let result = params
        .get("result")
        .ok_or(RpcIndexError::MalformedResponse)?;
    let slot = result
        .get("context")
        .and_then(|value| value.get("slot"))
        .and_then(Value::as_u64)
        .filter(|slot| *slot > 0)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let value = result
        .get("value")
        .ok_or(RpcIndexError::MalformedResponse)?;
    let address = value
        .get("pubkey")
        .and_then(Value::as_str)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let account = value
        .get("account")
        .ok_or(RpcIndexError::MalformedResponse)?;
    let data = decode_account_value(account, plan.bounds)?;
    let address = Address::from_str(address).map_err(|_| RpcIndexError::InvalidAccount)?;
    let provenance = RpcObservationProvenance {
        cluster_key: plan.cluster.key(),
        release_key: request.release_key.clone(),
        slot,
        commitment: RpcCommitment::Processed,
        source: RpcObservationSource::ProcessedSubscription { subscription_id },
        receive_sequence,
    };
    if let Some(kind) = classify_program_removal(&data, release)? {
        return Ok(ObservedRpcProgramUpdate::Removed(
            ObservedRpcAccountRemoval {
                address,
                observed_owner: data.owner,
                observed_lamports: data.lamports,
                observed_executable: data.executable,
                observed_data_bytes: data.data.len(),
                kind,
                provenance,
            },
        ));
    }
    Ok(ObservedRpcProgramUpdate::Present(ObservedRpcAccount {
        address,
        owner: data.owner,
        lamports: data.lamports,
        executable: data.executable,
        rent_epoch: data.rent_epoch,
        data: data.data,
        provenance,
    }))
}

struct DecodedAccountValue {
    owner: Address,
    lamports: u64,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
}

fn classify_program_removal(
    account: &DecodedAccountValue,
    release: &IndexedProgramRelease,
) -> Result<Option<RpcAccountRemovalKind>> {
    if account.lamports == 0 && !account.executable && account.data.is_empty() {
        Ok(Some(RpcAccountRemovalKind::Closed))
    } else if account.owner != release.program_id && !account.executable {
        Ok(Some(RpcAccountRemovalKind::OwnerChanged))
    } else if account.owner != release.program_id {
        Err(RpcIndexError::WrongOwner)
    } else {
        Ok(None)
    }
}

fn decode_account_value(
    value: &Value,
    bounds: RpcAcquisitionBounds,
) -> Result<DecodedAccountValue> {
    let owner = value
        .get("owner")
        .and_then(Value::as_str)
        .and_then(|value| Address::from_str(value).ok())
        .ok_or(RpcIndexError::MalformedResponse)?;
    let tuple = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(RpcIndexError::MalformedResponse)?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(RpcIndexError::MalformedResponse);
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or(RpcIndexError::MalformedResponse)?;
    let maximum_base64_bytes = bounds
        .maximum_account_data_bytes
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or(RpcIndexError::InvalidBound)?;
    if encoded.len() > maximum_base64_bytes {
        return Err(RpcIndexError::ResponseTooLarge);
    }
    let data = BASE64
        .decode(encoded)
        .map_err(|_| RpcIndexError::InvalidAccount)?;
    if data.len() > bounds.maximum_account_data_bytes {
        return Err(RpcIndexError::ResponseTooLarge);
    }
    Ok(DecodedAccountValue {
        owner,
        lamports: value
            .get("lamports")
            .and_then(Value::as_u64)
            .ok_or(RpcIndexError::MalformedResponse)?,
        executable: value
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or(RpcIndexError::MalformedResponse)?,
        rent_epoch: value
            .get("rentEpoch")
            .and_then(Value::as_u64)
            .ok_or(RpcIndexError::MalformedResponse)?,
        data,
    })
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> IndexedProgramRelease {
        IndexedProgramRelease {
            program_id: Address::new_from_array([0x31; 32]),
            program_data: Address::new_from_array([0x32; 32]),
            elf_sha256: [0x33; 32],
            deployment_slot: 1,
            release_manifest_sha256: [0x34; 32],
            capability_profile_id: [0x35; 32],
            source_commit: "36".repeat(20),
            enabled_intents: vec![],
            enabled_intent_variants: vec![],
            families: vec![CanonicalFamily::General],
        }
    }

    fn account(
        owner: Address,
        lamports: u64,
        executable: bool,
        data: &[u8],
    ) -> DecodedAccountValue {
        DecodedAccountValue {
            owner,
            lamports,
            executable,
            rent_epoch: 0,
            data: data.to_vec(),
        }
    }

    #[test]
    fn endpoint_binding_redacts_path_and_query_but_hashes_exact_bytes() {
        let first = public_rpc_endpoint_binding("wss://rpc.example/v2/secret?api-key=alpha");
        let second = public_rpc_endpoint_binding("wss://rpc.example/v2/secret?api-key=beta");
        assert_eq!(first.redacted, "wss://rpc.example/<redacted>?<redacted>");
        assert!(!first.redacted.contains("secret"));
        assert!(!first.redacted.contains("alpha"));
        assert_ne!(first.binding_sha256, second.binding_sha256);
    }

    #[test]
    fn release_identity_refuses_a_zero_source_commit() {
        let mut value = release();
        value.source_commit = "0".repeat(40);
        assert_eq!(value.validate(), Err(RpcIndexError::InvalidRelease));
    }

    #[test]
    fn dealer_retire_variants_are_payload_scoped_and_canonical() {
        let mut value = release();
        value.families = vec![CanonicalFamily::Dealer];
        value.enabled_intent_variants = vec![
            CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit,
            CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit,
        ];
        assert!(value.validate().is_ok());

        value.enabled_intents = vec![
            CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit.coordinate(),
        ];
        assert_eq!(value.validate(), Err(RpcIndexError::InvalidRelease));

        value.enabled_intents.clear();
        value.enabled_intent_variants.swap(0, 1);
        assert_eq!(value.validate(), Err(RpcIndexError::InvalidRelease));
    }

    #[test]
    fn deployment_slot_locus_is_cluster_specific() {
        let bounds = RpcAcquisitionBounds {
            maximum_accounts_per_scan: 1,
            maximum_account_data_bytes: 1,
            maximum_total_response_bytes: 1,
            maximum_subscriptions: 4,
        };
        let mut local_release = release();
        local_release.deployment_slot = 0;
        let mut plan = RpcIndexPlan {
            cluster: RpcClusterBinding {
                cluster_name: "local-validator".to_string(),
                genesis_hash: "11".repeat(16),
                rpc_http_url: "http://127.0.0.1:9137".to_string(),
                rpc_websocket_url: "ws://127.0.0.1:9138".to_string(),
            },
            releases: vec![local_release],
            bounds,
        };
        assert!(plan.validate().is_ok());
        plan.releases[0].deployment_slot = 1;
        assert_eq!(plan.validate(), Err(RpcIndexError::InvalidRelease));

        plan.cluster.cluster_name = "solana-devnet".to_string();
        plan.cluster.rpc_http_url = "https://api.devnet.solana.com".to_string();
        plan.cluster.rpc_websocket_url = "wss://api.devnet.solana.com/".to_string();
        assert!(plan.validate().is_ok());
        plan.releases[0].deployment_slot = 0;
        assert_eq!(plan.validate(), Err(RpcIndexError::InvalidRelease));
    }

    #[test]
    fn only_unambiguous_non_executable_removals_are_admitted() {
        let release = release();
        assert_eq!(
            classify_program_removal(&account(release.program_id, 0, false, &[]), &release),
            Ok(Some(RpcAccountRemovalKind::Closed))
        );
        assert_eq!(
            classify_program_removal(
                &account(Address::new_from_array([0x44; 32]), 7, false, &[1]),
                &release
            ),
            Ok(Some(RpcAccountRemovalKind::OwnerChanged))
        );
        assert_eq!(
            classify_program_removal(
                &account(Address::new_from_array([0x44; 32]), 7, true, &[1]),
                &release
            ),
            Err(RpcIndexError::WrongOwner)
        );
        assert_eq!(
            classify_program_removal(&account(release.program_id, 7, false, &[1]), &release),
            Ok(None)
        );
    }
}
