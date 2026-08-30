//! Caller-owned, restart-safe devnet provisioning of one Pyth EncodedVaa.
//!
//! Hermes supplies an accumulator update. This exterior exact-parses its one
//! signed VAA and one Merkle price update, authenticates the pinned devnet
//! Router/Receiver release, and asks the Router to create a verified
//! `EncodedVaa` whose write authority is the same submitter the flagship
//! resolution executor will use. The fresh Receiver update signer remains
//! vacant: its public half is handed to the existing
//! `dclutch-flagship-pyth-update-facts-v1` consumer without minting another
//! protocol or workflow DTO.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_pyth_svm::{
    ENCODED_VAA_DISCRIMINATOR_V1, ENCODED_VAA_HEADER_BYTES_V1, ENCODED_VAA_VERIFIED_STATUS_V1,
    GuardianSetV1, PostUpdateParamsView, ProgramDataV3View, ProgramV3View, ReceiverConfigV2View,
    VerifiedEncodedVaaV1, devnet_release_v1,
};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    keccak,
    pubkey::Pubkey,
};
use solana_sdk::{
    hash::Hash,
    message::Message,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::create_account;

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH},
    plan::{hex, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, parse_json_without_duplicate_keys_v1},
};

const JOURNAL_SCHEMA_V1: &str = "dclutch-devnet-pyth-vaa-provision-v1";
const FACTS_SCHEMA_V1: &str = "dclutch-flagship-pyth-update-facts-v1";
const HERMES_ACCUMULATOR_MAGIC_V1: &[u8; 4] = b"PNAU";
const WORMHOLE_ACCUMULATOR_MAGIC_V1: &[u8; 4] = b"AUWV";
const PYTHNET_EMITTER_CHAIN_V1: u16 = 26;
const PYTHNET_EMITTER_V1: [u8; 32] = *b"PythnetPythnetPythnetPythnetPyth";
const PRICE_FEED_MESSAGE_BYTES_V1: usize = 85;
const MERKLE_NODE_BYTES_V1: usize = 20;
const VAA_SIGNATURE_BYTES_V1: usize = 66;
const ROUTER_WRITE_CHUNK_BYTES_V1: usize = 600;
const MAX_HERMES_RESPONSE_BYTES_V1: usize = 2 * 1024 * 1024;
const MAX_SIGNED_VAA_BYTES_V1: usize = ROUTER_WRITE_CHUNK_BYTES_V1;
const SOLANA_PACKET_BYTES_V1: usize = 1_232;
const FINALITY_WAIT_V1: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    hermes: HermesInputV1,
    feed_id: [u8; 32],
    minimum_publish_time: i64,
    maximum_publish_time: i64,
    submitter: Pubkey,
    fee_payer: Pubkey,
    encoded_vaa: Pubkey,
    update_account: Pubkey,
    journal: PathBuf,
    facts_output: PathBuf,
    execute: bool,
    submitter_keypair: Option<PathBuf>,
    fee_payer_keypair: Option<PathBuf>,
    encoded_vaa_keypair: Option<PathBuf>,
    update_keypair: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HermesInputV1 {
    Online {
        base_url: String,
        api_token_file: Option<PathBuf>,
    },
    Offline {
        response: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
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
    rent_epoch: u64,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionEvidenceV1 {
    program_id: String,
    accounts: Vec<InstructionAccountV1>,
    data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HermesEvidenceV1 {
    source: String,
    response_sha256: String,
    accumulator_base64: String,
    accumulator_sha256: String,
    signed_vaa_sha256: String,
    guardian_set_index: u32,
    signature_count: u8,
    wormhole_sequence: u64,
    accumulator_slot: u64,
    accumulator_ring_size: u32,
    merkle_root_hex: String,
    proof_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IntentV1 {
    genesis_hash: String,
    release_sha256: String,
    release_upstream_commit: String,
    release_sdk_crate_sha256: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    feed_id: String,
    minimum_publish_time: i64,
    maximum_publish_time: i64,
    price: i64,
    confidence: u64,
    exponent: i32,
    publish_time: i64,
    submitter: String,
    fee_payer: String,
    encoded_vaa: String,
    update_account: String,
    facts_output: String,
    router_program: String,
    router_programdata: String,
    receiver_program: String,
    receiver_programdata: String,
    receiver_config: String,
    guardian_set: String,
    encoded_vaa_bytes: usize,
    encoded_vaa_rent_lamports: u64,
    exact_fee_lamports: u64,
    total_fee_payer_debit_lamports: u64,
    recent_blockhash: String,
    last_valid_block_height: u64,
    message_base64: String,
    message_sha256: String,
    expected_wire_bytes: usize,
    instructions: Vec<InstructionEvidenceV1>,
    prestate: BTreeMap<String, AccountStateV1>,
    expected_verified_encoded_vaa_base64: String,
    post_update_body_base64: String,
    hermes: HermesEvidenceV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    signed_packet_sha256: String,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    poststate: BTreeMap<String, AccountStateV1>,
    facts_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JournalV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    authorized_mutation: bool,
    phase: PhaseV1,
    intent_sha256: String,
    envelope_sha256: String,
    intent: IntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<FinalizedEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProducerPythFactsV1 {
    format: String,
    encoded_vaa: String,
    update_account: String,
    post_update_body_base64: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AccumulatorUpdateV1 {
    signed_vaa: Vec<u8>,
    message: Vec<u8>,
    proof: Vec<[u8; MERKLE_NODE_BYTES_V1]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedVaaViewV1<'a> {
    guardian_set_index: u32,
    signature_count: u8,
    emitter_chain: u16,
    emitter_address: [u8; 32],
    sequence: u64,
    accumulator_slot: u64,
    accumulator_ring_size: u32,
    merkle_root: [u8; MERKLE_NODE_BYTES_V1],
    bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PriceFeedMessageV1 {
    feed_id: [u8; 32],
    price: i64,
    confidence: u64,
    exponent: i32,
    publish_time: i64,
}

#[derive(Clone)]
struct ProvisionSnapshotV1 {
    slot: u64,
    unix_timestamp: i64,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

struct JournalFileV1 {
    path: PathBuf,
    expected_bytes: Option<Vec<u8>>,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    if arguments.origin.label() != "devnet" {
        return Err(Error::new(
            "Pyth VAA provisioning is devnet-only; use captured fixtures for local rehearsal",
        ));
    }
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    authenticate_genesis(&mut rpc, &arguments.origin)?;

    if arguments.journal.exists() {
        let bytes = fs::read(&arguments.journal)?;
        let mut journal: JournalV1 = serde_json::from_slice(&bytes)?;
        let mut file = JournalFileV1::existing(arguments.journal.clone(), bytes);
        authenticate_existing_journal(&journal, &arguments)?;
        if arguments.execute && !journal.authorized_mutation {
            journal.authorized_mutation = true;
            file.persist(&mut journal)?;
        }
        resume(&mut rpc, &arguments, &mut journal, &mut file)?;
        return print_journal(&journal);
    }

    let (response_source, response_bytes) = read_hermes(&arguments.hermes, arguments.feed_id)?;
    let accumulator_bytes = parse_hermes_response(&response_bytes)?;
    let accumulator = parse_accumulator_update(&accumulator_bytes)?;
    let vaa = parse_signed_vaa(&accumulator.signed_vaa)?;
    let price = parse_price_feed_message(&accumulator.message)?;
    authenticate_selected_update(&arguments, &accumulator, vaa, price)?;
    let post_update_body = post_update_body(&accumulator)?;
    PostUpdateParamsView::parse(&post_update_body)
        .map_err(|error| Error::new(format!("derived PostUpdateParams: {error:?}")))?;

    // This one-context read authenticates the complete upstream release and
    // every account that can affect the exact packet before a key file is read.
    let snapshot = acquire_snapshot(&mut rpc, &arguments, vaa.guardian_set_index)?;
    authenticate_release_snapshot(&arguments, &snapshot, vaa)?;
    let intent = build_intent(
        &mut rpc,
        &arguments,
        &snapshot,
        response_source,
        &response_bytes,
        &accumulator_bytes,
        &accumulator,
        vaa,
        price,
        &post_update_body,
    )?;
    let mut journal = JournalV1 {
        schema: JOURNAL_SCHEMA_V1.into(),
        cluster: "devnet".into(),
        rpc_url: arguments.origin.redacted_url(),
        authorized_mutation: arguments.execute,
        phase: PhaseV1::Planned,
        intent_sha256: sha256_hex(&serde_json::to_vec(&intent)?),
        envelope_sha256: String::new(),
        intent,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        finalized: None,
    };
    let mut file = JournalFileV1::vacant(arguments.journal.clone());
    file.persist(&mut journal)?;
    if arguments.execute {
        resume(&mut rpc, &arguments, &mut journal, &mut file)?;
    }
    print_journal(&journal)
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut hermes_url = None;
    let mut hermes_response = None;
    let mut hermes_api_token_file = None;
    let mut feed_id = None;
    let mut minimum_publish_time = None;
    let mut maximum_publish_time = None;
    let mut submitter = None;
    let mut fee_payer = None;
    let mut encoded_vaa = None;
    let mut update_account = None;
    let mut journal = None;
    let mut facts_output = None;
    let mut execute = false;
    let mut submitter_keypair = None;
    let mut fee_payer_keypair = None;
    let mut encoded_vaa_keypair = None;
    let mut update_keypair = None;
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
        match argument.as_str() {
            "--rpc-url" => set_once(&mut rpc_url, value, "--rpc-url")?,
            flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG => {
                set_once(&mut acknowledgment, value, DEVNET_ACKNOWLEDGMENT_FLAG)?
            }
            "--hermes-url" => set_once(&mut hermes_url, value, "--hermes-url")?,
            "--hermes-response" => set_once(
                &mut hermes_response,
                absolute_path(value, "--hermes-response")?,
                "--hermes-response",
            )?,
            "--hermes-api-token-file" => set_once(
                &mut hermes_api_token_file,
                absolute_path(value, "--hermes-api-token-file")?,
                "--hermes-api-token-file",
            )?,
            "--feed-id" => set_once(&mut feed_id, parse_hex32(&value, "--feed-id")?, "--feed-id")?,
            "--minimum-publish-time" => set_once(
                &mut minimum_publish_time,
                parse_i64(&value, "--minimum-publish-time")?,
                "--minimum-publish-time",
            )?,
            "--maximum-publish-time" => set_once(
                &mut maximum_publish_time,
                parse_i64(&value, "--maximum-publish-time")?,
                "--maximum-publish-time",
            )?,
            "--submitter" => set_once(&mut submitter, pubkey(&value)?, "--submitter")?,
            "--fee-payer" => set_once(&mut fee_payer, pubkey(&value)?, "--fee-payer")?,
            "--encoded-vaa" => set_once(&mut encoded_vaa, pubkey(&value)?, "--encoded-vaa")?,
            "--update-account" => {
                set_once(&mut update_account, pubkey(&value)?, "--update-account")?
            }
            "--journal" => set_once(
                &mut journal,
                absolute_path(value, "--journal")?,
                "--journal",
            )?,
            "--facts-output" => set_once(
                &mut facts_output,
                absolute_path(value, "--facts-output")?,
                "--facts-output",
            )?,
            "--submitter-keypair" => set_once(
                &mut submitter_keypair,
                absolute_path(value, "--submitter-keypair")?,
                "--submitter-keypair",
            )?,
            "--fee-payer-keypair" => set_once(
                &mut fee_payer_keypair,
                absolute_path(value, "--fee-payer-keypair")?,
                "--fee-payer-keypair",
            )?,
            "--encoded-vaa-keypair" => set_once(
                &mut encoded_vaa_keypair,
                absolute_path(value, "--encoded-vaa-keypair")?,
                "--encoded-vaa-keypair",
            )?,
            "--update-keypair" => set_once(
                &mut update_keypair,
                absolute_path(value, "--update-keypair")?,
                "--update-keypair",
            )?,
            _ => return Err(Error::new(format!("unknown Pyth VAA argument: {argument}"))),
        }
    }
    let origin =
        ClusterOriginV1::parse(&required(rpc_url, "--rpc-url")?, acknowledgment.as_deref())?;
    let hermes = match (hermes_url, hermes_response) {
        (Some(base_url), None) => HermesInputV1::Online {
            base_url,
            api_token_file: hermes_api_token_file,
        },
        (None, Some(response)) if hermes_api_token_file.is_none() => {
            HermesInputV1::Offline { response }
        }
        _ => {
            return Err(Error::new(
                "supply exactly one of --hermes-url or --hermes-response; an API token belongs only to --hermes-url",
            ));
        }
    };
    let minimum_publish_time = required(minimum_publish_time, "--minimum-publish-time")?;
    let maximum_publish_time = required(maximum_publish_time, "--maximum-publish-time")?;
    if minimum_publish_time > maximum_publish_time {
        return Err(Error::new(
            "--minimum-publish-time must not exceed --maximum-publish-time",
        ));
    }
    if !execute
        && [
            submitter_keypair.as_ref(),
            fee_payer_keypair.as_ref(),
            encoded_vaa_keypair.as_ref(),
            update_keypair.as_ref(),
        ]
        .iter()
        .any(|path| path.is_some())
    {
        return Err(Error::new(
            "keypair paths are refused during read-only preflight; add them only with --execute",
        ));
    }
    if execute
        && [
            submitter_keypair.as_ref(),
            fee_payer_keypair.as_ref(),
            encoded_vaa_keypair.as_ref(),
            update_keypair.as_ref(),
        ]
        .iter()
        .any(|path| path.is_none())
    {
        return Err(Error::new(
            "--execute requires submitter, fee-payer, encoded-VAA, and update keypair paths",
        ));
    }
    let arguments = ArgumentsV1 {
        origin,
        hermes,
        feed_id: required(feed_id, "--feed-id")?,
        minimum_publish_time,
        maximum_publish_time,
        submitter: required(submitter, "--submitter")?,
        fee_payer: required(fee_payer, "--fee-payer")?,
        encoded_vaa: required(encoded_vaa, "--encoded-vaa")?,
        update_account: required(update_account, "--update-account")?,
        journal: required(journal, "--journal")?,
        facts_output: required(facts_output, "--facts-output")?,
        execute,
        submitter_keypair,
        fee_payer_keypair,
        encoded_vaa_keypair,
        update_keypair,
    };
    authenticate_argument_aliases(&arguments)?;
    Ok(arguments)
}

fn authenticate_argument_aliases(arguments: &ArgumentsV1) -> Result<()> {
    let ephemeral = [arguments.encoded_vaa, arguments.update_account];
    if ephemeral[0] == ephemeral[1]
        || ephemeral.contains(&arguments.submitter)
        || ephemeral.contains(&arguments.fee_payer)
        || arguments.submitter == Pubkey::default()
        || arguments.fee_payer == Pubkey::default()
    {
        return Err(Error::new(
            "submitter, fee payer, EncodedVaa signer, and update signer must be nonzero with distinct ephemeral coordinates",
        ));
    }
    if arguments.execute {
        let paths = [
            arguments.submitter_keypair.as_ref().expect("checked"),
            arguments.fee_payer_keypair.as_ref().expect("checked"),
            arguments.encoded_vaa_keypair.as_ref().expect("checked"),
            arguments.update_keypair.as_ref().expect("checked"),
        ];
        for left in 0..paths.len() {
            for right in left + 1..paths.len() {
                let same_authority =
                    left == 0 && right == 1 && arguments.submitter == arguments.fee_payer;
                if paths[left] == paths[right] && !same_authority {
                    return Err(Error::new(
                        "distinct signer authorities may not alias one keypair path",
                    ));
                }
                if same_authority && paths[left] != paths[right] {
                    return Err(Error::new(
                        "an aliased submitter/fee payer must name one exact keypair path",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn read_hermes(input: &HermesInputV1, feed_id: [u8; 32]) -> Result<(String, Vec<u8>)> {
    match input {
        HermesInputV1::Offline { response } => Ok((
            format!("offline:{}", response.display()),
            bounded_file(response, MAX_HERMES_RESPONSE_BYTES_V1, "Hermes response")?,
        )),
        HermesInputV1::Online {
            base_url,
            api_token_file,
        } => fetch_hermes(base_url, api_token_file.as_deref(), feed_id),
    }
}

fn fetch_hermes(
    base_url: &str,
    api_token_file: Option<&Path>,
    feed_id: [u8; 32],
) -> Result<(String, Vec<u8>)> {
    let mut base =
        Url::parse(base_url).map_err(|error| Error::new(format!("Hermes base URL: {error}")))?;
    if base.scheme() != "https"
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(Error::new(
            "Hermes base URL must be credential-free HTTPS without query or fragment",
        ));
    }
    if !base.path().ends_with('/') {
        let path = format!("{}/", base.path());
        base.set_path(&path);
    }
    let mut url = base
        .join("v2/updates/price/latest")
        .map_err(|error| Error::new(format!("Hermes update URL: {error}")))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("ids[]", &hex(&feed_id));
        query.append_pair("encoding", "base64");
        query.append_pair("parsed", "false");
    }
    let client = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| Error::new(format!("build Hermes client: {error}")))?;
    let mut request = client.get(url.clone()).header("accept", "application/json");
    if let Some(path) = api_token_file {
        let token = bounded_file(path, 4_096, "Hermes API token")?;
        let token = std::str::from_utf8(&token)
            .map_err(|_| Error::new("Hermes API token was not UTF-8"))?
            .trim();
        if token.is_empty() || token.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(Error::new(
                "Hermes API token was empty or contained control bytes",
            ));
        }
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|error| Error::new(format!("Hermes transport: {error}")))?;
    if !response.status().is_success() {
        return Err(Error::new(format!(
            "Hermes returned HTTP {} from a no-redirect request",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_HERMES_RESPONSE_BYTES_V1 as u64)
    {
        return Err(Error::new("Hermes response exceeded the 2 MiB bound"));
    }
    let bytes = response
        .bytes()
        .map_err(|error| Error::new(format!("Hermes response body: {error}")))?
        .to_vec();
    if bytes.len() > MAX_HERMES_RESPONSE_BYTES_V1 {
        return Err(Error::new("Hermes response exceeded the 2 MiB bound"));
    }
    // The journal records the credential-free URL. Authorization material is
    // neither printed nor persisted.
    Ok((url.to_string(), bytes))
}

fn parse_hermes_response(bytes: &[u8]) -> Result<Vec<u8>> {
    let value = parse_json_without_duplicate_keys_v1(bytes)?;
    let root = value
        .as_object()
        .ok_or_else(|| Error::new("Hermes response was not an object"))?;
    if root.keys().any(|key| key != "binary" && key != "parsed") {
        return Err(Error::new(
            "Hermes response carried an unknown top-level field",
        ));
    }
    let binary = root
        .get("binary")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Hermes response omitted binary object"))?;
    if binary.len() != 2 || binary.get("encoding").and_then(Value::as_str) != Some("base64") {
        return Err(Error::new(
            "Hermes binary response must contain exactly base64 encoding and data",
        ));
    }
    let data = binary
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Hermes binary.data was not an array"))?;
    if data.len() != 1 {
        return Err(Error::new(
            "one-feed provisioning requires exactly one Hermes accumulator blob",
        ));
    }
    let encoded = data[0]
        .as_str()
        .ok_or_else(|| Error::new("Hermes binary.data entry was not a string"))?;
    let decoded = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Hermes accumulator base64: {error}")))?;
    if BASE64.encode(&decoded) != encoded {
        return Err(Error::new("Hermes accumulator base64 was not canonical"));
    }
    Ok(decoded)
}

fn parse_accumulator_update(bytes: &[u8]) -> Result<AccumulatorUpdateV1> {
    let mut cursor = WireCursorV1::new(bytes);
    if cursor.take_array::<4>()? != *HERMES_ACCUMULATOR_MAGIC_V1
        || cursor.take_u8()? != 1
        || cursor.take_u8()? != 0
    {
        return Err(Error::new(
            "Hermes accumulator must be exact PNAU major 1 minor 0",
        ));
    }
    let trailing = usize::from(cursor.take_u8()?);
    if trailing != 0 {
        return Err(Error::new(
            "Hermes accumulator extension bytes are not admitted by the pinned parser",
        ));
    }
    if cursor.take_u8()? != 0 {
        return Err(Error::new(
            "Hermes accumulator proof must be WormholeMerkle variant zero",
        ));
    }
    let vaa_len = usize::from(cursor.take_u16_be()?);
    let signed_vaa = cursor.take(vaa_len)?.to_vec();
    let update_count = cursor.take_u8()?;
    if update_count != 1 {
        return Err(Error::new(
            "one-feed provisioning requires exactly one Merkle price update",
        ));
    }
    let message_len = usize::from(cursor.take_u16_be()?);
    let message = cursor.take(message_len)?.to_vec();
    let proof_count = cursor.take_u8()?;
    let mut proof = Vec::with_capacity(usize::from(proof_count));
    for _ in 0..proof_count {
        proof.push(cursor.take_array()?);
    }
    if !cursor.is_empty() {
        return Err(Error::new("Hermes accumulator carried trailing bytes"));
    }
    Ok(AccumulatorUpdateV1 {
        signed_vaa,
        message,
        proof,
    })
}

fn parse_signed_vaa(bytes: &[u8]) -> Result<SignedVaaViewV1<'_>> {
    if bytes.len() > MAX_SIGNED_VAA_BYTES_V1 {
        return Err(Error::new(format!(
            "signed VAA is {} bytes; the current devnet 3-of-5 release must fit the measured one-packet {}-byte Router write bound",
            bytes.len(),
            MAX_SIGNED_VAA_BYTES_V1
        )));
    }
    let mut cursor = WireCursorV1::new(bytes);
    if cursor.take_u8()? != 1 {
        return Err(Error::new("signed VAA version must be one"));
    }
    let guardian_set_index = cursor.take_u32_be()?;
    let signature_count = cursor.take_u8()?;
    if signature_count == 0 {
        return Err(Error::new("signed VAA omitted guardian signatures"));
    }
    let mut prior = None;
    for _ in 0..signature_count {
        let guardian_index = cursor.take_u8()?;
        if prior.is_some_and(|index| guardian_index <= index) {
            return Err(Error::new(
                "signed VAA guardian indices must be strictly increasing",
            ));
        }
        prior = Some(guardian_index);
        cursor.take(VAA_SIGNATURE_BYTES_V1 - 1)?;
    }
    cursor.take_u32_be()?; // timestamp; authenticated by signatures in Router
    cursor.take_u32_be()?; // nonce
    let emitter_chain = cursor.take_u16_be()?;
    let emitter_address = cursor.take_array()?;
    let sequence = cursor.take_u64_be()?;
    cursor.take_u8()?; // consistency level
    if cursor.take_array::<4>()? != *WORMHOLE_ACCUMULATOR_MAGIC_V1 || cursor.take_u8()? != 0 {
        return Err(Error::new(
            "signed VAA payload was not exact AUWV Merkle variant zero",
        ));
    }
    let accumulator_slot = cursor.take_u64_be()?;
    let accumulator_ring_size = cursor.take_u32_be()?;
    let merkle_root = cursor.take_array()?;
    if !cursor.is_empty() || accumulator_ring_size == 0 {
        return Err(Error::new(
            "signed VAA accumulator payload trailed or named a zero ring",
        ));
    }
    Ok(SignedVaaViewV1 {
        guardian_set_index,
        signature_count,
        emitter_chain,
        emitter_address,
        sequence,
        accumulator_slot,
        accumulator_ring_size,
        merkle_root,
        bytes,
    })
}

fn parse_price_feed_message(bytes: &[u8]) -> Result<PriceFeedMessageV1> {
    if bytes.len() != PRICE_FEED_MESSAGE_BYTES_V1 || bytes.first() != Some(&0) {
        return Err(Error::new(
            "Hermes update must carry one exact 85-byte PriceFeedMessage variant",
        ));
    }
    let field = |start: usize, width: usize| -> Result<&[u8]> {
        bytes
            .get(start..start + width)
            .ok_or_else(|| Error::new("PriceFeedMessage field exceeded exact body"))
    };
    Ok(PriceFeedMessageV1 {
        feed_id: field(1, 32)?
            .try_into()
            .map_err(|_| Error::new("feed width"))?,
        price: i64::from_be_bytes(
            field(33, 8)?
                .try_into()
                .map_err(|_| Error::new("price width"))?,
        ),
        confidence: u64::from_be_bytes(
            field(41, 8)?
                .try_into()
                .map_err(|_| Error::new("confidence width"))?,
        ),
        exponent: i32::from_be_bytes(
            field(49, 4)?
                .try_into()
                .map_err(|_| Error::new("exponent width"))?,
        ),
        publish_time: i64::from_be_bytes(
            field(53, 8)?
                .try_into()
                .map_err(|_| Error::new("publish-time width"))?,
        ),
    })
}

fn authenticate_selected_update(
    arguments: &ArgumentsV1,
    update: &AccumulatorUpdateV1,
    vaa: SignedVaaViewV1<'_>,
    price: PriceFeedMessageV1,
) -> Result<()> {
    if vaa.emitter_chain != PYTHNET_EMITTER_CHAIN_V1
        || vaa.emitter_address != PYTHNET_EMITTER_V1
        || price.feed_id != arguments.feed_id
        || !(arguments.minimum_publish_time..=arguments.maximum_publish_time)
            .contains(&price.publish_time)
    {
        return Err(Error::new(
            "Hermes VAA emitter, feed id, or exact publication window refused",
        ));
    }
    let mut current = keccak160(&[&[0], &update.message]);
    for sibling in &update.proof {
        let (left, right) = if current <= *sibling {
            (&current, sibling)
        } else {
            (sibling, &current)
        };
        current = keccak160(&[&[1], left, right]);
    }
    if current != vaa.merkle_root {
        return Err(Error::new(
            "Hermes Merkle proof did not join the selected message to the signed VAA root",
        ));
    }
    Ok(())
}

fn post_update_body(update: &AccumulatorUpdateV1) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(4 + update.message.len() + 4 + update.proof.len() * 20 + 1);
    body.extend_from_slice(
        &u32::try_from(update.message.len())
            .map_err(|_| Error::new("Pyth message length exceeded u32"))?
            .to_le_bytes(),
    );
    body.extend_from_slice(&update.message);
    body.extend_from_slice(
        &u32::try_from(update.proof.len())
            .map_err(|_| Error::new("Pyth proof count exceeded u32"))?
            .to_le_bytes(),
    );
    for node in &update.proof {
        body.extend_from_slice(node);
    }
    body.push(0); // current devnet Config treasury id
    Ok(body)
}

fn keccak160(parts: &[&[u8]]) -> [u8; 20] {
    let digest = keccak::hashv(parts).to_bytes();
    let mut output = [0_u8; 20];
    output.copy_from_slice(&digest[..20]);
    output
}

fn acquire_snapshot(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    guardian_set_index: u32,
) -> Result<ProvisionSnapshotV1> {
    let release = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    let router = Pubkey::new_from_array(release.router_program());
    let guardian = Pubkey::find_program_address(
        &[b"GuardianSet", &guardian_set_index.to_be_bytes()],
        &router,
    )
    .0;
    let keys = vec![
        Pubkey::new_from_array(release.receiver_program()),
        Pubkey::new_from_array(release.receiver_programdata()),
        Pubkey::new_from_array(release.receiver_config()),
        router,
        Pubkey::new_from_array(release.router_programdata()),
        guardian,
        arguments.submitter,
        arguments.fee_payer,
        arguments.encoded_vaa,
        arguments.update_account,
        system_program::ID,
    ];
    let mut unique = BTreeSet::new();
    for key in &keys {
        if !unique.insert(*key) {
            // The only permitted stable alias is submitter == fee payer.
            if *key != arguments.submitter || arguments.submitter != arguments.fee_payer {
                return Err(Error::new(
                    "Pyth release, guardian, signer, or System coordinates unexpectedly aliased",
                ));
            }
        }
    }
    let ordered = unique.into_iter().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&ordered, 0)?;
    let unix_timestamp = rpc.block_time(slot)?;
    Ok(ProvisionSnapshotV1 {
        slot,
        unix_timestamp,
        accounts: ordered.into_iter().zip(values).collect(),
    })
}

fn authenticate_release_snapshot(
    arguments: &ArgumentsV1,
    snapshot: &ProvisionSnapshotV1,
    vaa: SignedVaaViewV1<'_>,
) -> Result<()> {
    let release = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    if release.cluster_id() != parse_genesis_pubkey(DEVNET_GENESIS_HASH)?.to_bytes() {
        return Err(Error::new(
            "compiled Pyth release did not name the admitted devnet genesis",
        ));
    }
    for (label, program_bytes, programdata_bytes, deployment_slot, elf_sha256) in [
        (
            "Receiver",
            release.receiver_program(),
            release.receiver_programdata(),
            release.receiver_deployment_slot(),
            release.receiver_abi_id(),
        ),
        (
            "Router",
            release.router_program(),
            release.router_programdata(),
            release.router_deployment_slot(),
            release.router_abi_id(),
        ),
    ] {
        let program_key = Pubkey::new_from_array(program_bytes);
        let data_key = Pubkey::new_from_array(programdata_bytes);
        let program = snapshot.required(program_key, &format!("{label} program"))?;
        let data = snapshot.required(data_key, &format!("{label} ProgramData"))?;
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{label} program: {error:?}")))?;
        let data_view = ProgramDataV3View::parse(&data.data)
            .map_err(|error| Error::new(format!("{label} ProgramData: {error:?}")))?;
        if program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || program_view.programdata() != data_key.to_bytes()
            || data.owner != bpf_loader_upgradeable::ID
            || data.executable
            || data_view.deployment_slot() != deployment_slot
            || snapshot.slot < deployment_slot
            || Sha256::digest(data_view.elf()).as_slice() != elf_sha256
        {
            return Err(Error::new(format!(
                "current {label} Program/ProgramData link, slot, owner, privilege, or ELF digest refused"
            )));
        }
    }
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let router = Pubkey::new_from_array(release.router_program());
    let config_key = Pubkey::new_from_array(release.receiver_config());
    let config = snapshot.required(config_key, "Receiver Config")?;
    let config_view = ReceiverConfigV2View::parse(&config.data)
        .map_err(|error| Error::new(format!("Receiver Config: {error:?}")))?;
    let source = config_view
        .data_source(0)
        .ok_or_else(|| Error::new("Receiver Config omitted Pythnet source"))?;
    if config.owner != receiver
        || config.executable
        || Sha256::digest(&config.data).as_slice() != release.config_digest()
        || config_view.router_program() != router.to_bytes()
        || config_view.data_source_count() != 1
        || source.emitter_chain() != PYTHNET_EMITTER_CHAIN_V1
        || source.emitter_address() != PYTHNET_EMITTER_V1
        || config_view.fee() != 0
        || config_view.minimum_signatures() != release.required_guardian_count()
    {
        return Err(Error::new(
            "Receiver Config address, owner, body, Router, source, fee, or threshold refused",
        ));
    }
    let guardian_key = Pubkey::find_program_address(
        &[b"GuardianSet", &vaa.guardian_set_index.to_be_bytes()],
        &router,
    )
    .0;
    let guardian = snapshot.required(guardian_key, "Router GuardianSet")?;
    let guardian_view = GuardianSetV1::parse(&guardian.data)
        .map_err(|error| Error::new(format!("Router GuardianSet: {error:?}")))?;
    let synthetic = expected_verified_encoded_vaa(arguments.submitter, vaa.bytes)?;
    let verified = VerifiedEncodedVaaV1::parse(&synthetic)
        .map_err(|error| Error::new(format!("projected verified EncodedVaa: {error:?}")))?;
    if guardian.owner != router
        || guardian.executable
        || guardian_view
            .authenticate(
                verified,
                release.guardian_set_count(),
                release.required_guardian_count(),
            )
            .is_err()
        || guardian_view.expiration_time() != 0
        || vaa.emitter_chain != source.emitter_chain()
        || vaa.emitter_address != source.emitter_address()
    {
        return Err(Error::new(
            "GuardianSet owner, PDA, active lifetime, threshold, or VAA source refused",
        ));
    }
    validate_guardian_indices(vaa.bytes, guardian_view.guardian_count())?;
    for (label, key) in [
        ("submitter", arguments.submitter),
        ("fee payer", arguments.fee_payer),
    ] {
        let account = snapshot.required(key, label)?;
        if account.owner != system_program::ID || account.executable || !account.data.is_empty() {
            return Err(Error::new(format!(
                "{label} must be an existing System-owned data-empty wallet"
            )));
        }
    }
    for (label, key) in [
        ("EncodedVaa signer", arguments.encoded_vaa),
        ("Receiver update signer", arguments.update_account),
    ] {
        if snapshot.optional(key).is_some() {
            return Err(Error::new(format!(
                "fresh {label} {key} must be vacant before provisioning"
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_intent(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    snapshot: &ProvisionSnapshotV1,
    response_source: String,
    response_bytes: &[u8],
    accumulator_bytes: &[u8],
    accumulator: &AccumulatorUpdateV1,
    vaa: SignedVaaViewV1<'_>,
    price: PriceFeedMessageV1,
    post_update_body: &[u8],
) -> Result<IntentV1> {
    let release = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    let router = Pubkey::new_from_array(release.router_program());
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let guardian = Pubkey::find_program_address(
        &[b"GuardianSet", &vaa.guardian_set_index.to_be_bytes()],
        &router,
    )
    .0;
    let encoded_bytes = ENCODED_VAA_HEADER_BYTES_V1
        .checked_add(accumulator.signed_vaa.len())
        .ok_or_else(|| Error::new("EncodedVaa allocation overflow"))?;
    let rent = rpc.minimum_balance(encoded_bytes)?;
    let instructions = provision_instructions(
        arguments.fee_payer,
        arguments.submitter,
        arguments.encoded_vaa,
        guardian,
        router,
        rent,
        &accumulator.signed_vaa,
    )?;
    let (recent_blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    let message =
        Message::new_with_blockhash(&instructions, Some(&arguments.fee_payer), &recent_blockhash);
    authenticate_message(
        &message,
        &instructions,
        arguments.fee_payer,
        arguments.submitter,
        arguments.encoded_vaa,
        recent_blockhash,
    )?;
    let message_bytes = bincode::serialize(&message)
        .map_err(|error| Error::new(format!("serialize Router message: {error}")))?;
    let message_base64 = BASE64.encode(&message_bytes);
    let exact_fee = fee_for_message(rpc, &message_base64)?;
    let expected_wire_bytes = bincode::serialize(&Transaction {
        signatures: vec![Signature::default(); usize::from(message.header.num_required_signatures)],
        message: message.clone(),
    })
    .map_err(|error| Error::new(format!("size Router packet: {error}")))?
    .len();
    if expected_wire_bytes > SOLANA_PACKET_BYTES_V1 {
        return Err(Error::new(format!(
            "current VAA compiles to {expected_wire_bytes} bytes, above Solana's {SOLANA_PACKET_BYTES_V1}-byte packet; a multi-packet Router upload journal is required"
        )));
    }
    let total = rent
        .checked_add(exact_fee)
        .ok_or_else(|| Error::new("Router rent+fee overflow"))?;
    if snapshot
        .required(arguments.fee_payer, "fee payer")?
        .lamports
        < total
    {
        return Err(Error::new(format!(
            "fee payer lacks the exact {rent} rent + {exact_fee} transaction fee"
        )));
    }
    let expected_verified = expected_verified_encoded_vaa(arguments.submitter, vaa.bytes)?;
    let mut prestate = BTreeMap::new();
    for (label, key) in [
        ("receiver_program", receiver),
        (
            "receiver_programdata",
            Pubkey::new_from_array(release.receiver_programdata()),
        ),
        (
            "receiver_config",
            Pubkey::new_from_array(release.receiver_config()),
        ),
        ("router_program", router),
        (
            "router_programdata",
            Pubkey::new_from_array(release.router_programdata()),
        ),
        ("guardian_set", guardian),
        ("submitter", arguments.submitter),
        ("fee_payer", arguments.fee_payer),
        ("encoded_vaa", arguments.encoded_vaa),
        ("update_account", arguments.update_account),
        ("system_program", system_program::ID),
    ] {
        prestate.insert(label.into(), account_state(key, snapshot.optional(key)));
    }
    let intent = IntentV1 {
        genesis_hash: DEVNET_GENESIS_HASH.into(),
        release_sha256: sha256_hex(&release.to_bytes()),
        release_upstream_commit: hex(&release.upstream_commit()),
        release_sdk_crate_sha256: hex(&release.sdk_crate_digest()),
        observation_slot: snapshot.slot,
        observation_unix_timestamp: snapshot.unix_timestamp,
        feed_id: hex(&arguments.feed_id),
        minimum_publish_time: arguments.minimum_publish_time,
        maximum_publish_time: arguments.maximum_publish_time,
        price: price.price,
        confidence: price.confidence,
        exponent: price.exponent,
        publish_time: price.publish_time,
        submitter: arguments.submitter.to_string(),
        fee_payer: arguments.fee_payer.to_string(),
        encoded_vaa: arguments.encoded_vaa.to_string(),
        update_account: arguments.update_account.to_string(),
        facts_output: arguments.facts_output.display().to_string(),
        router_program: router.to_string(),
        router_programdata: Pubkey::new_from_array(release.router_programdata()).to_string(),
        receiver_program: receiver.to_string(),
        receiver_programdata: Pubkey::new_from_array(release.receiver_programdata()).to_string(),
        receiver_config: Pubkey::new_from_array(release.receiver_config()).to_string(),
        guardian_set: guardian.to_string(),
        encoded_vaa_bytes: encoded_bytes,
        encoded_vaa_rent_lamports: rent,
        exact_fee_lamports: exact_fee,
        total_fee_payer_debit_lamports: total,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        message_base64,
        message_sha256: sha256_hex(&message_bytes),
        expected_wire_bytes,
        instructions: instructions.iter().map(instruction_evidence).collect(),
        prestate,
        expected_verified_encoded_vaa_base64: BASE64.encode(expected_verified),
        post_update_body_base64: BASE64.encode(post_update_body),
        hermes: HermesEvidenceV1 {
            source: response_source,
            response_sha256: sha256_hex(response_bytes),
            accumulator_base64: BASE64.encode(accumulator_bytes),
            accumulator_sha256: sha256_hex(accumulator_bytes),
            signed_vaa_sha256: sha256_hex(&accumulator.signed_vaa),
            guardian_set_index: vaa.guardian_set_index,
            signature_count: vaa.signature_count,
            wormhole_sequence: vaa.sequence,
            accumulator_slot: vaa.accumulator_slot,
            accumulator_ring_size: vaa.accumulator_ring_size,
            merkle_root_hex: hex(&vaa.merkle_root),
            proof_count: u8::try_from(accumulator.proof.len())
                .map_err(|_| Error::new("Merkle proof count exceeded u8"))?,
        },
    };
    authenticate_intent(&intent, arguments)?;
    Ok(intent)
}

fn provision_instructions(
    fee_payer: Pubkey,
    submitter: Pubkey,
    encoded_vaa: Pubkey,
    guardian_set: Pubkey,
    router: Pubkey,
    rent: u64,
    signed_vaa: &[u8],
) -> Result<Vec<Instruction>> {
    let size = ENCODED_VAA_HEADER_BYTES_V1
        .checked_add(signed_vaa.len())
        .ok_or_else(|| Error::new("EncodedVaa allocation overflow"))?;
    let mut instructions = vec![
        create_account(
            &fee_payer,
            &encoded_vaa,
            rent,
            u64::try_from(size).map_err(|_| Error::new("EncodedVaa size exceeded u64"))?,
            &router,
        ),
        Instruction {
            program_id: router,
            accounts: vec![
                AccountMeta::new_readonly(submitter, true),
                AccountMeta::new(encoded_vaa, false),
            ],
            data: anchor_discriminator(b"global:init_encoded_vaa"),
        },
    ];
    for (index, chunk) in signed_vaa.chunks(ROUTER_WRITE_CHUNK_BYTES_V1).enumerate() {
        let offset = index
            .checked_mul(ROUTER_WRITE_CHUNK_BYTES_V1)
            .ok_or_else(|| Error::new("Router write offset overflow"))?;
        let mut data = anchor_discriminator(b"global:write_encoded_vaa");
        data.extend_from_slice(
            &u32::try_from(offset)
                .map_err(|_| Error::new("Router write offset exceeded u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(
            &u32::try_from(chunk.len())
                .map_err(|_| Error::new("Router write length exceeded u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(chunk);
        instructions.push(Instruction {
            program_id: router,
            accounts: vec![
                AccountMeta::new_readonly(submitter, true),
                AccountMeta::new(encoded_vaa, false),
            ],
            data,
        });
    }
    instructions.push(Instruction {
        program_id: router,
        accounts: vec![
            AccountMeta::new_readonly(submitter, true),
            AccountMeta::new(encoded_vaa, false),
            AccountMeta::new_readonly(guardian_set, false),
        ],
        data: anchor_discriminator(b"global:verify_encoded_vaa_v1"),
    });
    Ok(instructions)
}

fn expected_verified_encoded_vaa(authority: Pubkey, signed_vaa: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = vec![0_u8; ENCODED_VAA_HEADER_BYTES_V1 + signed_vaa.len()];
    bytes[..8].copy_from_slice(&ENCODED_VAA_DISCRIMINATOR_V1);
    bytes[8] = ENCODED_VAA_VERIFIED_STATUS_V1;
    bytes[9..41].copy_from_slice(authority.as_ref());
    bytes[41] = 1;
    bytes[42..46].copy_from_slice(
        &u32::try_from(signed_vaa.len())
            .map_err(|_| Error::new("signed VAA length exceeded u32"))?
            .to_le_bytes(),
    );
    bytes[46..].copy_from_slice(signed_vaa);
    VerifiedEncodedVaaV1::parse(&bytes)
        .map_err(|error| Error::new(format!("projected EncodedVaa: {error:?}")))?;
    Ok(bytes)
}

fn validate_guardian_indices(bytes: &[u8], guardian_count: u8) -> Result<()> {
    let count = *bytes
        .get(5)
        .ok_or_else(|| Error::new("signed VAA omitted signature count"))?;
    let mut offset = 6_usize;
    for _ in 0..count {
        let index = *bytes
            .get(offset)
            .ok_or_else(|| Error::new("signed VAA omitted guardian index"))?;
        if index >= guardian_count {
            return Err(Error::new(
                "signed VAA selected a guardian outside the current set",
            ));
        }
        offset = offset
            .checked_add(VAA_SIGNATURE_BYTES_V1)
            .ok_or_else(|| Error::new("signed VAA signature offset overflow"))?;
    }
    Ok(())
}

fn anchor_discriminator(name: &[u8]) -> Vec<u8> {
    hash(name).to_bytes()[..8].to_vec()
}

fn instruction_evidence(instruction: &Instruction) -> InstructionEvidenceV1 {
    InstructionEvidenceV1 {
        program_id: instruction.program_id.to_string(),
        accounts: instruction
            .accounts
            .iter()
            .map(|account| InstructionAccountV1 {
                address: account.pubkey.to_string(),
                signer: account.is_signer,
                writable: account.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(&instruction.data),
    }
}

fn authenticate_intent(intent: &IntentV1, arguments: &ArgumentsV1) -> Result<()> {
    let release = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    if intent.genesis_hash != DEVNET_GENESIS_HASH
        || intent.release_sha256 != sha256_hex(&release.to_bytes())
        || intent.release_upstream_commit != hex(&release.upstream_commit())
        || intent.release_sdk_crate_sha256 != hex(&release.sdk_crate_digest())
        || intent.feed_id != hex(&arguments.feed_id)
        || intent.minimum_publish_time != arguments.minimum_publish_time
        || intent.maximum_publish_time != arguments.maximum_publish_time
        || pubkey(&intent.submitter)? != arguments.submitter
        || pubkey(&intent.fee_payer)? != arguments.fee_payer
        || pubkey(&intent.encoded_vaa)? != arguments.encoded_vaa
        || pubkey(&intent.update_account)? != arguments.update_account
        || intent.facts_output != arguments.facts_output.display().to_string()
        || pubkey(&intent.router_program)? != Pubkey::new_from_array(release.router_program())
        || pubkey(&intent.router_programdata)?
            != Pubkey::new_from_array(release.router_programdata())
        || pubkey(&intent.receiver_program)? != Pubkey::new_from_array(release.receiver_program())
        || pubkey(&intent.receiver_programdata)?
            != Pubkey::new_from_array(release.receiver_programdata())
        || pubkey(&intent.receiver_config)? != Pubkey::new_from_array(release.receiver_config())
    {
        return Err(Error::new(
            "durable Pyth intent changed cluster, release, feed, signer, output, or program identity",
        ));
    }
    let accumulator_bytes =
        decode_canonical_base64(&intent.hermes.accumulator_base64, "Hermes accumulator")?;
    if sha256_hex(&accumulator_bytes) != intent.hermes.accumulator_sha256 {
        return Err(Error::new("durable Hermes accumulator digest changed"));
    }
    let accumulator = parse_accumulator_update(&accumulator_bytes)?;
    let vaa = parse_signed_vaa(&accumulator.signed_vaa)?;
    let price = parse_price_feed_message(&accumulator.message)?;
    authenticate_selected_update(arguments, &accumulator, vaa, price)?;
    let body = post_update_body(&accumulator)?;
    let expected_verified = expected_verified_encoded_vaa(arguments.submitter, vaa.bytes)?;
    let router = Pubkey::new_from_array(release.router_program());
    let guardian = Pubkey::find_program_address(
        &[b"GuardianSet", &vaa.guardian_set_index.to_be_bytes()],
        &router,
    )
    .0;
    let instructions = provision_instructions(
        arguments.fee_payer,
        arguments.submitter,
        arguments.encoded_vaa,
        guardian,
        router,
        intent.encoded_vaa_rent_lamports,
        &accumulator.signed_vaa,
    )?;
    let message_bytes = decode_canonical_base64(&intent.message_base64, "Router message")?;
    let message: Message = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("durable Router message: {error}")))?;
    let recent_blockhash = Hash::from_str(&intent.recent_blockhash)
        .map_err(|error| Error::new(format!("durable Router blockhash: {error}")))?;
    authenticate_message(
        &message,
        &instructions,
        arguments.fee_payer,
        arguments.submitter,
        arguments.encoded_vaa,
        recent_blockhash,
    )?;
    let placeholder = Transaction {
        signatures: vec![Signature::default(); usize::from(message.header.num_required_signatures)],
        message,
    };
    if intent.observation_slot == 0
        || intent.message_sha256 != sha256_hex(&message_bytes)
        || intent.recent_blockhash != placeholder.message.recent_blockhash.to_string()
        || intent.instructions
            != instructions
                .iter()
                .map(instruction_evidence)
                .collect::<Vec<_>>()
        || intent.expected_wire_bytes
            != bincode::serialize(&placeholder)
                .map_err(|error| Error::new(format!("size durable Router packet: {error}")))?
                .len()
        || intent.expected_wire_bytes > SOLANA_PACKET_BYTES_V1
        || intent.encoded_vaa_bytes != ENCODED_VAA_HEADER_BYTES_V1 + accumulator.signed_vaa.len()
        || intent.total_fee_payer_debit_lamports
            != intent
                .encoded_vaa_rent_lamports
                .checked_add(intent.exact_fee_lamports)
                .ok_or_else(|| Error::new("durable rent+fee overflow"))?
        || intent.expected_verified_encoded_vaa_base64 != BASE64.encode(expected_verified)
        || intent.post_update_body_base64 != BASE64.encode(body)
        || intent.hermes.signed_vaa_sha256 != sha256_hex(&accumulator.signed_vaa)
        || intent.hermes.guardian_set_index != vaa.guardian_set_index
        || intent.hermes.signature_count != vaa.signature_count
        || intent.hermes.wormhole_sequence != vaa.sequence
        || intent.hermes.accumulator_slot != vaa.accumulator_slot
        || intent.hermes.accumulator_ring_size != vaa.accumulator_ring_size
        || intent.hermes.merkle_root_hex != hex(&vaa.merkle_root)
        || usize::from(intent.hermes.proof_count) != accumulator.proof.len()
        || intent.price != price.price
        || intent.confidence != price.confidence
        || intent.exponent != price.exponent
        || intent.publish_time != price.publish_time
        || pubkey(&intent.guardian_set)? != guardian
    {
        return Err(Error::new(
            "durable Hermes, message, instruction, rent, or post-update derivation changed",
        ));
    }
    authenticate_prestate_shape(intent, arguments, &release)?;
    Ok(())
}

fn authenticate_message(
    message: &Message,
    instructions: &[Instruction],
    fee_payer: Pubkey,
    submitter: Pubkey,
    encoded_vaa: Pubkey,
    recent_blockhash: Hash,
) -> Result<()> {
    let rebuilt = Message::new_with_blockhash(instructions, Some(&fee_payer), &recent_blockhash);
    if message != &rebuilt
        || message.account_keys.first() != Some(&fee_payer)
        || message.header.num_required_signatures != if fee_payer == submitter { 2 } else { 3 }
        || !message
            .account_keys
            .iter()
            .take(usize::from(message.header.num_required_signatures))
            .any(|key| *key == encoded_vaa)
        || !message
            .account_keys
            .iter()
            .take(usize::from(message.header.num_required_signatures))
            .any(|key| *key == submitter)
    {
        return Err(Error::new(
            "Router message differed from the exact semantic instruction owner or signer set",
        ));
    }
    Ok(())
}

fn authenticate_prestate_shape(
    intent: &IntentV1,
    arguments: &ArgumentsV1,
    release: &dclutch_pyth_svm::PythReleaseV1,
) -> Result<()> {
    let expected = BTreeMap::from([
        (
            "receiver_program",
            Pubkey::new_from_array(release.receiver_program()),
        ),
        (
            "receiver_programdata",
            Pubkey::new_from_array(release.receiver_programdata()),
        ),
        (
            "receiver_config",
            Pubkey::new_from_array(release.receiver_config()),
        ),
        (
            "router_program",
            Pubkey::new_from_array(release.router_program()),
        ),
        (
            "router_programdata",
            Pubkey::new_from_array(release.router_programdata()),
        ),
        ("guardian_set", pubkey(&intent.guardian_set)?),
        ("submitter", arguments.submitter),
        ("fee_payer", arguments.fee_payer),
        ("encoded_vaa", arguments.encoded_vaa),
        ("update_account", arguments.update_account),
        ("system_program", system_program::ID),
    ]);
    if intent.prestate.len() != expected.len() {
        return Err(Error::new("durable Pyth prestate label set changed"));
    }
    for (label, key) in expected {
        let state = intent
            .prestate
            .get(label)
            .ok_or_else(|| Error::new(format!("durable Pyth prestate omitted {label}")))?;
        if pubkey(&state.address)? != key {
            return Err(Error::new(format!(
                "durable Pyth prestate substituted {label}"
            )));
        }
    }
    for label in ["encoded_vaa", "update_account"] {
        let state = &intent.prestate[label];
        if state.owner != system_program::ID.to_string()
            || state.lamports != 0
            || state.executable
            || state.data_len != 0
            || state.data_sha256 != sha256_hex(&[])
        {
            return Err(Error::new(format!(
                "durable fresh {label} prestate was not vacant"
            )));
        }
    }
    Ok(())
}

fn authenticate_existing_journal(journal: &JournalV1, arguments: &ArgumentsV1) -> Result<()> {
    if journal.schema != JOURNAL_SCHEMA_V1
        || journal.cluster != "devnet"
        || journal.rpc_url != arguments.origin.redacted_url()
        || journal.intent_sha256 != sha256_hex(&serde_json::to_vec(&journal.intent)?)
        || journal.envelope_sha256 != envelope_sha256(journal)?
    {
        return Err(Error::new(
            "Pyth journal schema, cluster, endpoint, intent digest, or envelope changed",
        ));
    }
    authenticate_intent(&journal.intent, arguments)?;
    authenticate_phase_shape(journal)
}

fn authenticate_phase_shape(journal: &JournalV1) -> Result<()> {
    let signed = journal.signed_packet_base64.is_some()
        && journal.signed_packet_sha256.is_some()
        && journal.expected_signature.is_some();
    let valid = match journal.phase {
        PhaseV1::Planned => !signed && journal.finalized.is_none(),
        PhaseV1::Submitted => signed && journal.finalized.is_none(),
        PhaseV1::Finalized => signed && journal.finalized.is_some(),
    };
    if !valid {
        return Err(Error::new("Pyth journal phase envelope was not canonical"));
    }
    Ok(())
}

fn resume(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    journal: &mut JournalV1,
    file: &mut JournalFileV1,
) -> Result<()> {
    authenticate_existing_journal(journal, arguments)?;
    authenticate_genesis(rpc, &arguments.origin)?;
    match journal.phase {
        PhaseV1::Planned => {
            require_fresh_prestate(rpc, arguments, &journal.intent)?;
            if arguments.execute {
                sign_submit_finalize(rpc, arguments, journal, file)?;
            }
        }
        PhaseV1::Submitted => {
            let signature = journal
                .expected_signature
                .clone()
                .ok_or_else(|| Error::new("submitted Pyth journal omitted signature"))?;
            if finalized_transaction(rpc, &signature)?.is_none() {
                return Err(Error::new(format!(
                    "REFUSED: Router packet {signature} is not in finalized history; recovery is poll-only and will not submit it again"
                )));
            }
            finalize_known_signature(rpc, arguments, journal, &signature)?;
            file.persist(journal)?;
            write_facts(arguments, journal)?;
        }
        PhaseV1::Finalized => {
            verify_persisted_finalized(rpc, arguments, journal)?;
            write_facts(arguments, journal)?;
        }
    }
    Ok(())
}

fn require_fresh_prestate(rpc: &mut Rpc, arguments: &ArgumentsV1, intent: &IntentV1) -> Result<()> {
    let accumulator = parse_accumulator_update(&decode_canonical_base64(
        &intent.hermes.accumulator_base64,
        "Hermes accumulator",
    )?)?;
    let vaa = parse_signed_vaa(&accumulator.signed_vaa)?;
    let snapshot = acquire_snapshot(rpc, arguments, vaa.guardian_set_index)?;
    authenticate_release_snapshot(arguments, &snapshot, vaa)?;
    if snapshot.slot < intent.observation_slot {
        return Err(Error::new("fresh Pyth snapshot preceded the durable one"));
    }
    for (label, durable) in &intent.prestate {
        let key = pubkey(&durable.address)?;
        if account_state(key, snapshot.optional(key)) != *durable {
            return Err(Error::new(format!(
                "fresh finalized {label} differed from the durable Pyth prestate"
            )));
        }
    }
    Ok(())
}

fn sign_submit_finalize(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    journal: &mut JournalV1,
    file: &mut JournalFileV1,
) -> Result<()> {
    let height = block_height(rpc)?;
    if height > journal.intent.last_valid_block_height {
        return Err(Error::new(
            "planned Router packet expired before key access; archive this unsigned journal and plan from a fresh finalized snapshot",
        ));
    }
    authenticate_genesis(rpc, &arguments.origin)?;
    // First signer-file access: the complete Hermes bytes, release snapshot,
    // message, rent, fee, prestate, facts destination, and journal are durable.
    let submitter = read_keypair(
        arguments.submitter_keypair.as_deref(),
        "submitter",
        arguments.submitter,
    )?;
    let fee_payer = if arguments.fee_payer == arguments.submitter {
        None
    } else {
        Some(read_keypair(
            arguments.fee_payer_keypair.as_deref(),
            "fee-payer",
            arguments.fee_payer,
        )?)
    };
    let encoded = read_keypair(
        arguments.encoded_vaa_keypair.as_deref(),
        "encoded-vaa",
        arguments.encoded_vaa,
    )?;
    // Possession of the still-vacant Receiver destination is proven before
    // the facts handoff, but it intentionally does not sign the Router packet.
    let _update = read_keypair(
        arguments.update_keypair.as_deref(),
        "update-account",
        arguments.update_account,
    )?;
    let payer_signer: &Keypair = fee_payer.as_ref().unwrap_or(&submitter);
    let message_bytes = decode_canonical_base64(&journal.intent.message_base64, "Router message")?;
    let message: Message = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("durable Router message: {error}")))?;
    let blockhash = Hash::from_str(&journal.intent.recent_blockhash)
        .map_err(|error| Error::new(format!("durable Router blockhash: {error}")))?;
    if message.recent_blockhash != blockhash {
        return Err(Error::new(
            "durable Router message blockhash differed from its intent",
        ));
    }
    let mut transaction = Transaction::new_unsigned(message);
    let signers: Vec<&dyn Signer> = if arguments.fee_payer == arguments.submitter {
        vec![&submitter, &encoded]
    } else {
        vec![payer_signer, &submitter, &encoded]
    };
    transaction
        .try_sign(&signers, blockhash)
        .map_err(|error| Error::new(format!("sign Router packet: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("verify Router signatures: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| Error::new("Router packet omitted payer signature"))?;
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize Router packet: {error}")))?;
    if wire.len() != journal.intent.expected_wire_bytes {
        return Err(Error::new(
            "signed Router packet width differed from the durable placeholder",
        ));
    }
    journal.signed_packet_base64 = Some(BASE64.encode(&wire));
    journal.signed_packet_sha256 = Some(sha256_hex(&wire));
    journal.expected_signature = Some(signature.to_string());
    journal.phase = PhaseV1::Submitted;
    file.persist(journal)?;

    authenticate_genesis(rpc, &arguments.origin)?;
    require_fresh_prestate(rpc, arguments, &journal.intent)?;
    let returned = rpc
        .call_once(
            "sendTransaction",
            &json!([BASE64.encode(&wire), {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"finalized",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("sendTransaction signature: {error}")))?;
    if returned != signature {
        return Err(Error::new(
            "RPC returned a signature different from the exact Router packet",
        ));
    }
    wait_finalized(rpc, &signature.to_string())?;
    finalize_known_signature(rpc, arguments, journal, &signature.to_string())?;
    file.persist(journal)?;
    write_facts(arguments, journal)
}

fn finalize_known_signature(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    journal: &mut JournalV1,
    signature: &str,
) -> Result<()> {
    let evidence = authenticate_finalized_history(rpc, arguments, journal, signature, None)?;
    journal.finalized = Some(evidence);
    journal.phase = PhaseV1::Finalized;
    Ok(())
}

fn verify_persisted_finalized(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    journal: &JournalV1,
) -> Result<()> {
    let persisted = journal
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized Pyth journal omitted evidence"))?;
    let observed = authenticate_finalized_history(
        rpc,
        arguments,
        journal,
        &persisted.signature,
        Some(&persisted.poststate),
    )?;
    if &observed != persisted {
        return Err(Error::new(
            "persisted Pyth receipt differed from immutable finalized history",
        ));
    }
    Ok(())
}

fn authenticate_finalized_history(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    journal: &JournalV1,
    signature: &str,
    persisted_poststate: Option<&BTreeMap<String, AccountStateV1>>,
) -> Result<FinalizedEvidenceV1> {
    authenticate_intent(&journal.intent, arguments)?;
    let packet = authenticate_signed_packet(journal)?;
    if journal.expected_signature.as_deref() != Some(signature)
        || packet
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != Some(signature)
    {
        return Err(Error::new(
            "finalized Router signature differed from the durable packet",
        ));
    }
    let transaction = finalized_transaction(rpc, signature)?.ok_or_else(|| {
        Error::new(format!(
            "Router signature {signature} has not reached finalized history"
        ))
    })?;
    if transaction
        .pointer("/transaction/signatures/0")
        .and_then(Value::as_str)
        != Some(signature)
    {
        return Err(Error::new(
            "finalized Router history omitted the exact signature",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized Router transaction omitted slot"))?;
    if slot < journal.intent.observation_slot {
        return Err(Error::new(
            "finalized Router transaction preceded its durable observation",
        ));
    }
    let (wire_slot, history_wire) = finalized_transaction_wire(rpc, signature)?;
    let durable_wire = decode_canonical_base64(
        journal
            .signed_packet_base64
            .as_deref()
            .ok_or_else(|| Error::new("Router journal omitted signed packet"))?,
        "signed Router packet",
    )?;
    if history_wire != durable_wire || wire_slot != slot {
        return Err(Error::new(
            "finalized Router packet or slot differed from the durable submission",
        ));
    }
    let meta = transaction
        .get("meta")
        .ok_or_else(|| Error::new("finalized Router transaction omitted meta"))?;
    if meta.get("err").is_none_or(|error| !error.is_null()) {
        return Err(Error::new(format!(
            "finalized Router transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    if meta.get("returnData").is_some_and(|value| !value.is_null()) {
        return Err(Error::new(
            "Router provisioning unexpectedly returned program data",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized Router transaction omitted fee"))?;
    if fee != journal.intent.exact_fee_lamports {
        return Err(Error::new(
            "finalized Router fee differed from getFeeForMessage",
        ));
    }
    let pre_balances = exact_u64_vector(meta, "preBalances")?;
    let post_balances = exact_u64_vector(meta, "postBalances")?;
    let finalized_balances = authenticate_balance_vector(
        &packet.message,
        &journal.intent,
        &pre_balances,
        &post_balances,
    )?;
    let poststate = match persisted_poststate {
        Some(poststate) => {
            authenticate_poststate(poststate, arguments, &journal.intent, &finalized_balances)?;
            poststate.clone()
        }
        None => {
            observe_exact_poststate(rpc, arguments, &journal.intent, slot, &finalized_balances)?
        }
    };
    let facts_sha256 = sha256_hex(&facts_bytes(journal)?);
    Ok(FinalizedEvidenceV1 {
        signature: signature.into(),
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        signed_packet_sha256: sha256_hex(&history_wire),
        pre_balances,
        post_balances,
        poststate,
        facts_sha256,
    })
}

fn authenticate_signed_packet(journal: &JournalV1) -> Result<Transaction> {
    let wire = decode_canonical_base64(
        journal
            .signed_packet_base64
            .as_deref()
            .ok_or_else(|| Error::new("signed Router journal omitted packet"))?,
        "signed Router packet",
    )?;
    let expected_digest = journal
        .signed_packet_sha256
        .as_deref()
        .ok_or_else(|| Error::new("signed Router journal omitted packet digest"))?;
    let expected_signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("signed Router journal omitted expected signature"))?;
    authenticate_signed_packet_bytes(
        &wire,
        expected_digest,
        journal.intent.expected_wire_bytes,
        &journal.intent.message_base64,
        expected_signature,
    )
}

fn authenticate_signed_packet_bytes(
    wire: &[u8],
    expected_digest: &str,
    expected_wire_bytes: usize,
    expected_message_base64: &str,
    expected_signature: &str,
) -> Result<Transaction> {
    if sha256_hex(wire) != expected_digest || wire.len() != expected_wire_bytes {
        return Err(Error::new(
            "signed Router packet digest or exact width changed",
        ));
    }
    let transaction: Transaction = bincode::deserialize(wire)
        .map_err(|error| Error::new(format!("signed Router packet: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("signed Router signatures: {error}")))?;
    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|error| Error::new(format!("signed Router message: {error}")))?;
    if BASE64.encode(&message_bytes) != expected_message_base64
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != Some(expected_signature)
    {
        return Err(Error::new(
            "signed Router packet message or expected signature changed",
        ));
    }
    Ok(transaction)
}

fn authenticate_balance_vector(
    message: &Message,
    intent: &IntentV1,
    pre: &[u64],
    post: &[u64],
) -> Result<BTreeMap<Pubkey, u64>> {
    if pre.len() != message.account_keys.len() || post.len() != message.account_keys.len() {
        return Err(Error::new(
            "Router balance vectors did not match exact message keys",
        ));
    }
    let mut durable = BTreeMap::new();
    for state in intent.prestate.values() {
        let key = pubkey(&state.address)?;
        if durable
            .insert(key, state.lamports)
            .is_some_and(|prior| prior != state.lamports)
        {
            return Err(Error::new(
                "aliased durable Router prestates disagreed on lamports",
            ));
        }
    }
    let fee_payer = pubkey(&intent.fee_payer)?;
    let encoded = pubkey(&intent.encoded_vaa)?;
    let mut outputs = BTreeMap::new();
    let mut pre_sum = 0_u128;
    let mut post_sum = 0_u128;
    for ((key, before), after) in message.account_keys.iter().zip(pre).zip(post) {
        if durable.get(key).copied() != Some(*before) {
            return Err(Error::new(format!(
                "Router pre-balance for {key} differed from durable prestate"
            )));
        }
        let mut expected = *before;
        if *key == fee_payer {
            expected = expected
                .checked_sub(intent.total_fee_payer_debit_lamports)
                .ok_or_else(|| Error::new("Router payer balance underflow"))?;
        }
        if *key == encoded {
            expected = expected
                .checked_add(intent.encoded_vaa_rent_lamports)
                .ok_or_else(|| Error::new("Router EncodedVaa balance overflow"))?;
        }
        if *after != expected {
            return Err(Error::new(format!(
                "Router lamport delta for {key} was not exact rent/fee arithmetic"
            )));
        }
        pre_sum += u128::from(*before);
        post_sum += u128::from(*after);
        outputs.insert(*key, *after);
    }
    if pre_sum.checked_sub(post_sum) != Some(u128::from(intent.exact_fee_lamports)) {
        return Err(Error::new(
            "Router whole-message balances did not conserve lamports except exact fee",
        ));
    }
    Ok(outputs)
}

fn observe_exact_poststate(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    intent: &IntentV1,
    minimum_slot: u64,
    balances: &BTreeMap<Pubkey, u64>,
) -> Result<BTreeMap<String, AccountStateV1>> {
    let keys = [arguments.encoded_vaa, arguments.update_account];
    let (_slot, accounts) = rpc.finalized_accounts(&keys, minimum_slot)?;
    let encoded = accounts[0]
        .as_ref()
        .ok_or_else(|| Error::new("finalized Router packet omitted EncodedVaa poststate"))?;
    if accounts[1].is_some() {
        return Err(Error::new(
            "Router provisioning consumed the Receiver update signer that must remain vacant",
        ));
    }
    let expected = decode_canonical_base64(
        &intent.expected_verified_encoded_vaa_base64,
        "expected verified EncodedVaa",
    )?;
    if encoded.owner != pubkey(&intent.router_program)?
        || encoded.executable
        || encoded.lamports != intent.encoded_vaa_rent_lamports
        || encoded.data != expected
        || VerifiedEncodedVaaV1::parse(&encoded.data).is_err()
    {
        return Err(Error::new(
            "finalized EncodedVaa owner, privilege, rent, exact bytes, or verified shape refused",
        ));
    }
    let mut poststate = project_wallet_poststates(arguments, intent, balances)?;
    poststate.insert(
        "encoded_vaa".into(),
        account_state(arguments.encoded_vaa, Some(encoded)),
    );
    poststate.insert(
        "update_account".into(),
        account_state(arguments.update_account, None),
    );
    authenticate_poststate(&poststate, arguments, intent, balances)?;
    Ok(poststate)
}

fn project_wallet_poststates(
    arguments: &ArgumentsV1,
    intent: &IntentV1,
    balances: &BTreeMap<Pubkey, u64>,
) -> Result<BTreeMap<String, AccountStateV1>> {
    let mut output = BTreeMap::new();
    for (label, key) in [
        ("submitter", arguments.submitter),
        ("fee_payer", arguments.fee_payer),
    ] {
        let before = intent
            .prestate
            .get(label)
            .ok_or_else(|| Error::new(format!("Pyth prestate omitted {label}")))?;
        let lamports = balances
            .get(&key)
            .copied()
            .ok_or_else(|| Error::new(format!("Router balance vector omitted {label}")))?;
        output.insert(
            label.into(),
            account_state(
                key,
                Some(&RpcAccount {
                    owner: system_program::ID,
                    lamports,
                    executable: false,
                    rent_epoch: before.rent_epoch,
                    data: Vec::new(),
                }),
            ),
        );
    }
    Ok(output)
}

fn authenticate_poststate(
    poststate: &BTreeMap<String, AccountStateV1>,
    arguments: &ArgumentsV1,
    intent: &IntentV1,
    balances: &BTreeMap<Pubkey, u64>,
) -> Result<()> {
    if poststate.len() != 4 {
        return Err(Error::new(
            "Router poststate must contain exactly four labels",
        ));
    }
    let wallets = project_wallet_poststates(arguments, intent, balances)?;
    if poststate.get("submitter") != wallets.get("submitter")
        || poststate.get("fee_payer") != wallets.get("fee_payer")
    {
        return Err(Error::new(
            "Router wallet poststate differed from immutable balance history",
        ));
    }
    let encoded = poststate
        .get("encoded_vaa")
        .ok_or_else(|| Error::new("Router poststate omitted EncodedVaa"))?;
    let expected = decode_canonical_base64(
        &intent.expected_verified_encoded_vaa_base64,
        "expected verified EncodedVaa",
    )?;
    if pubkey(&encoded.address)? != arguments.encoded_vaa
        || encoded.owner != intent.router_program
        || encoded.executable
        || encoded.lamports != intent.encoded_vaa_rent_lamports
        || encoded.data_len != expected.len()
        || encoded.data_sha256 != sha256_hex(&expected)
    {
        return Err(Error::new(
            "persisted verified EncodedVaa poststate changed",
        ));
    }
    let update = poststate
        .get("update_account")
        .ok_or_else(|| Error::new("Router poststate omitted Receiver update"))?;
    if *update != account_state(arguments.update_account, None) {
        return Err(Error::new(
            "persisted Receiver update poststate was not vacant",
        ));
    }
    Ok(())
}

fn facts(journal: &JournalV1) -> Result<ProducerPythFactsV1> {
    Ok(ProducerPythFactsV1 {
        format: FACTS_SCHEMA_V1.into(),
        encoded_vaa: journal.intent.encoded_vaa.clone(),
        update_account: journal.intent.update_account.clone(),
        post_update_body_base64: journal.intent.post_update_body_base64.clone(),
    })
}

fn facts_bytes(journal: &JournalV1) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&facts(journal)?)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_facts(arguments: &ArgumentsV1, journal: &JournalV1) -> Result<()> {
    if journal.phase != PhaseV1::Finalized {
        return Err(Error::new(
            "Pyth facts are published only after finalized Router verification",
        ));
    }
    let bytes = facts_bytes(journal)?;
    let evidence = journal
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized Pyth journal omitted receipt"))?;
    if sha256_hex(&bytes) != evidence.facts_sha256 {
        return Err(Error::new("finalized Pyth facts digest changed"));
    }
    write_create_or_exact(&arguments.facts_output, &bytes, "Pyth facts")
}

fn finalized_transaction(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
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
            "encoding":"json",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(Error::new(
            "finalized Router signature omitted finalized transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn finalized_transaction_wire(rpc: &mut Rpc, signature: &str) -> Result<(u64, Vec<u8>)> {
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
            "finalized Router signature omitted exact packet history",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("exact Router history omitted slot"))?;
    let tuple = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("exact Router history packet was not an encoding tuple"))?;
    if tuple.len() != 2 || tuple[1].as_str() != Some("base64") {
        return Err(Error::new(
            "exact Router history did not select base64 encoding",
        ));
    }
    let wire = BASE64
        .decode(
            tuple[0]
                .as_str()
                .ok_or_else(|| Error::new("exact Router packet was not a string"))?,
        )
        .map_err(|error| Error::new(format!("exact Router packet base64: {error}")))?;
    Ok((slot, wire))
}

fn wait_finalized(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + FINALITY_WAIT_V1;
    while Instant::now() < deadline {
        if finalized_transaction(rpc, signature)?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "Router transaction {signature} did not finalize within 300 seconds; its durable signature is retained for poll-only resume"
    )))
}

fn latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let result = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = result
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("latest blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((blockhash, last_valid))
}

fn block_height(rpc: &mut Rpc) -> Result<u64> {
    rpc.call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))
}

fn fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact fee"))
}

fn authenticate_genesis(rpc: &mut Rpc, origin: &ClusterOriginV1) -> Result<()> {
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    if genesis != DEVNET_GENESIS_HASH {
        return Err(Error::new(format!(
            "Pyth provisioner expected devnet genesis {DEVNET_GENESIS_HASH}, observed {genesis}"
        )));
    }
    origin.authenticate_genesis(&genesis)
}

fn read_keypair(path: Option<&Path>, label: &str, expected: Pubkey) -> Result<Keypair> {
    let path = path.ok_or_else(|| Error::new(format!("--{label}-keypair is required")))?;
    let keypair = Keypair::new_from_array(campaign::read_keypair_file(path, label)?);
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair does not expand to its declared public key"
        )));
    }
    Ok(keypair)
}

fn exact_u64_vector(meta: &Value, label: &str) -> Result<Vec<u64>> {
    meta.get(label)
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(format!("finalized Router meta omitted {label}")))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| Error::new(format!("Router {label} contained a non-u64")))
        })
        .collect()
}

fn account_state(address: Pubkey, account: Option<&RpcAccount>) -> AccountStateV1 {
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
        address: address.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        rent_epoch,
        data_len: data.len(),
        data_sha256: sha256_hex(data),
        account_sha256: hex(&exact.finalize()),
    }
}

impl ProvisionSnapshotV1 {
    fn optional(&self, key: Pubkey) -> Option<&RpcAccount> {
        self.accounts.get(&key).and_then(Option::as_ref)
    }

    fn required(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.optional(key)
            .ok_or_else(|| Error::new(format!("finalized snapshot omitted {label} {key}")))
    }
}

impl JournalFileV1 {
    fn vacant(path: PathBuf) -> Self {
        Self {
            path,
            expected_bytes: None,
        }
    }

    fn existing(path: PathBuf, expected_bytes: Vec<u8>) -> Self {
        Self {
            path,
            expected_bytes: Some(expected_bytes),
        }
    }

    fn persist(&mut self, journal: &mut JournalV1) -> Result<()> {
        authenticate_phase_shape(journal)?;
        journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent)?);
        journal.envelope_sha256 = String::new();
        journal.envelope_sha256 = envelope_sha256(journal)?;
        let mut bytes = serde_json::to_vec_pretty(journal)?;
        bytes.push(b'\n');
        persist_cas(
            &self.path,
            self.expected_bytes.as_deref(),
            &bytes,
            "Pyth journal",
        )?;
        self.expected_bytes = Some(bytes);
        Ok(())
    }
}

fn envelope_sha256(journal: &JournalV1) -> Result<String> {
    let mut projected = journal.clone();
    projected.envelope_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&projected)?))
}

fn persist_cas(path: &Path, expected: Option<&[u8]>, bytes: &[u8], label: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} path omitted parent")))?;
    let name = path
        .file_name()
        .ok_or_else(|| Error::new(format!("{label} path omitted file name")))?;
    let mut lock_name = OsString::from(".");
    lock_name.push(name);
    lock_name.push(".lock");
    let lock_path = parent.join(lock_name);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| Error::new(format!("open {label} lock: {error}")))?;
    lock.try_lock().map_err(|error| {
        Error::new(format!(
            "REFUSED: another {label} writer holds lock: {error}"
        ))
    })?;
    match expected {
        Some(expected) => {
            let current = fs::read(path)
                .map_err(|error| Error::new(format!("read {label} CAS prestate: {error}")))?;
            if current != expected {
                return Err(Error::new(format!(
                    "REFUSED: {label} changed since this process read it"
                )));
            }
        }
        None if path.exists() => {
            return Err(Error::new(format!(
                "REFUSED: vacant {label} path already exists"
            )));
        }
        None => {}
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create temporary {label}: {error}")))?;
    if let Err(error) = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
        Ok(())
    })() {
        let _ = fs::remove_file(&temporary);
        return Err(Error::new(format!("persist {label}: {error}")));
    }
    Ok(())
}

fn write_create_or_exact(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if let Ok(current) = fs::read(path) {
        if current == bytes {
            return Ok(());
        }
        return Err(Error::new(format!(
            "REFUSED: existing {label} differs from finalized receipt"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} path omitted parent")))?;
    let name = path
        .file_name()
        .ok_or_else(|| Error::new(format!("{label} path omitted file name")))?;
    let temporary = parent.join(format!(
        ".{}.{}.publish.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create temporary {label}: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| Error::new(format!("write temporary {label}: {error}")))?;
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)
                .map_err(|error| Error::new(format!("remove temporary {label}: {error}")))?;
            OpenOptions::new().read(true).open(parent)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            if fs::read(path)? == bytes {
                Ok(())
            } else {
                Err(Error::new(format!(
                    "REFUSED: raced {label} differs from finalized receipt"
                )))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(Error::new(format!("publish {label}: {error}")))
        }
    }
}

fn print_journal(journal: &JournalV1) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(journal)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!("{label} may be supplied only once")));
    }
    Ok(())
}

fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute_path(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn parse_i64(value: &str, label: &str) -> Result<i64> {
    value
        .parse()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn parse_hex32(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(Error::new(format!(
            "{label} must be exactly 64 lowercase hexadecimal digits"
        )));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|error| Error::new(format!("{label}: {error}")))?;
    }
    Ok(output)
}

fn parse_genesis_pubkey(value: &str) -> Result<Pubkey> {
    pubkey(value).map_err(|error| Error::new(format!("devnet genesis hash: {error}")))
}

fn bounded_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .map_err(|error| Error::new(format!("read {label} metadata: {error}")))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(Error::new(format!(
            "{label} must be a regular file no larger than {maximum} bytes"
        )));
    }
    let bytes = fs::read(path).map_err(|error| Error::new(format!("read {label}: {error}")))?;
    if bytes.len() > maximum {
        return Err(Error::new(format!("{label} exceeded its byte bound")));
    }
    Ok(bytes)
}

fn decode_canonical_base64(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(Error::new(format!("{label} base64 was not canonical")));
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WireCursorV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WireCursorV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, width: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(width)
            .ok_or_else(|| Error::new("Pyth wire offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| Error::new("Pyth wire ended before an exact field"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::new("Pyth wire array width changed"))
    }

    fn take_u8(&mut self) -> Result<u8> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| Error::new("Pyth wire omitted byte"))
    }

    fn take_u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take_array()?))
    }

    fn take_u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take_array()?))
    }

    fn take_u64_be(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take_array()?))
    }

    fn is_empty(self) -> bool {
        self.offset == self.bytes.len()
    }
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap devnet-pyth-vaa-provision-v1 \
     --rpc-url URL --i-mean-devnet DEVNET_GENESIS \
     (--hermes-url HTTPS_BASE [--hermes-api-token-file ABSOLUTE_FILE] | \
      --hermes-response ABSOLUTE_JSON) \
     --feed-id 64_LOWERCASE_HEX --minimum-publish-time I64 --maximum-publish-time I64 \
     --submitter PUBKEY --fee-payer PUBKEY --encoded-vaa PUBKEY --update-account PUBKEY \
     --journal ABSOLUTE_JSON --facts-output ABSOLUTE_JSON \
     [--execute --submitter-keypair ABSOLUTE_JSON --fee-payer-keypair ABSOLUTE_JSON \
      --encoded-vaa-keypair ABSOLUTE_JSON --update-keypair ABSOLUTE_JSON]\n\nThe command is \
     devnet-only and defaults to a key-free, read-only plan. It exact-parses one Hermes v2 \
     accumulator, joins its feed and Merkle proof to a signed Pythnet VAA, authenticates the \
     pinned Router/Receiver release from one finalized snapshot, and plans one atomic \
     create/init/write/verify Router packet. With --execute it persists the complete unsigned \
     message, prestate, exact rent and fee before opening signer files, then persists the signed \
     packet and expected signature before one maxRetries=0 send. Recovery only polls that \
     signature. Finalized exact packet, balance, and verified-account evidence is required before \
     publishing the existing dclutch-flagship-pyth-update-facts-v1 document. The update signer is \
     authenticated but remains vacant for flagship-resolution-v1."
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price_message(feed: [u8; 32], publish_time: i64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(PRICE_FEED_MESSAGE_BYTES_V1);
        bytes.push(0);
        bytes.extend_from_slice(&feed);
        bytes.extend_from_slice(&12_345_i64.to_be_bytes());
        bytes.extend_from_slice(&99_u64.to_be_bytes());
        bytes.extend_from_slice(&(-8_i32).to_be_bytes());
        bytes.extend_from_slice(&publish_time.to_be_bytes());
        bytes.extend_from_slice(&(publish_time - 1).to_be_bytes());
        bytes.extend_from_slice(&12_300_i64.to_be_bytes());
        bytes.extend_from_slice(&100_u64.to_be_bytes());
        assert_eq!(bytes.len(), PRICE_FEED_MESSAGE_BYTES_V1);
        bytes
    }

    fn signed_vaa(root: [u8; 20]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(1);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes.push(3);
        for index in 0..3_u8 {
            bytes.push(index);
            bytes.extend_from_slice(&[index + 1; VAA_SIGNATURE_BYTES_V1 - 1]);
        }
        bytes.extend_from_slice(&1_000_u32.to_be_bytes());
        bytes.extend_from_slice(&7_u32.to_be_bytes());
        bytes.extend_from_slice(&PYTHNET_EMITTER_CHAIN_V1.to_be_bytes());
        bytes.extend_from_slice(&PYTHNET_EMITTER_V1);
        bytes.extend_from_slice(&42_u64.to_be_bytes());
        bytes.push(0);
        bytes.extend_from_slice(WORMHOLE_ACCUMULATOR_MAGIC_V1);
        bytes.push(0);
        bytes.extend_from_slice(&123_u64.to_be_bytes());
        bytes.extend_from_slice(&64_u32.to_be_bytes());
        bytes.extend_from_slice(&root);
        bytes
    }

    fn accumulator(feed: [u8; 32], publish_time: i64) -> Vec<u8> {
        let message = price_message(feed, publish_time);
        let vaa = signed_vaa(keccak160(&[&[0], &message]));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HERMES_ACCUMULATOR_MAGIC_V1);
        bytes.extend_from_slice(&[1, 0, 0, 0]);
        bytes.extend_from_slice(&u16::try_from(vaa.len()).expect("VAA width").to_be_bytes());
        bytes.extend_from_slice(&vaa);
        bytes.push(1);
        bytes.extend_from_slice(
            &u16::try_from(message.len())
                .expect("message width")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&message);
        bytes.push(0);
        bytes
    }

    #[test]
    fn exact_hermes_accumulator_derives_existing_facts_body() {
        let feed = [0x44; 32];
        let update_bytes = accumulator(feed, 1_234);
        let response = serde_json::to_vec(&json!({
            "binary": {"encoding":"base64", "data":[BASE64.encode(&update_bytes)]},
            "parsed": null
        }))
        .expect("Hermes response");
        let decoded = parse_hermes_response(&response).expect("Hermes binary");
        assert_eq!(decoded, update_bytes);
        let update = parse_accumulator_update(&decoded).expect("accumulator");
        let vaa = parse_signed_vaa(&update.signed_vaa).expect("VAA");
        let price = parse_price_feed_message(&update.message).expect("price");
        assert_eq!(vaa.guardian_set_index, 0);
        assert_eq!(vaa.signature_count, 3);
        assert_eq!(price.feed_id, feed);
        assert_eq!(price.publish_time, 1_234);
        let body = post_update_body(&update).expect("PostUpdateParams");
        let parsed = PostUpdateParamsView::parse(&body).expect("receiver body");
        assert_eq!(parsed.message(), update.message);
        assert_eq!(parsed.proof_count(), 0);
        assert_eq!(parsed.treasury_id(), 0);
    }

    #[test]
    fn accumulator_and_merkle_substitutions_refuse() {
        let feed = [0x45; 32];
        let exact = accumulator(feed, 1_500);
        let update = parse_accumulator_update(&exact).expect("accumulator");
        let vaa = parse_signed_vaa(&update.signed_vaa).expect("VAA");
        let arguments_feed = feed;
        let mut hostile = update.clone();
        hostile.message[33] ^= 1;
        let dummy_origin =
            ClusterOriginV1::parse("https://api.devnet.solana.com", Some(DEVNET_GENESIS_HASH))
                .expect("devnet origin");
        let arguments = ArgumentsV1 {
            origin: dummy_origin,
            hermes: HermesInputV1::Offline {
                response: PathBuf::from("/tmp/hermes.json"),
            },
            feed_id: arguments_feed,
            minimum_publish_time: 1_400,
            maximum_publish_time: 1_600,
            submitter: Pubkey::new_from_array([1; 32]),
            fee_payer: Pubkey::new_from_array([2; 32]),
            encoded_vaa: Pubkey::new_from_array([3; 32]),
            update_account: Pubkey::new_from_array([4; 32]),
            journal: PathBuf::from("/tmp/journal.json"),
            facts_output: PathBuf::from("/tmp/facts.json"),
            execute: false,
            submitter_keypair: None,
            fee_payer_keypair: None,
            encoded_vaa_keypair: None,
            update_keypair: None,
        };
        assert!(
            authenticate_selected_update(
                &arguments,
                &hostile,
                vaa,
                parse_price_feed_message(&hostile.message).expect("hostile price")
            )
            .is_err()
        );
        let mut trailing = exact;
        trailing.push(0);
        assert!(parse_accumulator_update(&trailing).is_err());
    }

    #[test]
    fn current_three_signature_vaa_is_one_atomic_packet() {
        let feed = [0x46; 32];
        let update = parse_accumulator_update(&accumulator(feed, 2_000)).expect("update");
        let payer = Pubkey::new_from_array([1; 32]);
        let submitter = Pubkey::new_from_array([2; 32]);
        let encoded = Pubkey::new_from_array([3; 32]);
        let guardian = Pubkey::new_from_array([4; 32]);
        let router = Pubkey::new_from_array([5; 32]);
        let instructions = provision_instructions(
            payer,
            submitter,
            encoded,
            guardian,
            router,
            1_000_000,
            &update.signed_vaa,
        )
        .expect("instructions");
        assert_eq!(instructions.len(), 4);
        assert_eq!(instructions[0].program_id, system_program::ID);
        assert_eq!(
            instructions[1].data,
            anchor_discriminator(b"global:init_encoded_vaa")
        );
        assert_eq!(
            instructions[3].data,
            anchor_discriminator(b"global:verify_encoded_vaa_v1")
        );
        let blockhash = Hash::new_from_array([0x48; 32]);
        let message = Message::new_with_blockhash(&instructions, Some(&payer), &blockhash);
        authenticate_message(
            &message,
            &instructions,
            payer,
            submitter,
            encoded,
            blockhash,
        )
        .expect("message owner");
        let wire = bincode::serialize(&Transaction {
            signatures: vec![
                Signature::default();
                usize::from(message.header.num_required_signatures)
            ],
            message,
        })
        .expect("wire");
        assert!(wire.len() <= SOLANA_PACKET_BYTES_V1, "{}", wire.len());
    }

    #[test]
    fn offline_signed_packet_accepts_exact_blockhash_and_refuses_substitution() {
        let update = parse_accumulator_update(&accumulator([0x49; 32], 2_100)).expect("update");
        let payer = Keypair::new();
        let submitter = Keypair::new();
        let encoded = Keypair::new();
        let guardian = Pubkey::new_from_array([4; 32]);
        let router = Pubkey::new_from_array([5; 32]);
        let instructions = provision_instructions(
            payer.pubkey(),
            submitter.pubkey(),
            encoded.pubkey(),
            guardian,
            router,
            1_000_000,
            &update.signed_vaa,
        )
        .expect("instructions");
        let blockhash = Hash::new_from_array([0x50; 32]);
        let message = Message::new_with_blockhash(&instructions, Some(&payer.pubkey()), &blockhash);
        authenticate_message(
            &message,
            &instructions,
            payer.pubkey(),
            submitter.pubkey(),
            encoded.pubkey(),
            blockhash,
        )
        .expect("exact blockhash message");
        let message_bytes = bincode::serialize(&message).expect("message bytes");
        let mut transaction = Transaction::new_unsigned(message);
        transaction
            .try_sign(&[&payer, &submitter, &encoded], blockhash)
            .expect("sign exact packet");
        let wire = bincode::serialize(&transaction).expect("packet bytes");
        authenticate_signed_packet_bytes(
            &wire,
            &sha256_hex(&wire),
            wire.len(),
            &BASE64.encode(&message_bytes),
            &transaction.signatures[0].to_string(),
        )
        .expect("exact signed packet");

        let hostile_blockhash = Hash::new_from_array([0x51; 32]);
        let hostile_message =
            Message::new_with_blockhash(&instructions, Some(&payer.pubkey()), &hostile_blockhash);
        let mut hostile = Transaction::new_unsigned(hostile_message);
        hostile
            .try_sign(&[&payer, &submitter, &encoded], hostile_blockhash)
            .expect("sign self-consistent hostile packet");
        let hostile_wire = bincode::serialize(&hostile).expect("hostile packet bytes");
        assert!(
            authenticate_signed_packet_bytes(
                &hostile_wire,
                &sha256_hex(&hostile_wire),
                hostile_wire.len(),
                &BASE64.encode(&message_bytes),
                &hostile.signatures[0].to_string(),
            )
            .is_err()
        );
    }

    #[test]
    fn verified_projection_binds_authority_and_exact_vaa() {
        let update = parse_accumulator_update(&accumulator([0x47; 32], 3_000)).expect("update");
        let authority = Pubkey::new_from_array([9; 32]);
        let bytes = expected_verified_encoded_vaa(authority, &update.signed_vaa)
            .expect("verified projection");
        let view = VerifiedEncodedVaaV1::parse(&bytes).expect("verified view");
        assert_eq!(view.write_authority(), authority.to_bytes());
        assert_eq!(view.signed_vaa(), update.signed_vaa);
        let mut substituted = bytes;
        substituted[9] ^= 1;
        assert_ne!(
            VerifiedEncodedVaaV1::parse(&substituted)
                .expect("shape remains valid")
                .write_authority(),
            authority.to_bytes()
        );
    }

    #[test]
    fn hermes_json_duplicate_unknown_and_noncanonical_base64_refuse() {
        let duplicate = br#"{"binary":{"encoding":"base64","data":["UE5BVQ=="]},"binary":{}}"#;
        assert!(parse_hermes_response(duplicate).is_err());
        let unknown = br#"{"binary":{"encoding":"base64","data":["UE5BVQ=="]},"route":"hostile"}"#;
        assert!(parse_hermes_response(unknown).is_err());
        let padded = br#"{"binary":{"encoding":"base64","data":["UE5BVQ"]}}"#;
        assert!(parse_hermes_response(padded).is_err());
    }
}
