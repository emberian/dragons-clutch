//! Derive the complete General `OpenBatch` hot frame from a founded Market's
//! own records, and name the first conjunct no caller can satisfy.
//!
//! # Why this module exists
//!
//! `docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md` recorded
//! `OpenBatch` as "not reachable from this tree" twice, and both times the
//! thing said to be owed was a *driver*: first "a caller that takes a
//! `--market`", then "the commit half through Trading". Cohort-14 then founded
//! and activated a General Market (`8ExdC1Rwb…`), so the market half of that
//! debt is paid and the question became what a driver would actually have to
//! state.
//!
//! This command answers it by deriving the whole frame and reporting, per
//! account, WHO PRODUCES IT on a real chain. That is the deliverable, because
//! the answer for four of the accounts is *nobody can*, and a driver written
//! before that was known would have been a driver aimed at an unreachable
//! route.
//!
//! # The caller-authority coordinate, and why this file no longer states a wall
//!
//! Trading's admitted-AOT CPI derives one caller-authority PDA per accelerator
//! invocation (`admitted_composition_v3.rs`), seeded by
//! `CallerAuthoritySeedsV1::new(release_set, market, Trading, root,
//! role_request_digest)`. It then REQUIRES the account it was handed at that
//! top-level coordinate to equal the address it just derived, or refuses
//! `TradingSbfError::Release` (`0x4001`).
//!
//! Until `3a8ac205d` the `role_request_digest` was
//! `sha256(accelerator request header ‖ inline bank)`, and `OpenBatch` is one
//! of the seven window-gated actions whose AccountProfile declares
//! `TrustedEnvironmentV2::CurrentSlot` — so Trading seeded
//! `scalar::CURRENT_SLOT` from `Clock::get()` into exactly that bank on every
//! execution, the digest moved every slot, and the address had to be in an
//! account list that is fixed when the transaction is signed. No caller could
//! state it. That was a real wall and this command reported it.
//!
//! `3a8ac205d` moved the preimage to
//! `accelerator_caller_authority_digest_v1(kind, parent_request_digest, index)`
//! over the digest of the SIGNED `DCLTHOT3` family request and the invocation
//! ordinal. No trusted-environment scalar enters any seed, so the span is a
//! function of the signed instruction alone and a caller can name it.
//!
//! **This file used to publish that wall as a hardcoded verdict**, pushed
//! unconditionally with a `detail` describing the pre-`3a8ac205d` seed — so it
//! kept reporting a wall the deployed bytes no longer had, and could not go
//! red if the tree changed again. It now DERIVES the span through the same two
//! authors the program calls, and the row appears only when a derivation
//! actually fails. A hex constant typed into a checker agrees right up until
//! the tree moves; a hardcoded verdict is worse, because nothing can disagree
//! with it.
//!
//! # What this command is, and is not
//!
//! Read-only. It reads no keypair, signs nothing, submits nothing, and calls
//! no write RPC method. Its output is one JSON report and one refusal.
//!
//! Every semantic fact is re-derived from the Market rather than supplied:
//! the manifest record address comes from the Market's own
//! `capability_manifest` identity, the General entry is FOUND by kind, the
//! ProgramSet comes from that entry's `release_id`, the `OpenBatch` descriptor
//! is SELECTED out of that set by a canonical family request, and the six
//! artifact record pairs come from the identities that on-chain descriptor
//! carries. Two record addresses are untrusted routing hints — the Product
//! graph's ResultDomain and Portfolio, which the Market header does not name —
//! and both are reauthenticated by
//! `authenticate_product_graph_observation_v3` before anything is reported.
//!
//! The published AccountProfile's external widths are RECOVERED from the
//! chain's own bytes rather than read from the policy file that produced them:
//! two encodings that differ only in one width locate that width's offset, and
//! the value is then read out of the finalized record. A recovered width set
//! that does not re-encode to the exact published bytes is a refusal.

use std::path::PathBuf;

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3,
        HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_CONFIG_COORDINATE_V3,
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, HOT_RUNTIME_PRODUCT_COORDINATE_V3,
        HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        HOT_TRANSITION_RAW_ACCOUNT_V3,
    },
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_capability_seal_contract::CapabilitySealKeyV1;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    admitted_v3::{
        ADMITTED_RUNTIME_ACCOUNTS_START_V3, ADMITTED_STRATEGY_EVIDENCE_COUNT_V3,
        ADMITTED_STRATEGY_EVIDENCE_START_V3,
    },
    shadow_digest_v3::{AcceleratorCallerKindV1, accelerator_caller_authority_digest_v1},
    v2::{
        BankTransportV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
        StrategyDispositionV2, classify_bank_transport_v2,
    },
};
use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3, general_account_profile_fixed_count_v3,
    },
    artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_scalar_count_v3,
    },
    release_v3::authenticate_general_program_set_v3,
    state_artifacts_v3::{
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, general_system_program_account_v3,
    },
};
use dclutch_general_codec::{Action, successor_request_v2::CONTROLLER_REQUEST_BYTES_V2};
use dclutch_general_config_contract::GENERAL_CAPABILITY_KIND_ID_V1;
use dclutch_market_core_codec::{CoreState, Phase as CorePhase};
use dclutch_operator::resolution_core_v3::product_graph_observation_v3::{
    FinalizedProductGraphAccountsV3, authenticate_product_graph_observation_v3,
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1;
use dclutch_registry_contract::ActivatedExecutionReleaseSetV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_program::pubkey::Pubkey;
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) const DEVNET_GENERAL_SESSION_COMMAND_V1: &str = "devnet-general-session";
const REPORT_SCHEMA_V1: &str = "dclutch-devnet-general-session-frame-report-v1";

/// The action this command frames. `OpenBatch` is the first act of the General
/// batch lifecycle, so it is the one whose reachability decides the family's.
const SESSION_ACTION_V1: Action = Action::OpenBatch;

/// The preimage of the stated probe family-request digest.
///
/// It is a fixed string rather than a random or chain-derived value so that two
/// runs of this read-only command against one market report the same probe
/// span, and so that a reader can tell a probe address from a real one by
/// recomputing it. It is never presented as the caller's own digest: the frame
/// row and the report both say which was used.
const CALLER_AUTHORITY_PROBE_PREIMAGE_V1: &[u8] =
    b"dclutch:devnet-general-session:caller-authority-probe:v1";

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-general-session \
     --rpc-url URL --i-mean-devnet GENESIS_HASH \
     --plan ABSOLUTE_JSON --market GENERAL_OPEN_MARKET \
     --result-domain-record ADDRESS --portfolio-record ADDRESS \
     --linked-basis-record ADDRESS \
     --payer PUBKEY --output ABSOLUTE_NEW_JSON \
     [--parent-request-digest HEX64]\n     \
     Read-only. Derives the complete General OpenBatch hot frame from the \
     Market's own records, recovers the published AccountProfile's external \
     widths from the chain's bytes, and reports each account's producer. \
     --parent-request-digest is the digest of the signed DCLTHOT3 family \
     request; supplied, the admitted caller-authority span is reported at the \
     exact addresses Trading will require."
}

