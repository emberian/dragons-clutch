//! Shared read-only General V5 successor-plan production.
//!
//! The route document is deliberately untrusted routing input: account
//! addresses, requested privileges, the payer, one lookup table, and checked
//! manifest identities. It carries no widths, seed recipes, request fields, or
//! artifact bodies. Every named account is reacquired in one finalized
//! `getMultipleAccounts` response; the General operator then hostile-decodes
//! the selected artifacts and chain state, derives the request and lifecycle,
//! and compiles the exact unsigned v0 message. This command never reads a
//! keypair, signs, simulates, submits, or calls a write RPC method.

use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_program_contract::hot_v3::{
    HOT_MARKET_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_effect_kernel::v2::FixedRole;
use dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactSelectionV3;
use dclutch_general_codec::Action;
use dclutch_operator::general_hot_v3::{
    CheckedGeneralHotReleaseV3, GENERAL_HOT_HEAP_FRAME_BYTES_V3, GeneralHotArtifactDigestsV3,
    GeneralHotStateV3, GeneralObservedAccountMetaV3, GeneralSuccessorInstructionV5,
    GeneralSuccessorTransactionPlanV0, build_general_successor_instruction_v5,
    canonical_general_lookup_addresses_v3, compile_general_successor_v0,
    general_artifact_bytes_from_hot_state_v3,
};
use dclutch_versioned_message_operator::{Finality, ObservedAccount};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_sdk::{
    hash::Hash, message::VersionedMessage, pubkey::Pubkey, signature::Signature,
    transaction::VersionedTransaction,
};

/// Shared producer refusal with no endpoint or key-bearing context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    /// Name one exact producer refusal.
    pub fn new(message: impl Into<String>) -> Self {
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
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

/// Shared producer result.
pub type Result<T> = core::result::Result<T, Error>;

/// Stable command name used by both first-party host binaries.
pub const COMMAND_V1: &str = "general-successor-plan-v5";
/// The exact `format` string every General successor route document carries.
///
/// Public so a route PRODUCER states the format this parser checks rather than
/// a copy of it: the one thing a producer and a parser must never disagree
/// about is the name of the agreement.
pub const ROUTE_FORMAT_V1: &str = "dclutch/general-successor-route/v1";
const PLAN_FORMAT_V5: &str = "dclutch/general-successor-plan/v5";
const MAX_ROUTE_BYTES_V1: usize = 256 * 1024;
/// Exact UTF-8 JSON bound shared by the producer and both hostile client twins.
pub const GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5: usize = 65_536;
const MAX_SNAPSHOT_ACCOUNTS_V1: usize = 100;

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

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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

fn parse_json_without_duplicate_keys_v1(bytes: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| Error::new(format!("JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| Error::new(format!("JSON trailing bytes: {error}")))?;
    Ok(value)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteMetaWireV1 {
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckedReleaseWireV1 {
    trading_program: String,
    trading_artifact_release: String,
    general_artifact_release: String,
    checked_manifest_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArtifactSelectionWireV1 {
    program_set: String,
    config: String,
    artifact_release: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneralSuccessorRouteWireV1 {
    format: String,
    action: String,
    minimum_finalized_slot: String,
    payer: String,
    lookup_table: String,
    release_set: String,
    generation: String,
    checked_release: CheckedReleaseWireV1,
    artifact_selection: ArtifactSelectionWireV1,
    fixed_accounts: Vec<RouteMetaWireV1>,
    strategy_accounts: Vec<RouteMetaWireV1>,
    runtime_suffix_accounts: Vec<RouteMetaWireV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RouteMetaV1 {
    address: Pubkey,
    is_signer: bool,
    is_writable: bool,
}

/// An exact, hostile-decoded General successor route.
///
/// Its fields remain private so callers cannot construct or mutate routing
/// authority around the parser's validation. Host binaries may inspect only
/// the minimum snapshot floor and the complete set of addresses they must
/// reacquire atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSuccessorRouteV1 {
    action: Action,
    minimum_finalized_slot: u64,
    payer: Pubkey,
    lookup_table: Pubkey,
    release_set: [u8; 32],
    generation: u64,
    checked_release: CheckedGeneralHotReleaseV3,
    artifact_selection: GeneralArtifactSelectionV3,
    fixed_accounts: Vec<RouteMetaV1>,
    strategy_accounts: Vec<RouteMetaV1>,
    runtime_suffix_accounts: Vec<RouteMetaV1>,
}

impl GeneralSuccessorRouteV1 {
    /// Finalized observation floor carried by the untrusted route and enforced
    /// again against every supplied account observation.
    #[must_use]
    pub const fn minimum_finalized_slot(&self) -> u64 {
        self.minimum_finalized_slot
    }

    /// Exact unique account list one provider must reacquire atomically.
    pub fn snapshot_addresses(&self) -> Result<Vec<Pubkey>> {
        snapshot_addresses_v1(self)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdsV5 {
    program_set: String,
    descriptor: String,
    config: String,
    account_profile: String,
    lifecycle_policy: String,
    request_profile: String,
    strategy: String,
    certificate: String,
    admission: String,
    transition: String,
    effect: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleStateV5 {
    account_coordinate: u16,
    account: String,
    bump: u8,
    is_materialized: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleV5 {
    primary: LifecycleStateV5,
    secondary: Option<LifecycleStateV5>,
    conditional_result: Option<LifecycleStateV5>,
    terminal_coordinate: Option<String>,
    child_account_start: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptDependencyV5 {
    producer_role: &'static str,
    producer_route: u16,
    expected_receipt_bytes: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChildRouteV5 {
    route: u16,
    role: &'static str,
    account_start: u16,
    account_count: u16,
    receipt_dependencies: Vec<ReceiptDependencyV5>,
}

/// One complete unsigned General V5 wallet-handoff document.
///
/// The document is deliberately opaque. It can be encoded or published only
/// by this crate, keeping the report rejoin and its exact JSON schema under the
/// same semantic owner as route parsing and transaction compilation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSuccessorPlanDocumentV5 {
    format: &'static str,
    action: &'static str,
    transaction_base64: String,
    observed_slot: String,
    outcome_count: u32,
    admitted_invocation_count: u32,
    heap_frame_bytes: u32,
    trading_program: String,
    lookup_table: String,
    payer: String,
    required_signers: Vec<String>,
    market: String,
    root: String,
    generation: String,
    release_set: String,
    root_prestate_digest: String,
    family_request_digest: String,
    checked_manifest_digest: String,
    trading_artifact_release: String,
    general_artifact_release: String,
    product_record: String,
    artifacts: ArtifactIdsV5,
    lifecycle: LifecycleV5,
    child_routes: Vec<ChildRouteV5>,
}

impl GeneralSuccessorPlanDocumentV5 {
    /// Canonical action name carried by the compiled instruction.
    #[must_use]
    pub const fn action(&self) -> &'static str {
        self.action
    }

    /// Finalized slot shared by every account used to build the plan.
    #[must_use]
    pub fn observed_slot(&self) -> &str {
        &self.observed_slot
    }

    /// Market address authenticated into the Hot execution envelope.
    #[must_use]
    pub fn market(&self) -> &str {
        &self.market
    }

    /// Ordered signer addresses required by the unsigned transaction.
    #[must_use]
    pub fn required_signers(&self) -> &[String] {
        &self.required_signers
    }
}

/// Read one ordinary bounded route file without following a symlink.
pub fn read_bounded_route_file_v1(path: &Path) -> Result<Vec<u8>> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::new(
            "--route must name one ordinary non-symlink file",
        ));
    }
    if metadata.len() > MAX_ROUTE_BYTES_V1 as u64 {
        return Err(Error::new("General route document exceeds 256 KiB"));
    }
    fs::read(path).map_err(Into::into)
}

/// Hostile-decode one exact General successor route document.
pub fn parse_route_v1(bytes: &[u8]) -> Result<GeneralSuccessorRouteV1> {
    if bytes.is_empty() || bytes.len() > MAX_ROUTE_BYTES_V1 {
        return Err(Error::new(
            "General route document has an invalid bounded width",
        ));
    }
    let value = parse_json_without_duplicate_keys_v1(bytes)?;
    let wire: GeneralSuccessorRouteWireV1 = serde_json::from_value(value)
        .map_err(|error| Error::new(format!("General route shape: {error}")))?;
    if wire.format != ROUTE_FORMAT_V1 {
        return Err(Error::new("General route format is not exact V1"));
    }
    let fixed_accounts = parse_metas_v1(wire.fixed_accounts, "fixedAccounts", false)?;
    if fixed_accounts.len()
        != dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3
    {
        return Err(Error::new(
            "General route must carry exactly the canonical Hot fixed frame",
        ));
    }
    let strategy_accounts = parse_metas_v1(wire.strategy_accounts, "strategyAccounts", false)?;
    // THE RUNTIME SUFFIX IS THE ONE PLACE THE SYSTEM PROGRAM CAN APPEAR.
    //
    // Every General AccountProfile declares a System-program runtime
    // coordinate, and on Solana the System program IS the all-zero key -- so
    // the blanket "nonzero" refusal below made this grammar unable to state any
    // General action at all. Nothing found it for as long as nothing produced a
    // route: the parser, the projection and the refusal were all written and
    // only the failure path was ever exercised. The guard that matters for an
    // ACCOUNT is the canonical base58 round trip; nonzero is a guard for
    // content IDENTITIES, where the zero value is reserved and means nothing.
    let runtime_suffix_accounts =
        parse_metas_v1(wire.runtime_suffix_accounts, "runtimeSuffixAccounts", true)?;
    let minimum_finalized_slot =
        decimal_u64_v1(&wire.minimum_finalized_slot, "minimumFinalizedSlot", false)?;
    let generation = decimal_u64_v1(&wire.generation, "generation", false)?;
    let checked_release = CheckedGeneralHotReleaseV3 {
        trading_program: address_v1(&wire.checked_release.trading_program, "tradingProgram")?,
        trading_artifact_release: identity_v1(
            &wire.checked_release.trading_artifact_release,
            "tradingArtifactRelease",
        )?,
        general_artifact_release: identity_v1(
            &wire.checked_release.general_artifact_release,
            "generalArtifactRelease",
        )?,
        checked_manifest_digest: identity_v1(
            &wire.checked_release.checked_manifest_digest,
            "checkedManifestDigest",
        )?,
    };
    let artifact_selection = GeneralArtifactSelectionV3 {
        program_set: identity_v1(&wire.artifact_selection.program_set, "programSet")?,
        config: identity_v1(&wire.artifact_selection.config, "config")?,
        artifact_release: identity_v1(
            &wire.artifact_selection.artifact_release,
            "artifactRelease",
        )?,
    };
    if artifact_selection.artifact_release != checked_release.general_artifact_release {
        return Err(Error::new(
            "artifactSelection.artifactRelease differs from the checked General release",
        ));
    }
    let route = GeneralSuccessorRouteV1 {
        action: action_v1(&wire.action)?,
        minimum_finalized_slot,
        payer: address_v1(&wire.payer, "payer")?,
        lookup_table: address_v1(&wire.lookup_table, "lookupTable")?,
        release_set: identity_v1(&wire.release_set, "releaseSet")?,
        generation,
        checked_release,
        artifact_selection,
        fixed_accounts,
        strategy_accounts,
        runtime_suffix_accounts,
    };
    for (index, meta) in route.fixed_accounts.iter().enumerate() {
        if meta.is_signer || meta.is_writable != (index == HOT_ROOT_ACCOUNT_V3) {
            return Err(Error::new(format!(
                "fixedAccounts[{index}] carries noncanonical Hot privileges"
            )));
        }
    }
    if route
        .strategy_accounts
        .iter()
        .any(|meta| meta.is_signer || meta.is_writable)
    {
        return Err(Error::new(
            "General strategy accounts must all be read-only nonsigners",
        ));
    }
    if route.payer == route.lookup_table
        || route
            .fixed_accounts
            .iter()
            .chain(&route.strategy_accounts)
            .chain(&route.runtime_suffix_accounts)
            .any(|meta| meta.address == route.lookup_table)
    {
        return Err(Error::new(
            "General lookup table aliases the payer or one instruction account",
        ));
    }
    let _ = snapshot_addresses_v1(&route)?;
    Ok(route)
}

fn parse_metas_v1(
    values: Vec<RouteMetaWireV1>,
    field: &str,
    admits_system_program: bool,
) -> Result<Vec<RouteMetaV1>> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let field = format!("{field}[{index}].address");
            Ok(RouteMetaV1 {
                address: if admits_system_program {
                    account_address_v1(&value.address, &field)?
                } else {
                    address_v1(&value.address, &field)?
                },
                is_signer: value.is_signer,
                is_writable: value.is_writable,
            })
        })
        .collect()
}

fn action_v1(value: &str) -> Result<Action> {
    match value {
        "consider" => Ok(Action::Consider),
        "freeze" => Ok(Action::Freeze),
        "initialize-settlement" => Ok(Action::InitializeSettlement),
        "collect" => Ok(Action::Collect),
        "materialize" => Ok(Action::Materialize),
        "distribute" => Ok(Action::Distribute),
        "close" => Ok(Action::Close),
        "open-batch" => Ok(Action::OpenBatch),
        "place-order" => Ok(Action::PlaceOrder),
        "cancel-order" => Ok(Action::CancelOrder),
        "close-batch" => Ok(Action::CloseBatch),
        "submit-candidate" => Ok(Action::SubmitCandidate),
        "verify-candidate-row" => Ok(Action::VerifyCandidateRow),
        "release-order" => Ok(Action::ReleaseOrder),
        "close-candidate" => Ok(Action::CloseCandidate),
        _ => Err(Error::new("General route action is unknown")),
    }
}

/// The exact route/plan spelling of one General action.
///
/// Public because a route PRODUCER must spell the action the way
/// [`parse_route_v1`] reads it, and a producer that keeps its own table is a
/// second author for a name whose only job is to match.
#[must_use]
pub fn action_name_v1(action: Action) -> &'static str {
    match action {
        Action::Consider => "consider",
        Action::Freeze => "freeze",
        Action::InitializeSettlement => "initialize-settlement",
        Action::Collect => "collect",
        Action::Materialize => "materialize",
        Action::Distribute => "distribute",
        Action::Close => "close",
        Action::OpenBatch => "open-batch",
        Action::PlaceOrder => "place-order",
        Action::CancelOrder => "cancel-order",
        Action::CloseBatch => "close-batch",
        Action::SubmitCandidate => "submit-candidate",
        Action::VerifyCandidateRow => "verify-candidate-row",
        Action::ReleaseOrder => "release-order",
        Action::CloseCandidate => "close-candidate",
    }
}

fn decimal_u64_v1(value: &str, field: &str, allow_zero: bool) -> Result<u64> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(Error::new(format!(
            "{field} is not canonical unsigned decimal text"
        )));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|error| Error::new(format!("{field}: {error}")))?;
    if !allow_zero && parsed == 0 {
        return Err(Error::new(format!("{field} must be positive")));
    }
    Ok(parsed)
}

fn address_v1(value: &str, field: &str) -> Result<Pubkey> {
    let parsed = value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{field}: {error}")))?;
    if parsed == Pubkey::default() || parsed.to_string() != value {
        return Err(Error::new(format!(
            "{field} is not a nonzero canonical public key"
        )));
    }
    Ok(parsed)
}

/// Accept one canonical account address, INCLUDING the System program.
///
/// [`address_v1`] additionally refuses the all-zero key, which is right for the
/// payer, the lookup table and the Trading program -- a zero there is a field
/// nobody filled in -- and wrong for a runtime coordinate whose published
/// AccountProfile requires the System program to be exactly there.
fn account_address_v1(value: &str, field: &str) -> Result<Pubkey> {
    let parsed = value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{field}: {error}")))?;
    if parsed.to_string() != value {
        return Err(Error::new(format!("{field} is not a canonical public key")));
    }
    Ok(parsed)
}

fn identity_v1(value: &str, field: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(Error::new(format!(
            "{field} is not 32 lowercase hexadecimal bytes"
        )));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| Error::new(format!("{field}: {error}")))?;
    }
    if output == [0; 32] {
        return Err(Error::new(format!("{field} is the reserved zero identity")));
    }
    Ok(output)
}

fn snapshot_addresses_v1(route: &GeneralSuccessorRouteV1) -> Result<Vec<Pubkey>> {
    let mut addresses = Vec::new();
    for address in route
        .fixed_accounts
        .iter()
        .chain(&route.strategy_accounts)
        .chain(&route.runtime_suffix_accounts)
        .map(|meta| meta.address)
        .chain(core::iter::once(route.lookup_table))
    {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() || addresses.len() > MAX_SNAPSHOT_ACCOUNTS_V1 {
        return Err(Error::new(format!(
            "General route requires {} unique snapshot accounts; one exact snapshot admits at most {MAX_SNAPSHOT_ACCOUNTS_V1}",
            addresses.len()
        )));
    }
    Ok(addresses)
}

/// Project one exact finalized snapshot onto an authenticated route.
///
/// This is the sole non-test constructor of [`GeneralHotStateV3`], and it is
/// public so that a PRODUCER of routes can close its own loop: a host that
/// emits a route document has no other way to prove the document it just wrote
/// is one this parser and this projection accept. Callers get no new authority
/// from it -- every field it fills is taken from the route it was handed and
/// the observation it was handed, and both are re-checked here.
pub fn acquire_route_v1(
    route: &GeneralSuccessorRouteV1,
    observed: Vec<ObservedAccount>,
) -> Result<(GeneralHotStateV3, ObservedAccount)> {
    let addresses = snapshot_addresses_v1(route)?;
    if observed.len() != addresses.len() {
        return Err(Error::new(
            "General snapshot response width differed from its exact request",
        ));
    }
    let observation = observed
        .first()
        .map(|account| account.observation)
        .ok_or_else(|| Error::new("General snapshot was empty"))?;
    if observation.finality != Finality::Finalized
        || observation.slot < route.minimum_finalized_slot
        || observed
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(Error::new(
            "General accounts were not one finalized observation at or after the route floor",
        ));
    }
    if observed
        .iter()
        .map(|account| account.key)
        .collect::<std::collections::BTreeSet<_>>()
        != addresses.iter().copied().collect()
    {
        return Err(Error::new(
            "General snapshot keys differed from the exact route request",
        ));
    }
    let by_key = observed
        .into_iter()
        .map(|account| (account.key, account))
        .collect::<BTreeMap<_, _>>();
    let project = |values: &[RouteMetaV1]| -> Result<Vec<GeneralObservedAccountMetaV3>> {
        values
            .iter()
            .map(|meta| {
                Ok(GeneralObservedAccountMetaV3 {
                    account: by_key
                        .get(&meta.address)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("snapshot omitted {}", meta.address)))?,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
            })
            .collect()
    };
    let lookup_table = by_key
        .get(&route.lookup_table)
        .cloned()
        .ok_or_else(|| Error::new("snapshot omitted the lookup table"))?;
    Ok((
        GeneralHotStateV3 {
            fixed_accounts: project(&route.fixed_accounts)?,
            strategy_accounts: project(&route.strategy_accounts)?,
            runtime_suffix_accounts: project(&route.runtime_suffix_accounts)?,
            release_set: route.release_set,
            generation: route.generation,
            minimum_finalized_slot: route.minimum_finalized_slot,
            checked_release: Some(route.checked_release),
        },
        lookup_table,
    ))
}

/// The exact address set this route's compiled instruction requires of its
/// lookup table.
///
/// `compile_general_hot_v0` accepts one table and requires
/// `table.addresses == canonical_general_lookup_addresses_v3(instruction, payer)`
/// -- byte-for-byte slice equality, not a superset and not a permutation. So a
/// caller who does not already hold such a table has no way to build one from
/// the route document alone: the set is a function of the COMPILED
/// instruction, which needs the snapshot, the artifacts and the derived
/// request. This returns it, doing everything `produce_plan_v5` does except the
/// compilation that would refuse.
///
/// It exists because nothing in this tree creates a table over a General Hot
/// frame. `publish_routing_table` creates them for foundings, activations and
/// Direct fills, and the General family's only table is `GENERAL-ACT`, whose
/// set is the ACTIVATION instruction's. Measured on devnet, 2026-09-04: the
/// first route ever produced reached `General v0 compilation: LookupTable` and
/// stopped there.
pub fn canonical_lookup_addresses_v1(
    route: &GeneralSuccessorRouteV1,
    observed: Vec<ObservedAccount>,
) -> Result<Vec<Pubkey>> {
    let (state, _) = acquire_route_v1(route, observed)?;
    let artifacts = general_artifact_bytes_from_hot_state_v3(&state)
        .map_err(|error| Error::new(format!("General artifact carriers: {error:?}")))?;
    let successor = build_general_successor_instruction_v5(
        &state,
        route.artifact_selection,
        artifacts,
        route.action,
    )
    .map_err(|error| Error::new(format!("General successor construction: {error:?}")))?;
    canonical_general_lookup_addresses_v3(&successor.hot.instruction, route.payer)
        .map_err(|error| Error::new(format!("General lookup addresses: {error:?}")))
}

/// The payer this route names, which the address set above is relative to.
#[must_use]
pub fn route_payer_v1(route: &GeneralSuccessorRouteV1) -> Pubkey {
    route.payer
}

/// Build one complete unsigned plan from an exact route, one finalized atomic
/// observation, and one recent finalized blockhash. No key is read and no
/// transaction is signed, simulated, or submitted.
pub fn produce_plan_v5(
    route: &GeneralSuccessorRouteV1,
    observed: Vec<ObservedAccount>,
    recent_blockhash: Hash,
) -> Result<GeneralSuccessorPlanDocumentV5> {
    if recent_blockhash == Hash::default() {
        return Err(Error::new("General planner received the zero blockhash"));
    }
    let (state, lookup_table) = acquire_route_v1(route, observed)?;
    let artifacts = general_artifact_bytes_from_hot_state_v3(&state)
        .map_err(|error| Error::new(format!("General artifact carriers: {error:?}")))?;
    let successor = build_general_successor_instruction_v5(
        &state,
        route.artifact_selection,
        artifacts,
        route.action,
    )
    .map_err(|error| Error::new(format!("General successor construction: {error:?}")))?;
    let transaction =
        compile_general_successor_v0(&successor, route.payer, recent_blockhash, &lookup_table)
            .map_err(|error| Error::new(format!("General v0 compilation: {error:?}")))?;
    serialize_plan_v5(&state, &successor, &transaction, route.payer, &lookup_table)
}

fn serialize_plan_v5(
    state: &GeneralHotStateV3,
    successor: &GeneralSuccessorInstructionV5,
    transaction: &GeneralSuccessorTransactionPlanV0,
    payer: Pubkey,
    lookup_table: &ObservedAccount,
) -> Result<GeneralSuccessorPlanDocumentV5> {
    let checked = state
        .checked_release
        .ok_or_else(|| Error::new("General serializer lost checked release evidence"))?;
    let report = &successor.hot;
    let transaction_report = &transaction.hot;
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or_else(|| Error::new("General serializer lost the Market account"))?;
    let root = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or_else(|| Error::new("General serializer lost the root account"))?;
    let (envelope, request_bytes) =
        HotExecutionEnvelopeV3::split_instruction(&report.instruction.data)
            .map_err(|_| Error::new("General serializer could not rejoin the Hot envelope"))?;
    let canonical_request = successor
        .request
        .to_bytes()
        .map_err(|_| Error::new("General serializer could not encode the canonical request"))?;
    let root_digest: [u8; 32] = Sha256::digest(&root.account.data).into();
    let request_digest: [u8; 32] = Sha256::digest(request_bytes).into();
    let VersionedMessage::V0(compiled_message) = &transaction_report.message.message else {
        return Err(Error::new("General serializer requires one v0 message"));
    };
    if request_bytes != canonical_request
        || successor.request != transaction.request
        || successor.request.action != report.action
        || report.action != transaction_report.action
        || successor.outcome_count != report.outcome_count
        || report.outcome_count != transaction_report.outcome_count
        || successor.admitted_invocation_count != transaction.admitted_invocation_count
        || successor.heap_frame_bytes != GENERAL_HOT_HEAP_FRAME_BYTES_V3
        || transaction.heap_frame_bytes != successor.heap_frame_bytes
        || transaction_report.heap_frame_bytes != successor.heap_frame_bytes
        || successor.child_routes != transaction.child_routes
        || report.observation != market.account.observation
        || report.observation != root.account.observation
        || report.instruction.program_id != checked.trading_program
        || envelope.market() != market.account.key.to_bytes()
        || envelope.release_set() != state.release_set
        || envelope.generation() != state.generation
        || envelope.root_prestate_digest() != root_digest
        || report.family_request_digest != request_digest
        || report.checked_manifest_digest != checked.checked_manifest_digest
        || report.trading_artifact_release != checked.trading_artifact_release
        || report.general_artifact_release != checked.general_artifact_release
        || transaction_report.checked_manifest_digest != report.checked_manifest_digest
        || transaction_report.trading_artifact_release != report.trading_artifact_release
        || transaction_report.general_artifact_release != report.general_artifact_release
        || transaction_report.artifacts != report.artifacts
        || transaction_report.product_record != report.product_record
        || transaction_report.lifecycle != report.lifecycle
        || transaction_report.message.lookup_tables.as_slice() != [lookup_table.key]
        || compiled_message.address_table_lookups.len() != 1
        || compiled_message.address_table_lookups[0].account_key != lookup_table.key
        || transaction_report.required_signers.first() != Some(&payer)
        || usize::from(transaction_report.message.required_signatures)
            != transaction_report.required_signers.len()
    {
        return Err(Error::new(
            "General serializer refused a state/successor/transaction rejoin mismatch",
        ));
    }
    let [compiled_heap, compiled_hot] = compiled_message.instructions.as_slice() else {
        return Err(Error::new(
            "General serializer requires exactly one heap declaration followed by one Hot instruction",
        ));
    };
    let expected_heap =
        ComputeBudgetInstruction::request_heap_frame(GENERAL_HOT_HEAP_FRAME_BYTES_V3);
    let heap_program = compiled_message
        .account_keys
        .get(usize::from(compiled_heap.program_id_index))
        .ok_or_else(|| Error::new("General heap declaration program index was out of bounds"))?;
    let hot_program = compiled_message
        .account_keys
        .get(usize::from(compiled_hot.program_id_index))
        .ok_or_else(|| Error::new("General Hot instruction program index was out of bounds"))?;
    if *heap_program != expected_heap.program_id
        || !compiled_heap.accounts.is_empty()
        || compiled_heap.data != expected_heap.data
        || *hot_program != report.instruction.program_id
        || compiled_hot.data != report.instruction.data
    {
        return Err(Error::new(
            "General serializer refused a substituted heap declaration or instruction order",
        ));
    }
    let unsigned = VersionedTransaction {
        signatures: vec![
            Signature::default();
            usize::from(transaction_report.message.required_signatures)
        ],
        message: transaction_report.message.message.clone(),
    };
    let packet = bincode::serialize(&unsigned)
        .map_err(|error| Error::new(format!("General unsigned packet serialization: {error}")))?;
    if packet.len() != transaction_report.message.wire_bytes {
        return Err(Error::new(
            "General unsigned packet width differs from the compiler report",
        ));
    }
    let lifecycle = report.lifecycle;
    Ok(GeneralSuccessorPlanDocumentV5 {
        format: PLAN_FORMAT_V5,
        action: action_name_v1(report.action),
        transaction_base64: BASE64.encode(packet),
        observed_slot: report.observation.slot.to_string(),
        outcome_count: report.outcome_count,
        admitted_invocation_count: successor.admitted_invocation_count,
        heap_frame_bytes: successor.heap_frame_bytes,
        trading_program: report.instruction.program_id.to_string(),
        lookup_table: lookup_table.key.to_string(),
        payer: payer.to_string(),
        required_signers: transaction_report
            .required_signers
            .iter()
            .map(ToString::to_string)
            .collect(),
        market: market.account.key.to_string(),
        root: root.account.key.to_string(),
        generation: state.generation.to_string(),
        release_set: hex_v1(state.release_set),
        root_prestate_digest: hex_v1(root_digest),
        family_request_digest: hex_v1(report.family_request_digest),
        checked_manifest_digest: hex_v1(report.checked_manifest_digest),
        trading_artifact_release: hex_v1(report.trading_artifact_release),
        general_artifact_release: hex_v1(report.general_artifact_release),
        product_record: hex_v1(report.product_record),
        artifacts: artifact_ids_v5(report.artifacts),
        lifecycle: LifecycleV5 {
            primary: lifecycle_state_v5(lifecycle.primary),
            secondary: lifecycle.secondary.map(lifecycle_state_v5),
            conditional_result: lifecycle.conditional_result.map(lifecycle_state_v5),
            terminal_coordinate: lifecycle.terminal_coordinate.map(|value| value.to_string()),
            child_account_start: lifecycle.child_account_start,
        },
        child_routes: successor
            .child_routes
            .iter()
            .map(|route| {
                Ok(ChildRouteV5 {
                    route: route.route,
                    role: role_name_v1(route.role)?,
                    account_start: route.account_start,
                    account_count: route.account_count,
                    receipt_dependencies: route
                        .receipt_dependencies
                        .iter()
                        .map(|dependency| {
                            Ok(ReceiptDependencyV5 {
                                producer_role: role_name_v1(dependency.producer_role)?,
                                producer_route: dependency.producer_route,
                                expected_receipt_bytes: dependency.expected_receipt_bytes,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn lifecycle_state_v5(
    state: dclutch_operator::general_hot_v3::GeneralLifecycleStateProjectionV3,
) -> LifecycleStateV5 {
    LifecycleStateV5 {
        account_coordinate: state.account_coordinate,
        account: state.account.to_string(),
        bump: state.bump,
        is_materialized: state.is_materialized,
    }
}

fn role_name_v1(role: FixedRole) -> Result<&'static str> {
    match role {
        FixedRole::Claims => Ok("claims"),
        FixedRole::Custody => Ok("custody"),
        FixedRole::Core | FixedRole::Resolution => Err(Error::new(
            "General child route selected a role outside Claims/Custody",
        )),
    }
}

fn artifact_ids_v5(value: GeneralHotArtifactDigestsV3) -> ArtifactIdsV5 {
    ArtifactIdsV5 {
        program_set: hex_v1(value.program_set),
        descriptor: hex_v1(value.descriptor),
        config: hex_v1(value.config),
        account_profile: hex_v1(value.account_profile),
        lifecycle_policy: hex_v1(value.lifecycle_policy),
        request_profile: hex_v1(value.request_profile),
        strategy: hex_v1(value.strategy),
        certificate: hex_v1(value.certificate),
        admission: hex_v1(value.admission),
        transition: hex_v1(value.transition),
        effect: hex_v1(value.effect),
    }
}

fn hex_v1(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Encode one exact pretty JSON plan with the shared trailing-newline and byte
/// ceiling contract used by every producer surface.
pub fn encode_plan_v5(value: &GeneralSuccessorPlanDocumentV5) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    if bytes.len() > GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5 {
        return Err(Error::new("General V5 plan exceeds 64 KiB"));
    }
    Ok(bytes)
}

/// Atomically publish one new private plan file without clobbering an existing
/// path or following a symlinked parent.
pub fn write_new_plan_v5(path: &Path, value: &GeneralSuccessorPlanDocumentV5) -> Result<()> {
    if !path.is_absolute() || path.exists() {
        return Err(Error::new("--output must be one absent absolute path"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("--output has no parent directory"))?;
    let canonical_parent = parent.canonicalize()?;
    if canonical_parent != parent || !canonical_parent.is_dir() {
        return Err(Error::new(
            "--output parent must be one canonical directory",
        ));
    }
    let bytes = encode_plan_v5(value)?;
    let temporary = temporary_plan_path_v5(&canonical_parent, &bytes);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let publish = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        // A same-filesystem hard link is an atomic no-clobber publication:
        // unlike `rename`, it refuses if a racing writer created `path` after
        // the preflight check. The temporary and final names then address the
        // same already-fsynced inode until the private name is removed.
        fs::hard_link(&temporary, path)?;
        fs::remove_file(&temporary)?;
        File::open(&canonical_parent)?.sync_all()?;
        Ok(())
    })();
    if publish.is_err() {
        // This exact file was created by this invocation. Best-effort cleanup
        // never touches the requested destination, which may belong to a
        // racing successful producer.
        let _ = fs::remove_file(&temporary);
    }
    publish
}

fn temporary_plan_path_v5(parent: &Path, bytes: &[u8]) -> PathBuf {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    parent.join(format!(
        ".dclutch-general-plan-{}-{}.partial",
        std::process::id(),
        hex_v1(digest)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_program_contract::hot_v3::HOT_TRADING_PROGRAM_ACCOUNT_V3;
    use dclutch_general_adapter_contract::artifacts_v3::{
        GeneralDecodedRequestV3, GeneralRequestWireV3,
    };
    use dclutch_operator::general_hot_v3::{
        GeneralHotInstructionV3, GeneralHotTransactionPlanV3, GeneralLifecycleProjectionV3,
        GeneralLifecycleStateProjectionV3,
    };
    use dclutch_versioned_message_operator::{Finality, Observation, VersionedMessagePlanV0};
    use serde_json::{Value, json};
    use solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{AddressLookupTableAccount, VersionedMessage, v0},
    };
    use std::os::unix::fs::PermissionsExt as _;

    fn key(value: u8) -> String {
        Pubkey::new_from_array([value; 32]).to_string()
    }

    fn identity(value: u8) -> String {
        hex_v1([value; 32])
    }

    fn route_value() -> Value {
        let fixed = (1_u8..=39)
            .map(|value| {
                json!({
                    "address": key(value),
                    "isSigner": false,
                    "isWritable": value == 2,
                })
            })
            .collect::<Vec<_>>();
        let strategy = (40_u8..=47)
            .map(|value| json!({"address":key(value),"isSigner":false,"isWritable":false}))
            .collect::<Vec<_>>();
        json!({
            "format": ROUTE_FORMAT_V1,
            "action": "open-batch",
            "minimumFinalizedSlot": "77",
            "payer": key(90),
            "lookupTable": key(91),
            "releaseSet": identity(1),
            "generation": "7",
            "checkedRelease": {
                "tradingProgram": key(26),
                "tradingArtifactRelease": identity(2),
                "generalArtifactRelease": identity(3),
                "checkedManifestDigest": identity(4)
            },
            "artifactSelection": {
                "programSet": identity(5),
                "config": identity(6),
                "artifactRelease": identity(3)
            },
            "fixedAccounts": fixed,
            "strategyAccounts": strategy,
            "runtimeSuffixAccounts": []
        })
    }

    /// THE SYSTEM PROGRAM IS THE ALL-ZERO KEY, AND EVERY GENERAL ACTION
    /// DECLARES IT AS A RUNTIME COORDINATE.
    ///
    /// This grammar refused it everywhere until a producer existed to try, so
    /// no General route could be stated at all -- the parser, the projection
    /// and the refusal were all written and only the failure path was ever
    /// exercised. The runtime suffix admits it; the fixed frame and the
    /// strategy accounts, where no profile ever puts it, still do not.
    #[test]
    fn the_runtime_suffix_may_name_the_system_program_and_nothing_else_may() {
        // `Pubkey::default()` IS `11111111111111111111111111111111`, the System
        // program. That identity is the whole finding.
        let mut route = route_value();
        route["runtimeSuffixAccounts"] = json!([
            {"address": key(50), "isSigner": false, "isWritable": true},
            {"address": key(51), "isSigner": true, "isWritable": true},
            {"address": key(52), "isSigner": false, "isWritable": true},
            {
                "address": Pubkey::default().to_string(),
                "isSigner": false,
                "isWritable": false
            },
        ]);
        let parsed = parse_route_v1(&serde_json::to_vec(&route).expect("route JSON"))
            .expect("the System program is a runtime coordinate");
        assert_eq!(parsed.runtime_suffix_accounts[3].address, Pubkey::default());

        let mut fixed_zero = route.clone();
        fixed_zero["fixedAccounts"][7]["address"] = json!(Pubkey::default().to_string());
        let error = parse_route_v1(&serde_json::to_vec(&fixed_zero).expect("JSON"))
            .expect_err("no fixed coordinate is the System program");
        assert!(
            error.to_string().contains("nonzero canonical public key"),
            "{error}"
        );

        let mut payer_zero = route;
        payer_zero["payer"] = json!(Pubkey::default().to_string());
        assert!(
            parse_route_v1(&serde_json::to_vec(&payer_zero).expect("JSON")).is_err(),
            "a zero payer is a field nobody filled in"
        );
    }

    #[test]
    fn route_parser_accepts_only_exact_bounded_machine_input() {
        let bytes = serde_json::to_vec(&route_value()).expect("route JSON");
        let route = parse_route_v1(&bytes).expect("exact route");
        assert_eq!(route.action, Action::OpenBatch);
        assert_eq!(route.minimum_finalized_slot, 77);
        assert_eq!(route.fixed_accounts.len(), 39);
        assert_eq!(snapshot_addresses_v1(&route).expect("snapshot").len(), 48);

        let mut extra = route_value();
        extra["extra"] = json!(true);
        assert!(parse_route_v1(&serde_json::to_vec(&extra).expect("extra JSON")).is_err());
        let duplicate = br#"{"format":"a","format":"b"}"#;
        assert!(parse_route_v1(duplicate).is_err());
    }

    #[test]
    fn route_parser_refuses_identity_alias_and_snapshot_expansion() {
        let mut wrong_release = route_value();
        wrong_release["artifactSelection"]["artifactRelease"] = json!(identity(9));
        assert!(
            parse_route_v1(&serde_json::to_vec(&wrong_release).expect("wrong release JSON"))
                .is_err()
        );

        let mut noncanonical_slot = route_value();
        noncanonical_slot["minimumFinalizedSlot"] = json!("077");
        assert!(
            parse_route_v1(&serde_json::to_vec(&noncanonical_slot).expect("slot JSON")).is_err()
        );

        let mut too_wide = route_value();
        too_wide["runtimeSuffixAccounts"] = Value::Array(
            (100_u8..=160)
                .map(|value| {
                    json!({
                        "address": Pubkey::new_from_array([value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value]).to_string(),
                        "isSigner":false,
                        "isWritable":false
                    })
                })
                .collect(),
        );
        assert!(parse_route_v1(&serde_json::to_vec(&too_wide).expect("wide JSON")).is_err());

        let mut writable_fixed = route_value();
        writable_fixed["fixedAccounts"][4]["isWritable"] = json!(true);
        assert!(
            parse_route_v1(
                &serde_json::to_vec(&writable_fixed).expect("writable fixed-account JSON")
            )
            .is_err()
        );

        let mut writable_strategy = route_value();
        writable_strategy["strategyAccounts"][0]["isWritable"] = json!(true);
        assert!(
            parse_route_v1(
                &serde_json::to_vec(&writable_strategy).expect("writable strategy JSON")
            )
            .is_err()
        );

        let mut aliased_lookup = route_value();
        aliased_lookup["lookupTable"] = aliased_lookup["payer"].clone();
        assert!(
            parse_route_v1(&serde_json::to_vec(&aliased_lookup).expect("aliased lookup JSON"))
                .is_err()
        );

        let mut uppercase_identity = route_value();
        uppercase_identity["releaseSet"] = json!(identity(0xab).to_uppercase());
        assert!(
            parse_route_v1(
                &serde_json::to_vec(&uppercase_identity).expect("uppercase identity JSON")
            )
            .is_err()
        );
    }

    #[test]
    fn action_text_is_exhaustive_for_the_current_rust_catalogue() {
        for action in [
            Action::Consider,
            Action::Freeze,
            Action::InitializeSettlement,
            Action::Collect,
            Action::Materialize,
            Action::Distribute,
            Action::Close,
            Action::OpenBatch,
            Action::PlaceOrder,
            Action::CancelOrder,
            Action::CloseBatch,
            Action::SubmitCandidate,
            Action::VerifyCandidateRow,
            Action::ReleaseOrder,
            Action::CloseCandidate,
        ] {
            assert_eq!(action_v1(action_name_v1(action)).expect("action"), action);
        }
        assert!(action_v1("close_candidate").is_err());
    }

    #[test]
    fn serializer_rejoins_every_report_and_emits_zero_signature_wire() {
        let observation = Observation {
            slot: 77,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let payer = Pubkey::new_from_array([90; 32]);
        let lookup_key = Pubkey::new_from_array([91; 32]);
        let market = Pubkey::new_from_array([92; 32]);
        let root = Pubkey::new_from_array([93; 32]);
        let trading = Pubkey::new_from_array([94; 32]);
        let root_data = vec![0x31; 96];
        let observed = |key: Pubkey, data: Vec<u8>| ObservedAccount {
            observation,
            key,
            owner: Pubkey::new_from_array([95; 32]),
            lamports: 1,
            executable: false,
            data,
        };
        let mut fixed_accounts = (0_u8..39)
            .map(|index| GeneralObservedAccountMetaV3 {
                account: observed(Pubkey::new_from_array([index + 1; 32]), vec![index]),
                is_signer: false,
                is_writable: index == 1,
            })
            .collect::<Vec<_>>();
        fixed_accounts[HOT_MARKET_ACCOUNT_V3].account = observed(market, vec![0x21; 64]);
        fixed_accounts[HOT_ROOT_ACCOUNT_V3].account = observed(root, root_data.clone());
        fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3].account = observed(trading, vec![0x41; 64]);
        fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3]
            .account
            .executable = true;
        let checked = CheckedGeneralHotReleaseV3 {
            trading_program: trading,
            trading_artifact_release: [0x42; 32],
            general_artifact_release: [0x43; 32],
            checked_manifest_digest: [0x44; 32],
        };
        let state = GeneralHotStateV3 {
            fixed_accounts,
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: Vec::new(),
            release_set: [0x45; 32],
            generation: 7,
            minimum_finalized_slot: 70,
            checked_release: Some(checked),
        };
        let request = GeneralDecodedRequestV3 {
            wire: GeneralRequestWireV3::V3,
            action: Action::OpenBatch,
            expected_revision: 9,
            candidate_id: Some([0x46; 32]),
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            state_bump: 7,
            terminal_record_bump: 0,
            result_state_bump: 0,
        };
        let request_bytes = request.to_bytes().expect("canonical request");
        let root_digest: [u8; 32] = Sha256::digest(&root_data).into();
        let envelope = HotExecutionEnvelopeV3::new(
            request_bytes.len() as u32,
            state.release_set,
            market.to_bytes(),
            state.generation,
            root_digest,
        )
        .expect("Hot envelope");
        let mut instruction_data = envelope.to_bytes().to_vec();
        instruction_data.extend_from_slice(&request_bytes);
        let instruction = Instruction {
            program_id: trading,
            accounts: vec![AccountMeta::new_readonly(market, false)],
            data: instruction_data,
        };
        let lifecycle = GeneralLifecycleProjectionV3 {
            primary: GeneralLifecycleStateProjectionV3 {
                account_coordinate: 5,
                account: Pubkey::new_from_array([0x47; 32]),
                bump: 7,
                is_materialized: false,
            },
            secondary: None,
            conditional_result: None,
            terminal_coordinate: None,
            child_account_start: 8,
        };
        let artifacts = GeneralHotArtifactDigestsV3 {
            program_set: [1; 32],
            descriptor: [2; 32],
            config: [3; 32],
            account_profile: [4; 32],
            lifecycle_policy: [5; 32],
            request_profile: [6; 32],
            strategy: [7; 32],
            certificate: [8; 32],
            admission: [9; 32],
            transition: [10; 32],
            effect: [11; 32],
        };
        let report = GeneralHotInstructionV3 {
            instruction: instruction.clone(),
            action: Action::OpenBatch,
            outcome_count: 3,
            observation,
            required_instruction_signers: Vec::new(),
            checked_manifest_digest: checked.checked_manifest_digest,
            trading_artifact_release: checked.trading_artifact_release,
            general_artifact_release: checked.general_artifact_release,
            artifacts,
            product_record: [0x48; 32],
            family_request_digest: Sha256::digest(request_bytes).into(),
            lifecycle,
        };
        let successor = GeneralSuccessorInstructionV5 {
            hot: report.clone(),
            heap_frame_bytes: GENERAL_HOT_HEAP_FRAME_BYTES_V3,
            request,
            outcome_count: 3,
            admitted_invocation_count: 1,
            child_routes: Vec::new(),
        };
        let heap_frame =
            ComputeBudgetInstruction::request_heap_frame(GENERAL_HOT_HEAP_FRAME_BYTES_V3);
        let compiled = v0::Message::try_compile(
            &payer,
            &[heap_frame, instruction],
            &[AddressLookupTableAccount {
                key: lookup_key,
                addresses: vec![market],
            }],
            Hash::new_unique(),
        )
        .expect("v0 message");
        assert_eq!(compiled.address_table_lookups.len(), 1);
        let required_signatures = usize::from(compiled.header.num_required_signatures);
        let loaded_addresses = compiled
            .address_table_lookups
            .iter()
            .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
            .sum();
        let message = VersionedMessage::V0(compiled);
        let unsigned = VersionedTransaction {
            signatures: vec![Signature::default(); required_signatures],
            message: message.clone(),
        };
        let message_plan = VersionedMessagePlanV0 {
            message,
            required_signatures: u8::try_from(required_signatures).expect("signature count"),
            wire_bytes: bincode::serialize(&unsigned).expect("unsigned wire").len(),
            loaded_addresses,
            lookup_tables: vec![lookup_key],
        };
        let transaction = GeneralSuccessorTransactionPlanV0 {
            hot: GeneralHotTransactionPlanV3 {
                message: message_plan,
                heap_frame_bytes: GENERAL_HOT_HEAP_FRAME_BYTES_V3,
                required_signers: vec![payer],
                action: report.action,
                checked_manifest_digest: report.checked_manifest_digest,
                outcome_count: report.outcome_count,
                trading_artifact_release: report.trading_artifact_release,
                general_artifact_release: report.general_artifact_release,
                artifacts: report.artifacts,
                product_record: report.product_record,
                lifecycle: report.lifecycle,
            },
            heap_frame_bytes: GENERAL_HOT_HEAP_FRAME_BYTES_V3,
            request,
            admitted_invocation_count: successor.admitted_invocation_count,
            child_routes: Vec::new(),
        };
        let lookup = observed(lookup_key, Vec::new());
        let document =
            serialize_plan_v5(&state, &successor, &transaction, payer, &lookup).expect("plan");
        assert_eq!(document.format, PLAN_FORMAT_V5);
        assert_eq!(document.action, "open-batch");
        assert_eq!(document.observed_slot, "77");
        assert_eq!(document.heap_frame_bytes, GENERAL_HOT_HEAP_FRAME_BYTES_V3);
        assert_eq!(document.required_signers, vec![payer.to_string()]);
        let decoded: VersionedTransaction = bincode::deserialize(
            &BASE64
                .decode(&document.transaction_base64)
                .expect("base64 wire"),
        )
        .expect("versioned transaction");
        assert_eq!(decoded.signatures, vec![Signature::default()]);

        let mut substituted = transaction.clone();
        substituted.hot.product_record[0] ^= 1;
        assert!(serialize_plan_v5(&state, &successor, &substituted, payer, &lookup).is_err());

        let mut substituted_heap = transaction.clone();
        substituted_heap.heap_frame_bytes /= 2;
        assert!(serialize_plan_v5(&state, &successor, &substituted_heap, payer, &lookup).is_err());

        let directory = fs::canonicalize(std::env::temp_dir())
            .expect("canonical temporary root")
            .join(format!("dclutch-general-plan-{}", Pubkey::new_unique()));
        fs::create_dir(&directory).expect("private test directory");
        let directory = fs::canonicalize(directory).expect("canonical test directory");
        let output = directory.join("plan.json");
        write_new_plan_v5(&output, &document).expect("durable new plan");
        assert_eq!(
            fs::metadata(&output)
                .expect("plan metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let persisted = fs::read(&output).expect("persisted plan bytes");
        let persisted_json: Value = serde_json::from_slice(&persisted).expect("persisted JSON");
        assert_eq!(persisted_json["format"], PLAN_FORMAT_V5);
        assert_eq!(
            persisted_json["transactionBase64"],
            document.transaction_base64
        );
        assert_eq!(
            fs::read_dir(&directory)
                .expect("published directory")
                .count(),
            1,
            "successful publication must leave no private partial name"
        );
        assert!(write_new_plan_v5(&output, &document).is_err());
        fs::remove_file(&output).expect("remove exact test output");
        fs::remove_dir(&directory).expect("remove exact test directory");
    }
}
