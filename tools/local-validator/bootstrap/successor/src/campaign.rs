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
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use dclutch_pyth_svm::devnet_release_v1;
use dclutch_registry_svm::{
    LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataMetadataV3View, ProgramDataV3View,
};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Value, json};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    model::SuccessorPlan,
    plan::{hex, pubkey},
    rpc::{Rpc, WritePolicyV1},
    runtime,
    seed::{KeyForge, role},
};

/// The acknowledgment flag's literal spelling, for the argument table.
pub(crate) const DEVNET_ACKNOWLEDGMENT_FLAG_NAME: &str = DEVNET_ACKNOWLEDGMENT_FLAG;

/// The roles a driver run must be handed a keypair file for.
///
/// Exactly the signers the stages below reach. `hostile-authority` is not here:
/// its only job is to prove a refusal, the proof costs a funded wallet and two
/// transaction fees, and a driver that silently demanded a second funded key to
/// run at all would be trading the operator's lamports for evidence they did not
/// ask for. It is opt-in through `--keypair-hostile-authority`.
pub(crate) const REQUIRED_ROLES: &[&str] = &[
    role::CORE_UPGRADE_AUTHORITY,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_FOUNDER,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
];

/// Every role a `--keypair-<role>` flag may name.
pub(crate) const KEYPAIR_ROLES: &[&str] = &[
    role::CORE_UPGRADE_AUTHORITY,
    role::HOSTILE_AUTHORITY,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_FOUNDER,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
    role::SUBSTITUTED_FOUNDER,
];

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
    /// The release activation cache exists and verifies.
    Activation,
    /// A Market exists, founded and Open.
    Founding,
}

impl StageV1 {
    /// The canonical order. A campaign runs a prefix of this and stops.
    pub(crate) const ORDER: [Self; 5] = [
        Self::Substrate,
        Self::Publication,
        Self::Initialize,
        Self::Activation,
        Self::Founding,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Substrate => "substrate",
            Self::Publication => "publication",
            Self::Initialize => "initialize",
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
        let mut conflicts = Vec::new();
        if !self.slot_pin_holds() {
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

/// The command surface, already parsed and validated.
pub(crate) struct CampaignArgsV1 {
    pub(crate) origin: ClusterOriginV1,
    pub(crate) plan_path: PathBuf,
    /// The market input the founding stage founds — the run spec's `market`
    /// block as its own JSON document. Optional because every earlier stage
    /// runs without one; the founding stage refuses by name when it is absent.
    pub(crate) market_path: Option<PathBuf>,
    pub(crate) evidence_path: Option<PathBuf>,
    pub(crate) keypairs: BTreeMap<String, [u8; 32]>,
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
fn load_market_input(bytes: &[u8]) -> Result<crate::model::MarketRunInput> {
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
    Ok(())
}

fn write_evidence_atomically(path: &Path, value: &Value) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("evidence output requires a UTF-8 file name"))?;
    let temporary = path.with_file_name(format!(".{file_name}.dclutch-{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temporary, &bytes)
        .map_err(|error| Error::new(format!("write {}: {error}", temporary.display())))?;
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically replace {} from {}: {error}",
            path.display(),
            temporary.display()
        ))
    })?;
    Ok(())
}

fn compatible_founding_checkpoint(
    path: &Path,
    plan_sha256: &str,
    market_sha256: Option<&str>,
) -> Result<Option<Value>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::new(format!(
                "read prior evidence {}: {error}",
                path.display()
            )));
        }
    };
    let prior: Value = serde_json::from_slice(&bytes)?;
    if prior.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256)
        || prior.get("market_sha256").and_then(Value::as_str) != market_sha256
    {
        return Ok(None);
    }
    Ok(prior.get("foundingCheckpoint").cloned())
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
            drifted.extend(row.pin_conflicts());
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

/// Is this market input's founding already on the chain?
///
/// Complete is the market-account core of the executor's own
/// `authenticate_open_market_poststate_v1`: the DCLTGMF1 Market exists at its
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

