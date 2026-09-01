//! Owned-loopback provisioning of the provenance-pinned real Pyth transport.
//!
//! This is deliberately not a second public-provider path.  It admits only a
//! validator owned by the release gauntlet, authenticates the captured Router
//! and Receiver ELFs through `local_validator_release_v1`, initializes their
//! real prerequisite accounts, and asks the real Router to verify the captured
//! signed VAA.  The Receiver update account remains vacant.  The existing
//! flagship-resolution executor is the sole owner of `PostUpdate` and every
//! subsequent protocol transaction.

#[path = "owned_loopback_capture.rs"]
pub(crate) mod owned_loopback_capture;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_pyth_svm::{
    ENCODED_VAA_DISCRIMINATOR_V1, ENCODED_VAA_HEADER_BYTES_V1, ENCODED_VAA_VERIFIED_STATUS_V1,
    GuardianSetV1, PostUpdateParamsView, ProgramDataV3View, ProgramV3View, ReceiverConfigV2View,
    VerifiedEncodedVaaV1, local_validator_release_v1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::{
    hash::Hash,
    message::Message,
    rent::Rent,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::{create_account, transfer};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::{LOCAL_PYTH_RECEIVER_ELF, LOCAL_PYTH_ROUTER_ELF, account_sha256_v1, hex, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, parse_json_without_duplicate_keys_v1},
};
use owned_loopback_capture::{CaptureRefV1, DeploymentSlotPolicyV1};

const JOURNAL_SCHEMA_V1: &str = "dclutch-owned-loopback-pyth-prerequisite-transaction-v1";
const FACTS_SCHEMA_V1: &str = "dclutch-flagship-pyth-update-facts-v1";
pub(crate) const PROVIDER_CLOSURE_SCHEMA_V1: &str =
    "dclutch-owned-loopback-pyth-provider-closure-v1";
const WRITE_CHUNK_BYTES_V1: usize = 600;
const PACKET_BYTES_V1: usize = 1_232;
const VERIFY_COMPUTE_UNIT_LIMIT_V1: u32 = 400_000;
const LOCAL_RECEIVER_MINIMUM_SIGNATURES_V1: u8 = 5;
const FINALITY_WAIT_V1: Duration = Duration::from_secs(60);

const ROUTER_INITIALIZE: &[u8] =
    include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE: &[u8] = include_bytes!(
    "../../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data"
);
const SIGNED_VAA: &[u8] =
    include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
const RECEIVER_POST_UPDATE: &[u8] = include_bytes!(
    "../../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
);
const RECEIVER_CONFIG: &[u8] = include_bytes!(
    "../../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account"
);
const ROUTER_INITIALIZE_SHA256_V1: &str =
    "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b";
const RECEIVER_INITIALIZE_SHA256_V1: &str =
    "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f";
const SIGNED_VAA_SHA256_V1: &str =
    "ed8b973f36a932b9ec88659953859c8096f14e5aebd085bbe32b22c41a142c0d";
const RECEIVER_POST_UPDATE_SHA256_V1: &str =
    "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d";
const RECEIVER_CONFIG_SHA256_V1: &str =
    "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    payer: Pubkey,
    encoded_vaa: Pubkey,
    update_account: Pubkey,
    journal_dir: PathBuf,
    facts_output: PathBuf,
    payer_keypair: PathBuf,
    encoded_vaa_keypair: PathBuf,
    execute: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ActionV1 {
    RouterInitialize,
    ReceiverInitialize,
    TreasuryCapitalize,
    EncodedVaaCreate,
    EncodedVaaInitialize,
    EncodedVaaWrite0000,
    EncodedVaaWrite0600,
    EncodedVaaVerify,
}

impl ActionV1 {
    const ORDERED: [Self; 8] = [
        Self::RouterInitialize,
        Self::ReceiverInitialize,
        Self::TreasuryCapitalize,
        Self::EncodedVaaCreate,
        Self::EncodedVaaInitialize,
        Self::EncodedVaaWrite0000,
        Self::EncodedVaaWrite0600,
        Self::EncodedVaaVerify,
    ];

    const fn file_name(self) -> &'static str {
        match self {
            Self::RouterInitialize => "00-router-initialize.json",
            Self::ReceiverInitialize => "01-receiver-initialize.json",
            Self::TreasuryCapitalize => "02-treasury-capitalize.json",
            Self::EncodedVaaCreate => "03-encoded-vaa-create.json",
            Self::EncodedVaaInitialize => "04-encoded-vaa-initialize.json",
            Self::EncodedVaaWrite0000 => "05-encoded-vaa-write-0000.json",
            Self::EncodedVaaWrite0600 => "06-encoded-vaa-write-0600.json",
            Self::EncodedVaaVerify => "07-encoded-vaa-verify.json",
        }
    }