struct ArgumentsV1 {
    rpc_url: String,
    acknowledgment: String,
    plan: PathBuf,
    market: Pubkey,
    result_domain_record: Pubkey,
    portfolio_record: Pubkey,
    linked_basis_record: Pubkey,
    payer: Pubkey,
    output: PathBuf,
    /// The digest of the signed `DCLTHOT3` family request the caller intends,
    /// when the caller has one. It is the only coordinate of the admitted
    /// caller-authority span this read-only command cannot observe, because
    /// the span is a function of the SIGNED instruction and this command signs
    /// nothing. Supplied, the reported addresses are the exact ones Trading
    /// will require; omitted, the span is still derived — against a stated
    /// probe digest — so that a derivation that has stopped working is a
    /// refusal rather than a silence.
    parent_request_digest: Option<ContentId>,
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market = None;
    let mut result_domain_record = None;
    let mut portfolio_record = None;
    let mut linked_basis_record = None;
    let mut payer = None;
    let mut output = None;
    let mut parent_request_digest = None;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| refusal("input/missing-value", format!("{flag}; usage: {}", usage())))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--market" => &mut market,
            "--result-domain-record" => &mut result_domain_record,
            "--portfolio-record" => &mut portfolio_record,
            "--linked-basis-record" => &mut linked_basis_record,
            "--payer" => &mut payer,
            "--output" => &mut output,
            "--parent-request-digest" => &mut parent_request_digest,
            other => return Err(refusal("input/unknown-flag", other)),
        };
        if slot.replace(value).is_some() {
            return Err(refusal("input/repeated-flag", flag));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| refusal("input/missing-flag", format!("{name}; usage: {}", usage())))
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        acknowledgment: required(acknowledgment, DEVNET_ACKNOWLEDGMENT_FLAG)?,
        plan: PathBuf::from(required(plan, "--plan")?),
        market: pubkey(&required(market, "--market")?)?,
        result_domain_record: pubkey(&required(result_domain_record, "--result-domain-record")?)?,
        portfolio_record: pubkey(&required(portfolio_record, "--portfolio-record")?)?,
        linked_basis_record: pubkey(&required(linked_basis_record, "--linked-basis-record")?)?,
        payer: pubkey(&required(payer, "--payer")?)?,
        output: PathBuf::from(required(output, "--output")?),
        parent_request_digest: parent_request_digest
            .map(|value| content_id_from_hex_v1(&value))
            .transpose()?,
    })
}

/// Accept one 64-character lowercase-hex content identity.
///
/// Lowercase and exact width are required rather than normalized: a digest a
/// caller retyped in another case is a digest a caller may have retyped
/// wrongly, and the span this seeds has no other check on it.
fn content_id_from_hex_v1(value: &str) -> Result<ContentId> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refusal(
            "input/parent-request-digest",
            "--parent-request-digest takes exactly 64 hexadecimal characters",
        ));
    }
    if value.chars().any(|character| character.is_ascii_uppercase()) {
        return Err(refusal(
            "input/parent-request-digest",
            "--parent-request-digest must be lowercase hexadecimal",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| refusal("input/parent-request-digest", error.to_string()))?;
    }
    ContentId::new(bytes).map_err(|_| {
        refusal(
            "input/parent-request-digest",
            "the reserved all-zero content identity is not a family request digest",
        )
    })
}

/// The raw and staging coordinates of one finalized record.
///
/// The two seed domains are not spelled here: `dclutch-record-contract` owns
/// them and exports the constructors that place them, so a module that merely
/// READS these addresses takes each domain from `seeds.domain()`.
#[derive(Clone, Copy, Debug)]
struct RecordCoordinateV1 {
    raw: Pubkey,
    staging: Pubkey,
}

fn record_coordinate(
    registry: &Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<RecordCoordinateV1> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|_| refusal("session/record-schema", "a record schema was all zero"))?,
        ContentDigest::new(content)
            .map_err(|_| refusal("session/record-identity", "a record identity was all zero"))?,
    );
    Ok(RecordCoordinateV1 {
        raw: record_address(registry, key.raw_record_pda_seeds()),
        staging: record_address(registry, key.staging_cursor_pda_seeds()),
    })
}

fn record_address(registry: &Pubkey, seeds: RecordPdaSeedsV1) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        registry,
    )
    .0
}

/// One frame coordinate, its author on a real chain, and what the chain holds.
struct FrameRowV1 {
    coordinate: usize,
    label: &'static str,
    address: Pubkey,
    author: String,
    observed: Option<ObservedShapeV1>,
}

struct ObservedShapeV1 {
    owner: Pubkey,
    bytes: usize,
    executable: bool,
    lamports: u64,
}

impl FrameRowV1 {
    fn to_json(&self) -> Value {
        json!({
            "coordinate": self.coordinate,
            "label": self.label,
            "address": self.address.to_string(),
            "author": self.author,
            "observed": self.observed.as_ref().map_or(Value::Null, |shape| json!({
                "owner": shape.owner.to_string(),
                "bytes": shape.bytes,
                "executable": shape.executable,
                "lamports": shape.lamports,
            })),
        })
    }
}

/// Recover one external width from the chain's own AccountProfile bytes.
///
/// Two encodings that differ in exactly one width locate that width's offset;
/// the published value is then read there. This is a recovery rather than an
/// assumption: the caller re-encodes with every recovered width and requires
/// the result to equal the finalized record byte-for-byte.
fn recover_width_v1(
    published: &[u8],
    base: GeneralExternalAccountWidthsV3,
    set: impl Fn(&mut GeneralExternalAccountWidthsV3, u32),
    label: &str,
) -> Result<u32> {
    let mut low = base;
    set(&mut low, 0x0101_0101);
    let mut high = base;
    set(&mut high, 0x0202_0202);
    let low = encode_profile_v1(low)?;
    let high = encode_profile_v1(high)?;
    if low.len() != published.len() || high.len() != published.len() {
        return Err(refusal(
            "session/profile-width",
            format!(
                "the published AccountProfile is {} bytes; this tree encodes {} for the same action",
                published.len(),
                low.len()
            ),
        ));
    }
    let mut offsets = low
        .iter()
        .zip(high.iter())
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(index, _)| index);
    let first = offsets.next().ok_or_else(|| {
        refusal(
            "session/width-unlocatable",
            format!("{label} moves no byte of the encoded AccountProfile"),
        )
    })?;
    // The two probes differ in all four bytes of one little-endian u32, so the
    // differing run must be exactly four contiguous bytes. Anything else means
    // the width reaches more than one field and this recovery does not apply.
    for expected in 1..4 {
        if offsets.next() != Some(first + expected) {
            return Err(refusal(
                "session/width-unlocatable",
                format!("{label} does not occupy one contiguous little-endian u32"),
            ));
        }
    }
    if offsets.next().is_some() {
        return Err(refusal(
            "session/width-unlocatable",
            format!("{label} reaches more than one field of the encoded AccountProfile"),
        ));
    }
    let bytes: [u8; 4] = published
        .get(first..first + 4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| {
            refusal(
                "session/width-unlocatable",
                "width offset outside the record",
            )
        })?;
    Ok(u32::from_le_bytes(bytes))
}

fn encode_profile_v1(widths: GeneralExternalAccountWidthsV3) -> Result<Vec<u8>> {
    let bytes = general_account_profile_bytes_v3(SESSION_ACTION_V1)
        .map_err(|error| Error::new(format!("General AccountProfile width: {error:?}")))?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(SESSION_ACTION_V1, widths, &mut scratch, &mut output)
        .map_err(|error| Error::new(format!("General AccountProfile encode: {error:?}")))?;
    Ok(output)
}

/// The width set every recovery probe starts from.
///
/// Only the widths this action's profile actually reaches are recovered; the
/// rest are nonzero placeholders, because `GeneralExternalAccountWidthsV3`
/// refuses a zero and a width the profile never reads cannot move a byte.
const PROBE_WIDTHS_V1: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
    linked_basis_prefix: 1,
    result_domain: 1,
    rent_sysvar: 1,
    core_market: 1,
    activation_cache: 1,
    upgradeable_program: 1,
    trading_programdata_prefix: 1,
    claims_programdata_prefix: 1,
    core_programdata_prefix: 1,
    realm_record: 1,
    rent_credit: 1,
};