/// Does the release activation cache exist?
///
/// Exact cache progress is the detector. One through four byte-identical role
/// slots are an inert resume point; a complete cache is done; any mismatched
/// header, role, owner, privilege, or width is a conflict.
pub(crate) fn activation_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let address = pubkey(&plan.activation)?;
    Ok(match runtime::activation_progress(rpc, plan) {
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
    })
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
    for pair in plan.records.values() {
        let body = runtime::decode_hex(&pair.body_hex)?;
        record_rent = record_rent.saturating_add(rpc.minimum_balance(body.len())?);
    }
    let profile_rent =
        rpc.minimum_balance(runtime::decode_hex(&plan.infrastructure_profile.body_hex)?.len())?;
    // The activation cache's width is the contract's, not a number this tool
    // owns; read the account if it exists and price the plan's own profile
    // width otherwise, which is the closest honest proxy available offline.
    let activation_rent = match rpc.account(pubkey(&plan.activation)?)? {
        Some(account) => rpc.minimum_balance(account.data.len())?,
        None => profile_rent,
    };
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

/// Run the driver.
pub(crate) fn execute(args: CampaignArgsV1) -> Result<()> {
    if args.execute && args.evidence_path.is_none() {
        return Err(Error::new(
            "--execute requires --evidence ABSOLUTE_JSON so intent is durable before any mutation",
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&args.plan_path)?)?;
    let plan_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(fs::read(
        &args.plan_path,
    )?));
    let market_sha256 = args
        .market_path
        .as_ref()
        .map(|path| fs::read(path).map(|bytes| hex(&<sha2::Sha256 as sha2::Digest>::digest(bytes))))
        .transpose()?;
    let policy = if args.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&args.origin, policy)?;

    let forge = KeyForge::persisted(args.keypairs.clone(), REQUIRED_ROLES)?;
    let authority = forge.keypair(role::CORE_UPGRADE_AUTHORITY);

    // The market input, decoded and validated before any detector runs so a
    // malformed input refuses before a single RPC call, not mid-ladder.
    let market: Option<crate::model::MarketRunInput> = match &args.market_path {
        None => None,
        Some(path) => Some(load_market_input(&fs::read(path)?)?),
    };

    // Every detector, always, before anything is written. A stage that is
    // already complete is skipped; a stage in conflict stops the run.
    let (substrate, observed_roles) = substrate_state(&mut rpc, &plan)?;
    let mut states = vec![(StageV1::Substrate, substrate)];
    states.push((StageV1::Publication, publication_state(&mut rpc, &plan)?));
    states.push((StageV1::Initialize, initialize_state(&mut rpc, &plan)?));
    states.push((StageV1::Activation, activation_state(&mut rpc, &plan)?));
    // PEEKED, never drawn: the detector must look at exactly the mint key the
    // executor will draw, and drawing it here would shift the executor onto
    // the next index (the measured drift `KeyForge::peek_pubkey` documents).
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

    let wallet = wallet_arithmetic(&mut rpc, &plan, authority.pubkey())?;
    let pyth = match &args.origin {
        ClusterOriginV1::AcknowledgedDevnet { .. } => Some(authenticate_pyth_row(&mut rpc)?),
        // The committed row is a devnet fact. Authenticating it against a
        // local ledger that has never seen the Pyth programs would produce a
        // page of false negatives and teach nobody anything.
        ClusterOriginV1::Loopback { .. } => None,
    };

    let mut report = json!({
        "schema": "dclutch-successor-campaign-report-v1",
        "cluster": args.origin.label(),
        "rpc_url": args.origin.redacted_url(),
        "mode": if args.execute { "execute" } else { "preflight (reads only, enforced)" },
        "plan": args.plan_path.display().to_string(),
        "plan_sha256": plan_sha256,
        "market_input": args.market_path.as_ref().map(|path| path.display().to_string()),
        "market_sha256": market_sha256,
        "evidence_output": args.evidence_path.as_ref().map(|path| path.display().to_string()),
        "through_stage": args.through.name(),
        "execution_intent": {
            "authorized_mutation": args.execute,
            "through_stage": args.through.name(),
            "plan": args.plan_path.display().to_string(),
            "market": args.market_path.as_ref().map(|path| path.display().to_string()),
        },
        "payer": authority.pubkey().to_string(),
        "keypair_derivation": forge.derivation_label(),
        "private_key_persisted": forge.persists_private_keys(),
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
        "wallet": {
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
                "this driver never airdrops: the devnet faucet is rate-limited far below a \
                 campaign's needs, so a run that begged for lamports would fail INSIDE a ladder \
                 rather than here. Fund the payer address above before running with --execute."
            },
        },
        "pyth_devnet_release_authentication": pyth.as_ref().map(|rows| rows.iter().map(|(what, ok, detail)| json!({
            "fact": what,
            "holds": ok,
            "observed": detail,
        })).collect::<Vec<_>>()),
        "founding_targets": founding_targets.as_ref().map(|targets| json!({
            "market_input": args.market_path.as_ref().map(|path| path.display().to_string()),
            "collateral_mint": targets.collateral_mint.to_string(),
            "realm_record": targets.realm_record.to_string(),
            "found31_market": targets.found31_market.to_string(),
            "open_market": targets.open_market.to_string(),
            "abort_market": targets.abort_market.to_string(),
        })),
        "deploy_ladder": deploy_ladder(&plan, &args.origin),
        "transport_policy": "driver traffic: paced RPC (SMOKE-0 §6.4 -- the founding ladder and \
                             life are RPC-shaped end to end). Buffer writes: TPU, via the solana \
                             CLI, never this process (SMOKE-0 §3.1 -- ~100x, and it is the CLI's \
                             ladder, not the driver's).",
    });

    if let Some(path) = &args.evidence_path
        && let Some(checkpoint) = compatible_founding_checkpoint(
            path,
            report["plan_sha256"].as_str().unwrap_or_default(),
            report["market_sha256"].as_str(),
        )?
    {
        report["foundingCheckpoint"] = checkpoint;
    }

    if let Some(path) = &args.evidence_path {
        write_evidence_atomically(path, &report)?;
    }

    if !args.execute {
        let mut stdout = std::io::stdout();
        stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }
    let execution = {
        let mut checkpoint = |value: &crate::market::MarketExecutionCheckpointV1| -> Result<()> {
            report["foundingCheckpoint"] = serde_json::to_value(value)?;
            if let Some(path) = &args.evidence_path {
                write_evidence_atomically(path, &report)?;
            }
            Ok(())
        };
        execute_stages(
            &mut rpc,
            &plan,
            &authority,
            &forge,
            market.as_ref(),
            founding_keys,
            &states,
            args.through,
            &mut checkpoint,
        )?
    };
    report["execution"] = json!({
        "completed": true,
        "recoveredFinalizedFounding": execution.recovered_finalized_founding,
        "transactions": execution.transactions,
        "market": execution.market,
    });
    if execution.recovered_finalized_founding && report.get("foundingCheckpoint").is_none() {
        return Err(Error::new(
            "the chain proves this founding complete but its crash-safe evidence has no compatible founding checkpoint",
        ));
    }
    if let Some(path) = &args.evidence_path {
        write_evidence_atomically(path, &report)?;
    }
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Advance the chain through the requested stages, skipping what is done.
///
/// The stages through activation sign with the Core authority alone; the
/// founding is the one that needs the forge's other roles and the market
/// input, and it refuses by name when the input is absent.
struct CampaignExecutionEvidenceV1 {
    transactions: Vec<crate::model::TransactionEvidence>,
    market: Option<crate::market::MarketExecutionEvidence>,
    recovered_finalized_founding: bool,
}