    const fn needs_encoded_signer(self) -> bool {
        matches!(self, Self::EncodedVaaCreate)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
    SignedNotSubmitted,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountStateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IntentV1 {
    genesis_hash: String,
    rpc_url: String,
    action: ActionV1,
    payer: String,
    encoded_vaa: String,
    update_account: String,
    release_sha256: String,
    fixture_sha256: BTreeMap<String, String>,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    recent_blockhash: String,
    last_valid_block_height: u64,
    instruction_program: String,
    instruction_accounts: Vec<(String, bool, bool)>,
    instruction_data_base64: String,
    compute_unit_limit: Option<u32>,
    message_base64: String,
    message_sha256: String,
    required_signers: Vec<String>,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    exact_fee_lamports: u64,
    expected_wire_bytes: usize,
    prestate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    packet_sha256: String,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    return_data_producer: Option<String>,
    return_data_base64: Option<String>,
    poststate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JournalV1 {
    schema: String,
    cluster: String,
    authorized_mutation: bool,
    phase: PhaseV1,
    intent_sha256: String,
    state_sha256: String,
    intent: IntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<FinalizedV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProducerPythFactsV1 {
    format: String,
    encoded_vaa: String,
    update_account: String,
    post_update_body_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderSourceRefV1 {
    path: String,
    sha256: String,
    schema: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderClosureProgramV1 {
    pub(crate) role: String,
    pub(crate) program_id: String,
    pub(crate) program_data_address: String,
    pub(crate) deployment_slot: String,
    pub(crate) elf_sha256: String,
    pub(crate) genesis_program_data_sha256: String,
    pub(crate) upgrade_authority: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderClosureReceiptV1 {
    pub(crate) schema: String,
    pub(crate) cluster: String,
    pub(crate) genesis_hash: String,
    pub(crate) status: String,
    pub(crate) finalized_observation_slot: String,
    plan: ProviderSourceRefV1,
    local_validator_profile: ProviderSourceRefV1,
    pub(crate) finalized_capture: CaptureRefV1,
    pub(crate) provider_programs: Vec<ProviderClosureProgramV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalValidatorProfileV1 {
    schema: String,
    launcher_version: String,
    network: LocalValidatorNetworkV1,
    ledger: String,
    account_dir: String,
    genesis_plan: String,
    genesis_boundary: String,
    key_policy: String,
    validator: LocalValidatorBinaryV1,
    programs: Vec<Value>,
    loader_time_boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalValidatorNetworkV1 {
    bind_address: String,
    rpc_url: String,
    faucet_port: u16,
    gossip_port: u16,
    dynamic_port_range: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalValidatorBinaryV1 {
    path: String,
    version: String,
}

#[derive(Clone, Debug)]
struct ProviderExpectedV1 {
    role: &'static str,
    program_id: Pubkey,
    program_data_address: Pubkey,
    elf_sha256: String,
    program_data_sha256: String,
    program_bytes: Vec<u8>,
    program_data_bytes: Vec<u8>,
    program_lamports: u64,
    program_account_sha256: String,
    program_data_lamports: u64,
    program_data_account_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProviderClosureArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    local_validator_profile: PathBuf,
    finalized_capture: PathBuf,
    output: PathBuf,
}

const PROVIDER_PLAN_SCHEMA_V1: &str = "dclutch-local-successor-infrastructure-plan-v2";
const PROVIDER_PROFILE_SCHEMA_V1: &str = "dclutch-successor-local-validator-profile-v1";
const MAX_PROVIDER_SOURCE_BYTES_V1: u64 = 32 * 1024 * 1024;
const PROVIDER_RECEIPT_MODE_V1: u32 = 0o600;

#[derive(Clone)]
struct SnapshotV1 {
    slot: u64,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl SnapshotV1 {
    fn required(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.accounts
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("local Pyth snapshot omitted {label} {key}")))
    }

    fn optional(&self, key: Pubkey) -> Option<&RpcAccount> {
        self.accounts.get(&key).and_then(Option::as_ref)
    }
}

#[derive(Clone, Copy)]
struct AddressesV1 {
    receiver: Pubkey,
    receiver_programdata: Pubkey,
    config: Pubkey,
    treasury: Pubkey,
    router: Pubkey,
    router_programdata: Pubkey,
    guardian: Pubkey,
    bridge: Pubkey,
    fee_collector: Pubkey,
}

pub(crate) fn run_provider_closure(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_provider_closure_arguments_v1(arguments)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&arguments.origin)?;
    let (plan_ref, profile_ref, expected) = authenticate_provider_sources_v1(
        &arguments.plan,
        &arguments.local_validator_profile,
        &arguments.origin,
    )?;
    let (capture_ref, capture_genesis_hash, capture_slot, captured_snapshot) =
        authenticate_provider_capture_v1(&arguments.finalized_capture, &expected)?;
    let captured_programs =
        provider_closure_programs_from_snapshot_v1(&captured_snapshot, &expected)?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let live_genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("owned-loopback getGenesisHash omitted a string"))?
        .to_owned();
    Hash::from_str(&live_genesis_hash)
        .map_err(|error| Error::new(format!("owned-loopback genesis hash: {error}")))?;
    if live_genesis_hash != capture_genesis_hash {
        return Err(Error::new(
            "live owned-loopback genesis hash differs from the finalized capture",
        ));
    }
    let keys = expected
        .iter()
        .flat_map(|row| [row.program_id, row.program_data_address])
        .collect::<Vec<_>>();
    let (live_slot, accounts) = rpc.finalized_accounts(&keys, capture_slot)?;
    let live_snapshot = SnapshotV1 {
        slot: live_slot,
        accounts: keys.into_iter().zip(accounts).collect(),
    };
    if provider_closure_programs_from_snapshot_v1(&live_snapshot, &expected)? != captured_programs {
        return Err(Error::new(
            "live finalized provider Loader pairs differ from their singular capture owner",
        ));
    }
    let receipt = ProviderClosureReceiptV1 {
        schema: PROVIDER_CLOSURE_SCHEMA_V1.into(),
        cluster: "owned-loopback".into(),
        genesis_hash: capture_genesis_hash,
        status: "finalized".into(),
        finalized_observation_slot: capture_slot.to_string(),
        plan: plan_ref,
        local_validator_profile: profile_ref,
        finalized_capture: capture_ref,
        provider_programs: captured_programs,
    };
    write_provider_closure_receipt_v1(&arguments.output, &receipt)?;
    let authenticated = authenticate_provider_closure_receipt_v1(&arguments.output)?;
    print_provider_receipt_v1(&authenticated)
}

fn parse_provider_closure_arguments_v1(
    arguments: Vec<String>,
) -> Result<ProviderClosureArgumentsV1> {
    let mut rpc_url = None;
    let mut plan = None;
    let mut local_validator_profile = None;
    let mut finalized_capture = None;
    let mut output = None;
    let mut values = arguments.into_iter();
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| Error::new(format!("{flag} requires a value")))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--plan" => &mut plan,
            "--local-validator-profile" => &mut local_validator_profile,
            "--finalized-capture" => &mut finalized_capture,
            "--output" => &mut output,
            _ => {
                return Err(Error::new(format!(
                    "unknown local provider closure argument: {flag}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} may be supplied only once")));
        }
    }
    Ok(ProviderClosureArgumentsV1 {
        origin: ClusterOriginV1::parse(&required(rpc_url, "--rpc-url")?, None)?,
        plan: absolute(required(plan, "--plan")?, "--plan")?,
        local_validator_profile: absolute(
            required(local_validator_profile, "--local-validator-profile")?,
            "--local-validator-profile",
        )?,
        finalized_capture: absolute(
            required(finalized_capture, "--finalized-capture")?,
            "--finalized-capture",
        )?,
        output: absolute(required(output, "--output")?, "--output")?,
    })
}

fn authenticate_provider_sources_v1(
    plan_path: &Path,
    profile_path: &Path,
    origin: &ClusterOriginV1,
) -> Result<(
    ProviderSourceRefV1,
    ProviderSourceRefV1,
    Vec<ProviderExpectedV1>,
)> {
    let (plan_path, plan_bytes) = read_provider_source_v1(plan_path, "successor plan")?;
    let plan_value = parse_json_without_duplicate_keys_v1(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan JSON: {error}")))?;
    let plan: SuccessorPlan = serde_json::from_value(plan_value)?;
    let (profile_path, profile_bytes) =
        read_provider_source_v1(profile_path, "local-validator profile")?;
    let profile_value = parse_json_without_duplicate_keys_v1(&profile_bytes)
        .map_err(|error| Error::new(format!("local-validator profile JSON: {error}")))?;
    let profile: LocalValidatorProfileV1 = serde_json::from_value(profile_value)?;

    let mut expected_plan_labels = [
        "registry",
        "core",
        "claims",
        "trading",
        "resolution",
        "custody",
        "rent-credit",
        "pyth-receiver",
        "pyth-router",
    ]
    .into_iter()
    .flat_map(|role| {
        [
            format!("loader.{role}.program"),
            format!("loader.{role}.programdata"),
        ]
    })
    .collect::<BTreeSet<_>>();
    if plan.infrastructure_succession.is_some() {
        expected_plan_labels.insert(crate::plan::REGISTRY_SUCCESSION_BUFFER_LABEL_V1.into());
    }
    if plan.schema != PROVIDER_PLAN_SCHEMA_V1
        || plan.record_publication != "transaction"
        || plan.genesis_accounts.len() != expected_plan_labels.len()
        || plan
            .genesis_accounts
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_plan_labels
    {
        return Err(Error::new(
            "provider closure requires an exact transaction-publication successor plan v2 account set",
        ));
    }
    if profile.schema != PROVIDER_PROFILE_SCHEMA_V1
        || profile.launcher_version != "3"
        || profile.network.bind_address != "127.0.0.1"
        || profile.genesis_boundary.is_empty()
        || profile.key_policy.is_empty()
        || profile.loader_time_boundary.is_empty()
        || profile.ledger.is_empty()
        || profile.validator.path.is_empty()
        || profile.validator.version.is_empty()
    {
        return Err(Error::new(
            "local-validator profile schema, launcher, containment, or boundary text changed",
        ));
    }
    let profile_origin = ClusterOriginV1::parse(&profile.network.rpc_url, None)?;
    if &profile_origin != origin {
        return Err(Error::new(
            "local-validator profile RPC origin differs from --rpc-url",
        ));
    }
    let port = origin
        .loopback_port()
        .ok_or_else(|| Error::new("provider closure requires one loopback port"))?;
    if profile.network.faucet_port != port.saturating_add(2)
        || profile.network.gossip_port != port.saturating_add(3)
        || profile.network.dynamic_port_range
            != format!("{}-{}", port.saturating_add(10), port.saturating_add(41))
    {
        return Err(Error::new(
            "local-validator profile port block differs from its RPC base",
        ));
    }
    let profile_plan =
        canonical_provider_regular_v1(Path::new(&profile.genesis_plan), "profile genesis plan")?;
    let account_dir = canonical_provider_directory_v1(
        Path::new(&profile.account_dir),
        "profile account directory",
    )?;
    let plan_account_dir =
        canonical_provider_directory_v1(Path::new(&plan.account_dir), "plan account directory")?;
    let ledger = canonical_provider_directory_v1(Path::new(&profile.ledger), "profile ledger")?;
    if profile_plan != plan_path
        || account_dir != plan_account_dir
        || profile_path.parent() != Some(ledger.as_path())
    {
        return Err(Error::new(
            "local-validator profile does not bind the exact plan, account directory, and ledger",
        ));
    }
    let expected = provider_expected_from_sources_v1(&plan, &profile)?;
    Ok((
        ProviderSourceRefV1 {
            path: provider_path_text_v1(&plan_path, "successor plan")?,
            sha256: sha256_hex_v1(&plan_bytes),
            schema: plan.schema,
        },
        ProviderSourceRefV1 {
            path: provider_path_text_v1(&profile_path, "local-validator profile")?,
            sha256: sha256_hex_v1(&profile_bytes),
            schema: profile.schema,
        },
        expected,
    ))
}

fn provider_expected_from_sources_v1(
    plan: &SuccessorPlan,
    profile: &LocalValidatorProfileV1,
) -> Result<Vec<ProviderExpectedV1>> {
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    let release = local.release();
    if plan.provider_release_id != sha256_hex_v1(&release.to_bytes()) {
        return Err(Error::new(
            "successor plan provider release digest differs from the compiled semantic owner",
        ));
    }
    let names = profile
        .programs
        .iter()
        .map(|row| {
            row.get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("local-validator profile program omitted its name"))
        })
        .collect::<Result<Vec<_>>>()?;
    if names
        != [
            "registry",
            "core",
            "claims",
            "trading",
            "resolution",
            "custody",
            "rent-credit",
            "pyth-receiver",
            "pyth-router",
        ]
    {
        return Err(Error::new(
            "local-validator profile program order or membership changed",
        ));
    }
    let specifications = [
        (
            "pyth-receiver",
            Pubkey::new_from_array(release.receiver_program()),
            Pubkey::new_from_array(release.receiver_programdata()),
            release.receiver_abi_id(),
            LOCAL_PYTH_RECEIVER_ELF,
        ),
        (
            "pyth-router",
            Pubkey::new_from_array(release.router_program()),
            Pubkey::new_from_array(release.router_programdata()),
            release.router_abi_id(),
            LOCAL_PYTH_ROUTER_ELF,
        ),
    ];
    let mut expected = Vec::with_capacity(2);
    for (index, (role, program_id, program_data_address, release_abi, elf)) in
        specifications.into_iter().enumerate()
    {
        if Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0
            != program_data_address
            || Sha256::digest(elf).as_slice() != release_abi.as_slice()
        {
            return Err(Error::new(format!(
                "compiled {role} ProgramData link or ELF release digest changed"
            )));
        }
        let mut program_bytes = vec![0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(program_data_address.as_ref());
        let mut program_data_bytes = vec![0_u8; 45];
        program_data_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        program_data_bytes.extend_from_slice(elf);
        let elf_sha256 = sha256_hex_v1(elf);
        let program_data_sha256 = sha256_hex_v1(&program_data_bytes);
        let program_pin = plan
            .genesis_accounts
            .get(&format!("loader.{role}.program"))
            .ok_or_else(|| Error::new(format!("successor plan omitted {role} Program pin")))?;
        let program_data_pin = plan
            .genesis_accounts
            .get(&format!("loader.{role}.programdata"))
            .ok_or_else(|| Error::new(format!("successor plan omitted {role} ProgramData pin")))?;
        let program_account_sha256 = account_sha256_v1(
            bpf_loader_upgradeable::ID,
            program_pin.lamports,
            true,
            0,
            &program_bytes,
        )?;
        let program_data_account_sha256 = account_sha256_v1(
            bpf_loader_upgradeable::ID,
            program_data_pin.lamports,
            false,
            0,
            &program_data_bytes,
        )?;
        if program_pin.address != program_id.to_string()
            || program_pin.owner != bpf_loader_upgradeable::ID.to_string()
            || program_pin.data_len != program_bytes.len()
            || program_pin.data_sha256 != sha256_hex_v1(&program_bytes)
            || program_pin.account_sha256 != program_account_sha256
            || program_data_pin.address != program_data_address.to_string()
            || program_data_pin.owner != bpf_loader_upgradeable::ID.to_string()
            || program_data_pin.data_len != program_data_bytes.len()
            || program_data_pin.data_sha256 != program_data_sha256
            || program_data_pin.account_sha256 != program_data_account_sha256
        {
            return Err(Error::new(format!(
                "successor plan {role} Loader pins differ from the compiled immutable provider"
            )));
        }
        authenticate_provider_profile_row_v1(
            &profile.programs[index + 7],
            role,
            program_id,
            program_data_address,
            &program_data_sha256,
            &elf_sha256,
        )?;
        expected.push(ProviderExpectedV1 {
            role,
            program_id,
            program_data_address,
            elf_sha256,
            program_data_sha256,
            program_bytes,
            program_data_bytes,
            program_lamports: program_pin.lamports,
            program_account_sha256,
            program_data_lamports: program_data_pin.lamports,
            program_data_account_sha256,
        });
    }
    Ok(expected)
}

fn authenticate_provider_profile_row_v1(
    value: &Value,
    role: &str,
    program_id: Pubkey,
    program_data_address: Pubkey,
    program_data_sha256: &str,
    elf_sha256: &str,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new(format!("profile {role} program row is not an object")))?;
    let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if keys
        != BTreeSet::from([
            "deployment_slot",
            "elf_sha256",
            "name",
            "program_id",
            "programdata_id",
            "programdata_sha256",
            "upgrade_authority",
        ])
        || object.get("name").and_then(Value::as_str) != Some(role)
        || object.get("program_id").and_then(Value::as_str) != Some(program_id.to_string().as_str())
        || object.get("programdata_id").and_then(Value::as_str)
            != Some(program_data_address.to_string().as_str())
        || object.get("deployment_slot").and_then(Value::as_u64) != Some(0)
        || object
            .get("upgrade_authority")
            .is_none_or(|authority| !authority.is_null())
        || object.get("programdata_sha256").and_then(Value::as_str) != Some(program_data_sha256)
        || object.get("elf_sha256").and_then(Value::as_str) != Some(elf_sha256)
    {
        return Err(Error::new(format!(
            "profile {role} row differs from its exact immutable Loader closure"
        )));
    }
    Ok(())
}

fn provider_closure_programs_from_snapshot_v1(
    snapshot: &SnapshotV1,
    expected: &[ProviderExpectedV1],
) -> Result<Vec<ProviderClosureProgramV1>> {
    let exact_keys = expected
        .iter()
        .flat_map(|row| [row.program_id, row.program_data_address])
        .collect::<BTreeSet<_>>();
    if snapshot.accounts.keys().copied().collect::<BTreeSet<_>>() != exact_keys {
        return Err(Error::new(
            "provider finalized snapshot did not contain exactly four Loader accounts",
        ));
    }
    let mut rows = Vec::with_capacity(expected.len());
    for row in expected {
        let program = snapshot.required(row.program_id, &format!("{} Program", row.role))?;
        let program_data = snapshot.required(
            row.program_data_address,
            &format!("{} ProgramData", row.role),
        )?;
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{} Program: {error:?}", row.role)))?;
        let program_data_view = ProgramDataV3View::parse(&program_data.data)
            .map_err(|error| Error::new(format!("{} ProgramData: {error:?}", row.role)))?;
        if program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || program.rent_epoch != 0
            || program.lamports != row.program_lamports
            || program.data != row.program_bytes
            || program_view.programdata() != row.program_data_address.to_bytes()
            || account_sha256_v1(
                program.owner,
                program.lamports,
                program.executable,
                program.rent_epoch,
                &program.data,
            )? != row.program_account_sha256
            || program_data.owner != bpf_loader_upgradeable::ID
            || program_data.executable
            || program_data.rent_epoch != 0
            || program_data.lamports != row.program_data_lamports
            || program_data.data != row.program_data_bytes
            || program_data_view.deployment_slot() != 0
            || program_data_view.upgrade_authority().is_some()
            || program_data.data.get(12) != Some(&0)
            || !program_data
                .data
                .get(13..45)
                .is_some_and(|inactive| inactive.iter().all(|byte| *byte == 0))
            || sha256_hex_v1(&program_data.data) != row.program_data_sha256
            || sha256_hex_v1(program_data_view.elf()) != row.elf_sha256
            || account_sha256_v1(
                program_data.owner,
                program_data.lamports,
                program_data.executable,
                program_data.rent_epoch,
                &program_data.data,
            )? != row.program_data_account_sha256
        {
            return Err(Error::new(format!(
                "finalized {} Loader pair differs from plan/profile slot-zero immutable bytes",
                row.role
            )));
        }
        rows.push(provider_closure_program_v1(row));
    }
    Ok(rows)
}

fn authenticate_provider_capture_v1(
    path: &Path,
    expected: &[ProviderExpectedV1],
) -> Result<(CaptureRefV1, String, u64, SnapshotV1)> {
    let capture = owned_loopback_capture::authenticate_v1(path)?;
    let mut accounts = BTreeMap::new();
    for row in expected {
        let pair = capture.loader_pair_v1(
            row.role,
            row.program_id,
            Some(row.program_data_address),
            DeploymentSlotPolicyV1::ExactZeroImmutable,
        )?;
        for account in [pair.program, pair.program_data] {
            if accounts
                .insert(
                    account.address,
                    Some(RpcAccount {
                        lamports: account.lamports,
                        owner: account.owner,
                        executable: account.executable,
                        rent_epoch: account.rent_epoch,
                        data: account.data,
                    }),
                )
                .is_some()
            {
                return Err(Error::new("captured provider Loader accounts alias"));
            }
        }
    }
    let reference = capture.reference_v1()?;
    let genesis_hash = capture.genesis_hash.clone();
    let finalized_slot = capture.finalized_slot;
    Ok((
        reference,
        genesis_hash,
        finalized_slot,
        SnapshotV1 {
            slot: finalized_slot,
            accounts,
        },
    ))
}

fn authenticate_provider_receipt_capture_v1(
    receipt: &ProviderClosureReceiptV1,
    expected: &[ProviderExpectedV1],
) -> Result<()> {
    let (capture_ref, capture_genesis, capture_slot, capture_snapshot) =
        authenticate_provider_capture_v1(Path::new(&receipt.finalized_capture.path), expected)?;
    let captured_programs =
        provider_closure_programs_from_snapshot_v1(&capture_snapshot, expected)?;
    if receipt.finalized_capture != capture_ref
        || receipt.genesis_hash != capture_genesis
        || receipt.finalized_observation_slot != capture_slot.to_string()
        || receipt.provider_programs != captured_programs
    {
        return Err(Error::new(
            "provider closure capture reference, cluster boundary, or captured Loader rows changed",
        ));
    }
    Ok(())
}

fn provider_closure_program_v1(expected: &ProviderExpectedV1) -> ProviderClosureProgramV1 {
    ProviderClosureProgramV1 {
        role: expected.role.into(),
        program_id: expected.program_id.to_string(),
        program_data_address: expected.program_data_address.to_string(),
        deployment_slot: "0".into(),
        elf_sha256: expected.elf_sha256.clone(),
        genesis_program_data_sha256: expected.program_data_sha256.clone(),
        upgrade_authority: None,
    }
}

pub(crate) fn authenticate_provider_closure_receipt_v1(
    path: &Path,
) -> Result<ProviderClosureReceiptV1> {
    let (path, bytes) = read_provider_source_v1(path, "provider closure receipt")?;
    if fs::metadata(&path)?.permissions().mode() & 0o777 != PROVIDER_RECEIPT_MODE_V1 {
        return Err(Error::new(
            "provider closure receipt mode must be exactly 0600",
        ));
    }
    let value = parse_json_without_duplicate_keys_v1(&bytes)
        .map_err(|error| Error::new(format!("provider closure receipt JSON: {error}")))?;
    let receipt: ProviderClosureReceiptV1 = serde_json::from_value(value)?;
    if receipt.schema != PROVIDER_CLOSURE_SCHEMA_V1
        || receipt.cluster != "owned-loopback"
        || receipt.status != "finalized"
    {
        return Err(Error::new(
            "provider closure schema, cluster, or completion status changed",
        ));
    }
    Hash::from_str(&receipt.genesis_hash)
        .map_err(|error| Error::new(format!("provider closure genesis hash: {error}")))?;
    let slot = receipt
        .finalized_observation_slot
        .parse::<u64>()
        .map_err(|error| Error::new(format!("provider closure observation slot: {error}")))?;
    if slot.to_string() != receipt.finalized_observation_slot {
        return Err(Error::new(
            "provider closure observation slot is not canonical decimal",
        ));
    }
    let profile_path = Path::new(&receipt.local_validator_profile.path);
    let (_, profile_bytes) = read_provider_source_v1(profile_path, "local-validator profile")?;
    if sha256_hex_v1(&profile_bytes) != receipt.local_validator_profile.sha256 {
        return Err(Error::new(
            "provider closure profile reference digest changed",
        ));
    }
    let profile_value = parse_json_without_duplicate_keys_v1(&profile_bytes)?;
    let profile: LocalValidatorProfileV1 = serde_json::from_value(profile_value)?;
    let origin = ClusterOriginV1::parse(&profile.network.rpc_url, None)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
    let (plan_ref, profile_ref, expected) =
        authenticate_provider_sources_v1(Path::new(&receipt.plan.path), profile_path, &origin)?;
    if receipt.plan != plan_ref || receipt.local_validator_profile != profile_ref {
        return Err(Error::new(
            "provider closure source references or ordered program rows changed",
        ));
    }
    authenticate_provider_receipt_capture_v1(&receipt, &expected)?;
    Ok(receipt)
}

fn read_provider_source_v1(path: &Path, label: &str) -> Result<(PathBuf, Vec<u8>)> {
    let path = canonical_provider_regular_v1(path, label)?;
    let metadata = fs::metadata(&path)?;
    if metadata.len() == 0 || metadata.len() > MAX_PROVIDER_SOURCE_BYTES_V1 {
        return Err(Error::new(format!(
            "{label} must contain one through 32 MiB"
        )));
    }
    Ok((path.clone(), fs::read(path)?))
}

fn canonical_provider_regular_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} path must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    let canonical = fs::canonicalize(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || canonical != path {
        return Err(Error::new(format!(
            "{label} must be one canonical regular non-symlink file"
        )));
    }
    Ok(canonical)
}

fn canonical_provider_directory_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} path must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    let canonical = fs::canonicalize(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || canonical != path {
        return Err(Error::new(format!(
            "{label} must be one canonical non-symlink directory"
        )));
    }
    Ok(canonical)
}

fn provider_path_text_v1(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("{label} path is not UTF-8")))
}

fn write_provider_closure_receipt_v1(
    path: &Path,
    receipt: &ProviderClosureReceiptV1,
) -> Result<()> {
    if !path.is_absolute() || fs::symlink_metadata(path).is_ok() {
        return Err(Error::new(
            "provider closure --output must be one absent absolute path",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("provider closure output omitted its parent"))?;
    let parent = canonical_provider_directory_v1(parent, "provider closure output parent")?;
    if path.parent() != Some(parent.as_path()) {
        return Err(Error::new(
            "provider closure output parent is not canonical",
        ));
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("provider closure output omitted a UTF-8 name"))?;
    let temporary = parent.join(format!(".{name}.{}.publish.tmp", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(receipt)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PROVIDER_RECEIPT_MODE_V1)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create provider closure temporary: {error}")))?;
    file.set_permissions(fs::Permissions::from_mode(PROVIDER_RECEIPT_MODE_V1))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)?;
            fs::File::open(&parent)?.sync_all()?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(Error::new(format!(
                "publish provider closure receipt: {error}"
            )))
        }
    }
}

fn print_provider_receipt_v1<T: Serialize>(receipt: &T) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(receipt)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&arguments.origin)?;
    authenticate_fixture_constants_v1()?;
    if !arguments.journal_dir.is_dir() {
        return Err(Error::new(
            "--journal-dir must be one existing absolute directory",
        ));
    }
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("owned-loopback getGenesisHash omitted a string"))?
        .to_owned();
    let addresses = addresses_v1()?;

    for action in ActionV1::ORDERED {
        let path = arguments.journal_dir.join(action.file_name());
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let _: Value = parse_json_without_duplicate_keys_v1(&bytes)?;
        let mut journal: JournalV1 = serde_json::from_slice(&bytes)?;
        authenticate_journal_v1(&journal, &arguments, &genesis, action)?;
        if journal.phase != PhaseV1::Finalized {
            resume_journal_v1(&mut rpc, &arguments, &addresses, &path, &mut journal)?;
            print_journal_v1(&journal)?;
            return Ok(());
        }
        authenticate_finalized_history_v1(&mut rpc, &journal)?;
    }

    let snapshot = snapshot_v1(&mut rpc, &arguments, &addresses)?;
    authenticate_release_v1(&snapshot, &arguments, &addresses)?;
    let next = next_action_v1(&arguments, &addresses, &snapshot)?;
    let Some(action) = next else {
        authenticate_complete_v1(&arguments, &addresses, &snapshot)?;
        require_complete_journals_v1(&arguments)?;
        write_facts_v1(&arguments)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema": "dclutch-owned-loopback-pyth-prerequisites-v1",
                "status": "finalized",
                "encodedVaa": arguments.encoded_vaa.to_string(),
                "updateAccount": arguments.update_account.to_string(),
                "facts": arguments.facts_output.display().to_string()
            }))?
        );
        return Ok(());
    };
    require_canonical_journal_prefix_v1(&arguments, action)?;
    let instructions = instructions_v1(&mut rpc, &arguments, &addresses, &snapshot, action)?;
    let journal = build_journal_v1(
        &mut rpc,
        &arguments,
        &addresses,
        &snapshot,
        &genesis,
        action,
        instructions,
    )?;
    let path = arguments.journal_dir.join(action.file_name());
    write_journal_v1(&path, &journal, true)?;
    let mut journal = journal;
    resume_journal_v1(&mut rpc, &arguments, &addresses, &path, &mut journal)?;
    print_journal_v1(&journal)
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut payer = None;
    let mut encoded_vaa = None;
    let mut update_account = None;
    let mut journal_dir = None;
    let mut facts_output = None;
    let mut payer_keypair = None;
    let mut encoded_vaa_keypair = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--payer" => &mut payer,
            "--encoded-vaa" => &mut encoded_vaa,
            "--update-account" => &mut update_account,
            "--journal-dir" => &mut journal_dir,
            "--facts-output" => &mut facts_output,
            "--payer-keypair" => &mut payer_keypair,
            "--encoded-vaa-keypair" => &mut encoded_vaa_keypair,
            _ => {
                return Err(Error::new(format!(
                    "unknown local Pyth provisioning argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let origin = ClusterOriginV1::parse(&rpc_url, None)?;
    let payer = pubkey(&required(payer, "--payer")?)?;
    let encoded_vaa = pubkey(&required(encoded_vaa, "--encoded-vaa")?)?;
    let update_account = pubkey(&required(update_account, "--update-account")?)?;
    if payer == Pubkey::default()
        || encoded_vaa == Pubkey::default()
        || update_account == Pubkey::default()
        || payer == encoded_vaa
        || payer == update_account
        || encoded_vaa == update_account
    {
        return Err(Error::new(
            "local Pyth payer, EncodedVaa, and update identities must be nonzero and distinct",
        ));
    }
    Ok(ArgumentsV1 {
        origin,
        payer,
        encoded_vaa,
        update_account,
        journal_dir: absolute(required(journal_dir, "--journal-dir")?, "--journal-dir")?,
        facts_output: absolute(required(facts_output, "--facts-output")?, "--facts-output")?,
        payer_keypair: absolute(
            required(payer_keypair, "--payer-keypair")?,
            "--payer-keypair",
        )?,
        encoded_vaa_keypair: absolute(
            required(encoded_vaa_keypair, "--encoded-vaa-keypair")?,
            "--encoded-vaa-keypair",
        )?,
        execute,
    })
}

fn addresses_v1() -> Result<AddressesV1> {
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    let release = local.release();
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let router = Pubkey::new_from_array(release.router_program());
    Ok(AddressesV1 {
        receiver,
        receiver_programdata: Pubkey::new_from_array(release.receiver_programdata()),
        config: Pubkey::new_from_array(release.receiver_config()),
        treasury: Pubkey::find_program_address(&[b"treasury", &[0]], &receiver).0,
        router,
        router_programdata: Pubkey::new_from_array(release.router_programdata()),
        guardian: Pubkey::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router).0,
        bridge: Pubkey::find_program_address(&[b"Bridge"], &router).0,
        fee_collector: Pubkey::find_program_address(&[b"fee_collector"], &router).0,
    })
}

fn snapshot_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
) -> Result<SnapshotV1> {
    let mut keys = BTreeSet::from([
        arguments.payer,
        arguments.encoded_vaa,
        arguments.update_account,
        addresses.receiver,
        addresses.receiver_programdata,
        addresses.config,
        addresses.treasury,
        addresses.router,
        addresses.router_programdata,
        addresses.guardian,
        addresses.bridge,
        addresses.fee_collector,
        system_program::ID,
        sysvar::clock::ID,
        sysvar::rent::ID,
    ]);
    let ordered = keys.iter().copied().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&ordered, 0)?;
    keys.clear();
    Ok(SnapshotV1 {
        slot,
        accounts: ordered.into_iter().zip(values).collect(),
    })
}