/// Derive and report the General OpenBatch frame on acknowledged devnet.
pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    if arguments.output.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite {}", arguments.output.display()),
        ));
    }
    // The origin rail runs before anything is read: it is what makes an
    // accidental mainnet endpoint impossible, and it costs nothing.
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(&arguments.acknowledgment))?;
    ExpectedClusterV1::Devnet.authenticate(&origin)?;
    let plan_bytes = std::fs::read(&arguments.plan)
        .map_err(|error| refusal("input/unreadable", format!("plan: {error}")))?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan: {error}")))?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;

    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let activation_cache = pubkey(&plan.activation)?;
    let accelerator_pin = plan.general_accelerator.as_ref().ok_or_else(|| {
        refusal(
            "session/no-accelerator",
            "the plan names no General accelerator deployment",
        )
    })?;
    let accelerator = pubkey(&accelerator_pin.program_id)?;
    let accelerator_programdata = pubkey(&accelerator_pin.programdata_id)?;

    // ------------------------------------------------------------ the Market
    let market_account = rpc.required_account(arguments.market, "Core Market state")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market state: {error:?}")))?;
    if market_account.owner != core || market_state.phase != CorePhase::Open {
        return Err(refusal(
            "session/market-phase",
            format!(
                "market {} is {:?}, owner {}",
                arguments.market, market_state.phase, market_account.owner
            ),
        ));
    }
    let generation = market_state.identity.generation;
    let release_set = market_state.identity.selected_release_set;
    let manifest_id = market_state.identity.capability_manifest.to_bytes();

    // ------------------------------------------- the manifest, and the entry
    let manifest_record = record_coordinate(
        &registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_id,
    )?;
    let manifest_body = read_record(
        &mut rpc,
        &registry,
        manifest_record,
        manifest_id,
        "capability manifest",
    )?;
    let manifest = CapabilityManifestV1::decode(&manifest_body)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let mut selected: Option<u16> = None;
    for index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|error| Error::new(format!("manifest entry {index}: {error:?}")))?;
        if entry.kind_id().to_bytes() == GENERAL_CAPABILITY_KIND_ID_V1
            && selected.replace(index).is_some()
        {
            return Err(refusal(
                "session/ambiguous-entry",
                "the manifest carries two General entries",
            ));
        }
    }
    let entry_index = selected.ok_or_else(|| {
        refusal(
            "session/no-general-entry",
            format!("market {} selected no General capability", arguments.market),
        )
    })?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;

    // --------------------------- the published set, and the OpenBatch program
    let program_set_record = record_coordinate(
        &registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        entry.release_id().to_bytes(),
    )?;
    let program_set_body = read_record(
        &mut rpc,
        &registry,
        program_set_record,
        entry.release_id().to_bytes(),
        "General ProgramSet",
    )?;
    let (set, profile) = authenticate_general_program_set_v3(
        entry.release_id().to_bytes(),
        sha256(&program_set_body),
        &program_set_body,
    )
    .map_err(|error| Error::new(format!("General ProgramSet: {error:?}")))?;

    // The descriptor is SELECTED by a canonical family request whose only
    // meaningful byte is the action selector, exactly as Trading selects it.
    let mut selector_request = vec![0_u8; CONTROLLER_REQUEST_BYTES_V2];
    *selector_request
        .get_mut(
            usize::try_from(GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3).unwrap_or(usize::MAX),
        )
        .ok_or_else(|| Error::new("controller selector offset".to_string()))? =
        SESSION_ACTION_V1 as u8;
    let descriptor_reference = set
        .select_descriptor(&selector_request)
        .map_err(|error| Error::new(format!("OpenBatch descriptor selection: {error:?}")))?;
    if descriptor_reference.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4 {
        return Err(refusal(
            "session/descriptor-schema",
            "the selected OpenBatch entry does not carry the V4 descriptor schema",
        ));
    }
    let descriptor_record = record_coordinate(
        &registry,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        descriptor_reference.program().to_bytes(),
    )?;
    let descriptor_body = read_record(
        &mut rpc,
        &registry,
        descriptor_record,
        descriptor_reference.program().to_bytes(),
        "General OpenBatch descriptor",
    )?;
    let descriptor = CapabilityProgramV4::decode(&descriptor_body)
        .map_err(|error| Error::new(format!("General OpenBatch descriptor: {error:?}")))?;

    // ------------------------------------------- the six artifact coordinates
    let config_record = record_coordinate(
        &registry,
        descriptor.config_schema().to_bytes(),
        entry.config_id().to_bytes(),
    )?;
    let artifact_record =
        |reference: dclutch_capability_program_contract::v4::ArtifactReferenceV4| {
            record_coordinate(
                &registry,
                reference.schema().to_bytes(),
                reference.program().to_bytes(),
            )
        };
    let account_profile_record = artifact_record(descriptor.account_profile())?;
    let request_profile_record = artifact_record(descriptor.request_profile())?;
    let transition_record = artifact_record(descriptor.transition())?;
    let effect_record = artifact_record(descriptor.effect())?;
    let lifecycle_record = artifact_record(descriptor.lifecycle())?;
    let strategy_record = artifact_record(descriptor.strategy())?;

    // ------------------------------------------------- the Product graph
    let product_record = record_coordinate(
        &registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market_state.identity.product_record.to_bytes(),
    )?;
    let (result_domain_record, portfolio_record) = hinted_graph_records_v1(
        &mut rpc,
        &registry,
        arguments.result_domain_record,
        arguments.portfolio_record,
    )?;

    // ------------------------------------------------------------- the root
    let selection = dclutch_release_set_contract::CapabilityExecutionSelectionV1::new(
        entry_index,
        ContentId::new(manifest_id).map_err(|_| Error::new("manifest identity".to_string()))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set.to_bytes())
            .map_err(|_| Error::new("release set".to_string()))?,
        arguments.market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|error| Error::new(format!("root header: {error:?}")))?;
    let (root, _) = Pubkey::find_program_address(&root_header.seeds().as_slices(), &trading);

    // --------------------------------------------------- the fixed 39 frame
    let mut fixed = vec![Pubkey::default(); HOT_FIXED_ACCOUNT_COUNT_V3];
    place(&mut fixed, HOT_MARKET_ACCOUNT_V3, arguments.market)?;
    place(&mut fixed, HOT_ROOT_ACCOUNT_V3, root)?;
    for (raw_index, coordinate) in [
        (HOT_MANIFEST_RAW_ACCOUNT_V3, manifest_record),
        (HOT_PROGRAM_SET_RAW_ACCOUNT_V3, program_set_record),
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, descriptor_record),
        (HOT_CONFIG_RAW_ACCOUNT_V3, config_record),
        (HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, account_profile_record),
        (HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, request_profile_record),
        (HOT_TRANSITION_RAW_ACCOUNT_V3, transition_record),
        (HOT_EFFECT_RAW_ACCOUNT_V3, effect_record),
        (HOT_LIFECYCLE_RAW_ACCOUNT_V3, lifecycle_record),
        (HOT_STRATEGY_RAW_ACCOUNT_V3, strategy_record),
        (HOT_PRODUCT_RAW_ACCOUNT_V3, product_record),
        (HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, result_domain_record),
        (HOT_PORTFOLIO_RAW_ACCOUNT_V3, portfolio_record),
    ] {
        place(&mut fixed, raw_index, coordinate.raw)?;
        place(&mut fixed, raw_index + 1, coordinate.staging)?;
    }
    place(
        &mut fixed,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        activation_cache,
    )?;
    place(&mut fixed, HOT_CORE_PROGRAM_ACCOUNT_V3, core)?;
    place(
        &mut fixed,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        core_programdata,
    )?;
    place(&mut fixed, HOT_TRADING_PROGRAM_ACCOUNT_V3, trading)?;
    place(
        &mut fixed,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        trading_programdata,
    )?;
    place(&mut fixed, HOT_REGISTRY_PROGRAM_ACCOUNT_V3, registry)?;
    place(&mut fixed, HOT_RENT_SYSVAR_ACCOUNT_V3, sysvar::rent::ID)?;
    place(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        sysvar::instructions::ID,
    )?;

    // The linked-basis pair is derived from the graph, which is authenticated
    // below; its address needs the graph's own liability-basis identity, so it
    // is filled after the snapshot rather than here.

    // ------------------------------------------------------- one observation
    //
    // A vacant staging cursor is not an omission: it is exactly what a closed
    // publication ladder leaves behind, and every record authenticator in this
    // tree requires that shape (System-owned, zero data). `getMultipleAccounts`
    // reports it as absent, so the snapshot synthesizes it rather than refusing.
    let mut snapshot_addresses = fixed.clone();
    snapshot_addresses.truncate(HOT_LINKED_BASIS_RAW_ACCOUNT_V3);
    let (_, first_pass) = finalized_frame_v1(&mut rpc, &snapshot_addresses)?;
    let by_key = |observed: &'_ [ObservedAccount], key: Pubkey| {
        observed
            .iter()
            .find(|account| account.key == key)
            .cloned()
            .ok_or_else(|| Error::new(format!("snapshot omitted {key}")))
    };
    let graph = authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: registry,
        product_raw: &by_key(&first_pass, product_record.raw)?,
        product_staging: &by_key(&first_pass, product_record.staging)?,
        domain_raw: &by_key(&first_pass, result_domain_record.raw)?,
        domain_staging: &by_key(&first_pass, result_domain_record.staging)?,
        portfolio_raw: &by_key(&first_pass, portfolio_record.raw)?,
        portfolio_staging: &by_key(&first_pass, portfolio_record.staging)?,
    })
    .map_err(|error| {
        refusal(
            "session/product-graph",
            format!("the two supplied graph hints do not reauthenticate: {error:?}"),
        )
    })?;
    let tail_count = graph.outcome_count;
    // THE GRADED-BASIS RECORD IS NOT KEYED BY THE SEMANTIC BASIS IDENTITY.
    // `graph.liability_basis_id` is the semantic identity the Product graph
    // joins on; the record's own address is keyed by SHA-256 of its bytes, like
    // every other raw record. Deriving it from the semantic id lands on a
    // vacant PDA, which is exactly what this command reported before the hint
    // existed -- a wrong address that reads as a missing account.
    let linked_basis_record = hinted_record_v1(
        &mut rpc,
        &registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        arguments.linked_basis_record,
        "linked liability basis",
    )?;
    place(
        &mut fixed,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        linked_basis_record.raw,
    )?;
    place(
        &mut fixed,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3 + 1,
        linked_basis_record.staging,
    )?;

    // ------------------------------------------------------ the capability seal
    //
    // Its address is derivable and its route exists; only the host builder is
    // missing. The Trading semantic release the key is bound to is read out of
    // the activation cache, which is the same account the on-chain route reads
    // it from.
    let activation_bytes = by_key(&first_pass, activation_cache)?.data;
    let activated = ActivatedExecutionReleaseSetV1::decode(&activation_bytes)
        .map_err(|error| Error::new(format!("activation cache: {error:?}")))?;
    let trading_semantic_release = activated
        .role(ExecutionRoleV1::Trading)
        .release()
        .semantic_release_id()
        .to_bytes();
    let seal_key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        descriptor_reference.program().to_bytes(),
        u32::from(SESSION_ACTION_V1 as u8),
        trading_semantic_release,
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("capability seal key: {error:?}")))?;
    let seal_seeds = seal_key.seeds();
    let (capability_seal, _) = Pubkey::find_program_address(&seal_seeds.as_slices(), &trading);
    place(&mut fixed, HOT_CAPABILITY_SEAL_ACCOUNT_V3, capability_seal)?;

    // ------------------------------------------------- the strategy evidence
    let strategy_body = read_record(
        &mut rpc,
        &registry,
        strategy_record,
        descriptor.strategy().program().to_bytes(),
        "General OpenBatch strategy",
    )?;
    let strategy = ExecutionStrategyProgramV2::decode(&strategy_body)
        .map_err(|error| Error::new(format!("General OpenBatch strategy: {error:?}")))?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot {
        return Err(refusal(
            "session/disposition",
            format!(
                "the published OpenBatch strategy is {:?}; this command frames the admitted-AOT route",
                strategy.disposition()
            ),
        ));
    }
    let transport = strategy
        .transport_profile()
        .map_err(|error| Error::new(format!("accelerator transport: {error:?}")))?;
    let certificate_id = strategy.certificate_program().ok_or_else(|| {
        refusal(
            "session/certificate",
            "an admitted-AOT strategy with no certificate",
        )
    })?;
    let admission_id = strategy.admission_program().ok_or_else(|| {
        refusal(
            "session/admission",
            "an admitted-AOT strategy with no admission",
        )
    })?;
    let certificate_record = record_coordinate(
        &registry,
        strategy.certificate_schema().to_bytes(),
        certificate_id.to_bytes(),
    )?;
    let admission_record = record_coordinate(
        &registry,
        strategy.admission_schema().to_bytes(),
        admission_id.to_bytes(),
    )?;
    let certificate_body = read_record(
        &mut rpc,
        &registry,
        certificate_record,
        certificate_id.to_bytes(),
        "General OpenBatch certificate",
    )?;
    let certificate = ExecutionStrategyCertificateV2::decode(&certificate_body)
        .map_err(|error| Error::new(format!("General OpenBatch certificate: {error:?}")))?;
    let artifact_release = certificate
        .artifact_release()
        .map_err(|error| Error::new(format!("accelerator ArtifactRelease: {error:?}")))?;
    let artifact_record_pair = record_coordinate(
        &registry,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        artifact_release.to_bytes(),
    )?;

    // ----------------------------------- the published widths, recovered
    let published_profile = read_record(
        &mut rpc,
        &registry,
        account_profile_record,
        descriptor.account_profile().program().to_bytes(),
        "General OpenBatch AccountProfile",
    )?;
    let linked_basis_prefix = recover_width_v1(
        &published_profile,
        PROBE_WIDTHS_V1,
        |widths, value| widths.linked_basis_prefix = value,
        "linked_basis_prefix",
    )?;
    let rent_credit = recover_width_v1(
        &published_profile,
        PROBE_WIDTHS_V1,
        |widths, value| widths.rent_credit = value,
        "rent_credit",
    )?;
    let recovered = GeneralExternalAccountWidthsV3 {
        linked_basis_prefix,
        rent_credit,
        ..PROBE_WIDTHS_V1
    };
    if encode_profile_v1(recovered)? != published_profile {
        return Err(refusal(
            "session/width-recovery",
            "the recovered widths do not re-encode to the AccountProfile the chain holds",
        ));
    }

    // ------------------------------------------------------- the frame report
    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(SESSION_ACTION_V1)
            .map_err(|error| Error::new(format!("General account geometry: {error:?}")))?,
    );
    let invocation_count = admitted_invocation_count_v1(tail_count)?;
    let strategy_account_count = ADMITTED_STRATEGY_EVIDENCE_COUNT_V3 + invocation_count;
    let runtime_suffix_count = fixed_count
        .checked_sub(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
        .ok_or_else(|| Error::new("runtime suffix width".to_string()))?;
    let top_level_count =
        HOT_FIXED_ACCOUNT_COUNT_V3 + strategy_account_count + runtime_suffix_count;

    // THE CALLER-AUTHORITY SPAN, DERIVED BEFORE ANYTHING REPORTS ON IT.
    //
    // Attempted, not asserted. A derivation that fails becomes a wall row with
    // the failure's own words below; a derivation that succeeds leaves no row
    // at all. The predecessor of this code pushed the row unconditionally with
    // a `detail` describing a preimage `3a8ac205d` had already replaced, so it
    // reported a wall the deployed bytes did not have and no tree change could
    // ever have made it disagree.
    //
    // When the caller has not stated its family request digest this command
    // cannot know the exact addresses -- it signs nothing -- so it derives
    // against a STATED probe. The probe proves the route is derivable and that
    // both authors still accept these coordinates; it does not claim to be the
    // caller's span, and the row says which it is.
    let caller_authority_digest_is_probe = arguments.parent_request_digest.is_none();
    let parent_request_digest = match arguments.parent_request_digest {
        Some(digest) => digest,
        None => ContentId::new(
            Sha256::digest(CALLER_AUTHORITY_PROBE_PREIMAGE_V1)
                .as_slice()
                .try_into()
                .map_err(|_| Error::new("probe digest width".to_string()))?,
        )
        .map_err(|_| Error::new("probe digest is the reserved zero identity".to_string()))?,
    };
    let caller_authority_span = admitted_caller_authority_span_v1(
        trading,
        release_set.to_bytes(),
        arguments.market,
        root,
        parent_request_digest,
        invocation_count,
    );

    let mut report_addresses = fixed.clone();
    report_addresses.extend_from_slice(&[
        certificate_record.raw,
        certificate_record.staging,
        admission_record.raw,
        admission_record.staging,
        artifact_record_pair.raw,
        artifact_record_pair.staging,
        accelerator,
        accelerator_programdata,
    ]);
    let (observed_slot, observed) = finalized_frame_v1(&mut rpc, &report_addresses)?;
    let rows = frame_rows_v1(
        &fixed,
        &observed,
        FrameAuthorsV1 {
            entry_index,
            registry,
            trading,
            accelerator,
            accelerator_programdata,
            certificate: certificate_record,
            admission: admission_record,
            artifact: artifact_record_pair,
            invocation_count,
            caller_authorities: caller_authority_span
                .as_ref()
                .cloned()
                .unwrap_or_default(),
            caller_authority_digest_is_probe,
        },
    );

    // --------------------------------------------- the conjuncts nobody meets
    //
    // EVERY wall, not the first one. An ordering that reports only the earliest
    // refusal is how the activation lane's two cluster checks became one
    // (`a34bfb7b`): the second was real, reachable and invisible. Both of these
    // are real, they have different remedies, and a reader who fixes only the
    // one printed would find the other on the next run.
    let mut walls: Vec<Value> = Vec::new();
    if rent_credit != u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).unwrap_or(u32::MAX) {
        walls.push(json!({
            "code": "session/rent-credit-width",
            "kind": "founding input",
            "coordinate": format!("runtime {GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3}"),
            "conjunct": "AccountProfileV2 prestate Exact(data_length) at the RentCredit coordinate",
            "required": rent_credit,
            "protocolProduces": LIFECYCLE_RENT_CREDIT_BYTES_V2,
            "detail": format!(
                "the Market's published OpenBatch AccountProfile requires an EXACTLY {rent_credit}-byte \
                 account at the RentCredit coordinate; the only RentCredit this protocol produces is \
                 {LIFECYCLE_RENT_CREDIT_BYTES_V2} bytes (LIFECYCLE_RENT_CREDIT_BYTES_V2). The width is a \
                 compile-time INPUT to `devnet-general-market`, not an observation of this cohort, so it \
                 is fixed by re-founding rather than by any producer. Nothing on the OpenBatch commit \
                 path decodes this account -- `apply_lifecycle_creates_v3` touches only the state, the \
                 payer and System -- so the width binds no authority here; it is a coordinate whose only \
                 effect is to refuse."
            ),
            "remedy": "re-found the General market with widths observed from the cohort, not from the \
                       unit-test fixture in account_rules_v3.rs",
        }));
    }
    if let Err(error) = &caller_authority_span {
        walls.push(json!({
            "code": "session/caller-authority-derivation",
            "kind": "protocol",
            "coordinate": format!(
                "top-level {}..{}",
                HOT_FIXED_ACCOUNT_COUNT_V3 + ADMITTED_STRATEGY_EVIDENCE_COUNT_V3,
                HOT_FIXED_ACCOUNT_COUNT_V3 + ADMITTED_STRATEGY_EVIDENCE_COUNT_V3
                    + invocation_count - 1
            ),
            "conjunct": "caller_authority.key != &expected_authority",
            "refusal": "TradingSbfError::Release 0x4001 (admitted_composition_v3.rs)",
            "detail": format!(
                "the {invocation_count} admitted caller authorities could not be derived \
                 through the authors Trading itself calls -- \
                 accelerator_caller_authority_digest_v1(Admitted, parent_request_digest, \
                 index) seeded into CallerAuthoritySeedsV1 -- so no caller can state the \
                 top-level account list this route requires. The derivation reported: \
                 {error}"
            ),
            "remedy": "read the derivation's own message; a caller-authority span that \
                       cannot be derived is a change in one of those two authors, not a \
                       property of this market",
        }));
    }

    let report = json!({
        "schema": REPORT_SCHEMA_V1,
        "cluster": "devnet",
        "rpcUrl": origin.redacted_url(),
        "market": arguments.market.to_string(),
        "action": format!("{SESSION_ACTION_V1:?}"),
        "observedSlot": observed_slot,
        "releaseSet": hex(&release_set.to_bytes()),
        "generation": generation.to_string(),
        "entryIndex": entry_index,
        "releaseProfile": format!("{profile:?}"),
        "root": root.to_string(),
        "productOutcomeCount": tail_count,
        "transportProfile": format!("{transport:?}"),
        "acceleratorInvocationCount": invocation_count,
        "callerAuthority": {
            "parentRequestDigest": hex(parent_request_digest.as_bytes()),
            "parentRequestDigestIsProbe": caller_authority_digest_is_probe,
            "preimage": "accelerator_caller_authority_digest_v1(Admitted, parent_request_digest, index)",
            "span": caller_authority_span
                .as_ref()
                .map(|span| {
                    span.iter().map(|key| key.to_string()).collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            "derived": caller_authority_span.is_ok(),
        },
        "acceleratorProgram": accelerator.to_string(),
        "acceleratorArtifactRelease": hex(&artifact_release.to_bytes()),
        "frame": {
            "fixedAccounts": HOT_FIXED_ACCOUNT_COUNT_V3,
            "strategyAccounts": strategy_account_count,
            "runtimeSuffixAccounts": runtime_suffix_count,
            "topLevelAccounts": top_level_count,
            "acceleratorCpiAccounts": ADMITTED_RUNTIME_ACCOUNTS_START_V3 + fixed_count,
            "inlineBankBytes": general_hot_candidate_bank_len_v3(SESSION_ACTION_V1, tail_count)
                .map_err(|error| Error::new(format!("bank width: {error:?}")))?,
            "scalarCount": general_hot_scalar_count_v3(SESSION_ACTION_V1, tail_count)
                .map_err(|error| Error::new(format!("scalar count: {error:?}")))?,
            "identityCount": GENERAL_HOT_COMMON_IDENTITIES_V3,
        },
        "publishedExternalWidths": {
            "linkedBasisPrefix": linked_basis_prefix,
            "rentCredit": rent_credit,
            "recoveredFrom": "the finalized AccountProfile record's own bytes",
        },
        "protocolRentCreditBytes": LIFECYCLE_RENT_CREDIT_BYTES_V2,
        "accounts": rows.iter().map(FrameRowV1::to_json).collect::<Vec<_>>(),
        "runtimeVector": runtime_vector_json_v1(
            fixed_count,
            rent_credit,
            arguments.payer,
        )?,
        "walls": walls,
    });
    std::fs::write(
        &arguments.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .map_err(|error| Error::new(format!("frame report: {error}")))?;

    // AN EMPTY WALL LIST IS A PASS, AND IT HAS TO BE SAYABLE.
    //
    // This command was written when the answer for four of the accounts was
    // "nobody can", so its only exits were a refusal and a refusal. With the
    // rent-credit width re-founded (cohort-15) and the caller-authority seed
    // moved off the executing slot (`3a8ac205d`), `walls` can now be empty --
    // and the tail below reported that as
    // `[session/unreachable] ... 0 unsatisfiable conjunct(s)`, which is a
    // checker that cannot say the thing it exists to detect. A gate whose
    // green is spelled as a refusal is a gate nobody can put in front of a
    // producer.
    if walls.is_empty() {
        println!(
            "DELIVERABLE: OpenBatch names no unsatisfiable conjunct at any of the {} \
             top-level coordinates of market {}; frame report {}",
            top_level_count,
            arguments.market,
            arguments.output.display()
        );
        return Ok(());
    }

    let first = walls
        .first()
        .and_then(|wall| wall.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("session/unreachable")
        .to_owned();
    Err(refusal(
        &first,
        format!(
            "OpenBatch is not deliverable against market {}: {} unsatisfiable conjunct(s), every one \
             of them written to the frame report at {}. {}",
            arguments.market,
            walls.len(),
            arguments.output.display(),
            walls
                .iter()
                .filter_map(|wall| wall.get("detail").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" -- ")
        ),
    ))
}

/// Derive the admitted caller-authority span for ONE signed family request.
///
/// Both authors here are the ones `invoke_admitted_accelerator_v3` calls, in
/// the order it calls them: `accelerator_caller_authority_digest_v1` mints the
/// `role_request_digest` and `CallerAuthoritySeedsV1` places it in the sole
/// universal release-pinned seed order. Nothing is transcribed, so a change to
/// either author moves this span and the rows that report it — which is the
/// whole point, because the row this replaced could not move at all.
///
/// `parent_request_digest` and `index` are the only coordinates that vary
/// across one execution, and neither is a trusted-environment observation.
/// There is no slot argument because the preimage has no slot in it; if one
/// ever returns, this function stops compiling rather than quietly agreeing.
fn admitted_caller_authority_span_v1(
    trading: Pubkey,
    release_set: [u8; 32],
    market: Pubkey,
    root: Pubkey,
    parent_request_digest: ContentId,
    invocation_count: usize,
) -> Result<Vec<Pubkey>> {
    (0..invocation_count)
        .map(|index| {
            let index = u32::try_from(index).map_err(|_| {
                refusal(
                    "session/caller-authority-derivation",
                    "the accelerator invocation ordinal does not fit its wire width",
                )
            })?;
            let digest = accelerator_caller_authority_digest_v1(
                AcceleratorCallerKindV1::Admitted,
                parent_request_digest,
                index,
            )
            .map_err(|error| {
                refusal(
                    "session/caller-authority-derivation",
                    format!("accelerator caller authority digest {index}: {error:?}"),
                )
            })?;
            let seeds = CallerAuthoritySeedsV1::from_bytes(
                release_set,
                market.to_bytes(),
                ExecutionRoleV1::Trading,
                root.to_bytes(),
                digest.to_bytes(),
            )
            .map_err(|error| {
                refusal(
                    "session/caller-authority-derivation",
                    format!("caller authority seeds {index}: {error:?}"),
                )
            })?;
            Ok(Pubkey::find_program_address(&seeds.as_slices(), &trading).0)
        })
        .collect()
}

struct FrameAuthorsV1 {
    entry_index: u16,
    registry: Pubkey,
    trading: Pubkey,
    accelerator: Pubkey,
    accelerator_programdata: Pubkey,
    certificate: RecordCoordinateV1,
    admission: RecordCoordinateV1,
    artifact: RecordCoordinateV1,
    invocation_count: usize,
    /// The derived admitted caller-authority span, and whether its
    /// `parent_request_digest` was the caller's own or a stated probe.
    caller_authorities: Vec<Pubkey>,
    caller_authority_digest_is_probe: bool,
}

fn frame_rows_v1(
    fixed: &[Pubkey],
    observed: &[dclutch_operator::ObservedAccount],
    authors: FrameAuthorsV1,
) -> Vec<FrameRowV1> {
    let shape = |key: Pubkey| {
        observed
            .iter()
            .find(|account| account.key == key)
            .map(|account| ObservedShapeV1 {
                owner: account.owner,
                bytes: account.data.len(),
                executable: account.executable,
                lamports: account.lamports,
            })
    };
    let published = format!(
        "published by the founding ladder into Registry {}; address = raw-record PDA over (schema, content digest)",
        authors.registry
    );
    let staged = format!(
        "vacant staging cursor PDA under Registry {}; the publication ladder closes it",
        authors.registry
    );
    let labels: [(&'static str, String); HOT_FIXED_ACCOUNT_COUNT_V3] = [
        (
            "market",
            "created by the founding campaign (Core)".to_owned(),
        ),
        (
            "capability root",
            format!(
                "created by capability activation; PDA under Trading {} over the Market's own header (entry {})",
                authors.trading, authors.entry_index
            ),
        ),
        ("manifest raw", published.clone()),
        ("manifest staging", staged.clone()),
        ("program set raw", published.clone()),
        ("program set staging", staged.clone()),
        ("descriptor raw", published.clone()),
        ("descriptor staging", staged.clone()),
        ("config raw", published.clone()),
        ("config staging", staged.clone()),
        ("account profile raw", published.clone()),
        ("account profile staging", staged.clone()),
        ("request profile raw", published.clone()),
        ("request profile staging", staged.clone()),
        ("transition raw", published.clone()),
        ("transition staging", staged.clone()),
        ("effect raw", published.clone()),
        ("effect staging", staged.clone()),
        ("lifecycle raw", published.clone()),
        ("lifecycle staging", staged.clone()),
        ("strategy raw", published.clone()),
        ("strategy staging", staged.clone()),
        (
            "activation cache",
            "created by the Registry activation ladder".to_owned(),
        ),
        ("core program", "loader deploy".to_owned()),
        ("core programdata", "loader deploy".to_owned()),
        ("trading program", "loader deploy".to_owned()),
        ("trading programdata", "loader deploy".to_owned()),
        ("registry program", "loader deploy".to_owned()),
        ("rent sysvar", "runtime".to_owned()),
        ("instructions sysvar", "runtime".to_owned()),
        ("product raw", published.clone()),
        ("product staging", staged.clone()),
        ("result domain raw", published.clone()),
        ("result domain staging", staged.clone()),
        ("portfolio raw", published.clone()),
        ("portfolio staging", staged.clone()),
        ("linked basis raw", published.clone()),
        ("linked basis staging", staged),
        (
            "capability seal",
            format!(
                "PRODUCIBLE, AND THE PRODUCER EXISTS since 2026-09-03: \
                 `dclutch_operator::capability_seal_v1::capability_seal_instruction_v1` \
                 composes the permissionless `DCLTSEL1` outer for ANY family's descriptor \
                 and action, and derives this address rather than taking it. It replaced \
                 Direct's builder as the only one, which hard-coded \
                 `DirectExecutionActionV3::InlineOrdinary`. A General OpenBatch descriptor \
                 seals through it against the real Trading ELF in \
                 `a_general_descriptor_seals_through_the_family_neutral_producer`. Still \
                 unproduced ON THIS CHAIN, and it costs one transaction plus rent. PDA under \
                 Trading {} over (descriptor schema, descriptor digest, action, Trading \
                 semantic release, Registry)",
                authors.trading
            ),
        ),
    ];
    let mut rows = Vec::with_capacity(HOT_FIXED_ACCOUNT_COUNT_V3 + 16);
    for (coordinate, (label, author)) in labels.into_iter().enumerate() {
        let address = fixed.get(coordinate).copied().unwrap_or_default();
        rows.push(FrameRowV1 {
            coordinate,
            label,
            address,
            author,
            observed: shape(address),
        });
    }
    let evidence: [(&'static str, Pubkey, String); ADMITTED_STRATEGY_EVIDENCE_COUNT_V3] = [
        (
            "certificate raw",
            authors.certificate.raw,
            published.clone(),
        ),
        (
            "certificate staging",
            authors.certificate.staging,
            "vacant staging cursor".to_owned(),
        ),
        ("admission raw", authors.admission.raw, published.clone()),
        (
            "admission staging",
            authors.admission.staging,
            "vacant staging cursor".to_owned(),
        ),
        (
            "accelerator artifact release raw",
            authors.artifact.raw,
            published,
        ),
        (
            "accelerator artifact release staging",
            authors.artifact.staging,
            "vacant staging cursor".to_owned(),
        ),
        (
            "accelerator program",
            authors.accelerator,
            "loader deploy".to_owned(),
        ),
        (
            "accelerator programdata",
            authors.accelerator_programdata,
            "loader deploy".to_owned(),
        ),
    ];
    for (offset, (label, address, author)) in evidence.into_iter().enumerate() {
        rows.push(FrameRowV1 {
            coordinate: ADMITTED_STRATEGY_EVIDENCE_START_V3 - 1 + offset,
            label,
            address,
            author,
            observed: shape(address),
        });
    }
    let provenance = if authors.caller_authority_digest_is_probe {
        "a STATED PROBE family request digest, because this command signs nothing; \
         supply --parent-request-digest to report the exact address"
    } else {
        "the caller's own signed DCLTHOT3 family request digest"
    };
    for index in 0..authors.invocation_count {
        let address = authors
            .caller_authorities
            .get(index)
            .copied()
            .unwrap_or_default();
        rows.push(FrameRowV1 {
            coordinate: HOT_FIXED_ACCOUNT_COUNT_V3 + ADMITTED_STRATEGY_EVIDENCE_COUNT_V3 + index,
            label: "admitted caller authority",
            address,
            author: format!(
                "STATEABLE BY THE CALLER since 3a8ac205d, and derived here rather than \
                 described: PDA under Trading {} over (release set, market, Trading, root, \
                 accelerator_caller_authority_digest_v1(Admitted, parent_request_digest, \
                 {index})). The parent request digest used is {provenance}. It carries no \
                 trusted-environment scalar, so the address does not move with the \
                 executing slot and a signed account list can name it.",
                authors.trading
            ),
            observed: shape(address),
        });
    }
    rows
}

fn runtime_vector_json_v1(fixed_count: usize, rent_credit: u32, payer: Pubkey) -> Result<Value> {
    let state = usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3);
    let payer_coordinate = usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3);
    let credit = usize::from(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3);
    let system = general_system_program_account_v3(SESSION_ACTION_V1).map(usize::from);
    let mut rows = Vec::with_capacity(fixed_count);
    for coordinate in 0..fixed_count {
        let (label, author) = match coordinate {
            HOT_RUNTIME_ROOT_COORDINATE_V3 => {
                ("capability root", "injected from fixed coordinate 1")
            }
            HOT_RUNTIME_CONFIG_COORDINATE_V3 => ("config raw", "injected from fixed coordinate 8"),
            HOT_RUNTIME_PRODUCT_COORDINATE_V3 => {
                ("product raw", "injected from fixed coordinate 30")
            }
            HOT_RUNTIME_PORTFOLIO_COORDINATE_V3 => {
                ("portfolio raw", "injected from fixed coordinate 34")
            }
            HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3 => {
                ("linked basis raw", "injected from fixed coordinate 36")
            }
            other if other == state => (
                "General batch state",
                "created by THIS instruction's lifecycle plan; PDA under Trading",
            ),
            other if other == payer_coordinate => (
                "lifecycle payer",
                "an ordinary System-owned signer wallet; nothing derives it",
            ),
            other if other == credit => (
                "RentCredit",
                "nothing on chain names it, and nothing on the OpenBatch commit path decodes it",
            ),
            other if Some(other) == system => ("System program", "runtime"),
            _ => ("unmapped", "unmapped"),
        };
        rows.push(json!({
            "coordinate": coordinate,
            "label": label,
            "author": author,
            "requiredBytes": if coordinate == credit { Some(rent_credit) } else { None },
        }));
    }
    Ok(json!({
        "logicalCount": fixed_count,
        "payer": payer.to_string(),
        "coordinates": rows,
    }))
}

/// The accelerator invocation count the published effect selects.
///
/// One caller authority per invocation, which is what makes the count part of
/// the top-level account list rather than an internal detail.
fn admitted_invocation_count_v1(tail_count: u32) -> Result<usize> {
    let scalar_count = general_hot_scalar_count_v3(SESSION_ACTION_V1, tail_count)
        .map_err(|error| Error::new(format!("scalar count: {error:?}")))?;
    let transport = classify_bank_transport_v2(scalar_count, GENERAL_HOT_COMMON_IDENTITIES_V3)
        .map_err(|error| Error::new(format!("bank transport: {error:?}")))?;
    let count = match transport {
        BankTransportV2::InlineReturnData { bank_bytes } if bank_bytes != 0 => 1,
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } if page_count != 0 => {
            page_count
        }
        _ => {
            return Err(refusal(
                "session/bank-transport",
                "the published bank classifies to no invocation count",
            ));
        }
    };
    usize::try_from(count).map_err(|_| Error::new("invocation count".to_string()))
}

fn hinted_graph_records_v1(
    rpc: &mut Rpc,
    registry: &Pubkey,
    result_domain_raw: Pubkey,
    portfolio_raw: Pubkey,
) -> Result<(RecordCoordinateV1, RecordCoordinateV1)> {
    Ok((
        hinted_record_v1(
            rpc,
            registry,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            result_domain_raw,
            "ResultDomain",
        )?,
        hinted_record_v1(
            rpc,
            registry,
            PORTFOLIO_SCHEMA_ID_V2,
            portfolio_raw,
            "Portfolio",
        )?,
    ))
}

/// Admit one untrusted record address by REPRODUCING it from its own bytes.
///
/// A hint is routing input and nothing else: the schema is this command's, the
/// digest is the chain's, and an address that is not the raw-record PDA of the
/// bytes found at it refuses by name rather than being carried forward.
fn hinted_record_v1(
    rpc: &mut Rpc,
    registry: &Pubkey,
    schema: [u8; 32],
    raw: Pubkey,
    label: &str,
) -> Result<RecordCoordinateV1> {
    let account = rpc.required_account(raw, label)?;
    if account.owner != *registry {
        return Err(refusal(
            "session/record-owner",
            format!("{label} record at {raw} is not Registry-owned"),
        ));
    }
    let coordinate = record_coordinate(registry, schema, sha256(&account.data))?;
    if coordinate.raw != raw {
        return Err(refusal(
            "session/record-hint",
            format!("{label} address {raw} is not the raw-record PDA of its own bytes"),
        ));
    }
    Ok(coordinate)
}

/// One finalized observation over an exact address list, with vacancy as data.
fn finalized_frame_v1(rpc: &mut Rpc, addresses: &[Pubkey]) -> Result<(u64, Vec<ObservedAccount>)> {
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(addresses, floor)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let observed = addresses
        .iter()
        .copied()
        .zip(values)
        .map(|(key, value)| match value {
            Some(account) => ObservedAccount {
                observation,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            },
            None => ObservedAccount {
                observation,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
        })
        .collect();
    Ok((slot, observed))
}

fn place(fixed: &mut [Pubkey], index: usize, key: Pubkey) -> Result<()> {
    *fixed
        .get_mut(index)
        .ok_or_else(|| Error::new(format!("fixed coordinate {index}")))? = key;
    Ok(())
}

fn read_record(
    rpc: &mut Rpc,
    registry: &Pubkey,
    coordinate: RecordCoordinateV1,
    content: [u8; 32],
    label: &str,
) -> Result<Vec<u8>> {
    let account = rpc.required_account(coordinate.raw, label)?;
    if account.owner != *registry {
        return Err(refusal(
            "session/record-owner",
            format!("{label} at {} is not Registry-owned", coordinate.raw),
        ));
    }
    if sha256(&account.data) != content {
        return Err(refusal(
            "session/record-content",
            format!("{label} bytes do not hash to the identity its address names"),
        ));
    }
    Ok(account.data)
}

const _: () = assert!(CAPABILITY_ROOT_HEADER_BYTES_V1 > 0);
const _: () = assert!(system_program::ID.to_bytes()[0] == 0);

#[cfg(test)]
mod tests {
    use super::*;

    const TRADING: Pubkey = Pubkey::new_from_array([7_u8; 32]);
    const RELEASE_SET: [u8; 32] = [9_u8; 32];
    const MARKET: Pubkey = Pubkey::new_from_array([11_u8; 32]);
    const ROOT: Pubkey = Pubkey::new_from_array([13_u8; 32]);

    fn digest(bytes: &[u8]) -> ContentId {
        ContentId::new(
            Sha256::digest(bytes)
                .as_slice()
                .try_into()
                .expect("sha256 is 32 bytes"),
        )
        .expect("the fixture preimages are not the zero identity")
    }

    /// One admitted caller authority under the PRE-`3a8ac205d` seed.
    ///
    /// The old `role_request_digest` was `sha256(accelerator request header ‖
    /// inline bank)`, and a window-gated action's bank carries
    /// `scalar::CURRENT_SLOT`. This models exactly that and nothing else, so
    /// the comparison below is between two seeds rather than between a seed
    /// and a description of one.
    fn old_slot_bound_authority(bank: &[u8]) -> Pubkey {
        let seeds = CallerAuthoritySeedsV1::from_bytes(
            RELEASE_SET,
            MARKET.to_bytes(),
            ExecutionRoleV1::Trading,
            ROOT.to_bytes(),
            digest(bank).to_bytes(),
        )
        .expect("fixture seeds are non-zero");
        Pubkey::find_program_address(&seeds.as_slices(), &TRADING).0
    }

    fn bank_at_slot(slot: u64) -> Vec<u8> {
        let mut bank = b"accelerator-request-header".to_vec();
        bank.extend_from_slice(&slot.to_le_bytes());
        bank
    }

    /// The wall was real under the old seed and is gone under the new one.
    ///
    /// Both halves are measured, and the inputs are asserted DIFFERENT before
    /// the addresses are compared -- "nothing moved" and "my instrument was
    /// disconnected" log identically, and this test would otherwise pass with
    /// two identical banks.
    #[test]
    fn the_old_slot_bound_seed_moves_between_slots_and_the_new_preimage_does_not() {
        let early = bank_at_slot(492_745_516);
        let late = bank_at_slot(492_745_563);
        assert_ne!(early, late, "the two banks must differ, or this proves nothing");

        // The old seed: one signed account list could not name both addresses.
        let old_early = old_slot_bound_authority(&early);
        let old_late = old_slot_bound_authority(&late);
        assert_ne!(
            old_early, old_late,
            "the pre-3a8ac205d seed moved with the executing slot; that WAS the wall"
        );

        // The new preimage takes the SIGNED family request digest, which is the
        // same value in both executions, and no bank at all.
        let family = digest(b"one signed DCLTHOT3 family request");
        let span_early =
            admitted_caller_authority_span_v1(TRADING, RELEASE_SET, MARKET, ROOT, family, 4)
                .expect("the span derives");
        let span_late =
            admitted_caller_authority_span_v1(TRADING, RELEASE_SET, MARKET, ROOT, family, 4)
                .expect("the span derives");
        assert_eq!(
            span_early, span_late,
            "one signed family request names one authority span at every execution slot"
        );
        assert!(
            !span_early.contains(&old_early) && !span_early.contains(&old_late),
            "the new span is not the old one under another name"
        );
    }

    /// Slot-independence is worthless if the span stopped depending on the
    /// things it must depend on. Every coordinate that separates two
    /// invocations, two markets, two roots or two release sets moves the
    /// address, and the four invocations of one execution are distinct.
    #[test]
    fn the_caller_authority_span_moves_with_every_coordinate_that_must_move_it() {
        let family = digest(b"one signed DCLTHOT3 family request");
        let base = admitted_caller_authority_span_v1(TRADING, RELEASE_SET, MARKET, ROOT, family, 4)
            .expect("the span derives");
        assert_eq!(base.len(), 4);
        let mut unique = base.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 4, "each invocation ordinal names its own authority");

        let other_family = digest(b"another signed DCLTHOT3 family request");
        for (label, span) in [
            (
                "family request digest",
                admitted_caller_authority_span_v1(
                    TRADING,
                    RELEASE_SET,
                    MARKET,
                    ROOT,
                    other_family,
                    4,
                ),
            ),
            (
                "market",
                admitted_caller_authority_span_v1(
                    TRADING,
                    RELEASE_SET,
                    Pubkey::new_from_array([12_u8; 32]),
                    ROOT,
                    family,
                    4,
                ),
            ),
            (
                "root",
                admitted_caller_authority_span_v1(
                    TRADING,
                    RELEASE_SET,
                    MARKET,
                    Pubkey::new_from_array([14_u8; 32]),
                    family,
                    4,
                ),
            ),
            (
                "release set",
                admitted_caller_authority_span_v1(
                    TRADING,
                    [10_u8; 32],
                    MARKET,
                    ROOT,
                    family,
                    4,
                ),
            ),
            (
                "composing program",
                admitted_caller_authority_span_v1(
                    Pubkey::new_from_array([8_u8; 32]),
                    RELEASE_SET,
                    MARKET,
                    ROOT,
                    family,
                    4,
                ),
            ),
        ] {
            let span = span.expect("the span derives");
            assert_ne!(base, span, "substituting the {label} must move the span");
        }
    }

    /// The derivation is the verdict, so it has to be able to fail.
    ///
    /// A zero root is the coordinate `CallerAuthoritySeedsV1` refuses, and it
    /// is refused HERE rather than being turned into an address -- which is
    /// what makes the wall row a consequence rather than a decoration.
    #[test]
    fn a_zero_caller_authority_coordinate_refuses_instead_of_naming_an_address() {
        let family = digest(b"one signed DCLTHOT3 family request");
        let refused = admitted_caller_authority_span_v1(
            TRADING,
            RELEASE_SET,
            MARKET,
            Pubkey::default(),
            family,
            4,
        );
        let error = refused.expect_err("a zero context is not a caller-authority coordinate");
        assert!(
            error.to_string().contains("session/caller-authority-derivation"),
            "the refusal names the derivation that produced it, got: {error}"
        );
    }

    /// The probe preimage is a constant a reader can recompute, and it is never
    /// the reserved zero identity.
    #[test]
    fn the_stated_probe_digest_is_recomputable_and_non_zero() {
        let probe = digest(CALLER_AUTHORITY_PROBE_PREIMAGE_V1);
        assert_ne!(probe.to_bytes(), [0_u8; 32]);
        assert_eq!(
            probe.to_bytes(),
            digest(b"dclutch:devnet-general-session:caller-authority-probe:v1").to_bytes(),
            "the probe preimage is the literal this test restates, not a moving value"
        );
    }
}