#[allow(clippy::too_many_arguments)]
fn execute_stages(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    authority: &Keypair,
    forge: &KeyForge,
    market: Option<&crate::model::MarketRunInput>,
    founding_keys: Option<(Pubkey, Pubkey)>,
    states: &[(StageV1, StageStateV1)],
    through: StageV1,
    checkpoint: &mut dyn FnMut(&crate::market::MarketExecutionCheckpointV1) -> Result<()>,
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
    let mut recovered_finalized_founding = false;
    for (stage, state) in states {
        if *stage > through {
            break;
        }
        if *state == StageStateV1::Complete {
            eprintln!("campaign stage {}: already complete, skipped", stage.name());
            if *stage == StageV1::Founding {
                recovered_finalized_founding = true;
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
            StageV1::Activation => {
                for (label, instruction) in
                    runtime::pending_activation_instructions(rpc, plan, authority.pubkey())?
                {
                    transactions.push(rpc.send(label, &[instruction], authority)?);
                }
                runtime::verify_activation(rpc, plan)?;
            }
            StageV1::Founding => {
                let Some(input) = market else {
                    return Err(Error::new(
                        "the founding stage needs a market input: pass --market ABSOLUTE_JSON \
                         carrying the run spec's `market` block as its own document (the \
                         `demo-market` subcommand prints the local-fixture shape). Every earlier \
                         stage runs without one.",
                    ));
                };
                if let StageStateV1::Partial(detail) = state {
                    return Err(Error::new(format!(
                        "this founding has STARTED on this chain and the driver will not write \
                         into a half-founded market: {detail}. Record publication is \
                         chain-deriving and re-verifies, but the collateral, credit and market \
                         stages are one-shot, and a founding that fails midway has real \
                         principal behind it. Inspect the named accounts; found again at a fresh \
                         generation (a distinct, still-vacant Market PDA) with fresh \
                         collateral-mint/collateral-wallet keypair files, or finish this one by \
                         hand against the same input.",
                    )));
                }
                let (mint, wallet) = founding_keys.ok_or_else(|| {
                    Error::new("the founding stage reached execution without peeked keys")
                })?;
                let evidence = crate::market::execute_found_market_with_checkpoint(
                    rpc,
                    plan,
                    input,
                    authority,
                    forge,
                    &mut transactions,
                    checkpoint,
                )?;
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

    #[test]
    fn fake_runner_crash_restart_preserves_checkpoint_and_finalizes_atomically() {
        let path = std::env::temp_dir().join(format!(
            "dclutch-campaign-crash-restart-{}.json",
            Pubkey::new_unique()
        ));
        let mut first_runner = json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "plan_sha256": "11".repeat(32),
            "market_sha256": "22".repeat(32),
            "evidence_output": path.display().to_string(),
            "intent": { "execute": true, "through": "founding" },
        });
        write_evidence_atomically(&path, &first_runner).expect("durable prewrite");
        first_runner["foundingCheckpoint"] = json!({
            "market": Pubkey::new_unique().to_string(),
            "foundingCustodyContext": "33".repeat(32),
            "directSelectedManifestEntryIndex": 2,
            "direct_capability_root": Pubkey::new_unique().to_string(),
            "direct_trading_funding_ledger": Pubkey::new_unique().to_string(),
        });
        write_evidence_atomically(&path, &first_runner).expect("pre-mutation checkpoint");
        drop(first_runner); // fake process death after mutation, before final evidence update

        let checkpoint =
            compatible_founding_checkpoint(&path, &"11".repeat(32), Some(&"22".repeat(32)))
                .expect("restart reads durable evidence")
                .expect("compatible checkpoint");
        let mut restarted_runner = json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "plan_sha256": "11".repeat(32),
            "market_sha256": "22".repeat(32),
            "evidence_output": path.display().to_string(),
            "foundingCheckpoint": checkpoint,
            "execution": {
                "completed": true,
                "recoveredFinalizedFounding": true,
                "transactions": [],
            },
        });
        write_evidence_atomically(&path, &restarted_runner).expect("atomic final evidence");
        let finalized: Value =
            serde_json::from_slice(&fs::read(&path).expect("read final evidence"))
                .expect("decode final evidence");
        assert_eq!(finalized["execution"]["recoveredFinalizedFounding"], true);
        assert_eq!(
            finalized["foundingCheckpoint"]["directSelectedManifestEntryIndex"],
            2
        );
        assert!(
            compatible_founding_checkpoint(&path, &"44".repeat(32), Some(&"22".repeat(32)))
                .expect("incompatible restart")
                .is_none()
        );
        restarted_runner = Value::Null;
        drop(restarted_runner);
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

    #[test]
    fn the_stage_order_is_the_only_order_a_chain_accepts() {
        // Publication before Initialize is not a preference: Core's
        // infrastructure initialization READS the Registry and Rent artifact
        // records, and activation reads the five role records plus the release
        // set. The enum's Ord is what `execute_stages` uses to stop at
        // `--through`, so the declaration order is load-bearing.
        assert!(StageV1::Substrate < StageV1::Publication);
        assert!(StageV1::Publication < StageV1::Initialize);
        assert!(StageV1::Initialize < StageV1::Activation);
        assert!(StageV1::Activation < StageV1::Founding);
        assert_eq!(StageV1::ORDER.len(), 5);
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
    fn every_required_role_is_one_a_keypair_flag_can_name() {
        for role in REQUIRED_ROLES {
            assert!(
                KEYPAIR_ROLES.contains(role),
                "{role} is required but no flag names it"
            );
        }
        // The hostile authority is deliberately NOT required: proving a refusal
        // costs a second funded wallet and two fees the operator did not ask
        // for.
        assert!(!REQUIRED_ROLES.contains(&role::HOSTILE_AUTHORITY));
        assert!(KEYPAIR_ROLES.contains(&role::HOSTILE_AUTHORITY));
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