fn authenticate_release_v1(
    snapshot: &SnapshotV1,
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
) -> Result<()> {
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    let release = local.release();
    for (label, program_key, data_key, expected_elf) in [
        (
            "Receiver",
            addresses.receiver,
            addresses.receiver_programdata,
            release.receiver_abi_id(),
        ),
        (
            "Router",
            addresses.router,
            addresses.router_programdata,
            release.router_abi_id(),
        ),
    ] {
        let program = snapshot.required(program_key, &format!("{label} Program"))?;
        let data = snapshot.required(data_key, &format!("{label} ProgramData"))?;
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("local {label} Program: {error:?}")))?;
        let data_view = ProgramDataV3View::parse(&data.data)
            .map_err(|error| Error::new(format!("local {label} ProgramData: {error:?}")))?;
        if program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || program_view.programdata() != data_key.to_bytes()
            || data.owner != bpf_loader_upgradeable::ID
            || data.executable
            || data_view.deployment_slot() != 0
            || data_view.upgrade_authority().is_some()
            || data.data.get(12) != Some(&0)
            || !data
                .data
                .get(13..45)
                .is_some_and(|inactive| inactive.iter().all(|byte| *byte == 0))
            || Sha256::digest(data_view.elf()).as_slice() != expected_elf
        {
            return Err(Error::new(format!(
                "owned-loopback {label} Program/ProgramData link, zero slot, null authority, owner, privilege, or ELF digest refused"
            )));
        }
    }
    let payer = snapshot.required(arguments.payer, "local Pyth payer")?;
    if payer.owner != system_program::ID || payer.executable || !payer.data.is_empty() {
        return Err(Error::new(
            "local Pyth payer must be an existing System-owned data-empty wallet",
        ));
    }
    Ok(())
}

fn next_action_v1(
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
    snapshot: &SnapshotV1,
) -> Result<Option<ActionV1>> {
    if snapshot.optional(addresses.guardian).is_none() {
        if snapshot.optional(addresses.bridge).is_some()
            || snapshot.optional(addresses.fee_collector).is_some()
        {
            return Err(Error::new(
                "local Router prerequisite accounts were partially initialized",
            ));
        }
        return Ok(Some(ActionV1::RouterInitialize));
    }
    if snapshot.optional(addresses.config).is_none() {
        return Ok(Some(ActionV1::ReceiverInitialize));
    }
    let treasury_rent = rent_minimum_from_snapshot(snapshot, 0)?;
    if snapshot
        .optional(addresses.treasury)
        .map(|account| account.lamports)
        .unwrap_or(0)
        < treasury_rent
    {
        return Ok(Some(ActionV1::TreasuryCapitalize));
    }
    let Some(encoded) = snapshot.optional(arguments.encoded_vaa) else {
        return Ok(Some(ActionV1::EncodedVaaCreate));
    };
    if encoded.owner != addresses.router
        || encoded.executable
        || encoded.data.len() != ENCODED_VAA_HEADER_BYTES_V1 + SIGNED_VAA.len()
    {
        return Err(Error::new(
            "local EncodedVaa owner, privilege, or exact width refused",
        ));
    }
    if encoded.data.iter().all(|byte| *byte == 0) {
        return Ok(Some(ActionV1::EncodedVaaInitialize));
    }
    if encoded.data.get(..8) != Some(ENCODED_VAA_DISCRIMINATOR_V1.as_slice())
        || encoded.data.get(9..41) != Some(arguments.payer.as_ref())
    {
        return Err(Error::new(
            "local EncodedVaa discriminator or authority refused",
        ));
    }
    let length = u32::from_le_bytes(
        encoded
            .data
            .get(42..46)
            .ok_or_else(|| Error::new("local EncodedVaa length was truncated"))?
            .try_into()
            .map_err(|_| Error::new("local EncodedVaa length width changed"))?,
    ) as usize;
    if length != SIGNED_VAA.len() {
        return Err(Error::new(
            "local EncodedVaa allocated vector width differed from the pinned VAA",
        ));
    }
    let payload = encoded
        .data
        .get(46..46 + length)
        .ok_or_else(|| Error::new("local EncodedVaa payload was truncated"))?;
    if payload.iter().all(|byte| *byte == 0) {
        return Ok(Some(ActionV1::EncodedVaaWrite0000));
    }
    if payload.get(..WRITE_CHUNK_BYTES_V1) == Some(&SIGNED_VAA[..WRITE_CHUNK_BYTES_V1])
        && payload[WRITE_CHUNK_BYTES_V1..]
            .iter()
            .all(|byte| *byte == 0)
    {
        return Ok(Some(ActionV1::EncodedVaaWrite0600));
    }
    if payload != SIGNED_VAA {
        return Err(Error::new(
            "local EncodedVaa write prefix or vacant tail differed from the pinned VAA",
        ));
    }
    if encoded.data.get(8) != Some(&ENCODED_VAA_VERIFIED_STATUS_V1) {
        if encoded.data.get(8) != Some(&1) || encoded.data.get(41) != Some(&0) {
            return Err(Error::new(
                "local EncodedVaa writing status or pre-verification account version refused",
            ));
        }
        return Ok(Some(ActionV1::EncodedVaaVerify));
    }
    Ok(None)
}

fn instructions_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
    snapshot: &SnapshotV1,
    action: ActionV1,
) -> Result<Vec<Instruction>> {
    let instruction = match action {
        ActionV1::RouterInitialize => Instruction {
            program_id: addresses.router,
            accounts: vec![
                AccountMeta::new(addresses.bridge, false),
                AccountMeta::new(addresses.guardian, false),
                AccountMeta::new(addresses.fee_collector, false),
                AccountMeta::new(arguments.payer, true),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: ROUTER_INITIALIZE.to_vec(),
        },
        ActionV1::ReceiverInitialize => Instruction {
            program_id: addresses.receiver,
            accounts: vec![
                AccountMeta::new(arguments.payer, true),
                AccountMeta::new(addresses.config, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: RECEIVER_INITIALIZE.to_vec(),
        },
        ActionV1::TreasuryCapitalize => {
            let minimum = rpc.minimum_balance(0)?;
            let current = snapshot
                .optional(addresses.treasury)
                .map(|account| account.lamports)
                .unwrap_or(0);
            let delta = minimum
                .checked_sub(current)
                .ok_or_else(|| Error::new("local treasury capitalization underflowed"))?;
            if delta == 0 {
                return Err(Error::new("local treasury is already capitalized"));
            }
            transfer(&arguments.payer, &addresses.treasury, delta)
        }
        ActionV1::EncodedVaaCreate => create_account(
            &arguments.payer,
            &arguments.encoded_vaa,
            rpc.minimum_balance(ENCODED_VAA_HEADER_BYTES_V1 + SIGNED_VAA.len())?,
            (ENCODED_VAA_HEADER_BYTES_V1 + SIGNED_VAA.len()) as u64,
            &addresses.router,
        ),
        ActionV1::EncodedVaaInitialize => Instruction {
            program_id: addresses.router,
            accounts: vec![
                AccountMeta::new_readonly(arguments.payer, true),
                AccountMeta::new(arguments.encoded_vaa, false),
            ],
            data: anchor_discriminator_v1(b"global:init_encoded_vaa"),
        },
        ActionV1::EncodedVaaWrite0000 | ActionV1::EncodedVaaWrite0600 => {
            let offset = if action == ActionV1::EncodedVaaWrite0000 {
                0
            } else {
                WRITE_CHUNK_BYTES_V1
            };
            let chunk = &SIGNED_VAA[offset..(offset + WRITE_CHUNK_BYTES_V1).min(SIGNED_VAA.len())];
            let mut data = anchor_discriminator_v1(b"global:write_encoded_vaa");
            data.extend_from_slice(
                &u32::try_from(offset)
                    .map_err(|_| Error::new("local VAA offset exceeded u32"))?
                    .to_le_bytes(),
            );
            data.extend_from_slice(
                &u32::try_from(chunk.len())
                    .map_err(|_| Error::new("local VAA chunk exceeded u32"))?
                    .to_le_bytes(),
            );
            data.extend_from_slice(chunk);
            Instruction {
                program_id: addresses.router,
                accounts: vec![
                    AccountMeta::new_readonly(arguments.payer, true),
                    AccountMeta::new(arguments.encoded_vaa, false),
                ],
                data,
            }
        }
        ActionV1::EncodedVaaVerify => Instruction {
            program_id: addresses.router,
            accounts: vec![
                AccountMeta::new_readonly(arguments.payer, true),
                AccountMeta::new(arguments.encoded_vaa, false),
                AccountMeta::new_readonly(addresses.guardian, false),
            ],
            data: anchor_discriminator_v1(b"global:verify_encoded_vaa_v1"),
        },
    };
    let mut instructions = Vec::with_capacity(if action == ActionV1::EncodedVaaVerify {
        2
    } else {
        1
    });
    if action == ActionV1::EncodedVaaVerify {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            VERIFY_COMPUTE_UNIT_LIMIT_V1,
        ));
    }
    instructions.push(instruction);
    Ok(instructions)
}

#[allow(clippy::too_many_arguments)]
fn build_journal_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    _addresses: &AddressesV1,
    snapshot: &SnapshotV1,
    genesis: &str,
    action: ActionV1,
    instructions: Vec<Instruction>,
) -> Result<JournalV1> {
    let provider_instruction = instructions
        .last()
        .ok_or_else(|| Error::new("local Pyth action omitted its provider instruction"))?;
    let compute_unit_limit =
        (action == ActionV1::EncodedVaaVerify).then_some(VERIFY_COMPUTE_UNIT_LIMIT_V1);
    if instructions.len() != 1 + usize::from(compute_unit_limit.is_some()) {
        return Err(Error::new(
            "local Pyth action changed its exact instruction count",
        ));
    }
    let (recent_blockhash, last_valid_block_height) = latest_blockhash_v1(rpc)?;
    let message =
        Message::new_with_blockhash(&instructions, Some(&arguments.payer), &recent_blockhash);
    let expected_signers = if action.needs_encoded_signer() {
        vec![arguments.payer, arguments.encoded_vaa]
    } else {
        vec![arguments.payer]
    };
    let actual_signers = message
        .account_keys
        .iter()
        .take(usize::from(message.header.num_required_signatures))
        .copied()
        .collect::<Vec<_>>();
    if actual_signers != expected_signers {
        return Err(Error::new(
            "local Pyth message changed its exact signer order",
        ));
    }
    let resolved = message.account_keys.clone();
    let (observation_slot, values) = rpc.finalized_accounts(&resolved, snapshot.slot)?;
    let observation_unix_timestamp = rpc.block_time(observation_slot)?;
    let pre_balances = values
        .iter()
        .map(|account| account.as_ref().map(|value| value.lamports).unwrap_or(0))
        .collect::<Vec<_>>();
    let prestate = resolved
        .iter()
        .copied()
        .zip(values.iter())
        .map(|(key, account)| (key.to_string(), account_state_v1(key, account.as_ref())))
        .collect::<BTreeMap<_, _>>();
    let message_bytes = bincode::serialize(&message)
        .map_err(|error| Error::new(format!("serialize local Pyth message: {error}")))?;
    let message_base64 = BASE64.encode(&message_bytes);
    let exact_fee_lamports = fee_for_message_v1(rpc, &message_base64)?;
    let expected_wire_bytes = bincode::serialize(&Transaction::new_unsigned(message.clone()))
        .map_err(|error| Error::new(format!("size local Pyth packet: {error}")))?
        .len();
    if expected_wire_bytes > PACKET_BYTES_V1 {
        return Err(Error::new(format!(
            "local Pyth {:?} packet is {expected_wire_bytes} bytes, above {PACKET_BYTES_V1}",
            action
        )));
    }
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    let intent = IntentV1 {
        genesis_hash: genesis.into(),
        rpc_url: arguments.origin.redacted_url(),
        action,
        payer: arguments.payer.to_string(),
        encoded_vaa: arguments.encoded_vaa.to_string(),
        update_account: arguments.update_account.to_string(),
        release_sha256: sha256_hex_v1(&local.release().to_bytes()),
        fixture_sha256: fixture_sha256_v1(),
        observation_slot,
        observation_unix_timestamp,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        instruction_program: provider_instruction.program_id.to_string(),
        instruction_accounts: provider_instruction
            .accounts
            .iter()
            .map(|meta| (meta.pubkey.to_string(), meta.is_signer, meta.is_writable))
            .collect(),
        instruction_data_base64: BASE64.encode(&provider_instruction.data),
        compute_unit_limit,
        message_base64,
        message_sha256: sha256_hex_v1(&message_bytes),
        required_signers: expected_signers.iter().map(ToString::to_string).collect(),
        resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
        pre_balances,
        exact_fee_lamports,
        expected_wire_bytes,
        prestate,
    };
    let mut journal = JournalV1 {
        schema: JOURNAL_SCHEMA_V1.into(),
        cluster: "owned-loopback".into(),
        authorized_mutation: arguments.execute,
        phase: PhaseV1::Planned,
        intent_sha256: sha256_hex_v1(&serde_json::to_vec(&intent)?),
        state_sha256: String::new(),
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
    };
    refresh_journal_digest_v1(&mut journal)?;
    authenticate_journal_v1(&journal, arguments, genesis, action)?;
    Ok(journal)
}

fn resume_journal_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
    path: &Path,
    journal: &mut JournalV1,
) -> Result<()> {
    match journal.phase {
        PhaseV1::Finalized => authenticate_finalized_history_v1(rpc, journal),
        PhaseV1::SignedNotSubmitted | PhaseV1::Submitted => {
            let signature = journal
                .expected_signature
                .clone()
                .ok_or_else(|| Error::new("ambiguous local Pyth journal omitted signature"))?;
            let Some(transaction) = finalized_transaction_v1(rpc, &signature)? else {
                return Err(Error::new(format!(
                    "local Pyth transaction {signature} is not finalized; {:?} recovery is poll-only and will not re-sign or resubmit",
                    journal.phase
                )));
            };
            finalize_journal_v1(rpc, arguments, addresses, journal, transaction)?;
            write_journal_v1(path, journal, false)
        }
        PhaseV1::Planned if !arguments.execute => Ok(()),
        PhaseV1::Planned => {
            require_current_prestate_v1(rpc, journal)?;
            let height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| Error::new("local Pyth getBlockHeight was not u64"))?;
            if height > journal.intent.last_valid_block_height {
                return Err(Error::new(
                    "local Pyth planned blockhash expired before key access; preserve the journal and start a fresh contained validator",
                ));
            }
            let payer = load_keypair_v1(
                &arguments.payer_keypair,
                "local-pyth-payer",
                arguments.payer,
            )?;
            let encoded = if journal.intent.action.needs_encoded_signer() {
                Some(load_keypair_v1(
                    &arguments.encoded_vaa_keypair,
                    "local-pyth-encoded-vaa",
                    arguments.encoded_vaa,
                )?)
            } else {
                None
            };
            let message_bytes = BASE64
                .decode(&journal.intent.message_base64)
                .map_err(|error| Error::new(format!("local Pyth message base64: {error}")))?;
            let message: Message = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("local Pyth message: {error}")))?;
            if bincode::serialize(&message)
                .map_err(|error| Error::new(format!("reserialize local Pyth message: {error}")))?
                != message_bytes
            {
                return Err(Error::new("local Pyth message encoding was noncanonical"));
            }
            let mut transaction = Transaction::new_unsigned(message);
            let blockhash = Hash::from_str(&journal.intent.recent_blockhash)
                .map_err(|error| Error::new(format!("local Pyth blockhash: {error}")))?;
            let signers: Vec<&dyn Signer> = match encoded.as_ref() {
                Some(encoded) => vec![&payer, encoded],
                None => vec![&payer],
            };
            transaction
                .try_sign(&signers, blockhash)
                .map_err(|error| Error::new(format!("sign local Pyth transaction: {error}")))?;
            transaction
                .verify()
                .map_err(|error| Error::new(format!("verify local Pyth signatures: {error}")))?;
            let packet = bincode::serialize(&transaction).map_err(|error| {
                Error::new(format!("serialize local Pyth transaction: {error}"))
            })?;
            if packet.len() != journal.intent.expected_wire_bytes || packet.len() > PACKET_BYTES_V1
            {
                return Err(Error::new(
                    "local Pyth signed packet width differed from its durable intent",
                ));
            }
            let signature = transaction.signatures[0];
            journal.signed_packet_base64 = Some(BASE64.encode(&packet));
            journal.expected_signature = Some(signature.to_string());
            journal.phase = PhaseV1::SignedNotSubmitted;
            refresh_journal_digest_v1(journal)?;
            write_journal_v1(path, journal, false)?;
            require_current_prestate_v1(rpc, journal)?;
            journal.phase = PhaseV1::Submitted;
            refresh_journal_digest_v1(journal)?;
            write_journal_v1(path, journal, false)?;
            let returned = rpc
                .call(
                    "sendTransaction",
                    &json!([BASE64.encode(&packet), {
                        "encoding":"base64",
                        "skipPreflight":false,
                        "preflightCommitment":"finalized",
                        "maxRetries":0
                    }]),
                )?
                .as_str()
                .ok_or_else(|| Error::new("local Pyth sendTransaction omitted signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("local Pyth returned signature: {error}")))?;
            if returned != signature {
                return Err(Error::new(
                    "local Pyth RPC returned another packet signature",
                ));
            }
            let transaction = wait_finalized_v1(rpc, &signature.to_string())?;
            finalize_journal_v1(rpc, arguments, addresses, journal, transaction)?;
            write_journal_v1(path, journal, false)
        }
    }
}

fn finalize_journal_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
    journal: &mut JournalV1,
    transaction: Value,
) -> Result<()> {
    let finalized = finalized_evidence_v1(rpc, journal, &transaction)?;
    journal.finalized = Some(finalized);
    journal.phase = PhaseV1::Finalized;
    refresh_journal_digest_v1(journal)?;
    let snapshot = snapshot_v1(rpc, arguments, addresses)?;
    authenticate_release_v1(&snapshot, arguments, addresses)?;
    let next = next_action_v1(arguments, addresses, &snapshot)?;
    if next.is_some_and(|next| next <= journal.intent.action) {
        return Err(Error::new(
            "finalized local Pyth transaction did not advance its authenticated provider state",
        ));
    }
    Ok(())
}

fn finalized_evidence_v1(
    rpc: &mut Rpc,
    journal: &JournalV1,
    transaction: &Value,
) -> Result<FinalizedV1> {
    let meta = transaction
        .get("meta")
        .ok_or_else(|| Error::new("local Pyth finalized transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "local Pyth transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let tuple = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("local Pyth finalized history omitted transaction tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(Error::new(
            "local Pyth finalized transaction was not exact base64 history",
        ));
    }
    let packet = BASE64
        .decode(
            tuple
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("local Pyth history omitted packet"))?,
        )
        .map_err(|error| Error::new(format!("local Pyth history packet: {error}")))?;
    let durable = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| Error::new("local Pyth journal omitted signed packet"))?,
        )
        .map_err(|error| Error::new(format!("local Pyth durable packet: {error}")))?;
    if packet != durable {
        return Err(Error::new(
            "local Pyth finalized packet differed byte-for-byte from its journal",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("local Pyth finalized fee was not u64"))?;
    if fee != journal.intent.exact_fee_lamports {
        return Err(Error::new(
            "local Pyth finalized fee differed from its exact durable message fee",
        ));
    }
    let vector = |name: &str| -> Result<Vec<u64>> {
        meta.get(name)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("local Pyth {name} was not an array")))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| Error::new(format!("local Pyth {name} entry was not u64")))
            })
            .collect()
    };
    let pre_balances = vector("preBalances")?;
    let post_balances = vector("postBalances")?;
    if pre_balances != journal.intent.pre_balances
        || pre_balances.len() != journal.intent.resolved_account_keys.len()
        || post_balances.len() != pre_balances.len()
    {
        return Err(Error::new(
            "local Pyth finalized key/pre/post balance vector changed",
        ));
    }
    let pre_total = pre_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    let post_total = post_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    if pre_total.and_then(|pre| post_total.and_then(|post| pre.checked_sub(post)))
        != Some(u128::from(fee))
    {
        return Err(Error::new(
            "local Pyth transaction balance vector concealed a lamport delta beyond its fee",
        ));
    }
    let (return_data_producer, return_data_base64) = parse_return_data_v1(meta)?;
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("local Pyth finalized slot was not u64"))?;
    let keys = journal
        .intent
        .resolved_account_keys
        .iter()
        .map(|key| pubkey(key))
        .collect::<Result<Vec<_>>>()?;
    let (post_slot, values) = rpc.finalized_accounts(&keys, slot)?;
    if post_slot < slot {
        return Err(Error::new("local Pyth poststate preceded finalization"));
    }
    let poststate = keys
        .into_iter()
        .zip(values.iter())
        .map(|(key, account)| (key.to_string(), account_state_v1(key, account.as_ref())))
        .collect();
    Ok(FinalizedV1 {
        signature: journal
            .expected_signature
            .clone()
            .ok_or_else(|| Error::new("local Pyth finalized journal omitted signature"))?,
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        packet_sha256: sha256_hex_v1(&packet),
        pre_balances,
        post_balances,
        return_data_producer,
        return_data_base64,
        poststate,
    })
}

fn finalized_transaction_v1(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let statuses = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = statuses
        .get("value")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .filter(|value| !value.is_null());
    let Some(status) = status else {
        return Ok(None);
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(None);
    }
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"base64",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(Error::new(
            "finalized local Pyth signature omitted transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn wait_finalized_v1(rpc: &mut Rpc, signature: &str) -> Result<Value> {
    let deadline = Instant::now() + FINALITY_WAIT_V1;
    while Instant::now() < deadline {
        if let Some(transaction) = finalized_transaction_v1(rpc, signature)? {
            return Ok(transaction);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(Error::new(format!(
        "local Pyth transaction {signature} did not reach finalized history within 60 seconds; its durable signature is retained and no replay is attempted"
    )))
}

fn authenticate_finalized_history_v1(rpc: &mut Rpc, journal: &JournalV1) -> Result<()> {
    let expected = journal
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized local Pyth journal omitted evidence"))?;
    let transaction = finalized_transaction_v1(rpc, &expected.signature)?
        .ok_or_else(|| Error::new("persisted local Pyth finalization disappeared"))?;
    let mut observed = finalized_evidence_v1(rpc, journal, &transaction)?;
    // Account bytes are a point-in-time receipt. Later canonical steps
    // legitimately mutate the same EncodedVaa, so archived journals recheck
    // only immutable transaction history and retain (rather than reread) the
    // exact poststate captured when that step finalized.
    observed.poststate = expected.poststate.clone();
    if &observed != expected {
        return Err(Error::new(
            "persisted local Pyth finalized packet, fee, balances, or return data changed",
        ));
    }
    Ok(())
}

fn require_current_prestate_v1(rpc: &mut Rpc, journal: &JournalV1) -> Result<()> {
    let keys = journal
        .intent
        .resolved_account_keys
        .iter()
        .map(|key| pubkey(key))
        .collect::<Result<Vec<_>>>()?;
    let (_slot, values) = rpc.finalized_accounts(&keys, journal.intent.observation_slot)?;
    for (key, account) in keys.iter().copied().zip(values.iter()) {
        // Clock and Rent legitimately advance without any mutation by this
        // lifecycle.  Their identities and readonly privileges are bound in
        // the exact message; every writable/provider prestate stays exact.
        if key == sysvar::clock::ID || key == sysvar::rent::ID {
            continue;
        }
        let expected = journal
            .intent
            .prestate
            .get(&key.to_string())
            .ok_or_else(|| Error::new("local Pyth intent omitted exact prestate"))?;
        if &account_state_v1(key, account.as_ref()) != expected {
            return Err(Error::new(
                "local Pyth provider or payer prestate changed before its sole signature",
            ));
        }
    }
    Ok(())
}

fn authenticate_complete_v1(
    arguments: &ArgumentsV1,
    addresses: &AddressesV1,
    snapshot: &SnapshotV1,
) -> Result<()> {
    authenticate_release_v1(snapshot, arguments, addresses)?;
    if snapshot.optional(arguments.update_account).is_some() {
        return Err(Error::new(
            "local Pyth facts require the Receiver update signer to remain vacant",
        ));
    }
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    let release = local.release();
    let config = snapshot.required(addresses.config, "Receiver Config")?;
    let config_view = ReceiverConfigV2View::parse(&config.data)
        .map_err(|error| Error::new(format!("local Receiver Config: {error:?}")))?;
    if config.owner != addresses.receiver
        || config.executable
        || config.data.as_slice() != RECEIVER_CONFIG
        || sha256_hex_v1(&config.data) != RECEIVER_CONFIG_SHA256_V1
        || config_view.router_program() != addresses.router.to_bytes()
        || config_view.data_source_count() != 1
        || config_view.fee() != 1
        // Receiver acceptance policy and Router VAA quorum are separate
        // persisted facts in this superseded 19-guardian lab capture: 5 and
        // strict-majority 10 respectively.  The exact Config digest above
        // binds the former; the GuardianSet authentication below binds the
        // latter.
        || config_view.minimum_signatures() != LOCAL_RECEIVER_MINIMUM_SIGNATURES_V1
        || Sha256::digest(&config.data).as_slice() != release.config_digest()
    {
        return Err(Error::new(
            "local Receiver Config owner, exact fixture bytes, Router, source, fee, or threshold refused",
        ));
    }
    let encoded = snapshot.required(arguments.encoded_vaa, "verified EncodedVaa")?;
    let verified = VerifiedEncodedVaaV1::parse(&encoded.data)
        .map_err(|error| Error::new(format!("local verified EncodedVaa: {error:?}")))?;
    if encoded.owner != addresses.router
        || encoded.executable
        || verified.write_authority() != arguments.payer.to_bytes()
        || verified.guardian_set_index() != 0
        || verified.signature_count() != 13
        || verified.signed_vaa() != SIGNED_VAA
    {
        return Err(Error::new(
            "local verified EncodedVaa owner, authority, guardian set, quorum, or exact VAA bytes refused",
        ));
    }
    let guardian = snapshot.required(addresses.guardian, "Router GuardianSet")?;
    let guardian_view = GuardianSetV1::parse(&guardian.data)
        .map_err(|error| Error::new(format!("local Router GuardianSet: {error:?}")))?;
    if guardian.owner != addresses.router
        || guardian.executable
        || guardian_view.index() != 0
        || guardian_view.expiration_time() != 0
        || guardian_view
            .authenticate(
                verified,
                release.guardian_set_count(),
                release.required_guardian_count(),
            )
            .is_err()
    {
        return Err(Error::new(
            "local GuardianSet owner, PDA body, active lifetime, or VAA quorum refused",
        ));
    }
    let bridge = snapshot.required(addresses.bridge, "Router Bridge")?;
    if bridge.owner != addresses.router || bridge.executable || bridge.data.is_empty() {
        return Err(Error::new(
            "local Router Bridge owner, privilege, or body refused",
        ));
    }
    let treasury = snapshot.required(addresses.treasury, "Receiver treasury")?;
    if treasury.owner != system_program::ID
        || treasury.executable
        || !treasury.data.is_empty()
        || treasury.lamports < rent_minimum_from_snapshot(snapshot, 0)?
    {
        return Err(Error::new(
            "local Receiver treasury owner, width, privilege, or capitalization refused",
        ));
    }
    let body = RECEIVER_POST_UPDATE
        .get(8..)
        .ok_or_else(|| Error::new("captured Receiver PostUpdate omitted its body"))?;
    PostUpdateParamsView::parse(body)
        .map_err(|error| Error::new(format!("captured PostUpdate body: {error:?}")))?;
    Ok(())
}

fn require_canonical_journal_prefix_v1(arguments: &ArgumentsV1, next: ActionV1) -> Result<()> {
    for action in ActionV1::ORDERED {
        let path = arguments.journal_dir.join(action.file_name());
        if action < next {
            if !path.is_file() {
                return Err(Error::new(format!(
                    "local Pyth chain state advanced past {:?} without its durable journal",
                    action
                )));
            }
            let bytes = fs::read(&path)?;
            let _: Value = parse_json_without_duplicate_keys_v1(&bytes)?;
            let journal: JournalV1 = serde_json::from_slice(&bytes)?;
            if journal.phase != PhaseV1::Finalized {
                return Err(Error::new(
                    "local Pyth journal prefix contains a non-finalized predecessor",
                ));
            }
        } else if path.exists() {
            return Err(Error::new(
                "local Pyth journal prefix contains a future or duplicate action",
            ));
        }
    }
    Ok(())
}

fn require_complete_journals_v1(arguments: &ArgumentsV1) -> Result<()> {
    for action in ActionV1::ORDERED {
        let path = arguments.journal_dir.join(action.file_name());
        let bytes = fs::read(&path).map_err(|error| {
            Error::new(format!(
                "complete local Pyth provider omitted {:?} journal: {error}",
                action
            ))
        })?;
        let _: Value = parse_json_without_duplicate_keys_v1(&bytes)?;
        let journal: JournalV1 = serde_json::from_slice(&bytes)?;
        if journal.phase != PhaseV1::Finalized || journal.intent.action != action {
            return Err(Error::new(
                "complete local Pyth provider journal prefix changed",
            ));
        }
    }
    Ok(())
}

fn write_facts_v1(arguments: &ArgumentsV1) -> Result<()> {
    let body = RECEIVER_POST_UPDATE
        .get(8..)
        .ok_or_else(|| Error::new("captured Receiver PostUpdate omitted its body"))?;
    let facts = ProducerPythFactsV1 {
        format: FACTS_SCHEMA_V1.into(),
        encoded_vaa: arguments.encoded_vaa.to_string(),
        update_account: arguments.update_account.to_string(),
        post_update_body_base64: BASE64.encode(body),
    };
    let mut bytes = serde_json::to_vec_pretty(&facts)?;
    bytes.push(b'\n');
    write_create_or_exact_v1(&arguments.facts_output, &bytes, "local Pyth facts")
}

fn authenticate_fixture_constants_v1() -> Result<()> {
    for (label, bytes, expected) in [
        (
            "router-initialize.data",
            ROUTER_INITIALIZE,
            ROUTER_INITIALIZE_SHA256_V1,
        ),
        (
            "receiver-initialize.data",
            RECEIVER_INITIALIZE,
            RECEIVER_INITIALIZE_SHA256_V1,
        ),
        ("signed.vaa", SIGNED_VAA, SIGNED_VAA_SHA256_V1),
        (
            "receiver-post-update.data",
            RECEIVER_POST_UPDATE,
            RECEIVER_POST_UPDATE_SHA256_V1,
        ),
        (
            "receiver-config.account",
            RECEIVER_CONFIG,
            RECEIVER_CONFIG_SHA256_V1,
        ),
    ] {
        if sha256_hex_v1(bytes) != expected {
            return Err(Error::new(format!(
                "captured local Pyth fixture {label} digest changed"
            )));
        }
    }
    if SIGNED_VAA.len() != 952 {
        return Err(Error::new("captured signed VAA width changed"));
    }
    Ok(())
}

fn fixture_sha256_v1() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("receiverConfig".into(), RECEIVER_CONFIG_SHA256_V1.into()),
        (
            "receiverInitialize".into(),
            RECEIVER_INITIALIZE_SHA256_V1.into(),
        ),
        (
            "receiverPostUpdate".into(),
            RECEIVER_POST_UPDATE_SHA256_V1.into(),
        ),
        (
            "routerInitialize".into(),
            ROUTER_INITIALIZE_SHA256_V1.into(),
        ),
        ("signedVaa".into(), SIGNED_VAA_SHA256_V1.into()),
    ])
}

fn authenticate_journal_v1(
    journal: &JournalV1,
    arguments: &ArgumentsV1,
    genesis: &str,
    action: ActionV1,
) -> Result<()> {
    if journal.schema != JOURNAL_SCHEMA_V1
        || journal.cluster != "owned-loopback"
        || journal.intent.genesis_hash != genesis
        || journal.intent.rpc_url != arguments.origin.redacted_url()
        || journal.intent.action != action
        || journal.intent.payer != arguments.payer.to_string()
        || journal.intent.encoded_vaa != arguments.encoded_vaa.to_string()
        || journal.intent.update_account != arguments.update_account.to_string()
        || journal.intent.fixture_sha256 != fixture_sha256_v1()
        || journal.intent_sha256 != sha256_hex_v1(&serde_json::to_vec(&journal.intent)?)
        || journal_state_digest_v1(journal)? != journal.state_sha256
    {
        return Err(Error::new(
            "local Pyth journal schema, origin, identities, fixtures, intent, or state digest changed",
        ));
    }
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("compiled local Pyth release: {error:?}")))?;
    if journal.intent.release_sha256 != sha256_hex_v1(&local.release().to_bytes())
        || journal.intent.observation_slot == 0
        || journal.intent.resolved_account_keys.is_empty()
        || journal.intent.resolved_account_keys.len() != journal.intent.pre_balances.len()
        || journal.intent.expected_wire_bytes == 0
        || journal.intent.expected_wire_bytes > PACKET_BYTES_V1
    {
        return Err(Error::new(
            "local Pyth release, observation, key, balance, or packet evidence changed",
        ));
    }
    let blockhash = Hash::from_str(&journal.intent.recent_blockhash)
        .map_err(|error| Error::new(format!("local Pyth durable blockhash: {error}")))?;
    let program = pubkey(&journal.intent.instruction_program)?;
    let accounts = journal
        .intent
        .instruction_accounts
        .iter()
        .map(|(key, signer, writable)| {
            Ok(AccountMeta {
                pubkey: pubkey(key)?,
                is_signer: *signer,
                is_writable: *writable,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let data = BASE64
        .decode(&journal.intent.instruction_data_base64)
        .map_err(|error| Error::new(format!("local Pyth instruction base64: {error}")))?;
    let provider_instruction = Instruction {
        program_id: program,
        accounts,
        data,
    };
    let expected_instructions = match journal.intent.compute_unit_limit {
        Some(limit)
            if action == ActionV1::EncodedVaaVerify && limit == VERIFY_COMPUTE_UNIT_LIMIT_V1 =>
        {
            vec![
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                provider_instruction,
            ]
        }
        None if action != ActionV1::EncodedVaaVerify => vec![provider_instruction],
        _ => {
            return Err(Error::new(
                "local Pyth compute-unit limit was absent, misplaced, or changed",
            ));
        }
    };
    let expected =
        Message::new_with_blockhash(&expected_instructions, Some(&arguments.payer), &blockhash);
    let message_bytes = BASE64
        .decode(&journal.intent.message_base64)
        .map_err(|error| Error::new(format!("local Pyth message base64: {error}")))?;
    if sha256_hex_v1(&message_bytes) != journal.intent.message_sha256
        || bincode::serialize(&expected).map_err(|error| {
            Error::new(format!("serialize local Pyth expected message: {error}"))
        })? != message_bytes
        || expected
            .account_keys
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            != journal.intent.resolved_account_keys
        || expected
            .account_keys
            .iter()
            .take(usize::from(expected.header.num_required_signatures))
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            != journal.intent.required_signers
    {
        return Err(Error::new(
            "local Pyth durable instruction did not recompile to its exact message",
        ));
    }
    match journal.phase {
        PhaseV1::Planned
            if journal.signed_packet_base64.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none() => {}
        PhaseV1::SignedNotSubmitted | PhaseV1::Submitted
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_none()
                && journal.authorized_mutation =>
        {
            authenticate_signed_packet_v1(journal)?;
        }
        PhaseV1::Finalized
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_some()
                && journal.authorized_mutation =>
        {
            authenticate_signed_packet_v1(journal)?;
            let finalized = journal.finalized.as_ref().expect("checked");
            let packet = BASE64
                .decode(journal.signed_packet_base64.as_deref().expect("checked"))
                .map_err(|error| Error::new(format!("local Pyth packet base64: {error}")))?;
            if finalized.signature != journal.expected_signature.as_deref().unwrap_or_default()
                || finalized.packet_sha256 != sha256_hex_v1(&packet)
                || finalized.fee_lamports != journal.intent.exact_fee_lamports
                || finalized.pre_balances != journal.intent.pre_balances
            {
                return Err(Error::new(
                    "local Pyth finalized signature, packet, fee, or pre-balance evidence changed",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                "local Pyth journal phase and durable evidence shape is noncanonical",
            ));
        }
    }
    Ok(())
}

fn authenticate_signed_packet_v1(journal: &JournalV1) -> Result<()> {
    let packet = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| Error::new("local Pyth signed phase omitted packet"))?,
        )
        .map_err(|error| Error::new(format!("local Pyth packet base64: {error}")))?;
    let transaction: Transaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("local Pyth signed packet: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("local Pyth durable signatures: {error}")))?;
    let message = bincode::serialize(&transaction.message)
        .map_err(|error| Error::new(format!("local Pyth signed message: {error}")))?;
    if BASE64.encode(&message) != journal.intent.message_base64
        || packet.len() != journal.intent.expected_wire_bytes
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != journal.expected_signature.as_deref()
    {
        return Err(Error::new(
            "local Pyth signed packet changed its durable message, width, or transaction id",
        ));
    }
    Ok(())
}

fn refresh_journal_digest_v1(journal: &mut JournalV1) -> Result<()> {
    journal.intent_sha256 = sha256_hex_v1(&serde_json::to_vec(&journal.intent)?);
    journal.state_sha256.clear();
    journal.state_sha256 = journal_state_digest_v1(journal)?;
    Ok(())
}

fn journal_state_digest_v1(journal: &JournalV1) -> Result<String> {
    let mut projected = journal.clone();
    projected.state_sha256.clear();
    Ok(sha256_hex_v1(&serde_json::to_vec(&projected)?))
}

fn write_journal_v1(path: &Path, journal: &JournalV1, create_new: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("local Pyth journal path must be absolute"));
    }
    let mut bytes = serde_json::to_vec_pretty(journal)?;
    bytes.push(b'\n');
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("local Pyth journal omitted parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("local Pyth journal omitted UTF-8 file name"))?;
    let temporary = parent.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        std::thread::current().name().unwrap_or("writer")
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create local Pyth journal temporary: {error}")))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    let result = if create_new {
        fs::hard_link(&temporary, path).map(|()| ())
    } else {
        fs::rename(&temporary, path)
    };
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(Error::new(format!("persist local Pyth journal: {error}")));
    }
    if create_new {
        fs::remove_file(&temporary)?;
    }
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn write_create_or_exact_v1(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if let Ok(current) = fs::read(path) {
        if current == bytes {
            return Ok(());
        }
        return Err(Error::new(format!(
            "existing {label} differs from finalized local provider facts"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} omitted parent")))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new(format!("{label} omitted UTF-8 name")))?;
    let temporary = parent.join(format!(".{name}.{}.publish.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)?;
            OpenOptions::new().read(true).open(parent)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            if fs::read(path)? == bytes {
                Ok(())
            } else {
                Err(Error::new(format!("raced {label} differed")))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(Error::new(format!("publish {label}: {error}")))
        }
    }
}

fn parse_return_data_v1(meta: &Value) -> Result<(Option<String>, Option<String>)> {
    let Some(return_data) = meta.get("returnData").filter(|value| !value.is_null()) else {
        return Ok((None, None));
    };
    let producer = return_data
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("local Pyth return data omitted producer"))?
        .to_owned();
    let tuple = return_data
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("local Pyth return data omitted body tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(Error::new(
            "local Pyth return data tuple was not exactly [body, base64]",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("local Pyth return data omitted base64 body"))?;
    let body = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("local Pyth return data base64: {error}")))?;
    if BASE64.encode(&body) != encoded {
        return Err(Error::new("local Pyth return data base64 was noncanonical"));
    }
    Ok((Some(producer), Some(encoded.to_owned())))
}

fn latest_blockhash_v1(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let result = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = result
        .get("value")
        .ok_or_else(|| Error::new("local Pyth getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("local Pyth getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("local Pyth latest blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("local Pyth latest blockhash omitted validity height"))?;
    Ok((blockhash, last_valid))
}

fn fee_for_message_v1(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("local Pyth getFeeForMessage omitted exact fee"))
}

fn rent_minimum_from_snapshot(snapshot: &SnapshotV1, bytes: usize) -> Result<u64> {
    let rent: Rent =
        bincode::deserialize(&snapshot.required(sysvar::rent::ID, "Rent sysvar")?.data)
            .map_err(|error| Error::new(format!("local Pyth Rent sysvar: {error}")))?;
    Ok(rent.minimum_balance(bytes))
}

fn account_state_v1(key: Pubkey, account: Option<&RpcAccount>) -> AccountStateV1 {
    let (owner, lamports, executable, rent_epoch, data): (Pubkey, u64, bool, u64, &[u8]) =
        match account {
            Some(account) => (
                account.owner,
                account.lamports,
                account.executable,
                account.rent_epoch,
                &account.data,
            ),
            None => (system_program::ID, 0, false, 0, &[]),
        };
    let mut exact = Sha256::new();
    exact.update(owner.as_ref());
    exact.update(lamports.to_le_bytes());
    exact.update([u8::from(executable)]);
    exact.update(rent_epoch.to_le_bytes());
    exact.update(u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes());
    exact.update(data);
    AccountStateV1 {
        address: key.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        data_len: data.len(),
        data_sha256: sha256_hex_v1(data),
        account_sha256: hex(&exact.finalize()),
    }
}

fn load_keypair_v1(path: &Path, label: &str, expected: Pubkey) -> Result<Keypair> {
    let keypair = Keypair::new_from_array(campaign::read_keypair_file(path, label)?);
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair does not expand to its declared public key"
        )));
    }
    Ok(keypair)
}

fn anchor_discriminator_v1(name: &[u8]) -> Vec<u8> {
    Sha256::digest(name).get(..8).unwrap_or_default().to_vec()
}

fn sha256_hex_v1(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn print_journal_v1(journal: &JournalV1) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(journal)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-private-validator-pyth-vaa-provision-v1 \\\n     --rpc-url http://127.0.0.1:PORT/ --payer PUBKEY --encoded-vaa PUBKEY \\\n     --update-account PUBKEY --journal-dir ABSOLUTE_EXISTING_DIR \\\n     --facts-output ABSOLUTE_JSON --payer-keypair ABSOLUTE_DISPOSABLE_JSON \\\n     --encoded-vaa-keypair ABSOLUTE_DISPOSABLE_JSON [--execute]\n\nThis command is \
     owned-loopback-only. It authenticates the slot-zero captured Router and Receiver ELFs, \
     executes their real prerequisite initialization and signed-VAA verification through \
     crash-safe exact-packet journals, and emits the existing \
     dclutch-flagship-pyth-update-facts-v1 DTO. The update account remains vacant; the \
     flagship resolution executor alone owns Receiver PostUpdate. Every external origin, \
     every devnet release row, and every already-populated update account is refused."
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static PROVIDER_TEST_NEXT: AtomicU64 = AtomicU64::new(0);

    struct ProviderTestRoot(PathBuf);

    impl ProviderTestRoot {
        fn new() -> Self {
            let path = fs::canonicalize(std::env::temp_dir())
                .expect("canonical temp")
                .join(format!(
                    "dclutch-provider-closure-test-{}-{}",
                    std::process::id(),
                    PROVIDER_TEST_NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir(&path).expect("create test root");
            Self(path)
        }

        fn write_capture(
            &self,
            name: &str,
            expected: &[ProviderExpectedV1],
            genesis: &str,
            slot: u64,
        ) -> PathBuf {
            let mut accounts = serde_json::Map::new();
            for row in expected {
                for (address, data, executable, lamports) in [
                    (
                        row.program_id,
                        &row.program_bytes,
                        true,
                        row.program_lamports,
                    ),
                    (
                        row.program_data_address,
                        &row.program_data_bytes,
                        false,
                        row.program_data_lamports,
                    ),
                ] {
                    accounts.insert(
                        address.to_string(),
                        json!({
                            "contextSlot": slot.to_string(),
                            "value": {
                                "lamports": lamports,
                                "owner": bpf_loader_upgradeable::ID.to_string(),
                                "data": [BASE64.encode(data), "base64"],
                                "executable": executable,
                                "rentEpoch": 0,
                                "space": data.len(),
                            }
                        }),
                    );
                }
            }
            let capture = json!({
                "schema": owned_loopback_capture::SCHEMA_V1,
                "genesisHash": genesis,
                "commitment": "finalized",
                "finalizedSlot": slot.to_string(),
                "transactions": {},
                "accounts": accounts,
            });
            let path = self.0.join(name);
            let mut bytes = serde_json::to_vec_pretty(&capture).expect("capture JSON");
            bytes.push(b'\n');
            fs::write(&path, bytes).expect("write capture");
            path
        }
    }

    impl Drop for ProviderTestRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove scoped test root");
        }
    }

    fn provider_test_expected() -> Vec<ProviderExpectedV1> {
        let local = local_validator_release_v1().expect("local release");
        let release = local.release();
        [
            (
                "pyth-receiver",
                Pubkey::new_from_array(release.receiver_program()),
                Pubkey::new_from_array(release.receiver_programdata()),
                LOCAL_PYTH_RECEIVER_ELF,
            ),
            (
                "pyth-router",
                Pubkey::new_from_array(release.router_program()),
                Pubkey::new_from_array(release.router_programdata()),
                LOCAL_PYTH_ROUTER_ELF,
            ),
        ]
        .into_iter()
        .map(|(role, program_id, program_data_address, elf)| {
            let mut program_bytes = vec![0_u8; 36];
            program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
            program_bytes[4..].copy_from_slice(program_data_address.as_ref());
            let mut program_data_bytes = vec![0_u8; 45];
            program_data_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
            program_data_bytes.extend_from_slice(elf);
            let program_lamports = Rent::default().minimum_balance(program_bytes.len());
            let program_data_lamports = Rent::default().minimum_balance(program_data_bytes.len());
            ProviderExpectedV1 {
                role,
                program_id,
                program_data_address,
                elf_sha256: sha256_hex_v1(elf),
                program_data_sha256: sha256_hex_v1(&program_data_bytes),
                program_account_sha256: account_sha256_v1(
                    bpf_loader_upgradeable::ID,
                    program_lamports,
                    true,
                    0,
                    &program_bytes,
                )
                .expect("Program account digest"),
                program_data_account_sha256: account_sha256_v1(
                    bpf_loader_upgradeable::ID,
                    program_data_lamports,
                    false,
                    0,
                    &program_data_bytes,
                )
                .expect("ProgramData account digest"),
                program_bytes,
                program_data_bytes,
                program_lamports,
                program_data_lamports,
            }
        })
        .collect()
    }

    fn provider_test_receipt(
        path: &Path,
        expected: &[ProviderExpectedV1],
    ) -> ProviderClosureReceiptV1 {
        let capture = owned_loopback_capture::authenticate_v1(path).expect("capture");
        ProviderClosureReceiptV1 {
            schema: PROVIDER_CLOSURE_SCHEMA_V1.into(),
            cluster: "owned-loopback".into(),
            genesis_hash: capture.genesis_hash.clone(),
            status: "finalized".into(),
            finalized_observation_slot: capture.finalized_slot.to_string(),
            plan: ProviderSourceRefV1 {
                path: "/unused-plan".into(),
                sha256: "00".repeat(32),
                schema: PROVIDER_PLAN_SCHEMA_V1.into(),
            },
            local_validator_profile: ProviderSourceRefV1 {
                path: "/unused-profile".into(),
                sha256: "00".repeat(32),
                schema: PROVIDER_PROFILE_SCHEMA_V1.into(),
            },
            finalized_capture: capture.reference_v1().expect("capture ref"),
            provider_programs: expected.iter().map(provider_closure_program_v1).collect(),
        }
    }

    #[test]
    fn captured_local_provider_fixtures_and_facts_shape_are_exact() {
        authenticate_fixture_constants_v1().expect("pinned fixtures");
        assert_eq!(SIGNED_VAA.len(), 952);
        assert_eq!(
            SIGNED_VAA
                .chunks(WRITE_CHUNK_BYTES_V1)
                .map(<[u8]>::len)
                .collect::<Vec<_>>(),
            [600, 352]
        );
        let body = &RECEIVER_POST_UPDATE[8..];
        PostUpdateParamsView::parse(body).expect("exact PostUpdate body");
        let facts = ProducerPythFactsV1 {
            format: FACTS_SCHEMA_V1.into(),
            encoded_vaa: Pubkey::new_unique().to_string(),
            update_account: Pubkey::new_unique().to_string(),
            post_update_body_base64: BASE64.encode(body),
        };
        let value = serde_json::to_value(facts).expect("facts");
        assert_eq!(
            value
                .as_object()
                .expect("object")
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "encodedVaa".into(),
                "format".into(),
                "postUpdateBodyBase64".into(),
                "updateAccount".into(),
            ])
        );
    }

    #[test]
    fn local_action_order_is_complete_and_disjoint() {
        assert_eq!(ActionV1::ORDERED.len(), 8);
        assert_eq!(
            ActionV1::ORDERED
                .iter()
                .map(|action| action.file_name())
                .collect::<BTreeSet<_>>()
                .len(),
            ActionV1::ORDERED.len()
        );
        assert!(ActionV1::EncodedVaaCreate.needs_encoded_signer());
        assert_eq!(VERIFY_COMPUTE_UNIT_LIMIT_V1, 400_000);
        assert_eq!(LOCAL_RECEIVER_MINIMUM_SIGNATURES_V1, 5);
        assert!(
            ActionV1::ORDERED
                .iter()
                .filter(|action| action.needs_encoded_signer())
                .count()
                == 1
        );
    }

    #[test]
    fn provider_receipt_reopens_capture_and_refuses_static_or_raw_substitution() {
        let root = ProviderTestRoot::new();
        let expected = provider_test_expected();
        let genesis = Pubkey::new_unique().to_string();
        let capture_path = root.write_capture("capture.json", &expected, &genesis, 10);
        let receipt = provider_test_receipt(&capture_path, &expected);
        authenticate_provider_receipt_capture_v1(&receipt, &expected).expect("exact capture");

        let mut hostile = receipt.clone();
        hostile.finalized_capture.sha256 = "00".repeat(32);
        assert!(authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err());
        let mut hostile = receipt.clone();
        hostile.finalized_capture.finalized_slot = "11".into();
        assert!(authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err());
        let mut hostile = receipt.clone();
        hostile.finalized_observation_slot = "11".into();
        assert!(authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err());
        let mut hostile = receipt.clone();
        hostile.genesis_hash = Pubkey::new_unique().to_string();
        assert!(authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err());
        let substitute = root.write_capture("substitute.json", &expected, &genesis, 12);
        let mut hostile = receipt.clone();
        hostile.finalized_capture.path = substitute.display().to_string();
        assert!(authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err());

        for (index, mutation) in ["tag", "slot", "link", "elf", "full", "substitute"]
            .into_iter()
            .enumerate()
        {
            let mutated = expected.clone();
            let path = root.write_capture(&format!("raw-{index}.json"), &mutated, &genesis, 10);
            let mut capture_value: Value =
                serde_json::from_slice(&fs::read(&path).expect("capture bytes"))
                    .expect("capture value");
            let receiver = &expected[0];
            let encoded = capture_value["accounts"]
                [receiver.program_data_address.to_string()]["value"]["data"][0]
                .as_str()
                .expect("ProgramData base64");
            let mut bytes = BASE64.decode(encoded).expect("ProgramData bytes");
            match mutation {
                "tag" => {
                    bytes[12] = 1;
                    bytes[13..45].copy_from_slice(system_program::ID.as_ref());
                }
                "slot" => bytes[4] = 1,
                "elf" => *bytes.last_mut().expect("ELF") ^= 1,
                "full" => bytes[13] = 1,
                "substitute" => bytes = expected[1].program_data_bytes.clone(),
                "link" => {
                    let encoded =
                        capture_value["accounts"][receiver.program_id.to_string()]["value"]["data"]
                            [0]
                        .as_str()
                        .expect("Program base64");
                    let mut program = BASE64.decode(encoded).expect("Program bytes");
                    program[4] ^= 1;
                    capture_value["accounts"][receiver.program_id.to_string()]["value"]["data"]
                        [0] = json!(BASE64.encode(program));
                }
                _ => unreachable!(),
            }
            if mutation != "link" {
                capture_value["accounts"][receiver.program_data_address.to_string()]["value"]["data"]
                    [0] = json!(BASE64.encode(&bytes));
                capture_value["accounts"][receiver.program_data_address.to_string()]["value"]["space"] =
                    json!(bytes.len());
            }
            fs::write(
                &path,
                serde_json::to_vec_pretty(&capture_value).expect("hostile capture"),
            )
            .expect("rewrite hostile capture");
            let hostile = provider_test_receipt(&path, &expected);
            assert!(
                authenticate_provider_receipt_capture_v1(&hostile, &expected).is_err(),
                "{mutation} capture substitution was accepted"
            );
        }
    }

    #[test]
    fn prerequisite_release_auth_refuses_some_authority_even_with_same_elf() {
        let expected = provider_test_expected();
        let addresses = addresses_v1().expect("addresses");
        let payer = Pubkey::new_unique();
        let mut accounts = BTreeMap::new();
        for row in &expected {
            accounts.insert(
                row.program_id,
                Some(RpcAccount {
                    lamports: row.program_lamports,
                    owner: bpf_loader_upgradeable::ID,
                    executable: true,
                    rent_epoch: 0,
                    data: row.program_bytes.clone(),
                }),
            );
            accounts.insert(
                row.program_data_address,
                Some(RpcAccount {
                    lamports: row.program_data_lamports,
                    owner: bpf_loader_upgradeable::ID,
                    executable: false,
                    rent_epoch: 0,
                    data: row.program_data_bytes.clone(),
                }),
            );
        }
        let receiver_data = accounts
            .get_mut(&addresses.receiver_programdata)
            .and_then(Option::as_mut)
            .expect("Receiver ProgramData");
        receiver_data.data[12] = 1;
        receiver_data.data[13..45].copy_from_slice(system_program::ID.as_ref());
        accounts.insert(
            payer,
            Some(RpcAccount {
                lamports: 1,
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
                data: Vec::new(),
            }),
        );
        let arguments = ArgumentsV1 {
            origin: ClusterOriginV1::parse("http://127.0.0.1:20890/", None).expect("origin"),
            payer,
            encoded_vaa: Pubkey::new_unique(),
            update_account: Pubkey::new_unique(),
            journal_dir: PathBuf::from("/unused"),
            facts_output: PathBuf::from("/unused-facts"),
            payer_keypair: PathBuf::from("/unused-payer"),
            encoded_vaa_keypair: PathBuf::from("/unused-vaa"),
            execute: false,
        };
        assert!(
            authenticate_release_v1(&SnapshotV1 { slot: 10, accounts }, &arguments, &addresses)
                .is_err()
        );
    }
}
